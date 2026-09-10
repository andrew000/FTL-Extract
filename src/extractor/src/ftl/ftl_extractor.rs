use crate::ftl::code_extractor::{extract_fluent_keys, sort_fluent_keys_by_path};
use crate::ftl::consts::{CommentsKeyModes, LineEndings};
use crate::ftl::diagnostics::ExtractionDiagnostic;
use crate::ftl::ftl_importer::import_ftl_from_dir;
use crate::ftl::matcher::{FluentEntry, FluentKey};
use crate::ftl::process::commentator::comment_ftl_key;
use crate::ftl::process::kwargs_extractor::extract_kwargs;
use crate::ftl::process::serializer::generate_ftl;
use crate::ftl::utils::{ExtractionStatistics, FastHashMap, FastHashSet};
use anyhow::{Result, bail};
use common::write_atomically;
use log::{debug, info, warn};
use rayon::prelude::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ExtractConfig {
    pub code_path: PathBuf,
    pub locales_path: PathBuf,
    pub languages: Vec<String>,
    pub i18n_keys: FastHashSet<String>,
    pub i18n_keys_prefix: FastHashSet<String>,
    pub exclude_dirs: FastHashSet<String>,
    pub ignore_attributes: FastHashSet<String>,
    pub ignore_kwargs: FastHashSet<String>,
    pub default_ftl_file: PathBuf,
    pub comment_junks: bool,
    pub comment_keys_mode: CommentsKeyModes,
    pub line_endings: LineEndings,
    pub dry_run: bool,
    pub cache: bool,
    pub cache_path: Option<PathBuf>,
    pub clear_cache: bool,
    pub allow_parse_errors: bool,
}

pub fn extract(config: ExtractConfig) -> Result<ExtractionStatistics> {
    let mut statistics = ExtractionStatistics::new();

    // Statistics for each language
    for lang in &config.languages {
        statistics.init_lang(lang);
    }

    let start = std::time::Instant::now();
    let extraction = extract_fluent_keys(
        &config.code_path,
        config.i18n_keys.clone(),
        config.i18n_keys_prefix.clone(),
        &config.exclude_dirs,
        config.ignore_attributes.clone(),
        config.ignore_kwargs.clone(),
        &config.default_ftl_file,
        config.cache,
        config.cache_path.as_deref(),
        config.clear_cache,
    )?;
    statistics.py_files_count += extraction.py_files_with_keys;
    info!(target: "extractor::ftl", "FTL Extraction completed in {:.3?}s.", start.elapsed().as_secs_f64());

    // Abort before touching any .ftl file: keys from an unparseable file would otherwise look
    // unused and get commented out of every locale.
    check_extraction_diagnostics(&extraction.diagnostics, config.allow_parse_errors)?;

    let in_code_fluent_keys = extraction.keys;
    statistics.ftl_in_code_keys_count = in_code_fluent_keys.len();

    let start = std::time::Instant::now();
    let results: Result<Vec<ExtractionStatistics>> = config
        .languages
        .par_iter()
        .map(|lang| {
            let mut thread_local_stats = ExtractionStatistics::new();
            thread_local_stats.init_lang(lang);

            process_language(lang, &in_code_fluent_keys, &config, &mut thread_local_stats)?;

            Ok(thread_local_stats)
        })
        .collect();

    let results = results?;

    info!(target: "extractor::ftl", "FTL Processing completed in {:.3?}s.", start.elapsed().as_secs_f64());

    // Merge statistics back into the main object
    for stat in results {
        statistics.merge(stat);
    }

    Ok(statistics)
}

/// Fails when extraction reported problems that make the key set untrustworthy
fn check_extraction_diagnostics(
    diagnostics: &[ExtractionDiagnostic],
    allow_parse_errors: bool,
) -> Result<()> {
    if diagnostics.is_empty() {
        return Ok(());
    }

    let (file_errors, conflicts): (Vec<_>, Vec<_>) = diagnostics
        .iter()
        .partition(|diagnostic| diagnostic.is_file_error());

    let failing = if allow_parse_errors {
        for diagnostic in &file_errors {
            warn!(target: "extractor::ftl", "Skipping Python file: {diagnostic}");
        }
        conflicts
    } else {
        diagnostics.iter().collect()
    };

    if failing.is_empty() {
        return Ok(());
    }

    let noun = if failing.len() == 1 {
        "problem"
    } else {
        "problems"
    };
    let mut message = format!(
        "Extraction aborted: {} {noun} found in Python sources, no .ftl files were written.",
        failing.len()
    );
    for diagnostic in &failing {
        let _ = write!(message, "\n  - {diagnostic}");
    }
    if !allow_parse_errors && !file_errors.is_empty() {
        message.push_str(
            "\nPass --allow-parse-errors to skip unreadable or unparseable files and continue.",
        );
    }

    bail!(message)
}

fn process_language(
    lang: &String,
    in_code_fluent_keys: &FastHashMap<String, FluentKey>,
    config: &ExtractConfig,
    statistics: &mut ExtractionStatistics,
) -> Result<()> {
    let (mut stored_fluent_keys, stored_terms, mut leave_as_is) =
        import_ftl_from_dir(&config.locales_path, lang, statistics)?;
    let lang_dir = config.locales_path.join(lang);

    let mut keys_to_comment: FastHashMap<String, FluentKey> = FastHashMap::default();
    let mut keys_to_add: FastHashMap<String, FluentKey> = FastHashMap::default();

    // Compare Code Keys vs Stored Keys (Path mismatch & New keys)
    for (key, fluent_key) in in_code_fluent_keys.iter() {
        if let Some(stored_key) = stored_fluent_keys.get_mut(key) {
            if fluent_key.path != stored_key.path {
                // Path changed: comment old, add new
                if let Some(old_key) = stored_fluent_keys.remove(key) {
                    keys_to_comment.insert(key.clone(), old_key);
                }
                keys_to_add.insert(key.clone(), fluent_key.clone());

                *statistics.ftl_keys_commented.get_mut(lang).unwrap() += 1;
                *statistics.ftl_keys_updated.get_mut(lang).unwrap() += 1;
            } else {
                // Update code path for reference
                stored_key.code_path = fluent_key.code_path.clone();
            }
        } else {
            // New key
            keys_to_add.insert(key.clone(), fluent_key.clone());
            *statistics.ftl_keys_added.get_mut(lang).unwrap() += 1;
        }
    }

    // Compare Key Kwargs
    let mut depend_keys: FastHashSet<String> = FastHashSet::default();

    // Mismatches are collected first and removed afterwards, so the stored map stays
    // borrowed for reference resolution instead of being copied before the loop.
    let mut kwargs_mismatches: Vec<(&String, &FluentKey)> = Vec::new();

    for (key, fluent_key) in in_code_fluent_keys.iter() {
        let Some(stored_key) = stored_fluent_keys.get(key) else {
            continue;
        };

        let code_args = extract_kwargs(
            fluent_key,
            &stored_terms,
            in_code_fluent_keys,
            &mut depend_keys,
        )?;

        let stored_args = extract_kwargs(
            stored_key,
            &stored_terms,
            &stored_fluent_keys,
            &mut depend_keys,
        )?;

        if code_args != stored_args {
            kwargs_mismatches.push((key, fluent_key));
        }
    }

    for (key, fluent_key) in kwargs_mismatches {
        if let Some(stored_key) = stored_fluent_keys.remove(key) {
            keys_to_comment.insert(key.clone(), stored_key);
            keys_to_add.insert(key.clone(), fluent_key.clone());

            *statistics.ftl_keys_commented.get_mut(lang).unwrap() += 1;
            *statistics.ftl_keys_updated.get_mut(lang).unwrap() += 1;
        }
    }

    // Identify obsolete keys (in stored but not in code)
    stored_fluent_keys.retain(|key, val| {
        if in_code_fluent_keys.contains_key(key) || depend_keys.contains(key) {
            true
        } else {
            keys_to_comment.insert(key.clone(), val.clone());
            *statistics.ftl_keys_commented.get_mut(lang).unwrap() += 1;
            false
        }
    });

    handle_comments_and_junk(
        &mut keys_to_comment,
        &mut keys_to_add,
        &mut leave_as_is,
        &lang_dir,
        config,
        statistics,
        lang,
    );

    // Merge and Write
    write_results(
        stored_fluent_keys,
        keys_to_add,
        keys_to_comment,
        stored_terms,
        leave_as_is,
        &lang_dir,
        config,
        statistics,
        lang,
    )?;

    Ok(())
}
fn handle_comments_and_junk(
    keys_to_comment: &mut FastHashMap<String, FluentKey>,
    keys_to_add: &mut FastHashMap<String, FluentKey>,
    leave_as_is: &mut Vec<FluentKey>,
    lang_dir: &Path,
    config: &ExtractConfig,
    statistics: &mut ExtractionStatistics,
    lang: &str,
) {
    match config.comment_keys_mode {
        CommentsKeyModes::Comment => {
            for fluent_key in keys_to_comment.values_mut() {
                comment_ftl_key(fluent_key);
            }
        }
        CommentsKeyModes::Warn => {
            for fluent_key in keys_to_comment.values_mut() {
                keys_to_add.remove(&fluent_key.key);
                warn!(
                    target: "extractor::ftl",
                    "Key `{}` in `{}` is not in code (kwargs mismatch or missing).",
                    fluent_key.key,
                    lang_dir.join(fluent_key.path.as_ref()).display()
                );
            }
        }
    }

    if config.comment_junks {
        for fluent_key in leave_as_is {
            if matches!(fluent_key.entry.as_ref(), FluentEntry::Junk(_)) {
                comment_ftl_key(fluent_key);
                *statistics.ftl_keys_commented.get_mut(lang).unwrap() += 1;
            }
        }
    }
}
fn write_results(
    stored_keys: FastHashMap<String, FluentKey>,
    added_keys: FastHashMap<String, FluentKey>,
    commented_keys: FastHashMap<String, FluentKey>,
    terms: FastHashMap<String, FluentKey>,
    leave_as_is: Vec<FluentKey>,
    lang_dir: &Path,
    config: &ExtractConfig,
    statistics: &mut ExtractionStatistics,
    lang: &str,
) -> Result<()> {
    let mut sorted_fluent_keys = sort_fluent_keys_by_path(stored_keys);

    // Merge all buckets into the sorted structure
    for (path, keys) in sort_fluent_keys_by_path(added_keys) {
        sorted_fluent_keys.entry(path).or_default().extend(keys);
    }
    for (path, keys) in sort_fluent_keys_by_path(commented_keys) {
        sorted_fluent_keys.entry(path).or_default().extend(keys);
    }
    for (path, keys) in sort_fluent_keys_by_path(terms) {
        sorted_fluent_keys.entry(path).or_default().extend(keys);
    }

    // Standalone comments keep their place in the files that get rewritten. A file holding
    // nothing but comments has no bucket here and is left untouched on disk.
    let mut leave_as_is_map: FastHashMap<Arc<PathBuf>, Vec<FluentKey>> = FastHashMap::default();
    for item in leave_as_is {
        leave_as_is_map
            .entry(item.path.clone())
            .or_default()
            .push(item);
    }
    for (path, keys) in sorted_fluent_keys.iter_mut() {
        if let Some(misc_entries) = leave_as_is_map.remove(path) {
            keys.extend(misc_entries);
        }
    }

    let stored_keys_count = std::sync::atomic::AtomicUsize::new(0);

    // Every file is attempted even if one fails, so the error lists all of them at once.
    let mut write_errors = sorted_fluent_keys
        .into_par_iter()
        .filter_map(|(path, keys)| {
            let full_path = lang_dir.join(path.as_ref());
            let entries = keys.len();
            let messages = keys
                .iter()
                .filter(|k| matches!(k.entry.as_ref(), FluentEntry::Message(_)))
                .count();

            let ftl_content = generate_ftl(keys);

            if config.dry_run {
                debug!(
                    "[DRY-RUN] Would write to {}. {entries} entries.",
                    full_path.display()
                )
            } else if let Err(err) = write(&full_path, ftl_content, &config.line_endings) {
                return Some(format!("{}: {err}", full_path.display()));
            } else {
                debug!("Saved {}. {entries} entries.", full_path.display());
            }

            stored_keys_count.fetch_add(messages, std::sync::atomic::Ordering::Relaxed);
            None
        })
        .collect::<Vec<_>>();

    if !write_errors.is_empty() {
        write_errors.sort_unstable();
        let noun = if write_errors.len() == 1 {
            "file"
        } else {
            "files"
        };
        bail!(
            "Failed to write {} .ftl {noun} for locale `{lang}`:\n  - {}",
            write_errors.len(),
            write_errors.join("\n  - ")
        );
    }

    *statistics.ftl_stored_keys_count.get_mut(lang).unwrap() +=
        stored_keys_count.load(std::sync::atomic::Ordering::Relaxed);

    Ok(())
}

fn normalize_line_endings(s: String, line_endings: &LineEndings) -> String {
    match line_endings {
        LineEndings::Default => s,
        LineEndings::LF => s.replace("\r\n", "\n").replace('\r', "\n"),
        LineEndings::CR => s.replace("\r\n", "\r").replace('\n', "\r"),
        LineEndings::CRLF => s.replace('\r', "").replace('\n', "\r\n"),
    }
}

fn write(path: &Path, ftl: String, line_endings: &LineEndings) -> std::io::Result<()> {
    let ftl_with_line_endings = normalize_line_endings(ftl, line_endings);
    write_atomically(path, ftl_with_line_endings.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ftl::consts::{
        DEFAULT_EXCLUDE_DIRS, DEFAULT_FTL_FILENAME, DEFAULT_I18N_KEYS, DEFAULT_IGNORE_ATTRIBUTES,
        DEFAULT_IGNORE_KWARGS,
    };
    use pretty_assertions::assert_eq;
    use std::fs;
    use tempfile::TempDir;

    fn config(code_path: PathBuf, locales_path: PathBuf) -> ExtractConfig {
        ExtractConfig {
            code_path,
            locales_path,
            languages: vec!["en".to_string()],
            i18n_keys: DEFAULT_I18N_KEYS.clone(),
            i18n_keys_prefix: FastHashSet::default(),
            exclude_dirs: DEFAULT_EXCLUDE_DIRS.clone(),
            ignore_attributes: DEFAULT_IGNORE_ATTRIBUTES.clone(),
            ignore_kwargs: DEFAULT_IGNORE_KWARGS.clone(),
            default_ftl_file: PathBuf::from(DEFAULT_FTL_FILENAME),
            comment_junks: false,
            comment_keys_mode: CommentsKeyModes::Comment,
            line_endings: LineEndings::Default,
            dry_run: false,
            cache: false,
            cache_path: None,
            clear_cache: false,
            allow_parse_errors: false,
        }
    }

    #[test]
    fn test_extract_refuses_to_write_when_python_file_does_not_parse() {
        let temp = TempDir::new().unwrap();
        let code_path = temp.path().join("code");
        let locales_path = temp.path().join("locales");
        let locale_path = locales_path.join("en");
        fs::create_dir_all(&code_path).unwrap();
        fs::create_dir_all(&locale_path).unwrap();

        fs::write(code_path.join("good.py"), r#"i18n.get("hello")"#).unwrap();
        fs::write(code_path.join("broken.py"), "i18n.get(\"keep-me\"\n").unwrap();
        let ftl_path = locale_path.join("_default.ftl");
        fs::write(&ftl_path, "keep-me = Keep me\n").unwrap();

        let error = extract(config(code_path.clone(), locales_path.clone())).unwrap_err();

        let message = error.to_string();
        assert!(message.contains("Extraction aborted"), "{message}");
        assert!(message.contains("broken.py"), "{message}");
        assert!(message.contains("--allow-parse-errors"), "{message}");
        assert_eq!(
            fs::read_to_string(&ftl_path).unwrap(),
            "keep-me = Keep me\n",
            "existing .ftl must be left untouched"
        );
        assert!(!locale_path.join("hello.ftl").exists());
    }

    #[test]
    fn test_extract_allow_parse_errors_skips_broken_files_and_writes() {
        let temp = TempDir::new().unwrap();
        let code_path = temp.path().join("code");
        let locales_path = temp.path().join("locales");
        fs::create_dir_all(&code_path).unwrap();

        fs::write(code_path.join("good.py"), r#"i18n.get("hello")"#).unwrap();
        fs::write(code_path.join("broken.py"), "i18n.get(").unwrap();

        let mut cfg = config(code_path, locales_path.clone());
        cfg.allow_parse_errors = true;

        let stats = extract(cfg).unwrap();

        assert_eq!(stats.ftl_in_code_keys_count, 1);
        let content = fs::read_to_string(locales_path.join("en").join("_default.ftl")).unwrap();
        assert!(content.contains("hello = hello"));
    }

    #[test]
    fn test_extract_conflicting_paths_abort_even_with_allow_parse_errors() {
        let temp = TempDir::new().unwrap();
        let code_path = temp.path().join("code");
        let locales_path = temp.path().join("locales");
        fs::create_dir_all(&code_path).unwrap();

        fs::write(
            code_path.join("a.py"),
            r#"i18n.get("hello", _path="one.ftl")"#,
        )
        .unwrap();
        fs::write(
            code_path.join("b.py"),
            r#"i18n.get("hello", _path="two.ftl")"#,
        )
        .unwrap();

        let mut cfg = config(code_path.clone(), locales_path.clone());
        cfg.allow_parse_errors = true;

        let error = extract(cfg).unwrap_err();

        let message = error.to_string();
        assert!(message.contains("key-path-conflict"), "{message}");
        assert!(message.contains("a.py:1:1"), "{message}");
        assert!(message.contains("b.py:1:1"), "{message}");
        assert!(!message.contains("--allow-parse-errors"), "{message}");
        assert!(!locales_path.exists(), "nothing may be written on conflict");
    }

    #[test]
    fn test_extract_writes_new_keys_and_reuses_cache() {
        let temp = TempDir::new().unwrap();
        let code_path = temp.path().join("code");
        let locales_path = temp.path().join("locales");
        let cache_path = temp.path().join("cache");
        fs::create_dir_all(&code_path).unwrap();

        fs::write(
            code_path.join("app.py"),
            r#"
i18n.get("hello", name=user.name)
i18n.page.title(_path="pages/main.ftl")
"#,
        )
        .unwrap();

        let mut first = config(code_path.clone(), locales_path.clone());
        first.cache = true;
        first.cache_path = Some(cache_path.clone());
        first.clear_cache = true;
        first.line_endings = LineEndings::CRLF;

        let first_stats = extract(first).unwrap();

        assert_eq!(first_stats.py_files_count, 1);
        assert_eq!(first_stats.ftl_in_code_keys_count, 2);
        assert_eq!(first_stats.ftl_keys_added["en"], 2);
        assert!(
            cache_path
                .join(format!("extract-{}-v3.bin", env!("CARGO_PKG_VERSION")))
                .exists()
        );

        let default_content =
            fs::read_to_string(locales_path.join("en").join("_default.ftl")).unwrap();
        assert!(default_content.contains("hello = hello"));
        assert!(default_content.contains("{ $name }"));
        assert!(default_content.contains("\r\n"));

        let nested_content =
            fs::read_to_string(locales_path.join("en").join("pages").join("main.ftl")).unwrap();
        assert!(nested_content.contains("page-title = page-title"));

        let mut second = config(code_path, locales_path);
        second.cache = true;
        second.cache_path = Some(cache_path);

        let second_stats = extract(second).unwrap();

        assert_eq!(second_stats.py_files_count, 1);
        assert_eq!(second_stats.ftl_in_code_keys_count, 2);
        assert_eq!(second_stats.ftl_keys_added["en"], 0);
        assert_eq!(second_stats.ftl_keys_updated["en"], 0);
    }

    #[test]
    fn test_extract_updates_kwargs_and_comments_obsolete_keys() {
        let temp = TempDir::new().unwrap();
        let code_path = temp.path().join("code");
        let locales_path = temp.path().join("locales");
        let locale_path = locales_path.join("en");
        fs::create_dir_all(&code_path).unwrap();
        fs::create_dir_all(&locale_path).unwrap();

        fs::write(
            code_path.join("app.py"),
            r#"i18n.get("hello", name=user.name)"#,
        )
        .unwrap();
        fs::write(
            locale_path.join("_default.ftl"),
            "hello = Hello\nobsolete = Obsolete\n",
        )
        .unwrap();

        let stats = extract(config(code_path, locales_path.clone())).unwrap();

        assert_eq!(stats.ftl_keys_updated["en"], 1);
        assert_eq!(stats.ftl_keys_commented["en"], 2);

        let output = fs::read_to_string(locales_path.join("en").join("_default.ftl")).unwrap();
        assert!(output.contains("# hello = Hello"));
        assert!(output.contains("hello = hello"));
        assert!(output.contains("{ $name }"));
        assert!(output.contains("# obsolete = Obsolete"));
    }

    #[test]
    fn test_extract_updates_key_when_path_changes() {
        let temp = TempDir::new().unwrap();
        let code_path = temp.path().join("code");
        let locales_path = temp.path().join("locales");
        let locale_path = locales_path.join("en");
        fs::create_dir_all(&code_path).unwrap();
        fs::create_dir_all(&locale_path).unwrap();

        fs::write(
            code_path.join("app.py"),
            r#"i18n.get("moved", _path="new.ftl")"#,
        )
        .unwrap();
        fs::write(locale_path.join("_default.ftl"), "moved = Old path\n").unwrap();

        let stats = extract(config(code_path, locales_path.clone())).unwrap();

        assert_eq!(stats.ftl_keys_updated["en"], 1);
        assert_eq!(stats.ftl_keys_commented["en"], 1);
        assert!(
            fs::read_to_string(locale_path.join("_default.ftl"))
                .unwrap()
                .contains("# moved = Old path")
        );
        assert!(
            fs::read_to_string(locale_path.join("new.ftl"))
                .unwrap()
                .contains("moved = moved")
        );
    }

    #[test]
    fn test_extract_preserves_standalone_comments_and_leaves_comment_only_files_alone() {
        let temp = TempDir::new().unwrap();
        let code_path = temp.path().join("code");
        let locales_path = temp.path().join("locales");
        let locale_path = locales_path.join("en");
        fs::create_dir_all(&code_path).unwrap();
        fs::create_dir_all(&locale_path).unwrap();

        fs::write(code_path.join("app.py"), r#"i18n.get("greeting")"#).unwrap();
        fs::write(
            locale_path.join("_default.ftl"),
            "### Resource\n\n# Standalone\n\ngreeting = Hello\n",
        )
        .unwrap();
        // Would be re-formatted by the serializer if it were rewritten.
        let notes = "# only comments\n## group\n";
        fs::write(locale_path.join("notes.ftl"), notes).unwrap();

        let stats = extract(config(code_path, locales_path)).unwrap();

        // Comments do not count as stored keys.
        assert_eq!(stats.ftl_stored_keys_count["en"], 1);
        let default = fs::read_to_string(locale_path.join("_default.ftl")).unwrap();
        assert!(default.contains("### Resource"), "{default}");
        assert!(default.contains("# Standalone"), "{default}");
        assert!(default.contains("greeting = Hello"), "{default}");
        assert_eq!(
            fs::read_to_string(locale_path.join("notes.ftl")).unwrap(),
            notes
        );
    }

    #[test]
    fn test_extract_keeps_keys_in_nested_files_in_place() {
        let temp = TempDir::new().unwrap();
        let code_path = temp.path().join("code");
        let locales_path = temp.path().join("locales");
        let pages = locales_path.join("en").join("pages");
        fs::create_dir_all(&code_path).unwrap();
        fs::create_dir_all(&pages).unwrap();

        fs::write(
            code_path.join("app.py"),
            r#"i18n.get("title", _path="pages")"#,
        )
        .unwrap();
        fs::write(pages.join("_default.ftl"), "title = Main page\n").unwrap();

        let stats = extract(config(code_path, locales_path)).unwrap();

        assert_eq!(stats.ftl_keys_added["en"], 0);
        assert_eq!(stats.ftl_keys_updated["en"], 0);
        assert_eq!(stats.ftl_keys_commented["en"], 0);
        assert_eq!(stats.ftl_stored_keys_count["en"], 1);
        assert_eq!(
            fs::read_to_string(pages.join("_default.ftl"))
                .unwrap()
                .trim(),
            "title = Main page"
        );
    }

    #[test]
    fn test_handle_comments_and_junk_comments_junk_entries() {
        let mut config = config(PathBuf::from("code"), PathBuf::from("locales"));
        config.comment_junks = true;
        let mut statistics = ExtractionStatistics::new();
        statistics.init_lang("en");
        let mut leave_as_is = vec![FluentKey::new(
            Arc::new(PathBuf::new()),
            String::new(),
            FluentEntry::Junk("bad = {".to_string()),
            Arc::new(PathBuf::from("_default.ftl")),
            Some("en".to_string()),
            Some(0),
            FastHashSet::default(),
        )];

        handle_comments_and_junk(
            &mut FastHashMap::default(),
            &mut FastHashMap::default(),
            &mut leave_as_is,
            Path::new("locales/en"),
            &config,
            &mut statistics,
            "en",
        );

        assert!(matches!(
            leave_as_is[0].entry.as_ref(),
            FluentEntry::Comment(_)
        ));
        assert_eq!(statistics.ftl_keys_commented["en"], 1);
    }

    #[test]
    fn test_extract_reports_write_failures_instead_of_panicking() {
        let temp = TempDir::new().unwrap();
        let code_path = temp.path().join("code");
        let locales_path = temp.path().join("locales");
        let locale_path = locales_path.join("en");
        fs::create_dir_all(&code_path).unwrap();
        fs::create_dir_all(&locale_path).unwrap();

        fs::write(
            code_path.join("app.py"),
            r#"
i18n.get("hello")
i18n.get("nested", _path="pages/main.ftl")
"#,
        )
        .unwrap();
        // Both target files are blocked: one is a directory, the other has a file as parent.
        fs::create_dir_all(locale_path.join("_default.ftl")).unwrap();
        fs::write(locale_path.join("pages"), "not a directory").unwrap();

        let error = extract(config(code_path, locales_path)).unwrap_err();

        let message = error.to_string();
        assert!(
            message.contains("Failed to write 2 .ftl files"),
            "{message}"
        );
        assert!(message.contains("_default.ftl"), "{message}");
        assert!(message.contains("main.ftl"), "{message}");
        assert!(locale_path.join("_default.ftl").is_dir());
        assert_eq!(
            fs::read_to_string(locale_path.join("pages")).unwrap(),
            "not a directory"
        );
    }

    #[test]
    fn test_extract_warn_mode_dry_run_does_not_rewrite_file() {
        let temp = TempDir::new().unwrap();
        let code_path = temp.path().join("code");
        let locales_path = temp.path().join("locales");
        let locale_path = locales_path.join("en");
        fs::create_dir_all(&code_path).unwrap();
        fs::create_dir_all(&locale_path).unwrap();

        fs::write(code_path.join("app.py"), "print('no translations')").unwrap();
        let ftl_path = locale_path.join("_default.ftl");
        fs::write(&ftl_path, "obsolete = Obsolete\n").unwrap();

        let mut cfg = config(code_path, locales_path);
        cfg.comment_keys_mode = CommentsKeyModes::Warn;
        cfg.dry_run = true;

        let stats = extract(cfg).unwrap();

        assert_eq!(stats.py_files_count, 0);
        assert_eq!(stats.ftl_keys_commented["en"], 1);
        assert_eq!(
            fs::read_to_string(ftl_path).unwrap(),
            "obsolete = Obsolete\n"
        );
    }

    #[test]
    fn test_normalize_line_endings() {
        assert_eq!(
            normalize_line_endings("a\r\nb\rc\n".to_string(), &LineEndings::LF),
            "a\nb\nc\n"
        );
        let cr = normalize_line_endings("a\r\nb\n".to_string(), &LineEndings::CR);
        assert_eq!(cr.as_bytes(), b"a\rb\r");
        assert_eq!(
            normalize_line_endings("a\rb\n".to_string(), &LineEndings::CRLF),
            "ab\r\n"
        );
    }

    /// Runs `extract` on one Python file and one `_default.ftl`; returns the statistics and the
    /// file content afterwards.
    fn extract_fixture(code: &str, ftl: &str) -> (ExtractionStatistics, String, TempDir) {
        let temp = TempDir::new().unwrap();
        let code_path = temp.path().join("code");
        let locales_path = temp.path().join("locales");
        let locale_path = locales_path.join("en");
        fs::create_dir_all(&code_path).unwrap();
        fs::create_dir_all(&locale_path).unwrap();
        fs::write(code_path.join("app.py"), code).unwrap();
        let ftl_path = locale_path.join("_default.ftl");
        fs::write(&ftl_path, ftl).unwrap();

        let stats = extract(config(code_path, locales_path)).unwrap();

        let output = fs::read_to_string(&ftl_path).unwrap();
        (stats, output, temp)
    }

    /// Asserts that `extract` neither comments out nor rewrites anything in `ftl`. The fixture
    /// has to be in the serializer's canonical form (no blank lines between entries), otherwise
    /// re-serialization alone would change the bytes.
    fn assert_extract_leaves_unchanged(code: &str, ftl: &str) {
        let (stats, output, _temp) = extract_fixture(code, ftl);

        assert_eq!(
            stats.ftl_keys_commented["en"], 0,
            "commented keys in {ftl:?}"
        );
        assert_eq!(stats.ftl_keys_updated["en"], 0, "updated keys in {ftl:?}");
        assert_eq!(stats.ftl_keys_added["en"], 0, "added keys in {ftl:?}");
        assert_eq!(output, ftl);
    }

    #[test]
    fn test_extract_unchanged_harness_detects_a_rewrite() {
        // Control for the tests below: a real kwargs mismatch is still commented and replaced.
        let (stats, output, _temp) =
            extract_fixture("i18n.items()\n", "items = You have { $count } items\n");

        assert_eq!(stats.ftl_keys_commented["en"], 1);
        assert_eq!(stats.ftl_keys_updated["en"], 1);
        assert_eq!(
            output,
            "# items = You have { $count } items\n\nitems = items\n"
        );
    }

    #[test]
    fn test_extract_keeps_message_whose_variable_is_a_function_argument() {
        assert_extract_leaves_unchanged(
            "i18n.items(count=5)\n",
            "items = You have { NUMBER($count) } items\n",
        );
    }

    #[test]
    fn test_extract_keeps_message_whose_variable_is_only_in_a_function_selector() {
        assert_extract_leaves_unchanged(
            "i18n.items(count=5)\n",
            "items =\n    { NUMBER($count) ->\n        [one] One item\n       *[other] Many items\n    }\n",
        );
    }

    #[test]
    fn test_extract_keeps_message_using_a_parameterized_term() {
        let term = "-brand =\n    { $case ->\n        [gen] Bota\n       *[nom] Bot\n    }\n";
        let with_argument = format!("{term}about = Pro {{ -brand(case: \"gen\") }}\n");
        let without_argument = format!("{term}about = Pro {{ -brand }}\n");

        assert_extract_leaves_unchanged("i18n.about()\n", &with_argument);
        assert_extract_leaves_unchanged("i18n.about()\n", &without_argument);
    }

    #[test]
    fn test_extract_keeps_message_referencing_an_attribute() {
        assert_extract_leaves_unchanged(
            "i18n.a(x=1)\ni18n.b()\n",
            "b = B\n    .title = Title { $x }\na = See { b.title }\n",
        );
    }

    #[test]
    fn test_extract_keeps_message_reached_only_through_an_attribute_reference() {
        // `b` is not called from code, but `a` depends on `b.title`, so it is not stale.
        assert_extract_leaves_unchanged(
            "i18n.a(x=1)\n",
            "b = B\n    .title = Title { $x }\na = See { b.title }\n",
        );
    }

    #[test]
    fn test_extract_keeps_non_ascii_messages_byte_for_byte() {
        // Same three cases with Ukrainian text, compared as bytes so that encoding problems
        // cannot hide behind a lossy string comparison.
        let cases: [(&str, &str); 3] = [
            (
                "i18n.about()\n",
                "-brand =\n    { $case ->\n        [gen] Бота\n       *[nom] Бот\n    }\nabout = Про { -brand(case: \"gen\") }\n",
            ),
            (
                "i18n.items(count=5)\n",
                "items = У вас { NUMBER($count) } елементів\n",
            ),
            (
                "i18n.a(x=1)\ni18n.b(x=1)\n",
                "b = Кнопка { $x }\n    .title = Підказка { $x }\na = Див. { b.title }\n",
            ),
        ];

        for (code, ftl) in cases {
            let temp = TempDir::new().unwrap();
            let code_path = temp.path().join("code");
            let locale_path = temp.path().join("locales").join("en");
            fs::create_dir_all(&code_path).unwrap();
            fs::create_dir_all(&locale_path).unwrap();
            fs::write(code_path.join("app.py"), code).unwrap();
            let ftl_path = locale_path.join("_default.ftl");
            fs::write(&ftl_path, ftl.as_bytes()).unwrap();

            let stats = extract(config(code_path, temp.path().join("locales"))).unwrap();

            assert_eq!(
                stats.ftl_keys_commented["en"], 0,
                "commented keys in {ftl:?}"
            );
            assert_eq!(stats.ftl_keys_updated["en"], 0, "updated keys in {ftl:?}");
            assert_eq!(
                fs::read(&ftl_path).unwrap(),
                ftl.as_bytes(),
                "bytes changed in {ftl:?}"
            );
        }
    }

    #[test]
    fn test_extract_comments_unused_entries_line_by_line() {
        // One unused entry of each shape; the commented copy must keep every line.
        let cases: [(&str, &str); 6] = [
            (
                "hello = Hello\n\nold-rules =\n    Rule one.\n    Rule two.\n    Rule three.\n",
                "hello = Hello\n\n# old-rules =\n#     Rule one.\n#     Rule two.\n#     Rule three.\n\n",
            ),
            (
                "hello = Hello\n\nitems =\n    { $n ->\n        [one] One item\n       *[other] { $n } items\n    }\n",
                "hello = Hello\n\n# items =\n#     { $n ->\n#         [one] One item\n#        *[other] { $n } items\n#     }\n\n",
            ),
            (
                "hello = Hello\n\nbtn = Click\n    .title = Tooltip\n",
                "hello = Hello\n\n# btn = Click\n#     .title = Tooltip\n\n",
            ),
            (
                "hello = Hello\n\n# ftl-extract: ignore stale\nstatus-ok = OK\n",
                "hello = Hello\n\n# # ftl-extract: ignore stale\n# status-ok = OK\n\n",
            ),
            (
                "hello = Hello\n\nold-rules =\n    Правило перше.\n    Правило друге.\n",
                "hello = Hello\n\n# old-rules =\n#     Правило перше.\n#     Правило друге.\n\n",
            ),
            (
                "hello = Hello\n\n# Shown on the legacy screen\nlegacy = Legacy text\n",
                "hello = Hello\n\n# # Shown on the legacy screen\n# legacy = Legacy text\n\n",
            ),
        ];

        for (ftl, expected) in cases {
            let (stats, output, _temp) = extract_fixture("i18n.hello()\n", ftl);

            assert_eq!(
                stats.ftl_keys_commented["en"], 1,
                "commented keys for {ftl:?}"
            );
            assert_eq!(output, expected);
        }
    }

    #[test]
    fn test_comment_junks_keeps_every_junk_line() {
        // `import_ftl_from_dir` refuses files with junk, so `comment_junks` is only reachable
        // with keys built in memory; it goes through the same `comment_ftl_key`.
        let mut config = config(PathBuf::from("code"), PathBuf::from("locales"));
        config.comment_junks = true;
        let mut statistics = ExtractionStatistics::new();
        statistics.init_lang("en");
        let mut leave_as_is = vec![FluentKey::new(
            Arc::new(PathBuf::new()),
            String::new(),
            FluentEntry::Junk("bad = {\nstill bad\n\n\n".to_string()),
            Arc::new(PathBuf::from("_default.ftl")),
            Some("en".to_string()),
            Some(0),
            FastHashSet::default(),
        )];

        handle_comments_and_junk(
            &mut FastHashMap::default(),
            &mut FastHashMap::default(),
            &mut leave_as_is,
            Path::new("locales/en"),
            &config,
            &mut statistics,
            "en",
        );

        assert_eq!(
            leave_as_is[0].entry.as_ref(),
            &FluentEntry::Comment(fluent_syntax::ast::Comment {
                content: vec!["bad = {".to_string(), "still bad".to_string()],
            })
        );
        assert_eq!(generate_ftl(leave_as_is), "# bad = {\n# still bad\n\n");
    }
}

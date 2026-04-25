use crate::ftl::code_extractor::{extract_fluent_keys, sort_fluent_keys_by_path};
use crate::ftl::consts::{CommentsKeyModes, LineEndings};
use crate::ftl::ftl_importer::import_ftl_from_dir;
use crate::ftl::matcher::{FluentEntry, FluentKey};
use crate::ftl::process::commentator::comment_ftl_key;
use crate::ftl::process::kwargs_extractor::extract_kwargs;
use crate::ftl::process::serializer::generate_ftl;
use crate::ftl::utils::{ExtractionStatistics, FastHashMap, FastHashSet};
use anyhow::Result;
use globset::{Glob, GlobSetBuilder};
use log::{debug, info, warn};
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ExtractConfig {
    pub code_path: PathBuf,
    pub output_path: PathBuf,
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
}

pub fn extract(config: ExtractConfig) -> Result<ExtractionStatistics> {
    let mut statistics = ExtractionStatistics::new();

    // Statistics for each language
    for lang in &config.languages {
        statistics.init_lang(lang);
    }

    // GlobSet for exclusions
    let mut ignore_builder = GlobSetBuilder::new();
    for exclude in &config.exclude_dirs {
        ignore_builder.add(Glob::new(exclude.as_str())?);
    }
    let ignore_set = ignore_builder.build()?;

    let start = std::time::Instant::now();
    let in_code_fluent_keys = extract_fluent_keys(
        &config.code_path,
        config.i18n_keys.clone(),
        config.i18n_keys_prefix.clone(),
        &ignore_set,
        config.ignore_attributes.clone(),
        config.ignore_kwargs.clone(),
        &config.default_ftl_file,
        config.cache,
        config.cache_path.as_deref(),
        config.clear_cache,
        &mut statistics,
    );
    statistics.ftl_in_code_keys_count = in_code_fluent_keys.len();
    info!(target: "extractor::ftl", "FTL Extraction completed in {:.3?}s.", start.elapsed().as_secs_f64());

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

fn process_language(
    lang: &String,
    in_code_fluent_keys: &FastHashMap<String, FluentKey>,
    config: &ExtractConfig,
    statistics: &mut ExtractionStatistics,
) -> Result<()> {
    let (mut stored_fluent_keys, mut stored_terms, mut leave_as_is) =
        import_ftl_from_dir(&config.output_path, lang, statistics)?;

    // Normalize paths relative to the language directory
    let lang_dir = config.output_path.join(lang);
    normalize_paths(&mut stored_fluent_keys, &lang_dir);
    normalize_paths(&mut stored_terms, &lang_dir);

    let mut keys_to_comment: FastHashMap<String, FluentKey> = FastHashMap::default();
    let mut keys_to_add: FastHashMap<String, FluentKey> = FastHashMap::default();

    // Compare Code Keys vs Stored Keys (Path mismatch & New keys)
    for (key, fluent_key) in in_code_fluent_keys.iter() {
        if let Some(stored_key) = stored_fluent_keys.get(key) {
            if fluent_key.path != stored_key.path {
                // Path changed: comment old, add new
                let old_key = stored_fluent_keys.remove(key).unwrap();
                keys_to_comment.insert(key.clone(), old_key);
                keys_to_add.insert(key.clone(), fluent_key.clone());

                *statistics.ftl_keys_commented.get_mut(lang).unwrap() += 1;
                *statistics.ftl_keys_updated.get_mut(lang).unwrap() += 1;
            } else {
                // Update code path for reference
                stored_fluent_keys.get_mut(key).unwrap().code_path = fluent_key.code_path.clone();
            }
        } else {
            // New key
            keys_to_add.insert(key.clone(), fluent_key.clone());
            *statistics.ftl_keys_added.get_mut(lang).unwrap() += 1;
        }
    }

    // Compare Key Kwargs
    let stored_fluent_keys_ref = stored_fluent_keys.clone();
    let mut depend_keys: FastHashSet<String> = FastHashSet::default();

    for (key, fluent_key) in in_code_fluent_keys.iter() {
        if !stored_fluent_keys.contains_key(key) {
            continue;
        }

        let code_args = extract_kwargs(
            fluent_key,
            &mut stored_terms,
            in_code_fluent_keys,
            &mut depend_keys,
        );

        let stored_args = extract_kwargs(
            stored_fluent_keys.get(key).unwrap(),
            &mut stored_terms,
            &stored_fluent_keys_ref,
            &mut depend_keys,
        );

        if code_args != stored_args {
            keys_to_comment.insert(key.clone(), stored_fluent_keys.remove(key).unwrap());
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
    );

    Ok(())
}
fn normalize_paths(keys: &mut FastHashMap<String, FluentKey>, base: &Path) {
    for key in keys.values_mut() {
        if let Ok(stripped) = key.path.strip_prefix(base) {
            key.path = Arc::new(stripped.to_path_buf());
        }
    }
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
) {
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

    // Group "leave as is" items by path
    let mut leave_as_is_map: FastHashMap<Arc<PathBuf>, Vec<FluentKey>> = FastHashMap::default();
    for item in leave_as_is {
        leave_as_is_map
            .entry(item.path.clone())
            .or_default()
            .push(item);
    }

    let stored_keys_count = std::sync::atomic::AtomicUsize::new(0);

    sorted_fluent_keys.par_iter().for_each(|(path, keys)| {
        let full_path = lang_dir.join(path.as_ref());

        let misc_entries = leave_as_is_map.get(&full_path).cloned().unwrap_or_default();

        let ftl_content = generate_ftl(keys, &misc_entries);

        if config.dry_run {
            debug!(
                "[DRY-RUN] Would write to {}. {} keys found.",
                full_path.display(),
                keys.len()
            )
        } else {
            write(full_path.clone(), ftl_content, &config.line_endings);
            debug!("Saved {}. {} keys.", full_path.display(), keys.len());
        }

        let count = keys
            .iter()
            .filter(|k| matches!(k.entry.as_ref(), FluentEntry::Message(_)))
            .count();
        stored_keys_count.fetch_add(count, std::sync::atomic::Ordering::Relaxed);
    });

    *statistics.ftl_stored_keys_count.get_mut(lang).unwrap() +=
        stored_keys_count.load(std::sync::atomic::Ordering::Relaxed);
}

fn normalize_line_endings(s: String, line_endings: &LineEndings) -> String {
    match line_endings {
        LineEndings::Default => s,
        LineEndings::LF => s.replace("\r\n", "\n").replace('\r', "\n"),
        LineEndings::CR => s.replace("\r\n", "\r").replace('\n', "\r"),
        LineEndings::CRLF => s.replace('\r', "").replace('\n', "\r\n"),
    }
}

fn write(path: PathBuf, ftl: String, line_endings: &LineEndings) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let ftl_with_line_endings = normalize_line_endings(ftl, line_endings);
    fs::write(path, ftl_with_line_endings).expect("Unable to write file");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ftl::consts::{
        DEFAULT_EXCLUDE_DIRS, DEFAULT_FTL_FILENAME, DEFAULT_I18N_KEYS, DEFAULT_IGNORE_ATTRIBUTES,
        DEFAULT_IGNORE_KWARGS,
    };
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    fn config(code_path: PathBuf, output_path: PathBuf) -> ExtractConfig {
        ExtractConfig {
            code_path,
            output_path,
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
        }
    }

    #[test]
    fn test_extract_writes_new_keys_and_reuses_cache() {
        let temp = TempDir::new().unwrap();
        let code_path = temp.path().join("code");
        let output_path = temp.path().join("locales");
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

        let mut first = config(code_path.clone(), output_path.clone());
        first.cache = true;
        first.cache_path = Some(cache_path.clone());
        first.clear_cache = true;
        first.line_endings = LineEndings::CRLF;

        let first_stats = extract(first).unwrap();

        assert_eq!(first_stats.py_files_count, 1);
        assert_eq!(first_stats.ftl_in_code_keys_count, 2);
        assert_eq!(first_stats.ftl_keys_added["en"], 2);
        assert!(cache_path.join("extract-0.11.0-v1.bin").exists());

        let default_content =
            fs::read_to_string(output_path.join("en").join("_default.ftl")).unwrap();
        assert!(default_content.contains("hello = hello"));
        assert!(default_content.contains("{ $name }"));
        assert!(default_content.contains("\r\n"));

        let nested_content =
            fs::read_to_string(output_path.join("en").join("pages").join("main.ftl")).unwrap();
        assert!(nested_content.contains("page-title = page-title"));

        let mut second = config(code_path, output_path);
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
        let output_path = temp.path().join("locales");
        let locale_path = output_path.join("en");
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

        let stats = extract(config(code_path, output_path.clone())).unwrap();

        assert_eq!(stats.ftl_keys_updated["en"], 1);
        assert_eq!(stats.ftl_keys_commented["en"], 2);

        let output = fs::read_to_string(output_path.join("en").join("_default.ftl")).unwrap();
        assert!(output.contains("# hello = Hello"));
        assert!(output.contains("hello = hello"));
        assert!(output.contains("{ $name }"));
        assert!(output.contains("# obsolete = Obsolete"));
    }

    #[test]
    fn test_extract_updates_key_when_path_changes() {
        let temp = TempDir::new().unwrap();
        let code_path = temp.path().join("code");
        let output_path = temp.path().join("locales");
        let locale_path = output_path.join("en");
        fs::create_dir_all(&code_path).unwrap();
        fs::create_dir_all(&locale_path).unwrap();

        fs::write(
            code_path.join("app.py"),
            r#"i18n.get("moved", _path="new.ftl")"#,
        )
        .unwrap();
        fs::write(locale_path.join("_default.ftl"), "moved = Old path\n").unwrap();

        let stats = extract(config(code_path, output_path.clone())).unwrap();

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
    fn test_extract_warn_mode_dry_run_does_not_rewrite_file() {
        let temp = TempDir::new().unwrap();
        let code_path = temp.path().join("code");
        let output_path = temp.path().join("locales");
        let locale_path = output_path.join("en");
        fs::create_dir_all(&code_path).unwrap();
        fs::create_dir_all(&locale_path).unwrap();

        fs::write(code_path.join("app.py"), "print('no translations')").unwrap();
        let ftl_path = locale_path.join("_default.ftl");
        fs::write(&ftl_path, "obsolete = Obsolete\n").unwrap();

        let mut cfg = config(code_path, output_path);
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
}

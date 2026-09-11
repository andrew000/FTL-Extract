use crate::ftl::matcher::{FluentEntry, FluentKey};
use crate::ftl::utils::{ExtractionStatistics, FastHashMap, FastHashSet};
use anyhow::{Context, Result, bail};
use common::{FtlWalk, ftl_files, line_column};
use fluent_syntax::ast::Entry;
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The messages, terms and standalone comments of a locale, keyed by name.
pub(crate) type ImportResult = (
    FastHashMap<String, FluentKey>,
    FastHashMap<String, FluentKey>,
    Vec<FluentKey>,
);

/// Names defined more than once while importing, `name` for a message and `-name` for a term,
/// with every file that defines them. Only names are tracked here; the lines are looked up
/// when the error is reported.
type Duplicates = BTreeMap<String, BTreeSet<Arc<PathBuf>>>;

type FileImport = (
    FastHashMap<String, FluentKey>,
    FastHashMap<String, FluentKey>,
    Vec<FluentKey>,
    Duplicates,
);

/// A message or term that one locale defines more than once. The import keeps one definition
/// per name, so `extract` aborts instead of silently dropping the others.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DuplicateKey {
    pub(crate) locale: String,
    /// `name` for a message, `-name` for a term.
    pub(crate) key: String,
    /// `<locale>/<file>:<line>` of every definition, by file and then by line.
    pub(crate) definitions: Vec<String>,
}

impl fmt::Display for DuplicateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Fluent key {} is defined more than once in locale {}: ",
            self.key, self.locale
        )?;
        match self.definitions.split_last() {
            Some((last, [])) => f.write_str(last),
            Some((last, rest)) => write!(f, "{} and {last}", rest.join(", ")),
            None => Ok(()),
        }
    }
}

fn process_raw_ftl(
    body: &[Entry<String>],
    path: &Path,
    locale: &String,
    ftl_keys: &mut FastHashMap<String, FluentKey>,
    terms: &mut FastHashMap<String, FluentKey>,
    leave_as_is_keys: &mut Vec<FluentKey>,
) -> Result<()> {
    for (position, entry) in body.iter().enumerate() {
        match entry {
            Entry::Message(message) => {
                ftl_keys.insert(
                    message.id.name.clone(),
                    FluentKey::new(
                        Arc::new(PathBuf::new()),
                        message.id.name.clone(),
                        FluentEntry::Message(message.clone()),
                        Arc::new(path.to_path_buf()),
                        Some(locale.to_string()),
                        Some(position),
                        FastHashSet::default(),
                    ),
                );
            }
            Entry::Term(term) => {
                terms.insert(
                    term.id.name.clone(),
                    FluentKey::new(
                        Arc::new(PathBuf::new()),
                        term.id.name.clone(),
                        FluentEntry::Term(term.clone()),
                        Arc::new(path.to_path_buf()),
                        Some(locale.to_string()),
                        Some(position),
                        FastHashSet::default(),
                    ),
                );
            }
            Entry::Comment(comment) => leave_as_is_keys.push(FluentKey::new(
                Arc::new(PathBuf::new()),
                "".to_string(),
                FluentEntry::Comment(comment.clone()),
                Arc::new(path.to_path_buf()),
                Some(locale.to_string()),
                Some(position),
                FastHashSet::default(),
            )),
            Entry::GroupComment(comment) => {
                leave_as_is_keys.push(FluentKey::new(
                    Arc::new(PathBuf::new()),
                    "".to_string(),
                    FluentEntry::GroupComment(comment.clone()),
                    Arc::new(path.to_path_buf()),
                    Some(locale.to_string()),
                    Some(position),
                    FastHashSet::default(),
                ));
            }
            Entry::ResourceComment(comment) => {
                leave_as_is_keys.push(FluentKey::new(
                    Arc::new(PathBuf::new()),
                    "".to_string(),
                    FluentEntry::ResourceComment(comment.clone()),
                    Arc::new(path.to_path_buf()),
                    Some(locale.to_string()),
                    Some(position),
                    FastHashSet::default(),
                ));
            }
            _ => {
                bail!(
                    "Unsupported FTL entry type in file {} at position {}",
                    path.display(),
                    position
                )
            }
        }
    }
    Ok(())
}

/// `Failed to parse FTL file <path>:<line>:<column>: <what>`, like `ftl check --check syntax`,
/// for the first error; further errors are only counted. The file is read again for the
/// position, which only happens on this error path.
fn ftl_syntax_error(path: &Path, errors: &[fluent_syntax::parser::ParserError]) -> String {
    let content = fs::read_to_string(path).unwrap_or_default();
    let (line, column) = errors
        .first()
        .map(|error| line_column(&content, error.pos.start))
        .unwrap_or((1, 1));
    let what = errors
        .first()
        .map(|error| error.kind.to_string())
        .unwrap_or_default();
    let more = match errors.len() {
        0 | 1 => String::new(),
        n => format!(" (and {} more)", n - 1),
    };
    format!(
        "Failed to parse FTL file {}:{line}:{column}: {what}{more}",
        path.display()
    )
}

fn import_from_ftl(path: &Path, locale_dir: &Path, locale: &str) -> Result<FileImport> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read FTL file: {}", path.display()))?;

    let resource = match fluent_syntax::parser::parse(content) {
        Ok(resource) => resource,
        Err((_, errors)) => bail!(ftl_syntax_error(path, &errors)),
    };

    // Nearly every entry is a message, so the body length is a good size for the key map
    // and saves the rehashes that growing it from empty would take.
    let mut keys = FastHashMap::with_capacity_and_hasher(resource.body.len(), Default::default());
    let mut terms = FastHashMap::default();
    let mut misc = Vec::new();
    let mut duplicates = Duplicates::new();

    let path_arc = Arc::new(path.strip_prefix(locale_dir).unwrap_or(path).to_path_buf());
    // Empty code path for imported keys
    let code_path_arc = Arc::new(PathBuf::new());

    for (pos, entry) in resource.body.into_iter().enumerate() {
        // Helper closure to avoid repeating new() calls
        let make_key = |key_name: String, entry_type: FluentEntry| -> FluentKey {
            FluentKey::new(
                code_path_arc.clone(),
                key_name,
                entry_type,
                path_arc.clone(),
                Some(locale.to_string()),
                Some(pos),
                FastHashSet::default(),
            )
        };

        // A name defined twice in one file: the map keeps the last definition, the error
        // path lists both lines.
        match entry {
            Entry::Message(m) => {
                let name = m.id.name.clone();
                if keys
                    .insert(
                        name.clone(),
                        make_key(name.clone(), FluentEntry::Message(m)),
                    )
                    .is_some()
                {
                    duplicates.entry(name).or_default().insert(path_arc.clone());
                }
            }
            Entry::Term(t) => {
                let name = t.id.name.clone();
                if terms
                    .insert(name.clone(), make_key(name.clone(), FluentEntry::Term(t)))
                    .is_some()
                {
                    duplicates
                        .entry(format!("-{name}"))
                        .or_default()
                        .insert(path_arc.clone());
                }
            }
            Entry::Comment(c) => misc.push(make_key("".into(), FluentEntry::Comment(c))),
            Entry::GroupComment(c) => misc.push(make_key("".into(), FluentEntry::GroupComment(c))),
            Entry::ResourceComment(c) => {
                misc.push(make_key("".into(), FluentEntry::ResourceComment(c)))
            }
            _ => bail!("Unsupported entry in {}: {:?}", path.display(), entry),
        }
    }

    Ok((keys, terms, misc, duplicates))
}

/// Imports every `.ftl` file of `locale`. When a name is defined more than once in the locale,
/// in one file or across files, the maps hold whichever definition was merged last and the
/// returned list names every definition; the caller must abort then instead of writing.
pub(crate) fn import_ftl_from_dir(
    path: &Path,
    locale: &String,
    statistics: &mut ExtractionStatistics,
) -> Result<(ImportResult, Vec<DuplicateKey>)> {
    let locale_dir = path.join(locale);
    let paths = ftl_files(&locale_dir, FtlWalk::Filtered)?;

    let files_count = paths.len();
    *statistics.ftl_files_count.get_mut(locale).unwrap() += files_count;

    let (stored_keys, stored_terms, stored_misc, duplicates) = paths
        .par_iter()
        .map(|file_path| import_from_ftl(file_path, &locale_dir, locale))
        .try_fold(empty_import, |acc, result| {
            Ok::<FileImport, anyhow::Error>(merge_imports(acc, result?))
        })
        .try_reduce(empty_import, |a, b| Ok(merge_imports(a, b)))?;

    let duplicates = duplicate_keys(&locale_dir, locale, duplicates);
    Ok(((stored_keys, stored_terms, stored_misc), duplicates))
}

fn empty_import() -> FileImport {
    (
        FastHashMap::default(),
        FastHashMap::default(),
        Vec::new(),
        Duplicates::new(),
    )
}

/// Merges `b` into `a`. The common case is one `.ftl` file per locale, where `a` is still
/// the empty accumulator: the file's maps are then moved over instead of being re-inserted
/// entry by entry. A name present on both sides is recorded as a duplicate with both files;
/// which definition the map keeps does not matter, because the caller aborts.
fn merge_imports(a: FileImport, b: FileImport) -> FileImport {
    fn merge_map(
        mut a: FastHashMap<String, FluentKey>,
        b: FastHashMap<String, FluentKey>,
        duplicates: &mut Duplicates,
        term: bool,
    ) -> FastHashMap<String, FluentKey> {
        if a.is_empty() {
            return b;
        }
        a.reserve(b.len());
        for (name, key) in b {
            match a.entry(name) {
                std::collections::hash_map::Entry::Occupied(mut existing) => {
                    let name = if term {
                        format!("-{}", existing.key())
                    } else {
                        existing.key().clone()
                    };
                    let files = duplicates.entry(name).or_default();
                    files.insert(existing.get().path.clone());
                    files.insert(key.path.clone());
                    existing.insert(key);
                }
                std::collections::hash_map::Entry::Vacant(slot) => {
                    slot.insert(key);
                }
            }
        }
        a
    }
    fn merge_vec<V>(mut a: Vec<V>, mut b: Vec<V>) -> Vec<V> {
        if a.is_empty() {
            b
        } else {
            a.append(&mut b);
            a
        }
    }

    let (a_keys, a_terms, a_misc, mut duplicates) = a;
    let (b_keys, b_terms, b_misc, b_duplicates) = b;
    for (name, files) in b_duplicates {
        duplicates.entry(name).or_default().extend(files);
    }

    (
        merge_map(a_keys, b_keys, &mut duplicates, false),
        merge_map(a_terms, b_terms, &mut duplicates, true),
        merge_vec(a_misc, b_misc),
        duplicates,
    )
}

/// Turns the names collected while importing into one [`DuplicateKey`] per name, sorted by
/// name, with `<locale>/<file>:<line>` for every definition. The files are read again for the
/// lines, which only happens on this error path.
fn duplicate_keys(locale_dir: &Path, locale: &str, duplicates: Duplicates) -> Vec<DuplicateKey> {
    duplicates
        .into_iter()
        .map(|(key, files)| {
            let mut definitions = Vec::new();
            for file in files {
                let display = Path::new(locale).join(file.as_path()).display().to_string();
                let content =
                    fs::read_to_string(locale_dir.join(file.as_path())).unwrap_or_default();
                let lines = definition_lines(&content, &key);
                if lines.is_empty() {
                    definitions.push(display);
                } else {
                    definitions.extend(lines.into_iter().map(|line| format!("{display}:{line}")));
                }
            }
            DuplicateKey {
                locale: locale.to_string(),
                key,
                definitions,
            }
        })
        .collect()
}

/// 1-based lines on which `content` defines `key` (`name` for a message, `-name` for a term).
/// An entry starts at the beginning of a line with its identifier followed by `=`, so
/// continuation lines, attributes, comments and longer identifiers sharing the prefix never
/// match.
fn definition_lines(content: &str, key: &str) -> Vec<usize> {
    content
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let rest = line.strip_prefix(key)?;
            rest.trim_start().starts_with('=').then_some(index + 1)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::ftl::utils::FastHashMap;
    use fluent_syntax::ast::{Comment, Entry, Entry::Junk, Identifier, Message, Pattern, Term};
    use std::path::PathBuf;
    use tempfile::TempDir;

    const DEFAULT_FTL: &str = r#"text = This is text
text-kwargs = This is text with args { $kwarg1 } { $kwarg2 }
-term1 = This is term1
-term2 = This is term2
text-args-term = This is text with args as term { -term1 } { -term2 }
-term1-with-args = This is term1 with args { $kwarg1 } { $kwarg2 }
-term2-with-args = This is term2 with args { $kwarg1 } { $kwarg2 }
message_reference = This is message_reference, uses as variable for `text-message_reference`
text-message_reference = This is text with another text { message_reference }
message_reference-args = This is message_reference with args { $kwarg1 } { $kwarg2 }, uses as variable for `text-message_reference-args`
text-message_reference-args = This is text with another text { message_reference-args }
text-args-term-args = This is text with args as term { -term1-with-args } { -term2-with-args }
text-selector =
    This is text with selector { $selector ->
        [1] Ok
        [2] Ok
       *[other] { $selector }, 🤔
    }
text-selector-selectors =
    This is text with selectors { $selector ->
        [1] Ok, { $selector }
        [2] Ok, { $selector }
       *[other] { $selector }, 🤔
    }
text-selector-kwargs =
    This is text with selector args { $selector ->
        [1] Ok, { $kwarg1 }
        [2] Ok, { $kwarg2 }
       *[other] 🤔
    }
-text-selector-reference-selector-kwargs-terms-term1 = This is term1 with args { $kwarg1 } { $kwarg2 }
text-selector-reference-selector-kwargs-terms-reference =
    This is text with selector args { $selector ->
        [1] Ok, { -text-selector-reference-selector-kwargs-terms-term1 }
        [2] Ok, { $kwarg2 }
       *[other] 🤔
    }
text-selector-reference-selector-kwargs-terms =
    This is text with selector args { $selector ->
        [1] Ok, { text-selector-reference-selector-kwargs-terms-reference }
        [2] Ok, { $kwarg1 }
       *[other] 🤔
    }

# Lease as is 1 comment

## Group Comment
## Group Comment
## Group Comment

### Resource Comment
"#;

    const JUNK_FTL: &str = r#"text = This is text
-term1 = This is term1

# Lease as is 1

JUNK!!!
"#;

    fn write_locale_fixture(root: &std::path::Path) -> PathBuf {
        let path = root.join("locales").join("en").join("_default.ftl");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, DEFAULT_FTL).unwrap();
        path
    }

    #[test]
    fn test_import_from_ftl() {
        let temp = TempDir::new().unwrap();
        let path = write_locale_fixture(temp.path());
        let locale = "en".to_string();
        let (ftl_keys, terms, leave_as_is_keys, duplicates) =
            super::import_from_ftl(&path, path.parent().unwrap(), &locale).unwrap();
        assert!(duplicates.is_empty());

        assert_eq!(ftl_keys.len(), 13);
        assert_eq!(terms.len(), 5);
        assert_eq!(leave_as_is_keys.len(), 3);
        // Paths are relative to the locale directory, for every kind of entry.
        let relative = PathBuf::from("_default.ftl");
        assert!(ftl_keys.values().all(|key| *key.path == relative));
        assert!(terms.values().all(|key| *key.path == relative));
        assert!(leave_as_is_keys.iter().all(|key| *key.path == relative));
    }

    #[test]
    fn test_process_raw_ftl_supported_entries() {
        let path = PathBuf::from("messages.ftl");
        let locale = "en".to_string();
        let body: Vec<Entry<String>> = vec![
            Entry::Message(Message {
                id: Identifier {
                    name: "hello".to_string(),
                },
                value: Some(Pattern { elements: vec![] }),
                attributes: vec![],
                comment: None,
            }),
            Entry::Term(Term {
                id: Identifier {
                    name: "brand".to_string(),
                },
                value: Pattern { elements: vec![] },
                attributes: vec![],
                comment: None,
            }),
            Entry::Comment(Comment {
                content: vec!["Comment".to_string()],
            }),
            Entry::GroupComment(Comment {
                content: vec!["Group".to_string()],
            }),
            Entry::ResourceComment(Comment {
                content: vec!["Resource".to_string()],
            }),
        ];
        let mut ftl_keys: FastHashMap<String, super::FluentKey> = FastHashMap::default();
        let mut terms: FastHashMap<String, super::FluentKey> = FastHashMap::default();
        let mut leave_as_is_keys: Vec<super::FluentKey> = Vec::new();

        super::process_raw_ftl(
            &body,
            &path,
            &locale,
            &mut ftl_keys,
            &mut terms,
            &mut leave_as_is_keys,
        )
        .unwrap();

        assert_eq!(ftl_keys.len(), 1);
        assert_eq!(terms.len(), 1);
        assert_eq!(leave_as_is_keys.len(), 3);
        assert_eq!(ftl_keys["hello"].position, 0);
        assert_eq!(terms["brand"].position, 1);
    }

    #[test]
    #[should_panic = "Unsupported FTL entry type in file"]
    fn test_process_raw_ftl_with_junk() {
        let path = PathBuf::from("_junk.ftl");
        let locale = "en".to_string();
        let body: Vec<Entry<String>> = vec![Junk {
            content: "This is junk".to_string(),
        }];
        let mut ftl_keys: FastHashMap<String, super::FluentKey> = FastHashMap::default();
        let mut terms: FastHashMap<String, super::FluentKey> = FastHashMap::default();
        let mut leave_as_is_keys: Vec<super::FluentKey> = Vec::new();

        super::process_raw_ftl(
            &body,
            &path,
            &locale,
            &mut ftl_keys,
            &mut terms,
            &mut leave_as_is_keys,
        )
        .unwrap();
    }

    #[test]
    #[should_panic = "Failed to parse FTL file"]
    fn test_import_from_ftl_with_junk() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("_junk.ftl");
        std::fs::write(&path, JUNK_FTL).unwrap();
        let locale = "en".to_string();
        let (_ftl_keys, _terms, _leave_as_is_keys, _duplicates) =
            super::import_from_ftl(&path, temp.path(), &locale).unwrap();
    }

    #[test]
    fn test_import_ftl_from_dir() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("locales");
        write_locale_fixture(temp.path());
        let locale = "en".to_string();
        let mut statistics = super::ExtractionStatistics::new();
        statistics.ftl_files_count.insert(locale.clone(), 0);

        let ((ftl_keys, terms, leave_as_is_keys), duplicates) =
            super::import_ftl_from_dir(&path, &locale, &mut statistics).unwrap();
        assert!(duplicates.is_empty());
        assert_eq!(ftl_keys.len(), 13);
        assert_eq!(terms.len(), 5);
        assert_eq!(leave_as_is_keys.len(), 3);
        assert_eq!(*statistics.ftl_files_count.get(&locale).unwrap(), 1);
        assert!(
            ftl_keys
                .values()
                .all(|key| key.path.as_path() == std::path::Path::new("_default.ftl"))
        );
    }

    #[test]
    fn test_import_ftl_from_dir_merges_several_files() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("locales");
        let locale_dir = path.join("en");
        std::fs::create_dir_all(locale_dir.join("pages")).unwrap();
        std::fs::write(
            locale_dir.join("_default.ftl"),
            "# Default\n\nhello = Hello\n-brand = Brand\n",
        )
        .unwrap();
        std::fs::write(
            locale_dir.join("pages").join("main.ftl"),
            "# Main\n\ntitle = Main\n-product = Product\n",
        )
        .unwrap();
        std::fs::write(locale_dir.join("empty.ftl"), "").unwrap();
        let locale = "en".to_string();
        let mut statistics = super::ExtractionStatistics::new();
        statistics.ftl_files_count.insert(locale.clone(), 0);

        let ((ftl_keys, terms, leave_as_is_keys), duplicates) =
            super::import_ftl_from_dir(&path, &locale, &mut statistics).unwrap();
        assert!(duplicates.is_empty());

        let mut key_names: Vec<&str> = ftl_keys.keys().map(String::as_str).collect();
        key_names.sort_unstable();
        assert_eq!(key_names, ["hello", "title"]);
        let mut term_names: Vec<&str> = terms.keys().map(String::as_str).collect();
        term_names.sort_unstable();
        assert_eq!(term_names, ["brand", "product"]);
        assert_eq!(leave_as_is_keys.len(), 2);
        assert_eq!(*statistics.ftl_files_count.get(&locale).unwrap(), 3);
    }

    #[test]
    fn test_import_ftl_from_dir_keeps_subdirectories_in_relative_paths() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("locales");
        let nested = path.join("en").join("pages");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("main.ftl"), "title = Main\n").unwrap();
        let locale = "en".to_string();
        let mut statistics = super::ExtractionStatistics::new();
        statistics.ftl_files_count.insert(locale.clone(), 0);

        let ((ftl_keys, _terms, _leave_as_is_keys), _duplicates) =
            super::import_ftl_from_dir(&path, &locale, &mut statistics).unwrap();

        assert_eq!(
            *ftl_keys["title"].path,
            PathBuf::from("pages").join("main.ftl")
        );
    }

    #[test]
    fn test_import_from_ftl_names_the_file_line_and_column_of_a_syntax_error() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("_broken.ftl");
        std::fs::write(&path, "hello = Hello\n\nbroken = {\n    Text\n").unwrap();

        let error = super::import_from_ftl(&path, temp.path(), "en").unwrap_err();

        assert_eq!(
            error.to_string(),
            format!(
                "Failed to parse FTL file {}:5:1: Expected a token starting with \"}}\"",
                path.display()
            )
        );
    }

    #[test]
    fn test_import_from_ftl_counts_further_syntax_errors() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("_broken.ftl");
        std::fs::write(&path, "a = {\n\nb = fine\n\nc = {\n\nd = fine\n").unwrap();

        let error = super::import_from_ftl(&path, temp.path(), "en").unwrap_err();

        // The first error is reported where the parser stopped inside `a`, the second one is
        // only counted.
        assert_eq!(
            error.to_string(),
            format!(
                "Failed to parse FTL file {}:3:3: Expected a token starting with \"}}\" (and 1 more)",
                path.display()
            )
        );
    }

    /// Writes `files` (relative name, content) under `locales/en` and imports the locale.
    fn import_locale(files: &[(&str, &str)]) -> (super::ImportResult, Vec<super::DuplicateKey>) {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("locales");
        for (name, content) in files {
            let file = path.join("en").join(name);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, content).unwrap();
        }
        let locale = "en".to_string();
        let mut statistics = super::ExtractionStatistics::new();
        statistics.ftl_files_count.insert(locale.clone(), 0);

        super::import_ftl_from_dir(&path, &locale, &mut statistics).unwrap()
    }

    /// `en/<name>` with the platform's separator, as the error prints it.
    fn en(name: &str) -> String {
        PathBuf::from("en").join(name).display().to_string()
    }

    #[test]
    fn test_import_ftl_from_dir_reports_a_key_defined_in_two_files() {
        let (_, duplicates) = import_locale(&[
            ("_default.ftl", "dup = One\n"),
            ("other.ftl", "# comment\n\ndup = Two\n"),
        ]);

        assert_eq!(duplicates.len(), 1);
        assert_eq!(duplicates[0].key, "dup");
        assert_eq!(duplicates[0].locale, "en");
        assert_eq!(
            duplicates[0].to_string(),
            format!(
                "Fluent key dup is defined more than once in locale en: {}:1 and {}:3",
                en("_default.ftl"),
                en("other.ftl")
            )
        );
    }

    #[test]
    fn test_import_ftl_from_dir_reports_a_key_defined_twice_in_one_file() {
        let (_, duplicates) =
            import_locale(&[("_default.ftl", "dup = One\nother = O\ndup =\n    Two\n")]);

        assert_eq!(
            duplicates
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            [format!(
                "Fluent key dup is defined more than once in locale en: {}:1 and {}:3",
                en("_default.ftl"),
                en("_default.ftl")
            )]
        );
    }

    #[test]
    fn test_import_ftl_from_dir_reports_a_duplicate_term() {
        let (_, duplicates) = import_locale(&[
            ("_default.ftl", "-brand = Brand\nbrand = Not a term\n"),
            ("other.ftl", "-brand = Other brand\n"),
        ]);

        // The message `brand` and the term `-brand` are different names.
        assert_eq!(
            duplicates
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            [format!(
                "Fluent key -brand is defined more than once in locale en: {}:1 and {}:1",
                en("_default.ftl"),
                en("other.ftl")
            )]
        );
    }

    #[test]
    fn test_import_ftl_from_dir_lists_every_duplicate_sorted_by_key() {
        let (_, duplicates) = import_locale(&[
            ("_default.ftl", "b = 1\na = 1\n"),
            ("pages/main.ftl", "a = 2\nb = 2\nunique = 1\n"),
        ]);

        assert_eq!(
            duplicates
                .iter()
                .map(|d| d.key.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(
            duplicates[0].definitions,
            [
                format!("{}:2", en("_default.ftl")),
                format!(
                    "{}:1",
                    en(&PathBuf::from("pages")
                        .join("main.ftl")
                        .display()
                        .to_string())
                ),
            ]
        );
    }

    #[test]
    fn test_import_ftl_from_dir_duplicate_spread_over_three_files_is_deterministic() {
        let expected = format!(
            "Fluent key dup is defined more than once in locale en: {}:1, {}:2 and {}:1",
            en("a.ftl"),
            en("b.ftl"),
            en("c.ftl")
        );

        for _ in 0..10 {
            let (_, duplicates) = import_locale(&[
                ("a.ftl", "dup = A\n"),
                ("b.ftl", "other = O\ndup = B\n"),
                ("c.ftl", "dup = C\n"),
            ]);
            assert_eq!(
                duplicates
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
                std::slice::from_ref(&expected)
            );
        }
    }

    #[test]
    fn test_definition_lines_match_only_entry_starts() {
        let content = "dup = One\n# dup = commented\ndup-long = Other\n    .dup = attr\ndup=Two\n-dup = Term\n";

        assert_eq!(super::definition_lines(content, "dup"), [1, 5]);
        assert_eq!(super::definition_lines(content, "-dup"), [6]);
        assert_eq!(super::definition_lines(content, "dup-long"), [3]);
        assert!(super::definition_lines(content, "missing").is_empty());
    }
}

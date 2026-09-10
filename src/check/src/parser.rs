use crate::types::MessageEntry;
use anyhow::{Context, Result, bail};
use common::FastHashMap;
use common::{FtlWalk, ftl_files, line_column};
use fluent_syntax::ast::{Comment, Entry, Message, PatternElement, Resource};
use fluent_syntax::parser::ParserError;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn discover_locales(locales_path: &Path) -> Result<Vec<String>> {
    let mut locales = fs::read_dir(locales_path)
        .with_context(|| format!("Failed to read locales path: {}", locales_path.display()))?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            entry
                .file_type()
                .ok()
                .filter(|ft| ft.is_dir())
                .map(|_| entry.file_name().to_string_lossy().to_string())
        })
        .collect::<Vec<_>>();
    locales.sort();
    Ok(locales)
}

#[derive(Debug)]
pub struct CheckLocaleCache {
    locales_path: PathBuf,
    checked_locales: Vec<String>,
    loaded_locales: FastHashMap<String, Vec<CachedLocaleFile>>,
}

#[derive(Debug)]
pub(crate) struct CachedLocaleFile {
    pub(crate) path: PathBuf,
    pub(crate) relative_to_locale: PathBuf,
    pub(crate) relative_to_locales: PathBuf,
    pub(crate) entries: Vec<LocatedEntry>,
    pub(crate) syntax_errors: Vec<SyntaxParseError>,
    messages: Vec<MessageEntry>,
}

impl CheckLocaleCache {
    pub fn load(
        locales_path: &Path,
        requested_locales: &[String],
        extra_locales: &[String],
    ) -> Result<Self> {
        let available_locales = discover_locales(locales_path)?;
        let checked_locales = if requested_locales.is_empty() {
            available_locales.clone()
        } else {
            validate_locale_names(locales_path, &available_locales, requested_locales)?;
            requested_locales.to_vec()
        };

        let mut locales_to_load = checked_locales.clone();
        for locale in extra_locales {
            if !locales_to_load.iter().any(|loaded| loaded == locale) {
                locales_to_load.push(locale.clone());
            }
        }
        validate_locale_names(locales_path, &available_locales, &locales_to_load)?;

        let mut loaded_locales = FastHashMap::default();
        for locale in &locales_to_load {
            let files = ftl_files_for_locale(locales_path, locale)?
                .into_iter()
                .map(|path| parse_cached_ftl_file(locales_path, locale, path))
                .collect::<Result<Vec<_>>>()?;
            loaded_locales.insert(locale.clone(), files);
        }

        Ok(Self {
            locales_path: locales_path.to_path_buf(),
            checked_locales,
            loaded_locales,
        })
    }

    pub fn checked_locales(&self) -> &[String] {
        &self.checked_locales
    }

    pub(crate) fn locales_path(&self) -> &Path {
        &self.locales_path
    }

    pub(crate) fn files(&self, locale: &str) -> impl Iterator<Item = &CachedLocaleFile> {
        self.loaded_locales
            .get(locale)
            .into_iter()
            .flat_map(|files| files.iter())
    }

    pub(crate) fn messages(&self, locale: &str) -> Result<Vec<MessageEntry>> {
        let mut messages = Vec::new();
        for file in self.files(locale) {
            if !file.syntax_errors.is_empty() {
                bail!(
                    "Failed to parse FTL file {}: {:?}",
                    file.path.display(),
                    file.syntax_errors
                );
            }
            messages.extend(file.messages.iter().cloned());
        }
        Ok(messages)
    }

    pub fn load_extra_locales(&mut self, extra_locales: &[String]) -> Result<()> {
        let locales_to_load = extra_locales
            .iter()
            .filter(|locale| !self.loaded_locales.contains_key(*locale))
            .cloned()
            .collect::<Vec<_>>();

        if locales_to_load.is_empty() {
            return Ok(());
        }

        let available_locales = discover_locales(&self.locales_path)?;
        validate_locale_names(&self.locales_path, &available_locales, &locales_to_load)?;

        for locale in locales_to_load {
            let files = ftl_files_for_locale(&self.locales_path, &locale)?
                .into_iter()
                .map(|path| parse_cached_ftl_file(&self.locales_path, &locale, path))
                .collect::<Result<Vec<_>>>()?;
            self.loaded_locales.insert(locale, files);
        }

        Ok(())
    }
}

fn validate_locale_names(
    locales_path: &Path,
    available_locales: &[String],
    locales: &[String],
) -> Result<()> {
    for locale in locales {
        if !available_locales.iter().any(|existing| existing == locale) {
            bail!(
                "Locale `{}` does not exist in `{}`",
                locale,
                locales_path.display()
            );
        }
    }
    Ok(())
}

fn parse_cached_ftl_file(
    locales_path: &Path,
    locale: &str,
    path: PathBuf,
) -> Result<CachedLocaleFile> {
    let locale_path = locales_path.join(locale);
    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read FTL file: {}", path.display()))?;
    let (resource, syntax_errors) = match fluent_syntax::parser::parse(content.clone()) {
        Ok(resource) => (resource, Vec::new()),
        Err((resource, errors)) => (
            resource,
            errors
                .into_iter()
                .map(|error| syntax_parse_error(&content, error))
                .collect(),
        ),
    };
    let entries = located_entries(content.as_str(), resource);
    let messages = message_entries_from_located_entries(&entries, &path, locale);
    let relative_to_locale = path
        .strip_prefix(&locale_path)
        .unwrap_or(&path)
        .to_path_buf();
    let relative_to_locales = path
        .strip_prefix(locales_path)
        .unwrap_or(&path)
        .to_path_buf();

    Ok(CachedLocaleFile {
        path,
        relative_to_locale,
        relative_to_locales,
        entries,
        syntax_errors,
        messages,
    })
}

pub(crate) fn ftl_files_for_locale(locales_path: &Path, locale: &str) -> Result<Vec<PathBuf>> {
    ftl_files(&locales_path.join(locale), FtlWalk::Filtered)
}

#[derive(Debug, Clone)]
pub(crate) struct LocatedEntry {
    pub(crate) entry: Entry<String>,
    pub(crate) line: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SyntaxParseError {
    pub(crate) line: Option<usize>,
    pub(crate) column: Option<usize>,
    pub(crate) message: String,
}

fn syntax_parse_error(content: &str, error: ParserError) -> SyntaxParseError {
    let (line, column) = line_column(content, error.pos.start);

    SyntaxParseError {
        line: Some(line),
        column: Some(column),
        message: error.kind.to_string(),
    }
}

fn message_entries_from_located_entries(
    entries: &[LocatedEntry],
    path: &Path,
    locale: &str,
) -> Vec<MessageEntry> {
    entries
        .iter()
        .filter_map(|located| {
            let Entry::Message(message) = &located.entry else {
                return None;
            };

            Some(MessageEntry {
                locale: locale.to_string(),
                file_path: path.to_path_buf(),
                key: message.id.name.clone(),
                value: extract_message_value(message),
                line: located.line,
                ignored: ignored_checks(message.comment.as_ref()),
            })
        })
        .collect()
}

fn extract_message_value(message: &Message<String>) -> Option<String> {
    let pattern = message.value.as_ref()?;
    let mut buffer = String::new();
    for element in &pattern.elements {
        match element {
            PatternElement::TextElement { value } => {
                buffer.push_str(value);
            }
            PatternElement::Placeable { .. } => return None,
        }
    }
    Some(buffer.trim().to_string())
}

/// Checks a message opts out of through a `# ftl-extract: ignore ...` comment marker.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct IgnoredChecks {
    pub(crate) stale: bool,
    pub(crate) untranslated: bool,
}

impl IgnoredChecks {
    const ALL: Self = Self {
        stale: true,
        untranslated: true,
    };

    fn merge(&mut self, other: Self) {
        self.stale |= other.stale;
        self.untranslated |= other.untranslated;
    }
}

/// Parses the ignore markers in a message comment.
///
/// Recognised spellings, case-insensitive:
/// - `# ftl-extract: ignore` and `# ftl-extract: ignore all` ignore every per-message check
/// - `# ftl-extract: ignore stale` / `# ftl-extract: ignore untranslated`, or several names
///   separated by commas or spaces
/// - `# ftl-extract: ignore-untranslated` and a bare `# ignore` line, kept as aliases of
///   `ignore untranslated` from earlier releases
///
/// Unknown check names are skipped.
pub(crate) fn ignored_checks(comment: Option<&Comment<String>>) -> IgnoredChecks {
    let mut ignored = IgnoredChecks::default();
    let Some(comment) = comment else {
        return ignored;
    };

    for line in &comment.content {
        ignored.merge(parse_ignore_marker(line));
    }
    ignored
}

fn parse_ignore_marker(line: &str) -> IgnoredChecks {
    const PREFIX: &str = "ftl-extract:";

    let normalized = line.trim().to_ascii_lowercase();
    if normalized == "ignore" {
        return IgnoredChecks {
            untranslated: true,
            ..IgnoredChecks::default()
        };
    }

    let Some(position) = normalized.find(PREFIX) else {
        return IgnoredChecks::default();
    };
    let directive = normalized[position + PREFIX.len()..].trim();

    if directive == "ignore-untranslated" {
        return IgnoredChecks {
            untranslated: true,
            ..IgnoredChecks::default()
        };
    }

    let Some(names) = directive.strip_prefix("ignore") else {
        return IgnoredChecks::default();
    };
    if !names.is_empty() && !names.starts_with(char::is_whitespace) {
        return IgnoredChecks::default();
    }

    let mut ignored = IgnoredChecks::default();
    let mut any_name = false;
    for name in names.split(|c: char| c == ',' || c.is_whitespace()) {
        if name.is_empty() {
            continue;
        }
        any_name = true;
        match name {
            "all" => ignored.merge(IgnoredChecks::ALL),
            "stale" => ignored.stale = true,
            "untranslated" => ignored.untranslated = true,
            _ => {}
        }
    }

    if any_name {
        ignored
    } else {
        IgnoredChecks::ALL
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryKind {
    Message,
    Term,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EntryStart {
    kind: EntryKind,
    key: String,
    line: usize,
}

fn located_entries(content: &str, resource: Resource<String>) -> Vec<LocatedEntry> {
    let starts = entry_starts(content);
    let mut next_start = 0;

    resource
        .body
        .into_iter()
        .map(|entry| {
            let line = match &entry {
                Entry::Message(message) => find_entry_line(
                    &starts,
                    &mut next_start,
                    EntryKind::Message,
                    &message.id.name,
                ),
                Entry::Term(term) => {
                    find_entry_line(&starts, &mut next_start, EntryKind::Term, &term.id.name)
                }
                _ => None,
            };

            LocatedEntry { entry, line }
        })
        .collect()
}

fn find_entry_line(
    starts: &[EntryStart],
    next_start: &mut usize,
    kind: EntryKind,
    key: &str,
) -> Option<usize> {
    let found = starts
        .iter()
        .enumerate()
        .skip(*next_start)
        .find(|(_, start)| start.kind == kind && start.key == key);

    if let Some((index, start)) = found {
        *next_start = index + 1;
        Some(start.line)
    } else {
        None
    }
}

fn entry_starts(content: &str) -> Vec<EntryStart> {
    content
        .lines()
        .enumerate()
        .filter_map(|(index, line)| entry_start(line).map(|entry| (index + 1, entry)))
        .map(|(line, (kind, key))| EntryStart { kind, key, line })
        .collect()
}

fn entry_start(line: &str) -> Option<(EntryKind, String)> {
    let line = line.trim_start();
    if line.is_empty() || line.starts_with('#') || line.starts_with('.') {
        return None;
    }

    if let Some(rest) = line.strip_prefix('-') {
        let (key, rest) = take_identifier(rest)?;
        if rest.trim_start().starts_with('=') {
            return Some((EntryKind::Term, key.to_string()));
        }
        return None;
    }

    let (key, rest) = take_identifier(line)?;
    if rest.trim_start().starts_with('=') {
        Some((EntryKind::Message, key.to_string()))
    } else {
        None
    }
}

fn take_identifier(value: &str) -> Option<(&str, &str)> {
    let mut chars = value.char_indices();
    let (_, first) = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }

    let mut end = first.len_utf8();
    for (index, ch) in chars {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            end = index + ch.len_utf8();
        } else {
            break;
        }
    }

    Some((&value[..end], &value[end..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_starts_use_real_source_lines() {
        let content = "# comment\n\nwelcome = Welcome\n    .title = Title\n-brand = Brand\n";

        let starts = entry_starts(content);

        assert_eq!(
            starts,
            vec![
                EntryStart {
                    kind: EntryKind::Message,
                    key: "welcome".to_string(),
                    line: 3,
                },
                EntryStart {
                    kind: EntryKind::Term,
                    key: "brand".to_string(),
                    line: 5,
                },
            ]
        );
    }
}

#[cfg(test)]
mod ignore_marker_tests {
    use super::{IgnoredChecks, ignored_checks};
    use fluent_syntax::ast::Comment;

    fn parse(lines: &[&str]) -> IgnoredChecks {
        let comment = Comment {
            content: lines.iter().map(|line| line.to_string()).collect(),
        };
        ignored_checks(Some(&comment))
    }

    const NONE: IgnoredChecks = IgnoredChecks {
        stale: false,
        untranslated: false,
    };
    const STALE: IgnoredChecks = IgnoredChecks {
        stale: true,
        untranslated: false,
    };
    const UNTRANSLATED: IgnoredChecks = IgnoredChecks {
        stale: false,
        untranslated: true,
    };

    #[test]
    fn test_no_comment_or_unrelated_comment_ignores_nothing() {
        assert_eq!(ignored_checks(None), NONE);
        assert_eq!(parse(&["Brand name, do not translate"]), NONE);
        assert_eq!(parse(&["ftl-extract: ignored"]), NONE);
        assert_eq!(parse(&["ftl-extract: skip stale"]), NONE);
    }

    #[test]
    fn test_legacy_spellings_ignore_untranslated_only() {
        assert_eq!(parse(&["ftl-extract: ignore-untranslated"]), UNTRANSLATED);
        assert_eq!(parse(&["FTL-Extract: Ignore-Untranslated"]), UNTRANSLATED);
        assert_eq!(parse(&["ignore"]), UNTRANSLATED);
        assert_eq!(
            parse(&["Note: ftl-extract: ignore-untranslated"]),
            UNTRANSLATED
        );
    }

    #[test]
    fn test_named_checks_and_all() {
        assert_eq!(parse(&["ftl-extract: ignore stale"]), STALE);
        assert_eq!(parse(&["ftl-extract: ignore untranslated"]), UNTRANSLATED);
        assert_eq!(
            parse(&["ftl-extract: ignore stale, untranslated"]),
            IgnoredChecks::ALL
        );
        assert_eq!(
            parse(&["ftl-extract: ignore stale untranslated"]),
            IgnoredChecks::ALL
        );
        assert_eq!(parse(&["ftl-extract: ignore all"]), IgnoredChecks::ALL);
        assert_eq!(parse(&["ftl-extract: ignore"]), IgnoredChecks::ALL);
        assert_eq!(parse(&["ftl-extract:ignore"]), IgnoredChecks::ALL);
    }

    #[test]
    fn test_unknown_names_are_skipped_and_lines_merge() {
        assert_eq!(parse(&["ftl-extract: ignore missing"]), NONE);
        assert_eq!(parse(&["ftl-extract: ignore missing, stale"]), STALE);
        assert_eq!(
            parse(&[
                "Brand name",
                "ftl-extract: ignore stale",
                "ftl-extract: ignore untranslated"
            ]),
            IgnoredChecks::ALL
        );
    }
}

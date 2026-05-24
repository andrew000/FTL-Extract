use crate::types::MessageEntry;
use anyhow::{Context, Result};
use extractor::ftl::matcher::line_column;
use fluent_syntax::ast::{Comment, Entry, Message, PatternElement, Resource};
use fluent_syntax::parser::ParserError;
use ignore::WalkBuilder;
use ignore::types::TypesBuilder;
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

pub(crate) fn read_locale_messages(locales_path: &Path, locale: &str) -> Result<Vec<MessageEntry>> {
    let locale_path = locales_path.join(locale);
    if !locale_path.exists() {
        return Ok(Vec::new());
    }

    let mut type_builder = TypesBuilder::new();
    type_builder.add("ftl", "*.ftl")?;
    type_builder.select("ftl");
    let types = type_builder.build()?;

    let walker = WalkBuilder::new(&locale_path)
        .types(types)
        .parents(false)
        .git_global(false)
        .build();

    let mut entries = Vec::new();
    for entry in walker {
        let Some(path) = entry.ok().map(|it| it.into_path()) else {
            continue;
        };
        if !path.is_file() {
            continue;
        }
        entries.extend(read_ftl_messages(&path, locale)?);
    }
    Ok(entries)
}

pub(crate) fn ftl_files_for_locale(locales_path: &Path, locale: &str) -> Result<Vec<PathBuf>> {
    let locale_path = locales_path.join(locale);
    if !locale_path.exists() {
        return Ok(Vec::new());
    }

    let mut type_builder = TypesBuilder::new();
    type_builder.add("ftl", "*.ftl")?;
    type_builder.select("ftl");
    let types = type_builder.build()?;

    let walker = WalkBuilder::new(&locale_path)
        .types(types)
        .parents(false)
        .git_global(false)
        .build();

    let mut files = Vec::new();
    for entry in walker {
        let Some(path) = entry.ok().map(|it| it.into_path()) else {
            continue;
        };
        if path.is_file() {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

pub(crate) fn parse_ftl_syntax_errors(path: &Path) -> Result<Vec<SyntaxParseError>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read FTL file: {}", path.display()))?;

    match fluent_syntax::parser::parse(content.as_str()) {
        Ok(_) => Ok(Vec::new()),
        Err((_, errors)) => Ok(errors
            .into_iter()
            .map(|error| syntax_parse_error(&content, error))
            .collect()),
    }
}

pub(crate) fn parse_ftl_resource_lossy(path: &Path) -> Result<Resource<String>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read FTL file: {}", path.display()))?;

    match fluent_syntax::parser::parse(content) {
        Ok(resource) => Ok(resource),
        Err((resource, _)) => Ok(resource),
    }
}

pub(crate) fn parse_ftl_entries_lossy(path: &Path) -> Result<Vec<LocatedEntry>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read FTL file: {}", path.display()))?;

    let resource =
        fluent_syntax::parser::parse(content.clone()).unwrap_or_else(|(resource, _)| resource);

    Ok(located_entries(content.as_str(), resource))
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

fn read_ftl_messages(path: &Path, locale: &str) -> Result<Vec<MessageEntry>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read FTL file: {}", path.display()))?;
    let resource = fluent_syntax::parser::parse(content.clone()).map_err(|err| {
        anyhow::anyhow!("Failed to parse FTL file {}: {:?}", path.display(), err.1)
    })?;

    let mut messages = Vec::new();
    for located in located_entries(content.as_str(), resource) {
        if let Entry::Message(message) = located.entry {
            messages.push(MessageEntry {
                locale: locale.to_string(),
                file_path: path.to_path_buf(),
                key: message.id.name.clone(),
                value: extract_message_value(&message),
                line: located.line,
                ignore_untranslated: has_ignore_untranslated_marker(message.comment.as_ref()),
            });
        }
    }

    Ok(messages)
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

fn has_ignore_untranslated_marker(comment: Option<&Comment<String>>) -> bool {
    let Some(comment) = comment else {
        return false;
    };

    comment.content.iter().any(|line| {
        let normalized = line.trim().to_ascii_lowercase();
        normalized.contains("ftl-extract: ignore-untranslated") || normalized == "ignore"
    })
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

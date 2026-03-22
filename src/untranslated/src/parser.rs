use crate::types::MessageEntry;
use anyhow::{Context, Result};
use fluent_syntax::ast::{Comment, Entry, Message, PatternElement};
use ignore::WalkBuilder;
use ignore::types::TypesBuilder;
use std::fs;
use std::path::Path;

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

fn read_ftl_messages(path: &Path, locale: &str) -> Result<Vec<MessageEntry>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read FTL file: {}", path.display()))?;
    let resource = fluent_syntax::parser::parse(content).map_err(|err| {
        anyhow::anyhow!("Failed to parse FTL file {}: {:?}", path.display(), err.1)
    })?;

    let mut line = 1usize;
    let mut messages = Vec::new();
    for node in &resource.body {
        if let Entry::Message(message) = node {
            messages.push(MessageEntry {
                locale: locale.to_string(),
                file_path: path.to_path_buf(),
                key: message.id.name.clone(),
                value: extract_message_value(message),
                line: Some(line),
                ignore_untranslated: has_ignore_untranslated_marker(message.comment.as_ref()),
            });
        }
        line += 1;
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

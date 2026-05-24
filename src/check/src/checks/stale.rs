use crate::checks::validate_locales;
use crate::parser::{
    discover_locales, ftl_files_for_locale, parse_ftl_resource_lossy, read_locale_messages,
};
use crate::types::{CheckStaleConfig, CheckStaleResult, CodeExtractionError, StaleKey};
use anyhow::Result;
use extractor::ftl::code_extractor::extract_code_with_diagnostics;
use extractor::ftl::utils::FastHashSet;
use fluent_syntax::ast::{Entry, Expression, InlineExpression, Pattern, PatternElement};
use globset::{Glob, GlobSetBuilder};
use std::path::Path;

pub fn check_stale(config: CheckStaleConfig) -> Result<CheckStaleResult> {
    let available_locales = discover_locales(&config.locales_path)?;
    validate_locales(&config.locales_path, &available_locales, &config.locales)?;

    let ignore_set = build_ignore_set(&config.exclude_dirs)?;
    let extracted = extract_code_with_diagnostics(
        &config.code_path,
        config.i18n_keys,
        config.i18n_keys_prefix,
        &ignore_set,
        config.ignore_attributes,
        config.ignore_kwargs,
        &config.default_ftl_file,
    );
    let used_keys = extracted
        .keys
        .iter()
        .map(|key| (key.key.clone(), key.ftl_path.clone()))
        .collect::<FastHashSet<_>>();

    let mut stale_keys = Vec::new();
    for locale in &config.locales {
        let locale_path = config.locales_path.join(locale);
        let referenced_messages = referenced_messages(&config.locales_path, locale)?;

        for entry in read_locale_messages(&config.locales_path, locale)? {
            let relative_to_locale = entry
                .file_path
                .strip_prefix(&locale_path)
                .unwrap_or(&entry.file_path)
                .to_path_buf();
            if used_keys.contains(&(entry.key.clone(), relative_to_locale)) {
                continue;
            }
            if referenced_messages.contains(&entry.key) {
                continue;
            }

            stale_keys.push(StaleKey {
                locale: locale.clone(),
                file_path: entry
                    .file_path
                    .strip_prefix(&config.locales_path)
                    .unwrap_or(&entry.file_path)
                    .to_path_buf(),
                line: entry.line,
                key: entry.key,
            });
        }
    }

    stale_keys.sort_by(|a, b| {
        a.locale
            .cmp(&b.locale)
            .then_with(|| a.file_path.cmp(&b.file_path))
            .then_with(|| a.line.cmp(&b.line))
            .then_with(|| a.key.cmp(&b.key))
    });

    Ok(CheckStaleResult {
        checked_locales: config.locales,
        stale_keys,
        extraction_errors: extracted
            .diagnostics
            .into_iter()
            .map(|diagnostic| CodeExtractionError {
                key: diagnostic.key,
                message: diagnostic.message,
                locations: diagnostic.locations.into_iter().map(Into::into).collect(),
            })
            .collect(),
    })
}

fn build_ignore_set(exclude_dirs: &FastHashSet<String>) -> Result<globset::GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for exclude in exclude_dirs {
        builder.add(Glob::new(exclude.as_str())?);
    }
    Ok(builder.build()?)
}

fn referenced_messages(locales_path: &Path, locale: &str) -> Result<FastHashSet<String>> {
    let mut references = FastHashSet::default();

    for path in ftl_files_for_locale(locales_path, locale)? {
        let resource = parse_ftl_resource_lossy(&path)?;
        for entry in &resource.body {
            match entry {
                Entry::Message(message) => {
                    if let Some(pattern) = &message.value {
                        collect_pattern_message_references(pattern, &mut references);
                    }
                    for attribute in &message.attributes {
                        collect_pattern_message_references(&attribute.value, &mut references);
                    }
                }
                Entry::Term(term) => {
                    collect_pattern_message_references(&term.value, &mut references);
                    for attribute in &term.attributes {
                        collect_pattern_message_references(&attribute.value, &mut references);
                    }
                }
                _ => {}
            }
        }
    }

    Ok(references)
}

fn collect_pattern_message_references(
    pattern: &Pattern<String>,
    references: &mut FastHashSet<String>,
) {
    for element in &pattern.elements {
        if let PatternElement::Placeable { expression } = element {
            collect_expression_message_references(expression, references);
        }
    }
}

fn collect_expression_message_references(
    expression: &Expression<String>,
    references: &mut FastHashSet<String>,
) {
    match expression {
        Expression::Inline(inline) => collect_inline_message_references(inline, references),
        Expression::Select { selector, variants } => {
            collect_inline_message_references(selector, references);
            for variant in variants {
                collect_pattern_message_references(&variant.value, references);
            }
        }
    }
}

fn collect_inline_message_references(
    inline: &InlineExpression<String>,
    references: &mut FastHashSet<String>,
) {
    match inline {
        InlineExpression::MessageReference { id, .. } => {
            references.insert(id.name.clone());
        }
        InlineExpression::Placeable { expression } => {
            collect_expression_message_references(expression, references);
        }
        InlineExpression::FunctionReference { arguments, .. } => {
            for positional in &arguments.positional {
                collect_inline_message_references(positional, references);
            }
            for named in &arguments.named {
                collect_inline_message_references(&named.value, references);
            }
        }
        InlineExpression::TermReference { .. }
        | InlineExpression::StringLiteral { .. }
        | InlineExpression::NumberLiteral { .. }
        | InlineExpression::VariableReference { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use extractor::ftl::consts::{
        DEFAULT_EXCLUDE_DIRS, DEFAULT_FTL_FILENAME, DEFAULT_I18N_KEYS, DEFAULT_IGNORE_ATTRIBUTES,
        DEFAULT_IGNORE_KWARGS,
    };
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    fn config(temp: &TempDir, locales: Vec<String>) -> CheckStaleConfig {
        CheckStaleConfig {
            locales_path: temp.path().join("locales"),
            code_path: temp.path().join("code"),
            locales,
            i18n_keys: DEFAULT_I18N_KEYS.clone(),
            i18n_keys_prefix: FastHashSet::default(),
            exclude_dirs: DEFAULT_EXCLUDE_DIRS.clone(),
            ignore_attributes: DEFAULT_IGNORE_ATTRIBUTES.clone(),
            ignore_kwargs: DEFAULT_IGNORE_KWARGS.clone(),
            default_ftl_file: PathBuf::from(DEFAULT_FTL_FILENAME),
        }
    }

    #[test]
    fn test_check_stale_reports_unused_locale_key() {
        let temp = TempDir::new().unwrap();
        write(&temp.path().join("code/app.py"), r#"i18n.get("hello")"#);
        write(
            &temp.path().join("locales/uk/_default.ftl"),
            "hello = Hello\nold = Old\n",
        );

        let result = check_stale(config(&temp, vec!["uk".to_string()])).unwrap();

        assert_eq!(result.stale_keys.len(), 1);
        assert_eq!(result.stale_keys[0].key, "old");
        assert_eq!(
            result.stale_keys[0].file_path,
            PathBuf::from("uk").join("_default.ftl")
        );
        assert_eq!(result.stale_keys[0].line, Some(2));
    }

    #[test]
    fn test_check_stale_accepts_used_key_in_expected_file() {
        let temp = TempDir::new().unwrap();
        write(
            &temp.path().join("code/app.py"),
            r#"i18n.get("hello", _path="nested.ftl")"#,
        );
        write(
            &temp.path().join("locales/uk/nested.ftl"),
            "hello = Hello\n",
        );

        let result = check_stale(config(&temp, vec!["uk".to_string()])).unwrap();

        assert!(result.stale_keys.is_empty());
        assert!(result.extraction_errors.is_empty());
    }

    #[test]
    fn test_check_stale_checks_expected_path() {
        let temp = TempDir::new().unwrap();
        write(
            &temp.path().join("code/app.py"),
            r#"i18n.get("hello", _path="nested.ftl")"#,
        );
        write(
            &temp.path().join("locales/uk/_default.ftl"),
            "hello = Hello\n",
        );

        let result = check_stale(config(&temp, vec!["uk".to_string()])).unwrap();

        assert_eq!(result.stale_keys.len(), 1);
        assert_eq!(result.stale_keys[0].key, "hello");
        assert_eq!(
            result.stale_keys[0].file_path,
            PathBuf::from("uk").join("_default.ftl")
        );
    }

    #[test]
    fn test_check_stale_keeps_referenced_message() {
        let temp = TempDir::new().unwrap();
        write(&temp.path().join("code/app.py"), r#"i18n.get("welcome")"#);
        write(
            &temp.path().join("locales/uk/_default.ftl"),
            "welcome = { title }\ntitle = Welcome\n",
        );

        let result = check_stale(config(&temp, vec!["uk".to_string()])).unwrap();

        assert!(result.stale_keys.is_empty());
    }

    #[test]
    fn test_check_stale_reports_extraction_conflicts() {
        let temp = TempDir::new().unwrap();
        write(
            &temp.path().join("code/app.py"),
            r#"i18n.get("hello", _path="one.ftl")
i18n.get("hello", _path="two.ftl")
"#,
        );
        write(&temp.path().join("locales/uk/one.ftl"), "hello = Hello\n");

        let result = check_stale(config(&temp, vec!["uk".to_string()])).unwrap();

        assert_eq!(result.extraction_errors.len(), 1);
        assert_eq!(result.extraction_errors[0].key, "hello");
    }
}

use crate::checks::{run_code_aware_check, run_code_aware_check_with_extracted};
use crate::parser::CheckLocaleCache;
use crate::types::{CheckCodeAwareConfig, CheckStaleConfig, CheckStaleResult, StaleKey};
use anyhow::Result;
use common::{FastHashMap, FastHashSet};
use extractor::ftl::diagnostics::ExtractedCode;
use fluent_syntax::ast::{Entry, Expression, InlineExpression, Pattern, PatternElement};

pub fn check_stale(config: CheckStaleConfig) -> Result<CheckStaleResult> {
    run_code_aware_check(config, check_stale_with_cache)
}

pub fn check_stale_with_extracted(
    config: CheckCodeAwareConfig,
    extracted: &ExtractedCode,
) -> Result<CheckStaleResult> {
    run_code_aware_check_with_extracted(config, extracted, check_stale_with_cache)
}

pub fn check_stale_with_cache(
    cache: &CheckLocaleCache,
    extracted: &ExtractedCode,
) -> Result<CheckStaleResult> {
    let used_keys = extracted
        .keys
        .iter()
        .map(|key| (key.key.clone(), key.ftl_path.clone()))
        .collect::<FastHashSet<_>>();

    let mut stale_keys = Vec::new();
    for locale in cache.checked_locales() {
        let locale_path = cache.locales_path().join(locale);
        let referenced_messages = live_referenced_messages(cache, locale, &used_keys);

        for entry in cache.messages(locale)? {
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
                    .strip_prefix(cache.locales_path())
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
        checked_locales: cache.checked_locales().to_vec(),
        stale_keys,
        extraction_errors: Vec::new(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ReferenceNode {
    Message(String),
    Term(String),
}

fn live_referenced_messages(
    cache: &CheckLocaleCache,
    locale: &str,
    used_keys: &FastHashSet<(String, std::path::PathBuf)>,
) -> FastHashSet<String> {
    let mut graph: FastHashMap<ReferenceNode, FastHashSet<ReferenceNode>> = FastHashMap::default();
    let mut roots = FastHashSet::default();

    for file in cache.files(locale) {
        for located in &file.entries {
            match &located.entry {
                Entry::Message(message) => {
                    let node = ReferenceNode::Message(message.id.name.clone());
                    if used_keys
                        .contains(&(message.id.name.clone(), file.relative_to_locale.clone()))
                    {
                        roots.insert(node.clone());
                    }
                    let references = graph.entry(node).or_default();
                    if let Some(pattern) = &message.value {
                        collect_pattern_references(pattern, references);
                    }
                    for attribute in &message.attributes {
                        collect_pattern_references(&attribute.value, references);
                    }
                }
                Entry::Term(term) => {
                    let references = graph
                        .entry(ReferenceNode::Term(term.id.name.clone()))
                        .or_default();
                    collect_pattern_references(&term.value, references);
                    for attribute in &term.attributes {
                        collect_pattern_references(&attribute.value, references);
                    }
                }
                _ => {}
            }
        }
    }

    let mut seen = roots.clone();
    let mut stack = roots.into_iter().collect::<Vec<_>>();
    let mut referenced_messages = FastHashSet::default();

    while let Some(node) = stack.pop() {
        let Some(references) = graph.get(&node) else {
            continue;
        };

        for reference in references {
            if let ReferenceNode::Message(message) = reference {
                referenced_messages.insert(message.clone());
            }
            if seen.insert(reference.clone()) {
                stack.push(reference.clone());
            }
        }
    }

    referenced_messages
}

fn collect_pattern_references(
    pattern: &Pattern<String>,
    references: &mut FastHashSet<ReferenceNode>,
) {
    for element in &pattern.elements {
        if let PatternElement::Placeable { expression } = element {
            collect_expression_references(expression, references);
        }
    }
}

fn collect_expression_references(
    expression: &Expression<String>,
    references: &mut FastHashSet<ReferenceNode>,
) {
    match expression {
        Expression::Inline(inline) => collect_inline_references(inline, references),
        Expression::Select { selector, variants } => {
            collect_inline_references(selector, references);
            for variant in variants {
                collect_pattern_references(&variant.value, references);
            }
        }
    }
}

fn collect_inline_references(
    inline: &InlineExpression<String>,
    references: &mut FastHashSet<ReferenceNode>,
) {
    match inline {
        InlineExpression::MessageReference { id, .. } => {
            references.insert(ReferenceNode::Message(id.name.clone()));
        }
        InlineExpression::TermReference { id, arguments, .. } => {
            references.insert(ReferenceNode::Term(id.name.clone()));
            if let Some(arguments) = arguments {
                for positional in &arguments.positional {
                    collect_inline_references(positional, references);
                }
                for named in &arguments.named {
                    collect_inline_references(&named.value, references);
                }
            }
        }
        InlineExpression::Placeable { expression } => {
            collect_expression_references(expression, references);
        }
        InlineExpression::FunctionReference { arguments, .. } => {
            for positional in &arguments.positional {
                collect_inline_references(positional, references);
            }
            for named in &arguments.named {
                collect_inline_references(&named.value, references);
            }
        }
        InlineExpression::StringLiteral { .. }
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
    use std::path::{Path, PathBuf};
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
            cache: false,
            cache_path: None,
            clear_cache: false,
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
    fn test_check_stale_keeps_message_referenced_from_term_argument() {
        let temp = TempDir::new().unwrap();
        write(&temp.path().join("code/app.py"), r#"i18n.get("welcome")"#);
        write(
            &temp.path().join("locales/uk/_default.ftl"),
            "-brand = { $label }\nwelcome = { -brand(label: child) }\nchild = Child\n",
        );

        let result = check_stale(config(&temp, vec!["uk".to_string()])).unwrap();

        assert!(result.stale_keys.is_empty());
    }

    #[test]
    fn test_check_stale_reports_messages_referenced_only_by_stale_messages() {
        let temp = TempDir::new().unwrap();
        write(&temp.path().join("code/app.py"), r#"i18n.get("used")"#);
        write(
            &temp.path().join("locales/uk/_default.ftl"),
            "used = Used\norphan = { child }\nchild = Child\n",
        );

        let result = check_stale(config(&temp, vec!["uk".to_string()])).unwrap();
        let stale_keys = result
            .stale_keys
            .iter()
            .map(|item| item.key.as_str())
            .collect::<Vec<_>>();

        assert_eq!(stale_keys, vec!["orphan", "child"]);
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
        assert_eq!(result.extraction_errors[0].key.as_deref(), Some("hello"));
    }
}

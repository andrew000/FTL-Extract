use crate::checks::{run_code_aware_check, run_code_aware_check_with_extracted};
use crate::parser::CheckLocaleCache;
use crate::types::{CheckCodeAwareConfig, CheckKwargsConfig, CheckKwargsResult, KwargsMismatch};
use anyhow::Result;
use extractor::ftl::diagnostics::ExtractedCode;
use extractor::ftl::utils::{FastHashMap, FastHashSet};
use fluent_syntax::ast::{
    Entry, Expression, InlineExpression, Message, Pattern, PatternElement, Term,
};
use std::path::PathBuf;

pub fn check_kwargs(config: CheckKwargsConfig) -> Result<CheckKwargsResult> {
    run_code_aware_check(config, check_kwargs_with_cache)
}

pub fn check_kwargs_with_extracted(
    config: CheckCodeAwareConfig,
    extracted: &ExtractedCode,
) -> Result<CheckKwargsResult> {
    run_code_aware_check_with_extracted(config, extracted, check_kwargs_with_cache)
}

pub fn check_kwargs_with_cache(
    cache: &CheckLocaleCache,
    extracted: &ExtractedCode,
) -> Result<CheckKwargsResult> {
    let mut mismatches = Vec::new();
    for locale in cache.checked_locales() {
        let locale_messages = read_locale_messages_with_ast(cache, locale);

        for code_key in &extracted.keys {
            let Some(locale_message) = locale_messages
                .by_expected_path
                .get(&(code_key.key.clone(), code_key.ftl_path.clone()))
            else {
                continue;
            };

            let code_kwargs = code_key.kwargs.iter().cloned().collect::<FastHashSet<_>>();
            let ftl_kwargs = locale_messages
                .message_kwargs(&locale_message.key)
                .cloned()
                .unwrap_or_default();

            let mut missing_kwargs = ftl_kwargs
                .difference(&code_kwargs)
                .cloned()
                .collect::<Vec<_>>();
            let mut unused_kwargs = code_kwargs
                .difference(&ftl_kwargs)
                .cloned()
                .collect::<Vec<_>>();
            missing_kwargs.sort();
            unused_kwargs.sort();

            if missing_kwargs.is_empty() && unused_kwargs.is_empty() {
                continue;
            }

            mismatches.push(KwargsMismatch {
                locale: locale.clone(),
                key: code_key.key.clone(),
                file_path: locale_message.path.clone(),
                line: locale_message.line,
                code_location: code_key.code_location.clone().map(Into::into),
                missing_kwargs,
                unused_kwargs,
            });
        }
    }

    mismatches.sort_by(|a, b| {
        a.locale
            .cmp(&b.locale)
            .then_with(|| a.file_path.cmp(&b.file_path))
            .then_with(|| a.line.cmp(&b.line))
            .then_with(|| a.key.cmp(&b.key))
    });

    Ok(CheckKwargsResult {
        checked_locales: cache.checked_locales().to_vec(),
        mismatches,
        extraction_errors: Vec::new(),
    })
}

#[derive(Debug, Clone)]
struct LocaleMessage {
    key: String,
    path: PathBuf,
    line: Option<usize>,
}

#[derive(Debug)]
struct LocaleMessages {
    by_expected_path: FastHashMap<(String, PathBuf), LocaleMessage>,
    messages: FastHashMap<String, Message<String>>,
    terms: FastHashMap<String, Term<String>>,
    message_kwargs: FastHashMap<String, FastHashSet<String>>,
}

impl LocaleMessages {
    fn message_kwargs(&self, key: &str) -> Option<&FastHashSet<String>> {
        self.message_kwargs.get(key)
    }

    fn build_message_kwargs(&mut self) {
        let keys = self.messages.keys().cloned().collect::<Vec<_>>();
        for key in keys {
            let Some(message) = self.messages.get(&key) else {
                continue;
            };
            let kwargs = self.collect_message_kwargs_set(message);
            self.message_kwargs.insert(key, kwargs);
        }
    }

    fn collect_message_kwargs_set(&self, message: &Message<String>) -> FastHashSet<String> {
        let mut kwargs = FastHashSet::default();
        let mut seen_messages = FastHashSet::default();
        let mut seen_terms = FastHashSet::default();
        let local_bindings = FastHashSet::default();
        self.collect_message_kwargs(
            message,
            &local_bindings,
            &mut kwargs,
            &mut seen_messages,
            &mut seen_terms,
        );
        kwargs
    }

    fn collect_message_kwargs(
        &self,
        message: &Message<String>,
        local_bindings: &FastHashSet<String>,
        kwargs: &mut FastHashSet<String>,
        seen_messages: &mut FastHashSet<String>,
        seen_terms: &mut FastHashSet<String>,
    ) {
        if !seen_messages.insert(message.id.name.clone()) {
            return;
        }

        if let Some(pattern) = &message.value {
            self.collect_pattern_kwargs(pattern, local_bindings, kwargs, seen_messages, seen_terms);
        }
        for attribute in &message.attributes {
            self.collect_pattern_kwargs(
                &attribute.value,
                local_bindings,
                kwargs,
                seen_messages,
                seen_terms,
            );
        }
    }

    fn collect_term_kwargs(
        &self,
        term: &Term<String>,
        local_bindings: &FastHashSet<String>,
        kwargs: &mut FastHashSet<String>,
        seen_messages: &mut FastHashSet<String>,
        seen_terms: &mut FastHashSet<String>,
    ) {
        if !seen_terms.insert(term_seen_key(&term.id.name, local_bindings)) {
            return;
        }

        self.collect_pattern_kwargs(
            &term.value,
            local_bindings,
            kwargs,
            seen_messages,
            seen_terms,
        );
        for attribute in &term.attributes {
            self.collect_pattern_kwargs(
                &attribute.value,
                local_bindings,
                kwargs,
                seen_messages,
                seen_terms,
            );
        }
    }

    fn collect_pattern_kwargs(
        &self,
        pattern: &Pattern<String>,
        local_bindings: &FastHashSet<String>,
        kwargs: &mut FastHashSet<String>,
        seen_messages: &mut FastHashSet<String>,
        seen_terms: &mut FastHashSet<String>,
    ) {
        for element in &pattern.elements {
            if let PatternElement::Placeable { expression } = element {
                self.collect_expression_kwargs(
                    expression,
                    local_bindings,
                    kwargs,
                    seen_messages,
                    seen_terms,
                );
            }
        }
    }

    fn collect_expression_kwargs(
        &self,
        expression: &Expression<String>,
        local_bindings: &FastHashSet<String>,
        kwargs: &mut FastHashSet<String>,
        seen_messages: &mut FastHashSet<String>,
        seen_terms: &mut FastHashSet<String>,
    ) {
        match expression {
            Expression::Inline(inline) => {
                self.collect_inline_kwargs(
                    inline,
                    local_bindings,
                    kwargs,
                    seen_messages,
                    seen_terms,
                );
            }
            Expression::Select { selector, variants } => {
                self.collect_inline_kwargs(
                    selector,
                    local_bindings,
                    kwargs,
                    seen_messages,
                    seen_terms,
                );
                for variant in variants {
                    self.collect_pattern_kwargs(
                        &variant.value,
                        local_bindings,
                        kwargs,
                        seen_messages,
                        seen_terms,
                    );
                }
            }
        }
    }

    fn collect_inline_kwargs(
        &self,
        inline: &InlineExpression<String>,
        local_bindings: &FastHashSet<String>,
        kwargs: &mut FastHashSet<String>,
        seen_messages: &mut FastHashSet<String>,
        seen_terms: &mut FastHashSet<String>,
    ) {
        match inline {
            InlineExpression::VariableReference { id } => {
                if !local_bindings.contains(&id.name) {
                    kwargs.insert(id.name.clone());
                }
            }
            InlineExpression::MessageReference { id, .. } => {
                if let Some(message) = self.messages.get(&id.name) {
                    self.collect_message_kwargs(
                        message,
                        local_bindings,
                        kwargs,
                        seen_messages,
                        seen_terms,
                    );
                }
            }
            InlineExpression::TermReference { id, arguments, .. } => {
                let mut term_bindings = FastHashSet::default();
                if let Some(arguments) = arguments {
                    for positional in &arguments.positional {
                        self.collect_inline_kwargs(
                            positional,
                            local_bindings,
                            kwargs,
                            seen_messages,
                            seen_terms,
                        );
                    }
                    for named in &arguments.named {
                        term_bindings.insert(named.name.name.clone());
                        self.collect_inline_kwargs(
                            &named.value,
                            local_bindings,
                            kwargs,
                            seen_messages,
                            seen_terms,
                        );
                    }
                }
                if let Some(term) = self.terms.get(&id.name) {
                    self.collect_term_kwargs(
                        term,
                        &term_bindings,
                        kwargs,
                        seen_messages,
                        seen_terms,
                    );
                }
            }
            InlineExpression::Placeable { expression } => {
                self.collect_expression_kwargs(
                    expression,
                    local_bindings,
                    kwargs,
                    seen_messages,
                    seen_terms,
                );
            }
            InlineExpression::FunctionReference { arguments, .. } => {
                for positional in &arguments.positional {
                    self.collect_inline_kwargs(
                        positional,
                        local_bindings,
                        kwargs,
                        seen_messages,
                        seen_terms,
                    );
                }
                for named in &arguments.named {
                    self.collect_inline_kwargs(
                        &named.value,
                        local_bindings,
                        kwargs,
                        seen_messages,
                        seen_terms,
                    );
                }
            }
            InlineExpression::StringLiteral { .. } | InlineExpression::NumberLiteral { .. } => {}
        }
    }
}

fn term_seen_key(term_id: &str, local_bindings: &FastHashSet<String>) -> String {
    let mut bindings = local_bindings.iter().cloned().collect::<Vec<_>>();
    bindings.sort();
    format!("{term_id}:{}", bindings.join(","))
}

fn read_locale_messages_with_ast(cache: &CheckLocaleCache, locale: &str) -> LocaleMessages {
    let mut by_expected_path = FastHashMap::default();
    let mut messages = FastHashMap::default();
    let mut terms = FastHashMap::default();

    for file in cache.files(locale) {
        for located in &file.entries {
            match &located.entry {
                Entry::Message(message) => {
                    by_expected_path.insert(
                        (message.id.name.clone(), file.relative_to_locale.clone()),
                        LocaleMessage {
                            key: message.id.name.clone(),
                            path: file.relative_to_locales.clone(),
                            line: located.line,
                        },
                    );
                    messages.insert(message.id.name.clone(), message.clone());
                }
                Entry::Term(term) => {
                    terms.insert(term.id.name.clone(), term.clone());
                }
                _ => {}
            }
        }
    }

    let mut locale_messages = LocaleMessages {
        by_expected_path,
        messages,
        terms,
        message_kwargs: FastHashMap::default(),
    };
    locale_messages.build_message_kwargs();
    locale_messages
}

#[cfg(test)]
mod tests {
    use super::*;
    use extractor::ftl::consts::{
        DEFAULT_EXCLUDE_DIRS, DEFAULT_FTL_FILENAME, DEFAULT_I18N_KEYS, DEFAULT_IGNORE_ATTRIBUTES,
        DEFAULT_IGNORE_KWARGS,
    };
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    fn config(temp: &TempDir, locales: Vec<String>) -> CheckKwargsConfig {
        CheckKwargsConfig {
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
    fn test_check_kwargs_accepts_matching_variables() {
        let temp = TempDir::new().unwrap();
        write(
            &temp.path().join("code/app.py"),
            r#"i18n.get("hello", name=user.name)"#,
        );
        write(
            &temp.path().join("locales/uk/_default.ftl"),
            "hello = Hello { $name }\n",
        );

        let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

        assert!(result.mismatches.is_empty());
        assert!(result.extraction_errors.is_empty());
    }

    #[test]
    fn test_check_kwargs_reports_missing_and_unused_variables() {
        let temp = TempDir::new().unwrap();
        write(
            &temp.path().join("code/app.py"),
            r#"def handler():
    i18n.get("hello", name=user.name)
"#,
        );
        write(
            &temp.path().join("locales/uk/_default.ftl"),
            "hello = Hello { $username }\n",
        );

        let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

        assert_eq!(result.mismatches.len(), 1);
        assert_eq!(result.mismatches[0].key, "hello");
        assert_eq!(result.mismatches[0].missing_kwargs, vec!["username"]);
        assert_eq!(result.mismatches[0].unused_kwargs, vec!["name"]);
        assert_eq!(result.mismatches[0].line, Some(1));
        assert_eq!(
            result.mismatches[0].code_location.as_ref().unwrap().line,
            Some(2)
        );
    }

    #[test]
    fn test_check_kwargs_checks_expected_path() {
        let temp = TempDir::new().unwrap();
        write(
            &temp.path().join("code/app.py"),
            r#"i18n.get("hello", name=user.name, _path="nested.ftl")"#,
        );
        write(
            &temp.path().join("locales/uk/_default.ftl"),
            "hello = Hello { $username }\n",
        );
        write(
            &temp.path().join("locales/uk/nested.ftl"),
            "hello = Hello { $name }\n",
        );

        let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

        assert!(result.mismatches.is_empty());
    }

    #[test]
    fn test_check_kwargs_includes_referenced_message_variables() {
        let temp = TempDir::new().unwrap();
        write(
            &temp.path().join("code/app.py"),
            r#"i18n.get("welcome", name=user.name)"#,
        );
        write(
            &temp.path().join("locales/uk/_default.ftl"),
            "welcome = { title }\ntitle = Welcome { $name }\n",
        );

        let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

        assert!(result.mismatches.is_empty());
    }

    #[test]
    fn test_check_kwargs_term_string_argument_satisfies_term_variable() {
        let temp = TempDir::new().unwrap();
        write(&temp.path().join("code/app.py"), r#"i18n.get("title")"#);
        write(
            &temp.path().join("locales/uk/_default.ftl"),
            r#"-brand = { $case ->
    [genitive] Brand
   *[nominative] Brand
}

title = Welcome to { -brand(case: "genitive") }
"#,
        );

        let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

        assert!(result.mismatches.is_empty());
    }

    #[test]
    fn test_check_kwargs_term_number_argument_satisfies_term_variable() {
        let temp = TempDir::new().unwrap();
        write(&temp.path().join("code/app.py"), r#"i18n.get("title")"#);
        write(
            &temp.path().join("locales/uk/_default.ftl"),
            r#"-brand = { $case ->
    [1] Brand
   *[0] Brand
}

title = Welcome to { -brand(case: 1) }
"#,
        );

        let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

        assert!(result.mismatches.is_empty());
    }

    #[test]
    fn test_check_kwargs_term_without_argument_requires_term_variable() {
        let temp = TempDir::new().unwrap();
        write(&temp.path().join("code/app.py"), r#"i18n.get("title")"#);
        write(
            &temp.path().join("locales/uk/_default.ftl"),
            r#"-brand = { $case ->
    [genitive] Brand
   *[nominative] Brand
}

title = Welcome to { -brand }
"#,
        );

        let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

        assert_eq!(result.mismatches.len(), 1);
        assert_eq!(result.mismatches[0].key, "title");
        assert_eq!(result.mismatches[0].missing_kwargs, vec!["case"]);
        assert!(result.mismatches[0].unused_kwargs.is_empty());
    }

    #[test]
    fn test_check_kwargs_message_reference_still_propagates_variables() {
        let temp = TempDir::new().unwrap();
        write(
            &temp.path().join("code/app.py"),
            r#"i18n.get("welcome", name=user.name)"#,
        );
        write(
            &temp.path().join("locales/uk/_default.ftl"),
            "welcome = { title }\ntitle = Welcome { $name }\n",
        );

        let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

        assert!(result.mismatches.is_empty());
    }

    #[test]
    fn test_check_kwargs_reports_extraction_conflicts() {
        let temp = TempDir::new().unwrap();
        write(
            &temp.path().join("code/app.py"),
            r#"i18n.get("hello", _path="one.ftl")
i18n.get("hello", _path="two.ftl")
"#,
        );
        write(&temp.path().join("locales/uk/one.ftl"), "hello = Hello\n");

        let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

        assert_eq!(result.extraction_errors.len(), 1);
        assert_eq!(result.extraction_errors[0].key, "hello");
    }
}

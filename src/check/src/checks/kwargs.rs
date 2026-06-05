use crate::checks::{run_code_aware_check, run_code_aware_check_with_extracted};
use crate::parser::CheckLocaleCache;
use crate::types::{CheckCodeAwareConfig, CheckKwargsConfig, CheckKwargsResult, KwargsMismatch};
use anyhow::Result;
use extractor::ftl::diagnostics::ExtractedCode;
use extractor::ftl::utils::{FastHashMap, FastHashSet};
use fluent_syntax::ast::{
    Entry, Expression, InlineExpression, Message, Pattern, PatternElement, Term,
};
use std::{fs, path::PathBuf};

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
    fallback_message_term_references: FastHashMap<String, Vec<FallbackTermReference>>,
    fallback_term_references: FastHashMap<String, Vec<FallbackTermReference>>,
}

#[derive(Debug, Clone)]
struct FallbackTermReference {
    term_id: String,
    local_bindings: FastHashSet<String>,
    value_kwargs: FastHashSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FallbackOwnerKind {
    Message,
    Term,
}

#[derive(Debug)]
struct FallbackOwner {
    kind: FallbackOwnerKind,
    id: String,
    line: usize,
    term_references: Vec<FallbackTermReference>,
}

impl LocaleMessages {
    fn message_kwargs(&self, key: &str) -> Option<&FastHashSet<String>> {
        self.message_kwargs.get(key)
    }

    fn build_message_kwargs(&mut self) {
        let mut keys = self.messages.keys().cloned().collect::<FastHashSet<_>>();
        keys.extend(self.fallback_message_term_references.keys().cloned());
        for key in keys {
            let kwargs = self.collect_message_kwargs_set(&key);
            self.message_kwargs.insert(key, kwargs);
        }
    }

    fn collect_message_kwargs_set(&self, key: &str) -> FastHashSet<String> {
        let mut kwargs = FastHashSet::default();
        let mut seen_messages = FastHashSet::default();
        let mut seen_terms = FastHashSet::default();
        let local_bindings = FastHashSet::default();
        if let Some(message) = self.messages.get(key) {
            self.collect_message_kwargs(
                message,
                &local_bindings,
                &mut kwargs,
                &mut seen_messages,
                &mut seen_terms,
            );
        } else {
            self.collect_fallback_message_kwargs(
                key,
                &local_bindings,
                &mut kwargs,
                &mut seen_messages,
                &mut seen_terms,
            );
        }
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
        if !seen_messages.insert(local_bindings_seen_key(&message.id.name, local_bindings)) {
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
        self.collect_fallback_owner_kwargs(
            &message.id.name,
            &self.fallback_message_term_references,
            local_bindings,
            kwargs,
            seen_messages,
            seen_terms,
        );
    }

    fn collect_term_kwargs(
        &self,
        term: &Term<String>,
        local_bindings: &FastHashSet<String>,
        kwargs: &mut FastHashSet<String>,
        seen_messages: &mut FastHashSet<String>,
        seen_terms: &mut FastHashSet<String>,
    ) {
        if !seen_terms.insert(local_bindings_seen_key(&term.id.name, local_bindings)) {
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
        self.collect_fallback_owner_kwargs(
            &term.id.name,
            &self.fallback_term_references,
            local_bindings,
            kwargs,
            seen_messages,
            seen_terms,
        );
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
                } else {
                    self.collect_fallback_message_kwargs(
                        &id.name,
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
                } else {
                    self.collect_fallback_term_kwargs(
                        &id.name,
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

    fn collect_fallback_message_kwargs(
        &self,
        message_id: &str,
        local_bindings: &FastHashSet<String>,
        kwargs: &mut FastHashSet<String>,
        seen_messages: &mut FastHashSet<String>,
        seen_terms: &mut FastHashSet<String>,
    ) {
        if !seen_messages.insert(local_bindings_seen_key(message_id, local_bindings)) {
            return;
        }
        self.collect_fallback_owner_kwargs(
            message_id,
            &self.fallback_message_term_references,
            local_bindings,
            kwargs,
            seen_messages,
            seen_terms,
        );
    }

    fn collect_fallback_term_kwargs(
        &self,
        term_id: &str,
        local_bindings: &FastHashSet<String>,
        kwargs: &mut FastHashSet<String>,
        seen_messages: &mut FastHashSet<String>,
        seen_terms: &mut FastHashSet<String>,
    ) {
        if !seen_terms.insert(local_bindings_seen_key(term_id, local_bindings)) {
            return;
        }
        self.collect_fallback_owner_kwargs(
            term_id,
            &self.fallback_term_references,
            local_bindings,
            kwargs,
            seen_messages,
            seen_terms,
        );
    }

    fn collect_fallback_owner_kwargs(
        &self,
        owner_id: &str,
        fallback_references: &FastHashMap<String, Vec<FallbackTermReference>>,
        local_bindings: &FastHashSet<String>,
        kwargs: &mut FastHashSet<String>,
        seen_messages: &mut FastHashSet<String>,
        seen_terms: &mut FastHashSet<String>,
    ) {
        let Some(term_references) = fallback_references.get(owner_id) else {
            return;
        };

        for term_reference in term_references {
            for variable in &term_reference.value_kwargs {
                if !local_bindings.contains(variable) {
                    kwargs.insert(variable.clone());
                }
            }

            if let Some(term) = self.terms.get(&term_reference.term_id) {
                self.collect_term_kwargs(
                    term,
                    &term_reference.local_bindings,
                    kwargs,
                    seen_messages,
                    seen_terms,
                );
            } else {
                self.collect_fallback_term_kwargs(
                    &term_reference.term_id,
                    &term_reference.local_bindings,
                    kwargs,
                    seen_messages,
                    seen_terms,
                );
            }
        }
    }
}

fn local_bindings_seen_key(id: &str, local_bindings: &FastHashSet<String>) -> String {
    let mut bindings = local_bindings.iter().cloned().collect::<Vec<_>>();
    bindings.sort();
    format!("{id}:{}", bindings.join(","))
}

fn read_locale_messages_with_ast(cache: &CheckLocaleCache, locale: &str) -> LocaleMessages {
    let mut by_expected_path = FastHashMap::default();
    let mut messages = FastHashMap::default();
    let mut terms = FastHashMap::default();
    let mut fallback_message_term_references: FastHashMap<String, Vec<FallbackTermReference>> =
        FastHashMap::default();
    let mut fallback_term_references: FastHashMap<String, Vec<FallbackTermReference>> =
        FastHashMap::default();

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

        if !file.syntax_errors.is_empty()
            && let Ok(content) = fs::read_to_string(&file.path)
        {
            for owner in fallback_owners_from_source(&content) {
                match owner.kind {
                    FallbackOwnerKind::Message => {
                        by_expected_path
                            .entry((owner.id.clone(), file.relative_to_locale.clone()))
                            .or_insert_with(|| LocaleMessage {
                                key: owner.id.clone(),
                                path: file.relative_to_locales.clone(),
                                line: Some(owner.line),
                            });
                        fallback_message_term_references
                            .entry(owner.id)
                            .or_default()
                            .extend(owner.term_references);
                    }
                    FallbackOwnerKind::Term => {
                        fallback_term_references
                            .entry(owner.id)
                            .or_default()
                            .extend(owner.term_references);
                    }
                }
            }
        }
    }

    let mut locale_messages = LocaleMessages {
        by_expected_path,
        messages,
        terms,
        message_kwargs: FastHashMap::default(),
        fallback_message_term_references,
        fallback_term_references,
    };
    locale_messages.build_message_kwargs();
    locale_messages
}

fn fallback_owners_from_source(content: &str) -> Vec<FallbackOwner> {
    let mut owners = Vec::new();
    let mut current: Option<(FallbackOwnerKind, String, usize, String)> = None;

    for (index, line) in content.lines().enumerate() {
        if let Some((kind, id)) = entry_id_from_line(line) {
            push_fallback_owner(&mut owners, current.take());
            current = Some((kind, id, index + 1, String::new()));
        }

        if let Some((_, _, _, source)) = &mut current {
            source.push_str(line);
            source.push('\n');
        }
    }
    push_fallback_owner(&mut owners, current);

    owners
}

fn push_fallback_owner(
    owners: &mut Vec<FallbackOwner>,
    owner: Option<(FallbackOwnerKind, String, usize, String)>,
) {
    let Some((kind, id, line, source)) = owner else {
        return;
    };

    // Compatibility fallback for Fluent parser versions which do not expose
    // `-term(arg: $external_var)` as a usable AST. It is intentionally scoped to
    // kwargs inference for term references with variable named arguments.
    let term_references = fallback_term_references_from_source(&source);
    if term_references.is_empty() {
        return;
    }

    owners.push(FallbackOwner {
        kind,
        id,
        line,
        term_references,
    });
}

fn entry_id_from_line(line: &str) -> Option<(FallbackOwnerKind, String)> {
    if line.starts_with(char::is_whitespace) {
        return None;
    }

    let (raw_id, _) = line.split_once('=')?;
    let raw_id = raw_id.trim();
    if let Some(term_id) = raw_id.strip_prefix('-') {
        if is_fluent_identifier(term_id) {
            return Some((FallbackOwnerKind::Term, term_id.to_string()));
        }
    } else if is_fluent_identifier(raw_id) {
        return Some((FallbackOwnerKind::Message, raw_id.to_string()));
    }

    None
}

fn fallback_term_references_from_source(source: &str) -> Vec<FallbackTermReference> {
    let mut references = Vec::new();
    let chars = source.char_indices().collect::<Vec<_>>();
    let mut index = 0;

    while index < chars.len() {
        let (offset, ch) = chars[index];
        if ch != '-' || previous_is_identifier(source, offset) {
            index += 1;
            continue;
        }

        let Some((term_id, after_id)) = parse_term_id(source, offset + ch.len_utf8()) else {
            index += 1;
            continue;
        };
        let after_whitespace = skip_whitespace(source, after_id);
        if !source[after_whitespace..].starts_with('(') {
            index += 1;
            continue;
        }
        let Some((arguments, after_arguments)) = parse_parenthesized(source, after_whitespace)
        else {
            index += 1;
            continue;
        };

        if let Some(reference) = fallback_term_reference_from_arguments(term_id, arguments) {
            references.push(reference);
        }

        index = chars
            .iter()
            .position(|(next_offset, _)| *next_offset >= after_arguments)
            .unwrap_or(chars.len());
    }

    references
}

fn fallback_term_reference_from_arguments(
    term_id: String,
    arguments: &str,
) -> Option<FallbackTermReference> {
    let mut local_bindings = FastHashSet::default();
    let mut value_kwargs = FastHashSet::default();

    for argument in split_top_level_commas(arguments) {
        let Some((name, value)) = argument.split_once(':') else {
            continue;
        };
        let name = name.trim();
        if !is_fluent_identifier(name) {
            continue;
        }

        local_bindings.insert(name.to_string());
        if let Some(variable) = variable_reference_name(value.trim()) {
            value_kwargs.insert(variable.to_string());
        }
    }

    if value_kwargs.is_empty() {
        return None;
    }

    Some(FallbackTermReference {
        term_id,
        local_bindings,
        value_kwargs,
    })
}

fn parse_term_id(source: &str, start: usize) -> Option<(String, usize)> {
    let mut end = start;
    for (offset, ch) in source[start..].char_indices() {
        if is_identifier_continue(ch) {
            end = start + offset + ch.len_utf8();
        } else {
            break;
        }
    }

    if end == start {
        return None;
    }

    let id = &source[start..end];
    is_fluent_identifier(id).then(|| (id.to_string(), end))
}

fn parse_parenthesized(source: &str, start: usize) -> Option<(&str, usize)> {
    let mut in_string = false;
    let mut escaped = false;

    for (offset, ch) in source[start + 1..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        if ch == '"' {
            in_string = true;
        } else if ch == ')' {
            let end = start + 1 + offset;
            return Some((&source[start + 1..end], end + ch.len_utf8()));
        }
    }

    None
}

fn split_top_level_commas(arguments: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut in_string = false;
    let mut escaped = false;

    for (offset, ch) in arguments.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        if ch == '"' {
            in_string = true;
        } else if ch == ',' {
            parts.push(arguments[start..offset].trim());
            start = offset + ch.len_utf8();
        }
    }

    parts.push(arguments[start..].trim());
    parts
}

fn variable_reference_name(value: &str) -> Option<&str> {
    let variable = value.strip_prefix('$')?;
    let end = variable
        .char_indices()
        .find_map(|(offset, ch)| (!is_identifier_continue(ch)).then_some(offset))
        .unwrap_or(variable.len());
    let variable = &variable[..end];
    is_fluent_identifier(variable).then_some(variable)
}

fn previous_is_identifier(source: &str, offset: usize) -> bool {
    source[..offset]
        .chars()
        .next_back()
        .is_some_and(is_identifier_continue)
}

fn skip_whitespace(source: &str, start: usize) -> usize {
    source[start..]
        .char_indices()
        .find_map(|(offset, ch)| (!ch.is_whitespace()).then_some(start + offset))
        .unwrap_or(source.len())
}

fn is_fluent_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    (first.is_ascii_alphabetic() || first == '_') && chars.all(is_identifier_continue)
}

fn is_identifier_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'
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
    fn test_check_kwargs_term_variable_argument_requires_source_variable() {
        let temp = TempDir::new().unwrap();
        write(
            &temp.path().join("code/app.py"),
            r#"i18n.get("title", case=value)"#,
        );
        write(
            &temp.path().join("locales/uk/_default.ftl"),
            r#"-brand = { $case ->
    [genitive] Brand
   *[nominative] Brand
}

title = Welcome to { -brand(case: $case) }
"#,
        );

        let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();
        assert!(result.mismatches.is_empty());

        write(&temp.path().join("code/app.py"), r#"i18n.get("title")"#);
        let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();
        assert_eq!(result.mismatches.len(), 1);
        assert_eq!(result.mismatches[0].missing_kwargs, vec!["case"]);
        assert!(result.mismatches[0].unused_kwargs.is_empty());
    }

    #[test]
    fn test_check_kwargs_term_renamed_variable_argument_requires_external_variable() {
        let temp = TempDir::new().unwrap();
        write(
            &temp.path().join("code/app.py"),
            r#"i18n.get("title", brand_case=value)"#,
        );
        write(
            &temp.path().join("locales/uk/_default.ftl"),
            r#"-brand = { $case ->
    [genitive] Brand
   *[nominative] Brand
}

title = Welcome to { -brand(case: $brand_case) }
"#,
        );

        let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();
        assert!(result.mismatches.is_empty());

        write(&temp.path().join("code/app.py"), r#"i18n.get("title")"#);
        let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();
        assert_eq!(result.mismatches.len(), 1);
        assert_eq!(result.mismatches[0].missing_kwargs, vec!["brand_case"]);
        assert!(
            !result.mismatches[0]
                .missing_kwargs
                .contains(&"case".to_string())
        );
        assert!(result.mismatches[0].unused_kwargs.is_empty());
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
    fn test_check_kwargs_term_local_binding_does_not_hide_message_requirement_outside_term() {
        let temp = TempDir::new().unwrap();
        write(&temp.path().join("code/app.py"), r#"i18n.get("title")"#);
        write(
            &temp.path().join("locales/uk/_default.ftl"),
            r#"-brand = { subtitle }
subtitle = Brand { $case }
wrapper = { subtitle }
title = { -brand(case: "genitive") } { wrapper }
"#,
        );

        let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

        assert_eq!(result.mismatches.len(), 1);
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
    fn test_check_kwargs_referenced_message_reports_missing_variables() {
        let temp = TempDir::new().unwrap();
        write(&temp.path().join("code/app.py"), r#"i18n.get("welcome")"#);
        write(
            &temp.path().join("locales/uk/_default.ftl"),
            "welcome = { title }\ntitle = Welcome { $name }\n",
        );

        let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

        assert_eq!(result.mismatches.len(), 1);
        assert_eq!(result.mismatches[0].key, "welcome");
        assert_eq!(result.mismatches[0].missing_kwargs, vec!["name"]);
        assert!(result.mismatches[0].unused_kwargs.is_empty());
    }

    #[test]
    fn test_check_kwargs_referenced_message_uses_term_variable_argument() {
        let temp = TempDir::new().unwrap();
        write(
            &temp.path().join("code/app.py"),
            r#"i18n.get("title", brand_case=value)"#,
        );
        write(
            &temp.path().join("locales/uk/_default.ftl"),
            r#"title = { subtitle }
subtitle = Welcome to { -brand(case: $brand_case) }
-brand = { $case ->
    [genitive] Brand
   *[nominative] Brand
}
"#,
        );

        let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();
        assert!(result.mismatches.is_empty());

        write(&temp.path().join("code/app.py"), r#"i18n.get("title")"#);
        let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();
        assert_eq!(result.mismatches.len(), 1);
        assert_eq!(result.mismatches[0].missing_kwargs, vec!["brand_case"]);
        assert!(result.mismatches[0].unused_kwargs.is_empty());
    }

    #[test]
    fn test_check_kwargs_function_literal_named_argument_does_not_require_kwarg() {
        let temp = TempDir::new().unwrap();
        write(
            &temp.path().join("code/app.py"),
            r#"i18n.get("updated", created_at=created_at)"#,
        );
        write(
            &temp.path().join("locales/uk/_default.ftl"),
            r#"updated = Updated at { DATETIME($created_at, month: "long") }
"#,
        );

        let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();
        assert!(result.mismatches.is_empty());

        write(&temp.path().join("code/app.py"), r#"i18n.get("updated")"#);
        let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();
        assert_eq!(result.mismatches.len(), 1);
        assert_eq!(result.mismatches[0].missing_kwargs, vec!["created_at"]);
        assert!(
            !result.mismatches[0]
                .missing_kwargs
                .contains(&"month".to_string())
        );
        assert!(result.mismatches[0].unused_kwargs.is_empty());
    }

    #[test]
    fn test_check_kwargs_missing_and_unused_variables_are_sorted() {
        let temp = TempDir::new().unwrap();
        write(
            &temp.path().join("code/app.py"),
            r#"i18n.get("hello", d=4, c=3)"#,
        );
        write(
            &temp.path().join("locales/uk/_default.ftl"),
            "hello = Hello { $b } { $a }\n",
        );

        let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

        assert_eq!(result.mismatches.len(), 1);
        assert_eq!(result.mismatches[0].missing_kwargs, vec!["a", "b"]);
        assert_eq!(result.mismatches[0].unused_kwargs, vec!["c", "d"]);
    }

    #[test]
    fn test_check_kwargs_recursive_terms_with_local_bindings_terminate() {
        let temp = TempDir::new().unwrap();
        write(
            &temp.path().join("code/app.py"),
            r#"i18n.get("title", brand_case=value)"#,
        );
        write(
            &temp.path().join("locales/uk/_default.ftl"),
            r#"title = { -outer(case: $brand_case) }
-outer = { -inner(case: $case) }
-inner = { -outer(case: $case) } { $case }
"#,
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

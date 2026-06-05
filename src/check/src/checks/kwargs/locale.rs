use crate::parser::CheckLocaleCache;
use extractor::ftl::utils::{FastHashMap, FastHashSet};
use fluent_syntax::ast::{
    Entry, Expression, InlineExpression, Message, Pattern, PatternElement, Term,
};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub(super) struct LocaleMessage {
    pub(super) key: String,
    pub(super) path: PathBuf,
    pub(super) line: Option<usize>,
}

#[derive(Debug)]
pub(super) struct LocaleMessages {
    pub(super) by_expected_path: FastHashMap<(String, PathBuf), LocaleMessage>,
    messages: FastHashMap<String, Message<String>>,
    terms: FastHashMap<String, Term<String>>,
    message_kwargs: FastHashMap<String, FastHashSet<String>>,
}

impl LocaleMessages {
    pub(super) fn message_kwargs(&self, key: &str) -> Option<&FastHashSet<String>> {
        self.message_kwargs.get(key)
    }

    fn build_message_kwargs(&mut self) {
        for key in self.messages.keys() {
            let kwargs = self.collect_message_kwargs_set(key);
            self.message_kwargs.insert(key.clone(), kwargs);
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
            }
            InlineExpression::StringLiteral { .. } | InlineExpression::NumberLiteral { .. } => {}
        }
    }
}

fn local_bindings_seen_key(id: &str, local_bindings: &FastHashSet<String>) -> String {
    let mut bindings = local_bindings.iter().cloned().collect::<Vec<_>>();
    bindings.sort();
    format!("{id}:{}", bindings.join(","))
}

pub(super) fn read_locale_messages_with_ast(
    cache: &CheckLocaleCache,
    locale: &str,
) -> LocaleMessages {
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

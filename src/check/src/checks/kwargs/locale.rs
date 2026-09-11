use crate::parser::CheckLocaleCache;
use common::{
    FastHashMap, FastHashSet, FluentEntries, IgnoreMarker, VariableOptions, message_variables,
};
use fluent_syntax::ast::{Entry, Message, Term};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub(super) struct LocaleMessage {
    pub(super) key: String,
    pub(super) path: PathBuf,
    pub(super) line: Option<usize>,
    /// The message carries a `# ftl-extract: ignore kwargs` (or `ignore all`) marker.
    pub(super) ignores_kwargs: bool,
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

    /// The variables each message needs, with the shared rules of [`common::message_variables`],
    /// the same ones `ftl extract` uses. Unknown references are left to the `references` check.
    fn build_message_kwargs(&mut self) {
        let options = VariableOptions {
            // `i18n.key()` renders only the value of `key`, so a variable used only in one of
            // its own attributes is never read by that call.
            include_own_attributes: false,
        };
        for (key, message) in &self.messages {
            let collected = message_variables(self, message, options);
            self.message_kwargs.insert(key.clone(), collected.variables);
        }
    }
}

impl FluentEntries for LocaleMessages {
    fn message(&self, id: &str) -> Option<&Message<String>> {
        self.messages.get(id)
    }

    fn term(&self, id: &str) -> Option<&Term<String>> {
        self.terms.get(id)
    }
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
                            ignores_kwargs: IgnoreMarker::parse(message.comment.as_ref())
                                .is_some_and(|marker| marker.ignores("kwargs")),
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

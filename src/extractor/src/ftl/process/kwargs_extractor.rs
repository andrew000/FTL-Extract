use crate::ftl::matcher::{FluentEntry, FluentKey};
use crate::ftl::utils::{FastHashMap, FastHashSet};
use anyhow::{Result, bail};
use common::{FluentEntries, UnknownReference, VariableOptions, message_variables, term_variables};
use fluent_syntax::ast::{Message, Term};

/// Collects the variables (`{ $name }`) a Fluent entry needs from the code that formats it,
/// following message and term references with the rules of [`common::message_variables`]:
/// function arguments and selectors count, `{ msg.attr }` pulls in only that attribute, and
/// nothing inside a term is a caller variable. The entry's own attributes are not counted, since
/// formatting a message renders its value only.
///
/// Messages reached through references are recorded in `depend_keys` so that the extractor keeps
/// them even when Python code never calls them directly.
///
/// Fails when an entry references a message or term that does not exist. Reference cycles are
/// tolerated.
pub(crate) fn extract_kwargs(
    key: &FluentKey,
    terms: &FastHashMap<String, FluentKey>,
    all_fluent_keys: &FastHashMap<String, FluentKey>,
    depend_keys: &mut FastHashSet<String>,
) -> Result<FastHashSet<String>> {
    let entries = StoredEntries {
        messages: all_fluent_keys,
        terms,
    };

    let collected = match key.entry.as_ref() {
        FluentEntry::Message(message) => message_variables(
            &entries,
            message,
            VariableOptions {
                include_own_attributes: false,
            },
        ),
        FluentEntry::Term(term) => term_variables(&entries, term),
        _ => return Ok(FastHashSet::default()),
    };

    depend_keys.extend(collected.referenced_messages);

    if let Some(unknown) = collected.unknown_references.first() {
        let kind = match key.entry.as_ref() {
            FluentEntry::Term(_) => "Term",
            _ => "Message",
        };
        match unknown {
            UnknownReference::Message(name) => bail!(
                "{kind} `{}` in {} references unknown message `{name}`",
                key.key,
                key.path.display()
            ),
            UnknownReference::Term(name) => bail!(
                "{kind} `{}` in {} references unknown term `-{name}`",
                key.key,
                key.path.display()
            ),
        }
    }

    Ok(collected.variables)
}

/// The stored keys of one locale, seen as a lookup for the collector.
struct StoredEntries<'a> {
    messages: &'a FastHashMap<String, FluentKey>,
    terms: &'a FastHashMap<String, FluentKey>,
}

impl FluentEntries for StoredEntries<'_> {
    fn message(&self, id: &str) -> Option<&Message<String>> {
        match self.messages.get(id)?.entry.as_ref() {
            FluentEntry::Message(message) => Some(message),
            _ => None,
        }
    }

    fn term(&self, id: &str) -> Option<&Term<String>> {
        match self.terms.get(id)?.entry.as_ref() {
            FluentEntry::Term(term) => Some(term),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::extract_kwargs;
    use crate::ftl::matcher::{FluentEntry, FluentKey};
    use crate::ftl::utils::{FastHashMap, FastHashSet};
    use fluent_syntax::ast::{
        Attribute, CallArguments, Expression, Identifier, InlineExpression, Message, Pattern,
        PatternElement, Term, Variant, VariantKey,
    };
    use std::path::PathBuf;
    use std::sync::Arc;

    fn identifier(name: &str) -> Identifier<String> {
        Identifier {
            name: name.to_string(),
        }
    }

    fn variable(name: &str) -> PatternElement<String> {
        PatternElement::Placeable {
            expression: Expression::Inline(InlineExpression::VariableReference {
                id: identifier(name),
            }),
        }
    }

    fn message_reference(name: &str) -> PatternElement<String> {
        PatternElement::Placeable {
            expression: Expression::Inline(InlineExpression::MessageReference {
                id: identifier(name),
                attribute: None,
            }),
        }
    }

    fn attribute_reference(name: &str, attribute: &str) -> PatternElement<String> {
        PatternElement::Placeable {
            expression: Expression::Inline(InlineExpression::MessageReference {
                id: identifier(name),
                attribute: Some(identifier(attribute)),
            }),
        }
    }

    fn term_reference(name: &str) -> PatternElement<String> {
        PatternElement::Placeable {
            expression: Expression::Inline(InlineExpression::TermReference {
                id: identifier(name),
                attribute: None,
                arguments: None::<CallArguments<String>>,
            }),
        }
    }

    /// `{ NAME($variable) }`
    fn function_call(name: &str, variable: &str) -> PatternElement<String> {
        PatternElement::Placeable {
            expression: Expression::Inline(InlineExpression::FunctionReference {
                id: identifier(name),
                arguments: CallArguments {
                    positional: vec![InlineExpression::VariableReference {
                        id: identifier(variable),
                    }],
                    named: vec![],
                },
            }),
        }
    }

    fn select(
        selector: &str,
        variant_elements: Vec<PatternElement<String>>,
    ) -> PatternElement<String> {
        PatternElement::Placeable {
            expression: Expression::Select {
                selector: InlineExpression::VariableReference {
                    id: identifier(selector),
                },
                variants: vec![Variant {
                    key: VariantKey::Identifier {
                        name: "other".to_string(),
                    },
                    value: Pattern {
                        elements: variant_elements,
                    },
                    default: true,
                }],
            },
        }
    }

    fn message(name: &str, elements: Vec<PatternElement<String>>) -> FluentKey {
        message_with_attributes(name, elements, vec![])
    }

    fn message_with_attributes(
        name: &str,
        elements: Vec<PatternElement<String>>,
        attributes: Vec<(&str, Vec<PatternElement<String>>)>,
    ) -> FluentKey {
        FluentKey::new(
            Arc::new(PathBuf::from("tmp.py")),
            name.to_string(),
            FluentEntry::Message(Message {
                id: identifier(name),
                value: Some(Pattern { elements }),
                attributes: attributes
                    .into_iter()
                    .map(|(id, elements)| Attribute {
                        id: identifier(id),
                        value: Pattern { elements },
                    })
                    .collect(),
                comment: None,
            }),
            Arc::new(PathBuf::from("tmp.ftl")),
            Some("en".to_string()),
            Some(0),
            FastHashSet::default(),
        )
    }

    fn term(name: &str, elements: Vec<PatternElement<String>>) -> FluentKey {
        FluentKey::new(
            Arc::new(PathBuf::from("tmp.py")),
            name.to_string(),
            FluentEntry::Term(Term {
                id: identifier(name),
                value: Pattern { elements },
                attributes: vec![],
                comment: None,
            }),
            Arc::new(PathBuf::from("tmp.ftl")),
            Some("en".to_string()),
            Some(0),
            FastHashSet::default(),
        )
    }

    fn keyed(keys: Vec<FluentKey>) -> FastHashMap<String, FluentKey> {
        keys.into_iter().map(|key| (key.key.clone(), key)).collect()
    }

    fn names(set: &FastHashSet<String>) -> Vec<&str> {
        let mut names = set.iter().map(String::as_str).collect::<Vec<_>>();
        names.sort_unstable();
        names
    }

    #[test]
    fn test_message_variables() {
        let key = message("msg", vec![variable("username"), variable("count")]);
        let mut depend_keys = FastHashSet::default();

        let kwargs = extract_kwargs(
            &key,
            &FastHashMap::default(),
            &FastHashMap::default(),
            &mut depend_keys,
        )
        .unwrap();

        assert_eq!(names(&kwargs), vec!["count", "username"]);
        assert!(depend_keys.is_empty());
    }

    #[test]
    fn test_function_arguments_are_variables() {
        let key = message(
            "msg",
            vec![function_call("NUMBER", "count"), variable("username")],
        );

        let kwargs = extract_kwargs(
            &key,
            &FastHashMap::default(),
            &FastHashMap::default(),
            &mut FastHashSet::default(),
        )
        .unwrap();

        assert_eq!(names(&kwargs), vec!["count", "username"]);
    }

    #[test]
    fn test_term_variables() {
        let key = term("brand", vec![variable("username")]);

        let kwargs = extract_kwargs(
            &key,
            &FastHashMap::default(),
            &FastHashMap::default(),
            &mut FastHashSet::default(),
        )
        .unwrap();

        assert_eq!(names(&kwargs), vec!["username"]);
    }

    #[test]
    fn test_empty_and_valueless_entries_have_no_kwargs() {
        let empty_message = message("msg", vec![]);
        let mut valueless = message("attrs", vec![]);
        valueless.entry = Arc::new(FluentEntry::Message(Message {
            id: identifier("attrs"),
            value: None,
            attributes: vec![],
            comment: None,
        }));
        let comment = FluentKey::new(
            Arc::new(PathBuf::new()),
            String::new(),
            FluentEntry::Comment(fluent_syntax::ast::Comment { content: vec![] }),
            Arc::new(PathBuf::from("tmp.ftl")),
            None,
            None,
            FastHashSet::default(),
        );

        for key in [empty_message, valueless, comment] {
            let kwargs = extract_kwargs(
                &key,
                &FastHashMap::default(),
                &FastHashMap::default(),
                &mut FastHashSet::default(),
            )
            .unwrap();
            assert!(kwargs.is_empty());
        }
    }

    #[test]
    fn test_own_attributes_are_not_kwargs() {
        let key = message_with_attributes(
            "btn",
            vec![variable("label")],
            vec![("title", vec![variable("tooltip")])],
        );

        let kwargs = extract_kwargs(
            &key,
            &FastHashMap::default(),
            &FastHashMap::default(),
            &mut FastHashSet::default(),
        )
        .unwrap();

        assert_eq!(names(&kwargs), vec!["label"]);
    }

    #[test]
    fn test_message_reference_pulls_variables_and_records_dependency() {
        let key = message("msg", vec![message_reference("ref_msg")]);
        let messages = keyed(vec![message("ref_msg", vec![variable("username")])]);
        let mut depend_keys = FastHashSet::default();

        let kwargs =
            extract_kwargs(&key, &FastHashMap::default(), &messages, &mut depend_keys).unwrap();

        assert_eq!(names(&kwargs), vec!["username"]);
        assert_eq!(names(&depend_keys), vec!["ref_msg"]);
    }

    #[test]
    fn test_attribute_reference_pulls_only_that_attribute_and_records_dependency() {
        let key = message("msg", vec![attribute_reference("btn", "title")]);
        let messages = keyed(vec![message_with_attributes(
            "btn",
            vec![variable("label")],
            vec![
                ("title", vec![variable("tooltip")]),
                ("aria", vec![variable("aria")]),
            ],
        )]);
        let mut depend_keys = FastHashSet::default();

        let kwargs =
            extract_kwargs(&key, &FastHashMap::default(), &messages, &mut depend_keys).unwrap();

        assert_eq!(names(&kwargs), vec!["tooltip"]);
        assert_eq!(names(&depend_keys), vec!["btn"]);
    }

    #[test]
    fn test_term_reference_does_not_pull_term_variables() {
        // Variables inside a term resolve against the term's call arguments, never the caller's.
        let key = message("msg", vec![term_reference("brand")]);
        let terms = keyed(vec![term("brand", vec![variable("company")])]);
        let mut depend_keys = FastHashSet::default();

        let kwargs =
            extract_kwargs(&key, &terms, &FastHashMap::default(), &mut depend_keys).unwrap();

        assert!(kwargs.is_empty());
        assert!(
            depend_keys.is_empty(),
            "terms are not tracked as dependencies"
        );
    }

    #[test]
    fn test_select_expression_collects_selector_and_variant_variables() {
        let key = message(
            "msg",
            vec![select(
                "user_role",
                vec![variable("username"), message_reference("ref_msg")],
            )],
        );
        let messages = keyed(vec![message("ref_msg", vec![variable("nested")])]);

        let kwargs = extract_kwargs(
            &key,
            &FastHashMap::default(),
            &messages,
            &mut FastHashSet::default(),
        )
        .unwrap();

        assert_eq!(names(&kwargs), vec!["nested", "user_role", "username"]);
    }

    #[test]
    fn test_missing_message_reference_is_an_error_not_a_panic() {
        let key = message("msg", vec![message_reference("ref_msg")]);

        let error = extract_kwargs(
            &key,
            &FastHashMap::default(),
            &FastHashMap::default(),
            &mut FastHashSet::default(),
        )
        .unwrap_err();

        assert_eq!(
            error.to_string(),
            "Message `msg` in tmp.ftl references unknown message `ref_msg`"
        );
    }

    #[test]
    fn test_missing_term_reference_is_an_error_not_a_panic() {
        let key = term("outer", vec![term_reference("brand")]);

        let error = extract_kwargs(
            &key,
            &FastHashMap::default(),
            &FastHashMap::default(),
            &mut FastHashSet::default(),
        )
        .unwrap_err();

        assert_eq!(
            error.to_string(),
            "Term `outer` in tmp.ftl references unknown term `-brand`"
        );
    }

    #[test]
    fn test_missing_reference_inside_a_term_is_still_an_error() {
        let key = message("msg", vec![term_reference("brand")]);
        let terms = keyed(vec![term("brand", vec![message_reference("gone")])]);

        let error = extract_kwargs(
            &key,
            &terms,
            &FastHashMap::default(),
            &mut FastHashSet::default(),
        )
        .unwrap_err();

        assert_eq!(
            error.to_string(),
            "Message `msg` in tmp.ftl references unknown message `gone`"
        );
    }

    #[test]
    fn test_reference_cycles_terminate() {
        let messages = keyed(vec![
            message("a", vec![variable("x"), message_reference("b")]),
            message("b", vec![variable("y"), message_reference("a")]),
            message(
                "self_ref",
                vec![message_reference("self_ref"), variable("z")],
            ),
        ]);
        let terms = keyed(vec![
            term("t1", vec![term_reference("t2")]),
            term("t2", vec![term_reference("t1"), variable("w")]),
        ]);
        let mut depend_keys = FastHashSet::default();

        let kwargs = extract_kwargs(&messages["a"], &terms, &messages, &mut depend_keys).unwrap();
        assert_eq!(names(&kwargs), vec!["x", "y"]);
        assert_eq!(names(&depend_keys), vec!["a", "b"]);

        let kwargs =
            extract_kwargs(&messages["self_ref"], &terms, &messages, &mut depend_keys).unwrap();
        assert_eq!(names(&kwargs), vec!["z"]);

        // `t1` itself has no variables; `$w` belongs to `-t2`, which binds its own arguments.
        let kwargs = extract_kwargs(&terms["t1"], &terms, &messages, &mut depend_keys).unwrap();
        assert!(kwargs.is_empty());
        let kwargs = extract_kwargs(&terms["t2"], &terms, &messages, &mut depend_keys).unwrap();
        assert_eq!(names(&kwargs), vec!["w"]);
    }
}

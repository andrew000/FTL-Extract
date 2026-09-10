use crate::ftl::matcher::{FluentEntry, FluentKey};
use crate::ftl::utils::{FastHashMap, FastHashSet};
use anyhow::{Result, bail};
use fluent_syntax::ast::{
    Expression, Identifier, InlineExpression, Message, Pattern, PatternElement, Term,
};

/// Collects the variables (`{ $name }`) a Fluent entry needs, following message and term
/// references. Messages reached through references are recorded in `depend_keys` so that
/// the extractor keeps them even when Python code never calls them directly.
///
/// Fails when an entry references a message or term that does not exist. Reference cycles
/// are tolerated: every entry is visited at most once per call.
pub(crate) fn extract_kwargs(
    key: &FluentKey,
    terms: &FastHashMap<String, FluentKey>,
    all_fluent_keys: &FastHashMap<String, FluentKey>,
    depend_keys: &mut FastHashSet<String>,
) -> Result<FastHashSet<String>> {
    let mut collector = KwargsCollector {
        origin: key,
        terms,
        messages: all_fluent_keys,
        depend_keys,
        visited: FastHashSet::default(),
        kwargs: FastHashSet::default(),
    };
    collector.visit_key(key)?;
    Ok(collector.kwargs)
}

struct KwargsCollector<'a> {
    /// The entry the traversal started from; used in error messages.
    origin: &'a FluentKey,
    terms: &'a FastHashMap<String, FluentKey>,
    messages: &'a FastHashMap<String, FluentKey>,
    depend_keys: &'a mut FastHashSet<String>,
    /// Entries already visited in this traversal, as `name` for messages and `-name` for terms.
    visited: FastHashSet<String>,
    kwargs: FastHashSet<String>,
}

impl KwargsCollector<'_> {
    fn visit_key(&mut self, key: &FluentKey) -> Result<()> {
        match key.entry.as_ref() {
            FluentEntry::Message(message) => self.visit_message(message),
            FluentEntry::Term(term) => self.visit_term(term),
            _ => Ok(()),
        }
    }

    fn visit_message(&mut self, message: &Message<String>) -> Result<()> {
        if !self.visited.insert(message.id.name.clone()) {
            return Ok(());
        }
        match message.value.as_ref() {
            Some(pattern) => self.visit_pattern(pattern),
            None => Ok(()),
        }
    }

    fn visit_term(&mut self, term: &Term<String>) -> Result<()> {
        if !self.visited.insert(format!("-{}", term.id.name)) {
            return Ok(());
        }
        self.visit_pattern(&term.value)
    }

    fn visit_pattern(&mut self, pattern: &Pattern<String>) -> Result<()> {
        for element in &pattern.elements {
            if let PatternElement::Placeable { expression } = element {
                self.visit_expression(expression)?;
            }
        }
        Ok(())
    }

    fn visit_expression(&mut self, expression: &Expression<String>) -> Result<()> {
        match expression {
            Expression::Inline(inline) => self.visit_inline(inline),
            Expression::Select { selector, variants } => {
                self.visit_inline(selector)?;
                for variant in variants {
                    self.visit_pattern(&variant.value)?;
                }
                Ok(())
            }
        }
    }

    fn visit_inline(&mut self, inline: &InlineExpression<String>) -> Result<()> {
        match inline {
            InlineExpression::VariableReference { id } => {
                self.kwargs.insert(id.name.clone());
                Ok(())
            }
            InlineExpression::MessageReference { id, .. } => self.visit_message_reference(id),
            InlineExpression::TermReference { id, .. } => self.visit_term_reference(id),
            InlineExpression::Placeable { expression } => self.visit_expression(expression),
            InlineExpression::StringLiteral { .. }
            | InlineExpression::NumberLiteral { .. }
            | InlineExpression::FunctionReference { .. } => Ok(()),
        }
    }

    fn visit_message_reference(&mut self, id: &Identifier<String>) -> Result<()> {
        self.depend_keys.insert(id.name.clone());

        let Some(referenced) = self.messages.get(&id.name) else {
            bail!(
                "{} `{}` in {} references unknown message `{}`",
                self.origin_kind(),
                self.origin.key,
                self.origin.path.display(),
                id.name
            );
        };
        let referenced = referenced.clone();
        self.visit_key(&referenced)
    }

    fn visit_term_reference(&mut self, id: &Identifier<String>) -> Result<()> {
        let Some(term) = self.terms.get(&id.name) else {
            bail!(
                "{} `{}` in {} references unknown term `-{}`",
                self.origin_kind(),
                self.origin.key,
                self.origin.path.display(),
                id.name
            );
        };
        let term = term.clone();
        self.visit_key(&term)
    }

    fn origin_kind(&self) -> &'static str {
        match self.origin.entry.as_ref() {
            FluentEntry::Term(_) => "Term",
            _ => "Message",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::extract_kwargs;
    use crate::ftl::matcher::{FluentEntry, FluentKey};
    use crate::ftl::utils::{FastHashMap, FastHashSet};
    use fluent_syntax::ast::{
        CallArguments, Expression, Identifier, InlineExpression, Message, Pattern, PatternElement,
        Term, Variant, VariantKey,
    };
    use std::path::PathBuf;
    use std::sync::Arc;

    fn variable(name: &str) -> PatternElement<String> {
        PatternElement::Placeable {
            expression: Expression::Inline(InlineExpression::VariableReference {
                id: Identifier {
                    name: name.to_string(),
                },
            }),
        }
    }

    fn message_reference(name: &str) -> PatternElement<String> {
        PatternElement::Placeable {
            expression: Expression::Inline(InlineExpression::MessageReference {
                id: Identifier {
                    name: name.to_string(),
                },
                attribute: None,
            }),
        }
    }

    fn term_reference(name: &str) -> PatternElement<String> {
        PatternElement::Placeable {
            expression: Expression::Inline(InlineExpression::TermReference {
                id: Identifier {
                    name: name.to_string(),
                },
                attribute: None,
                arguments: None::<CallArguments<String>>,
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
                    id: Identifier {
                        name: selector.to_string(),
                    },
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
        FluentKey::new(
            Arc::new(PathBuf::from("tmp.py")),
            name.to_string(),
            FluentEntry::Message(Message {
                id: Identifier {
                    name: name.to_string(),
                },
                value: Some(Pattern { elements }),
                attributes: vec![],
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
                id: Identifier {
                    name: name.to_string(),
                },
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
            id: Identifier {
                name: "attrs".to_string(),
            },
            value: None,
            attributes: vec![],
            comment: None,
        }));
        let comment = FluentKey::new(
            Arc::new(PathBuf::new()),
            String::new(),
            FluentEntry::Junk("junk".to_string()),
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
    fn test_term_reference_pulls_variables() {
        let key = message("msg", vec![term_reference("brand")]);
        let terms = keyed(vec![term("brand", vec![variable("company")])]);
        let mut depend_keys = FastHashSet::default();

        let kwargs =
            extract_kwargs(&key, &terms, &FastHashMap::default(), &mut depend_keys).unwrap();

        assert_eq!(names(&kwargs), vec!["company"]);
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

        let kwargs = extract_kwargs(&terms["t1"], &terms, &messages, &mut depend_keys).unwrap();
        assert_eq!(names(&kwargs), vec!["w"]);
    }
}

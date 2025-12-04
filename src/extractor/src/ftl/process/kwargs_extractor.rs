#![allow(unused_variables)]

use crate::ftl::matcher::{FluentEntry, FluentKey};
use crate::ftl::utils::{FastHashMap, FastHashSet};
use anyhow::{Result, bail};
use fluent_syntax::ast::{CallArguments, Identifier};

pub(crate) fn extract_kwargs(
    key: &mut FluentKey,
    terms: &mut FastHashMap<String, FluentKey>,
    all_fluent_keys: &FastHashMap<String, FluentKey>,
    depend_keys: &mut FastHashSet<String>,
) -> FastHashSet<String> {
    let mut kwargs: FastHashSet<String> = FastHashSet::default();

    if let FluentEntry::Message(message) = &key.entry.as_ref() {
        if message.value.is_none() || message.value.as_ref().unwrap().elements.is_empty() {
            return kwargs;
        }

        extract_kwargs_from_message(key, &mut kwargs, terms, all_fluent_keys, depend_keys);
    } else if let FluentEntry::Term(term) = &key.entry.as_ref() {
        if term.value.elements.is_empty() {
            return kwargs;
        }

        extract_kwargs_from_term(key, &mut kwargs, terms, all_fluent_keys, depend_keys)
    };

    kwargs
}

fn extract_kwargs_from_placeable(
    key: &mut FluentKey,
    placeable: &fluent_syntax::ast::PatternElement<String>,
    kwargs: &mut FastHashSet<String>,
    terms: &mut FastHashMap<String, FluentKey>,
    all_fluent_keys: &FastHashMap<String, FluentKey>,
    depend_keys: &mut FastHashSet<String>,
) {
    if let fluent_syntax::ast::PatternElement::Placeable { expression } = placeable {
        if let fluent_syntax::ast::Expression::Inline(inline_expr) = expression {
            if let fluent_syntax::ast::InlineExpression::VariableReference { id } = inline_expr {
                extract_kwargs_from_variable_reference(id, kwargs);
            } else if let fluent_syntax::ast::InlineExpression::MessageReference { id, attribute } =
                inline_expr
            {
                // Add `ast.MessageReference.id.name` to depends_on_keys to avoid key to be removed
                key.depends_on_keys.insert(id.name.clone());
                depend_keys.insert(id.name.clone());

                // Extract kwargs
                extract_kwargs_from_message_reference(
                    key,
                    id,
                    attribute,
                    kwargs,
                    terms,
                    all_fluent_keys,
                    depend_keys,
                )
                .unwrap()
            } else if let fluent_syntax::ast::InlineExpression::TermReference {
                id,
                attribute,
                arguments,
            } = inline_expr
            {
                extract_kwargs_from_term_reference(
                    key,
                    id,
                    attribute,
                    arguments,
                    kwargs,
                    terms,
                    all_fluent_keys,
                )
                .unwrap()
            }
        } else if let fluent_syntax::ast::Expression::Select { selector, variants } = expression {
            extract_kwargs_from_selector_expression(
                key,
                selector,
                variants,
                kwargs,
                terms,
                all_fluent_keys,
                depend_keys,
            );
        }
    }
}

fn extract_kwargs_from_variable_reference(
    variable_reference: &Identifier<String>,
    kwargs: &mut FastHashSet<String>,
) {
    kwargs.insert(variable_reference.name.clone());
}

fn extract_kwargs_from_message_reference(
    key: &mut FluentKey,
    id: &Identifier<String>,
    attribute: &Option<Identifier<String>>,
    kwargs: &mut FastHashSet<String>,
    terms: &mut FastHashMap<String, FluentKey>,
    all_fluent_keys: &FastHashMap<String, FluentKey>,
    depend_keys: &mut FastHashSet<String>,
) -> Result<()> {
    let mut reference_key = match all_fluent_keys.get(&id.name) {
        Some(key) => key.clone(),
        None => {
            bail!(
                "Can't find reference key {} in {}",
                id.name,
                key.path.display()
            )
        }
    };

    kwargs.extend(extract_kwargs(
        &mut reference_key,
        terms,
        all_fluent_keys,
        depend_keys,
    ));

    Ok(())
}

fn extract_kwargs_from_term_reference(
    key: &mut FluentKey,
    id: &Identifier<String>,
    attribute: &Option<Identifier<String>>,
    arguments: &Option<CallArguments<String>>,
    kwargs: &mut FastHashSet<String>,
    terms: &mut FastHashMap<String, FluentKey>,
    all_fluent_keys: &FastHashMap<String, FluentKey>,
) -> Result<()> {
    let mut term = match terms.get(&id.name) {
        Some(term) => term.clone(),
        None => {
            bail!(
                "Can't find reference key {} in {}",
                id.name,
                key.path.display()
            )
        }
    };

    kwargs.extend(extract_kwargs(
        &mut term,
        terms,
        all_fluent_keys,
        &mut FastHashSet::default(),
    ));

    Ok(())
}

fn extract_kwargs_from_selector_expression(
    key: &mut FluentKey,
    selector: &fluent_syntax::ast::InlineExpression<String>,
    variants: &Vec<fluent_syntax::ast::Variant<String>>,
    kwargs: &mut FastHashSet<String>,
    terms: &mut FastHashMap<String, FluentKey>,
    all_fluent_keys: &FastHashMap<String, FluentKey>,
    depend_keys: &mut FastHashSet<String>,
) {
    if let fluent_syntax::ast::InlineExpression::VariableReference { id } = selector {
        extract_kwargs_from_variable_reference(id, kwargs);
    }

    for variant in variants {
        for element in &variant.value.elements {
            if let fluent_syntax::ast::PatternElement::Placeable { expression } = element {
                extract_kwargs_from_placeable(
                    key,
                    element,
                    kwargs,
                    terms,
                    all_fluent_keys,
                    depend_keys,
                );
            }
        }
    }
}

fn extract_kwargs_from_message(
    key: &mut FluentKey,
    kwargs: &mut FastHashSet<String>,
    terms: &mut FastHashMap<String, FluentKey>,
    all_fluent_keys: &FastHashMap<String, FluentKey>,
    depend_keys: &mut FastHashSet<String>,
) {
    let elements = if let FluentEntry::Message(message) = &key.entry.as_ref() {
        message.value.as_ref().unwrap().elements.clone()
    } else {
        panic!("Expected a Message entry");
    };

    for element in elements {
        if let fluent_syntax::ast::PatternElement::Placeable { expression } = &element {
            extract_kwargs_from_placeable(
                key,
                &element,
                kwargs,
                terms,
                all_fluent_keys,
                depend_keys,
            );
        }
    }
}

fn extract_kwargs_from_term(
    key: &mut FluentKey,
    kwargs: &mut FastHashSet<String>,
    terms: &mut FastHashMap<String, FluentKey>,
    all_fluent_keys: &FastHashMap<String, FluentKey>,
    depend_keys: &mut FastHashSet<String>,
) {
    let elements = if let FluentEntry::Term(term) = &key.entry.as_ref() {
        term.value.elements.clone()
    } else {
        panic!("Expected a Term entry");
    };

    for element in elements {
        if let fluent_syntax::ast::PatternElement::Placeable { expression } = &element {
            extract_kwargs_from_placeable(
                key,
                &element,
                kwargs,
                terms,
                all_fluent_keys,
                depend_keys,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ftl::matcher::{FluentEntry, FluentKey};
    use crate::ftl::utils::{FastHashMap, FastHashSet};
    use std::path::PathBuf;
    use std::sync::Arc;

    #[test]
    fn test_extract_kwargs_message() {
        let mut key = FluentKey::new(
            Arc::new(PathBuf::from("tmp.py")), // code_path
            String::from("msg"),               // key
            FluentEntry::Message(fluent_syntax::ast::Message {
                id: fluent_syntax::ast::Identifier {
                    name: "msg".to_string(),
                },
                value: Some(fluent_syntax::ast::Pattern {
                    elements: vec![fluent_syntax::ast::PatternElement::Placeable {
                        expression: fluent_syntax::ast::Expression::Inline(
                            fluent_syntax::ast::InlineExpression::VariableReference {
                                id: fluent_syntax::ast::Identifier {
                                    name: "username".to_string(),
                                },
                            },
                        ),
                    }],
                }),
                attributes: vec![],
                comment: None,
            }), // entry
            Arc::new(PathBuf::from("tmp.ftl")), // path
            Some("en".to_string()),            // locale
            Some(0),
            FastHashSet::default(),
        );
        let mut kwargs: FastHashSet<String> = FastHashSet::default();

        super::extract_kwargs_from_message(
            &mut key,
            &mut kwargs,
            &mut FastHashMap::<String, FluentKey>::default(),
            &FastHashMap::<String, FluentKey>::default(),
            &mut FastHashSet::<String>::default(),
        );

        assert!(kwargs.contains("username"));
    }

    #[test]
    fn test_extract_kwargs_term() {
        let mut key = FluentKey::new(
            Arc::new(PathBuf::from("tmp.py")), // code_path
            String::from("term"),              // key
            FluentEntry::Term(fluent_syntax::ast::Term {
                id: fluent_syntax::ast::Identifier {
                    name: "term".to_string(),
                },
                value: fluent_syntax::ast::Pattern {
                    elements: vec![fluent_syntax::ast::PatternElement::Placeable {
                        expression: fluent_syntax::ast::Expression::Inline(
                            fluent_syntax::ast::InlineExpression::VariableReference {
                                id: fluent_syntax::ast::Identifier {
                                    name: "username".to_string(),
                                },
                            },
                        ),
                    }],
                },
                attributes: vec![],
                comment: None,
            }), // entry
            Arc::new(PathBuf::from("tmp.ftl")), // path
            Some("en".to_string()),            // locale
            Some(0),
            FastHashSet::default(),
        );
        let mut kwargs: FastHashSet<String> = FastHashSet::default();

        super::extract_kwargs_from_term(
            &mut key,
            &mut kwargs,
            &mut FastHashMap::<String, FluentKey>::default(),
            &FastHashMap::<String, FluentKey>::default(),
            &mut FastHashSet::<String>::default(),
        );

        assert!(kwargs.contains("username"));
    }

    #[test]
    fn test_extract_kwargs_from_placeable_variable_reference() {
        let mut key = FluentKey::new(
            Arc::new(PathBuf::from("tmp.py")), // code_path
            String::from("msg"),               // key
            FluentEntry::Message(fluent_syntax::ast::Message {
                id: fluent_syntax::ast::Identifier {
                    name: "msg".to_string(),
                },
                value: Some(fluent_syntax::ast::Pattern {
                    elements: vec![fluent_syntax::ast::PatternElement::Placeable {
                        expression: fluent_syntax::ast::Expression::Inline(
                            fluent_syntax::ast::InlineExpression::VariableReference {
                                id: fluent_syntax::ast::Identifier {
                                    name: "username".to_string(),
                                },
                            },
                        ),
                    }],
                }),
                attributes: vec![],
                comment: None,
            }), // entry
            Arc::new(PathBuf::from("tmp.ftl")), // path
            Some("en".to_string()),            // locale
            Some(0),
            FastHashSet::default(),
        );
        let placeable = fluent_syntax::ast::PatternElement::Placeable {
            expression: fluent_syntax::ast::Expression::Inline(
                fluent_syntax::ast::InlineExpression::VariableReference {
                    id: fluent_syntax::ast::Identifier {
                        name: "username".to_string(),
                    },
                },
            ),
        };
        let mut kwargs: FastHashSet<String> = FastHashSet::default();

        super::extract_kwargs_from_placeable(
            &mut key,
            &placeable,
            &mut kwargs,
            &mut FastHashMap::<String, FluentKey>::default(),
            &FastHashMap::<String, FluentKey>::default(),
            &mut FastHashSet::<String>::default(),
        );

        assert!(kwargs.contains("username"));
    }

    #[test]
    fn test_extract_kwargs_from_placeable_message_reference() {
        let mut key = FluentKey::new(
            Arc::new(PathBuf::from("tmp.py")), // code_path
            String::from("msg"),               // key
            FluentEntry::Message(fluent_syntax::ast::Message {
                id: fluent_syntax::ast::Identifier {
                    name: "msg".to_string(),
                },
                value: Some(fluent_syntax::ast::Pattern {
                    elements: vec![fluent_syntax::ast::PatternElement::Placeable {
                        expression: fluent_syntax::ast::Expression::Inline(
                            fluent_syntax::ast::InlineExpression::MessageReference {
                                id: fluent_syntax::ast::Identifier {
                                    name: "ref_msg".to_string(),
                                },
                                attribute: None,
                            },
                        ),
                    }],
                }),
                attributes: vec![],
                comment: None,
            }), // entry
            Arc::new(PathBuf::from("tmp.ftl")), // path
            Some("en".to_string()),            // locale
            Some(0),
            FastHashSet::default(),
        );
        let placeable = fluent_syntax::ast::PatternElement::Placeable {
            expression: fluent_syntax::ast::Expression::Inline(
                fluent_syntax::ast::InlineExpression::MessageReference {
                    id: fluent_syntax::ast::Identifier {
                        name: "ref_msg".to_string(),
                    },
                    attribute: None,
                },
            ),
        };
        let mut kwargs: FastHashSet<String> = FastHashSet::default();
        let mut all_fluent_keys: FastHashMap<String, FluentKey> = FastHashMap::default();
        all_fluent_keys.insert(
            "ref_msg".to_string(),
            FluentKey::new(
                Arc::new(PathBuf::from("tmp.py")), // code_path
                String::from("ref_msg"),           // key
                FluentEntry::Message(fluent_syntax::ast::Message {
                    id: fluent_syntax::ast::Identifier {
                        name: "ref_msg".to_string(),
                    },
                    value: Some(fluent_syntax::ast::Pattern { elements: vec![] }),
                    attributes: vec![],
                    comment: None,
                }), // entry
                Arc::new(PathBuf::from("tmp.ftl")), // path
                Some("en".to_string()),            // locale
                Some(0),
                FastHashSet::default(),
            ),
        );

        super::extract_kwargs_from_placeable(
            &mut key,
            &placeable,
            &mut kwargs,
            &mut FastHashMap::<String, FluentKey>::default(),
            &all_fluent_keys,
            &mut FastHashSet::<String>::default(),
        );

        // Since all_fluent_keys is empty, no kwargs should be extracted
        assert!(!kwargs.contains("ref_msg"));
    }

    #[test]
    fn test_extract_kwargs_from_placeable_term_reference() {
        let mut key = FluentKey::new(
            Arc::new(PathBuf::from("tmp.py")), // code_path
            String::from("msg"),               // key
            FluentEntry::Message(fluent_syntax::ast::Message {
                id: fluent_syntax::ast::Identifier {
                    name: "msg".to_string(),
                },
                value: Some(fluent_syntax::ast::Pattern {
                    elements: vec![fluent_syntax::ast::PatternElement::Placeable {
                        expression: fluent_syntax::ast::Expression::Inline(
                            fluent_syntax::ast::InlineExpression::TermReference {
                                id: fluent_syntax::ast::Identifier {
                                    name: "term".to_string(),
                                },
                                attribute: None,
                                arguments: None,
                            },
                        ),
                    }],
                }),
                attributes: vec![],
                comment: None,
            }), // entry
            Arc::new(PathBuf::from("tmp.ftl")), // path
            Some("en".to_string()),            // locale
            Some(0),
            FastHashSet::default(),
        );
        let placeable = fluent_syntax::ast::PatternElement::Placeable {
            expression: fluent_syntax::ast::Expression::Inline(
                fluent_syntax::ast::InlineExpression::TermReference {
                    id: fluent_syntax::ast::Identifier {
                        name: "term".to_string(),
                    },
                    attribute: None,
                    arguments: None,
                },
            ),
        };
        let mut kwargs: FastHashSet<String> = FastHashSet::default();
        let mut terms: FastHashMap<String, FluentKey> = FastHashMap::default();
        terms.insert(
            "term".to_string(),
            FluentKey::new(
                Arc::new(PathBuf::from("tmp.py")), // code_path
                String::from("term"),              // key
                FluentEntry::Term(fluent_syntax::ast::Term {
                    id: fluent_syntax::ast::Identifier {
                        name: "term".to_string(),
                    },
                    value: fluent_syntax::ast::Pattern { elements: vec![] },
                    attributes: vec![],
                    comment: None,
                }), // entry
                Arc::new(PathBuf::from("tmp.ftl")), // path
                Some("en".to_string()),            // locale
                Some(0),
                FastHashSet::default(),
            ),
        );

        super::extract_kwargs_from_placeable(
            &mut key,
            &placeable,
            &mut kwargs,
            &mut terms,
            &FastHashMap::<String, FluentKey>::default(),
            &mut FastHashSet::<String>::default(),
        );

        assert!(!kwargs.contains("term"));
    }

    #[test]
    fn test_extract_kwargs_from_variable_reference() {
        let mut kwargs: FastHashSet<String> = FastHashSet::default();

        super::extract_kwargs_from_variable_reference(
            &fluent_syntax::ast::Identifier {
                name: "username".to_string(),
            },
            &mut kwargs,
        );

        assert!(kwargs.contains("username"));
    }

    #[test]
    fn test_extract_kwargs_from_placeable_select() {
        let mut key = FluentKey::new(
            Arc::new(PathBuf::from("tmp.py")), // code_path
            String::from("msg"),               // key
            FluentEntry::Message(fluent_syntax::ast::Message {
                id: fluent_syntax::ast::Identifier {
                    name: "msg".to_string(),
                },
                value: Some(fluent_syntax::ast::Pattern {
                    elements: vec![fluent_syntax::ast::PatternElement::Placeable {
                        expression: fluent_syntax::ast::Expression::Select {
                            selector: fluent_syntax::ast::InlineExpression::VariableReference {
                                id: fluent_syntax::ast::Identifier {
                                    name: "user_role".to_string(),
                                },
                            },
                            variants: vec![],
                        },
                    }],
                }),
                attributes: vec![],
                comment: None,
            }), // entry
            Arc::new(PathBuf::from("tmp.ftl")), // path
            Some("en".to_string()),            // locale
            Some(0),
            FastHashSet::default(),
        );
        let placeable = fluent_syntax::ast::PatternElement::Placeable {
            expression: fluent_syntax::ast::Expression::Select {
                selector: fluent_syntax::ast::InlineExpression::VariableReference {
                    id: fluent_syntax::ast::Identifier {
                        name: "user_role".to_string(),
                    },
                },
                variants: vec![],
            },
        };
        let mut kwargs: FastHashSet<String> = FastHashSet::default();

        super::extract_kwargs_from_placeable(
            &mut key,
            &placeable,
            &mut kwargs,
            &mut FastHashMap::<String, FluentKey>::default(),
            &FastHashMap::<String, FluentKey>::default(),
            &mut FastHashSet::<String>::default(),
        );

        assert!(kwargs.contains("user_role"));
    }

    #[test]
    fn test_extract_kwargs_from_message_reference() {
        let mut key = FluentKey::new(
            Arc::new(PathBuf::from("tmp.py")), // code_path
            String::from("msg"),               // key
            FluentEntry::Message(fluent_syntax::ast::Message {
                id: fluent_syntax::ast::Identifier {
                    name: "msg".to_string(),
                },
                value: Some(fluent_syntax::ast::Pattern {
                    elements: vec![fluent_syntax::ast::PatternElement::Placeable {
                        expression: fluent_syntax::ast::Expression::Inline(
                            fluent_syntax::ast::InlineExpression::MessageReference {
                                id: fluent_syntax::ast::Identifier {
                                    name: "ref_msg".to_string(),
                                },
                                attribute: None,
                            },
                        ),
                    }],
                }),
                attributes: vec![],
                comment: None,
            }), // entry
            Arc::new(PathBuf::from("tmp.ftl")), // path
            Some("en".to_string()),            // locale
            Some(0),
            FastHashSet::default(),
        );
        let mut kwargs: FastHashSet<String> = FastHashSet::default();
        let mut all_fluent_keys: FastHashMap<String, FluentKey> = FastHashMap::default();
        all_fluent_keys.insert(
            "ref_msg".to_string(),
            FluentKey::new(
                Arc::new(PathBuf::from("tmp.py")), // code_path
                String::from("ref_msg"),           // key
                FluentEntry::Message(fluent_syntax::ast::Message {
                    id: fluent_syntax::ast::Identifier {
                        name: "ref_msg".to_string(),
                    },
                    value: Some(fluent_syntax::ast::Pattern {
                        elements: vec![fluent_syntax::ast::PatternElement::Placeable {
                            expression: fluent_syntax::ast::Expression::Inline(
                                fluent_syntax::ast::InlineExpression::VariableReference {
                                    id: fluent_syntax::ast::Identifier {
                                        name: "username".to_string(),
                                    },
                                },
                            ),
                        }],
                    }),
                    attributes: vec![],
                    comment: None,
                }), // entry
                Arc::new(PathBuf::from("tmp.ftl")), // path
                Some("en".to_string()),            // locale
                Some(0),
                FastHashSet::default(),
            ),
        );

        super::extract_kwargs_from_message_reference(
            &mut key,
            &fluent_syntax::ast::Identifier {
                name: "ref_msg".to_string(),
            },
            &None,
            &mut kwargs,
            &mut FastHashMap::<String, FluentKey>::default(),
            &all_fluent_keys,
            &mut FastHashSet::default(),
        )
        .unwrap();

        assert!(kwargs.contains("username"));
    }

    #[test]
    #[should_panic(expected = "Can't find reference key ref_msg in tmp.ftl")]
    fn test_extract_kwargs_from_message_reference_panic_when_no_message() {
        let mut key = FluentKey::new(
            Arc::new(PathBuf::from("tmp.py")), // code_path
            String::from("msg"),               // key
            FluentEntry::Message(fluent_syntax::ast::Message {
                id: fluent_syntax::ast::Identifier {
                    name: "msg".to_string(),
                },
                value: Some(fluent_syntax::ast::Pattern {
                    elements: vec![fluent_syntax::ast::PatternElement::Placeable {
                        expression: fluent_syntax::ast::Expression::Inline(
                            fluent_syntax::ast::InlineExpression::MessageReference {
                                id: fluent_syntax::ast::Identifier {
                                    name: "ref_msg".to_string(),
                                },
                                attribute: None,
                            },
                        ),
                    }],
                }),
                attributes: vec![],
                comment: None,
            }), // entry
            Arc::new(PathBuf::from("tmp.ftl")), // path
            Some("en".to_string()),            // locale
            Some(0),
            FastHashSet::default(),
        );
        let id = fluent_syntax::ast::Identifier {
            name: "ref_msg".to_string(),
        };

        super::extract_kwargs_from_message_reference(
            &mut key,
            &id,
            &None,
            &mut FastHashSet::<String>::default(),
            &mut FastHashMap::<String, FluentKey>::default(),
            &FastHashMap::<String, FluentKey>::default(),
            &mut FastHashSet::default(),
        )
        .unwrap();
    }

    #[test]
    fn test_extract_kwargs_from_term_reference() {
        let mut key = FluentKey::new(
            Arc::new(PathBuf::from("tmp.py")), // code_path
            String::from("msg"),               // key
            FluentEntry::Message(fluent_syntax::ast::Message {
                id: fluent_syntax::ast::Identifier {
                    name: "msg".to_string(),
                },
                value: Some(fluent_syntax::ast::Pattern {
                    elements: vec![fluent_syntax::ast::PatternElement::Placeable {
                        expression: fluent_syntax::ast::Expression::Inline(
                            fluent_syntax::ast::InlineExpression::TermReference {
                                id: fluent_syntax::ast::Identifier {
                                    name: "term".to_string(),
                                },
                                attribute: None,
                                arguments: None,
                            },
                        ),
                    }],
                }),
                attributes: vec![],
                comment: None,
            }), // entry
            Arc::new(PathBuf::from("tmp.ftl")), // path
            Some("en".to_string()),            // locale
            Some(0),
            FastHashSet::default(),
        );
        let mut kwargs: FastHashSet<String> = FastHashSet::default();
        let mut terms: FastHashMap<String, FluentKey> = FastHashMap::default();
        terms.insert(
            "term".to_string(),
            FluentKey::new(
                Arc::new(PathBuf::from("tmp.py")), // code_path
                String::from("term"),              // key
                FluentEntry::Term(fluent_syntax::ast::Term {
                    id: fluent_syntax::ast::Identifier {
                        name: "term".to_string(),
                    },
                    value: fluent_syntax::ast::Pattern {
                        elements: vec![fluent_syntax::ast::PatternElement::Placeable {
                            expression: fluent_syntax::ast::Expression::Inline(
                                fluent_syntax::ast::InlineExpression::VariableReference {
                                    id: fluent_syntax::ast::Identifier {
                                        name: "username".to_string(),
                                    },
                                },
                            ),
                        }],
                    },
                    attributes: vec![],
                    comment: None,
                }), // entry
                Arc::new(PathBuf::from("tmp.ftl")), // path
                Some("en".to_string()),            // locale
                Some(0),
                FastHashSet::default(),
            ),
        );

        super::extract_kwargs_from_term_reference(
            &mut key,
            &fluent_syntax::ast::Identifier {
                name: "term".to_string(),
            },
            &None,
            &None,
            &mut kwargs,
            &mut terms,
            &FastHashMap::<String, FluentKey>::default(),
        )
        .unwrap();

        assert!(kwargs.contains("username"));
    }

    #[test]
    #[should_panic(expected = "Can't find reference key term in tmp.ftl")]
    fn test_extract_kwargs_from_term_reference_panic_when_no_term() {
        let mut key = FluentKey::new(
            Arc::new(PathBuf::from("tmp.py")), // code_path
            String::from("msg"),               // key
            FluentEntry::Message(fluent_syntax::ast::Message {
                id: fluent_syntax::ast::Identifier {
                    name: "msg".to_string(),
                },
                value: Some(fluent_syntax::ast::Pattern {
                    elements: vec![fluent_syntax::ast::PatternElement::Placeable {
                        expression: fluent_syntax::ast::Expression::Inline(
                            fluent_syntax::ast::InlineExpression::TermReference {
                                id: fluent_syntax::ast::Identifier {
                                    name: "term".to_string(),
                                },
                                attribute: None,
                                arguments: None,
                            },
                        ),
                    }],
                }),
                attributes: vec![],
                comment: None,
            }), // entry
            Arc::new(PathBuf::from("tmp.ftl")), // path
            Some("en".to_string()),            // locale
            Some(0),
            FastHashSet::default(),
        );

        super::extract_kwargs_from_term_reference(
            &mut key,
            &fluent_syntax::ast::Identifier {
                name: "term".to_string(),
            },
            &None,
            &None,
            &mut FastHashSet::<String>::default(),
            &mut FastHashMap::<String, FluentKey>::default(),
            &FastHashMap::<String, FluentKey>::default(),
        )
        .unwrap();
    }

    #[test]
    fn test_extract_kwargs_from_selector_expression() {
        let mut key = FluentKey::new(
            Arc::new(PathBuf::from("tmp.py")), // code_path
            String::from("msg"),               // key
            FluentEntry::Message(fluent_syntax::ast::Message {
                id: fluent_syntax::ast::Identifier {
                    name: "msg".to_string(),
                },
                value: Some(fluent_syntax::ast::Pattern {
                    elements: vec![fluent_syntax::ast::PatternElement::Placeable {
                        expression: fluent_syntax::ast::Expression::Select {
                            selector: fluent_syntax::ast::InlineExpression::VariableReference {
                                id: fluent_syntax::ast::Identifier {
                                    name: "user_role".to_string(),
                                },
                            },
                            variants: vec![
                                fluent_syntax::ast::Variant {
                                    key: fluent_syntax::ast::VariantKey::Identifier {
                                        name: "admin".to_string(),
                                    },
                                    value: fluent_syntax::ast::Pattern { elements: vec![] },
                                    default: false,
                                },
                                fluent_syntax::ast::Variant {
                                    key: fluent_syntax::ast::VariantKey::Identifier {
                                        name: "user".to_string(),
                                    },
                                    value: fluent_syntax::ast::Pattern { elements: vec![] },
                                    default: true,
                                },
                            ],
                        },
                    }],
                }),
                attributes: vec![],
                comment: None,
            }), // entry
            Arc::new(PathBuf::from("tmp.ftl")), // path
            Some("en".to_string()),            // locale
            Some(0),
            FastHashSet::default(),
        );
        let selector = fluent_syntax::ast::InlineExpression::VariableReference {
            id: fluent_syntax::ast::Identifier {
                name: "user_role".to_string(),
            },
        };
        let variants: Vec<fluent_syntax::ast::Variant<String>> = vec![
            fluent_syntax::ast::Variant {
                key: fluent_syntax::ast::VariantKey::Identifier {
                    name: "admin".to_string(),
                },
                value: fluent_syntax::ast::Pattern {
                    elements: vec![fluent_syntax::ast::PatternElement::Placeable {
                        expression: fluent_syntax::ast::Expression::Inline(
                            fluent_syntax::ast::InlineExpression::VariableReference {
                                id: fluent_syntax::ast::Identifier {
                                    name: "user_role".to_string(),
                                },
                            },
                        ),
                    }],
                },
                default: false,
            },
            fluent_syntax::ast::Variant {
                key: fluent_syntax::ast::VariantKey::Identifier {
                    name: "user".to_string(),
                },
                value: fluent_syntax::ast::Pattern { elements: vec![] },
                default: true,
            },
        ];
        let mut kwargs: FastHashSet<String> = FastHashSet::default();

        super::extract_kwargs_from_selector_expression(
            &mut key,
            &selector,
            &variants,
            &mut kwargs,
            &mut FastHashMap::<String, FluentKey>::default(),
            &FastHashMap::<String, FluentKey>::default(),
            &mut FastHashSet::<String>::default(),
        );

        assert!(kwargs.contains("user_role"));
    }

    #[test]
    fn test_extract_kwargs_from_message() {
        let mut key = FluentKey::new(
            Arc::new(PathBuf::from("tmp.py")), // code_path
            String::from("msg"),               // key
            FluentEntry::Message(fluent_syntax::ast::Message {
                id: fluent_syntax::ast::Identifier {
                    name: "msg".to_string(),
                },
                value: Some(fluent_syntax::ast::Pattern {
                    elements: vec![fluent_syntax::ast::PatternElement::Placeable {
                        expression: fluent_syntax::ast::Expression::Inline(
                            fluent_syntax::ast::InlineExpression::VariableReference {
                                id: fluent_syntax::ast::Identifier {
                                    name: "username".to_string(),
                                },
                            },
                        ),
                    }],
                }),
                attributes: vec![],
                comment: None,
            }), // entry
            Arc::new(PathBuf::from("tmp.ftl")), // path
            Some("en".to_string()),            // locale
            Some(0),
            FastHashSet::default(),
        );
        let mut kwargs: FastHashSet<String> = FastHashSet::default();

        super::extract_kwargs_from_message(
            &mut key,
            &mut kwargs,
            &mut FastHashMap::<String, FluentKey>::default(),
            &FastHashMap::<String, FluentKey>::default(),
            &mut FastHashSet::<String>::default(),
        );

        assert!(kwargs.contains("username"));
    }

    #[test]
    #[should_panic(expected = "Expected a Message entry")]
    fn test_extract_kwargs_from_message_panics_on_term() {
        let mut key = FluentKey::new(
            Arc::new(PathBuf::from("tmp.py")), // code_path
            String::from("term"),              // key
            FluentEntry::Term(fluent_syntax::ast::Term {
                id: fluent_syntax::ast::Identifier {
                    name: "term".to_string(),
                },
                value: fluent_syntax::ast::Pattern { elements: vec![] },
                attributes: vec![],
                comment: None,
            }), // entry
            Arc::new(PathBuf::from("tmp.ftl")), // path
            Some("en".to_string()),            // locale
            Some(0),
            FastHashSet::default(),
        );

        super::extract_kwargs_from_message(
            &mut key,
            &mut FastHashSet::<String>::default(),
            &mut FastHashMap::<String, FluentKey>::default(),
            &FastHashMap::<String, FluentKey>::default(),
            &mut FastHashSet::<String>::default(),
        );
    }

    #[test]
    fn test_extract_kwargs_from_term() {
        let mut key = FluentKey::new(
            Arc::new(PathBuf::from("tmp.py")), // code_path
            String::from("term"),              // key
            FluentEntry::Term(fluent_syntax::ast::Term {
                id: fluent_syntax::ast::Identifier {
                    name: "term".to_string(),
                },
                value: fluent_syntax::ast::Pattern {
                    elements: vec![fluent_syntax::ast::PatternElement::Placeable {
                        expression: fluent_syntax::ast::Expression::Inline(
                            fluent_syntax::ast::InlineExpression::VariableReference {
                                id: fluent_syntax::ast::Identifier {
                                    name: "username".to_string(),
                                },
                            },
                        ),
                    }],
                },
                attributes: vec![],
                comment: None,
            }), // entry
            Arc::new(PathBuf::from("tmp.ftl")), // path
            Some("en".to_string()),            // locale
            Some(0),
            FastHashSet::default(),
        );
        let mut kwargs: FastHashSet<String> = FastHashSet::default();

        super::extract_kwargs_from_term(
            &mut key,
            &mut kwargs,
            &mut FastHashMap::<String, FluentKey>::default(),
            &FastHashMap::<String, FluentKey>::default(),
            &mut FastHashSet::<String>::default(),
        );

        assert!(kwargs.contains("username"));
    }

    #[test]
    #[should_panic(expected = "Expected a Term entry")]
    fn test_extract_kwargs_from_term_panics_on_message() {
        let mut key = FluentKey::new(
            Arc::new(PathBuf::from("tmp.py")), // code_path
            String::from("msg"),               // key
            FluentEntry::Message(fluent_syntax::ast::Message {
                id: fluent_syntax::ast::Identifier {
                    name: "msg".to_string(),
                },
                value: Some(fluent_syntax::ast::Pattern { elements: vec![] }),
                attributes: vec![],
                comment: None,
            }), // entry
            Arc::new(PathBuf::from("tmp.ftl")), // path
            Some("en".to_string()),            // locale
            Some(0),
            FastHashSet::default(),
        );

        super::extract_kwargs_from_term(
            &mut key,
            &mut FastHashSet::<String>::default(),
            &mut FastHashMap::<String, FluentKey>::default(),
            &FastHashMap::<String, FluentKey>::default(),
            &mut FastHashSet::<String>::default(),
        );
    }
}

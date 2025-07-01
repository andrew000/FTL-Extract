#![allow(unused_variables)]

use crate::ftl::matcher::{FluentEntry, FluentKey};
use anyhow::{Result, bail};
use fluent_syntax::ast::{CallArguments, Identifier};
use hashbrown::{HashMap, HashSet};

pub(crate) fn extract_kwargs(
    key: &mut FluentKey,
    terms: &mut HashMap<String, FluentKey>,
    all_fluent_keys: &HashMap<String, FluentKey>,
    depend_keys: &mut HashSet<String>,
) -> HashSet<String> {
    let mut kwargs: HashSet<String> = HashSet::new();

    if let FluentEntry::Message(message) = &key.entry {
        if message.value.is_none() || message.value.as_ref().unwrap().elements.is_empty() {
            return kwargs;
        }

        extract_kwargs_from_message(key, &mut kwargs, terms, all_fluent_keys, depend_keys);
    } else if let FluentEntry::Term(term) = &key.entry {
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
    kwargs: &mut HashSet<String>,
    terms: &mut HashMap<String, FluentKey>,
    all_fluent_keys: &HashMap<String, FluentKey>,
    depend_keys: &mut HashSet<String>,
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
    kwargs: &mut HashSet<String>,
) {
    kwargs.insert(variable_reference.name.clone());
}

fn extract_kwargs_from_message_reference(
    key: &mut FluentKey,
    id: &Identifier<String>,
    attribute: &Option<Identifier<String>>,
    kwargs: &mut HashSet<String>,
    terms: &mut HashMap<String, FluentKey>,
    all_fluent_keys: &HashMap<String, FluentKey>,
    depend_keys: &mut HashSet<String>,
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
    kwargs: &mut HashSet<String>,
    terms: &mut HashMap<String, FluentKey>,
    all_fluent_keys: &HashMap<String, FluentKey>,
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
        &mut HashSet::new(),
    ));

    Ok(())
}

fn extract_kwargs_from_selector_expression(
    key: &mut FluentKey,
    selector: &fluent_syntax::ast::InlineExpression<String>,
    variants: &Vec<fluent_syntax::ast::Variant<String>>,
    kwargs: &mut HashSet<String>,
    terms: &mut HashMap<String, FluentKey>,
    all_fluent_keys: &HashMap<String, FluentKey>,
    depend_keys: &mut HashSet<String>,
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
    kwargs: &mut HashSet<String>,
    terms: &mut HashMap<String, FluentKey>,
    all_fluent_keys: &HashMap<String, FluentKey>,
    depend_keys: &mut HashSet<String>,
) {
    let elements = if let FluentEntry::Message(message) = &key.entry {
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
    kwargs: &mut HashSet<String>,
    terms: &mut HashMap<String, FluentKey>,
    all_fluent_keys: &HashMap<String, FluentKey>,
    depend_keys: &mut HashSet<String>,
) {
    let elements = if let FluentEntry::Term(term) = &key.entry {
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

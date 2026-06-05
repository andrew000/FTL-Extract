use super::definitions::{Definitions, message_reference_exists, term_reference_exists};
use super::locale::LocaleResource;
use crate::types::MissingReference;
use fluent_syntax::ast::{
    Entry, Expression, Identifier, InlineExpression, Message, Pattern, PatternElement, Term,
};
use std::path::Path;

#[derive(Debug)]
enum Owner<'a> {
    Message(&'a Message<String>),
    Term(&'a Term<String>),
}

impl Owner<'_> {
    fn key(&self) -> String {
        match self {
            Self::Message(message) => message.id.name.clone(),
            Self::Term(term) => format!("-{}", term.id.name),
        }
    }
}

pub(super) fn collect_missing_references(
    locale: &str,
    locale_resource: &LocaleResource<'_>,
    definitions: &Definitions,
    missing_references: &mut Vec<MissingReference>,
) {
    for located in locale_resource.entries {
        match &located.entry {
            Entry::Message(message) => {
                let owner = Owner::Message(message);
                if let Some(pattern) = &message.value {
                    collect_pattern_references(
                        locale,
                        locale_resource.path,
                        located.line,
                        &owner,
                        pattern,
                        definitions,
                        missing_references,
                    );
                }
                for attribute in &message.attributes {
                    collect_pattern_references(
                        locale,
                        locale_resource.path,
                        located.line,
                        &owner,
                        &attribute.value,
                        definitions,
                        missing_references,
                    );
                }
            }
            Entry::Term(term) => {
                let owner = Owner::Term(term);
                collect_pattern_references(
                    locale,
                    locale_resource.path,
                    located.line,
                    &owner,
                    &term.value,
                    definitions,
                    missing_references,
                );
                for attribute in &term.attributes {
                    collect_pattern_references(
                        locale,
                        locale_resource.path,
                        located.line,
                        &owner,
                        &attribute.value,
                        definitions,
                        missing_references,
                    );
                }
            }
            _ => {}
        }
    }
}

fn collect_pattern_references(
    locale: &str,
    path: &Path,
    line: Option<usize>,
    owner: &Owner<'_>,
    pattern: &Pattern<String>,
    definitions: &Definitions,
    missing_references: &mut Vec<MissingReference>,
) {
    for element in &pattern.elements {
        if let PatternElement::Placeable { expression } = element {
            collect_expression_references(
                locale,
                path,
                line,
                owner,
                expression,
                definitions,
                missing_references,
            );
        }
    }
}

fn collect_expression_references(
    locale: &str,
    path: &Path,
    line: Option<usize>,
    owner: &Owner<'_>,
    expression: &Expression<String>,
    definitions: &Definitions,
    missing_references: &mut Vec<MissingReference>,
) {
    match expression {
        Expression::Inline(inline) => collect_inline_references(
            locale,
            path,
            line,
            owner,
            inline,
            definitions,
            missing_references,
        ),
        Expression::Select { selector, variants } => {
            collect_inline_references(
                locale,
                path,
                line,
                owner,
                selector,
                definitions,
                missing_references,
            );
            for variant in variants {
                collect_pattern_references(
                    locale,
                    path,
                    line,
                    owner,
                    &variant.value,
                    definitions,
                    missing_references,
                );
            }
        }
    }
}

fn collect_inline_references(
    locale: &str,
    path: &Path,
    line: Option<usize>,
    owner: &Owner<'_>,
    inline: &InlineExpression<String>,
    definitions: &Definitions,
    missing_references: &mut Vec<MissingReference>,
) {
    match inline {
        InlineExpression::MessageReference { id, attribute } => {
            if !message_reference_exists(id, attribute, definitions) {
                missing_references.push(MissingReference {
                    locale: locale.to_string(),
                    file_path: path.to_path_buf(),
                    line,
                    key: Some(owner.key()),
                    reference: display_message_reference(id, attribute),
                });
            }
        }
        InlineExpression::TermReference {
            id,
            attribute,
            arguments,
        } => {
            if !term_reference_exists(id, attribute, definitions) {
                missing_references.push(MissingReference {
                    locale: locale.to_string(),
                    file_path: path.to_path_buf(),
                    line,
                    key: Some(owner.key()),
                    reference: display_term_reference(id, attribute),
                });
            }
            if let Some(arguments) = arguments {
                for positional in &arguments.positional {
                    collect_inline_references(
                        locale,
                        path,
                        line,
                        owner,
                        positional,
                        definitions,
                        missing_references,
                    );
                }
                for named in &arguments.named {
                    collect_inline_references(
                        locale,
                        path,
                        line,
                        owner,
                        &named.value,
                        definitions,
                        missing_references,
                    );
                }
            }
        }
        InlineExpression::Placeable { expression } => collect_expression_references(
            locale,
            path,
            line,
            owner,
            expression,
            definitions,
            missing_references,
        ),
        InlineExpression::FunctionReference { arguments, .. } => {
            for positional in &arguments.positional {
                collect_inline_references(
                    locale,
                    path,
                    line,
                    owner,
                    positional,
                    definitions,
                    missing_references,
                );
            }
            for named in &arguments.named {
                collect_inline_references(
                    locale,
                    path,
                    line,
                    owner,
                    &named.value,
                    definitions,
                    missing_references,
                );
            }
        }
        InlineExpression::StringLiteral { .. }
        | InlineExpression::NumberLiteral { .. }
        | InlineExpression::VariableReference { .. } => {}
    }
}

fn display_message_reference(
    id: &Identifier<String>,
    attribute: &Option<Identifier<String>>,
) -> String {
    if let Some(attribute) = attribute {
        format!("{}.{}", id.name, attribute.name)
    } else {
        id.name.clone()
    }
}

fn display_term_reference(
    id: &Identifier<String>,
    attribute: &Option<Identifier<String>>,
) -> String {
    if let Some(attribute) = attribute {
        format!("-{}.{}", id.name, attribute.name)
    } else {
        format!("-{}", id.name)
    }
}

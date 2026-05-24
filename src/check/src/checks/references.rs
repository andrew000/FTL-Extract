use crate::checks::validate_locales;
use crate::parser::{
    LocatedEntry, discover_locales, ftl_files_for_locale, parse_ftl_entries_lossy,
};
use crate::types::{CheckReferencesConfig, CheckReferencesResult, MissingReference};
use anyhow::Result;
use extractor::ftl::utils::{FastHashMap, FastHashSet};
use fluent_syntax::ast::{
    Entry, Expression, Identifier, InlineExpression, Message, Pattern, PatternElement, Term,
};
use std::path::{Path, PathBuf};

pub fn check_references(config: CheckReferencesConfig) -> Result<CheckReferencesResult> {
    let available_locales = discover_locales(&config.locales_path)?;
    validate_locales(&config.locales_path, &available_locales, &config.locales)?;

    let mut missing_references = Vec::new();
    for locale in &config.locales {
        let locale_resources = read_locale_resources(&config.locales_path, locale)?;
        let definitions = collect_definitions(&locale_resources);

        for resource in &locale_resources {
            collect_missing_references(locale, resource, &definitions, &mut missing_references);
        }
    }

    missing_references.sort_by(|a, b| {
        a.locale
            .cmp(&b.locale)
            .then_with(|| a.file_path.cmp(&b.file_path))
            .then_with(|| a.line.cmp(&b.line))
            .then_with(|| a.reference.cmp(&b.reference))
    });

    Ok(CheckReferencesResult {
        checked_locales: config.locales,
        missing_references,
    })
}

#[derive(Debug)]
struct LocaleResource {
    path: PathBuf,
    entries: Vec<LocatedEntry>,
}

#[derive(Debug, Default)]
struct Definitions {
    messages: FastHashMap<String, FastHashSet<String>>,
    terms: FastHashMap<String, FastHashSet<String>>,
}

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

fn read_locale_resources(locales_path: &Path, locale: &str) -> Result<Vec<LocaleResource>> {
    ftl_files_for_locale(locales_path, locale)?
        .into_iter()
        .map(|path| {
            let entries = parse_ftl_entries_lossy(&path)?;
            let path = path
                .strip_prefix(locales_path)
                .unwrap_or(&path)
                .to_path_buf();
            Ok(LocaleResource { path, entries })
        })
        .collect()
}

fn collect_definitions(resources: &[LocaleResource]) -> Definitions {
    let mut definitions = Definitions::default();

    for locale_resource in resources {
        for located in &locale_resource.entries {
            match &located.entry {
                Entry::Message(message) => {
                    definitions.messages.insert(
                        message.id.name.clone(),
                        message
                            .attributes
                            .iter()
                            .map(|attribute| attribute.id.name.clone())
                            .collect(),
                    );
                }
                Entry::Term(term) => {
                    definitions.terms.insert(
                        term.id.name.clone(),
                        term.attributes
                            .iter()
                            .map(|attribute| attribute.id.name.clone())
                            .collect(),
                    );
                }
                _ => {}
            }
        }
    }

    definitions
}

fn collect_missing_references(
    locale: &str,
    locale_resource: &LocaleResource,
    definitions: &Definitions,
    missing_references: &mut Vec<MissingReference>,
) {
    for located in &locale_resource.entries {
        match &located.entry {
            Entry::Message(message) => {
                let owner = Owner::Message(message);
                if let Some(pattern) = &message.value {
                    collect_pattern_references(
                        locale,
                        &locale_resource.path,
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
                        &locale_resource.path,
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
                    &locale_resource.path,
                    located.line,
                    &owner,
                    &term.value,
                    definitions,
                    missing_references,
                );
                for attribute in &term.attributes {
                    collect_pattern_references(
                        locale,
                        &locale_resource.path,
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
        InlineExpression::TermReference { id, attribute, .. } => {
            if !term_reference_exists(id, attribute, definitions) {
                missing_references.push(MissingReference {
                    locale: locale.to_string(),
                    file_path: path.to_path_buf(),
                    line,
                    key: Some(owner.key()),
                    reference: display_term_reference(id, attribute),
                });
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

fn message_reference_exists(
    id: &Identifier<String>,
    attribute: &Option<Identifier<String>>,
    definitions: &Definitions,
) -> bool {
    reference_exists(&definitions.messages, id, attribute)
}

fn term_reference_exists(
    id: &Identifier<String>,
    attribute: &Option<Identifier<String>>,
    definitions: &Definitions,
) -> bool {
    reference_exists(&definitions.terms, id, attribute)
}

fn reference_exists(
    definitions: &FastHashMap<String, FastHashSet<String>>,
    id: &Identifier<String>,
    attribute: &Option<Identifier<String>>,
) -> bool {
    let Some(attributes) = definitions.get(&id.name) else {
        return false;
    };

    match attribute {
        Some(attribute) => attributes.contains(&attribute.name),
        None => true,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_check_references_reports_missing_message_and_term() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let locales = temp_dir.path().join("locales");
        fs::create_dir_all(locales.join("en"))?;
        fs::write(
            locales.join("en").join("_default.ftl"),
            "welcome = { missing-message }\nbrand = { -missing-term }\n",
        )?;

        let result = check_references(CheckReferencesConfig {
            locales_path: locales,
            locales: vec!["en".to_string()],
        })?;

        assert_eq!(result.missing_references.len(), 2);
        assert!(
            result
                .missing_references
                .iter()
                .any(|item| item.reference == "missing-message")
        );
        assert!(
            result
                .missing_references
                .iter()
                .any(|item| item.reference == "-missing-term")
        );
        Ok(())
    }

    #[test]
    fn test_check_references_accepts_existing_references() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let locales = temp_dir.path().join("locales");
        fs::create_dir_all(locales.join("en"))?;
        fs::write(
            locales.join("en").join("_default.ftl"),
            "welcome = { title } { -brand }\ntitle = Welcome\n-brand = Brand\n",
        )?;

        let result = check_references(CheckReferencesConfig {
            locales_path: locales,
            locales: vec!["en".to_string()],
        })?;

        assert!(result.missing_references.is_empty());
        Ok(())
    }

    #[test]
    fn test_check_references_reports_missing_message_attribute() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let locales = temp_dir.path().join("locales");
        fs::create_dir_all(locales.join("en"))?;
        fs::write(
            locales.join("en").join("_default.ftl"),
            "title = Title\nwelcome = { title.missing }\n",
        )?;

        let result = check_references(CheckReferencesConfig {
            locales_path: locales,
            locales: vec!["en".to_string()],
        })?;

        assert_eq!(result.missing_references.len(), 1);
        assert!(
            result
                .missing_references
                .iter()
                .any(|item| item.reference == "title.missing")
        );
        Ok(())
    }

    #[test]
    fn test_check_references_accepts_existing_message_attribute() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let locales = temp_dir.path().join("locales");
        fs::create_dir_all(locales.join("en"))?;
        fs::write(
            locales.join("en").join("_default.ftl"),
            "title = Title\n    .short = T\nwelcome = { title.short }\n",
        )?;

        let result = check_references(CheckReferencesConfig {
            locales_path: locales,
            locales: vec!["en".to_string()],
        })?;

        assert!(result.missing_references.is_empty());
        Ok(())
    }

    #[test]
    fn test_check_references_reports_real_source_line() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let locales = temp_dir.path().join("locales");
        fs::create_dir_all(locales.join("en"))?;
        fs::write(
            locales.join("en").join("_default.ftl"),
            "# comment\n\nwelcome = { missing }\n",
        )?;

        let result = check_references(CheckReferencesConfig {
            locales_path: locales,
            locales: vec!["en".to_string()],
        })?;

        assert_eq!(result.missing_references[0].line, Some(3));
        Ok(())
    }
}

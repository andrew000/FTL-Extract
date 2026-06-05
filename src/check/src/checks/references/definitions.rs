use super::locale::LocaleResource;
use extractor::ftl::utils::{FastHashMap, FastHashSet};
use fluent_syntax::ast::{Entry, Identifier};

#[derive(Debug, Default)]
pub(super) struct Definitions {
    messages: FastHashMap<String, FastHashSet<String>>,
    terms: FastHashMap<String, FastHashSet<String>>,
}

pub(super) fn collect_definitions(resources: &[LocaleResource<'_>]) -> Definitions {
    let mut definitions = Definitions::default();

    for locale_resource in resources {
        for located in locale_resource.entries {
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

pub(super) fn message_reference_exists(
    id: &Identifier<String>,
    attribute: &Option<Identifier<String>>,
    definitions: &Definitions,
) -> bool {
    reference_exists(&definitions.messages, id, attribute)
}

pub(super) fn term_reference_exists(
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

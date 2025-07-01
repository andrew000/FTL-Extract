use crate::ftl::matcher::{FluentEntry, FluentKey};
use fluent_syntax::ast::Entry;
use fluent_syntax::serializer::{Options, Serializer};

pub(crate) fn generate_ftl(fluent_keys: &Vec<FluentKey>, leave_as_is: &[FluentKey]) -> String {
    let mut resource: fluent_syntax::ast::Resource<String> =
        fluent_syntax::ast::Resource { body: vec![] };

    let mut listed_fluent_keys = fluent_keys.to_owned();
    listed_fluent_keys.extend_from_slice(leave_as_is);

    // Sort fluent keys by position
    listed_fluent_keys.sort_by(|a, b| a.position.cmp(&b.position));

    for fluent_key in listed_fluent_keys {
        match fluent_key.entry {
            FluentEntry::Message(message) => {
                resource.body.push(Entry::Message(message.clone()));
            }
            FluentEntry::Term(term) => {
                resource.body.push(Entry::Term(term.clone()));
            }
            FluentEntry::Comment(comment) => {
                resource.body.push(Entry::Comment(comment.clone()));
            }
            FluentEntry::Junk(junk) => {
                resource.body.push(Entry::Junk {
                    content: junk.clone(),
                });
            }
        }
    }

    let mut ser = Serializer::new(Options::default());
    ser.serialize_resource(&resource);
    ser.into_serialized_text()
}

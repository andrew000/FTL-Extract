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
        match fluent_key.entry.as_ref() {
            FluentEntry::Message(message) => {
                resource.body.push(Entry::Message(message.clone()));
            }
            FluentEntry::Term(term) => {
                resource.body.push(Entry::Term(term.clone()));
            }
            FluentEntry::Comment(comment) => {
                resource.body.push(Entry::Comment(comment.clone()));
            }
            FluentEntry::GroupComment(comment) => {
                resource.body.push(Entry::GroupComment(comment.clone()));
            }
            FluentEntry::ResourceComment(comment) => {
                resource.body.push(Entry::ResourceComment(comment.clone()));
            }
            FluentEntry::Junk(junk) => {
                resource.body.push(Entry::Junk {
                    content: junk.clone(),
                });
            }
        }
    }

    let mut ser = Serializer::new(Options { with_junk: false });
    ser.serialize_resource(&resource);
    ser.into_serialized_text()
}

#[cfg(test)]
mod tests {
    use crate::ftl::matcher::{FluentEntry, FluentKey};
    use crate::ftl::utils::FastHashSet;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;
    use std::sync::Arc;

    #[test]
    fn test_generate_ftl() {
        let fluent_keys: Vec<FluentKey> = vec![
            FluentKey::new(
                Arc::new(PathBuf::from("tmp.py")), // code_path
                String::from("message"),           // key
                FluentEntry::Message(fluent_syntax::ast::Message {
                    id: fluent_syntax::ast::Identifier {
                        name: "message".to_string(),
                    },
                    value: Some(fluent_syntax::ast::Pattern {
                        elements: vec![fluent_syntax::ast::PatternElement::TextElement {
                            value: "Test message.".to_string(),
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
            FluentKey::new(
                Arc::new(PathBuf::from("tmp.py")),
                String::from("term"), // key
                FluentEntry::Term(fluent_syntax::ast::Term {
                    id: fluent_syntax::ast::Identifier {
                        name: "term".to_string(),
                    },
                    value: fluent_syntax::ast::Pattern {
                        elements: vec![fluent_syntax::ast::PatternElement::TextElement {
                            value: "Test term.".to_string(),
                        }],
                    },
                    attributes: vec![],
                    comment: None,
                }), // entry
                Arc::new(PathBuf::from("tmp.ftl")), // path
                Some("en".to_string()), // locale
                Some(1),
                FastHashSet::default(),
            ),
            FluentKey::new(
                Arc::new(PathBuf::from("tmp.py")),
                String::from("comment"), // key
                FluentEntry::Comment(fluent_syntax::ast::Comment {
                    content: vec!["This is a comment.".to_string()],
                }), // entry
                Arc::new(PathBuf::from("tmp.ftl")), // path
                Some("en".to_string()),  // locale
                Some(2),
                FastHashSet::default(),
            ),
            FluentKey::new(
                Arc::new(PathBuf::from("tmp.py")),
                String::from("junk"),                           // key
                FluentEntry::Junk("This is junk.".to_string()), // entry
                Arc::new(PathBuf::from("tmp.ftl")),             // path
                Some("en".to_string()),                         // locale
                Some(3),
                FastHashSet::default(),
            ),
        ];
        let leave_as_is: Vec<FluentKey> = vec![];

        let ftl_output = super::generate_ftl(&fluent_keys, &leave_as_is);
        let expected_output =
            "message = Test message.\n-term = Test term.\n\n# This is a comment.\n\n";
        assert_eq!(ftl_output, expected_output);
    }
}

use crate::ftl::matcher::{FluentEntry, FluentKey};
use fluent_syntax::ast::{Entry, Resource};
use fluent_syntax::serializer::{Options, Serializer};
use std::sync::Arc;

/// Serializes `fluent_keys` in `position` order.
///
/// Entries are moved out of their `Arc` when nothing else holds them, which is the case for
/// keys imported from `.ftl` files and for commented keys. Only entries shared between
/// locales (keys extracted from code) are cloned.
pub(crate) fn generate_ftl(mut fluent_keys: Vec<FluentKey>) -> String {
    // Stable sort: keys added from code all share `usize::MAX` and keep their order.
    fluent_keys.sort_by_key(|key| key.position);

    let body = fluent_keys
        .into_iter()
        .map(|key| match Arc::unwrap_or_clone(key.entry) {
            FluentEntry::Message(message) => Entry::Message(message),
            FluentEntry::Term(term) => Entry::Term(term),
            FluentEntry::Comment(comment) => Entry::Comment(comment),
            FluentEntry::GroupComment(comment) => Entry::GroupComment(comment),
            FluentEntry::ResourceComment(comment) => Entry::ResourceComment(comment),
            FluentEntry::Junk(content) => Entry::Junk { content },
        })
        .collect();
    let resource: Resource<String> = Resource { body };

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
        // Entries shared with another key (as code keys are between locales) are cloned,
        // the rest are moved; the output must not depend on which path was taken.
        let shared = fluent_keys[0].clone();

        let ftl_output = super::generate_ftl(fluent_keys);
        assert!(matches!(shared.entry.as_ref(), FluentEntry::Message(_)));
        let expected_output =
            "message = Test message.\n-term = Test term.\n\n# This is a comment.\n\n";
        assert_eq!(ftl_output, expected_output);
    }

    #[test]
    fn test_generate_ftl_group_and_resource_comments() {
        let fluent_keys: Vec<FluentKey> = vec![
            FluentKey::new(
                Arc::new(PathBuf::from("tmp.py")),
                String::new(),
                FluentEntry::GroupComment(fluent_syntax::ast::Comment {
                    content: vec!["Group".to_string()],
                }),
                Arc::new(PathBuf::from("tmp.ftl")),
                Some("en".to_string()),
                Some(0),
                FastHashSet::default(),
            ),
            FluentKey::new(
                Arc::new(PathBuf::from("tmp.py")),
                String::new(),
                FluentEntry::ResourceComment(fluent_syntax::ast::Comment {
                    content: vec!["Resource".to_string()],
                }),
                Arc::new(PathBuf::from("tmp.ftl")),
                Some("en".to_string()),
                Some(1),
                FastHashSet::default(),
            ),
        ];

        let ftl_output = super::generate_ftl(fluent_keys);

        assert!(ftl_output.contains("## Group"));
        assert!(ftl_output.contains("### Resource"));
    }
}

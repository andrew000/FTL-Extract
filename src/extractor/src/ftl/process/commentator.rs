use crate::ftl::matcher::{FluentEntry, FluentKey};
use fluent_syntax::ast::Comment;
use fluent_syntax::serializer::Serializer;
use std::sync::Arc;

fn split_content(raw_entry: String) -> Vec<String> {
    let mut content: Vec<String> = raw_entry.lines().map(str::to_string).collect();

    if let Some((pre_last, last)) = content.len().checked_sub(2).and_then(|_| {
        let last = content.pop()?;
        let pre_last = content.pop()?;
        Some((pre_last, last))
    }) {
        content.push(format!("{pre_last}{last}"));
    }

    content
}

pub(crate) fn comment_ftl_key(key: &mut FluentKey) {
    if let FluentEntry::Comment(_) = &key.entry.as_ref() {
        // If already a Comment, leave it unchanged.
        return;
    }

    let mut ser = Serializer::new(fluent_syntax::serializer::Options::default());

    match &key.entry.as_ref() {
        FluentEntry::Message(message) => {
            ser.serialize_message(message);
        }
        FluentEntry::Term(term) => {
            ser.serialize_term(term);
        }
        FluentEntry::Junk(junk) => {
            ser.serialize_junk(junk);
        }
        FluentEntry::Comment(_)
        | FluentEntry::GroupComment(_)
        | FluentEntry::ResourceComment(_) => {}
    }
    key.entry = Arc::new(FluentEntry::Comment(Comment {
        content: split_content(ser.into_serialized_text()),
    }));
}

#[cfg(test)]
mod tests {
    use crate::ftl::matcher::{FluentEntry, FluentKey};
    use crate::ftl::utils::FastHashSet;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;
    use std::sync::Arc;

    #[test]
    fn test_comment_ftl_key_message_entry() {
        let mut key = FluentKey::new(
            Arc::new(PathBuf::from("tmp.py")), // code_path
            String::from("message"),           // key
            FluentEntry::Message(fluent_syntax::ast::Message {
                id: fluent_syntax::ast::Identifier {
                    name: "message".to_string(),
                },
                value: Some(fluent_syntax::ast::Pattern {
                    elements: vec![fluent_syntax::ast::PatternElement::TextElement {
                        value: "This is a test message.".to_string(),
                    }],
                }),
                attributes: vec![],
                comment: Some(fluent_syntax::ast::Comment {
                    content: vec!["Original message comment.".to_string()],
                }),
            }), // entry
            Arc::new(PathBuf::from("tmp.ftl")), // path
            Some("en".to_string()),            // locale
            Some(0),
            FastHashSet::default(),
        );
        super::comment_ftl_key(&mut key);

        assert!(matches!(key.entry.as_ref(), FluentEntry::Comment(_)));
    }

    #[test]
    fn test_comment_ftl_key_term_entry() {
        let mut key = FluentKey::new(
            Arc::new(PathBuf::from("tmp.py")), // code_path
            String::from("term"),              // key
            FluentEntry::Term(fluent_syntax::ast::Term {
                id: fluent_syntax::ast::Identifier {
                    name: "term".to_string(),
                },
                value: fluent_syntax::ast::Pattern {
                    elements: vec![fluent_syntax::ast::PatternElement::TextElement {
                        value: "This is a test term.".to_string(),
                    }],
                },
                attributes: vec![],
                comment: Some(fluent_syntax::ast::Comment {
                    content: vec!["Original term comment.".to_string()],
                }),
            }), // entry
            Arc::new(PathBuf::from("tmp.ftl")), // path
            Some("en".to_string()),            // locale
            Some(0),
            FastHashSet::default(),
        );
        super::comment_ftl_key(&mut key);

        assert!(matches!(key.entry.as_ref(), FluentEntry::Comment(_)));
    }

    #[test]
    fn test_comment_ftl_key_junk_entry() {
        let mut key = FluentKey::new(
            Arc::new(PathBuf::from("tmp.py")),                      // code_path
            String::from("junk"),                                   // key
            FluentEntry::Junk("This is junk content.".to_string()), // entry
            Arc::new(PathBuf::from("tmp.ftl")),                     // path
            Some("en".to_string()),                                 // locale
            Some(0),
            FastHashSet::default(),
        );
        super::comment_ftl_key(&mut key);

        assert!(matches!(key.entry.as_ref(), FluentEntry::Comment(_)));
    }

    #[test]
    fn test_comment_ftl_key_comment_entry() {
        let original_key = FluentKey::new(
            Arc::new(PathBuf::from("tmp.py")), // code_path
            String::from("comment"),           // key
            FluentEntry::Comment(fluent_syntax::ast::Comment {
                content: vec!["Existing comment content.".to_string()],
            }), // entry
            Arc::new(PathBuf::from("tmp.ftl")), // path
            Some("en".to_string()),            // locale
            Some(0),
            FastHashSet::default(),
        );
        let mut copied_key = original_key.clone();
        super::comment_ftl_key(&mut copied_key);

        assert!(matches!(
            original_key.entry.as_ref(),
            FluentEntry::Comment(_)
        ));
        assert_eq!(original_key.entry, copied_key.entry);
    }
}

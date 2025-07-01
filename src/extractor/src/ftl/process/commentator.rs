use crate::ftl::matcher::{FluentEntry, FluentKey};
use fluent_syntax::ast::Comment;
use fluent_syntax::serializer::Serializer;

pub(crate) fn comment_ftl_key(key: &mut FluentKey) {
    let mut ser = Serializer::new(fluent_syntax::serializer::Options::default());

    match &key.entry {
        FluentEntry::Message(message) => {
            ser.serialize_message(message);
            let raw_entry: String = ser.into_serialized_text();
            let mut content: Vec<String> = raw_entry.lines().map(str::to_string).collect();

            if let Some((pre_last, last)) = content.len().checked_sub(2).and_then(|_| {
                let last = content.pop()?;
                let pre_last = content.pop()?;
                Some((pre_last, last))
            }) {
                content.push(format!("{pre_last}{last}"));
            }

            key.entry = FluentEntry::Comment(Comment { content });
        }
        FluentEntry::Term(term) => {
            ser.serialize_term(term);
            let raw_entry = ser.into_serialized_text();
            key.entry = FluentEntry::Comment(Comment {
                content: vec![raw_entry],
            });
        }
        FluentEntry::Junk(junk) => {
            ser.serialize_junk(junk);
            let raw_entry = ser.into_serialized_text();
            key.entry = FluentEntry::Comment(Comment {
                content: vec![raw_entry],
            });
        }
        FluentEntry::Comment(_) => {}
    }
}

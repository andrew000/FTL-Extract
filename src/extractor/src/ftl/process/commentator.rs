use crate::ftl::matcher::{FluentEntry, FluentKey};
use fluent_syntax::ast::Comment;
use fluent_syntax::serializer::Serializer;

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
    let mut ser = Serializer::new(fluent_syntax::serializer::Options::default());

    match &key.entry {
        FluentEntry::Message(message) => {
            ser.serialize_message(message);
        }
        FluentEntry::Term(term) => {
            ser.serialize_term(term);
        }
        FluentEntry::Junk(junk) => {
            ser.serialize_junk(junk);
        }
        FluentEntry::Comment(_) => {}
    }
    key.entry = FluentEntry::Comment(Comment {
        content: split_content(ser.into_serialized_text()),
    });
}

use crate::ftl::matcher::FluentKey;
use fluent_syntax::ast::Comment;
use fluent_syntax::serializer::Serializer;
pub(crate) fn comment_ftl_key(key: &mut FluentKey) {
    let mut ser = Serializer::new(fluent_syntax::serializer::Options::default());

    if let Some(message) = &key.message {
        ser.serialize_message(message);
        let raw_entry = ser.into_serialized_text();
        key.comment = Some(Comment {
            content: vec![raw_entry],
        });
        key.message = None;
    } else if let Some(term) = &key.term {
        ser.serialize_term(term);
        let raw_entry = ser.into_serialized_text();
        key.comment = Some(Comment {
            content: vec![raw_entry],
        });
        key.term = None;
    } else if let Some(junk) = &key.junk {
        ser.serialize_junk(junk);
        let raw_entry = ser.into_serialized_text();
        key.comment = Some(Comment {
            content: vec![raw_entry],
        });
        key.junk = None;
    }
}

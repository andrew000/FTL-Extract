use crate::ftl::matcher::{FluentEntry, FluentKey};
use fluent_syntax::ast::Comment;
use fluent_syntax::serializer::Serializer;
use std::sync::Arc;

/// Turns the serialized text of an entry into comment lines, one per line of text, so that
/// stripping the `# ` prefix from the written comment gives the entry back unchanged.
///
/// Messages and terms are serialized with exactly one trailing newline, which `str::lines`
/// does not turn into an empty item. Junk is written verbatim and can end with blank lines;
/// those are dropped, because an empty comment line would be written as a bare `#`.
fn split_content(raw_entry: &str) -> Vec<String> {
    let mut content: Vec<String> = raw_entry.lines().map(str::to_string).collect();
    while content.last().is_some_and(|line| line.trim().is_empty()) {
        content.pop();
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
        content: split_content(&ser.into_serialized_text()),
    }));
}

#[cfg(test)]
mod tests {
    use crate::ftl::matcher::{FluentEntry, FluentKey};
    use crate::ftl::process::serializer::generate_ftl;
    use crate::ftl::utils::FastHashSet;
    use fluent_syntax::ast::{Entry, Resource};
    use fluent_syntax::parser::parse;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn key(entry: FluentEntry) -> FluentKey {
        FluentKey::new(
            Arc::new(PathBuf::from("tmp.py")), // code_path
            String::from("key"),               // key
            entry,
            Arc::new(PathBuf::from("tmp.ftl")), // path
            Some("en".to_string()),             // locale
            Some(0),
            FastHashSet::default(),
        )
    }

    fn parse_resource(source: &str) -> Resource<String> {
        match parse(source.to_string()) {
            Ok(resource) => resource,
            Err((_, errors)) => panic!("fixture does not parse: {errors:?}"),
        }
    }

    /// The single entry of `source` as a `FluentKey`.
    fn key_from_source(source: &str) -> FluentKey {
        let mut resource = parse_resource(source);
        assert_eq!(resource.body.len(), 1, "one entry expected in {source:?}");
        let entry = match resource.body.remove(0) {
            Entry::Message(message) => FluentEntry::Message(message),
            Entry::Term(term) => FluentEntry::Term(term),
            Entry::Comment(comment) => FluentEntry::Comment(comment),
            Entry::Junk { content } => FluentEntry::Junk(content),
            other => panic!("unexpected entry {other:?}"),
        };
        key(entry)
    }

    fn commented_content(source: &str) -> Vec<String> {
        let mut key = key_from_source(source);
        super::comment_ftl_key(&mut key);
        match key.entry.as_ref() {
            FluentEntry::Comment(comment) => comment.content.clone(),
            other => panic!("expected a Comment, got {other:?}"),
        }
    }

    /// What a user does to restore a commented entry: drop the `# ` (or bare `#`) prefix.
    fn uncomment(written: &str) -> String {
        written
            .lines()
            .map(|line| {
                line.strip_prefix("# ")
                    .or(line.strip_prefix('#'))
                    .unwrap_or(line)
            })
            .map(|line| format!("{line}\n"))
            .collect()
    }

    /// Commenting an entry and stripping the prefix from the written file yields the entry.
    fn assert_round_trip(source: &str) {
        let original = parse_resource(source);
        let mut key = key_from_source(source);
        super::comment_ftl_key(&mut key);

        let written = generate_ftl(vec![key]);
        let restored = parse_resource(&uncomment(&written));

        assert!(
            !restored
                .body
                .iter()
                .any(|entry| matches!(entry, Entry::Junk { .. })),
            "junk after restoring {written:?}"
        );
        assert_eq!(restored, original, "restored from {written:?}");
    }

    #[test]
    fn test_single_line_message() {
        assert_eq!(
            commented_content("single = One line\n"),
            vec!["single = One line"]
        );
        assert_round_trip("single = One line\n");
    }

    #[test]
    fn test_multiline_message_keeps_every_line() {
        let source = "old-rules =\n    Rule one.\n    Rule two.\n    Rule three.\n";
        assert_eq!(
            commented_content(source),
            vec![
                "old-rules =",
                "    Rule one.",
                "    Rule two.",
                "    Rule three."
            ]
        );
        assert_round_trip(source);
    }

    #[test]
    fn test_select_expression() {
        let source =
            "items =\n    { $n ->\n        [one] One item\n       *[other] { $n } items\n    }\n";
        assert_eq!(
            commented_content(source),
            vec![
                "items =",
                "    { $n ->",
                "        [one] One item",
                "       *[other] { $n } items",
                "    }"
            ]
        );
        assert_round_trip(source);
    }

    #[test]
    fn test_message_with_attribute() {
        let source = "btn = Click\n    .title = Tooltip\n";
        assert_eq!(
            commented_content(source),
            vec!["btn = Click", "    .title = Tooltip"]
        );
        assert_round_trip(source);
    }

    #[test]
    fn test_message_with_comment_above() {
        let source = "# ftl-extract: ignore stale\nstatus-ok = OK\n";
        assert_eq!(
            commented_content(source),
            vec!["# ftl-extract: ignore stale", "status-ok = OK"]
        );
        assert_round_trip(source);
    }

    #[test]
    fn test_term() {
        assert_eq!(commented_content("-brand = Bot\n"), vec!["-brand = Bot"]);
        assert_round_trip("-brand = Bot\n");

        let source = "-brand =\n    { $case ->\n        [gen] Bota\n       *[nom] Bot\n    }\n";
        assert_eq!(
            commented_content(source),
            vec![
                "-brand =",
                "    { $case ->",
                "        [gen] Bota",
                "       *[nom] Bot",
                "    }"
            ]
        );
        assert_round_trip(source);
    }

    #[test]
    fn test_non_ascii_message() {
        let source = "old-rules =\n    Правило перше.\n    Правило друге.\n";
        assert_eq!(
            commented_content(source),
            vec!["old-rules =", "    Правило перше.", "    Правило друге."]
        );
        assert_round_trip(source);
    }

    #[test]
    fn test_junk_drops_trailing_blank_lines_only() {
        let mut junk = key(FluentEntry::Junk("bad = {\nstill bad\n\n\n".to_string()));
        super::comment_ftl_key(&mut junk);
        assert_eq!(
            junk.entry.as_ref(),
            &FluentEntry::Comment(fluent_syntax::ast::Comment {
                content: vec!["bad = {".to_string(), "still bad".to_string()],
            })
        );

        let mut junk = key(FluentEntry::Junk("bad = {".to_string()));
        super::comment_ftl_key(&mut junk);
        assert_eq!(
            junk.entry.as_ref(),
            &FluentEntry::Comment(fluent_syntax::ast::Comment {
                content: vec!["bad = {".to_string()],
            })
        );

        let mut junk = key(FluentEntry::Junk("a\n\nb\n".to_string()));
        super::comment_ftl_key(&mut junk);
        assert_eq!(
            junk.entry.as_ref(),
            &FluentEntry::Comment(fluent_syntax::ast::Comment {
                content: vec!["a".to_string(), String::new(), "b".to_string()],
            })
        );
    }

    #[test]
    fn test_written_comment_has_no_bare_hash_lines() {
        let mut junk = key(FluentEntry::Junk("bad = {\n\n".to_string()));
        super::comment_ftl_key(&mut junk);
        assert_eq!(generate_ftl(vec![junk]), "# bad = {\n\n");

        let key = {
            let mut key = key_from_source("old-rules =\n    Rule one.\n    Rule two.\n");
            super::comment_ftl_key(&mut key);
            key
        };
        assert_eq!(
            generate_ftl(vec![key]),
            "# old-rules =\n#     Rule one.\n#     Rule two.\n\n"
        );
    }

    #[test]
    fn test_comment_ftl_key_comment_entry() {
        let original_key = key(FluentEntry::Comment(fluent_syntax::ast::Comment {
            content: vec!["Existing comment content.".to_string()],
        }));
        let mut copied_key = original_key.clone();
        super::comment_ftl_key(&mut copied_key);

        assert!(matches!(
            original_key.entry.as_ref(),
            FluentEntry::Comment(_)
        ));
        assert_eq!(original_key.entry, copied_key.entry);
    }

    #[test]
    fn test_comment_ftl_key_group_and_resource_comments() {
        for entry in [
            FluentEntry::GroupComment(fluent_syntax::ast::Comment {
                content: vec!["Group".to_string()],
            }),
            FluentEntry::ResourceComment(fluent_syntax::ast::Comment {
                content: vec!["Resource".to_string()],
            }),
        ] {
            let mut key = key(entry);
            super::comment_ftl_key(&mut key);

            // Nothing is serialized for these, so the result is an empty comment (unchanged
            // behavior).
            assert_eq!(
                key.entry.as_ref(),
                &FluentEntry::Comment(fluent_syntax::ast::Comment { content: vec![] })
            );
        }
    }

    #[test]
    fn test_blank_line_inside_a_pattern_survives_as_a_bare_hash_line() {
        // Only trailing blank lines are dropped. The serializer indents the blank line inside the
        // pattern, `serialize_comment` writes a whitespace-only line as a bare `#`, and
        // uncommenting restores the blank line.
        let source = "msg =\n    Line one.\n\n    Line two.\n";
        assert_eq!(
            commented_content(source),
            vec!["msg =", "    Line one.", "    ", "    Line two."]
        );
        let mut key = key_from_source(source);
        super::comment_ftl_key(&mut key);
        assert_eq!(
            generate_ftl(vec![key]),
            "# msg =\n#     Line one.\n#\n#     Line two.\n\n"
        );
        assert_round_trip(source);
    }
}

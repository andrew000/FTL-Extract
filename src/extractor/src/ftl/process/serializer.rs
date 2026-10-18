use crate::ftl::matcher::{FluentEntry, FluentKey};
use fluent_syntax::ast::{
    Attribute, CallArguments, Comment, Expression, InlineExpression, Message, Pattern,
    PatternElement, Term, Variant, VariantKey,
};

/// Serializes `fluent_keys` in `position` order.
///
/// The output is byte for byte what `fluent_syntax::serializer` writes for the same entries
/// (without junk), but the entries are read in place: no `Resource` is built, so nothing is
/// moved out of or cloned from its `Arc`, and the caller decides when the keys are freed.
pub(crate) fn generate_ftl(fluent_keys: &[FluentKey]) -> String {
    // Only the position, the name and the entry matter from here on. Sorting those instead
    // of the whole keys keeps the sort from shuffling a few hundred bytes per key.
    //
    // Keys added from code all share the position `usize::MAX`, and they arrive in the
    // iteration order of a hash map built by a parallel reduction, which differs from run to
    // run. Breaking the tie on the name (unique within a locale) appends them in alphabetical
    // order, so the same input always gives the same file. The index only orders entries that
    // share both, which leaves the order of the input as it is.
    let mut entries: Vec<(usize, &str, usize, &FluentEntry)> = fluent_keys
        .iter()
        .enumerate()
        .map(|(index, key)| (key.position, key.key.as_str(), index, key.entry.as_ref()))
        .collect();
    entries.sort_unstable_by_key(|&(position, name, index, _)| (position, name, index));

    // A typical entry serializes to well under a hundred bytes; reserving that up front
    // saves most of the regrowth of the buffer.
    let mut writer = Writer::with_capacity(entries.len() * 96);
    for (_, _, _, entry) in entries {
        writer.entry(entry);
    }
    writer.buffer
}

/// A port of `fluent_syntax::serializer::Serializer` that writes with `push_str` instead of
/// `write!`. Each method mirrors the upstream method of the same name; keep them in step.
struct Writer {
    buffer: String,
    indent_level: usize,
    /// Whether an entry has been written. Upstream tracks "a non-junk entry", and there is no
    /// junk here.
    wrote_entry: bool,
}

impl Writer {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            buffer: String::with_capacity(capacity),
            indent_level: 0,
            wrote_entry: false,
        }
    }

    fn entry(&mut self, entry: &FluentEntry) {
        match entry {
            FluentEntry::Message(message) => self.message(message),
            FluentEntry::Term(term) => self.term(term),
            FluentEntry::Comment(comment) => self.free_comment(comment, "#"),
            FluentEntry::GroupComment(comment) => self.free_comment(comment, "##"),
            FluentEntry::ResourceComment(comment) => self.free_comment(comment, "###"),
        }
        self.wrote_entry = true;
    }

    fn free_comment(&mut self, comment: &Comment<String>, prefix: &str) {
        if self.wrote_entry {
            self.newline();
        }
        self.comment(comment, prefix);
        self.newline();
    }

    fn comment(&mut self, comment: &Comment<String>, prefix: &str) {
        for line in &comment.content {
            self.literal(prefix);

            if !line.trim_matches(is_fluent_ws).is_empty() {
                self.literal(" ");
                self.literal(line);
            }

            self.newline();
        }
    }

    fn message(&mut self, message: &Message<String>) {
        if let Some(comment) = message.comment.as_ref() {
            self.comment(comment, "#");
        }

        self.literal(&message.id.name);
        self.literal(" =");

        if let Some(value) = message.value.as_ref() {
            self.pattern(value);
        }

        self.attributes(&message.attributes);

        self.newline();
    }

    fn term(&mut self, term: &Term<String>) {
        if let Some(comment) = term.comment.as_ref() {
            self.comment(comment, "#");
        }

        self.literal("-");
        self.literal(&term.id.name);
        self.literal(" =");
        self.pattern(&term.value);

        self.attributes(&term.attributes);

        self.newline();
    }

    fn pattern(&mut self, pattern: &Pattern<String>) {
        let start_on_newline = starts_on_new_line(pattern);

        if start_on_newline {
            self.newline();
            self.indent_level += 1;
        } else {
            self.literal(" ");
        }

        for element in &pattern.elements {
            self.element(element);
        }

        if start_on_newline {
            self.dedent();
        }
    }

    fn attributes(&mut self, attributes: &[Attribute<String>]) {
        if attributes.is_empty() {
            return;
        }

        self.indent_level += 1;

        for attribute in attributes {
            self.newline();
            self.literal(".");
            self.literal(&attribute.id.name);
            self.literal(" =");
            self.pattern(&attribute.value);
        }

        self.dedent();
    }

    fn element(&mut self, element: &PatternElement<String>) {
        match element {
            PatternElement::TextElement { value } => self.literal(value),
            PatternElement::Placeable { expression } => match expression {
                // A placeable inside a placeable keeps the braces from looking like
                // "{ { Foo() } }".
                Expression::Inline(InlineExpression::Placeable { expression }) => {
                    self.literal("{{ ");
                    self.expression(expression);
                    self.literal(" }}");
                }
                // A select adds its own newline and indent, so the brace goes without a space.
                Expression::Select { .. } => {
                    self.literal("{ ");
                    self.expression(expression);
                    self.literal("}");
                }
                Expression::Inline(_) => {
                    self.literal("{ ");
                    self.expression(expression);
                    self.literal(" }");
                }
            },
        }
    }

    fn expression(&mut self, expression: &Expression<String>) {
        match expression {
            Expression::Inline(inline) => self.inline_expression(inline),
            Expression::Select { selector, variants } => self.select(selector, variants),
        }
    }

    fn inline_expression(&mut self, expression: &InlineExpression<String>) {
        match expression {
            InlineExpression::StringLiteral { value } => {
                self.literal("\"");
                self.literal(value);
                self.literal("\"");
            }
            InlineExpression::NumberLiteral { value } => self.literal(value),
            InlineExpression::VariableReference { id } => {
                self.literal("$");
                self.literal(&id.name);
            }
            InlineExpression::FunctionReference { id, arguments } => {
                self.literal(&id.name);
                self.call_arguments(arguments);
            }
            InlineExpression::MessageReference { id, attribute } => {
                self.literal(&id.name);

                if let Some(attribute) = attribute.as_ref() {
                    self.literal(".");
                    self.literal(&attribute.name);
                }
            }
            InlineExpression::TermReference {
                id,
                attribute,
                arguments,
            } => {
                self.literal("-");
                self.literal(&id.name);

                if let Some(attribute) = attribute.as_ref() {
                    self.literal(".");
                    self.literal(&attribute.name);
                }
                if let Some(arguments) = arguments.as_ref() {
                    self.call_arguments(arguments);
                }
            }
            InlineExpression::Placeable { expression } => {
                self.literal("{");
                self.expression(expression);
                self.literal("}");
            }
        }
    }

    fn select(&mut self, selector: &InlineExpression<String>, variants: &[Variant<String>]) {
        self.inline_expression(selector);
        self.literal(" ->");

        self.newline();
        self.indent_level += 1;

        for variant in variants {
            self.variant(variant);
            self.newline();
        }

        self.dedent();
    }

    fn variant(&mut self, variant: &Variant<String>) {
        if variant.default {
            self.char_into_indent('*');
        }

        self.literal("[");
        match &variant.key {
            VariantKey::NumberLiteral { value } | VariantKey::Identifier { name: value } => {
                self.literal(value);
            }
        }
        self.literal("]");
        self.pattern(&variant.value);
    }

    fn call_arguments(&mut self, arguments: &CallArguments<String>) {
        let mut argument_written = false;

        self.literal("(");

        for positional in &arguments.positional {
            if argument_written {
                self.literal(", ");
            }

            self.inline_expression(positional);
            argument_written = true;
        }

        for named in &arguments.named {
            if argument_written {
                self.literal(", ");
            }

            self.literal(&named.name.name);
            self.literal(": ");
            self.inline_expression(&named.value);
            argument_written = true;
        }

        self.literal(")");
    }

    fn dedent(&mut self) {
        self.indent_level = self
            .indent_level
            .checked_sub(1)
            .expect("Dedenting without a corresponding indent");
    }

    fn ends_with_newline(&self) -> bool {
        self.buffer.as_bytes().last() == Some(&b'\n')
    }

    fn write_indent(&mut self) {
        for _ in 0..self.indent_level {
            self.buffer.push_str("    ");
        }
    }

    fn newline(&mut self) {
        // A trailing `\r` would otherwise read as part of the line ending.
        if self.buffer.as_bytes().last() == Some(&b'\r') {
            self.buffer.push('\r');
        }
        self.buffer.push('\n');
    }

    fn literal(&mut self, item: &str) {
        if self.ends_with_newline() {
            self.write_indent();
        }
        self.buffer.push_str(item);
    }

    fn char_into_indent(&mut self, ch: char) {
        if self.ends_with_newline() {
            self.write_indent();
        }
        self.buffer.pop();
        self.buffer.push(ch);
    }
}

/// Whitespace as the Fluent syntax defines it.
fn is_fluent_ws(c: char) -> bool {
    c == ' ' || c == '\r' || c == '\n'
}

fn starts_on_new_line(pattern: &Pattern<String>) -> bool {
    !has_leading_text_dot(pattern) && is_multiline(pattern)
}

fn is_multiline(pattern: &Pattern<String>) -> bool {
    pattern.elements.iter().any(|element| match element {
        PatternElement::TextElement { value } => value.contains('\n'),
        PatternElement::Placeable { expression } => is_select_expression(expression),
    })
}

fn has_leading_text_dot(pattern: &Pattern<String>) -> bool {
    matches!(
        pattern.elements.first(),
        Some(PatternElement::TextElement { value }) if value.starts_with('.')
    )
}

fn is_select_expression(expression: &Expression<String>) -> bool {
    match expression {
        Expression::Select { .. } => true,
        Expression::Inline(InlineExpression::Placeable { expression }) => {
            is_select_expression(expression)
        }
        Expression::Inline(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::ftl::matcher::{FluentEntry, FluentKey};
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
                Some(0),
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
                Some(1),
            ),
            FluentKey::new(
                Arc::new(PathBuf::from("tmp.py")),
                String::from("comment"), // key
                FluentEntry::Comment(fluent_syntax::ast::Comment {
                    content: vec!["This is a comment.".to_string()],
                }), // entry
                Arc::new(PathBuf::from("tmp.ftl")), // path
                Some(2),
            ),
        ];
        // Entries shared with another key (as code keys are between locales) are cloned,
        // the rest are moved; the output must not depend on which path was taken.
        let shared = fluent_keys[0].clone();

        let ftl_output = super::generate_ftl(&fluent_keys);
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
                Some(0),
            ),
            FluentKey::new(
                Arc::new(PathBuf::from("tmp.py")),
                String::new(),
                FluentEntry::ResourceComment(fluent_syntax::ast::Comment {
                    content: vec!["Resource".to_string()],
                }),
                Arc::new(PathBuf::from("tmp.ftl")),
                Some(1),
            ),
        ];

        let ftl_output = super::generate_ftl(&fluent_keys);

        assert!(ftl_output.contains("## Group"));
        assert!(ftl_output.contains("### Resource"));
    }

    /// What `generate_ftl` writes for every entry of `source` except junk, next to what
    /// `fluent_syntax::serializer` writes for the same entries.
    fn both_serializations(source: &str) -> (String, String) {
        let resource = match fluent_syntax::parser::parse(source.to_string()) {
            Ok(resource) => resource,
            Err((resource, _)) => resource,
        };
        let body: Vec<_> = resource
            .body
            .into_iter()
            .filter(|entry| !matches!(entry, fluent_syntax::ast::Entry::Junk { .. }))
            .collect();

        let keys: Vec<FluentKey> = body
            .iter()
            .enumerate()
            .map(|(position, entry)| {
                let entry = match entry.clone() {
                    fluent_syntax::ast::Entry::Message(message) => FluentEntry::Message(message),
                    fluent_syntax::ast::Entry::Term(term) => FluentEntry::Term(term),
                    fluent_syntax::ast::Entry::Comment(comment) => FluentEntry::Comment(comment),
                    fluent_syntax::ast::Entry::GroupComment(comment) => {
                        FluentEntry::GroupComment(comment)
                    }
                    fluent_syntax::ast::Entry::ResourceComment(comment) => {
                        FluentEntry::ResourceComment(comment)
                    }
                    fluent_syntax::ast::Entry::Junk { .. } => unreachable!(),
                };
                FluentKey::new(
                    Arc::new(PathBuf::new()),
                    String::new(),
                    entry,
                    Arc::new(PathBuf::from("tmp.ftl")),
                    Some(position),
                )
            })
            .collect();

        let upstream = fluent_syntax::serializer::serialize_with_options(
            &fluent_syntax::ast::Resource { body },
            fluent_syntax::serializer::Options { with_junk: false },
        );
        (super::generate_ftl(&keys), upstream)
    }

    #[test]
    fn test_generate_ftl_matches_fluent_syntax_serializer() {
        let source = concat!(
            "### Resource comment\n",
            "###\n",
            "\n",
            "## Group comment\n",
            "##   \n",
            "\n",
            "# Standalone comment\n",
            "\n",
            "# Attached comment\n",
            "#\n",
            "hello = Hello, { $name }!\n",
            "multiline =\n",
            "    First line\n",
            "      indented line\n",
            "\n",
            "    after a blank line\n",
            "leading-dot = .starts with a dot\n",
            "    and continues\n",
            "only-attributes =\n",
            "    .title = Title\n",
            "    .label = { -brand } label\n",
            "-brand = Brand\n",
            "    .short = B\n",
            "-term-with-args = { $case ->\n",
            "   *[nominative] Firefox\n",
            "    [genitive] Firefoxa\n",
            "}\n",
            "uses-term = { -term-with-args(case: \"genitive\") } and { -brand }\n",
            "select = { $count ->\n",
            "    [0] none\n",
            "    [one] { $count } item\n",
            "   *[other]\n",
            "        { $count } items\n",
            "        on two lines\n",
            "}\n",
            "nested-select = { $a ->\n",
            "   *[x] { $b ->\n",
            "       *[y] deep\n",
            "    }\n",
            "}\n",
            "functions = { NUMBER($n, minimumFractionDigits: 2) } { DATETIME($d) } { FN() }\n",
            "positional = { FN($a, $b, 3) } { NUMBER({ $nested }) }\n",
            "# Attached to a term\n",
            "-commented-term = Term with a comment\n",
            "lone-cr = text ending in a lone carriage return\r\r\n",
            "# comment line ending in a lone carriage return\r\r\n",
            "lone-cr-comment = value\n",
            "literals = { \"string \\\" with \\\\ escapes \\u00A0\" } { -12.50 } { 3 }\n",
            "placeables = {{ $nested }} { { \"x\" } } {{ msg.attr }}\n",
            "references = { other } { other.attr }\n",
            "braces = {\"{\"}open{\"}\"}\n",
            "selector-term = { -brand.short ->\n",
            "   *[B] b\n",
            "}\n",
            "cr-value = ends with a carriage return\r\n",
            "unicode = Привіт, 世界 🌍\n",
            "# comment after entries\n",
            "last = value\n",
            "\n",
            "\n",
            "# trailing standalone comment\n",
        );
        assert!(
            fluent_syntax::parser::parse(source).is_ok(),
            "the sample must parse without junk"
        );
        let (ours, upstream) = both_serializations(source);
        assert_eq!(ours, upstream);
    }

    #[test]
    fn test_generate_ftl_matches_fluent_syntax_serializer_without_junk() {
        // Junk is never written; the entries around it serialize as if it were not there.
        let (ours, upstream) = both_serializations("a = A\nbroken =\n# c\nb = B\n");
        assert_eq!(ours, upstream);
    }

    #[test]
    fn test_generate_ftl_appends_added_keys_by_name_whatever_the_input_order() {
        let key = |name: &str, position: Option<usize>| {
            FluentKey::new(
                Arc::new(PathBuf::from("app.py")),
                name.to_string(),
                FluentEntry::Message(crate::ftl::matcher::code_message(name.to_string(), vec![])),
                Arc::new(PathBuf::from("tmp.ftl")),
                position,
            )
        };
        // `stored` sits at its place in the file; the added keys (no position) follow it.
        let expected = "stored = stored\nalpha = alpha\nmiddle = middle\nzulu = zulu\n";

        for names in [
            ["zulu", "alpha", "middle"],
            ["middle", "zulu", "alpha"],
            ["alpha", "middle", "zulu"],
        ] {
            let mut keys: Vec<FluentKey> = names.iter().map(|name| key(name, None)).collect();
            keys.push(key("stored", Some(0)));
            assert_eq!(
                super::generate_ftl(&keys),
                expected,
                "input order {names:?}"
            );
        }
    }
}

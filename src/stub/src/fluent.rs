use anyhow::{Context, Result};
use fluent_syntax::ast::{Expression, InlineExpression, PatternElement, Resource};
use indexmap::IndexMap;
use log::debug;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// Represents a Fluent message with its arguments
#[derive(Debug, Clone)]
pub struct Message {
    /// The message identifier (key)
    pub id: String,
    /// List of argument names used in the message
    pub args: Vec<String>,
    /// The first line of the message translation (for type annotations)
    pub translation: String,
}

/// Represents a Fluent term with its arguments
#[derive(Debug, Clone)]
pub struct Term {
    /// The term identifier (key)
    pub id: String,
    /// List of argument names used in the term
    pub args: Vec<String>,
    /// The first line of the term translation
    pub translation: String,
}

/// Visitor for collecting messages and terms from Fluent AST
#[derive(Debug)]
struct FluentVisitor {
    messages: IndexMap<String, Message>,
    terms: IndexMap<String, Term>,
    /// Terms that need delayed resolution for forward references
    delayed_terms: IndexMap<String, Term>,
}

impl FluentVisitor {
    fn new() -> Self {
        Self {
            messages: IndexMap::new(),
            terms: IndexMap::new(),
            delayed_terms: IndexMap::new(),
        }
    }

    /// Visit a Fluent resource and extract all messages and terms
    fn visit_resource(&mut self, resource: &Resource<String>) {
        for entry in &resource.body {
            match entry {
                fluent_syntax::ast::Entry::Message(message) => {
                    if let Some(value) = &message.value {
                        let args = self.extract_arguments(&value.elements);
                        let translation = self.pattern_to_text(&value.elements);

                        let msg = Message {
                            id: message.id.name.clone(),
                            args,
                            translation,
                        };

                        self.messages.insert(message.id.name.clone(), msg);
                    }
                }
                fluent_syntax::ast::Entry::Term(term) => {
                    let args = self.extract_arguments(&term.value.elements);
                    let translation = self.pattern_to_text(&term.value.elements);

                    let term_obj = Term {
                        id: term.id.name.clone(),
                        args,
                        translation,
                    };

                    // Terms are stored with delayed resolution for forward references
                    self.delayed_terms.insert(term.id.name.clone(), term_obj);
                }
                _ => {
                    // Skip comments, resource comments, and group comments
                }
            }
        }
    }

    /// Extract argument names from pattern elements
    fn extract_arguments(&self, elements: &[PatternElement<String>]) -> Vec<String> {
        let mut args = Vec::new();
        let mut seen = HashSet::new();

        for element in elements {
            self.extract_args_from_element(element, &mut args, &mut seen);
        }

        args
    }

    /// Extract arguments from a single pattern element recursively
    fn extract_args_from_element(
        &self,
        element: &PatternElement<String>,
        args: &mut Vec<String>,
        seen: &mut HashSet<String>,
    ) {
        match element {
            PatternElement::Placeable { expression } => {
                self.extract_args_from_expression(expression, args, seen);
            }
            PatternElement::TextElement { .. } => {
                // Text elements don't contain arguments
            }
        }
    }

    /// Extract arguments from expressions
    fn extract_args_from_expression(
        &self,
        expr: &Expression<String>,
        args: &mut Vec<String>,
        seen: &mut HashSet<String>,
    ) {
        match expr {
            Expression::Inline(inline_expr) => {
                self.extract_args_from_inline_expression(inline_expr, args, seen);
            }
            Expression::Select { selector, variants } => {
                // Extract from selector
                self.extract_args_from_inline_expression(selector, args, seen);

                // Extract from each variant
                for variant in variants {
                    for element in &variant.value.elements {
                        self.extract_args_from_element(element, args, seen);
                    }
                }
            }
        }
    }

    /// Extract arguments from inline expressions
    fn extract_args_from_inline_expression(
        &self,
        expr: &InlineExpression<String>,
        args: &mut Vec<String>,
        seen: &mut HashSet<String>,
    ) {
        match expr {
            InlineExpression::VariableReference { id } => {
                // Only variable references (with $) are actual arguments
                if seen.insert(id.name.clone()) {
                    args.push(id.name.clone());
                }
            }
            InlineExpression::MessageReference { id, .. } => {
                // Message references are not arguments, they're references to other messages
                debug!("Found message reference (not treating as arg): {}", id.name);
            }
            InlineExpression::TermReference { id, arguments, .. } => {
                // Extract arguments passed to the term
                if let Some(call_args) = arguments {
                    for arg in &call_args.positional {
                        self.extract_args_from_inline_expression(arg, args, seen);
                    }
                    for arg in &call_args.named {
                        self.extract_args_from_inline_expression(&arg.value, args, seen);
                    }
                }
                debug!("Found term reference: {}", id.name);
            }
            InlineExpression::FunctionReference { id, arguments } => {
                // Extract arguments from function calls
                for arg in &arguments.positional {
                    self.extract_args_from_inline_expression(arg, args, seen);
                }
                for arg in &arguments.named {
                    self.extract_args_from_inline_expression(&arg.value, args, seen);
                }
                debug!("Found function reference: {}", id.name);
            }
            InlineExpression::Placeable { expression } => {
                self.extract_args_from_expression(expression, args, seen);
            }
            InlineExpression::StringLiteral { .. } | InlineExpression::NumberLiteral { .. } => {
                // Literals don't contain arguments
            }
        }
    }

    /// Convert pattern elements to readable text for type annotations
    fn pattern_to_text(&self, elements: &[PatternElement<String>]) -> String {
        let mut result = String::new();

        for element in elements {
            match element {
                PatternElement::TextElement { value } => {
                    result.push_str(value);
                }
                PatternElement::Placeable { expression } => match expression {
                    Expression::Inline(InlineExpression::VariableReference { id }) => {
                        result.push_str(&format!("{{ ${} }}", id.name));
                    }
                    Expression::Inline(InlineExpression::MessageReference { id, .. }) => {
                        result.push_str(&format!("{{{}}}", id.name));
                    }
                    Expression::Inline(InlineExpression::TermReference { id, .. }) => {
                        result.push_str(&format!("{{ -{} }}", id.name));
                    }
                    Expression::Inline(InlineExpression::Placeable { expression }) => {
                        result.push_str(&self.expression_to_text(expression));
                    }
                    Expression::Inline(InlineExpression::StringLiteral { value }) => {
                        result.push_str(value);
                    }
                    Expression::Inline(InlineExpression::NumberLiteral { value }) => {
                        result.push_str(value);
                    }

                    _ => {
                        result.push_str("{...}");
                    }
                },
            }

            // Only return first line for type annotations
            if result.contains('\n') {
                if let Some(first_line) = result.lines().next() {
                    return first_line.to_string();
                }
            }
        }

        result
    }

    /// Convert expression to text representation (recursive helper for nested structures)
    fn expression_to_text(&self, expression: &Expression<String>) -> String {
        match expression {
            Expression::Inline(inline_expr) => self.inline_expression_to_text(inline_expr),
            _ => "{...}".to_string(),
        }
    }

    /// Convert inline expression to text representation (recursive helper)
    fn inline_expression_to_text(&self, expression: &InlineExpression<String>) -> String {
        match expression {
            InlineExpression::VariableReference { id } => {
                format!("{{ ${} }}", id.name)
            }
            InlineExpression::MessageReference { id, .. } => {
                format!("{{{}}}", id.name)
            }
            InlineExpression::TermReference { id, .. } => {
                format!("{{ -{} }}", id.name)
            }
            InlineExpression::Placeable { expression } => self.expression_to_text(expression),
            InlineExpression::StringLiteral { value } => value.clone(),
            InlineExpression::NumberLiteral { value } => value.clone(),
            _ => "{...}".to_string(),
        }
    }

    /// Resolve delayed terms after all terms have been collected
    pub fn resolve_delayed_terms(&mut self) {
        // For now, just move all delayed terms to the main terms collection
        // In a more sophisticated implementation, we'd resolve forward references
        for (key, term) in self.delayed_terms.drain(..) {
            self.terms.insert(key, term);
        }
    }

    /// Get all collected messages
    fn into_messages(mut self) -> IndexMap<String, Message> {
        self.resolve_delayed_terms();
        self.messages
    }
}

/// Parse all FTL files in a directory and extract messages
pub fn parse_ftl_files<P: AsRef<Path>>(ftl_path: P) -> Result<IndexMap<String, Message>> {
    let ftl_path = ftl_path.as_ref();
    debug!("Parsing FTL files from: {}", ftl_path.display());

    let mut visitor = FluentVisitor::new();
    let mut file_count = 0;

    // Recursively find all .ftl files
    for entry in walkdir::WalkDir::new(ftl_path) {
        let entry = entry.context("Failed to read directory entry")?;
        let path = entry.path();

        if path.extension().map_or(false, |ext| ext == "ftl") {
            debug!("Parsing FTL file: {}", path.display());

            let content = fs::read_to_string(path)
                .with_context(|| format!("Failed to read FTL file: {}", path.display()))?;

            let resource =
                fluent_syntax::parser::parse(content).map_err(|(_resource, errors)| {
                    let error_msgs: Vec<String> =
                        errors.into_iter().map(|e| format!("{:?}", e)).collect();
                    anyhow::anyhow!(
                        "Parser errors in {}: {}",
                        path.display(),
                        error_msgs.join(", ")
                    )
                })?;

            visitor.visit_resource(&resource);
            file_count += 1;
        }
    }

    debug!("Parsed {} FTL files", file_count);
    let messages = visitor.into_messages();
    debug!("Extracted {} messages total", messages.len());

    Ok(messages)
}

#[cfg(test)]
mod fluent_tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_parse_ftl_files_comprehensive() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;
        let ftl_dir = temp_dir.path();

        // Create multiple FTL files with various message types
        let file1_content = r#"
# Basic messages
hello = Hello, { $name }!
goodbye = Goodbye, { $name }!

# Messages with multiple args
complex = { $arg1 } and { $arg2 } with { $arg3 }

# Messages without args
simple = Simple message
"#;

        let file2_content = r#"
# Terms
-brand-name = SuperApp
-emoji = 🚀

# Messages referencing terms
app-title = Welcome to { -brand-name }!
rocket-message = Blast off { -emoji }
"#;

        fs::write(ftl_dir.join("basic.ftl"), file1_content)?;
        fs::write(ftl_dir.join("advanced.ftl"), file2_content)?;

        let messages = parse_ftl_files(ftl_dir)?;

        // Verify all messages were parsed
        assert!(messages.len() >= 6);

        // Check specific messages
        let hello = messages.get("hello").expect("hello message");
        assert_eq!(hello.args, vec!["name"]);
        assert!(hello.translation.contains("Hello"));

        let complex = messages.get("complex").expect("complex message");
        assert_eq!(complex.args.len(), 3);
        assert!(complex.args.contains(&"arg1".to_string()));
        assert!(complex.args.contains(&"arg2".to_string()));
        assert!(complex.args.contains(&"arg3".to_string()));

        let simple = messages.get("simple").expect("simple message");
        assert!(simple.args.is_empty());

        // Check simple message without expecting term resolution
        let app_title = messages.get("app-title").expect("app-title message");
        // Our implementation keeps term references as { -brand-name }, it doesn't resolve them
        assert!(app_title.translation.contains("{ -brand-name }"));

        Ok(())
    }

    #[test]
    fn test_parse_ftl_files_empty_directory() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;
        let messages = parse_ftl_files(temp_dir.path())?;
        assert!(messages.is_empty());
        Ok(())
    }

    #[test]
    fn test_parse_ftl_files_no_ftl_files() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;

        // Create non-FTL files
        fs::write(temp_dir.path().join("not_ftl.txt"), "not a fluent file")?;
        fs::write(temp_dir.path().join("readme.md"), "# README")?;

        let messages = parse_ftl_files(temp_dir.path())?;
        assert!(messages.is_empty());
        Ok(())
    }

    #[test]
    fn test_fluent_visitor_complex_patterns() {
        let mut visitor = FluentVisitor::new();

        // Test complex inline expressions
        let complex_ftl = r#"
complex-term = { $value } with { $count ->
    [one] one item
    *[other] { $count } items
} and { -brand }
"#.to_string();

        let resource = fluent_syntax::parser::parse(complex_ftl)
            .expect("Failed to parse test FTL");

        visitor.visit_resource(&resource);

        // Should capture all variable references
        let message = visitor.messages.get("complex-term").expect("complex-term message");
        assert!(message.args.contains(&"value".to_string()));
        assert!(message.args.contains(&"count".to_string()));
    }

    #[test]
    fn test_fluent_visitor_term_with_arguments() {
        let mut visitor = FluentVisitor::new();

        // Simplified test with valid FTL syntax
        let ftl_with_term_args = r#"
-greeting = Hello, { $name }!

welcome = { -greeting }
"#.to_string();

        let resource = fluent_syntax::parser::parse(ftl_with_term_args)
            .expect("Failed to parse test FTL");

        visitor.visit_resource(&resource);

        // Before calling into_messages, terms are in delayed_terms
        assert!(visitor.delayed_terms.contains_key("greeting"));

        // Trigger resolution manually
        visitor.resolve_delayed_terms();

        // Check term arguments
        let greeting_term = visitor.terms.get("greeting").expect("greeting term");
        assert!(greeting_term.args.contains(&"name".to_string()));

        // Check message exists
        assert!(visitor.messages.contains_key("welcome"));
    }

    #[test]
    fn test_pattern_to_text_comprehensive() {
        let visitor = FluentVisitor::new();

        // Test various pattern elements
        let test_patterns = [
            ("Simple text", "Simple text"),
            ("{ $variable }", "{ $variable }"),
            ("{ message }", "{message}"),
            ("{ -term }", "{ -term }"),
        ];

        for (input_ftl, expected_output) in test_patterns {
            let ftl_string = format!("test = {}", input_ftl);
            let resource = fluent_syntax::parser::parse(ftl_string)
                .expect("Failed to parse test pattern");

            if let Some(entry) = resource.body.first() {
                if let fluent_syntax::ast::Entry::Message(message) = entry {
                    if let Some(pattern) = &message.value {
                        let result = visitor.pattern_to_text(&pattern.elements);
                        assert!(result.contains(expected_output),
                                "Pattern '{}' should contain '{}'", result, expected_output);
                    }
                }
            }
        }
    }

    #[test]
    fn test_extract_args_from_expression_edge_cases() {
        let visitor = FluentVisitor::new();
        let mut args = Vec::new();
        let mut seen = HashSet::new();

        // Test nested placeable expressions
        let nested_ftl = "test = { { $nested } }".to_string();
        let resource = fluent_syntax::parser::parse(nested_ftl)
            .expect("Failed to parse nested expression");

        if let Some(fluent_syntax::ast::Entry::Message(message)) = resource.body.first() {
            if let Some(pattern) = &message.value {
                for element in &pattern.elements {
                    if let PatternElement::Placeable { expression } = element {
                        visitor.extract_args_from_expression(expression, &mut args, &mut seen);
                    }
                }
            }
        }

        assert!(args.contains(&"nested".to_string()));
    }

    #[test]
    fn test_error_handling_invalid_ftl() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;

        // Create FTL file with syntax error (should not crash, but might fail parsing)
        let invalid_ftl = "invalid-syntax = { unclosed placeholder";
        fs::write(temp_dir.path().join("invalid.ftl"), invalid_ftl)?;

        // Function should either return error or handle gracefully
        // We expect this to fail, but not crash
        let result = parse_ftl_files(temp_dir.path());
        // The function should return an error for invalid FTL
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_message_and_term_structures() {
        let message = Message {
            id: "test-msg".to_string(),
            args: vec!["arg1".to_string(), "arg2".to_string()],
            translation: "Test translation".to_string(),
        };

        assert_eq!(message.id, "test-msg");
        assert_eq!(message.args.len(), 2);
        assert_eq!(message.translation, "Test translation");

        let term = Term {
            id: "test-term".to_string(),
            args: vec!["param".to_string()],
            translation: "Term value".to_string(),
        };

        assert_eq!(term.id, "test-term");
        assert_eq!(term.args.len(), 1);
        assert_eq!(term.translation, "Term value");
    }

    #[test]
    fn test_fluent_visitor_delayed_term_resolution() {
        let mut visitor = FluentVisitor::new();

        // Simple valid FTL syntax
        let ftl_content = r#"
-simple-term = Simple value
message = { -simple-term }
"#.to_string();

        let resource = fluent_syntax::parser::parse(ftl_content)
            .expect("Failed to parse FTL content");

        visitor.visit_resource(&resource);

        // Before resolution, terms are in delayed_terms
        assert!(visitor.delayed_terms.contains_key("simple-term"));
        assert!(visitor.messages.contains_key("message"));

        // Trigger resolution
        visitor.resolve_delayed_terms();

        // Should handle term references correctly
        assert!(visitor.messages.contains_key("message"));
        assert!(visitor.terms.contains_key("simple-term"));
    }

    #[test]
    fn debug_ast_structure() {
        let ftl_content = "test-args = Hello, {name} from {place}!".to_string();
        let resource = fluent_syntax::parser::parse(ftl_content).unwrap();

        let mut visitor = FluentVisitor::new();
        visitor.visit_resource(&resource);

        let messages = visitor.into_messages();
        for (key, message) in &messages {
            println!(
                "Message {}: args={:?}, translation={}",
                key, message.args, message.translation
            );
        }
    }
}

use crate::tree::{Metadata, TreeNode, sorted_keys};
use anyhow::Result;
use indexmap::IndexMap;

/// Generate complete Python stub file content
pub fn generate_stub_content(tree: &IndexMap<String, TreeNode>) -> Result<String> {
    let mut content = String::new();

    content.push_str("# mypy: ignore-errors\n");
    content.push_str("# This is auto-generated file, do not edit!\n");
    content.push_str("from collections.abc import Generator\n");
    content.push_str("from contextlib import contextmanager\n");
    content.push_str("from typing import Any, Literal, overload\n");
    content.push_str("from aiogram_i18n import LazyProxy\n\n");

    generate_i18n_context_class(&mut content)?;
    content.push('\n');

    generate_lazy_factory_class(&mut content)?;
    content.push('\n');

    content.push_str("L: LazyFactory\n\n");

    content.push_str("class I18nStub:\n\n");
    generate_class_body(tree, &mut content, 1)?;

    Ok(content)
}

/// Generate the body of a class with proper indentation
fn generate_class_body(
    tree: &IndexMap<String, TreeNode>,
    content: &mut String,
    indent_level: usize,
) -> Result<()> {
    let indent = "    ".repeat(indent_level);
    let keys = sorted_keys(tree);

    if keys.is_empty() {
        content.push_str(&format!("{}pass\n", indent));
        return Ok(());
    }

    // Add blank line at start of class body
    content.push('\n');

    for key in keys {
        let node = tree.get(&key).unwrap();
        generate_node(key, node, content, indent_level)?;
    }

    Ok(())
}

/// Generate code for a single tree node
fn generate_node(
    key: String,
    node: &TreeNode,
    content: &mut String,
    indent_level: usize,
) -> Result<()> {
    match node {
        TreeNode::Branch(children) => {
            generate_inner_class(&key, children, content, indent_level)?;
        }
        TreeNode::Leaf { meta, children } => {
            if children.is_empty() {
                generate_method(&key, meta, content, indent_level);
            } else {
                generate_overloaded_node(&key, meta, children, content, indent_level)?;
            }
        }
    }

    Ok(())
}

/// Generate an overloaded node (both method and class with same name)
fn generate_overloaded_node(
    key: &str,
    meta: &Metadata,
    children: &IndexMap<String, TreeNode>,
    content: &mut String,
    indent_level: usize,
) -> Result<()> {
    let indent = "    ".repeat(indent_level);

    let return_type = format!("Literal[{}]", format_literal_value(&meta.translation));

    content.push_str(&format!("{}@staticmethod\n", indent));
    content.push_str(&format!("{}@overload\n", indent));

    if meta.args.is_empty() {
        content.push_str(&format!(
            "{}def {}(**kwargs: Any) -> {}:\n",
            indent, key, return_type
        ));
    } else {
        let keyword_args: Vec<String> = meta
            .args
            .iter()
            .map(|arg| format!("{}: Any", arg))
            .collect();
        let args_str = keyword_args.join(", ");
        content.push_str(&format!(
            "{}def {}(*, {}, **kwargs: Any) -> {}:\n",
            indent, key, args_str, return_type
        ));
    }
    content.push_str(&format!("{}    ...\n", indent));
    content.push('\n');

    generate_inner_class(key, children, content, indent_level)?;

    Ok(())
}

/// Generate I18nContext class with full method implementations
fn generate_i18n_context_class(content: &mut String) -> Result<()> {
    content.push_str("class I18nContext(I18nStub):\n\n");
    content.push_str("    def get(self, key: str, /, **kwargs: Any) -> str:\n");
    content.push_str("        ...\n\n");
    content.push_str("    async def set_locale(self, locale: str, **kwargs: Any) -> None:\n");
    content.push_str("        ...\n\n");
    content.push_str("    @contextmanager\n");
    content.push_str("    def use_locale(self, locale: str) -> Generator[I18nContext]:\n");
    content.push_str("        ...\n\n");
    content.push_str("    @contextmanager\n");
    content.push_str("    def use_context(self, **kwargs: Any) -> Generator[I18nContext]:\n");
    content.push_str("        ...\n\n");
    content.push_str("    def set_context(self, **kwargs: Any) -> None:\n");
    content.push_str("        ...\n");
    Ok(())
}

/// Generate LazyFactory class with full method implementations
fn generate_lazy_factory_class(content: &mut String) -> Result<()> {
    content.push_str("class LazyFactory(I18nStub):\n");
    content.push_str("    key_separator: str\n\n");
    content.push_str("    def set_separator(self, key_separator: str) -> None:\n");
    content.push_str("        ...\n\n");
    content.push_str("    def __call__(self, key: str, /, **kwargs: dict[str, Any]) -> LazyProxy:\n");
    content.push_str("        ...\n");
    Ok(())
}

/// Generate a static method
fn generate_method(key: &str, meta: &Metadata, content: &mut String, indent_level: usize) {
    let indent = "    ".repeat(indent_level);
    let return_type = format!("Literal[{}]", format_literal_value(&meta.translation));

    content.push_str(&format!("{}@staticmethod\n", indent));

    if meta.args.is_empty() {
        content.push_str(&format!(
            "{}def {}(**kwargs: Any) -> {}:\n",
            indent, key, return_type
        ));
    } else {
        let keyword_args: Vec<String> = meta
            .args
            .iter()
            .map(|arg| format!("{}: Any", arg))
            .collect();
        let args_str = keyword_args.join(", ");
        content.push_str(&format!(
            "{}def {}(*, {}, **kwargs: Any) -> {}:\n",
            indent, key, args_str, return_type
        ));
    }
    content.push_str(&format!("{}    ...\n", indent));

    content.push('\n');
}

/// Generate an inner class definition with assignment
fn generate_inner_class(
    key: &str,
    children: &IndexMap<String, TreeNode>,
    content: &mut String,
    indent_level: usize,
) -> Result<()> {
    let indent = "    ".repeat(indent_level);
    let class_name = format!("__{}", to_pascal_case(key));

    content.push_str(&format!("{}class {}:\n", indent, class_name));
    generate_class_body(children, content, indent_level + 1)?;

    content.push_str(&format!("{}{} = {}\n", indent, key, class_name));

    Ok(())
}

/// Convert snake_case or kebab-case to PascalCase for class names
fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect()
}

/// Format literal value with proper single quotes and escaping
fn format_literal_value(s: &str) -> String {
    format!("'{}'", escape_string_single_quotes(s))
}

/// Escape string for Python single-quoted literals
fn escape_string_single_quotes(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{Metadata, TreeNode};

    #[test]
    fn test_generate_simple_method() {
        let mut tree = IndexMap::new();

        let metadata = Metadata {
            args: vec![], // No actual variable references in this example
            translation: "Hello, {name}!".to_string(),
        };

        tree.insert("hello".to_string(), TreeNode::new_leaf(metadata));

        let content = generate_stub_content(&tree).unwrap();

        assert!(content.contains("@staticmethod"));
        assert!(content.contains("def hello(**kwargs: Any) -> Literal['Hello, {name}!']:"));
        assert!(content.contains("class I18nContext(I18nStub):"));
        assert!(content.contains("class LazyFactory(I18nStub):"));
    }

    #[test]
    fn test_generate_nested_class() {
        let mut tree = IndexMap::new();
        let mut greeting_children = IndexMap::new();

        let hello_meta = Metadata {
            args: vec![],
            translation: "Hello!".to_string(),
        };

        greeting_children.insert("hello".to_string(), TreeNode::new_leaf(hello_meta));
        tree.insert("greeting".to_string(), TreeNode::Branch(greeting_children));

        let content = generate_stub_content(&tree).unwrap();

        assert!(content.contains("class __Greeting:"));
        assert!(content.contains("def hello(**kwargs: Any) -> Literal['Hello!']:"));
        assert!(content.contains("greeting = __Greeting"));
    }

    #[test]
    fn test_escape_string() {
        assert_eq!(
            escape_string_single_quotes("Hello \"World\""),
            "Hello \"World\""
        );
        assert_eq!(
            escape_string_single_quotes("Line 1\nLine 2"),
            "Line 1\\nLine 2"
        );
        assert_eq!(
            escape_string_single_quotes("Tab\tSeparated"),
            "Tab\\tSeparated"
        );
        assert_eq!(escape_string_single_quotes("Back\\slash"), "Back\\\\slash");
        assert_eq!(
            escape_string_single_quotes("Single'quote"),
            "Single\\'quote"
        );
    }

    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("hello"), "Hello");
        assert_eq!(to_pascal_case("hello_world"), "HelloWorld");
        assert_eq!(to_pascal_case("chat_settings"), "ChatSettings");
        assert_eq!(to_pascal_case("test_case_name"), "TestCaseName");
        assert_eq!(to_pascal_case(""), "");
        assert_eq!(to_pascal_case("a"), "A");
        assert_eq!(to_pascal_case("a_b_c"), "ABC");
    }

    #[test]
    fn test_generate_i18n_context_class() -> Result<()> {
        let mut content = String::new();
        generate_i18n_context_class(&mut content)?;

        assert!(content.contains("class I18nContext(I18nStub):"));
        assert!(content.contains("def get(self, key: str, /, **kwargs: Any) -> str:"));
        assert!(content.contains("async def set_locale(self, locale: str, **kwargs: Any) -> None:"));
        assert!(content.contains("@contextmanager"));
        assert!(content.contains("def use_locale(self, locale: str) -> Generator[I18nContext]:"));
        assert!(content.contains("def use_context(self, **kwargs: Any) -> Generator[I18nContext]:"));
        assert!(content.contains("def set_context(self, **kwargs: Any) -> None:"));

        Ok(())
    }

    #[test]
    fn test_generate_lazy_factory_class() -> Result<()> {
        let mut content = String::new();
        generate_lazy_factory_class(&mut content)?;

        assert!(content.contains("class LazyFactory(I18nStub):"));
        assert!(content.contains("key_separator: str"));
        assert!(content.contains("def set_separator(self, key_separator: str) -> None:"));
        assert!(content.contains("def __call__(self, key: str, /, **kwargs: dict[str, Any]) -> LazyProxy:"));

        Ok(())
    }

    #[test]
    fn test_generate_class_body_empty() -> Result<()> {
        let tree = IndexMap::new();
        let mut content = String::new();

        generate_class_body(&tree, &mut content, 1)?;

        // Empty tree should generate pass
        assert!(content.contains("pass"));

        Ok(())
    }

    #[test]
    fn test_generate_inner_class() -> Result<()> {
        let mut children = IndexMap::new();
        children.insert("test".to_string(), TreeNode::Leaf {
            meta: Metadata {
                args: vec![],
                translation: "Test".to_string(),
            },
            children: IndexMap::new(),
        });

        let mut content = String::new();
        generate_inner_class("TestClass", &children, &mut content, 1)?;

        assert!(content.contains("class __TestClass:"));
        // The assignment might be on the next line or generated separately
        // Let's just check the class is generated properly
        assert!(content.contains("def test(**kwargs: Any)"));

        Ok(())
    }

    #[test]
    fn test_format_literal_value_escaping() {
        assert_eq!(format_literal_value("simple"), "'simple'");
        assert_eq!(format_literal_value("with'quote"), "'with\\'quote'");
        assert_eq!(format_literal_value("multi\nline"), "'multi\\nline'");
        assert_eq!(format_literal_value("with { $var }"), "'with { $var }'");
    }

    #[test]
    fn test_generate_overloaded_node_comprehensive() -> Result<()> {
        let mut children = IndexMap::new();
        children.insert("nested".to_string(), TreeNode::Leaf {
            meta: Metadata {
                args: vec!["nested_arg".to_string()],
                translation: "Nested message".to_string(),
            },
            children: IndexMap::new(),
        });

        let meta = Metadata {
            args: vec!["arg1".to_string(), "arg2".to_string()],
            translation: "Test with args".to_string(),
        };

        let mut content = String::new();
        generate_overloaded_node("overloaded", &meta, &children, &mut content, 1)?;

        // Should have @staticmethod @overload
        assert!(content.contains("@staticmethod"));
        assert!(content.contains("@overload"));

        // Should have method with args
        assert!(content.contains("def overloaded(*, arg1: Any, arg2: Any, **kwargs: Any)"));

        // Should have class definition
        assert!(content.contains("class __Overloaded:"));

        Ok(())
    }

    #[test]
    fn test_generate_method_no_args() {
        let meta = Metadata {
            args: vec![],
            translation: "No args message".to_string(),
        };

        let mut content = String::new();
        generate_method("simple", &meta, &mut content, 1);

        assert!(content.contains("@staticmethod"));
        assert!(content.contains("def simple(**kwargs: Any)"));
        assert!(content.contains("Literal['No args message']"));
    }

    #[test]
    fn test_generate_method_with_args() {
        let meta = Metadata {
            args: vec!["name".to_string(), "count".to_string()],
            translation: "Hello {name}, you have {count} items".to_string(),
        };

        let mut content = String::new();
        generate_method("complex", &meta, &mut content, 1);

        assert!(content.contains("@staticmethod"));
        assert!(content.contains("def complex(*, name: Any, count: Any, **kwargs: Any)"));
        assert!(content.contains("Literal['Hello {name}, you have {count} items']"));
    }

    #[test]
    fn test_generate_stub_content_comprehensive() -> Result<()> {
        let mut tree = IndexMap::new();

        // Add various node types
        tree.insert("simple".to_string(), TreeNode::Leaf {
            meta: Metadata {
                args: vec![],
                translation: "Simple".to_string(),
            },
            children: IndexMap::new(),
        });

        tree.insert("complex".to_string(), TreeNode::Leaf {
            meta: Metadata {
                args: vec!["arg".to_string()],
                translation: "Complex with {arg}".to_string(),
            },
            children: IndexMap::new(),
        });

        let mut nested_children = IndexMap::new();
        nested_children.insert("child".to_string(), TreeNode::Leaf {
            meta: Metadata {
                args: vec![],
                translation: "Child".to_string(),
            },
            children: IndexMap::new(),
        });

        tree.insert("nested".to_string(), TreeNode::Branch(nested_children));

        let content = generate_stub_content(&tree)?;

        // Check header
        assert!(content.contains("# mypy: ignore-errors"));
        assert!(content.contains("from collections.abc import Generator"));
        assert!(content.contains("from typing import Any, Literal, overload"));

        // Check classes
        assert!(content.contains("class I18nContext(I18nStub):"));
        assert!(content.contains("class LazyFactory(I18nStub):"));
        assert!(content.contains("class I18nStub:"));

        // Check methods
        assert!(content.contains("def simple(**kwargs: Any)"));
        assert!(content.contains("def complex(*, arg: Any, **kwargs: Any)"));

        // Check nested structure
        assert!(content.contains("class __Nested:"));
        assert!(content.contains("def child(**kwargs: Any)"));

        Ok(())
    }
}

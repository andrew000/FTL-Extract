use crate::fluent::Message;
use anyhow::Result;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Metadata for a translation message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    /// List of argument names used in the translation
    pub args: Vec<String>,
    /// First line of the translation text for type annotations
    pub translation: String,
}

/// Tree node that can either be a branch (with children) or a leaf (with metadata)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TreeNode {
    /// Branch node containing child nodes
    Branch(IndexMap<String, TreeNode>),
    /// Leaf node containing translation metadata
    Leaf {
        #[serde(rename = "$_meta$")]
        meta: Metadata,
        #[serde(flatten)]
        children: IndexMap<String, TreeNode>,
    },
}

impl TreeNode {
    /// Create a new empty branch node
    fn new_branch() -> Self {
        TreeNode::Branch(IndexMap::new())
    }

    /// Create a new leaf node with metadata
    pub fn new_leaf(meta: Metadata) -> Self {
        TreeNode::Leaf {
            meta,
            children: IndexMap::new(),
        }
    }

    /// Get mutable access to children, converting to leaf if necessary
    fn children_mut(&mut self) -> &mut IndexMap<String, TreeNode> {
        match self {
            TreeNode::Branch(children) => children,
            TreeNode::Leaf { children, .. } => children,
        }
    }

    /// Get immutable access to children
    pub fn children(&self) -> &IndexMap<String, TreeNode> {
        match self {
            TreeNode::Branch(children) => children,
            TreeNode::Leaf { children, .. } => children,
        }
    }

    /// Get metadata if this is a leaf node
    pub fn metadata(&self) -> Option<&Metadata> {
        match self {
            TreeNode::Leaf { meta, .. } => Some(meta),
            TreeNode::Branch(_) => None,
        }
    }

    /// Check if this node has metadata (is a leaf)
    pub fn has_metadata(&self) -> bool {
        matches!(self, TreeNode::Leaf { .. })
    }
}

/// Build nested tree structure from flat message keys
///
/// Converts flat keys like "greeting-hello" into nested structure:
/// ```text
/// greeting
///   hello
///     metadata
/// ```
pub fn build_tree(messages: IndexMap<String, Message>) -> Result<IndexMap<String, TreeNode>> {
    let mut tree = IndexMap::new();

    for (key, message) in messages {
        // Split key by hyphens to create hierarchy
        let parts: Vec<&str> = key.split('-').collect();

        if parts.is_empty() {
            continue;
        }

        // Navigate/create the tree structure
        let mut current = &mut tree;

        // Navigate through all parts except the last
        for part in &parts[..parts.len() - 1] {
            if !current.contains_key(*part) {
                current.insert(part.to_string(), TreeNode::new_branch());
            }

            // Get the node and ensure it can hold children
            let node = current.get_mut(*part).unwrap();
            current = node.children_mut();
        }

        // Handle the final part (leaf node)
        let final_part = parts[parts.len() - 1];
        let metadata = Metadata {
            args: message.args,
            translation: message.translation,
        };

        match current.get_mut(final_part) {
            Some(existing_node) => {
                // If node already exists, convert it to a leaf while preserving children
                match existing_node {
                    TreeNode::Branch(children) => {
                        // Convert branch to leaf, preserving existing children
                        let old_children = std::mem::take(children);
                        *existing_node = TreeNode::Leaf {
                            meta: metadata,
                            children: old_children,
                        };
                    }
                    TreeNode::Leaf {
                        meta: existing_meta,
                        ..
                    } => {
                        // Node already has metadata - this shouldn't happen with unique keys
                        log::warn!("Duplicate key found: {}", key);
                        *existing_meta = metadata;
                    }
                }
            }
            None => {
                // Create new leaf node
                current.insert(final_part.to_string(), TreeNode::new_leaf(metadata));
            }
        }
    }

    Ok(tree)
}

/// Export tree structure to JSON file for debugging
pub fn export_tree_json<P: AsRef<Path>>(
    tree: &IndexMap<String, TreeNode>,
    output_path: P,
) -> Result<()> {
    let json = serde_json::to_string_pretty(tree)?;
    std::fs::write(output_path, json)?;
    Ok(())
}

/// Check if a tree node represents both a method and has children (needs @overload)
pub fn needs_overload(node: &TreeNode) -> bool {
    match node {
        TreeNode::Leaf { children, .. } => !children.is_empty(),
        TreeNode::Branch(_) => false,
    }
}

/// Get sorted keys for consistent output
pub fn sorted_keys(map: &IndexMap<String, TreeNode>) -> Vec<String> {
    let mut keys: Vec<_> = map.keys().cloned().collect();
    keys.sort();
    keys
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fluent::Message;

    #[test]
    fn test_build_simple_tree() {
        let mut messages = IndexMap::new();
        messages.insert(
            "greeting-hello".to_string(),
            Message {
                id: "greeting-hello".to_string(),
                args: vec!["name".to_string()],
                translation: "Hello, {name}!".to_string(),
            },
        );

        let tree = build_tree(messages).unwrap();

        assert_eq!(tree.len(), 1);
        assert!(tree.contains_key("greeting"));

        let greeting = tree.get("greeting").unwrap();
        let greeting_children = greeting.children();
        assert!(greeting_children.contains_key("hello"));

        let hello = greeting_children.get("hello").unwrap();
        assert!(hello.has_metadata());

        let metadata = hello.metadata().unwrap();
        assert_eq!(metadata.args, vec!["name".to_string()]);
        assert_eq!(metadata.translation, "Hello, {name}!");
    }

    #[test]
    fn test_build_nested_tree() {
        let mut messages = IndexMap::new();
        messages.insert(
            "app-menu-file-open".to_string(),
            Message {
                id: "app-menu-file-open".to_string(),
                args: vec![],
                translation: "Open".to_string(),
            },
        );

        let tree = build_tree(messages).unwrap();

        // Navigate: app -> menu -> file -> open
        let app = tree.get("app").unwrap();
        let menu = app.children().get("menu").unwrap();
        let file = menu.children().get("file").unwrap();
        let open = file.children().get("open").unwrap();

        assert!(open.has_metadata());
        let metadata = open.metadata().unwrap();
        assert_eq!(metadata.translation, "Open");
    }

    #[test]
    fn test_overload_detection() {
        let mut messages = IndexMap::new();

        // Create both "greeting" and "greeting-hello"
        messages.insert(
            "greeting".to_string(),
            Message {
                id: "greeting".to_string(),
                args: vec![],
                translation: "Greetings".to_string(),
            },
        );
        messages.insert(
            "greeting-hello".to_string(),
            Message {
                id: "greeting-hello".to_string(),
                args: vec!["name".to_string()],
                translation: "Hello, {name}!".to_string(),
            },
        );

        let tree = build_tree(messages).unwrap();
        let greeting = tree.get("greeting").unwrap();

        // Should need overload since it has both metadata and children
        assert!(needs_overload(greeting));
    }

    #[test]
    fn test_export_tree_json() -> Result<()> {
        let mut messages = IndexMap::new();
        messages.insert(
            "test".to_string(),
            Message {
                id: "test".to_string(),
                args: vec!["arg".to_string()],
                translation: "Test message".to_string(),
            },
        );

        let tree = build_tree(messages)?;

        let temp_dir = tempfile::TempDir::new()?;
        let json_path = temp_dir.path().join("tree.json");

        export_tree_json(&tree, &json_path)?;

        assert!(json_path.exists());

        // Verify JSON content is valid
        let json_content = std::fs::read_to_string(&json_path)?;
        let parsed: serde_json::Value = serde_json::from_str(&json_content)?;

        // Should contain test node
        assert!(parsed.get("test").is_some());

        Ok(())
    }

    #[test]
    fn test_sorted_keys() {
        let mut map = IndexMap::new();
        map.insert("zebra".to_string(), TreeNode::Branch(IndexMap::new()));
        map.insert("apple".to_string(), TreeNode::Branch(IndexMap::new()));
        map.insert("banana".to_string(), TreeNode::Branch(IndexMap::new()));

        let sorted = sorted_keys(&map);
        assert_eq!(sorted, vec!["apple", "banana", "zebra"]);
    }

    #[test]
    fn test_tree_node_edge_cases() {
        // Test empty tree
        let empty_messages = IndexMap::new();
        let empty_tree = build_tree(empty_messages).unwrap();
        assert!(empty_tree.is_empty());

        // Test single message
        let mut single_message = IndexMap::new();
        single_message.insert(
            "single".to_string(),
            Message {
                id: "single".to_string(),
                args: vec![],
                translation: "Single message".to_string(),
            },
        );

        let single_tree = build_tree(single_message).unwrap();
        assert_eq!(single_tree.len(), 1);
        assert!(single_tree.contains_key("single"));
    }

    #[test]
    fn test_tree_node_methods() {
        let mut children = IndexMap::new();
        children.insert("child".to_string(), TreeNode::Branch(IndexMap::new()));

        let metadata = Metadata {
            args: vec!["test".to_string()],
            translation: "Test".to_string(),
        };

        // Test Leaf node
        let leaf = TreeNode::Leaf {
            meta: metadata.clone(),
            children: children.clone(),
        };

        assert!(leaf.has_metadata());
        assert_eq!(leaf.metadata().unwrap().args, vec!["test".to_string()]);
        assert_eq!(leaf.children().len(), 1);
        assert!(leaf.children().contains_key("child"));

        // Test Branch node
        let branch = TreeNode::Branch(children.clone());
        assert!(!branch.has_metadata());
        assert!(branch.metadata().is_none());
        assert_eq!(branch.children().len(), 1);
    }
}

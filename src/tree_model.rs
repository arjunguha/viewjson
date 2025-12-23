use crate::json_reader::ParseResult;
use crate::path_formatting::{build_array_path, build_object_path};
use crate::value_formatting::{format_value_literal, format_value_preview};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct TreeNode {
    pub name: String,
    pub preview: String,
    pub path: String,
    pub full_value: String,
    pub display_value: String,
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    fn from_value(name: &str, path: &str, value: &Value) -> Self {
        let preview = format_value_preview(value);
        let display_value = format_value_literal(value);
        let full_value = serde_json::to_string(value).unwrap_or_else(|_| preview.clone());

        let mut children = Vec::new();
        match value {
            Value::Object(map) => {
                for (key, val) in map.iter() {
                    let child_path = build_object_path(path, key);
                    children.push(TreeNode::from_value(key, &child_path, val));
                }
            }
            Value::Array(items) => {
                for (idx, val) in items.iter().enumerate() {
                    let child_path = build_array_path(path, idx);
                    let child_name = format!("[{}]", idx);
                    children.push(TreeNode::from_value(&child_name, &child_path, val));
                }
            }
            _ => {}
        }

        TreeNode {
            name: name.to_string(),
            preview,
            path: path.to_string(),
            full_value,
            display_value,
            children,
        }
    }
}

pub fn build_tree_from_parse_result(result: ParseResult, display_name: &str) -> TreeNode {
    match result {
        ParseResult::Single(value) => TreeNode::from_value(display_name, "$", &value),
        ParseResult::JsonL(values) => build_tree_from_jsonl(values, display_name),
    }
}

fn build_tree_from_jsonl(values: Vec<Value>, display_name: &str) -> TreeNode {
    let jsonl_name = format!("{} (JSONL)", display_name);
    let root_path = display_name.to_string();
    let root_value = serde_json::json!({ "lines": values.len() });

    let mut children = Vec::new();
    for (idx, value) in values.iter().enumerate() {
        let line_name = format!("Line {}", idx + 1);
        let path = build_array_path(&root_path, idx);
        children.push(TreeNode::from_value(&line_name, &path, value));
    }

    TreeNode {
        name: jsonl_name,
        preview: format!("{} objects", values.len()),
        path: root_path,
        full_value: serde_json::to_string(&root_value).unwrap_or_else(|_| "{}".to_string()),
        display_value: format_value_literal(&root_value),
        children,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_tree_for_simple_object() {
        let value = serde_json::json!({"name": "value", "items": [1, 2]});
        let root = build_tree_from_parse_result(ParseResult::Single(value), "test.json");
        assert_eq!(root.name, "test.json");
        assert_eq!(root.path, "$");
        assert_eq!(root.children.len(), 2);

        let name = root.children.iter().find(|child| child.name == "name");
        assert!(name.is_some());

        let items = root.children.iter().find(|child| child.name == "items");
        assert!(items.is_some());
        let items_children = &items.unwrap().children;
        assert_eq!(items_children.len(), 2);
        assert_eq!(items_children[0].name, "[0]");
        assert_eq!(items_children[0].path, "$.items[0]");
    }

    #[test]
    fn build_tree_for_jsonl() {
        let values = vec![serde_json::json!({"a": 1}), serde_json::json!({"b": 2})];
        let root = build_tree_from_parse_result(ParseResult::JsonL(values), "data.jsonl");
        assert_eq!(root.name, "data.jsonl (JSONL)");
        assert_eq!(root.children.len(), 2);
        assert_eq!(root.children[0].name, "Line 1");
        assert!(root.preview.contains("2"));
    }
}

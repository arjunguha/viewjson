// Copyright (C) 2025 Arjun Guha
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use glib::ToValue;
use gtk::prelude::{TreeStoreExt, TreeStoreExtManual};
use gtk::{TreeIter, TreeStore};
use serde_json::Value;
use slopjson::path_formatting::{build_array_path, build_object_path};
use slopjson::value_formatting::format_value_preview;

/// Sets all column values for a tree node.
///
/// # Arguments
///
/// * `tree_store` - The tree store to update
/// * `iter` - The tree iterator for the node
/// * `name` - Display name for the node (key or index)
/// * `value` - The JSON value to store
/// * `path` - The JSON path string for this node
pub fn set_tree_node_values(
    tree_store: &TreeStore,
    iter: &TreeIter,
    name: &str,
    value: &Value,
    path: &str,
) {
    tree_store.set_value(iter, 0, &name.to_value());
    tree_store.set_value(iter, 1, &format_value_preview(value).to_value());
    tree_store.set_value(iter, 2, &path.to_value());
    tree_store.set_value(
        iter,
        3,
        &serde_json::to_string(value).unwrap_or_default().to_value(),
    );
}

/// Recursively populates a tree store with JSON values.
///
/// # Arguments
///
/// * `tree_store` - The tree store to populate
/// * `parent` - The parent iterator (None for root)
/// * `value` - The JSON value to add
/// * `path` - The JSON path string for this node
pub fn populate_tree(tree_store: &TreeStore, parent: &TreeIter, value: &Value, path: &str) {
    match value {
        Value::Object(map) => {
            for (key, val) in map.iter() {
                let iter = tree_store.append(Some(parent));
                let new_path = build_object_path(path, key);
                set_tree_node_values(tree_store, &iter, key, val, &new_path);
                populate_tree(tree_store, &iter, val, &new_path);
            }
        }
        Value::Array(arr) => {
            for (idx, val) in arr.iter().enumerate() {
                let iter = tree_store.append(Some(parent));
                let new_path = build_array_path(path, idx);
                let name = format!("[{}]", idx);
                set_tree_node_values(tree_store, &iter, &name, val, &new_path);
                populate_tree(tree_store, &iter, val, &new_path);
            }
        }
        _ => {
            // Leaf value, already set in parent call
        }
    }
}

/// Adds a single JSON value to the tree store as a root node.
///
/// # Arguments
///
/// * `tree_store` - The tree store to add to
/// * `value` - The JSON value to add
/// * `root_name` - Display name for the root node
pub fn add_single_value_to_tree(tree_store: &TreeStore, value: &Value, root_name: &str) {
    let root_iter = tree_store.append(None);
    // Use "$" as the root path for single objects/arrays (JSONPath notation)
    // This ensures proper path generation for nested structures
    let root_path = "$";
    set_tree_node_values(tree_store, &root_iter, root_name, value, root_path);
    populate_tree(tree_store, &root_iter, value, root_path);
}

/// Adds a JSONL result to the tree store.
///
/// # Arguments
///
/// * `tree_store` - The tree store to add to
/// * `json_values` - The array of JSON values from the JSONL file
/// * `display_name` - Display name for the root node
/// * `root_path` - The root path string (typically the file name)
pub fn add_jsonl_to_tree(
    tree_store: &TreeStore,
    json_values: &[Value],
    display_name: &str,
    root_path: &str,
) {
    let root_iter = tree_store.append(None);
    let root_json = serde_json::json!({ "lines": json_values.len() });
    let root_name = format!("{} (JSONL)", display_name);
    tree_store.set_value(&root_iter, 0, &root_name.to_value());
    tree_store.set_value(
        &root_iter,
        1,
        &format!("{} objects", json_values.len()).to_value(),
    );
    tree_store.set_value(&root_iter, 2, &root_path.to_value());
    tree_store.set_value(
        &root_iter,
        3,
        &serde_json::to_string(&root_json)
            .unwrap_or_default()
            .to_value(),
    );

    for (idx, value) in json_values.iter().enumerate() {
        let line_iter = tree_store.append(Some(&root_iter));
        let path = build_array_path(root_path, idx);
        let name = format!("Line {}", idx + 1);
        set_tree_node_values(tree_store, &line_iter, &name, value, &path);
        populate_tree(tree_store, &line_iter, value, &path);
    }
}

// Note: Tree building functions are tightly coupled to GTK and require GTK initialization.
// Integration tests for these functions would require GTK to be initialized, which is
// complex in a test environment. The core logic (path building, value formatting) is
// tested separately in their respective modules.

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
mod json_reader;
mod path_formatting;

use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Box as GtkBox, CellRendererText, Clipboard, Entry,
    FileChooserAction, FileChooserDialog, IconSize, Image, Orientation, Paned, ResponseType,
    ScrolledWindow, Separator, TextBuffer, TextView, TreeStore, TreeView, TreeViewColumn,
};
use json_reader::{parse_file, parse_text_content, ParseResult};
use path_formatting::{build_array_path, build_object_path};
use serde_json::Value;
use std::path::Path;

fn main() {
    // Read command-line arguments before GTK initialization
    // Collect all arguments after the program name
    let file_paths: Vec<String> = std::env::args().skip(1).collect();

    let app = Application::builder()
        .application_id("com.example.viewjson")
        .build();

    let file_paths_clone = file_paths.clone();
    app.connect_activate(move |app| {
        build_ui(app, &file_paths_clone);
    });

    // Run with empty args to prevent GTK from trying to handle file arguments
    app.run_with_args(&[] as &[&str]);
}

fn build_ui(app: &Application, initial_files: &[String]) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("JSON Viewer")
        .default_width(1200)
        .default_height(800)
        .build();

    // Create paned widget for two-pane layout
    let paned = Paned::new(gtk::Orientation::Horizontal);

    // Left pane: Tree view
    let left_scroll = ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    left_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);

    // Create tree store with columns: name, value, json_path, full_value
    let tree_store = TreeStore::new(&[
        glib::Type::STRING, // Column 0: Display name (key/index)
        glib::Type::STRING, // Column 1: Value preview
        glib::Type::STRING, // Column 2: Full JSON path (for selection)
        glib::Type::STRING, // Column 3: Full JSON value as string
    ]);

    let tree_view = TreeView::with_model(&tree_store);

    // Create columns
    let col_name = TreeViewColumn::new();
    let col_value = TreeViewColumn::new();

    let cell_name = CellRendererText::new();
    let cell_value = CellRendererText::new();

    TreeViewColumnExt::pack_start(&col_name, &cell_name, true);
    TreeViewColumnExt::pack_start(&col_value, &cell_value, true);

    col_name.set_title("Key");
    col_value.set_title("Value");

    TreeViewColumnExt::add_attribute(&col_name, &cell_name, "text", 0);
    TreeViewColumnExt::add_attribute(&col_value, &cell_value, "text", 1);

    tree_view.append_column(&col_name);
    tree_view.append_column(&col_value);

    tree_view.set_headers_visible(true);
    tree_view.set_expander_column(Some(&col_name));

    left_scroll.add(&tree_view);

    // Right pane: Path and Value display
    let right_box = GtkBox::new(Orientation::Vertical, 0);

    // Path display (single line)
    let path_entry = Entry::new();
    path_entry.set_editable(false);
    path_entry.set_hexpand(true);
    path_entry.set_halign(gtk::Align::Fill);
    right_box.pack_start(&path_entry, false, false, 0);

    // Separator
    let separator = Separator::new(Orientation::Horizontal);
    right_box.pack_start(&separator, false, false, 0);

    // Value display (multi-line)
    let value_scroll = ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    value_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
    value_scroll.set_hexpand(true);
    value_scroll.set_vexpand(true);

    let value_text_view = TextView::new();
    value_text_view.set_editable(false);
    value_text_view.set_monospace(true);
    value_text_view.set_wrap_mode(gtk::WrapMode::Word);

    value_scroll.add(&value_text_view);
    right_box.pack_start(&value_scroll, true, true, 0);

    // Add panes to paned widget
    paned.add1(&left_scroll);
    paned.add2(&right_box);
    paned.set_position(400); // Initial split position

    // Handle tree selection
    let selection = tree_view.selection();
    let path_entry_clone = path_entry.clone();
    let value_text_buffer = value_text_view.buffer().unwrap();
    let value_text_buffer_clone = value_text_buffer.clone();

    selection.connect_changed(move |sel| {
        if let Some((model, iter)) = sel.selected() {
            let path = model.value(&iter, 2).get::<String>().unwrap_or_default();
            let full_value = model.value(&iter, 3).get::<String>().unwrap_or_default();

            // Set path in the entry
            path_entry_clone.set_text(&path);

            // Format the JSON value nicely
            let formatted_value = if !full_value.is_empty() {
                match serde_json::from_str::<Value>(&full_value) {
                    Ok(v) => format_value_literal(&v),
                    Err(_) => full_value,
                }
            } else {
                model.value(&iter, 1).get::<String>().unwrap_or_default()
            };

            value_text_buffer_clone.set_text(&formatted_value);
        } else {
            path_entry_clone.set_text("");
            value_text_buffer_clone.set_text("");
        }
    });

    // Handle Delete key to close files (root nodes)
    let tree_store_for_delete = tree_store.clone();
    let path_entry_for_delete = path_entry.clone();
    let value_text_buffer_for_delete = value_text_buffer.clone();
    tree_view.connect_key_press_event(move |tree_view, event| {
        let keyval = event.keyval();

        // Check for Delete key by name
        // Delete key is typically named "Delete" and Backspace is "BackSpace"
        if let Some(key_name) = keyval.name() {
            if key_name.as_str() == "Delete" || key_name.as_str() == "BackSpace" {
                let selection = tree_view.selection();
                if let Some((model, iter)) = selection.selected() {
                    // Check if this is a root node (no parent)
                    if model.iter_parent(&iter).is_none() {
                        // This is a root node - remove it
                        tree_store_for_delete.remove(&iter);

                        // Clear the display if we deleted the selected item
                        path_entry_for_delete.set_text("");
                        value_text_buffer_for_delete.set_text("");

                        // Try to select the next root node if available
                        if let Some(first_iter) = tree_store_for_delete.iter_first() {
                            selection.select_iter(&first_iter);
                        }

                        return gtk::glib::Propagation::Stop;
                    }
                }
            }
        }
        gtk::glib::Propagation::Proceed
    });

    // Add file chooser button
    let header_bar = gtk::HeaderBar::new();
    header_bar.set_show_close_button(true);
    header_bar.set_title(Some("JSON Viewer"));

    let open_button = gtk::Button::builder()
        .label("Open")
        .tooltip_text("Open a JSON/JSONL file")
        .build();
    let open_icon = Image::from_icon_name(Some("document-open"), IconSize::Button);
    open_button.set_image(Some(&open_icon));
    open_button.set_always_show_image(true);
    let tree_store_for_open = tree_store.clone();
    let value_text_buffer_for_open = value_text_buffer.clone();
    let window_clone = window.clone();

    open_button.connect_clicked(move |_| {
        let tree_store_clone = tree_store_for_open.clone();
        let value_text_buffer_clone = value_text_buffer_for_open.clone();
        let window_clone2 = window_clone.clone();

        let dialog = FileChooserDialog::new(
            Some("Open JSON File"),
            Some(&window_clone2),
            FileChooserAction::Open,
        );

        dialog.add_button("Cancel", ResponseType::Cancel);
        dialog.add_button("Open", ResponseType::Accept);

        dialog.connect_response(move |dialog, response| {
            if response == ResponseType::Accept {
                if let Some(file) = dialog.file() {
                    if let Some(path) = file.path() {
                        let name = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("Unknown")
                            .to_string();
                        load_json_content_from_file(
                            &path,
                            Some(&name),
                            &tree_store_clone,
                            &value_text_buffer_clone,
                        );
                    }
                }
            }
            dialog.close();
        });

        dialog.show();
    });

    // Add clipboard button
    let clipboard_button = gtk::Button::builder()
        .label("Paste")
        .tooltip_text("Load JSON/JSONL/YAML from clipboard")
        .build();
    let clipboard_icon = Image::from_icon_name(Some("edit-paste"), IconSize::Button);
    clipboard_button.set_image(Some(&clipboard_icon));
    clipboard_button.set_always_show_image(true);
    let tree_store_for_clipboard = tree_store.clone();
    let value_text_buffer_for_clipboard = value_text_buffer.clone();

    clipboard_button.connect_clicked(move |_| {
        let tree_store_clone = tree_store_for_clipboard.clone();
        let value_text_buffer_clone = value_text_buffer_for_clipboard.clone();

        let clipboard = Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD);
        clipboard.request_text(move |_clipboard, text| {
            if let Some(content) = text {
                load_json_content(
                    &content,
                    Some("Clipboard"),
                    &tree_store_clone,
                    &value_text_buffer_clone,
                );
            } else {
                value_text_buffer_clone.set_text("Clipboard is empty or does not contain text");
            }
        });
    });

    // Add copy value button
    let copy_value_button = gtk::Button::builder()
        .label("Copy")
        .tooltip_text("Copy selected value to clipboard")
        .build();
    let copy_icon = Image::from_icon_name(Some("edit-copy"), IconSize::Button);
    copy_value_button.set_image(Some(&copy_icon));
    copy_value_button.set_always_show_image(true);
    let selection_for_copy = selection.clone();
    let value_text_buffer_for_copy = value_text_buffer.clone();

    copy_value_button.connect_clicked(move |_| {
        let clipboard = Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD);

        // Get the currently selected value from the text buffer
        let start_iter = value_text_buffer_for_copy.start_iter();
        let end_iter = value_text_buffer_for_copy.end_iter();
        if let Some(value_text) = value_text_buffer_for_copy.text(&start_iter, &end_iter, false) {
            if !value_text.as_str().is_empty() {
                clipboard.set_text(value_text.as_str());
                return;
            }
        }

        // If text buffer is empty, try to get value from selected tree item
        if let Some((model, iter)) = selection_for_copy.selected() {
            let full_value = model.value(&iter, 3).get::<String>().unwrap_or_default();
            if !full_value.is_empty() {
                clipboard.set_text(&full_value);
            }
        }
    });

    header_bar.pack_start(&open_button);
    header_bar.pack_start(&clipboard_button);
    header_bar.pack_start(&copy_value_button);
    window.set_titlebar(Some(&header_bar));

    window.add(&paned);
    window.show_all();

    // Try to load from command line arguments
    for file_path in initial_files {
        let path = Path::new(file_path);
        if path.exists() {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown")
                .to_string();
            load_json_content_from_file(path, Some(&name), &tree_store, &value_text_buffer);
        }
    }
}

/// Adds a JSONL result to the tree store
fn add_jsonl_to_tree(
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

/// Adds a single JSON value to the tree store
fn add_single_value_to_tree(
    tree_store: &TreeStore,
    value: &Value,
    root_name: &str,
) {
    let root_iter = tree_store.append(None);
    set_tree_node_values(tree_store, &root_iter, root_name, value, root_name);
    populate_tree(tree_store, &root_iter, value, root_name);
}

/// Loads parsed content into the tree store
fn load_parse_result(
    result: Result<ParseResult, json_reader::ParseError>,
    default_name: &str,
    tree_store: &TreeStore,
    value_text_buffer: &TextBuffer,
    error_prefix: &str,
) {
    match result {
        Ok(ParseResult::JsonL(json_values)) => {
            add_jsonl_to_tree(tree_store, &json_values, default_name, default_name);
        }
        Ok(ParseResult::Single(value)) => {
            add_single_value_to_tree(tree_store, &value, default_name);
        }
        Err(e) => {
            value_text_buffer.set_text(&format!("{}: {}", error_prefix, e));
        }
    }
}

fn load_json_content_from_file(
    path: &Path,
    name: Option<&str>,
    tree_store: &TreeStore,
    value_text_buffer: &TextBuffer,
) {
    let display_name = name.unwrap_or("File");
    let result = parse_file(path);
    load_parse_result(result, &display_name, tree_store, value_text_buffer, "Error parsing file");
}

fn load_json_content(
    content: &str,
    name: Option<&str>,
    tree_store: &TreeStore,
    value_text_buffer: &TextBuffer,
) {
    let display_name = name.unwrap_or("Content");
    let result = parse_text_content(content);
    load_parse_result(result, &display_name, tree_store, value_text_buffer, "Error parsing content");
}

/// Sets all column values for a tree node
fn set_tree_node_values(
    tree_store: &TreeStore,
    iter: &gtk::TreeIter,
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

fn populate_tree(tree_store: &TreeStore, parent: &gtk::TreeIter, value: &Value, path: &str) {
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

fn format_value_preview(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => {
            if s.len() > 50 {
                format!("\"{}\"...", &s[..50])
            } else {
                format!("\"{}\"", s)
            }
        }
        Value::Array(arr) => format!("Array[{}]", arr.len()),
        Value::Object(map) => format!("Object{{{}}}", map.len()),
    }
}

fn format_value_literal(value: &Value) -> String {
    match value {
        Value::String(s) => {
            // Display string literally - escape sequences are already unescaped by serde_json
            // So we just display the string as-is
            s.clone()
        }
        _ => {
            // For non-string values, format as pretty JSON
            serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
        }
    }
}

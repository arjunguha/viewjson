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
    FileChooserAction, FileChooserDialog, Menu, MenuBar, MenuItem, Orientation, Paned,
    ResponseType, ScrolledWindow, Separator, TextBuffer, TextView, TreeStore, TreeView,
    TreeViewColumn,
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
    let left_box = GtkBox::new(Orientation::Vertical, 6);
    left_box.set_margin_start(6);
    left_box.set_margin_end(6);
    left_box.set_margin_top(6);
    left_box.set_margin_bottom(6);

    let tree_label = gtk::Label::new(Some("JSON Structure:"));
    tree_label.set_halign(gtk::Align::Start);
    left_box.pack_start(&tree_label, false, false, 0);

    let left_scroll = ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    left_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
    left_scroll.set_vexpand(true);

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
    left_box.pack_start(&left_scroll, true, true, 0);

    // Right pane: Path and Value display
    let right_box = GtkBox::new(Orientation::Vertical, 6);
    right_box.set_margin_start(6);
    right_box.set_margin_end(6);
    right_box.set_margin_top(6);
    right_box.set_margin_bottom(6);

    // Path label and display
    let path_label = gtk::Label::new(Some("JSON Path:"));
    path_label.set_halign(gtk::Align::Start);
    right_box.pack_start(&path_label, false, false, 0);

    let path_entry = Entry::new();
    path_entry.set_editable(false);
    path_entry.set_hexpand(true);
    path_entry.set_halign(gtk::Align::Fill);
    path_entry.set_placeholder_text(Some("Select an item to view its JSON path"));
    right_box.pack_start(&path_entry, false, false, 0);

    // Separator
    let separator = Separator::new(Orientation::Horizontal);
    right_box.pack_start(&separator, false, false, 6);

    // Value label and display
    let value_label = gtk::Label::new(Some("Value:"));
    value_label.set_halign(gtk::Align::Start);
    right_box.pack_start(&value_label, false, false, 0);

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
    paned.add1(&left_box);
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
            value_text_buffer_clone
                .set_text("Select an item in the tree to view its JSON path and value");
            path_entry_clone.set_placeholder_text(Some("Select an item to view its JSON path"));
        }
    });

    // Helper function to remove a root node
    let remove_root_node = |tree_store: &TreeStore,
                            selection: &gtk::TreeSelection,
                            path_entry: &Entry,
                            value_text_buffer: &TextBuffer| {
        if let Some((model, iter)) = selection.selected() {
            // Check if this is a root node (no parent)
            if model.iter_parent(&iter).is_none() {
                // This is a root node - remove it
                tree_store.remove(&iter);

                // Clear the display if we deleted the selected item
                path_entry.set_text("");
                value_text_buffer
                    .set_text("Select an item in the tree to view its JSON path and value");
                path_entry.set_placeholder_text(Some("Select an item to view its JSON path"));

                // Try to select the next root node if available
                if let Some(first_iter) = tree_store.iter_first() {
                    selection.select_iter(&first_iter);
                }
            }
        }
    };

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
                remove_root_node(
                    &tree_store_for_delete,
                    &selection,
                    &path_entry_for_delete,
                    &value_text_buffer_for_delete,
                );
                return gtk::glib::Propagation::Stop;
            }
        }
        gtk::glib::Propagation::Proceed
    });

    // Add context menu for removing files
    let tree_store_for_menu = tree_store.clone();
    let path_entry_for_menu = path_entry.clone();
    let value_text_buffer_for_menu = value_text_buffer.clone();
    let tree_view_for_menu = tree_view.clone();
    tree_view.connect_button_press_event(move |tree_view, event| {
        // Check for right-click (button 3)
        if event.button() == 3 {
            let (x, y) = event.position();
            // Get the path at the click position
            if let Some((Some(path), _, _, _)) = tree_view.path_at_pos(x as i32, y as i32) {
                let selection = tree_view.selection();
                selection.select_path(&path);

                // Check if a root node is selected before showing menu
                if let Some((model, iter)) = selection.selected() {
                    if model.iter_parent(&iter).is_none() {
                        // Create context menu
                        let menu = Menu::new();
                        let remove_item = MenuItem::with_label("Remove File");
                        let tree_store_clone = tree_store_for_menu.clone();
                        let path_entry_clone = path_entry_for_menu.clone();
                        let value_text_buffer_clone = value_text_buffer_for_menu.clone();
                        let tree_view_clone = tree_view_for_menu.clone();
                        remove_item.connect_activate(move |_| {
                            let selection = tree_view_clone.selection();
                            remove_root_node(
                                &tree_store_clone,
                                &selection,
                                &path_entry_clone,
                                &value_text_buffer_clone,
                            );
                        });

                        menu.append(&remove_item);
                        remove_item.show_all();
                        menu.popup_at_pointer(Some(event));
                        return gtk::glib::Propagation::Stop;
                    }
                }
            }
        }
        gtk::glib::Propagation::Proceed
    });

    // Create menu bar
    let menu_bar = MenuBar::new();

    // File menu
    let file_menu = Menu::new();
    let file_menu_item = MenuItem::with_label("File");
    file_menu_item.set_submenu(Some(&file_menu));

    // Open menu item
    let open_menu_item = MenuItem::with_label("Open");
    let tree_store_for_open = tree_store.clone();
    let value_text_buffer_for_open = value_text_buffer.clone();
    let window_clone = window.clone();

    open_menu_item.connect_activate(move |_| {
        let tree_store_clone = tree_store_for_open.clone();
        let value_text_buffer_clone = value_text_buffer_for_open.clone();
        let window_clone2 = window_clone.clone();

        let dialog = FileChooserDialog::new(
            Some("Open File"),
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

    file_menu.append(&open_menu_item);

    // Edit menu
    let edit_menu = Menu::new();
    let edit_menu_item = MenuItem::with_label("Edit");
    edit_menu_item.set_submenu(Some(&edit_menu));

    // Paste menu item
    let paste_menu_item = MenuItem::with_label("Paste");
    let tree_store_for_clipboard = tree_store.clone();
    let value_text_buffer_for_clipboard = value_text_buffer.clone();

    paste_menu_item.connect_activate(move |_| {
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

    // Copy menu item
    let copy_menu_item = MenuItem::with_label("Copy");
    let selection_for_copy = selection.clone();
    let value_text_buffer_for_copy = value_text_buffer.clone();

    copy_menu_item.connect_activate(move |_| {
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

    // Remove File menu item
    let remove_file_menu_item = MenuItem::with_label("Remove File");
    let tree_store_for_remove = tree_store.clone();
    let selection_for_remove = selection.clone();
    let path_entry_for_remove = path_entry.clone();
    let value_text_buffer_for_remove = value_text_buffer.clone();

    remove_file_menu_item.connect_activate(move |_| {
        let selection = selection_for_remove.clone();
        if let Some((model, iter)) = selection.selected() {
            // Check if this is a root node (no parent)
            if model.iter_parent(&iter).is_none() {
                // This is a root node - remove it
                tree_store_for_remove.remove(&iter);

                // Clear the display if we deleted the selected item
                path_entry_for_remove.set_text("");
                value_text_buffer_for_remove
                    .set_text("Select an item in the tree to view its JSON path and value");
                path_entry_for_remove
                    .set_placeholder_text(Some("Select an item to view its JSON path"));

                // Try to select the next root node if available
                if let Some(first_iter) = tree_store_for_remove.iter_first() {
                    selection.select_iter(&first_iter);
                }
            }
        }
    });

    edit_menu.append(&paste_menu_item);
    edit_menu.append(&copy_menu_item);
    edit_menu.append(&remove_file_menu_item);

    // Add menus to menu bar
    menu_bar.append(&file_menu_item);
    menu_bar.append(&edit_menu_item);

    // Create main container with menu bar and paned
    let main_box = GtkBox::new(Orientation::Vertical, 0);
    main_box.pack_start(&menu_bar, false, false, 0);
    main_box.pack_start(&paned, true, true, 0);

    window.add(&main_box);
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
fn add_single_value_to_tree(tree_store: &TreeStore, value: &Value, root_name: &str) {
    let root_iter = tree_store.append(None);
    // Use "$" as the root path for single objects/arrays (JSONPath notation)
    // This ensures proper path generation for nested structures
    let root_path = "$";
    set_tree_node_values(tree_store, &root_iter, root_name, value, root_path);
    populate_tree(tree_store, &root_iter, value, root_path);
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
    load_parse_result(
        result,
        &display_name,
        tree_store,
        value_text_buffer,
        "Error parsing file",
    );
}

fn load_json_content(
    content: &str,
    name: Option<&str>,
    tree_store: &TreeStore,
    value_text_buffer: &TextBuffer,
) {
    let display_name = name.unwrap_or("Content");
    let result = parse_text_content(content);
    load_parse_result(
        result,
        &display_name,
        tree_store,
        value_text_buffer,
        "Error parsing content",
    );
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

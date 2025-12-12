use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Paned, ScrolledWindow, TreeView, TreeStore, TreeViewColumn,
    CellRendererText, TextView, TextBuffer, FileChooserDialog, FileChooserAction,
    ResponseType, Clipboard, Entry, Box as GtkBox, Separator, Orientation,
};
use serde_json::Value;
use std::fs;
use std::path::Path;

fn main() {
    // Read command-line arguments before GTK initialization
    let file_path = std::env::args().nth(1);
    
    let app = Application::builder()
        .application_id("com.example.viewjson")
        .build();

    let file_path_clone = file_path.clone();
    app.connect_activate(move |app| {
        build_ui(app, file_path_clone.as_deref());
    });

    // Run with empty args to prevent GTK from trying to handle file arguments
    app.run_with_args(&[] as &[&str]);
}

fn build_ui(app: &Application, initial_file: Option<&str>) {
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
    
    let open_button = gtk::Button::with_label("Open File");
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
                        let name = path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("Unknown")
                            .to_string();
                        load_json_content_from_file(&path, Some(&name), &tree_store_clone, &value_text_buffer_clone);
                    }
                }
            }
            dialog.close();
        });
        
        dialog.show();
    });
    
    // Add clipboard button
    let clipboard_button = gtk::Button::with_label("From Clipboard");
    let tree_store_for_clipboard = tree_store.clone();
    let value_text_buffer_for_clipboard = value_text_buffer.clone();
    
    clipboard_button.connect_clicked(move |_| {
        let tree_store_clone = tree_store_for_clipboard.clone();
        let value_text_buffer_clone = value_text_buffer_for_clipboard.clone();
        
        let clipboard = Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD);
        clipboard.request_text(move |_clipboard, text| {
            if let Some(content) = text {
                load_json_content(&content, Some("Clipboard"), &tree_store_clone, &value_text_buffer_clone);
            } else {
                value_text_buffer_clone.set_text("Clipboard is empty or does not contain text");
            }
        });
    });
    
    header_bar.pack_start(&open_button);
    header_bar.pack_start(&clipboard_button);
    window.set_titlebar(Some(&header_bar));
    
    window.add(&paned);
    window.show_all();
    
    // Try to load from command line argument
    if let Some(file_path) = initial_file {
        let path = Path::new(file_path);
        if path.exists() {
            let name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown")
                .to_string();
            load_json_content_from_file(path, Some(&name), &tree_store, &value_text_buffer);
        }
    }
}

fn load_json_content_from_file(path: &Path, name: Option<&str>, tree_store: &TreeStore, value_text_buffer: &TextBuffer) {
    // Read file
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            value_text_buffer.set_text(&format!("Error reading file: {}", e));
            return;
        }
    };
    
    let display_name = name.unwrap_or("File");
    load_json_content(&content, Some(display_name), tree_store, value_text_buffer);
}

fn load_json_content(content: &str, name: Option<&str>, tree_store: &TreeStore, value_text_buffer: &TextBuffer) {
    // Try to parse as JSONL first (multiple JSON objects, one per line)
    let lines: Vec<&str> = content.lines().collect();
    let mut json_values: Vec<Value> = Vec::new();
    
    if lines.len() > 1 {
        // Try JSONL format
        for (_idx, line) in lines.iter().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<Value>(line) {
                Ok(value) => json_values.push(value),
                Err(_) => {
                    // If JSONL parsing fails, try as single JSON
                    json_values.clear();
                    break;
                }
            }
        }
        
        if !json_values.is_empty() {
            // JSONL format - create root node for each line
            let root_iter = tree_store.append(None);
            let root_json = serde_json::json!({ "lines": json_values.len() });
            let root_name = format!("{} (JSONL)", name.unwrap_or("Content"));
            tree_store.set_value(&root_iter, 0, &root_name.to_value());
            tree_store.set_value(&root_iter, 1, &format!("{} objects", json_values.len()).to_value());
            let root_path = format!("{}", name.unwrap_or("root"));
            tree_store.set_value(&root_iter, 2, &root_path.to_value());
            tree_store.set_value(&root_iter, 3, &serde_json::to_string(&root_json).unwrap_or_default().to_value());
            
            for (idx, value) in json_values.iter().enumerate() {
                let line_iter = tree_store.append(Some(&root_iter));
                let path = format!("{}[{}]", root_path, idx);
                tree_store.set_value(&line_iter, 0, &format!("Line {}", idx + 1).to_value());
                tree_store.set_value(&line_iter, 1, &format_value_preview(value).to_value());
                tree_store.set_value(&line_iter, 2, &path.to_value());
                tree_store.set_value(&line_iter, 3, &serde_json::to_string(value).unwrap_or_default().to_value());
                populate_tree(&tree_store, &line_iter, value, &path);
            }
            return;
        }
    }
    
    // Try to parse as single JSON
    match serde_json::from_str::<Value>(&content) {
        Ok(value) => {
            let root_iter = tree_store.append(None);
            let root_name = name.unwrap_or("root");
            tree_store.set_value(&root_iter, 0, &root_name.to_value());
            tree_store.set_value(&root_iter, 1, &format_value_preview(&value).to_value());
            tree_store.set_value(&root_iter, 2, &root_name.to_value());
            tree_store.set_value(&root_iter, 3, &serde_json::to_string(&value).unwrap_or_default().to_value());
            populate_tree(tree_store, &root_iter, &value, root_name);
        }
        Err(e) => {
            value_text_buffer.set_text(&format!("Error parsing JSON: {}", e));
        }
    }
}

fn populate_tree(tree_store: &TreeStore, parent: &gtk::TreeIter, value: &Value, path: &str) {
    match value {
        Value::Object(map) => {
            for (key, val) in map.iter() {
                let iter = tree_store.append(Some(parent));
                let new_path = format!("{}.{}", path, key);
                tree_store.set_value(&iter, 0, &key.to_value());
                tree_store.set_value(&iter, 1, &format_value_preview(val).to_value());
                tree_store.set_value(&iter, 2, &new_path.to_value());
                tree_store.set_value(&iter, 3, &serde_json::to_string(val).unwrap_or_default().to_value());
                populate_tree(tree_store, &iter, val, &new_path);
            }
        }
        Value::Array(arr) => {
            for (idx, val) in arr.iter().enumerate() {
                let iter = tree_store.append(Some(parent));
                let new_path = format!("{}[{}]", path, idx);
                tree_store.set_value(&iter, 0, &format!("[{}]", idx).to_value());
                tree_store.set_value(&iter, 1, &format_value_preview(val).to_value());
                tree_store.set_value(&iter, 2, &new_path.to_value());
                tree_store.set_value(&iter, 3, &serde_json::to_string(val).unwrap_or_default().to_value());
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

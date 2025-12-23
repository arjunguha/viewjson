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
mod search;
mod tree_builder;

use gtk::prelude::*;
use gtk::{
    AccelGroup, Application, ApplicationWindow, Box as GtkBox, Button, CellRendererText,
    CheckButton, Clipboard, Entry, FileChooserAction, FileChooserDialog, Menu, MenuBar, MenuItem,
    Orientation, Paned, ResponseType, ScrolledWindow, Separator, TextBuffer, TextView, TreeStore,
    TreeView, TreeViewColumn,
};
use search::{find_all_occurrences, find_occurrence_to_highlight};
use slopjson::json_reader::{parse_file, parse_text_content, ParseResult};
use slopjson::value_formatting::format_value_from_string;
use std::path::Path;
use tree_builder::{add_jsonl_to_tree, add_single_value_to_tree};

fn main() {
    // Read command-line arguments before GTK initialization
    // Collect all arguments after the program name
    let file_paths: Vec<String> = std::env::args().skip(1).collect();

    let app = Application::builder()
        .application_id("com.example.slopjson")
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
        .title("slopjson")
        .default_width(1200)
        .default_height(800)
        .build();

    // Create accelerator group for keyboard shortcuts
    let accel_group = AccelGroup::new();
    window.add_accel_group(&accel_group);

    // Import for accelerator setup
    use gtk::gdk::{keys::constants as keys, ModifierType};

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

    // We'll update this in the selection handler
    let remove_file_menu_item_for_selection =
        std::rc::Rc::new(std::cell::RefCell::new(None::<MenuItem>));

    selection.connect_changed({
        let remove_file_menu_item_for_selection = remove_file_menu_item_for_selection.clone();
        move |sel| {
            if let Some((model, iter)) = sel.selected() {
                let path = model.value(&iter, 2).get::<String>().unwrap_or_default();
                let full_value = model.value(&iter, 3).get::<String>().unwrap_or_default();

                // Set path in the entry
                path_entry_clone.set_text(&path);

                // Format the JSON value nicely
                let preview = model.value(&iter, 1).get::<String>().unwrap_or_default();
                let formatted_value = format_value_from_string(&full_value, &preview);

                value_text_buffer_clone.set_text(&formatted_value);

                // Enable/disable Remove File menu item based on whether a root node is selected
                if let Some(ref menu_item) = *remove_file_menu_item_for_selection.borrow() {
                    let is_root_node = model.iter_parent(&iter).is_none();
                    menu_item.set_sensitive(is_root_node);
                }
            } else {
                path_entry_clone.set_text("");
                value_text_buffer_clone
                    .set_text("Select an item in the tree to view its JSON path and value");
                path_entry_clone.set_placeholder_text(Some("Select an item to view its JSON path"));

                // Disable Remove File menu item when nothing is selected
                if let Some(ref menu_item) = *remove_file_menu_item_for_selection.borrow() {
                    menu_item.set_sensitive(false);
                }
            }
        }
    });

    // Helper function to remove a root node
    fn remove_root_node(
        tree_store: &TreeStore,
        selection: &gtk::TreeSelection,
        path_entry: &Entry,
        value_text_buffer: &TextBuffer,
    ) {
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
    }

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
    open_menu_item.add_accelerator(
        "activate",
        &accel_group,
        *keys::o as u32,
        ModifierType::CONTROL_MASK,
        gtk::AccelFlags::VISIBLE,
    );
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

    // Exit menu item
    let exit_menu_item = MenuItem::with_label("Exit");
    exit_menu_item.add_accelerator(
        "activate",
        &accel_group,
        *keys::F4 as u32,
        ModifierType::MOD1_MASK,
        gtk::AccelFlags::VISIBLE,
    );
    let window_for_exit = window.clone();
    exit_menu_item.connect_activate(move |_| {
        window_for_exit.close();
    });
    file_menu.append(&exit_menu_item);

    // Edit menu
    let edit_menu = Menu::new();
    let edit_menu_item = MenuItem::with_label("Edit");
    edit_menu_item.set_submenu(Some(&edit_menu));

    // Paste menu item
    let paste_menu_item = MenuItem::with_label("Paste");
    paste_menu_item.add_accelerator(
        "activate",
        &accel_group,
        *keys::v as u32,
        ModifierType::CONTROL_MASK,
        gtk::AccelFlags::VISIBLE,
    );
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
    copy_menu_item.add_accelerator(
        "activate",
        &accel_group,
        *keys::c as u32,
        ModifierType::CONTROL_MASK,
        gtk::AccelFlags::VISIBLE,
    );
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
    // Initially disable the menu item (no selection at startup)
    remove_file_menu_item.set_sensitive(false);

    // Add Delete key accelerator
    remove_file_menu_item.add_accelerator(
        "activate",
        &accel_group,
        *keys::Delete as u32,
        ModifierType::empty(),
        gtk::AccelFlags::VISIBLE,
    );

    // Store reference for selection handler
    *remove_file_menu_item_for_selection.borrow_mut() = Some(remove_file_menu_item.clone());

    let tree_store_for_remove = tree_store.clone();
    let selection_for_remove = selection.clone();
    let path_entry_for_remove = path_entry.clone();
    let value_text_buffer_for_remove = value_text_buffer.clone();

    remove_file_menu_item.connect_activate(move |_| {
        let selection = selection_for_remove.clone();
        remove_root_node(
            &tree_store_for_remove,
            &selection,
            &path_entry_for_remove,
            &value_text_buffer_for_remove,
        );
    });

    // Find menu item
    let find_menu_item = MenuItem::with_label("Find");
    find_menu_item.add_accelerator(
        "activate",
        &accel_group,
        *keys::f as u32,
        ModifierType::CONTROL_MASK,
        gtk::AccelFlags::VISIBLE,
    );

    edit_menu.append(&paste_menu_item);
    edit_menu.append(&copy_menu_item);
    edit_menu.append(&remove_file_menu_item);
    edit_menu.append(&find_menu_item);

    // Add menus to menu bar
    menu_bar.append(&file_menu_item);
    menu_bar.append(&edit_menu_item);

    // Create search toolbar (initially hidden)
    let search_toolbar = GtkBox::new(Orientation::Horizontal, 6);
    search_toolbar.set_margin_start(6);
    search_toolbar.set_margin_end(6);
    search_toolbar.set_margin_top(3);
    search_toolbar.set_margin_bottom(3);
    search_toolbar.set_visible(false);
    search_toolbar.set_no_show_all(true);

    let search_label = gtk::Label::new(Some("Find:"));
    search_toolbar.pack_start(&search_label, false, false, 0);

    let search_entry = Entry::new();
    search_entry.set_placeholder_text(Some("Search keys and values..."));
    search_entry.set_hexpand(true);
    search_toolbar.pack_start(&search_entry, true, true, 0);

    let case_sensitive_check = CheckButton::with_label("Case sensitive");
    search_toolbar.pack_start(&case_sensitive_check, false, false, 0);

    let prev_button = Button::with_label("Previous");
    search_toolbar.pack_start(&prev_button, false, false, 0);

    let next_button = Button::with_label("Next");
    search_toolbar.pack_start(&next_button, false, false, 0);

    let close_search_button = Button::with_label("Close");
    search_toolbar.pack_start(&close_search_button, false, false, 0);

    // Match information: tree path and character position within the value
    #[derive(Clone)]
    struct SearchMatch {
        path: gtk::TreePath,
        is_key_match: bool,
    }

    // Search state: store all matching occurrences and current index
    let search_matches: std::rc::Rc<std::cell::RefCell<Vec<SearchMatch>>> =
        std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let search_current_index: std::rc::Rc<std::cell::RefCell<Option<usize>>> =
        std::rc::Rc::new(std::cell::RefCell::new(None));
    let current_search_text: std::rc::Rc<std::cell::RefCell<String>> =
        std::rc::Rc::new(std::cell::RefCell::new(String::new()));
    let current_case_sensitive: std::rc::Rc<std::cell::RefCell<bool>> =
        std::rc::Rc::new(std::cell::RefCell::new(false));

    // Function to perform search
    let perform_search = |tree_store: &TreeStore,
                          search_text: &str,
                          case_sensitive: bool,
                          search_matches: &std::rc::Rc<std::cell::RefCell<Vec<SearchMatch>>>,
                          search_current_index: &std::rc::Rc<std::cell::RefCell<Option<usize>>>,
                          current_selection: Option<&gtk::TreePath>| {
        let mut matches = Vec::new();
        let search_text_lower = if case_sensitive {
            search_text.to_string()
        } else {
            search_text.to_lowercase()
        };

        // Recursively search through tree - only search leaf nodes
        fn search_tree(
            tree_store: &TreeStore,
            iter: &gtk::TreeIter,
            search_text: &str,
            search_text_lower: &str,
            case_sensitive: bool,
            matches: &mut Vec<SearchMatch>,
        ) {
            // Check if this node has children
            let has_children = tree_store.iter_children(Some(iter)).is_some();

            // Only search leaf nodes (nodes with no children)
            if !has_children {
                if let Some(path) = tree_store.path(iter) {
                    // Get key (column 0) and value (columns 1 and 3)
                    let key = tree_store
                        .value(iter, 0)
                        .get::<String>()
                        .unwrap_or_default();
                    let value_preview = tree_store
                        .value(iter, 1)
                        .get::<String>()
                        .unwrap_or_default();
                    let full_value = tree_store
                        .value(iter, 3)
                        .get::<String>()
                        .unwrap_or_default();

                    // Find all occurrences in the key
                    let key_occurrences = find_all_occurrences(&key, search_text, case_sensitive);
                    for (_start, _end) in key_occurrences {
                        matches.push(SearchMatch {
                            path: path.clone(),
                            is_key_match: true,
                        });
                    }

                    // Find all occurrences in the value
                    // We need to format it the same way as we display it, so offsets match
                    let value_to_search = format_value_from_string(&full_value, &value_preview);
                    let value_occurrences =
                        find_all_occurrences(&value_to_search, search_text, case_sensitive);
                    for (_start, _end) in value_occurrences {
                        matches.push(SearchMatch {
                            path: path.clone(),
                            is_key_match: false,
                        });
                    }
                }
            } else {
                // This node has children, so recursively search children
                if let Some(mut child_iter) = tree_store.iter_children(Some(iter)) {
                    loop {
                        search_tree(
                            tree_store,
                            &child_iter,
                            search_text,
                            search_text_lower,
                            case_sensitive,
                            matches,
                        );
                        if !tree_store.iter_next(&mut child_iter) {
                            break;
                        }
                    }
                }
            }
        }

        // Search from each root node
        if let Some(mut root_iter) = tree_store.iter_first() {
            loop {
                search_tree(
                    tree_store,
                    &root_iter,
                    search_text,
                    &search_text_lower,
                    case_sensitive,
                    &mut matches,
                );
                if !tree_store.iter_next(&mut root_iter) {
                    break;
                }
            }
        }

        let is_empty = matches.is_empty();

        // Find the starting index based on current selection (before moving matches)
        let starting_index = if is_empty {
            None
        } else if let Some(current_path) = current_selection {
            // Find the first match at or after the current selection
            // Compare paths by depth and indices lexicographically
            let current_depth = current_path.depth();
            let current_indices: Vec<i32> = current_path.indices().to_vec();

            matches
                .iter()
                .position(|m| {
                    let match_depth = m.path.depth();
                    let match_indices: Vec<i32> = m.path.indices().to_vec();

                    // Compare lexicographically: first by depth, then by indices
                    if current_depth < match_depth {
                        true
                    } else if current_depth > match_depth {
                        false
                    } else {
                        // Same depth, compare indices lexicographically
                        current_indices <= match_indices
                    }
                })
                .or(Some(0)) // If no match found after current position, start from beginning
        } else {
            Some(0) // No current selection, start from beginning
        };

        *search_matches.borrow_mut() = matches;
        *search_current_index.borrow_mut() = starting_index;
    };

    // Function to navigate to search result and highlight the occurrence
    let navigate_to_match = |tree_view: &TreeView,
                             selection: &gtk::TreeSelection,
                             tree_store: &TreeStore,
                             value_text_buffer: &TextBuffer,
                             value_text_view: &TextView,
                             matches: &[SearchMatch],
                             index: Option<usize>,
                             search_text: &str,
                             case_sensitive: bool| {
        if let Some(idx) = index {
            if idx < matches.len() {
                let search_match = &matches[idx];
                let path = &search_match.path;

                // Expand all parent nodes to show the path to the leaf
                // We need to expand from root to leaf, so we'll go up from the leaf
                // and expand each parent path
                let mut parent_path = path.clone();
                let mut paths_to_expand = Vec::new();

                // Collect all parent paths (from root to leaf)
                while parent_path.up() {
                    paths_to_expand.push(parent_path.clone());
                }

                // Expand from root to leaf (reverse order)
                for expand_path in paths_to_expand.iter().rev() {
                    tree_view.expand_row(expand_path, false);
                }

                // Select the tree node (leaf)
                selection.select_path(path);
                tree_view.scroll_to_cell(Some(path), None::<&TreeViewColumn>, false, 0.0, 0.0);

                // Get the iter for the selected path to get the value
                if let Some(iter) = tree_store.iter(path) {
                    let full_value = tree_store
                        .value(&iter, 3)
                        .get::<String>()
                        .unwrap_or_default();

                    // Format the JSON value nicely - must match the formatting used during search
                    let preview = tree_store
                        .value(&iter, 1)
                        .get::<String>()
                        .unwrap_or_default();
                    let formatted_value = format_value_from_string(&full_value, &preview);

                    // Set the text in the buffer
                    value_text_buffer.set_text(&formatted_value);

                    // Highlight the occurrence if it's a value match
                    if !search_match.is_key_match {
                        // Create or get the highlight tag
                        let tag_table = value_text_buffer.tag_table();
                        if let Some(ref table) = tag_table {
                            let highlight_tag = if let Some(tag) = table.lookup("search-highlight")
                            {
                                tag
                            } else {
                                let tag = gtk::TextTag::new(Some("search-highlight"));
                                tag.set_property("background", &"yellow");
                                tag.set_property("foreground", &"black");
                                table.add(&tag);
                                tag
                            };

                            // Remove any existing highlights
                            let mut start_iter = value_text_buffer.start_iter();
                            let mut end_iter = value_text_buffer.end_iter();
                            value_text_buffer.remove_tag(
                                &highlight_tag,
                                &mut start_iter,
                                &mut end_iter,
                            );

                            // Build list of matches for this same path (for use with find_occurrence_to_highlight)
                            // This contains all matches (both key and value) for the current path, in order
                            let mut path_matches: Vec<(usize, bool)> = Vec::new();
                            for (i, m) in matches.iter().enumerate() {
                                if m.path == *path {
                                    path_matches.push((i, m.is_key_match));
                                }
                            }

                            // Find which occurrence to highlight using the abstracted function
                            // This will find the correct occurrence within the formatted_value
                            // Only proceed if the current match is in path_matches
                            if path_matches
                                .iter()
                                .any(|(global_idx, _)| *global_idx == idx)
                            {
                                if let Some((start, end)) = find_occurrence_to_highlight(
                                    &path_matches,
                                    idx,
                                    &formatted_value,
                                    search_text,
                                    case_sensitive,
                                ) {
                                    // `start`/`end` are character offsets (not bytes). GTK expects character offsets.
                                    let formatted_chars = formatted_value.chars().count();

                                    // Verify the positions are valid (in character offsets)
                                    if start <= formatted_chars
                                        && end <= formatted_chars
                                        && start < end
                                    {
                                        let mut start_iter =
                                            value_text_buffer.iter_at_offset(start as i32);
                                        let mut end_iter =
                                            value_text_buffer.iter_at_offset(end as i32);
                                        value_text_buffer.apply_tag(
                                            &highlight_tag,
                                            &mut start_iter,
                                            &mut end_iter,
                                        );

                                        // Create a mark at the start position and scroll to it
                                        // This is more reliable than scroll_to_iter
                                        if let Some(mark) = value_text_buffer.create_mark(
                                            Some("search-scroll-mark"),
                                            &mut start_iter,
                                            true, // left_gravity
                                        ) {
                                            // Scroll to make the mark visible using idle to ensure it happens after layout
                                            let value_text_view_clone = value_text_view.clone();
                                            let mark_clone = mark.clone();
                                            let value_text_buffer_clone = value_text_buffer.clone();
                                            glib::idle_add_local(move || {
                                                value_text_view_clone
                                                    .scroll_mark_onscreen(&mark_clone);
                                                // Clean up the mark after scrolling
                                                if let Some(mark) = value_text_buffer_clone
                                                    .mark(&"search-scroll-mark")
                                                {
                                                    value_text_buffer_clone.delete_mark(&mark);
                                                }
                                                glib::ControlFlow::Break
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    // Connect search entry changes
    let tree_store_for_search = tree_store.clone();
    let tree_view_for_search = tree_view.clone();
    let selection_for_search = selection.clone();
    let value_text_buffer_for_search = value_text_buffer.clone();
    let value_text_view_for_search = value_text_view.clone();
    let search_matches_clone = search_matches.clone();
    let search_current_index_clone = search_current_index.clone();
    let current_search_text_clone = current_search_text.clone();
    let current_case_sensitive_clone = current_case_sensitive.clone();
    let case_sensitive_check_clone = case_sensitive_check.clone();
    let prev_button_clone = prev_button.clone();
    let next_button_clone = next_button.clone();

    search_entry.connect_changed({
        let tree_store_clone = tree_store_for_search.clone();
        let tree_view_clone = tree_view_for_search.clone();
        let selection_clone = selection_for_search.clone();
        let value_text_buffer_clone = value_text_buffer_for_search.clone();
        let value_text_view_clone = value_text_view_for_search.clone();
        let search_matches_clone2 = search_matches_clone.clone();
        let search_current_index_clone2 = search_current_index_clone.clone();
        let current_search_text_clone2 = current_search_text_clone.clone();
        let current_case_sensitive_clone2 = current_case_sensitive_clone.clone();
        let case_sensitive_check_clone2 = case_sensitive_check_clone.clone();
        let prev_button_clone2 = prev_button_clone.clone();
        let next_button_clone2 = next_button_clone.clone();
        move |entry| {
            let search_text = entry.text().to_string();
            if search_text.is_empty() {
                *search_matches_clone2.borrow_mut() = Vec::new();
                *search_current_index_clone2.borrow_mut() = None;
                prev_button_clone2.set_sensitive(false);
                next_button_clone2.set_sensitive(false);
                return;
            }

            let case_sensitive = case_sensitive_check_clone2.is_active();
            *current_search_text_clone2.borrow_mut() = search_text.clone();
            *current_case_sensitive_clone2.borrow_mut() = case_sensitive;

            // Get current selection path to start search from current view position
            let current_path = selection_clone
                .selected()
                .and_then(|(_model, iter)| tree_store_clone.path(&iter));

            perform_search(
                &tree_store_clone,
                &search_text,
                case_sensitive,
                &search_matches_clone2,
                &search_current_index_clone2,
                current_path.as_ref(),
            );

            let matches = search_matches_clone2.borrow();
            let has_matches = !matches.is_empty();
            prev_button_clone2.set_sensitive(has_matches);
            next_button_clone2.set_sensitive(has_matches);

            if has_matches {
                let current_idx = search_current_index_clone2.borrow();
                navigate_to_match(
                    &tree_view_clone,
                    &selection_clone,
                    &tree_store_clone,
                    &value_text_buffer_clone,
                    &value_text_view_clone,
                    &matches,
                    *current_idx,
                    &search_text,
                    case_sensitive,
                );
            }
        }
    });

    // Connect case sensitivity checkbox
    let tree_store_for_case = tree_store_for_search.clone();
    let tree_view_for_case = tree_view_for_search.clone();
    let selection_for_case = selection_for_search.clone();
    let value_text_buffer_for_case = value_text_buffer_for_search.clone();
    let value_text_view_for_case = value_text_view_for_search.clone();
    let search_entry_for_case = search_entry.clone();
    let search_matches_for_case = search_matches_clone.clone();
    let search_current_index_for_case = search_current_index_clone.clone();
    let current_search_text_for_case = current_search_text_clone.clone();
    let current_case_sensitive_for_case = current_case_sensitive_clone.clone();
    let prev_button_for_case = prev_button_clone.clone();
    let next_button_for_case = next_button_clone.clone();

    case_sensitive_check.connect_toggled({
        let tree_store_clone = tree_store_for_case.clone();
        let tree_view_clone = tree_view_for_case.clone();
        let selection_clone = selection_for_case.clone();
        let value_text_buffer_clone = value_text_buffer_for_case.clone();
        let value_text_view_clone = value_text_view_for_case.clone();
        let search_entry_clone = search_entry_for_case.clone();
        let search_matches_clone2 = search_matches_for_case.clone();
        let search_current_index_clone2 = search_current_index_for_case.clone();
        let current_search_text_clone2 = current_search_text_for_case.clone();
        let current_case_sensitive_clone2 = current_case_sensitive_for_case.clone();
        let case_sensitive_check_clone = case_sensitive_check.clone();
        let prev_button_clone2 = prev_button_for_case.clone();
        let next_button_clone2 = next_button_for_case.clone();
        move |_| {
            let search_text = search_entry_clone.text().to_string();
            if search_text.is_empty() {
                return;
            }

            let case_sensitive = case_sensitive_check_clone.is_active();
            *current_search_text_clone2.borrow_mut() = search_text.clone();
            *current_case_sensitive_clone2.borrow_mut() = case_sensitive;

            // Get current selection path to start search from current view position
            let current_path = selection_clone
                .selected()
                .and_then(|(_model, iter)| tree_store_clone.path(&iter));

            perform_search(
                &tree_store_clone,
                &search_text,
                case_sensitive,
                &search_matches_clone2,
                &search_current_index_clone2,
                current_path.as_ref(),
            );

            let matches = search_matches_clone2.borrow();
            let has_matches = !matches.is_empty();
            prev_button_clone2.set_sensitive(has_matches);
            next_button_clone2.set_sensitive(has_matches);

            if has_matches {
                let current_idx = search_current_index_clone2.borrow();
                navigate_to_match(
                    &tree_view_clone,
                    &selection_clone,
                    &tree_store_clone,
                    &value_text_buffer_clone,
                    &value_text_view_clone,
                    &matches,
                    *current_idx,
                    &search_text,
                    case_sensitive,
                );
            }
        }
    });

    // Connect Previous button
    let tree_store_for_prev = tree_store_for_search.clone();
    let tree_view_for_prev = tree_view_for_search.clone();
    let selection_for_prev = selection_for_search.clone();
    let value_text_buffer_for_prev = value_text_buffer_for_search.clone();
    let value_text_view_for_prev = value_text_view_for_search.clone();
    let search_matches_for_prev = search_matches_clone.clone();
    let search_current_index_for_prev = search_current_index_clone.clone();
    let current_search_text_for_prev = current_search_text_clone.clone();
    let current_case_sensitive_for_prev = current_case_sensitive_clone.clone();

    prev_button.connect_clicked({
        let tree_store_clone = tree_store_for_prev.clone();
        let tree_view_clone = tree_view_for_prev.clone();
        let selection_clone = selection_for_prev.clone();
        let value_text_buffer_clone = value_text_buffer_for_prev.clone();
        let value_text_view_clone = value_text_view_for_prev.clone();
        let search_matches_clone2 = search_matches_for_prev.clone();
        let search_current_index_clone2 = search_current_index_for_prev.clone();
        let current_search_text_clone2 = current_search_text_for_prev.clone();
        let current_case_sensitive_clone2 = current_case_sensitive_for_prev.clone();
        move |_| {
            let matches = search_matches_clone2.borrow();
            if matches.is_empty() {
                return;
            }

            let mut current_idx = search_current_index_clone2.borrow_mut();
            if let Some(idx) = *current_idx {
                let new_idx = if idx == 0 { matches.len() - 1 } else { idx - 1 };
                *current_idx = Some(new_idx);
                let search_text = current_search_text_clone2.borrow().clone();
                let case_sensitive = *current_case_sensitive_clone2.borrow();
                navigate_to_match(
                    &tree_view_clone,
                    &selection_clone,
                    &tree_store_clone,
                    &value_text_buffer_clone,
                    &value_text_view_clone,
                    &matches,
                    Some(new_idx),
                    &search_text,
                    case_sensitive,
                );
            }
        }
    });

    // Connect Next button
    let tree_store_for_next = tree_store_for_search.clone();
    let tree_view_for_next = tree_view_for_search.clone();
    let selection_for_next = selection_for_search.clone();
    let value_text_buffer_for_next = value_text_buffer_for_search.clone();
    let value_text_view_for_next = value_text_view_for_search.clone();
    let search_matches_for_next = search_matches_clone.clone();
    let search_current_index_for_next = search_current_index_clone.clone();
    let current_search_text_for_next = current_search_text_clone.clone();
    let current_case_sensitive_for_next = current_case_sensitive_clone.clone();

    next_button.connect_clicked({
        let tree_store_clone = tree_store_for_next.clone();
        let tree_view_clone = tree_view_for_next.clone();
        let selection_clone = selection_for_next.clone();
        let value_text_buffer_clone = value_text_buffer_for_next.clone();
        let value_text_view_clone = value_text_view_for_next.clone();
        let search_matches_clone2 = search_matches_for_next.clone();
        let search_current_index_clone2 = search_current_index_for_next.clone();
        let current_search_text_clone2 = current_search_text_for_next.clone();
        let current_case_sensitive_clone2 = current_case_sensitive_for_next.clone();
        move |_| {
            let matches = search_matches_clone2.borrow();
            if matches.is_empty() {
                return;
            }

            let mut current_idx = search_current_index_clone2.borrow_mut();
            if let Some(idx) = *current_idx {
                let new_idx = if idx == matches.len() - 1 { 0 } else { idx + 1 };
                *current_idx = Some(new_idx);
                let search_text = current_search_text_clone2.borrow().clone();
                let case_sensitive = *current_case_sensitive_clone2.borrow();
                navigate_to_match(
                    &tree_view_clone,
                    &selection_clone,
                    &tree_store_clone,
                    &value_text_buffer_clone,
                    &value_text_view_clone,
                    &matches,
                    Some(new_idx),
                    &search_text,
                    case_sensitive,
                );
            }
        }
    });

    // Connect Enter key in search entry for next match
    let tree_store_for_enter = tree_store_for_search.clone();
    let tree_view_for_enter = tree_view_for_search.clone();
    let selection_for_enter = selection_for_search.clone();
    let value_text_buffer_for_enter = value_text_buffer_for_search.clone();
    let value_text_view_for_enter = value_text_view_for_search.clone();
    let search_matches_for_enter = search_matches_clone.clone();
    let search_current_index_for_enter = search_current_index_clone.clone();
    let current_search_text_for_enter = current_search_text_clone.clone();
    let current_case_sensitive_for_enter = current_case_sensitive_clone.clone();
    search_entry.connect_activate({
        let tree_store_clone = tree_store_for_enter.clone();
        let tree_view_clone = tree_view_for_enter.clone();
        let selection_clone = selection_for_enter.clone();
        let value_text_buffer_clone = value_text_buffer_for_enter.clone();
        let value_text_view_clone = value_text_view_for_enter.clone();
        let search_matches_clone2 = search_matches_for_enter.clone();
        let search_current_index_clone2 = search_current_index_for_enter.clone();
        let current_search_text_clone2 = current_search_text_for_enter.clone();
        let current_case_sensitive_clone2 = current_case_sensitive_for_enter.clone();
        move |_| {
            let matches = search_matches_clone2.borrow();
            if matches.is_empty() {
                return;
            }

            let mut current_idx = search_current_index_clone2.borrow_mut();
            if let Some(idx) = *current_idx {
                let new_idx = if idx == matches.len() - 1 { 0 } else { idx + 1 };
                *current_idx = Some(new_idx);
                let search_text = current_search_text_clone2.borrow().clone();
                let case_sensitive = *current_case_sensitive_clone2.borrow();
                navigate_to_match(
                    &tree_view_clone,
                    &selection_clone,
                    &tree_store_clone,
                    &value_text_buffer_clone,
                    &value_text_view_clone,
                    &matches,
                    Some(new_idx),
                    &search_text,
                    case_sensitive,
                );
            }
        }
    });

    // Connect Close button
    let search_toolbar_for_close = search_toolbar.clone();
    let search_entry_for_close = search_entry.clone();
    close_search_button.connect_clicked(move |_| {
        search_toolbar_for_close.set_visible(false);
        search_toolbar_for_close.set_no_show_all(true);
        search_entry_for_close.set_text("");
    });

    // Connect Find menu item
    let search_toolbar_for_menu = search_toolbar.clone();
    let search_entry_for_menu = search_entry.clone();
    find_menu_item.connect_activate(move |_| {
        search_toolbar_for_menu.set_no_show_all(false);
        search_toolbar_for_menu.set_visible(true);
        search_toolbar_for_menu.show_all();
        // Use GLib idle to ensure focus happens after widget is shown
        let search_entry_clone = search_entry_for_menu.clone();
        glib::idle_add_local(move || {
            search_entry_clone.grab_focus();
            glib::ControlFlow::Break
        });
    });

    // Create main container with menu bar, search toolbar, and paned
    let main_box = GtkBox::new(Orientation::Vertical, 0);
    main_box.pack_start(&menu_bar, false, false, 0);
    main_box.pack_start(&search_toolbar, false, false, 0);
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

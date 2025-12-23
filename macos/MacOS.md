# macOS Conversion Notes

## Rust crate tips
- Gate Linux UI dependencies behind a Cargo feature (`gtk-app`) so the reusable library builds with `--no-default-features` and never tries to link against GTK. This keeps us from installing GTK via Homebrew, which is explicitly forbidden.
- Expose a `staticlib`/`cdylib` by adding an `ffi` module and a `TreeNode` model so Swift can consume a ready-to-render tree instead of rebuilding JSON shape logic.
- Always build the Rust library with `cargo build --release --lib --no-default-features` before invoking Xcode; this produces `target/release/libslopjson.a` without pulling in GTK system packages.

## Xcode integration tips
- Keep a bridging header (`slopjson/slopjson-Bridging-Header.h`) with the three exported C symbols (`slopjson_parse_file`, `slopjson_parse_text`, `slopjson_string_free`). Only this header is needed on the Swift side.
- Add a Run Script build phase that calls `cargo build --release --lib --no-default-features` so the static library is always up to date, and point `LIBRARY_SEARCH_PATHS` at `$(PROJECT_DIR)/../target/release` with `OTHER_LDFLAGS = -lslopjson`.
- Set `SWIFT_OBJC_BRIDGING_HEADER` and `MACOSX_DEPLOYMENT_TARGET = 14.0` inside the target build settings to avoid mismatched SDK defaults that Xcode 16 templates currently produce.

## UI implementation tips
- Use `NavigationSplitView` with an `OutlineGroup` to mirror the GTK tree and keep selection syncing via a `@Published` `selectedNodeID`.
- Leverage `NSOpenPanel` for multi-file import and `NSPasteboard` for clipboard workflows so the SwiftUI view model can mirror the Linux “Open” and “Paste” commands.
- Keep search entirely in Swift by traversing the Rust-provided tree and tracking matches; this avoids round-tripping to Rust while still supporting next/previous navigation and case sensitivity.

# viewjson

A graphical JSON viewer for Linux with support for JSON, JSONL, YAML, and Parquet files.

## Installation

### Build from Source

```bash
cargo build --release
```

The binary will be available at `target/release/viewjson`.

### Build AppImage

```bash
cd appimage
./build-appimage.sh
```

This creates `viewjson-x86_64.AppImage` in the project root. Make it executable and run it:

```bash
chmod +x viewjson-x86_64.AppImage
./viewjson-x86_64.AppImage
```

## Usage

Open one or more JSON files:

```bash
viewjson file1.json file2.json
```

## Supported Formats

- **JSON**: Standard JSON files
- **JSONL**: Newline-delimited JSON (one JSON object per line)
- **YAML**: YAML files (converted to JSON for viewing)
- **Parquet**: Parquet files (read as JSON)

## License

GPL-3.0 or later


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

use serde_json::Value;
use std::fmt;
use std::path::Path;

/// Result of parsing JSON/JSONL content
#[derive(Debug, PartialEq)]
pub enum ParseResult {
    /// Single JSON object/array/value
    Single(Value),
    /// JSONL format - multiple JSON objects, one per line
    JsonL(Vec<Value>),
}

/// Errors that can occur during JSON/JSONL/YAML/Parquet parsing
#[derive(Debug, PartialEq)]
pub enum ParseError {
    /// Failed to parse as JSON or JSONL
    InvalidJson(String),
    /// Failed to parse as YAML
    InvalidYaml(String),
    /// Failed to parse as Parquet
    InvalidParquet(String),
    /// File I/O error
    IoError(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::InvalidJson(msg) => write!(f, "Invalid JSON: {}", msg),
            ParseError::InvalidYaml(msg) => write!(f, "Invalid YAML: {}", msg),
            ParseError::InvalidParquet(msg) => write!(f, "Invalid Parquet: {}", msg),
            ParseError::IoError(msg) => write!(f, "I/O error: {}", msg),
        }
    }
}

/// Parses content as JSON or JSONL format.
///
/// First attempts to parse as JSONL (multiple JSON objects, one per line).
/// If that fails, falls back to parsing as a single JSON document.
///
/// # Arguments
///
/// * `content` - The string content to parse
///
/// # Returns
///
/// * `Ok(ParseResult::JsonL(_))` if content is valid JSONL
/// * `Ok(ParseResult::Single(_))` if content is valid single JSON
/// * `Err(ParseError::InvalidJson(_))` if content is neither valid JSON nor JSONL
pub fn parse_json_content(content: &str) -> Result<ParseResult, ParseError> {
    let lines: Vec<&str> = content.lines().collect();

    // Try JSONL format if there are multiple lines
    if lines.len() > 1 {
        let mut json_values: Vec<Value> = Vec::new();

        for line in lines.iter() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<Value>(line) {
                Ok(value) => json_values.push(value),
                Err(_) => {
                    // If any line fails, try as single JSON instead
                    json_values.clear();
                    break;
                }
            }
        }

        if !json_values.is_empty() {
            return Ok(ParseResult::JsonL(json_values));
        }
    }

    // Try to parse as single JSON
    match serde_json::from_str::<Value>(content) {
        Ok(value) => Ok(ParseResult::Single(value)),
        Err(e) => Err(ParseError::InvalidJson(e.to_string())),
    }
}

/// Parses YAML content and converts it to JSON format.
///
/// # Arguments
///
/// * `content` - The YAML string content to parse
///
/// # Returns
///
/// * `Ok(ParseResult::Single(_))` if content is valid YAML
/// * `Err(ParseError::InvalidYaml(_))` if content is not valid YAML
pub fn parse_yaml_content(content: &str) -> Result<ParseResult, ParseError> {
    match serde_yaml::from_str::<Value>(content) {
        Ok(value) => Ok(ParseResult::Single(value)),
        Err(e) => Err(ParseError::InvalidYaml(e.to_string())),
    }
}

/// Parses Parquet file content and converts it to JSON format.
///
/// Parquet files are converted to an array of objects, where each object represents a row.
///
/// # Arguments
///
/// * `data` - The binary Parquet file content
///
/// # Returns
///
/// * `Ok(ParseResult::Single(_))` if content is valid Parquet (returns array of row objects)
/// * `Err(ParseError::InvalidParquet(_))` if content is not valid Parquet
pub fn parse_parquet_content(data: &[u8]) -> Result<ParseResult, ParseError> {
    use arrow::array::Array;
    use bytes::Bytes;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

    // Convert to Bytes which implements ChunkReader
    let owned_data = data.to_vec();
    let bytes_data = Bytes::from(owned_data);
    let builder = ParquetRecordBatchReaderBuilder::try_new(bytes_data)
        .map_err(|e: parquet::errors::ParquetError| ParseError::InvalidParquet(e.to_string()))?;

    let reader = builder
        .build()
        .map_err(|e: parquet::errors::ParquetError| ParseError::InvalidParquet(e.to_string()))?;

    let mut all_rows: Vec<Value> = Vec::new();

    for batch_result in reader {
        let batch = batch_result.map_err(|e: arrow::error::ArrowError| ParseError::InvalidParquet(e.to_string()))?;

        // Convert each row in the batch to a JSON object
        let num_rows = batch.num_rows();
        let num_cols = batch.num_columns();
        let schema = batch.schema();

        for row_idx in 0..num_rows {
            let mut row_obj = serde_json::Map::new();

            for col_idx in 0..num_cols {
                let column = batch.column(col_idx);
                let field_name = schema.field(col_idx).name().to_string();

                // Extract value from the column at this row
                let value = if column.is_null(row_idx) {
                    Value::Null
                } else {
                    match column.data_type() {
                        arrow::datatypes::DataType::Utf8 => {
                            let array = column.as_any().downcast_ref::<arrow::array::StringArray>()
                                .ok_or_else(|| ParseError::InvalidParquet("Failed to cast string array".to_string()))?;
                            Value::String(array.value(row_idx).to_string())
                        }
                        arrow::datatypes::DataType::Int64 => {
                            let array = column.as_any().downcast_ref::<arrow::array::Int64Array>()
                                .ok_or_else(|| ParseError::InvalidParquet("Failed to cast int64 array".to_string()))?;
                            Value::Number(array.value(row_idx).into())
                        }
                        arrow::datatypes::DataType::Float64 => {
                            let array = column.as_any().downcast_ref::<arrow::array::Float64Array>()
                                .ok_or_else(|| ParseError::InvalidParquet("Failed to cast float64 array".to_string()))?;
                            Value::Number(serde_json::Number::from_f64(array.value(row_idx))
                                .unwrap_or_else(|| serde_json::Number::from(0)))
                        }
                        arrow::datatypes::DataType::Boolean => {
                            let array = column.as_any().downcast_ref::<arrow::array::BooleanArray>()
                                .ok_or_else(|| ParseError::InvalidParquet("Failed to cast boolean array".to_string()))?;
                            Value::Bool(array.value(row_idx))
                        }
                        _ => {
                            // For other types, convert to string representation
                            Value::String(format!("{:?}", column))
                        }
                    }
                };

                row_obj.insert(field_name, value);
            }

            all_rows.push(Value::Object(row_obj));
        }
    }

    Ok(ParseResult::Single(Value::Array(all_rows)))
}

/// Parses a file based on its extension.
///
/// Supports:
/// - `.json`, `.jsonl` - JSON/JSONL format
/// - `.yaml`, `.yml` - YAML format
/// - `.parquet` - Parquet format
///
/// # Arguments
///
/// * `path` - Path to the file
///
/// # Returns
///
/// * `Ok(ParseResult)` if file was successfully parsed
/// * `Err(ParseError)` if parsing failed or file type is unsupported
pub fn parse_file(path: &Path) -> Result<ParseResult, ParseError> {
    use std::fs;

    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_lowercase();

    match extension.as_str() {
        "json" | "jsonl" => {
            let content = fs::read_to_string(path)
                .map_err(|e| ParseError::IoError(e.to_string()))?;
            parse_json_content(&content)
        }
        "yaml" | "yml" => {
            let content = fs::read_to_string(path)
                .map_err(|e| ParseError::IoError(e.to_string()))?;
            parse_yaml_content(&content)
        }
        "parquet" => {
            let data = fs::read(path)
                .map_err(|e| ParseError::IoError(e.to_string()))?;
            parse_parquet_content(&data)
        }
        _ => {
            // Try to auto-detect: first try as JSON, then YAML
            let content = fs::read_to_string(path)
                .map_err(|e| ParseError::IoError(e.to_string()))?;
            parse_text_content(&content)
        }
    }
}

/// Parses text content, trying JSON/JSONL first, then YAML.
///
/// This is useful for clipboard content or other text sources where the format is unknown.
///
/// # Arguments
///
/// * `content` - The text content to parse
///
/// # Returns
///
/// * `Ok(ParseResult)` if content was successfully parsed as JSON/JSONL or YAML
/// * `Err(ParseError)` if parsing failed for all formats
pub fn parse_text_content(content: &str) -> Result<ParseResult, ParseError> {
    // Try JSON/JSONL first
    if let Ok(result) = parse_json_content(content) {
        return Ok(result);
    }
    
    // Then try YAML
    parse_yaml_content(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_json_object() {
        let content = r#"{"name": "test", "value": 42}"#;
        let result = parse_json_content(content).unwrap();
        
        match result {
            ParseResult::Single(value) => {
                assert_eq!(value["name"], "test");
                assert_eq!(value["value"], 42);
            }
            _ => panic!("Expected Single result"),
        }
    }

    #[test]
    fn test_parse_single_json_array() {
        let content = r#"[1, 2, 3, "four"]"#;
        let result = parse_json_content(content).unwrap();
        
        match result {
            ParseResult::Single(value) => {
                assert!(value.is_array());
                assert_eq!(value.as_array().unwrap().len(), 4);
            }
            _ => panic!("Expected Single result"),
        }
    }

    #[test]
    fn test_parse_jsonl_multiple_objects() {
        let content = r#"{"name": "first"}
{"name": "second", "value": 42}
{"name": "third"}"#;
        let result = parse_json_content(content).unwrap();
        
        match result {
            ParseResult::JsonL(values) => {
                assert_eq!(values.len(), 3);
                assert_eq!(values[0]["name"], "first");
                assert_eq!(values[1]["name"], "second");
                assert_eq!(values[1]["value"], 42);
                assert_eq!(values[2]["name"], "third");
            }
            _ => panic!("Expected JsonL result"),
        }
    }

    #[test]
    fn test_parse_jsonl_with_empty_lines() {
        let content = r#"{"name": "first"}

{"name": "second"}

{"name": "third"}"#;
        let result = parse_json_content(content).unwrap();
        
        match result {
            ParseResult::JsonL(values) => {
                assert_eq!(values.len(), 3);
                assert_eq!(values[0]["name"], "first");
                assert_eq!(values[1]["name"], "second");
                assert_eq!(values[2]["name"], "third");
            }
            _ => panic!("Expected JsonL result"),
        }
    }

    #[test]
    fn test_parse_invalid_json() {
        let content = r#"{"name": "test", invalid}"#;
        let result = parse_json_content(content);
        
        assert!(result.is_err());
        match result {
            Err(ParseError::InvalidJson(_)) => {}
            _ => panic!("Expected InvalidJson error"),
        }
    }

    #[test]
    fn test_parse_yaml_object() {
        let content = r#"
name: test
value: 42
nested:
  key: value
"#;
        let result = parse_yaml_content(content).unwrap();
        
        match result {
            ParseResult::Single(value) => {
                assert_eq!(value["name"], "test");
                assert_eq!(value["value"], 42);
                assert_eq!(value["nested"]["key"], "value");
            }
            _ => panic!("Expected Single result"),
        }
    }

    #[test]
    fn test_parse_yaml_array() {
        let content = r#"
- name: first
  value: 1
- name: second
  value: 2
"#;
        let result = parse_yaml_content(content).unwrap();
        
        match result {
            ParseResult::Single(value) => {
                assert!(value.is_array());
                let arr = value.as_array().unwrap();
                assert_eq!(arr.len(), 2);
                assert_eq!(arr[0]["name"], "first");
                assert_eq!(arr[1]["value"], 2);
            }
            _ => panic!("Expected Single result"),
        }
    }

    #[test]
    fn test_parse_invalid_yaml() {
        let content = r#"
name: test
  invalid: indentation
"#;
        let result = parse_yaml_content(content);
        
        assert!(result.is_err());
        match result {
            Err(ParseError::InvalidYaml(_)) => {}
            _ => panic!("Expected InvalidYaml error"),
        }
    }

    #[test]
    fn test_parse_parquet_simple() {
        use arrow::array::{Int64Array, StringArray};
        use arrow::datatypes::{DataType, Field, Schema};
        use arrow::record_batch::RecordBatch;
        use parquet::arrow::arrow_writer::ArrowWriter;
        use parquet::file::properties::WriterProperties;
        use std::sync::Arc;

        // Create a simple schema
        let schema = Schema::new(vec![
            Field::new("id", DataType::Int64, false),
            Field::new("name", DataType::Utf8, false),
        ]);

        // Create sample data
        let id_array = Int64Array::from(vec![1, 2, 3]);
        let name_array = StringArray::from(vec!["Alice", "Bob", "Charlie"]);

        // Create record batch
        let batch = RecordBatch::try_new(
            Arc::new(schema),
            vec![Arc::new(id_array), Arc::new(name_array)],
        )
        .unwrap();

        // Write to Parquet in memory
        let mut buffer = Vec::new();
        let props = WriterProperties::builder().build();
        let mut writer = ArrowWriter::try_new(&mut buffer, batch.schema().clone(), Some(props))
            .unwrap();
        writer.write(&batch).unwrap();
        writer.close().unwrap();

        // Parse the Parquet data
        let result = parse_parquet_content(&buffer).unwrap();

        match result {
            ParseResult::Single(value) => {
                assert!(value.is_array());
                let arr = value.as_array().unwrap();
                assert_eq!(arr.len(), 3);
                
                // Check first row
                assert_eq!(arr[0]["id"], 1);
                assert_eq!(arr[0]["name"], "Alice");
                
                // Check second row
                assert_eq!(arr[1]["id"], 2);
                assert_eq!(arr[1]["name"], "Bob");
                
                // Check third row
                assert_eq!(arr[2]["id"], 3);
                assert_eq!(arr[2]["name"], "Charlie");
            }
            _ => panic!("Expected Single result with array"),
        }
    }

    #[test]
    fn test_parse_parquet_with_nullable_fields() {
        use arrow::array::{BooleanArray, Float64Array};
        use arrow::datatypes::{DataType, Field, Schema};
        use arrow::record_batch::RecordBatch;
        use parquet::arrow::arrow_writer::ArrowWriter;
        use parquet::file::properties::WriterProperties;
        use std::sync::Arc;

        // Create schema with nullable fields
        let schema = Schema::new(vec![
            Field::new("active", DataType::Boolean, true),
            Field::new("score", DataType::Float64, true),
        ]);

        // Create sample data with nulls
        let active_array = BooleanArray::from(vec![Some(true), None, Some(false)]);
        let score_array = Float64Array::from(vec![Some(95.5), None, Some(87.0)]);

        // Create record batch
        let batch = RecordBatch::try_new(
            Arc::new(schema),
            vec![Arc::new(active_array), Arc::new(score_array)],
        )
        .unwrap();

        // Write to Parquet in memory
        let mut buffer = Vec::new();
        let props = WriterProperties::builder().build();
        let mut writer = ArrowWriter::try_new(&mut buffer, batch.schema().clone(), Some(props))
            .unwrap();
        writer.write(&batch).unwrap();
        writer.close().unwrap();

        // Parse the Parquet data
        let result = parse_parquet_content(&buffer).unwrap();

        match result {
            ParseResult::Single(value) => {
                assert!(value.is_array());
                let arr = value.as_array().unwrap();
                assert_eq!(arr.len(), 3);
                
                // Check first row
                assert_eq!(arr[0]["active"], true);
                assert_eq!(arr[0]["score"].as_f64().unwrap(), 95.5);
                
                // Check second row (with nulls)
                assert_eq!(arr[1]["active"], Value::Null);
                assert_eq!(arr[1]["score"], Value::Null);
                
                // Check third row
                assert_eq!(arr[2]["active"], false);
                assert_eq!(arr[2]["score"].as_f64().unwrap(), 87.0);
            }
            _ => panic!("Expected Single result with array"),
        }
    }

    #[test]
    fn test_parse_invalid_parquet() {
        let invalid_data = b"not a parquet file";
        let result = parse_parquet_content(invalid_data);
        
        assert!(result.is_err());
        match result {
            Err(ParseError::InvalidParquet(_)) => {}
            _ => panic!("Expected InvalidParquet error"),
        }
    }

    #[test]
    fn test_parse_text_content_json() {
        let content = r#"{"name": "test", "value": 42}"#;
        let result = parse_text_content(content).unwrap();
        
        match result {
            ParseResult::Single(value) => {
                assert_eq!(value["name"], "test");
                assert_eq!(value["value"], 42);
            }
            _ => panic!("Expected Single result"),
        }
    }

    #[test]
    fn test_parse_text_content_jsonl() {
        let content = r#"{"name": "first"}
{"name": "second"}"#;
        let result = parse_text_content(content).unwrap();
        
        match result {
            ParseResult::JsonL(values) => {
                assert_eq!(values.len(), 2);
                assert_eq!(values[0]["name"], "first");
                assert_eq!(values[1]["name"], "second");
            }
            _ => panic!("Expected JsonL result"),
        }
    }

    #[test]
    fn test_parse_text_content_yaml() {
        let content = r#"
name: test
value: 42
"#;
        let result = parse_text_content(content).unwrap();
        
        match result {
            ParseResult::Single(value) => {
                assert_eq!(value["name"], "test");
                assert_eq!(value["value"], 42);
            }
            _ => panic!("Expected Single result"),
        }
    }

    #[test]
    fn test_parse_text_content_yaml_fallback() {
        // Invalid JSON but valid YAML
        let content = r#"
name: test
value: 42
nested:
  key: value
"#;
        let result = parse_text_content(content).unwrap();
        
        match result {
            ParseResult::Single(value) => {
                assert_eq!(value["name"], "test");
                assert_eq!(value["value"], 42);
                assert_eq!(value["nested"]["key"], "value");
            }
            _ => panic!("Expected Single result"),
        }
    }
}


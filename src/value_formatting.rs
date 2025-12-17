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

/// Formats a JSON value as a preview string for display in tree nodes.
pub fn format_value_preview(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => {
            if s.chars().count() > 50 {
                let truncated: String = s.chars().take(50).collect();
                format!("\"{}\"...", truncated)
            } else {
                format!("\"{}\"", s)
            }
        }
        Value::Array(arr) => format!("Array[{}]", arr.len()),
        Value::Object(map) => format!("Object{{{}}}", map.len()),
    }
}

/// Formats a JSON value as a literal string for display in the value viewer.
/// Strings are displayed as-is, other values are formatted as pretty JSON.
pub fn format_value_literal(value: &Value) -> String {
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

/// Formats a JSON value from a string representation.
/// Attempts to parse the string as JSON and format it, falling back to the original string
/// or preview if parsing fails.
///
/// # Arguments
///
/// * `full_value` - The JSON string representation
/// * `preview_fallback` - Fallback string to use if full_value is empty
///
/// # Returns
///
/// Formatted string ready for display
pub fn format_value_from_string(full_value: &str, preview_fallback: &str) -> String {
    if !full_value.is_empty() {
        match serde_json::from_str::<Value>(full_value) {
            Ok(v) => format_value_literal(&v),
            Err(_) => full_value.to_string(),
        }
    } else {
        preview_fallback.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_value_preview_null() {
        assert_eq!(format_value_preview(&Value::Null), "null");
    }

    #[test]
    fn test_format_value_preview_bool() {
        assert_eq!(format_value_preview(&Value::Bool(true)), "true");
        assert_eq!(format_value_preview(&Value::Bool(false)), "false");
    }

    #[test]
    fn test_format_value_preview_number() {
        assert_eq!(format_value_preview(&serde_json::json!(42)), "42");
        assert_eq!(format_value_preview(&serde_json::json!(3.14)), "3.14");
    }

    #[test]
    fn test_format_value_preview_string_short() {
        assert_eq!(
            format_value_preview(&serde_json::json!("hello")),
            "\"hello\""
        );
    }

    #[test]
    fn test_format_value_preview_string_long() {
        let long_string = "a".repeat(60);
        let result = format_value_preview(&serde_json::json!(long_string));
        assert!(result.starts_with('"'));
        assert!(result.ends_with("..."));
        // Length should be: " (1) + 50 chars + " (1) + ... (3) = 55
        assert_eq!(result.len(), 55);
    }

    #[test]
    fn test_format_value_preview_array() {
        assert_eq!(
            format_value_preview(&serde_json::json!([1, 2, 3])),
            "Array[3]"
        );
    }

    #[test]
    fn test_format_value_preview_object() {
        assert_eq!(
            format_value_preview(&serde_json::json!({"a": 1, "b": 2})),
            "Object{2}"
        );
    }

    #[test]
    fn test_format_value_literal_string() {
        assert_eq!(format_value_literal(&serde_json::json!("hello")), "hello");
    }

    #[test]
    fn test_format_value_literal_object() {
        let value = serde_json::json!({"a": 1, "b": 2});
        let result = format_value_literal(&value);
        assert!(result.contains("\"a\": 1"));
        assert!(result.contains("\"b\": 2"));
    }

    #[test]
    fn test_format_value_literal_array() {
        let value = serde_json::json!([1, 2, 3]);
        let result = format_value_literal(&value);
        assert!(result.contains("1"));
        assert!(result.contains("2"));
        assert!(result.contains("3"));
    }

    #[test]
    fn test_format_value_from_string_valid_json() {
        let json_str = r#"{"name": "test", "value": 42}"#;
        let result = format_value_from_string(json_str, "fallback");
        assert!(result.contains("test"));
        assert!(result.contains("42"));
    }

    #[test]
    fn test_format_value_from_string_empty() {
        let result = format_value_from_string("", "fallback");
        assert_eq!(result, "fallback");
    }

    #[test]
    fn test_format_value_from_string_invalid_json() {
        let invalid = "not valid json";
        let result = format_value_from_string(invalid, "fallback");
        assert_eq!(result, invalid);
    }
}

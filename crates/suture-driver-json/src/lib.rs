use serde_json::Value;
use suture_driver::{DriverError, SemanticChange, SutureDriver};

pub struct JsonDriver;

impl JsonDriver {
    pub fn new() -> Self {
        Self
    }

    fn json_pointer_escape(s: &str) -> String {
        s.replace('~', "~0").replace('/', "~1")
    }

    fn diff_values(old: &Value, new: &Value, path: &str) -> Vec<SemanticChange> {
        let mut changes = Vec::new();

        match (old, new) {
            (Value::Object(old_map), Value::Object(new_map)) => {
                let old_keys: std::collections::HashSet<&str> =
                    old_map.keys().map(|s| s.as_str()).collect();
                let new_keys: std::collections::HashSet<&str> =
                    new_map.keys().map(|s| s.as_str()).collect();

                for key in &old_keys {
                    if !new_keys.contains(key) {
                        let escaped = Self::json_pointer_escape(key);
                        let child_path = if path == "/" {
                            format!("/{escaped}")
                        } else {
                            format!("{path}/{escaped}")
                        };
                        changes.push(SemanticChange::Removed {
                            path: child_path,
                            old_value: old_map[*key].to_string(),
                        });
                    }
                }

                for key in &new_keys {
                    if !old_keys.contains(key) {
                        let escaped = Self::json_pointer_escape(key);
                        let child_path = if path == "/" {
                            format!("/{escaped}")
                        } else {
                            format!("{path}/{escaped}")
                        };
                        changes.push(SemanticChange::Added {
                            path: child_path,
                            value: new_map[*key].to_string(),
                        });
                    }
                }

                for key in &old_keys {
                    if let Some(new_val) = new_keys.contains(key).then(|| &new_map[*key]) {
                        let escaped = Self::json_pointer_escape(key);
                        let child_path = if path == "/" {
                            format!("/{escaped}")
                        } else {
                            format!("{path}/{escaped}")
                        };
                        changes.extend(Self::diff_values(&old_map[*key], new_val, &child_path));
                    }
                }
            }
            (Value::Array(old_arr), Value::Array(new_arr)) => {
                let max_len = old_arr.len().max(new_arr.len());

                for i in 0..max_len {
                    let child_path = format!("{path}/{i}");
                    match (old_arr.get(i), new_arr.get(i)) {
                        (None, Some(new_val)) => {
                            changes.push(SemanticChange::Added {
                                path: child_path,
                                value: new_val.to_string(),
                            });
                        }
                        (Some(old_val), None) => {
                            changes.push(SemanticChange::Removed {
                                path: child_path,
                                old_value: old_val.to_string(),
                            });
                        }
                        (Some(old_val), Some(new_val)) => {
                            changes.extend(Self::diff_values(old_val, new_val, &child_path));
                        }
                        (None, None) => {}
                    }
                }
            }
            (old_val, new_val) if old_val != new_val => {
                changes.push(SemanticChange::Modified {
                    path: path.to_string(),
                    old_value: old_val.to_string(),
                    new_value: new_val.to_string(),
                });
            }
            _ => {}
        }

        changes
    }

    fn format_change(change: &SemanticChange) -> String {
        match change {
            SemanticChange::Added { path, value } => {
                format!("  ADDED     {path}: {value}")
            }
            SemanticChange::Removed { path, old_value } => {
                format!("  REMOVED   {path}: {old_value}")
            }
            SemanticChange::Modified {
                path,
                old_value,
                new_value,
            } => {
                format!("  MODIFIED  {path}: {old_value} → {new_value}")
            }
            SemanticChange::Moved {
                old_path,
                new_path,
                value,
            } => {
                format!("  MOVED     {old_path} → {new_path}: {value}")
            }
        }
    }
}

impl Default for JsonDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl SutureDriver for JsonDriver {
    fn name(&self) -> &str {
        "JSON"
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".json"]
    }

    fn diff(
        &self,
        base_content: Option<&str>,
        new_content: &str,
    ) -> Result<Vec<SemanticChange>, DriverError> {
        let new_val: Value = serde_json::from_str(new_content)
            .map_err(|e| DriverError::ParseError(e.to_string()))?;

        match base_content {
            None => {
                let mut changes = Vec::new();
                collect_all_paths(&new_val, "/".to_string(), &mut changes);
                Ok(changes)
            }
            Some(base) => {
                let old_val: Value = serde_json::from_str(base)
                    .map_err(|e| DriverError::ParseError(e.to_string()))?;
                Ok(Self::diff_values(&old_val, &new_val, "/"))
            }
        }
    }

    fn format_diff(
        &self,
        base_content: Option<&str>,
        new_content: &str,
    ) -> Result<String, DriverError> {
        let changes = self.diff(base_content, new_content)?;

        if changes.is_empty() {
            return Ok("no changes".to_string());
        }

        let lines: Vec<String> = changes.iter().map(Self::format_change).collect();
        Ok(lines.join("\n"))
    }
}

fn collect_all_paths(val: &Value, path: String, out: &mut Vec<SemanticChange>) {
    match val {
        Value::Object(map) => {
            for (key, child) in map {
                let escaped = JsonDriver::json_pointer_escape(key);
                let child_path = if path == "/" {
                    format!("/{escaped}")
                } else {
                    format!("{path}/{escaped}")
                };
                collect_all_paths(child, child_path, out);
            }
        }
        Value::Array(arr) => {
            for (i, child) in arr.iter().enumerate() {
                let child_path = format!("{path}/{i}");
                collect_all_paths(child, child_path, out);
            }
        }
        other => {
            out.push(SemanticChange::Added {
                path,
                value: other.to_string(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_driver_name() {
        let driver = JsonDriver::new();
        assert_eq!(driver.name(), "JSON");
    }

    #[test]
    fn test_json_driver_extensions() {
        let driver = JsonDriver::new();
        assert_eq!(driver.supported_extensions(), &[".json"]);
    }

    #[test]
    fn test_diff_added_key() {
        let driver = JsonDriver::new();
        let old = r#"{"name": "Alice"}"#;
        let new = r#"{"name": "Alice", "email": "alice@example.com"}"#;

        let changes = driver.diff(Some(old), new).unwrap();
        assert!(changes.contains(&SemanticChange::Added {
            path: "/email".to_string(),
            value: "\"alice@example.com\"".to_string(),
        }));
    }

    #[test]
    fn test_diff_removed_key() {
        let driver = JsonDriver::new();
        let old = r#"{"name": "Alice", "phone": "+1234567890"}"#;
        let new = r#"{"name": "Alice"}"#;

        let changes = driver.diff(Some(old), new).unwrap();
        assert!(changes.contains(&SemanticChange::Removed {
            path: "/phone".to_string(),
            old_value: "\"+1234567890\"".to_string(),
        }));
    }

    #[test]
    fn test_diff_modified_key() {
        let driver = JsonDriver::new();
        let old = r#"{"name": "Alice"}"#;
        let new = r#"{"name": "Bob"}"#;

        let changes = driver.diff(Some(old), new).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(
            changes[0],
            SemanticChange::Modified {
                path: "/name".to_string(),
                old_value: "\"Alice\"".to_string(),
                new_value: "\"Bob\"".to_string(),
            }
        );
    }

    #[test]
    fn test_diff_nested() {
        let driver = JsonDriver::new();
        let old = r#"{"address": {"city": "NYC", "zip": "10001"}}"#;
        let new = r#"{"address": {"city": "San Francisco", "zip": "10001"}}"#;

        let changes = driver.diff(Some(old), new).unwrap();
        assert!(changes.contains(&SemanticChange::Modified {
            path: "/address/city".to_string(),
            old_value: "\"NYC\"".to_string(),
            new_value: "\"San Francisco\"".to_string(),
        }));
    }

    #[test]
    fn test_diff_new_file() {
        let driver = JsonDriver::new();
        let new = r#"{"name": "Alice", "age": 30}"#;

        let changes = driver.diff(None, new).unwrap();
        assert!(!changes.is_empty());
        for change in &changes {
            assert!(matches!(change, SemanticChange::Added { .. }));
        }
    }

    #[test]
    fn test_format_diff() {
        let driver = JsonDriver::new();
        let old = r#"{"name": "Alice"}"#;
        let new = r#"{"name": "Bob", "email": "bob@example.com"}"#;

        let output = driver.format_diff(Some(old), new).unwrap();
        assert!(output.contains("MODIFIED"));
        assert!(output.contains("ADDED"));
        assert!(output.contains("/name"));
        assert!(output.contains("/email"));
    }

    #[test]
    fn test_format_diff_empty() {
        let driver = JsonDriver::new();
        let content = r#"{"name": "Alice"}"#;

        let output = driver.format_diff(Some(content), content).unwrap();
        assert_eq!(output, "no changes");
    }

    #[test]
    fn test_array_changes() {
        let driver = JsonDriver::new();
        let old = r#"{"items": ["a", "b"]}"#;
        let new = r#"{"items": ["a", "c", "d"]}"#;

        let changes = driver.diff(Some(old), new).unwrap();
        assert!(changes.contains(&SemanticChange::Modified {
            path: "/items/1".to_string(),
            old_value: "\"b\"".to_string(),
            new_value: "\"c\"".to_string(),
        }));
        assert!(changes.contains(&SemanticChange::Added {
            path: "/items/2".to_string(),
            value: "\"d\"".to_string(),
        }));
    }
}

use serde_yaml::Value;
use suture_driver::{DriverError, SemanticChange, SutureDriver};

pub struct YamlDriver;

impl YamlDriver {
    pub fn new() -> Self {
        Self
    }

    fn value_to_string(val: &Value) -> String {
        match val {
            Value::String(s) => s.clone(),
            other => serde_yaml::to_string(other).unwrap_or_else(|_| format!("{other:#?}")),
        }
    }

    fn diff_values(old: &Value, new: &Value, path: &str) -> Vec<SemanticChange> {
        let mut changes = Vec::new();

        match (old, new) {
            (Value::Mapping(old_map), Value::Mapping(new_map)) => {
                let old_keys: std::collections::HashSet<&Value> = old_map.keys().collect();
                let new_keys: std::collections::HashSet<&Value> = new_map.keys().collect();

                for key in &old_keys {
                    if !new_keys.contains(key) {
                        let child_path = Self::child_path(path, key);
                        changes.push(SemanticChange::Removed {
                            path: child_path,
                            old_value: Self::value_to_string(&old_map[*key]),
                        });
                    }
                }

                for key in &new_keys {
                    if !old_keys.contains(key) {
                        let child_path = Self::child_path(path, key);
                        changes.push(SemanticChange::Added {
                            path: child_path,
                            value: Self::value_to_string(&new_map[*key]),
                        });
                    }
                }

                for key in &old_keys {
                    if let Some(new_val) = new_keys.contains(key).then(|| &new_map[*key]) {
                        let child_path = Self::child_path(path, key);
                        changes.extend(Self::diff_values(&old_map[*key], new_val, &child_path));
                    }
                }
            }
            (Value::Sequence(old_arr), Value::Sequence(new_arr)) => {
                let max_len = old_arr.len().max(new_arr.len());

                for i in 0..max_len {
                    let child_path = format!("{path}/{i}");
                    match (old_arr.get(i), new_arr.get(i)) {
                        (None, Some(new_val)) => {
                            changes.push(SemanticChange::Added {
                                path: child_path,
                                value: Self::value_to_string(new_val),
                            });
                        }
                        (Some(old_val), None) => {
                            changes.push(SemanticChange::Removed {
                                path: child_path,
                                old_value: Self::value_to_string(old_val),
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
                    old_value: Self::value_to_string(old_val),
                    new_value: Self::value_to_string(new_val),
                });
            }
            _ => {}
        }

        changes
    }

    fn child_path(parent: &str, key: &Value) -> String {
        let key_str = match key.as_str() {
            Some(s) => s.to_string(),
            None => serde_yaml::to_string(key).unwrap_or_else(|_| format!("{key:#?}")),
        };
        if parent == "/" {
            format!("/{key_str}")
        } else {
            format!("{parent}/{key_str}")
        }
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

impl Default for YamlDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl SutureDriver for YamlDriver {
    fn name(&self) -> &str {
        "YAML"
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".yaml", ".yml"]
    }

    fn diff(
        &self,
        base_content: Option<&str>,
        new_content: &str,
    ) -> Result<Vec<SemanticChange>, DriverError> {
        let new_val: Value = serde_yaml::from_str(new_content)
            .map_err(|e| DriverError::ParseError(e.to_string()))?;

        match base_content {
            None => {
                let mut changes = Vec::new();
                collect_all_paths(&new_val, "/".to_string(), &mut changes);
                Ok(changes)
            }
            Some(base) => {
                let old_val: Value = serde_yaml::from_str(base)
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
        Value::Mapping(map) => {
            for (key, child) in map {
                let child_path = YamlDriver::child_path(&path, key);
                collect_all_paths(child, child_path, out);
            }
        }
        Value::Sequence(arr) => {
            for (i, child) in arr.iter().enumerate() {
                let child_path = format!("{path}/{i}");
                collect_all_paths(child, child_path, out);
            }
        }
        other => {
            out.push(SemanticChange::Added {
                path,
                value: YamlDriver::value_to_string(other),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yaml_driver_name() {
        let driver = YamlDriver::new();
        assert_eq!(driver.name(), "YAML");
    }

    #[test]
    fn test_yaml_driver_extensions() {
        let driver = YamlDriver::new();
        assert_eq!(driver.supported_extensions(), &[".yaml", ".yml"]);
    }

    #[test]
    fn test_yaml_diff_modified() {
        let driver = YamlDriver::new();
        let old = "name: Alice\nage: 30\n";
        let new = "name: Bob\nage: 30\n";

        let changes = driver.diff(Some(old), new).unwrap();
        assert!(changes.contains(&SemanticChange::Modified {
            path: "/name".to_string(),
            old_value: "Alice".to_string(),
            new_value: "Bob".to_string(),
        }));
    }

    #[test]
    fn test_yaml_diff_added() {
        let driver = YamlDriver::new();
        let old = "name: Alice\n";
        let new = "name: Alice\nemail: alice@example.com\n";

        let changes = driver.diff(Some(old), new).unwrap();
        assert!(changes.contains(&SemanticChange::Added {
            path: "/email".to_string(),
            value: "alice@example.com".to_string(),
        }));
    }

    #[test]
    fn test_yaml_diff_nested() {
        let driver = YamlDriver::new();
        let old = "address:\n  city: NYC\n  zip: \"10001\"\n";
        let new = "address:\n  city: San Francisco\n  zip: \"10001\"\n";

        let changes = driver.diff(Some(old), new).unwrap();
        assert!(changes.contains(&SemanticChange::Modified {
            path: "/address/city".to_string(),
            old_value: "NYC".to_string(),
            new_value: "San Francisco".to_string(),
        }));
    }
}

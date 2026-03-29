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

    fn merge_values(
        base: &Value,
        ours: &Value,
        theirs: &Value,
    ) -> Result<Option<Value>, DriverError> {
        match (base, ours, theirs) {
            (Value::Mapping(base_map), Value::Mapping(ours_map), Value::Mapping(theirs_map)) => {
                let base_keys: std::collections::HashSet<&Value> = base_map.keys().collect();
                let ours_keys: std::collections::HashSet<&Value> = ours_map.keys().collect();
                let theirs_keys: std::collections::HashSet<&Value> = theirs_map.keys().collect();

                let all_keys: std::collections::HashSet<&Value> = base_keys
                    .iter()
                    .chain(ours_keys.iter())
                    .chain(theirs_keys.iter())
                    .copied()
                    .collect();

                let mut merged = serde_yaml::Mapping::new();

                for key in &all_keys {
                    let in_base = base_keys.contains(key);
                    let in_ours = ours_keys.contains(key);
                    let in_theirs = theirs_keys.contains(key);

                    match (in_base, in_ours, in_theirs) {
                        (true, true, false) => {
                            if let Some(val) = ours_map.get(key) {
                                merged.insert((*key).clone(), val.clone());
                            }
                        }
                        (true, false, true) => {
                            if let Some(val) = theirs_map.get(key) {
                                merged.insert((*key).clone(), val.clone());
                            }
                        }
                        (true, true, true) => {
                            let base_val = &base_map[key];
                            let ours_val = &ours_map[key];
                            let theirs_val = &theirs_map[key];

                            if ours_val == theirs_val {
                                merged.insert((*key).clone(), ours_val.clone());
                            } else if ours_val == base_val {
                                merged.insert((*key).clone(), theirs_val.clone());
                            } else if theirs_val == base_val {
                                merged.insert((*key).clone(), ours_val.clone());
                            } else if let Some(m) =
                                Self::merge_values(base_val, ours_val, theirs_val)?
                            {
                                merged.insert((*key).clone(), m);
                            } else {
                                return Ok(None);
                            }
                        }
                        (false, true, true) => {
                            if ours_map[key] == theirs_map[key] {
                                merged.insert((*key).clone(), ours_map[key].clone());
                            } else {
                                return Ok(None);
                            }
                        }
                        (false, true, false) => {
                            if let Some(val) = ours_map.get(key) {
                                merged.insert((*key).clone(), val.clone());
                            }
                        }
                        (false, false, true) => {
                            if let Some(val) = theirs_map.get(key) {
                                merged.insert((*key).clone(), val.clone());
                            }
                        }
                        (true, false, false) | (false, false, false) => {}
                    }
                }

                Ok(Some(Value::Mapping(merged)))
            }
            (Value::Sequence(base_arr), Value::Sequence(ours_arr), Value::Sequence(theirs_arr)) => {
                let max_len = base_arr.len().max(ours_arr.len()).max(theirs_arr.len());
                let mut merged = Vec::new();

                for i in 0..max_len {
                    let base_val = base_arr.get(i);
                    let ours_val = ours_arr.get(i);
                    let theirs_val = theirs_arr.get(i);

                    match (base_val, ours_val, theirs_val) {
                        (None, Some(o), None) => merged.push(o.clone()),
                        (None, None, Some(t)) => merged.push(t.clone()),
                        (None, Some(o), Some(t)) => {
                            if o == t {
                                merged.push(o.clone());
                            } else {
                                return Ok(None);
                            }
                        }
                        (None, None, _) => {}
                        (Some(_), Some(o), None) => merged.push(o.clone()),
                        (Some(_), None, Some(t)) => merged.push(t.clone()),
                        (Some(_), None, None) => {}
                        (Some(b), Some(o), Some(t)) => {
                            if o == t {
                                merged.push(o.clone());
                            } else if o == b {
                                merged.push(t.clone());
                            } else if t == b {
                                merged.push(o.clone());
                            } else if let Some(m) = Self::merge_values(b, o, t)? {
                                merged.push(m);
                            } else {
                                return Ok(None);
                            }
                        }
                    }
                }

                Ok(Some(Value::Sequence(merged)))
            }
            (base_val, ours_val, theirs_val) => {
                if ours_val == theirs_val {
                    Ok(Some(ours_val.clone()))
                } else if ours_val == base_val {
                    Ok(Some(theirs_val.clone()))
                } else if theirs_val == base_val {
                    Ok(Some(ours_val.clone()))
                } else {
                    Ok(None)
                }
            }
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

    fn merge(&self, base: &str, ours: &str, theirs: &str) -> Result<Option<String>, DriverError> {
        let base_val: Value =
            serde_yaml::from_str(base).map_err(|e| DriverError::ParseError(e.to_string()))?;
        let ours_val: Value =
            serde_yaml::from_str(ours).map_err(|e| DriverError::ParseError(e.to_string()))?;
        let theirs_val: Value =
            serde_yaml::from_str(theirs).map_err(|e| DriverError::ParseError(e.to_string()))?;

        match Self::merge_values(&base_val, &ours_val, &theirs_val)? {
            Some(merged) => {
                Ok(Some(serde_yaml::to_string(&merged).map_err(|e| {
                    DriverError::SerializationError(e.to_string())
                })?))
            }
            None => Ok(None),
        }
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

    #[test]
    fn test_yaml_merge_no_conflict() {
        let driver = YamlDriver::new();
        let base = "a: 1\nb: 2\nc: 3\n";
        let ours = "a: 10\nb: 2\nc: 3\n";
        let theirs = "a: 1\nb: 2\nc: 30\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_yaml::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["a"], Value::Number(10.into()));
        assert_eq!(merged["b"], Value::Number(2.into()));
        assert_eq!(merged["c"], Value::Number(30.into()));
    }

    #[test]
    fn test_yaml_merge_conflict() {
        let driver = YamlDriver::new();
        let base = "key: original\n";
        let ours = "key: ours\n";
        let theirs = "key: theirs\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_yaml_merge_both_add_different_keys() {
        let driver = YamlDriver::new();
        let base = "shared: true\n";
        let ours = "shared: true\nadded_by_ours: yes\n";
        let theirs = "shared: true\nadded_by_theirs: yes\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_yaml::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["shared"], Value::Bool(true));
        assert_eq!(merged["added_by_ours"], Value::String("yes".into()));
        assert_eq!(merged["added_by_theirs"], Value::String("yes".into()));
    }

    #[test]
    fn test_yaml_merge_both_add_same_key() {
        let driver = YamlDriver::new();
        let base = "existing: ok\n";
        let ours = "existing: ok\nnew_key: from_ours\n";
        let theirs = "existing: ok\nnew_key: from_theirs\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_yaml_merge_nested() {
        let driver = YamlDriver::new();
        let base = "server:\n  host: localhost\n  port: 8080\n";
        let ours = "server:\n  host: 0.0.0.0\n  port: 8080\n";
        let theirs = "server:\n  host: localhost\n  port: 9090\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_yaml::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["server"]["host"], Value::String("0.0.0.0".into()));
        assert_eq!(merged["server"]["port"], Value::Number(9090.into()));
    }
}

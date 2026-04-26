#![allow(clippy::collapsible_match)]
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

    fn merge_values(
        base: &Value,
        ours: &Value,
        theirs: &Value,
    ) -> Result<Option<Value>, DriverError> {
        match (base, ours, theirs) {
            (Value::Object(base_map), Value::Object(ours_map), Value::Object(theirs_map)) => {
                let base_keys: std::collections::HashSet<&str> =
                    base_map.keys().map(|s| s.as_str()).collect();
                let ours_keys: std::collections::HashSet<&str> =
                    ours_map.keys().map(|s| s.as_str()).collect();
                let theirs_keys: std::collections::HashSet<&str> =
                    theirs_map.keys().map(|s| s.as_str()).collect();

                let all_keys: std::collections::HashSet<&str> = base_keys
                    .iter()
                    .chain(ours_keys.iter())
                    .chain(theirs_keys.iter())
                    .copied()
                    .collect();

                let mut merged = serde_json::Map::new();

                for key in &all_keys {
                    let in_base = base_keys.contains(key);
                    let in_ours = ours_keys.contains(key);
                    let in_theirs = theirs_keys.contains(key);

                    match (in_base, in_ours, in_theirs) {
                        (true, true, false) => {
                            merged.insert((*key).to_string(), ours_map[*key].clone());
                        }
                        (true, false, true) => {
                            merged.insert((*key).to_string(), theirs_map[*key].clone());
                        }
                        (true, true, true) => {
                            let base_val = &base_map[*key];
                            let ours_val = &ours_map[*key];
                            let theirs_val = &theirs_map[*key];

                            if ours_val == theirs_val {
                                merged.insert((*key).to_string(), ours_val.clone());
                            } else if ours_val == base_val {
                                merged.insert((*key).to_string(), theirs_val.clone());
                            } else if theirs_val == base_val {
                                merged.insert((*key).to_string(), ours_val.clone());
                            } else if let Some(m) =
                                Self::merge_values(base_val, ours_val, theirs_val)?
                            {
                                merged.insert((*key).to_string(), m);
                            } else {
                                return Ok(None);
                            }
                        }
                        (false, true, true) => {
                            if ours_map[*key] == theirs_map[*key] {
                                merged.insert((*key).to_string(), ours_map[*key].clone());
                            } else {
                                return Ok(None);
                            }
                        }
                        (false, true, false) => {
                            merged.insert((*key).to_string(), ours_map[*key].clone());
                        }
                        (false, false, true) => {
                            merged.insert((*key).to_string(), theirs_map[*key].clone());
                        }
                        (true, false, false) => {}
                        (false, false, false) => {}
                    }
                }

                Ok(Some(Value::Object(merged)))
            }
            (Value::Array(base_arr), Value::Array(ours_arr), Value::Array(theirs_arr)) => {
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

                Ok(Some(Value::Array(merged)))
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

    fn merge(&self, base: &str, ours: &str, theirs: &str) -> Result<Option<String>, DriverError> {
        let base_val: Value =
            serde_json::from_str(base).map_err(|e| DriverError::ParseError(e.to_string()))?;
        let ours_val: Value =
            serde_json::from_str(ours).map_err(|e| DriverError::ParseError(e.to_string()))?;
        let theirs_val: Value =
            serde_json::from_str(theirs).map_err(|e| DriverError::ParseError(e.to_string()))?;

        match Self::merge_values(&base_val, &ours_val, &theirs_val)? {
            Some(merged) => Ok(Some(
                serde_json::to_string_pretty(&merged)
                    .map_err(|e| DriverError::SerializationError(e.to_string()))?,
            )),
            None => Ok(None),
        }
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

    #[test]
    fn test_merge_no_conflict() {
        let driver = JsonDriver::new();
        let base = r#"{"a": 1, "b": 2, "c": 3}"#;
        let ours = r#"{"a": 10, "b": 2, "c": 3}"#;
        let theirs = r#"{"a": 1, "b": 2, "c": 30}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["a"], 10);
        assert_eq!(merged["b"], 2);
        assert_eq!(merged["c"], 30);
    }

    #[test]
    fn test_merge_conflict() {
        let driver = JsonDriver::new();
        let base = r#"{"key": "original"}"#;
        let ours = r#"{"key": "ours"}"#;
        let theirs = r#"{"key": "theirs"}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_merge_both_add_different_keys() {
        let driver = JsonDriver::new();
        let base = r#"{"a": 1}"#;
        let ours = r#"{"a": 1, "x": 100}"#;
        let theirs = r#"{"a": 1, "y": 200}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["a"], 1);
        assert_eq!(merged["x"], 100);
        assert_eq!(merged["y"], 200);
    }

    #[test]
    fn test_merge_both_add_same_key() {
        let driver = JsonDriver::new();
        let base = r#"{"a": 1}"#;
        let ours = r#"{"a": 1, "x": 100}"#;
        let theirs = r#"{"a": 1, "x": 999}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_merge_nested() {
        let driver = JsonDriver::new();
        let base = r#"{"outer": {"inner": "base", "other": "keep"}}"#;
        let ours = r#"{"outer": {"inner": "ours", "other": "keep"}}"#;
        let theirs = r#"{"outer": {"inner": "base", "other": "changed"}}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["outer"]["inner"], "ours");
        assert_eq!(merged["outer"]["other"], "changed");
    }

    #[test]
    fn test_merge_identical() {
        let driver = JsonDriver::new();
        let content = r#"{"a": 1, "b": 2}"#;

        let result = driver.merge(content, content, content).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["a"], 1);
        assert_eq!(merged["b"], 2);
    }

    #[test]
    fn test_correctness_merge_determinism() {
        let driver = JsonDriver::new();
        let base = r#"{"a": 1, "b": 2, "c": 3}"#;
        let ours = r#"{"a": 10, "b": 2, "d": 4}"#;
        let theirs = r#"{"a": 1, "b": 20, "e": 5}"#;

        let r1 = driver.merge(base, ours, theirs).unwrap();
        let r2 = driver.merge(base, theirs, ours).unwrap();
        assert_eq!(r1.is_some(), r2.is_some());
        if let (Some(m1), Some(m2)) = (r1, r2) {
            let v1: Value = serde_json::from_str(&m1).unwrap();
            let v2: Value = serde_json::from_str(&m2).unwrap();
            assert_eq!(
                v1, v2,
                "merge(base, ours, theirs) must equal merge(base, theirs, ours)"
            );
        }
    }

    #[test]
    fn test_correctness_merge_idempotency() {
        let driver = JsonDriver::new();
        let base = r#"{"a": 1, "b": 2}"#;
        let ours = r#"{"a": 10, "b": 2, "c": 3}"#;

        let result = driver.merge(base, ours, ours).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        let expected: Value = serde_json::from_str(ours).unwrap();
        assert_eq!(
            merged, expected,
            "merge(base, ours, ours) should equal ours"
        );
    }

    #[test]
    fn test_correctness_base_equals_ours() {
        let driver = JsonDriver::new();
        let base = r#"{"a": 1, "b": 2}"#;
        let theirs = r#"{"a": 10, "b": 2, "c": 3}"#;

        let result = driver.merge(base, base, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        let expected: Value = serde_json::from_str(theirs).unwrap();
        assert_eq!(
            merged, expected,
            "merge(base, base, theirs) should equal theirs"
        );
    }

    #[test]
    fn test_correctness_base_equals_theirs() {
        let driver = JsonDriver::new();
        let base = r#"{"a": 1, "b": 2}"#;
        let ours = r#"{"a": 10, "b": 2, "c": 3}"#;

        let result = driver.merge(base, ours, base).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        let expected: Value = serde_json::from_str(ours).unwrap();
        assert_eq!(
            merged, expected,
            "merge(base, ours, base) should equal ours"
        );
    }

    #[test]
    fn test_correctness_all_equal() {
        let driver = JsonDriver::new();
        let content = r#"{"x": 42, "y": "hello"}"#;

        let result = driver.merge(content, content, content).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        let expected: Value = serde_json::from_str(content).unwrap();
        assert_eq!(merged, expected);
    }

    #[test]
    fn test_correctness_both_add_different_keys() {
        let driver = JsonDriver::new();
        let base = r#"{"shared": true}"#;
        let ours = r#"{"shared": true, "from_ours": 100}"#;
        let theirs = r#"{"shared": true, "from_theirs": 200}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["shared"], true);
        assert_eq!(merged["from_ours"], 100);
        assert_eq!(merged["from_theirs"], 200);
        assert_eq!(merged.as_object().unwrap().len(), 3);
    }

    #[test]
    fn test_correctness_both_modify_different_keys() {
        let driver = JsonDriver::new();
        let base = r#"{"a": 1, "b": 2, "c": 3}"#;
        let ours = r#"{"a": 10, "b": 2, "c": 3}"#;
        let theirs = r#"{"a": 1, "b": 2, "c": 30}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["a"], 10, "ours change to 'a' should be kept");
        assert_eq!(merged["c"], 30, "theirs change to 'c' should be kept");
        assert_eq!(merged["b"], 2, "unchanged key should remain");
    }

    #[test]
    fn test_correctness_both_modify_same_key_same_value() {
        let driver = JsonDriver::new();
        let base = r#"{"key": "original"}"#;
        let ours = r#"{"key": "changed"}"#;
        let theirs = r#"{"key": "changed"}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some(), "identical changes should not conflict");
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["key"], "changed");
    }

    #[test]
    fn test_correctness_both_modify_same_key_different_value() {
        let driver = JsonDriver::new();
        let base = r#"{"key": "original"}"#;
        let ours = r#"{"key": "ours"}"#;
        let theirs = r#"{"key": "theirs"}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_none(), "conflicting changes should return None");
    }

    #[test]
    fn test_correctness_deeply_nested_merge() {
        let driver = JsonDriver::new();
        let base = r#"{"level1": {"level2": {"level3": {"a": 1, "b": 2, "c": 3}}}}"#;
        let ours = r#"{"level1": {"level2": {"level3": {"a": 10, "b": 2, "c": 3}}}}"#;
        let theirs = r#"{"level1": {"level2": {"level3": {"a": 1, "b": 2, "c": 30}}}}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["level1"]["level2"]["level3"]["a"], 10);
        assert_eq!(merged["level1"]["level2"]["level3"]["c"], 30);
        assert_eq!(merged["level1"]["level2"]["level3"]["b"], 2);
    }

    #[test]
    fn test_correctness_deeply_nested_merge_different_levels() {
        let driver = JsonDriver::new();
        let base = r#"{"outer": {"inner": "base", "other": "keep"}}"#;
        let ours = r#"{"outer": {"inner": "ours", "other": "keep", "extra": 1}}"#;
        let theirs = r#"{"outer": {"inner": "base", "other": "changed"}}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["outer"]["inner"], "ours");
        assert_eq!(merged["outer"]["other"], "changed");
        assert_eq!(merged["outer"]["extra"], 1);
    }

    #[test]
    fn test_correctness_unicode_keys_and_values() {
        let driver = JsonDriver::new();
        let base = r#"{"名前": "太郎", "age": 30}"#;
        let ours = r#"{"名前": "太郎", "age": 31}"#;
        let theirs = r#"{"名前": "次郎", "age": 30}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["名前"], "次郎");
        assert_eq!(merged["age"], 31);
    }

    #[test]
    fn test_correctness_unicode_emoji_keys() {
        let driver = JsonDriver::new();
        let base = r#"{"🌍": "earth", "🚀": "rocket"}"#;
        let ours = r#"{"🌍": "earth", "🚀": "falcon"}"#;
        let theirs = r#"{"🌍": "terra", "🚀": "rocket"}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["🌍"], "terra");
        assert_eq!(merged["🚀"], "falcon");
    }

    #[test]
    fn test_correctness_large_file() {
        let driver = JsonDriver::new();
        let mut base_obj = serde_json::Map::new();
        let mut ours_obj = serde_json::Map::new();
        let mut theirs_obj = serde_json::Map::new();

        for i in 0..500 {
            let key = format!("key_{i}");
            base_obj.insert(key.clone(), Value::String(format!("value_{i}")));
            ours_obj.insert(
                key.clone(),
                if i == 100 {
                    Value::String("modified_by_ours".to_string())
                } else {
                    Value::String(format!("value_{i}"))
                },
            );
            theirs_obj.insert(
                key.clone(),
                if i == 400 {
                    Value::String("modified_by_theirs".to_string())
                } else {
                    Value::String(format!("value_{i}"))
                },
            );
        }

        let base = serde_json::to_string(&Value::Object(base_obj)).unwrap();
        let ours = serde_json::to_string(&Value::Object(ours_obj)).unwrap();
        let theirs = serde_json::to_string(&Value::Object(theirs_obj)).unwrap();

        let result = driver.merge(&base, &ours, &theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["key_100"], "modified_by_ours");
        assert_eq!(merged["key_400"], "modified_by_theirs");
        assert_eq!(merged["key_0"], "value_0");
        assert_eq!(merged["key_499"], "value_499");
        assert_eq!(merged.as_object().unwrap().len(), 500);
    }

    #[test]
    fn test_correctness_null_values() {
        let driver = JsonDriver::new();
        let base = r#"{"a": 1, "b": null, "c": "hello"}"#;
        let ours = r#"{"a": 1, "b": "not_null", "c": "hello"}"#;
        let theirs = r#"{"a": 10, "b": null, "c": "hello"}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["a"], 10, "theirs change should apply");
        assert_eq!(merged["b"], "not_null", "ours change should apply");
        assert_eq!(merged["c"], "hello");
    }

    #[test]
    fn test_correctness_null_to_value() {
        let driver = JsonDriver::new();
        let base = r#"{"key": null}"#;
        let ours = r#"{"key": null}"#;
        let theirs = r#"{"key": "now_set"}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["key"], "now_set");
    }

    #[test]
    fn test_correctness_output_validity() {
        let driver = JsonDriver::new();
        let base = r#"{"a": 1, "b": {"c": [1, 2, 3]}}"#;
        let ours = r#"{"a": 10, "b": {"c": [1, 2, 3]}}"#;
        let theirs = r#"{"a": 1, "b": {"c": [1, 2, 3], "d": "new"}}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged_str = result.unwrap();
        let merged: Value = serde_json::from_str(&merged_str)
            .unwrap_or_else(|e| panic!("merged output should be valid JSON: {e}"));
        assert_eq!(merged["a"], 10);
        assert_eq!(merged["b"]["d"], "new");
    }

    #[test]
    fn test_correctness_array_merge_positional() {
        let driver = JsonDriver::new();
        let base = r#"{"items": [1, 2, 3, 4, 5]}"#;
        let ours = r#"{"items": [10, 2, 3, 4, 5]}"#;
        let theirs = r#"{"items": [1, 2, 3, 4, 50]}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["items"][0], 10);
        assert_eq!(merged["items"][1], 2);
        assert_eq!(merged["items"][4], 50);
        assert_eq!(merged["items"].as_array().unwrap().len(), 5);
    }

    #[test]
    fn test_correctness_array_both_append() {
        let driver = JsonDriver::new();
        let base = r#"{"items": [1, 2]}"#;
        let ours = r#"{"items": [1, 2, 3]}"#;
        let theirs = r#"{"items": [1, 2, 3, 4]}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        let arr = merged["items"].as_array().unwrap();
        assert_eq!(arr.len(), 4);
        assert_eq!(arr[2], 3);
        assert_eq!(arr[3], 4);
    }

    #[test]
    fn test_correctness_array_both_append_different() {
        let driver = JsonDriver::new();
        let base = r#"{"items": [1]}"#;
        let ours = r#"{"items": [1, 2]}"#;
        let theirs = r#"{"items": [1, 3]}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(
            result.is_none(),
            "both appending different values at same index should conflict"
        );
    }

    #[test]
    fn test_correctness_array_nested_objects() {
        let driver = JsonDriver::new();
        let base = r#"{"items": [{"a": 1}, {"a": 2}]}"#;
        let ours = r#"{"items": [{"a": 10}, {"a": 2}]}"#;
        let theirs = r#"{"items": [{"a": 1}, {"a": 20}]}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["items"][0]["a"], 10);
        assert_eq!(merged["items"][1]["a"], 20);
    }

    #[test]
    fn test_correctness_number_precision() {
        let driver = JsonDriver::new();
        let base = r#"{"value": 1}"#;
        let ours = r#"{"value": 1.0}"#;
        let theirs = r#"{"value": 1}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["value"], 1.0);
    }

    #[test]
    fn test_correctness_boolean_merge() {
        let driver = JsonDriver::new();
        let base = r#"{"a": true, "b": false}"#;
        let ours = r#"{"a": false, "b": false}"#;
        let theirs = r#"{"a": true, "b": true}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["a"], false, "ours change to false should apply");
        assert_eq!(merged["b"], true, "theirs change to true should apply");
    }

    #[test]
    fn test_correctness_nested_object_3_levels_deep() {
        let driver = JsonDriver::new();
        let base = r#"{"l1": {"l2": {"l3": {"x": 0, "y": 0, "z": 0}}}}"#;
        let ours = r#"{"l1": {"l2": {"l3": {"x": 1, "y": 0, "z": 0}}}}"#;
        let theirs = r#"{"l1": {"l2": {"l3": {"x": 0, "y": 0, "z": 1}}}}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["l1"]["l2"]["l3"]["x"], 1);
        assert_eq!(merged["l1"]["l2"]["l3"]["y"], 0);
        assert_eq!(merged["l1"]["l2"]["l3"]["z"], 1);
    }

    #[test]
    fn test_correctness_key_deletion_by_ours() {
        let driver = JsonDriver::new();
        let base = r#"{"a": 1, "b": 2, "c": 3}"#;
        let ours = r#"{"a": 1, "c": 3}"#;
        let theirs = r#"{"a": 1, "b": 2, "c": 3}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(
            merged.as_object().unwrap().contains_key("b"),
            "theirs kept 'b' since ours deleted it but theirs didn't"
        );
    }

    #[test]
    fn test_correctness_key_deletion_by_both() {
        let driver = JsonDriver::new();
        let base = r#"{"a": 1, "b": 2, "c": 3}"#;
        let ours = r#"{"a": 1, "c": 3}"#;
        let theirs = r#"{"a": 1, "c": 3}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(
            !merged.as_object().unwrap().contains_key("b"),
            "both deleted 'b'"
        );
    }

    #[test]
    fn test_correctness_key_deletion_conflict() {
        let driver = JsonDriver::new();
        let base = r#"{"a": 1, "b": 2}"#;
        let ours = r#"{"a": 1}"#;
        let theirs = r#"{"a": 1, "b": 99}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(
            result.is_some(),
            "ours deletes, theirs modifies → theirs wins"
        );
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(
            merged["b"], 99,
            "theirs modification is preserved since ours didn't have the key"
        );
    }

    #[test]
    fn test_correctness_empty_objects() {
        let driver = JsonDriver::new();
        let base = r#"{}"#;
        let ours = r#"{"a": 1}"#;
        let theirs = r#"{"b": 2}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["a"], 1);
        assert_eq!(merged["b"], 2);
    }

    #[test]
    fn test_correctness_mixed_types_no_conflict() {
        let driver = JsonDriver::new();
        let base = r#"{"a": 1, "b": "hello", "c": true, "d": [1,2], "e": {"nested": true}}"#;
        let ours = r#"{"a": 10, "b": "hello", "c": true, "d": [1,2], "e": {"nested": true}}"#;
        let theirs = r#"{"a": 1, "b": "world", "c": false, "d": [1,2], "e": {"nested": true}}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["a"], 10);
        assert_eq!(merged["b"], "world");
        assert_eq!(merged["c"], false);
        assert_eq!(merged["e"]["nested"], true);
    }

    #[test]
    fn test_correctness_type_change_conflict() {
        let driver = JsonDriver::new();
        let base = r#"{"key": 42}"#;
        let ours = r#"{"key": "string"}"#;
        let theirs = r#"{"key": [1,2,3]}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(
            result.is_none(),
            "both changing type differently should conflict"
        );
    }

    #[test]
    fn test_correctness_type_change_one_side() {
        let driver = JsonDriver::new();
        let base = r#"{"key": 42}"#;
        let ours = r#"{"key": 42}"#;
        let theirs = r#"{"key": "string"}"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["key"], "string");
    }
}

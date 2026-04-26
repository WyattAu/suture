#![allow(clippy::collapsible_match)]
use suture_driver::{DriverError, SemanticChange, SutureDriver};
use toml::Value;

pub struct TomlDriver;

impl TomlDriver {
    pub fn new() -> Self {
        Self
    }

    fn diff_values(old: &Value, new: &Value, path: &str) -> Vec<SemanticChange> {
        let mut changes = Vec::new();

        match (old, new) {
            (Value::Table(old_map), Value::Table(new_map)) => {
                let old_keys: std::collections::HashSet<&str> =
                    old_map.keys().map(|s| s.as_str()).collect();
                let new_keys: std::collections::HashSet<&str> =
                    new_map.keys().map(|s| s.as_str()).collect();

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
            (Value::Array(old_arr), Value::Array(new_arr)) => {
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

    fn child_path(parent: &str, key: &str) -> String {
        if parent == "/" {
            format!("/{key}")
        } else {
            format!("{parent}/{key}")
        }
    }

    fn value_to_string(val: &Value) -> String {
        match val {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        }
    }

    fn merge_values(
        base: &Value,
        ours: &Value,
        theirs: &Value,
    ) -> Result<Option<Value>, DriverError> {
        match (base, ours, theirs) {
            (Value::Table(base_map), Value::Table(ours_map), Value::Table(theirs_map)) => {
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

                let mut merged = toml::Table::new();

                for key in &all_keys {
                    let in_base = base_keys.contains(key);
                    let in_ours = ours_keys.contains(key);
                    let in_theirs = theirs_keys.contains(key);

                    match (in_base, in_ours, in_theirs) {
                        (true, true, false) => {
                            if let Some(val) = ours_map.get(*key) {
                                merged.insert(key.to_string(), val.clone());
                            }
                        }
                        (true, false, true) => {
                            if let Some(val) = theirs_map.get(*key) {
                                merged.insert(key.to_string(), val.clone());
                            }
                        }
                        (true, true, true) => {
                            let base_val = &base_map[*key];
                            let ours_val = &ours_map[*key];
                            let theirs_val = &theirs_map[*key];

                            if ours_val == theirs_val {
                                merged.insert(key.to_string(), ours_val.clone());
                            } else if ours_val == base_val {
                                merged.insert(key.to_string(), theirs_val.clone());
                            } else if theirs_val == base_val {
                                merged.insert(key.to_string(), ours_val.clone());
                            } else if let Some(m) =
                                Self::merge_values(base_val, ours_val, theirs_val)?
                            {
                                merged.insert(key.to_string(), m);
                            } else {
                                return Ok(None);
                            }
                        }
                        (false, true, true) => {
                            if ours_map[*key] == theirs_map[*key] {
                                merged.insert(key.to_string(), ours_map[*key].clone());
                            } else {
                                return Ok(None);
                            }
                        }
                        (false, true, false) => {
                            if let Some(val) = ours_map.get(*key) {
                                merged.insert(key.to_string(), val.clone());
                            }
                        }
                        (false, false, true) => {
                            if let Some(val) = theirs_map.get(*key) {
                                merged.insert(key.to_string(), val.clone());
                            }
                        }
                        (true, false, false) | (false, false, false) => {}
                    }
                }

                Ok(Some(Value::Table(merged)))
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
                format!("  MODIFIED  {path}: {old_value} -> {new_value}")
            }
            SemanticChange::Moved {
                old_path,
                new_path,
                value,
            } => {
                format!("  MOVED     {old_path} -> {new_path}: {value}")
            }
        }
    }
}

impl Default for TomlDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl SutureDriver for TomlDriver {
    fn name(&self) -> &str {
        "TOML"
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".toml"]
    }

    fn diff(
        &self,
        base_content: Option<&str>,
        new_content: &str,
    ) -> Result<Vec<SemanticChange>, DriverError> {
        let new_val: Value = new_content
            .parse()
            .map_err(|e: toml::de::Error| DriverError::ParseError(e.to_string()))?;

        match base_content {
            None => {
                let mut changes = Vec::new();
                collect_all_paths(&new_val, "/".to_string(), &mut changes);
                Ok(changes)
            }
            Some(base) => {
                let old_val: Value = base
                    .parse()
                    .map_err(|e: toml::de::Error| DriverError::ParseError(e.to_string()))?;
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
        let base_val: Value = base
            .parse()
            .map_err(|e: toml::de::Error| DriverError::ParseError(e.to_string()))?;
        let ours_val: Value = ours
            .parse()
            .map_err(|e: toml::de::Error| DriverError::ParseError(e.to_string()))?;
        let theirs_val: Value = theirs
            .parse()
            .map_err(|e: toml::de::Error| DriverError::ParseError(e.to_string()))?;

        match Self::merge_values(&base_val, &ours_val, &theirs_val)? {
            Some(merged) => {
                Ok(Some(toml::to_string_pretty(&merged).map_err(|e| {
                    DriverError::SerializationError(e.to_string())
                })?))
            }
            None => Ok(None),
        }
    }
}

fn collect_all_paths(val: &Value, path: String, out: &mut Vec<SemanticChange>) {
    match val {
        Value::Table(map) => {
            for (key, child) in map {
                let child_path = TomlDriver::child_path(&path, key);
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
                value: TomlDriver::value_to_string(other),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toml_driver_name() {
        let driver = TomlDriver::new();
        assert_eq!(driver.name(), "TOML");
    }

    #[test]
    fn test_toml_driver_extensions() {
        let driver = TomlDriver::new();
        assert_eq!(driver.supported_extensions(), &[".toml"]);
    }

    #[test]
    fn test_toml_diff_modified() {
        let driver = TomlDriver::new();
        let old = "name = \"Alice\"\nage = 30\n";
        let new = "name = \"Bob\"\nage = 30\n";

        let changes = driver.diff(Some(old), new).unwrap();
        assert!(changes.contains(&SemanticChange::Modified {
            path: "/name".to_string(),
            old_value: "Alice".to_string(),
            new_value: "Bob".to_string(),
        }));
    }

    #[test]
    fn test_toml_diff_added() {
        let driver = TomlDriver::new();
        let old = "name = \"Alice\"\n";
        let new = "name = \"Alice\"\nemail = \"alice@example.com\"\n";

        let changes = driver.diff(Some(old), new).unwrap();
        assert!(changes.contains(&SemanticChange::Added {
            path: "/email".to_string(),
            value: "alice@example.com".to_string(),
        }));
    }

    #[test]
    fn test_toml_diff_nested() {
        let driver = TomlDriver::new();
        let old = "[server]\nhost = \"localhost\"\nport = 8080\n";
        let new = "[server]\nhost = \"0.0.0.0\"\nport = 8080\n";

        let changes = driver.diff(Some(old), new).unwrap();
        assert!(changes.contains(&SemanticChange::Modified {
            path: "/server/host".to_string(),
            old_value: "localhost".to_string(),
            new_value: "0.0.0.0".to_string(),
        }));
    }

    #[test]
    fn test_toml_merge_no_conflict() {
        let driver = TomlDriver::new();
        let base = "a = 1\nb = 2\nc = 3\n";
        let ours = "a = 10\nb = 2\nc = 3\n";
        let theirs = "a = 1\nb = 2\nc = 30\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = result.unwrap().parse().unwrap();
        assert_eq!(merged["a"], Value::Integer(10));
        assert_eq!(merged["b"], Value::Integer(2));
        assert_eq!(merged["c"], Value::Integer(30));
    }

    #[test]
    fn test_toml_merge_conflict() {
        let driver = TomlDriver::new();
        let base = "key = \"original\"\n";
        let ours = "key = \"ours\"\n";
        let theirs = "key = \"theirs\"\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_correctness_merge_determinism() {
        let driver = TomlDriver::new();
        let base = "a = 1\nb = 2\nc = 3\n";
        let ours = "a = 10\nb = 2\nd = 4\n";
        let theirs = "a = 1\nb = 20\ne = 5\n";

        let r1 = driver.merge(base, ours, theirs).unwrap();
        let r2 = driver.merge(base, theirs, ours).unwrap();
        assert_eq!(r1.is_some(), r2.is_some());
        if let (Some(m1), Some(m2)) = (r1, r2) {
            let v1: Value = m1.parse().unwrap();
            let v2: Value = m2.parse().unwrap();
            assert_eq!(v1, v2, "merge must be commutative");
        }
    }

    #[test]
    fn test_correctness_merge_idempotency() {
        let driver = TomlDriver::new();
        let base = "a = 1\nb = 2\n";
        let ours = "a = 10\nb = 2\nc = 3\n";

        let result = driver.merge(base, ours, ours).unwrap();
        assert!(result.is_some());
        let merged: Value = result.unwrap().parse().unwrap();
        let expected: Value = ours.parse().unwrap();
        assert_eq!(
            merged, expected,
            "merge(base, ours, ours) should equal ours"
        );
    }

    #[test]
    fn test_correctness_base_equals_ours() {
        let driver = TomlDriver::new();
        let base = "a = 1\nb = 2\n";
        let theirs = "a = 10\nb = 2\nc = 3\n";

        let result = driver.merge(base, base, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = result.unwrap().parse().unwrap();
        let expected: Value = theirs.parse().unwrap();
        assert_eq!(merged, expected);
    }

    #[test]
    fn test_correctness_base_equals_theirs() {
        let driver = TomlDriver::new();
        let base = "a = 1\nb = 2\n";
        let ours = "a = 10\nb = 2\nc = 3\n";

        let result = driver.merge(base, ours, base).unwrap();
        assert!(result.is_some());
        let merged: Value = result.unwrap().parse().unwrap();
        let expected: Value = ours.parse().unwrap();
        assert_eq!(merged, expected);
    }

    #[test]
    fn test_correctness_all_equal() {
        let driver = TomlDriver::new();
        let content = "x = 42\ny = \"hello\"\n";

        let result = driver.merge(content, content, content).unwrap();
        assert!(result.is_some());
        let merged: Value = result.unwrap().parse().unwrap();
        let expected: Value = content.parse().unwrap();
        assert_eq!(merged, expected);
    }

    #[test]
    fn test_correctness_both_add_different_keys() {
        let driver = TomlDriver::new();
        let base = "shared = true\n";
        let ours = "shared = true\nfrom_ours = 100\n";
        let theirs = "shared = true\nfrom_theirs = 200\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = result.unwrap().parse().unwrap();
        assert_eq!(merged["shared"], Value::Boolean(true));
        assert_eq!(merged["from_ours"], Value::Integer(100));
        assert_eq!(merged["from_theirs"], Value::Integer(200));
    }

    #[test]
    fn test_correctness_both_modify_different_keys() {
        let driver = TomlDriver::new();
        let base = "a = 1\nb = 2\nc = 3\n";
        let ours = "a = 10\nb = 2\nc = 3\n";
        let theirs = "a = 1\nb = 2\nc = 30\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = result.unwrap().parse().unwrap();
        assert_eq!(merged["a"], Value::Integer(10));
        assert_eq!(merged["c"], Value::Integer(30));
        assert_eq!(merged["b"], Value::Integer(2));
    }

    #[test]
    fn test_correctness_both_modify_same_key_same_value() {
        let driver = TomlDriver::new();
        let base = "key = \"original\"\n";
        let ours = "key = \"changed\"\n";
        let theirs = "key = \"changed\"\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some(), "identical changes should not conflict");
        let merged: Value = result.unwrap().parse().unwrap();
        assert_eq!(merged["key"], Value::String("changed".to_string()));
    }

    #[test]
    fn test_correctness_both_modify_same_key_different_value() {
        let driver = TomlDriver::new();
        let base = "key = \"original\"\n";
        let ours = "key = \"ours\"\n";
        let theirs = "key = \"theirs\"\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_correctness_deeply_nested_merge() {
        let driver = TomlDriver::new();
        let base = "[l1.l2.l3]\na = 1\nb = 2\nc = 3\n";
        let ours = "[l1.l2.l3]\na = 10\nb = 2\nc = 3\n";
        let theirs = "[l1.l2.l3]\na = 1\nb = 2\nc = 30\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = result.unwrap().parse().unwrap();
        assert_eq!(merged["l1"]["l2"]["l3"]["a"], Value::Integer(10));
        assert_eq!(merged["l1"]["l2"]["l3"]["c"], Value::Integer(30));
        assert_eq!(merged["l1"]["l2"]["l3"]["b"], Value::Integer(2));
    }

    #[test]
    fn test_correctness_unicode_keys_and_values() {
        let driver = TomlDriver::new();
        let base = "name = \"Taro\"\nage = 30\n";
        let ours = "name = \"Taro\"\nage = 31\n";
        let theirs = "name = \"Jiro\"\nage = 30\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = result.unwrap().parse().unwrap();
        assert_eq!(merged["name"], Value::String("Jiro".to_string()));
        assert_eq!(merged["age"], Value::Integer(31));
    }

    #[test]
    fn test_correctness_unicode_values_in_strings() {
        let driver = TomlDriver::new();
        let base = "greeting = \"Hello\"\nfarewell = \"Goodbye\"\n";
        let ours = "greeting = \"こんにちは\"\nfarewell = \"Goodbye\"\n";
        let theirs = "greeting = \"Hello\"\nfarewell = \"さようなら\"\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = result.unwrap().parse().unwrap();
        assert_eq!(merged["greeting"], Value::String("こんにちは".to_string()));
        assert_eq!(merged["farewell"], Value::String("さようなら".to_string()));
    }

    #[test]
    fn test_correctness_large_file() {
        let driver = TomlDriver::new();
        let mut base_lines = Vec::new();
        let mut ours_lines = Vec::new();
        let mut theirs_lines = Vec::new();

        for i in 0..500 {
            let line = format!("key_{i} = \"value_{i}\"");
            base_lines.push(line.clone());
            ours_lines.push(if i == 100 {
                format!("key_{i} = \"modified_by_ours\"")
            } else {
                line.clone()
            });
            theirs_lines.push(if i == 400 {
                format!("key_{i} = \"modified_by_theirs\"")
            } else {
                line
            });
        }

        let base = base_lines.join("\n") + "\n";
        let ours = ours_lines.join("\n") + "\n";
        let theirs = theirs_lines.join("\n") + "\n";

        let result = driver.merge(&base, &ours, &theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = result.unwrap().parse().unwrap();
        assert_eq!(
            merged["key_100"],
            Value::String("modified_by_ours".to_string())
        );
        assert_eq!(
            merged["key_400"],
            Value::String("modified_by_theirs".to_string())
        );
        assert_eq!(merged["key_0"], Value::String("value_0".to_string()));
        assert_eq!(merged["key_499"], Value::String("value_499".to_string()));
    }

    #[test]
    fn test_correctness_output_validity() {
        let driver = TomlDriver::new();
        let base = "[server]\nhost = \"localhost\"\nport = 8080\n";
        let ours = "[server]\nhost = \"0.0.0.0\"\nport = 8080\n";
        let theirs = "[server]\nhost = \"localhost\"\nport = 9090\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged_str = result.unwrap();
        let merged: Value = merged_str
            .parse()
            .unwrap_or_else(|e: toml::de::Error| panic!("merged output should be valid TOML: {e}"));
        assert_eq!(
            merged["server"]["host"],
            Value::String("0.0.0.0".to_string())
        );
        assert_eq!(merged["server"]["port"], Value::Integer(9090));
    }

    #[test]
    fn test_correctness_array_of_tables_merge() {
        let driver = TomlDriver::new();
        let base = "[[items]]\nname = \"a\"\n\n[[items]]\nname = \"b\"\n";
        let ours = "[[items]]\nname = \"x\"\n\n[[items]]\nname = \"b\"\n";
        let theirs =
            "[[items]]\nname = \"a\"\n\n[[items]]\nname = \"b\"\n\n[[items]]\nname = \"c\"\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = result.unwrap().parse().unwrap();
        let arr = merged["items"].as_array().unwrap();
        assert_eq!(arr[0]["name"], Value::String("x".to_string()));
        assert_eq!(arr[1]["name"], Value::String("b".to_string()));
        assert_eq!(arr[2]["name"], Value::String("c".to_string()));
        assert_eq!(arr.len(), 3);
    }

    #[test]
    fn test_correctness_inline_table_merge() {
        let driver = TomlDriver::new();
        let base = "point = { x = 1, y = 2 }\n";
        let ours = "point = { x = 10, y = 2 }\n";
        let theirs = "point = { x = 1, y = 20 }\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = result.unwrap().parse().unwrap();
        assert_eq!(merged["point"]["x"], Value::Integer(10));
        assert_eq!(merged["point"]["y"], Value::Integer(20));
    }

    #[test]
    fn test_correctness_dotted_key_merge() {
        let driver = TomlDriver::new();
        let base = "a.b.c = 1\na.b.d = 2\n";
        let ours = "a.b.c = 10\na.b.d = 2\n";
        let theirs = "a.b.c = 1\na.b.d = 20\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = result.unwrap().parse().unwrap();
        assert_eq!(merged["a"]["b"]["c"], Value::Integer(10));
        assert_eq!(merged["a"]["b"]["d"], Value::Integer(20));
    }

    #[test]
    fn test_correctness_boolean_merge() {
        let driver = TomlDriver::new();
        let base = "enabled = true\nverbose = false\n";
        let ours = "enabled = false\nverbose = false\n";
        let theirs = "enabled = true\nverbose = true\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = result.unwrap().parse().unwrap();
        assert_eq!(merged["enabled"], Value::Boolean(false));
        assert_eq!(merged["verbose"], Value::Boolean(true));
    }

    #[test]
    fn test_correctness_array_merge() {
        let driver = TomlDriver::new();
        let base = "ports = [8080, 8081]\n";
        let ours = "ports = [9090, 8081]\n";
        let theirs = "ports = [8080, 8081, 8082]\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = result.unwrap().parse().unwrap();
        let arr = merged["ports"].as_array().unwrap();
        assert_eq!(arr[0], Value::Integer(9090));
        assert_eq!(arr[1], Value::Integer(8081));
        assert_eq!(arr[2], Value::Integer(8082));
        assert_eq!(arr.len(), 3);
    }

    #[test]
    fn test_correctness_empty_table() {
        let driver = TomlDriver::new();
        let base = "";
        let ours = "a = 1\n";
        let theirs = "b = 2\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = result.unwrap().parse().unwrap();
        assert_eq!(merged["a"], Value::Integer(1));
        assert_eq!(merged["b"], Value::Integer(2));
    }

    #[test]
    fn test_correctness_key_deletion_by_ours() {
        let driver = TomlDriver::new();
        let base = "a = 1\nb = 2\nc = 3\n";
        let ours = "a = 1\nc = 3\n";
        let theirs = "a = 1\nb = 2\nc = 3\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = result.unwrap().parse().unwrap();
        assert_eq!(
            merged["b"],
            Value::Integer(2),
            "theirs kept 'b' since ours deleted it but theirs didn't"
        );
    }

    #[test]
    fn test_correctness_float_values() {
        let driver = TomlDriver::new();
        let base = "pi = 3.14\ne = 2.71\n";
        let ours = "pi = 3.14159\ne = 2.71\n";
        let theirs = "pi = 3.14\ne = 2.71828\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = result.unwrap().parse().unwrap();
        assert_eq!(merged["pi"], Value::Float(3.14159));
        assert_eq!(merged["e"], Value::Float(2.71828));
    }

    #[test]
    fn test_correctness_nested_table_different_sections() {
        let driver = TomlDriver::new();
        let base = "[server]\nhost = \"localhost\"\nport = 8080\n\n[database]\nhost = \"localhost\"\nport = 5432\n";
        let ours = "[server]\nhost = \"0.0.0.0\"\nport = 8080\n\n[database]\nhost = \"localhost\"\nport = 5432\n";
        let theirs = "[server]\nhost = \"localhost\"\nport = 8080\n\n[database]\nhost = \"localhost\"\nport = 5433\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = result.unwrap().parse().unwrap();
        assert_eq!(
            merged["server"]["host"],
            Value::String("0.0.0.0".to_string())
        );
        assert_eq!(merged["database"]["port"], Value::Integer(5433));
    }
}

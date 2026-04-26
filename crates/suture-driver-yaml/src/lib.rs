#![allow(clippy::collapsible_match)]
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
    fn test_yaml_diff_multiline_folded_scalar() {
        let driver = YamlDriver::new();
        let old = "ref:\n  description: >\n    This is a multiline\n    description block\n";
        let new =
            "ref:\n  description: >\n    This is a changed multiline\n    description block\n";

        let changes = driver.diff(Some(old), new).unwrap();
        assert!(
            !changes.is_empty(),
            "should detect change in folded block scalar"
        );
    }

    #[test]
    fn test_yaml_diff_multiline_literal_scalar() {
        let driver = YamlDriver::new();
        let old = "ref:\n  description: |\n    Line 1\n    Line 2\n";
        let new = "ref:\n  description: |\n    Modified Line 1\n    Line 2\n";

        let changes = driver.diff(Some(old), new).unwrap();
        assert!(
            !changes.is_empty(),
            "should detect change in literal block scalar"
        );
    }

    #[test]
    fn test_yaml_diff_folded_strip_scalar() {
        let driver = YamlDriver::new();
        let old = "ref:\n  description: >-\n    This is a multiline\n    description block\n";
        let new =
            "ref:\n  description: >-\n    This is a changed multiline\n    description block\n";

        let changes = driver.diff(Some(old), new).unwrap();
        assert!(
            !changes.is_empty(),
            "should detect change in strip folded scalar"
        );
    }

    #[test]
    fn test_yaml_diff_literal_keep_scalar() {
        let driver = YamlDriver::new();
        let old = "ref:\n  description: |+\n    Line 1\n    Line 2\n\n";
        let new = "ref:\n  description: |+\n    Modified Line 1\n    Line 2\n\n";

        let changes = driver.diff(Some(old), new).unwrap();
        assert!(
            !changes.is_empty(),
            "should detect change in keep literal scalar"
        );
    }

    #[test]
    fn test_yaml_diff_block_scalar_with_empty_lines() {
        let driver = YamlDriver::new();
        let old = "ref:\n  description: >\n    Line 1\n\n    Line 3\n";
        let new = "ref:\n  description: >\n    Line 1\n\n    Modified Line 3\n";

        let changes = driver.diff(Some(old), new).unwrap();
        assert!(
            !changes.is_empty(),
            "should detect change with empty lines in block scalar"
        );
    }

    #[test]
    fn test_yaml_diff_block_scalar_in_sequence() {
        let driver = YamlDriver::new();
        let old = "items:\n  - >\n    First item\n    continues\n  - plain\n";
        let new = "items:\n  - >\n    Modified first item\n    continues\n  - plain\n";

        let changes = driver.diff(Some(old), new).unwrap();
        assert!(
            !changes.is_empty(),
            "should detect change in block scalar within sequence"
        );
    }

    #[test]
    fn test_yaml_diff_trailing_newline_difference() {
        let driver = YamlDriver::new();
        let yaml_with_newline = "description: >\n  Hello\n  World\n";
        let yaml_without_newline = "description: >\n  Hello\n  World";

        let changes = driver
            .diff(Some(yaml_with_newline), yaml_without_newline)
            .unwrap();
        assert!(
            !changes.is_empty(),
            "trailing newline difference should be detected"
        );
    }

    #[test]
    fn test_yaml_diff_block_scalar_value_change_vs_no_change() {
        let driver = YamlDriver::new();
        let old = "ref:\n  description: |\n    Line 1\n    Line 2\n";
        let same = "ref:\n  description: |\n    Line 1\n    Line 2\n";
        let modified = "ref:\n  description: |\n    Line 1\n    Line 2 modified\n";

        let no_changes = driver.diff(Some(old), same).unwrap();
        assert!(
            no_changes.is_empty(),
            "identical block scalars should have no changes"
        );

        let with_changes = driver.diff(Some(old), modified).unwrap();
        assert!(
            !with_changes.is_empty(),
            "modified block scalar text should be detected"
        );
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

    #[test]
    fn test_correctness_merge_determinism() {
        let driver = YamlDriver::new();
        let base = "a: 1\nb: 2\nc: 3\n";
        let ours = "a: 10\nb: 2\nd: 4\n";
        let theirs = "a: 1\nb: 20\ne: 5\n";

        let r1 = driver.merge(base, ours, theirs).unwrap();
        let r2 = driver.merge(base, theirs, ours).unwrap();
        assert_eq!(r1.is_some(), r2.is_some());
        if let (Some(m1), Some(m2)) = (r1, r2) {
            let v1: Value = serde_yaml::from_str(&m1).unwrap();
            let v2: Value = serde_yaml::from_str(&m2).unwrap();
            assert_eq!(v1, v2, "merge must be commutative");
        }
    }

    #[test]
    fn test_correctness_merge_idempotency() {
        let driver = YamlDriver::new();
        let base = "a: 1\nb: 2\n";
        let ours = "a: 10\nb: 2\nc: 3\n";

        let result = driver.merge(base, ours, ours).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_yaml::from_str(&result.unwrap()).unwrap();
        let expected: Value = serde_yaml::from_str(ours).unwrap();
        assert_eq!(
            merged, expected,
            "merge(base, ours, ours) should equal ours"
        );
    }

    #[test]
    fn test_correctness_base_equals_ours() {
        let driver = YamlDriver::new();
        let base = "a: 1\nb: 2\n";
        let theirs = "a: 10\nb: 2\nc: 3\n";

        let result = driver.merge(base, base, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_yaml::from_str(&result.unwrap()).unwrap();
        let expected: Value = serde_yaml::from_str(theirs).unwrap();
        assert_eq!(merged, expected);
    }

    #[test]
    fn test_yaml_merge_deep_nested_both_modify() {
        // Reproduces: team-a adds labels to web, team-b adds healthcheck to api
        // After both merges, the file should contain BOTH changes
        let driver = YamlDriver::new();
        let base = r#"services:
  web:
    image: nginx:latest
    ports:
      - "80:80"
  api:
    image: myapp:1.0
    ports:
      - "3000:3000"
  db:
    image: postgres:15
    environment:
      POSTGRES_PASSWORD: pass
      POSTGRES_DB: myapp
"#;

        let ours = r#"services:
  web:
    image: nginx:latest
    ports:
      - "80:80"
    labels:
      traefik.enable: 'true'
    restart: unless-stopped
  api:
    image: myapp:1.0
    ports:
      - "3000:3000"
  db:
    image: postgres:15
    environment:
      POSTGRES_PASSWORD: pass
      POSTGRES_DB: myapp
"#;

        let theirs = r#"services:
  web:
    image: nginx:latest
    ports:
      - "80:80"
  api:
    image: myapp:1.0
    ports:
      - "3000:3000"
    healthcheck:
      test: curl -f http://localhost:3000/health
      interval: 30s
  db:
    image: postgres:15
    environment:
      POSTGRES_PASSWORD: rotated-2024
      POSTGRES_DB: myapp
"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some(), "deep nested merge should succeed");
        let merged: Value = serde_yaml::from_str(&result.unwrap()).unwrap();

        // Team-a's changes must be preserved
        assert!(
            merged["services"]["web"].get("labels").is_some(),
            "web.labels from team-a should be preserved"
        );
        assert!(
            merged["services"]["web"].get("restart").is_some(),
            "web.restart from team-a should be preserved"
        );

        // Team-b's changes must be preserved
        assert!(
            merged["services"]["api"].get("healthcheck").is_some(),
            "api.healthcheck from team-b should be preserved"
        );
        assert_eq!(
            merged["services"]["db"]["environment"]["POSTGRES_PASSWORD"],
            Value::String("rotated-2024".into()),
            "db password rotation from team-b should be preserved"
        );

        // All original services must still exist
        assert!(
            merged["services"].get("web").is_some(),
            "web service should exist"
        );
        assert!(
            merged["services"].get("api").is_some(),
            "api service should exist"
        );
        assert!(
            merged["services"].get("db").is_some(),
            "db service should exist"
        );
    }

    #[test]
    fn test_correctness_base_equals_theirs() {
        let driver = YamlDriver::new();
        let base = "a: 1\nb: 2\n";
        let ours = "a: 10\nb: 2\nc: 3\n";

        let result = driver.merge(base, ours, base).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_yaml::from_str(&result.unwrap()).unwrap();
        let expected: Value = serde_yaml::from_str(ours).unwrap();
        assert_eq!(merged, expected);
    }

    #[test]
    fn test_correctness_all_equal() {
        let driver = YamlDriver::new();
        let content = "x: 42\ny: hello\n";

        let result = driver.merge(content, content, content).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_yaml::from_str(&result.unwrap()).unwrap();
        let expected: Value = serde_yaml::from_str(content).unwrap();
        assert_eq!(merged, expected);
    }

    #[test]
    fn test_correctness_both_add_different_keys() {
        let driver = YamlDriver::new();
        let base = "shared: true\n";
        let ours = "shared: true\nfrom_ours: 100\n";
        let theirs = "shared: true\nfrom_theirs: 200\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_yaml::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["shared"], Value::Bool(true));
        assert_eq!(merged["from_ours"], Value::Number(100.into()));
        assert_eq!(merged["from_theirs"], Value::Number(200.into()));
    }

    #[test]
    fn test_correctness_both_modify_different_keys() {
        let driver = YamlDriver::new();
        let base = "a: 1\nb: 2\nc: 3\n";
        let ours = "a: 10\nb: 2\nc: 3\n";
        let theirs = "a: 1\nb: 2\nc: 30\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_yaml::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["a"], Value::Number(10.into()));
        assert_eq!(merged["c"], Value::Number(30.into()));
        assert_eq!(merged["b"], Value::Number(2.into()));
    }

    #[test]
    fn test_correctness_both_modify_same_key_same_value() {
        let driver = YamlDriver::new();
        let base = "key: original\n";
        let ours = "key: changed\n";
        let theirs = "key: changed\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some(), "identical changes should not conflict");
        let merged: Value = serde_yaml::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["key"], Value::String("changed".into()));
    }

    #[test]
    fn test_correctness_both_modify_same_key_different_value() {
        let driver = YamlDriver::new();
        let base = "key: original\n";
        let ours = "key: ours\n";
        let theirs = "key: theirs\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_correctness_deeply_nested_merge() {
        let driver = YamlDriver::new();
        let base = "l1:\n  l2:\n    l3:\n      a: 1\n      b: 2\n      c: 3\n";
        let ours = "l1:\n  l2:\n    l3:\n      a: 10\n      b: 2\n      c: 3\n";
        let theirs = "l1:\n  l2:\n    l3:\n      a: 1\n      b: 2\n      c: 30\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_yaml::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["l1"]["l2"]["l3"]["a"], Value::Number(10.into()));
        assert_eq!(merged["l1"]["l2"]["l3"]["c"], Value::Number(30.into()));
        assert_eq!(merged["l1"]["l2"]["l3"]["b"], Value::Number(2.into()));
    }

    #[test]
    fn test_correctness_unicode_keys_and_values() {
        let driver = YamlDriver::new();
        let base = "名前: 太郎\nage: 30\n";
        let ours = "名前: 太郎\nage: 31\n";
        let theirs = "名前: 次郎\nage: 30\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_yaml::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["名前"], Value::String("次郎".into()));
        assert_eq!(merged["age"], Value::Number(31.into()));
    }

    #[test]
    fn test_correctness_large_file() {
        let driver = YamlDriver::new();
        let mut base_lines = Vec::new();
        let mut ours_lines = Vec::new();
        let mut theirs_lines = Vec::new();

        for i in 0..500 {
            let line = format!("key_{i}: value_{i}");
            base_lines.push(line.clone());
            ours_lines.push(if i == 100 {
                format!("key_{i}: modified_by_ours")
            } else {
                line.clone()
            });
            theirs_lines.push(if i == 400 {
                format!("key_{i}: modified_by_theirs")
            } else {
                line
            });
        }

        let base = base_lines.join("\n") + "\n";
        let ours = ours_lines.join("\n") + "\n";
        let theirs = theirs_lines.join("\n") + "\n";

        let result = driver.merge(&base, &ours, &theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_yaml::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["key_100"], Value::String("modified_by_ours".into()));
        assert_eq!(
            merged["key_400"],
            Value::String("modified_by_theirs".into())
        );
        assert_eq!(merged["key_0"], Value::String("value_0".into()));
        assert_eq!(merged["key_499"], Value::String("value_499".into()));
    }

    #[test]
    fn test_correctness_null_values() {
        let driver = YamlDriver::new();
        let base = "a: 1\nb: ~\nc: hello\n";
        let ours = "a: 1\nb: not_null\nc: hello\n";
        let theirs = "a: 10\nb: ~\nc: hello\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_yaml::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["a"], Value::Number(10.into()));
        assert_eq!(merged["b"], Value::String("not_null".into()));
        assert_eq!(merged["c"], Value::String("hello".into()));
    }

    #[test]
    fn test_correctness_output_validity() {
        let driver = YamlDriver::new();
        let base = "a: 1\nb: 2\n";
        let ours = "a: 10\nb: 2\n";
        let theirs = "a: 1\nb: 20\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged_str = result.unwrap();
        let merged: Value = serde_yaml::from_str(&merged_str)
            .unwrap_or_else(|e| panic!("merged output should be valid YAML: {e}"));
        assert_eq!(merged["a"], Value::Number(10.into()));
        assert_eq!(merged["b"], Value::Number(20.into()));
    }

    #[test]
    fn test_correctness_list_merge() {
        let driver = YamlDriver::new();
        let base = "items:\n  - a\n  - b\n";
        let ours = "items:\n  - x\n  - b\n";
        let theirs = "items:\n  - a\n  - b\n  - c\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_yaml::from_str(&result.unwrap()).unwrap();
        let arr = merged["items"].as_sequence().unwrap();
        assert_eq!(arr[0], Value::String("x".into()));
        assert_eq!(arr[1], Value::String("b".into()));
        assert_eq!(arr[2], Value::String("c".into()));
        assert_eq!(arr.len(), 3);
    }

    #[test]
    fn test_correctness_boolean_merge() {
        let driver = YamlDriver::new();
        let base = "enabled: true\nverbose: false\n";
        let ours = "enabled: false\nverbose: false\n";
        let theirs = "enabled: true\nverbose: true\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_yaml::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["enabled"], Value::Bool(false));
        assert_eq!(merged["verbose"], Value::Bool(true));
    }

    #[test]
    fn test_correctness_empty_mapping() {
        let driver = YamlDriver::new();
        let base = "{}\n";
        let ours = "a: 1\n";
        let theirs = "b: 2\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_yaml::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["a"], Value::Number(1.into()));
        assert_eq!(merged["b"], Value::Number(2.into()));
    }

    #[test]
    fn test_correctness_key_deletion_by_ours() {
        let driver = YamlDriver::new();
        let base = "a: 1\nb: 2\nc: 3\n";
        let ours = "a: 1\nc: 3\n";
        let theirs = "a: 1\nb: 2\nc: 3\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_yaml::from_str(&result.unwrap()).unwrap();
        assert_eq!(
            merged[&Value::String("b".into())],
            Value::Number(2.into()),
            "theirs kept 'b' since ours deleted it but theirs didn't"
        );
    }

    #[test]
    fn test_correctness_multi_document_fails_gracefully() {
        let driver = YamlDriver::new();
        let base = "---\na: 1\n---\nb: 2\n";
        let ours = "---\na: 1\n---\nb: 2\n";
        let theirs = "---\na: 1\n---\nb: 2\n";

        let result = driver.merge(base, ours, theirs);
        match result {
            Ok(_) => {}
            Err(_) => {}
        }
    }

    #[test]
    fn test_correctness_nested_different_levels() {
        let driver = YamlDriver::new();
        let base = "server:\n  host: localhost\n  port: 8080\n  ssl:\n    enabled: false\n";
        let ours = "server:\n  host: 0.0.0.0\n  port: 8080\n  ssl:\n    enabled: false\n";
        let theirs = "server:\n  host: localhost\n  port: 8080\n  ssl:\n    enabled: true\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged: Value = serde_yaml::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["server"]["host"], Value::String("0.0.0.0".into()));
        assert_eq!(merged["server"]["ssl"]["enabled"], Value::Bool(true));
        assert_eq!(merged["server"]["port"], Value::Number(8080.into()));
    }
}

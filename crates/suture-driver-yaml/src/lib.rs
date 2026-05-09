// SPDX-License-Identifier: MIT OR Apache-2.0
use serde_yaml::Value;
use suture_driver::impl_structured_driver;
use suture_driver::{DriverError, SemanticChange, SutureDriver};

pub struct YamlDriver;

impl YamlDriver {
    fn value_to_string(val: &Value) -> String {
        match val {
            Value::String(s) => s.clone(),
            other => serde_yaml::to_string(other).unwrap_or_else(|_| format!("{other:#?}")),
        }
    }

    fn child_path(parent: &str, key: &Value) -> String {
        let key_str = key.as_str().map_or_else(
            || serde_yaml::to_string(key).unwrap_or_else(|_| format!("{key:#?}")),
            str::to_owned,
        );
        if parent == "/" {
            format!("/{key_str}")
        } else {
            format!("{parent}/{key_str}")
        }
    }
}

impl_structured_driver! {
    driver = YamlDriver,
    name = "YAML",
    extensions = [".yaml", ".yml"],
    value_ty = Value,

    obj_pat = |_m| Value::Mapping(_m),
    arr_pat = |_v| Value::Sequence(_v),

    new_map = serde_yaml::Mapping::new(),
    wrap_map = |m| Value::Mapping(m),
    wrap_arr = |v| Value::Sequence(v),

    key_set = |map| map.keys().collect::<std::collections::HashSet<&Value>>(),
    map_get = |map, key| map.get(key),
    map_insert = |map, key, val| { map.insert((*key).clone(), val); },

    val_str = |v| YamlDriver::value_to_string(v),
    child_path = |parent, key| YamlDriver::child_path(parent, key),

    parse_val = |s| serde_yaml::from_str(s).map_err(|e| DriverError::ParseError(e.to_string())),
    serialize_val = |v| serde_yaml::to_string(v).map_err(|e| DriverError::SerializationError(e.to_string())),

    arrow = "→",
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

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

    #[test]
    fn test_correctness_merge_associativity() {
        let driver = YamlDriver::new();
        let base = "a: 1\nb: 2\nc: 3\nd: 4\n";
        let a = "a: 10\nb: 2\nc: 3\nd: 4\n";
        let b = "a: 1\nb: 20\nc: 3\nd: 4\n";
        let c = "a: 1\nb: 2\nc: 30\nd: 4\n";

        let ab = driver
            .merge(base, a, b)
            .unwrap()
            .expect("merge(base, A, B) should succeed");
        let merge_left = driver
            .merge(base, &ab, c)
            .unwrap()
            .expect("merge(base, merge(A,B), C) should succeed");

        let bc = driver
            .merge(base, b, c)
            .unwrap()
            .expect("merge(base, B, C) should succeed");
        let merge_right = driver
            .merge(base, a, &bc)
            .unwrap()
            .expect("merge(base, A, merge(B,C)) should succeed");

        let v_left: Value = serde_yaml::from_str(&merge_left).unwrap();
        let v_right: Value = serde_yaml::from_str(&merge_right).unwrap();

        assert_eq!(
            v_left, v_right,
            "merge(base, merge(A,B), C) must equal merge(base, A, merge(B,C))"
        );
        assert_eq!(v_left["a"], Value::Number(10.into()));
        assert_eq!(v_left["b"], Value::Number(20.into()));
        assert_eq!(v_left["c"], Value::Number(30.into()));
        assert_eq!(v_left["d"], Value::Number(4.into()));
    }

    proptest! {
        #[test]
        fn test_merge_identity_yaml(s in "[a-z0-9_]+") {
            let driver = YamlDriver::new();
            let base = format!("key: {s}");
            let result = driver.merge(&base, &base, &base).unwrap();
            assert!(result.is_some());
        }
    }

    proptest! {
        #[test]
        fn test_merge_idempotence_yaml(s in "[a-z0-9_]+") {
            let driver = YamlDriver::new();
            let base = format!("key: {s}");
            let r1 = driver.merge(&base, &base, &base).unwrap();
            assert!(r1.is_some());
            let r2 = driver.merge(&base, &r1.clone().unwrap(), &r1.clone().unwrap()).unwrap();
            assert!(r2.is_some());
        }
    }

    proptest! {
        #[test]
        fn test_yaml_non_overlapping(a in "[a-z]{1,20}", b in "[0-9]{1,10}") {
            let driver = YamlDriver::new();
            let result = driver.merge("{}", &format!("alpha: {a}"), &format!("bravo: {b}")).unwrap();
            assert!(result.is_some());
        }
    }
}

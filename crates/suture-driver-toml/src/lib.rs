// SPDX-License-Identifier: MIT OR Apache-2.0
use suture_driver::impl_structured_driver;
use suture_driver::{DriverError, SemanticChange, SutureDriver};
use toml::Value;

pub struct TomlDriver;

impl TomlDriver {
    fn value_to_string(val: &Value) -> String {
        match val {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        }
    }

    fn child_path(parent: &str, key: &str) -> String {
        if parent == "/" {
            format!("/{key}")
        } else {
            format!("{parent}/{key}")
        }
    }
}

impl_structured_driver! {
    driver = TomlDriver,
    name = "TOML",
    extensions = [".toml"],
    value_ty = Value,

    obj_pat = |_m| Value::Table(_m),
    arr_pat = |_v| Value::Array(_v),

    new_map = toml::Table::new(),
    wrap_map = |m| Value::Table(m),
    wrap_arr = |v| Value::Array(v),

    key_set = |map| map.keys().map(std::string::String::as_str).collect::<std::collections::HashSet<&str>>(),
    map_get = |map, key| map.get(*key),
    map_insert = |map, key, val| { map.insert(key.to_string(), val); },

    val_str = |v| TomlDriver::value_to_string(v),
    child_path = |parent, key| TomlDriver::child_path(parent, key),

    parse_val = |s| s.parse::<Value>().map_err(|e: toml::de::Error| DriverError::ParseError(e.to_string())),
    serialize_val = |v| toml::to_string_pretty(v).map_err(|e| DriverError::SerializationError(e.to_string())),

    arrow = "->",
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

    #[test]
    fn test_correctness_merge_associativity() {
        let driver = TomlDriver::new();
        let base = "a = 1\nb = 2\nc = 3\nd = 4\n";
        let a = "a = 10\nb = 2\nc = 3\nd = 4\n";
        let b = "a = 1\nb = 20\nc = 3\nd = 4\n";
        let c = "a = 1\nb = 2\nc = 30\nd = 4\n";

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

        let v_left: Value = merge_left.parse().unwrap();
        let v_right: Value = merge_right.parse().unwrap();

        assert_eq!(
            v_left, v_right,
            "merge(base, merge(A,B), C) must equal merge(base, A, merge(B,C))"
        );
        assert_eq!(v_left["a"], Value::Integer(10));
        assert_eq!(v_left["b"], Value::Integer(20));
        assert_eq!(v_left["c"], Value::Integer(30));
        assert_eq!(v_left["d"], Value::Integer(4));
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
#![allow(clippy::collapsible_match)]
use std::collections::{HashMap, HashSet};

use suture_driver::{DriverError, SemanticChange, SutureDriver};

pub struct CsvDriver;

impl CsvDriver {
    pub fn new() -> Self {
        Self
    }

    fn parse_csv(content: &str) -> Result<(Vec<String>, Vec<Vec<String>>), DriverError> {
        let mut reader = csv::ReaderBuilder::new()
            .flexible(true)
            .from_reader(content.as_bytes());
        let headers = reader
            .headers()
            .map_err(|e| DriverError::ParseError(e.to_string()))?
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        let mut rows = Vec::new();
        for result in reader.records() {
            let record = result.map_err(|e| DriverError::ParseError(e.to_string()))?;
            rows.push(record.iter().map(|s| s.to_string()).collect());
        }
        Ok((headers, rows))
    }

    fn row_key(row: &[String], occurrence: usize) -> String {
        let base = row.first().map(|s| s.as_str()).unwrap_or("");
        if occurrence == 0 {
            base.to_string()
        } else {
            format!("{base}__dup{occurrence}")
        }
    }

    fn build_keyed_rows(rows: &[Vec<String>]) -> (Vec<String>, HashMap<String, Vec<String>>) {
        let mut order = Vec::new();
        let mut map = HashMap::new();
        let mut counts: HashMap<String, usize> = HashMap::new();
        for row in rows {
            let raw = row.first().cloned().unwrap_or_default();
            let count = counts.entry(raw).or_insert(0);
            let key = Self::row_key(row, *count);
            *count += 1;
            order.push(key.clone());
            map.insert(key, row.clone());
        }
        (order, map)
    }

    fn diff_rows(
        old_headers: &[String],
        new_headers: &[String],
        old_rows: &[Vec<String>],
        new_rows: &[Vec<String>],
    ) -> Vec<SemanticChange> {
        let mut changes = Vec::new();

        let old_headers_set: HashSet<&str> = old_headers.iter().map(|s| s.as_str()).collect();
        let new_headers_set: HashSet<&str> = new_headers.iter().map(|s| s.as_str()).collect();

        for header in old_headers {
            if !new_headers_set.contains(header.as_str()) {
                changes.push(SemanticChange::Removed {
                    path: format!("/headers/{header}"),
                    old_value: header.clone(),
                });
            }
        }

        for header in new_headers {
            if !old_headers_set.contains(header.as_str()) {
                changes.push(SemanticChange::Added {
                    path: format!("/headers/{header}"),
                    value: header.clone(),
                });
            }
        }

        let common_headers: Vec<&String> = new_headers
            .iter()
            .filter(|h| old_headers_set.contains(h.as_str()))
            .collect();

        let (old_order, old_map) = Self::build_keyed_rows(old_rows);
        let (new_order, new_map) = Self::build_keyed_rows(new_rows);

        let old_keys: HashSet<&str> = old_order.iter().map(|s| s.as_str()).collect();
        let new_keys: HashSet<&str> = new_order.iter().map(|s| s.as_str()).collect();

        for key in &old_order {
            if !new_keys.contains(key.as_str())
                && let Some(row) = old_map.get(key)
            {
                changes.push(SemanticChange::Removed {
                    path: format!("/rows:{key}"),
                    old_value: row.join(","),
                });
            }
        }

        for key in &new_order {
            if !old_keys.contains(key.as_str())
                && let Some(row) = new_map.get(key)
            {
                changes.push(SemanticChange::Added {
                    path: format!("/rows:{key}"),
                    value: row.join(","),
                });
            }
        }

        for key in &old_order {
            if let (Some(old_row), Some(new_row)) = (old_map.get(key), new_map.get(key)) {
                for (col_idx, col_name) in common_headers.iter().enumerate() {
                    let old_val = old_row.get(col_idx).map(|s| s.as_str()).unwrap_or("");
                    let new_val = new_row.get(col_idx).map(|s| s.as_str()).unwrap_or("");

                    if old_val != new_val {
                        changes.push(SemanticChange::Modified {
                            path: format!("/{col_name}:{key}"),
                            old_value: old_val.to_string(),
                            new_value: new_val.to_string(),
                        });
                    }
                }
            }
        }

        changes
    }

    fn merge_csv(
        base_headers: &[String],
        base_rows: &[Vec<String>],
        ours_headers: &[String],
        ours_rows: &[Vec<String>],
        theirs_headers: &[String],
        theirs_rows: &[Vec<String>],
    ) -> Result<Option<String>, DriverError> {
        let base_set: HashSet<&str> = base_headers.iter().map(|s| s.as_str()).collect();
        let ours_set: HashSet<&str> = ours_headers.iter().map(|s| s.as_str()).collect();
        let theirs_set: HashSet<&str> = theirs_headers.iter().map(|s| s.as_str()).collect();

        let mut merged_headers: Vec<String> = Vec::new();
        let mut header_seen: HashSet<&str> = HashSet::new();

        for headers in [base_headers, ours_headers, theirs_headers] {
            for h in headers.iter() {
                if header_seen.insert(h.as_str()) {
                    let in_base = base_set.contains(h.as_str());
                    let in_ours = ours_set.contains(h.as_str());
                    let in_theirs = theirs_set.contains(h.as_str());
                    match (in_base, in_ours, in_theirs) {
                        (true, false, false) | (false, false, false) => {
                            header_seen.remove(h.as_str());
                        }
                        _ => merged_headers.push(h.clone()),
                    }
                }
            }
        }

        let (base_order, base_map) = Self::build_keyed_rows(base_rows);
        let (ours_order, ours_map) = Self::build_keyed_rows(ours_rows);
        let (theirs_order, theirs_map) = Self::build_keyed_rows(theirs_rows);

        let base_keys: HashSet<&str> = base_order.iter().map(|s| s.as_str()).collect();
        let ours_keys: HashSet<&str> = ours_order.iter().map(|s| s.as_str()).collect();
        let theirs_keys: HashSet<&str> = theirs_order.iter().map(|s| s.as_str()).collect();

        let mut key_order: Vec<String> = Vec::new();
        let mut seen_keys: HashSet<String> = HashSet::new();
        for key in base_order
            .iter()
            .chain(ours_order.iter())
            .chain(theirs_order.iter())
        {
            if seen_keys.insert(key.clone()) {
                key_order.push(key.clone());
            }
        }

        let mut merged_rows: Vec<Vec<String>> = Vec::new();

        for key in &key_order {
            let in_base = base_keys.contains(key.as_str());
            let in_ours = ours_keys.contains(key.as_str());
            let in_theirs = theirs_keys.contains(key.as_str());

            match (in_base, in_ours, in_theirs) {
                (true, true, true) => {
                    let b = &base_map[key];
                    let o = &ours_map[key];
                    let t = &theirs_map[key];
                    if o == t {
                        merged_rows.push(o.clone());
                    } else {
                        let max_cols = b.len().max(o.len()).max(t.len());
                        let mut merged_row = Vec::new();
                        for col in 0..max_cols {
                            let bv = b.get(col).map(|s| s.as_str()).unwrap_or("");
                            let ov = o.get(col).map(|s| s.as_str()).unwrap_or("");
                            let tv = t.get(col).map(|s| s.as_str()).unwrap_or("");
                            if ov == tv {
                                merged_row.push(ov.to_string());
                            } else if ov == bv {
                                merged_row.push(tv.to_string());
                            } else if tv == bv {
                                merged_row.push(ov.to_string());
                            } else {
                                return Ok(None);
                            }
                        }
                        merged_rows.push(merged_row);
                    }
                }
                (true, true, false) => {
                    let b = &base_map[key];
                    let o = &ours_map[key];
                    if o == b {
                    } else {
                        return Ok(None);
                    }
                }
                (true, false, true) => {
                    let b = &base_map[key];
                    let t = &theirs_map[key];
                    if t == b {
                    } else {
                        return Ok(None);
                    }
                }
                (true, false, false) => {}
                (false, true, false) => {
                    merged_rows.push(ours_map[key].clone());
                }
                (false, false, true) => {
                    merged_rows.push(theirs_map[key].clone());
                }
                (false, true, true) => {
                    let o = &ours_map[key];
                    let t = &theirs_map[key];
                    if o == t {
                        merged_rows.push(o.clone());
                    } else {
                        return Ok(None);
                    }
                }
                (false, false, false) => {}
            }
        }

        let mut output = csv::WriterBuilder::new().from_writer(vec![]);
        output
            .write_record(&merged_headers)
            .map_err(|e| DriverError::SerializationError(e.to_string()))?;
        for row in &merged_rows {
            output
                .write_record(row)
                .map_err(|e| DriverError::SerializationError(e.to_string()))?;
        }
        let bytes = output
            .into_inner()
            .map_err(|e| DriverError::SerializationError(e.to_string()))?;
        Ok(Some(String::from_utf8_lossy(&bytes).to_string()))
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

impl Default for CsvDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl SutureDriver for CsvDriver {
    fn name(&self) -> &str {
        "CSV"
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".csv", ".tsv"]
    }

    fn diff(
        &self,
        base_content: Option<&str>,
        new_content: &str,
    ) -> Result<Vec<SemanticChange>, DriverError> {
        let (new_headers, new_rows) = Self::parse_csv(new_content)?;

        match base_content {
            None => {
                let mut changes = Vec::new();
                for header in &new_headers {
                    changes.push(SemanticChange::Added {
                        path: format!("/headers/{header}"),
                        value: header.clone(),
                    });
                }
                for (i, row) in new_rows.iter().enumerate() {
                    let key = Self::row_key(row, 0);
                    changes.push(SemanticChange::Added {
                        path: format!("/rows:{key}"),
                        value: row.join(","),
                    });
                    let _ = i;
                }
                Ok(changes)
            }
            Some(base) => {
                let (old_headers, old_rows) = Self::parse_csv(base)?;
                Ok(Self::diff_rows(
                    &old_headers,
                    &new_headers,
                    &old_rows,
                    &new_rows,
                ))
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
        let (base_headers, base_rows) = Self::parse_csv(base)?;
        let (ours_headers, ours_rows) = Self::parse_csv(ours)?;
        let (theirs_headers, theirs_rows) = Self::parse_csv(theirs)?;
        Self::merge_csv(
            &base_headers,
            &base_rows,
            &ours_headers,
            &ours_rows,
            &theirs_headers,
            &theirs_rows,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::proptest;
    use proptest::prop_assert;

    #[test]
    fn test_csv_driver_name() {
        let driver = CsvDriver::new();
        assert_eq!(driver.name(), "CSV");
    }

    #[test]
    fn test_csv_driver_extensions() {
        let driver = CsvDriver::new();
        assert_eq!(driver.supported_extensions(), &[".csv", ".tsv"]);
    }

    #[test]
    fn test_csv_diff_cell_change() {
        let driver = CsvDriver::new();
        let old = "name,email\nAlice,alice@old.com\nBob,bob@example.com\n";
        let new = "name,email\nAlice,alice@new.com\nBob,bob@example.com\n";

        let changes = driver.diff(Some(old), new).unwrap();
        assert!(changes.contains(&SemanticChange::Modified {
            path: "/email:Alice".to_string(),
            old_value: "alice@old.com".to_string(),
            new_value: "alice@new.com".to_string(),
        }));
    }

    #[test]
    fn test_csv_diff_added_row() {
        let driver = CsvDriver::new();
        let old = "name,email\nAlice,alice@example.com\n";
        let new = "name,email\nAlice,alice@example.com\nBob,bob@example.com\n";

        let changes = driver.diff(Some(old), new).unwrap();
        assert!(changes.contains(&SemanticChange::Added {
            path: "/rows:Bob".to_string(),
            value: "Bob,bob@example.com".to_string(),
        }));
    }

    #[test]
    fn test_csv_diff_removed_row() {
        let driver = CsvDriver::new();
        let old = "name,email\nAlice,alice@example.com\nBob,bob@example.com\n";
        let new = "name,email\nAlice,alice@example.com\n";

        let changes = driver.diff(Some(old), new).unwrap();
        assert!(changes.contains(&SemanticChange::Removed {
            path: "/rows:Bob".to_string(),
            old_value: "Bob,bob@example.com".to_string(),
        }));
    }

    #[test]
    fn test_csv_merge_no_conflict() {
        let driver = CsvDriver::new();
        let base = "name,email,age\nAlice,alice@example.com,30\nBob,bob@example.com,25\n";
        let ours = "name,email,age\nAlice,alice@new.com,30\nBob,bob@example.com,25\n";
        let theirs = "name,email,age\nAlice,alice@example.com,30\nBob,bob@example.com,26\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("alice@new.com"));
        assert!(merged.contains("26"));
    }

    #[test]
    fn test_csv_merge_conflict() {
        let driver = CsvDriver::new();
        let base = "name,email\nAlice,alice@example.com\n";
        let ours = "name,email\nAlice,alice@ours.com\n";
        let theirs = "name,email\nAlice,alice@theirs.com\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_csv_merge_both_add_different_rows() {
        let driver = CsvDriver::new();
        let base = "id,name\n1,Alice\n";
        let ours = "id,name\n1,Alice\n3,Charlie\n";
        let theirs = "id,name\n1,Alice\n2,Bob\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(
            result.is_some(),
            "both sides adding different rows should merge"
        );
        let merged = result.unwrap();
        assert!(merged.contains("2,Bob"), "merged should contain theirs row");
        assert!(
            merged.contains("3,Charlie"),
            "merged should contain ours row"
        );
    }

    #[test]
    fn test_csv_merge_header_change() {
        let driver = CsvDriver::new();
        let base = "name,email\nAlice,alice@example.com\n";
        let ours = "name,email,phone\nAlice,alice@example.com,555-0001\n";
        let theirs = "name,email\nAlice,alice@example.com\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("phone"));
    }

    #[test]
    fn test_correctness_merge_determinism() {
        let driver = CsvDriver::new();
        let base = "name,age,city\nAlice,30,NYC\nBob,25,LA\n";
        let ours = "name,age,city\nAlice,31,NYC\nBob,25,LA\n";
        let theirs = "name,age,city\nAlice,30,NYC\nBob,25,SF\n";

        let r1 = driver.merge(base, ours, theirs).unwrap();
        let r2 = driver.merge(base, theirs, ours).unwrap();
        assert_eq!(r1.is_some(), r2.is_some());
        if let (Some(m1), Some(m2)) = (r1, r2) {
            let (h1, rows1) = CsvDriver::parse_csv(&m1).unwrap();
            let (h2, rows2) = CsvDriver::parse_csv(&m2).unwrap();
            let mut h1_sorted = h1.clone();
            let mut h2_sorted = h2.clone();
            h1_sorted.sort();
            h2_sorted.sort();
            assert_eq!(
                h1_sorted, h2_sorted,
                "headers must match (order-independent)"
            );
            assert_eq!(rows1, rows2, "rows must match");
        }
    }

    #[test]
    fn test_correctness_merge_idempotency() {
        let driver = CsvDriver::new();
        let base = "name,email\nAlice,alice@example.com\n";
        let ours = "name,email\nAlice,alice@new.com\n";

        let result = driver.merge(base, ours, ours).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("alice@new.com"));
    }

    #[test]
    fn test_correctness_base_equals_ours() {
        let driver = CsvDriver::new();
        let base = "name,email\nAlice,alice@example.com\n";
        let theirs = "name,email\nAlice,alice@new.com\n";

        let result = driver.merge(base, base, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("alice@new.com"));
    }

    #[test]
    fn test_correctness_base_equals_theirs() {
        let driver = CsvDriver::new();
        let base = "name,email\nAlice,alice@example.com\n";
        let ours = "name,email\nAlice,alice@new.com\n";

        let result = driver.merge(base, ours, base).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("alice@new.com"));
    }

    #[test]
    fn test_correctness_all_equal() {
        let driver = CsvDriver::new();
        let content = "name,email\nAlice,alice@example.com\n";

        let result = driver.merge(content, content, content).unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn test_correctness_both_add_different_rows() {
        let driver = CsvDriver::new();
        let base = "id,name\n1,Alice\n";
        let ours = "id,name\n1,Alice\n3,Charlie\n";
        let theirs = "id,name\n1,Alice\n2,Bob\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("2,Bob"), "theirs row should be present");
        assert!(merged.contains("3,Charlie"), "ours row should be present");
        assert!(merged.contains("1,Alice"), "base row should be present");
    }

    #[test]
    fn test_correctness_both_modify_same_cell_conflict() {
        let driver = CsvDriver::new();
        let base = "name,email\nAlice,alice@example.com\n";
        let ours = "name,email\nAlice,alice@ours.com\n";
        let theirs = "name,email\nAlice,alice@theirs.com\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(
            result.is_none(),
            "same cell modified differently should conflict"
        );
    }

    #[test]
    fn test_correctness_both_modify_different_cells() {
        let driver = CsvDriver::new();
        let base = "name,email,age\nAlice,alice@example.com,30\n";
        let ours = "name,email,age\nAlice,alice@new.com,30\n";
        let theirs = "name,email,age\nAlice,alice@example.com,31\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("alice@new.com"), "ours email change");
        assert!(merged.contains("31"), "theirs age change");
    }

    #[test]
    fn test_correctness_both_modify_same_key_same_value() {
        let driver = CsvDriver::new();
        let base = "name,email\nAlice,alice@example.com\n";
        let ours = "name,email\nAlice,alice@new.com\n";
        let theirs = "name,email\nAlice,alice@new.com\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some(), "identical changes should not conflict");
        let merged = result.unwrap();
        assert!(merged.contains("alice@new.com"));
    }

    #[test]
    fn test_correctness_unicode_values() {
        let driver = CsvDriver::new();
        let base = "id,名前,都市\n1,太郎,東京\n";
        let ours = "id,名前,都市\n1,太郎,大阪\n";
        let theirs = "id,名前,都市\n1,次郎,東京\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("次郎"), "theirs name change");
        assert!(merged.contains("大阪"), "ours city change");
    }

    #[test]
    fn test_correctness_large_file() {
        let driver = CsvDriver::new();
        let mut base_rows = vec!["id,name,email".to_string()];
        let mut ours_rows = vec!["id,name,email".to_string()];
        let mut theirs_rows = vec!["id,name,email".to_string()];

        for i in 0..500 {
            let row = format!("{i},user{i},user{i}@example.com");
            base_rows.push(row.clone());
            ours_rows.push(if i == 100 {
                format!("{i},user{i},modified_ours@example.com")
            } else {
                row.clone()
            });
            theirs_rows.push(if i == 400 {
                format!("{i},user{i},modified_theirs@example.com")
            } else {
                row
            });
        }

        let base = base_rows.join("\n") + "\n";
        let ours = ours_rows.join("\n") + "\n";
        let theirs = theirs_rows.join("\n") + "\n";

        let result = driver.merge(&base, &ours, &theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("modified_ours@example.com"));
        assert!(merged.contains("modified_theirs@example.com"));
        assert!(merged.contains("user0,user0@example.com"));
    }

    #[test]
    fn test_correctness_output_validity() {
        let driver = CsvDriver::new();
        let base = "name,email,age\nAlice,alice@example.com,30\n";
        let ours = "name,email,age\nAlice,alice@new.com,30\n";
        let theirs = "name,email,age\nAlice,alice@example.com,31\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged_str = result.unwrap();
        let (headers, rows) = CsvDriver::parse_csv(&merged_str)
            .unwrap_or_else(|e| panic!("merged output should be valid CSV: {e}"));
        assert_eq!(headers.len(), 3, "should have 3 columns");
        assert_eq!(rows.len(), 1, "should have 1 data row");
        assert!(merged_str.contains("alice@new.com"));
        assert!(merged_str.contains("31"));
    }

    #[test]
    fn test_correctness_header_only_change() {
        let driver = CsvDriver::new();
        let base = "name\nAlice\n";
        let ours = "name,email\nAlice,alice@example.com\n";
        let theirs = "name\nAlice\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(
            merged.contains("email"),
            "new header from ours should be present"
        );
    }

    #[test]
    fn test_correctness_both_add_rows_at_different_positions() {
        let driver = CsvDriver::new();
        let base = "id,name\n1,Alice\n";
        let ours = "id,name\n1,Alice\n2,Bob\n";
        let theirs = "id,name\n1,Alice\n2,Charlie\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(
            result.is_none(),
            "both sides added a row with the same key but different content → conflict"
        );
    }

    #[test]
    fn test_correctness_row_deletion_by_ours() {
        let driver = CsvDriver::new();
        let base = "name,email\nAlice,alice@example.com\nBob,bob@example.com\n";
        let ours = "name,email\nAlice,alice@example.com\n";
        let theirs = "name,email\nAlice,alice@example.com\nBob,bob@example.com\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        let (_headers, rows) = CsvDriver::parse_csv(&merged).unwrap();
        assert_eq!(
            rows.len(),
            1,
            "key-based merge: Bob deleted by ours, unchanged by theirs → deletion wins"
        );
        assert_eq!(rows[0][0], "Alice");
    }

    #[test]
    fn test_correctness_row_deletion_conflict() {
        let driver = CsvDriver::new();
        let base = "name,email\nAlice,alice@example.com\nBob,bob@example.com\n";
        let ours = "name,email\nAlice,alice@example.com\n";
        let theirs = "name,email\nAlice,alice@example.com\nBob,bob@new.com\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(
            result.is_none(),
            "key-based merge: ours deleted Bob, theirs modified Bob → delete vs modify conflict"
        );
    }

    #[test]
    fn test_correctness_empty_csv() {
        let driver = CsvDriver::new();
        let base = "name,email\n";
        let ours = "name,email\nAlice,alice@example.com\n";
        let theirs = "name,email\nBob,bob@example.com\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("Alice") || merged.contains("Bob"));
    }

    #[test]
    fn test_correctness_quoted_fields() {
        let driver = CsvDriver::new();
        let base = "name,desc\n\"Alice\",\"hello, world\"\n";
        let ours = "name,desc\n\"Alice\",\"hello, world\"\n";
        let theirs = "name,desc\n\"Alice\",\"goodbye, world\"\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("goodbye, world"));
    }

    #[test]
    fn test_correctness_both_delete_same_row() {
        let driver = CsvDriver::new();
        let base = "id,name\n1,Alice\n2,Bob\n3,Charlie\n";
        let ours = "id,name\n1,Alice\n3,Charlie\n";
        let theirs = "id,name\n1,Alice\n3,Charlie\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        let (_h, rows) = CsvDriver::parse_csv(&merged).unwrap();
        assert_eq!(rows.len(), 2, "Bob deleted by both sides should be omitted");
    }

    #[test]
    fn test_correctness_key_based_merge_insert_bug() {
        let driver = CsvDriver::new();
        let base = "id,name,email,age\n1,Alice,alice@example.com,30\n2,Bob,bob@example.com,25\n3,Carol,carol@example.com,35\n4,Dave,dave@example.com,28\n5,Eve,eve@example.com,22\n";
        let ours = "id,name,email,age\n1,Alice,alice@example.com,30\n2,Bob,bob@example.com,25\n3,Carol,carol@example.com,36\n4,Dave,dave@example.com,28\n5,Eve,eve@example.com,22\n6,Frank,frank@example.com,40\n";
        let theirs = "id,name,email,age\n1,Alice,alice@new.com,30\n2,Bob,bob@example.com,25\n3,Carol,carol@example.com,35\n4,Dave,dave@example.com,28\n5,Eve,eve@example.com,22\n7,Grace,grace@example.com,29\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(
            result.is_some(),
            "inserting rows should not cause misalignment with key-based merge"
        );
        let merged = result.unwrap();
        let (_headers, rows) = CsvDriver::parse_csv(&merged).unwrap();

        assert_eq!(
            rows.len(),
            7,
            "should have all 5 base rows plus 2 additions"
        );

        let alice = &rows.iter().find(|r| r[0] == "1").unwrap();
        assert_eq!(
            alice[2], "alice@new.com",
            "theirs' modification to row 1 email should be present"
        );

        let carol = &rows.iter().find(|r| r[0] == "3").unwrap();
        assert_eq!(
            carol[3], "36",
            "ours' modification to row 3 should be present"
        );

        assert!(
            rows.iter().any(|r| r[0] == "6" && r[1] == "Frank"),
            "ours' added row should be present"
        );
        assert!(
            rows.iter().any(|r| r[0] == "7" && r[1] == "Grace"),
            "theirs' added row should be present"
        );

        let bob = &rows.iter().find(|r| r[0] == "2").unwrap();
        assert_eq!(
            bob[2], "bob@example.com",
            "unchanged row 2 should be preserved"
        );

        let dave = &rows.iter().find(|r| r[0] == "4").unwrap();
        assert_eq!(dave[1], "Dave", "unchanged row 4 should be preserved");
    }

    #[test]
    fn test_correctness_duplicate_keys() {
        let driver = CsvDriver::new();
        let base = "id,name\n1,Alice\n1,Bob\n";
        let ours = "id,name\n1,Alice\n1,Robert\n";
        let theirs = "id,name\n1,Alice\n1,Bob\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some(), "duplicate keys should merge correctly");
        let merged = result.unwrap();
        let (_h, rows) = CsvDriver::parse_csv(&merged).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][1], "Alice");
        assert_eq!(rows[1][1], "Robert");
    }

    #[test]
    fn test_correctness_merge_associativity() {
        let driver = CsvDriver::new();
        let base = "id,a,b,c,d\nrow,1,2,3,4\n";
        let a = "id,a,b,c,d\nrow,10,2,3,4\n";
        let b = "id,a,b,c,d\nrow,1,20,3,4\n";
        let c = "id,a,b,c,d\nrow,1,2,30,4\n";

        let ab = driver.merge(base, a, b).unwrap().expect("merge(base, A, B) should succeed");
        let merge_left = driver
            .merge(base, &ab, c)
            .unwrap()
            .expect("merge(base, merge(A,B), C) should succeed");

        let bc = driver.merge(base, b, c).unwrap().expect("merge(base, B, C) should succeed");
        let merge_right = driver
            .merge(base, a, &bc)
            .unwrap()
            .expect("merge(base, A, merge(B,C)) should succeed");

        let (h_left, r_left) = CsvDriver::parse_csv(&merge_left).unwrap();
        let (h_right, r_right) = CsvDriver::parse_csv(&merge_right).unwrap();

        assert_eq!(h_left, h_right, "headers must match");
        assert_eq!(r_left, r_right, "rows must match");
        assert!(merge_left.contains("10"));
        assert!(merge_left.contains("20"));
        assert!(merge_left.contains("30"));
        assert!(merge_left.contains(",4\n"));
    }

    proptest! {
        #[test]
        fn test_merge_identity(content in "[a-z0-9]+") {
            let csv = format!("col\n{}\n", content);
            let driver = CsvDriver::new();
            let result = driver.merge(&csv, &csv, &csv).unwrap();
            prop_assert!(result.is_some());
            let merged = result.unwrap();
            prop_assert!(merged.contains(&content));
        }

        #[test]
        fn test_merge_idempotence(
            base in "[a-z0-9]+",
            modified in "[a-z0-9]+",
        ) {
            let base_csv = format!("col\n{}\n", base);
            let modified_csv = format!("col\n{}\n", modified);
            let driver = CsvDriver::new();
            let result = driver.merge(&base_csv, &modified_csv, &modified_csv).unwrap();
            prop_assert!(result.is_some());
            let merged = result.unwrap();
            prop_assert!(merged.contains(&modified));
        }

        #[test]
        fn test_csv_merge_non_overlapping_rows(
            col1 in "[a-z]+",
            col2 in "[a-z]+",
            row1_key in "[a-z0-9]+",
            row2_key in "[a-z0-9]+",
            row3_key in "[a-z0-9]+",
            row1_val in "[a-z0-9]+",
            row1_mod_val in "[a-z0-9]+",
            row3_val in "[a-z0-9]+",
            row3_mod_val in "[a-z0-9]+",
        ) {
            let base = format!("{col1},{col2}\n{row1_key},{row1_val}\n{row2_key},x\n{row3_key},{row3_val}\n");
            let ours = format!("{col1},{col2}\n{row1_key},{row1_mod_val}\n{row2_key},x\n{row3_key},{row3_val}\n");
            let theirs = format!("{col1},{col2}\n{row1_key},{row1_val}\n{row2_key},x\n{row3_key},{row3_mod_val}\n");

            let driver = CsvDriver::new();
            let result = driver.merge(&base, &ours, &theirs);
            prop_assert!(result.is_ok());
            let opt = result.unwrap();
            prop_assert!(opt.is_some());
            let merged = opt.unwrap();
            prop_assert!(merged.contains(&row1_mod_val));
            prop_assert!(merged.contains(&row3_mod_val));
        }
    }
}

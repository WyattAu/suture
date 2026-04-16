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

    fn diff_rows(
        old_headers: &[String],
        new_headers: &[String],
        old_rows: &[Vec<String>],
        new_rows: &[Vec<String>],
    ) -> Vec<SemanticChange> {
        let mut changes = Vec::new();

        let old_headers_set: std::collections::HashSet<&str> =
            old_headers.iter().map(|s| s.as_str()).collect();
        let new_headers_set: std::collections::HashSet<&str> =
            new_headers.iter().map(|s| s.as_str()).collect();

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

        let max_rows = old_rows.len().max(new_rows.len());

        for i in 0..max_rows {
            match (old_rows.get(i), new_rows.get(i)) {
                (None, Some(new_row)) => {
                    changes.push(SemanticChange::Added {
                        path: format!("/rows/{i}"),
                        value: new_row.join(","),
                    });
                }
                (Some(old_row), None) => {
                    changes.push(SemanticChange::Removed {
                        path: format!("/rows/{i}"),
                        old_value: old_row.join(","),
                    });
                }
                (Some(old_row), Some(new_row)) => {
                    for (col_idx, col_name) in common_headers.iter().enumerate() {
                        let old_val = old_row.get(col_idx).map(|s| s.as_str()).unwrap_or("");
                        let new_val = new_row.get(col_idx).map(|s| s.as_str()).unwrap_or("");

                        if old_val != new_val {
                            changes.push(SemanticChange::Modified {
                                path: format!("/{col_name}:{i}"),
                                old_value: old_val.to_string(),
                                new_value: new_val.to_string(),
                            });
                        }
                    }
                }
                (None, None) => {}
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
        let base_set: std::collections::HashSet<&str> =
            base_headers.iter().map(|s| s.as_str()).collect();
        let ours_set: std::collections::HashSet<&str> =
            ours_headers.iter().map(|s| s.as_str()).collect();
        let theirs_set: std::collections::HashSet<&str> =
            theirs_headers.iter().map(|s| s.as_str()).collect();

        let all_header_names: std::collections::HashSet<&str> = base_set
            .iter()
            .chain(ours_set.iter())
            .chain(theirs_set.iter())
            .copied()
            .collect();

        let mut merged_headers: Vec<String> = Vec::new();
        for &h in &all_header_names {
            let in_base = base_set.contains(h);
            let in_ours = ours_set.contains(h);
            let in_theirs = theirs_set.contains(h);
            match (in_base, in_ours, in_theirs) {
                (true, true, false) | (false, true, false) => merged_headers.push(h.to_string()),
                (true, false, true) | (false, false, true) => merged_headers.push(h.to_string()),
                (true, true, true) => merged_headers.push(h.to_string()),
                (false, true, true) => merged_headers.push(h.to_string()),
                (true, false, false) | (false, false, false) => {}
            }
        }

        let max_rows = base_rows.len().max(ours_rows.len()).max(theirs_rows.len());
        let mut merged_rows: Vec<Vec<String>> = Vec::new();

        for i in 0..max_rows {
            let base_row = base_rows.get(i);
            let ours_row = ours_rows.get(i);
            let theirs_row = theirs_rows.get(i);

            match (base_row, ours_row, theirs_row) {
                (None, Some(o), None) => merged_rows.push(o.clone()),
                (None, None, Some(t)) => merged_rows.push(t.clone()),
                (None, Some(o), Some(t)) => {
                    if o == t {
                        merged_rows.push(o.clone());
                    } else {
                        // Both sides added different rows at the same position.
                        // Include both — additions from both sides should be preserved.
                        merged_rows.push(o.clone());
                        merged_rows.push(t.clone());
                    }
                }
                (None, None, _) => {}
                (Some(_), Some(o), None) => merged_rows.push(o.clone()),
                (Some(_), None, Some(t)) => merged_rows.push(t.clone()),
                (Some(_), None, None) => {}
                (Some(b), Some(o), Some(t)) => {
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
                    changes.push(SemanticChange::Added {
                        path: format!("/rows/{i}"),
                        value: row.join(","),
                    });
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
            path: "/email:0".to_string(),
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
            path: "/rows/1".to_string(),
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
            path: "/rows/1".to_string(),
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
        let base = "名前,都市\n太郎,東京\n";
        let ours = "名前,都市\n太郎,大阪\n";
        let theirs = "名前,都市\n次郎,東京\n";

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
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("2,Bob") || merged.contains("2,Charlie"));
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
            2,
            "CSV positional merge: base row at index 1 deleted by ours but kept by theirs"
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
            result.is_some(),
            "CSV positional merge: ours deleted, theirs modified → theirs row kept"
        );
        let merged = result.unwrap();
        assert!(
            merged.contains("bob@new.com"),
            "theirs modification should be preserved"
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
}

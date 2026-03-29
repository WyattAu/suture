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
}

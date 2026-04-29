// SPDX-License-Identifier: MIT OR Apache-2.0
#![allow(clippy::collapsible_match)]
use suture_driver::{DriverError, SemanticChange, SutureDriver};

pub struct PropertiesDriver;

impl Default for PropertiesDriver {
    fn default() -> Self {
        Self
    }
}

impl PropertiesDriver {
    pub fn new() -> Self {
        Self
    }

    fn parse_properties(content: &str) -> Result<Vec<(String, String)>, String> {
        let mut entries = Vec::new();
        let mut continuation = String::new();
        let mut current_key = String::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
                if !current_key.is_empty() {
                    entries.push((current_key.clone(), continuation.trim_end().to_string()));
                    current_key.clear();
                    continuation.clear();
                }
                continue;
            }

            if let Some(eq_pos) = trimmed.find('=') {
                if !current_key.is_empty() {
                    entries.push((current_key.clone(), continuation.trim_end().to_string()));
                }
                current_key = trimmed[..eq_pos].trim().to_string();
                continuation = trimmed[eq_pos + 1..].trim().to_string();
            } else if let Some(stripped) = trimmed.strip_suffix('\\') {
                continuation.push_str(stripped);
            } else if !current_key.is_empty() {
                continuation.push_str(trimmed);
                entries.push((current_key.clone(), continuation.trim_end().to_string()));
                current_key.clear();
                continuation.clear();
            }
        }

        if !current_key.is_empty() {
            entries.push((current_key, continuation.trim_end().to_string()));
        }

        Ok(entries)
    }

    fn merge_properties(
        base: &[(String, String)],
        ours: &[(String, String)],
        theirs: &[(String, String)],
    ) -> Option<String> {
        let base_map: std::collections::HashMap<&str, &str> =
            base.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let ours_map: std::collections::HashMap<&str, &str> =
            ours.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let theirs_map: std::collections::HashMap<&str, &str> = theirs
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();

        let mut all_keys: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for (k, _) in base {
            all_keys.insert(k);
        }
        for (k, _) in ours {
            all_keys.insert(k);
        }
        for (k, _) in theirs {
            all_keys.insert(k);
        }

        for key in &all_keys {
            let base_val = base_map.get(key).copied();
            let ours_val = ours_map.get(key).copied();
            let theirs_val = theirs_map.get(key).copied();

            if ours_val != base_val && theirs_val != base_val && ours_val != theirs_val {
                return None;
            }
        }

        let mut result = String::new();
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();

        for (key, _) in ours {
            if seen.insert(key.as_str()) {
                let ours_val = ours_map.get(key.as_str()).copied();
                let theirs_val = theirs_map.get(key.as_str()).copied();
                let base_val = base_map.get(key.as_str()).copied();

                let merged_val = if ours_val != base_val {
                    ours_val
                } else {
                    theirs_val.or(base_val)
                };

                if let Some(val) = merged_val {
                    result.push_str(&format!("{}={}\n", key, val));
                }
            }
        }

        for (key, _) in theirs {
            if seen.insert(key.as_str()) {
                let theirs_val = theirs_map.get(key.as_str()).copied();
                if let Some(val) = theirs_val {
                    result.push_str(&format!("{}={}\n", key, val));
                }
            }
        }

        Some(result)
    }
}

impl SutureDriver for PropertiesDriver {
    fn name(&self) -> &str {
        "properties"
    }

    fn supported_extensions(&self) -> &[&str] {
        &["properties"]
    }

    fn diff(
        &self,
        base_content: Option<&str>,
        new_content: &str,
    ) -> Result<Vec<SemanticChange>, DriverError> {
        let base_entries: Vec<(String, String)> = base_content
            .map(|c| Self::parse_properties(c).unwrap_or_default())
            .unwrap_or_default();
        let new_entries = Self::parse_properties(new_content).map_err(DriverError::ParseError)?;

        let base_map: std::collections::HashMap<&str, &str> = base_entries
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let new_map: std::collections::HashMap<&str, &str> = new_entries
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();

        let mut changes = Vec::new();

        for (key, val) in &new_entries {
            match base_map.get(key.as_str()) {
                None => changes.push(SemanticChange::Added {
                    path: format!("/{}", key),
                    value: val.clone(),
                }),
                Some(base_val) if *base_val != val.as_str() => {
                    changes.push(SemanticChange::Modified {
                        path: format!("/{}", key),
                        old_value: base_val.to_string(),
                        new_value: val.clone(),
                    });
                }
                _ => {}
            }
        }

        for (key, val) in &base_entries {
            if !new_map.contains_key(key.as_str()) {
                changes.push(SemanticChange::Removed {
                    path: format!("/{}", key),
                    old_value: val.clone(),
                });
            }
        }

        Ok(changes)
    }

    fn format_diff(
        &self,
        base_content: Option<&str>,
        new_content: &str,
    ) -> Result<String, DriverError> {
        let changes = self.diff(base_content, new_content)?;
        let mut output = String::new();
        for change in &changes {
            match change {
                SemanticChange::Added { path, value } => {
                    output.push_str(&format!("+ {} = {}\n", path, value))
                }
                SemanticChange::Removed { path, .. } => output.push_str(&format!("- {}\n", path)),
                SemanticChange::Modified {
                    path,
                    old_value,
                    new_value,
                } => output.push_str(&format!("~ {} ({} -> {})\n", path, old_value, new_value)),
                SemanticChange::Moved { .. } => {}
            }
        }
        Ok(output)
    }

    fn merge(&self, base: &str, ours: &str, theirs: &str) -> Result<Option<String>, DriverError> {
        let base_entries = Self::parse_properties(base).map_err(DriverError::ParseError)?;
        let ours_entries = Self::parse_properties(ours).map_err(DriverError::ParseError)?;
        let theirs_entries = Self::parse_properties(theirs).map_err(DriverError::ParseError)?;

        Ok(Self::merge_properties(
            &base_entries,
            &ours_entries,
            &theirs_entries,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_properties() {
        let content = "host=localhost\nport=8080\ndb.name=mydb\n";
        let entries = PropertiesDriver::parse_properties(content).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0], ("host".to_string(), "localhost".to_string()));
        assert_eq!(entries[1], ("port".to_string(), "8080".to_string()));
        assert_eq!(entries[2], ("db.name".to_string(), "mydb".to_string()));
    }

    #[test]
    fn test_parse_properties_with_comments() {
        let content = "# Database configuration\nhost=localhost\n\n# Connection settings\nport=8080\n! legacy comment\ndebug=true\n";
        let entries = PropertiesDriver::parse_properties(content).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0], ("host".to_string(), "localhost".to_string()));
        assert_eq!(entries[1], ("port".to_string(), "8080".to_string()));
        assert_eq!(entries[2], ("debug".to_string(), "true".to_string()));
    }

    #[test]
    fn test_diff_properties() {
        let driver = PropertiesDriver::new();
        let base = "host=localhost\nport=8080\n";
        let new = "host=127.0.0.1\nport=8080\ndebug=true\n";

        let changes = driver.diff(Some(base), new).unwrap();
        assert_eq!(changes.len(), 2);

        let modified = changes
            .iter()
            .find(|c| matches!(c, SemanticChange::Modified { .. }));
        assert!(modified.is_some());
        if let Some(SemanticChange::Modified {
            path,
            old_value,
            new_value,
        }) = modified
        {
            assert_eq!(path, "/host");
            assert_eq!(old_value, "localhost");
            assert_eq!(new_value, "127.0.0.1");
        }

        let added = changes
            .iter()
            .find(|c| matches!(c, SemanticChange::Added { .. }));
        assert!(added.is_some());
        if let Some(SemanticChange::Added { path, value }) = added {
            assert_eq!(path, "/debug");
            assert_eq!(value, "true");
        }
    }

    #[test]
    fn test_merge_properties_no_conflict() {
        let driver = PropertiesDriver::new();
        let base = "host=localhost\nport=8080\n";
        let ours = "host=localhost\nport=9090\n";
        let theirs = "host=127.0.0.1\nport=8080\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("host=127.0.0.1"));
        assert!(merged.contains("port=9090"));
    }

    #[test]
    fn test_merge_properties_conflict() {
        let driver = PropertiesDriver::new();
        let base = "host=localhost\nport=8080\n";
        let ours = "host=host-a\nport=8080\n";
        let theirs = "host=host-b\nport=8080\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_merge_properties_one_side_adds() {
        let driver = PropertiesDriver::new();
        let base = "host=localhost\nport=8080\n";
        let ours = "host=localhost\nport=8080\ndebug=true\n";
        let theirs = "host=localhost\nport=8080\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("debug=true"));
        assert!(merged.contains("host=localhost"));
        assert!(merged.contains("port=8080"));
    }
}

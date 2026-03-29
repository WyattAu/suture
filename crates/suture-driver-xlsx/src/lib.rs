//! XLSX semantic driver — cell-level diff and merge for Excel spreadsheets.

use suture_driver::{DriverError, SemanticChange, SutureDriver};
use suture_ooxml::OoxmlDocument;

type Cell = (usize, usize, String);
type SheetData = (String, Vec<Cell>);

pub struct XlsxDriver;

impl XlsxDriver {
    pub fn new() -> Self {
        Self
    }

    #[allow(clippy::type_complexity)]
    fn parse_sheets(doc: &OoxmlDocument) -> Result<Vec<SheetData>, DriverError> {
        let mut sheets = Vec::new();

        let mut sheet_files: Vec<String> = doc
            .parts
            .keys()
            .filter(|k| k.contains("worksheets/") && k.ends_with(".xml"))
            .cloned()
            .collect();
        sheet_files.sort();

        for path in &sheet_files {
            let part = doc
                .get_part(path)
                .ok_or_else(|| DriverError::ParseError(format!("sheet part {} missing", path)))?;
            let name = path.rsplit('/').next().unwrap_or("sheet");
            let name = name.strip_suffix(".xml").unwrap_or(name);
            let cells = Self::parse_sheet_xml(&part.content);
            sheets.push((name.to_string(), cells));
        }

        Ok(sheets)
    }

    fn parse_sheet_xml(xml: &str) -> Vec<Cell> {
        let mut cells = Vec::new();
        let mut current_row: usize = 0;
        let mut current_col: usize = 0;
        let mut in_row = false;
        let mut in_cell = false;
        let mut cell_text = String::new();

        for line in xml.lines() {
            let trimmed = line.trim();
            if trimmed.contains("<row ") || trimmed.contains("<row>") {
                in_row = true;
                if let Some(idx) = Self::extract_attr(trimmed, "r")
                    && let Ok(n) = idx.parse::<usize>()
                {
                    current_row = n;
                }
            }
            if in_row && (trimmed.contains("<c ") || trimmed.contains("<c>")) {
                in_cell = true;
                cell_text.clear();
                if let Some(idx) = Self::extract_attr(trimmed, "r")
                    && let Ok(n) = idx.parse::<usize>()
                {
                    current_row = n;
                }
                if let Some(idx) = Self::extract_attr(trimmed, "t")
                    && let Ok(n) = idx.parse::<usize>()
                {
                    current_col = n;
                }
            }
            if in_cell {
                if let Some(start) = trimmed.find("<v>") {
                    let after = &trimmed[start + 3..];
                    if let Some(end) = after.find("</v>") {
                        cell_text = after[..end].to_string();
                    }
                }
                if trimmed.contains("</c>") {
                    cells.push((current_row, current_col, cell_text.clone()));
                    in_cell = false;
                }
            }
            if trimmed.contains("</row>") {
                in_row = false;
            }
        }
        cells
    }

    fn extract_attr(xml_line: &str, attr_name: &str) -> Option<String> {
        let pattern = format!("{}=\"", attr_name);
        let start = xml_line.find(&pattern)?;
        let start = start + pattern.len();
        let rest = &xml_line[start..];
        let end = rest.find('"')?;
        Some(rest[..end].to_string())
    }

    fn diff_cells(
        base_cells: &[Cell],
        new_cells: &[Cell],
        sheet_name: &str,
    ) -> Vec<SemanticChange> {
        let base_map: std::collections::HashMap<(usize, usize), &String> =
            base_cells.iter().map(|(r, c, v)| ((*r, *c), v)).collect();
        let new_map: std::collections::HashMap<(usize, usize), &String> =
            new_cells.iter().map(|(r, c, v)| ((*r, *c), v)).collect();

        let mut changes = Vec::new();
        let all_keys: std::collections::HashSet<_> =
            base_map.keys().chain(new_map.keys()).collect();

        for (row, col) in all_keys {
            let path = format!("/{}/{}/{}", sheet_name, row, col);
            match (base_map.get(&(*row, *col)), new_map.get(&(*row, *col))) {
                (None, Some(val)) => changes.push(SemanticChange::Added {
                    path,
                    value: (*val).clone(),
                }),
                (Some(val), None) => changes.push(SemanticChange::Removed {
                    path,
                    old_value: (*val).clone(),
                }),
                (Some(old), Some(new)) if old != new => {
                    changes.push(SemanticChange::Modified {
                        path,
                        old_value: (*old).clone(),
                        new_value: (*new).clone(),
                    });
                }
                _ => {}
            }
        }
        changes
    }

    #[allow(dead_code)]
    fn merge_cells(
        base: &[Cell],
        ours: &[Cell],
        theirs: &[Cell],
    ) -> Option<Vec<Cell>> {
        let base_map: std::collections::HashMap<(usize, usize), &String> =
            base.iter().map(|(r, c, v)| ((*r, *c), v)).collect();
        let ours_map: std::collections::HashMap<(usize, usize), &String> =
            ours.iter().map(|(r, c, v)| ((*r, *c), v)).collect();
        let theirs_map: std::collections::HashMap<(usize, usize), &String> =
            theirs.iter().map(|(r, c, v)| ((*r, *c), v)).collect();

        let all_keys: std::collections::HashSet<_> = base_map
            .keys()
            .chain(ours_map.keys())
            .chain(theirs_map.keys())
            .collect();
        let mut merged = Vec::new();

        for &(row, col) in all_keys {
            let b = base_map.get(&(row, col)).map(|s| s.as_str());
            let o = ours_map.get(&(row, col)).map(|s| s.as_str());
            let t = theirs_map.get(&(row, col)).map(|s| s.as_str());

            match (b, o, t) {
                (None, Some(o), None) => merged.push((row, col, o.to_string())),
                (None, None, Some(t)) => merged.push((row, col, t.to_string())),
                (None, Some(o), Some(t)) => {
                    if o == t {
                        merged.push((row, col, o.to_string()));
                    } else {
                        return None;
                    }
                }
                (Some(_), Some(o), None) => merged.push((row, col, o.to_string())),
                (Some(_), None, Some(t)) => merged.push((row, col, t.to_string())),
                (Some(_), None, None) => {}
                (Some(b), Some(o), Some(t)) => {
                    if o == t {
                        merged.push((row, col, o.to_string()));
                    } else if o == b {
                        merged.push((row, col, t.to_string()));
                    } else if t == b {
                        merged.push((row, col, o.to_string()));
                    } else {
                        return None;
                    }
                }
                (None, None, None) => {}
            }
        }
        Some(merged)
    }
}

impl Default for XlsxDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl SutureDriver for XlsxDriver {
    fn name(&self) -> &str {
        "XLSX"
    }
    fn supported_extensions(&self) -> &[&str] {
        &[".xlsx"]
    }

    fn diff(
        &self,
        base_content: Option<&str>,
        new_content: &str,
    ) -> Result<Vec<SemanticChange>, DriverError> {
        let new_doc = OoxmlDocument::from_bytes(new_content.as_bytes())
            .map_err(|e| DriverError::ParseError(e.to_string()))?;
        let new_sheets = Self::parse_sheets(&new_doc)?;

        #[allow(clippy::type_complexity)]
        let base_sheets: Vec<SheetData> = match base_content {
            None => Vec::new(),
            Some(base) => {
                let base_doc = OoxmlDocument::from_bytes(base.as_bytes())
                    .map_err(|e| DriverError::ParseError(e.to_string()))?;
                Self::parse_sheets(&base_doc)?
            }
        };

        let mut changes = Vec::new();
        for (name, cells) in &new_sheets {
            let base_cells = base_sheets
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, c)| c.as_slice())
                .unwrap_or(&[]);
            changes.extend(Self::diff_cells(base_cells, cells, name));
        }
        Ok(changes)
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
        let lines: Vec<String> = changes
            .iter()
            .map(|c| match c {
                SemanticChange::Added { path, value } => format!("  ADDED     {}: {}", path, value),
                SemanticChange::Removed { path, old_value } => {
                    format!("  REMOVED   {}: {}", path, old_value)
                }
                SemanticChange::Modified {
                    path,
                    old_value,
                    new_value,
                } => format!("  MODIFIED  {}: {} -> {}", path, old_value, new_value),
                SemanticChange::Moved {
                    old_path,
                    new_path,
                    value,
                } => format!("  MOVED     {} -> {}: {}", old_path, new_path, value),
            })
            .collect();
        Ok(lines.join("\n"))
    }

    fn merge(
        &self,
        _base: &str,
        _ours: &str,
        _theirs: &str,
    ) -> Result<Option<String>, DriverError> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_driver_name() {
        assert_eq!(XlsxDriver::new().name(), "XLSX");
    }
    #[test]
    fn test_extensions() {
        assert_eq!(XlsxDriver::new().supported_extensions(), &[".xlsx"]);
    }

    #[test]
    fn test_diff_cells() {
        let base = vec![(0, 0, "A".to_string()), (0, 1, "B".to_string())];
        let new = vec![
            (0, 0, "X".to_string()),
            (0, 1, "B".to_string()),
            (1, 0, "C".to_string()),
        ];
        let changes = XlsxDriver::diff_cells(&base, &new, "Sheet1");
        assert!(changes
            .iter()
            .any(|c| matches!(c, SemanticChange::Modified { .. })));
        assert!(changes
            .iter()
            .any(|c| matches!(c, SemanticChange::Added { value, .. } if value == "C")));
    }

    #[test]
    fn test_merge_cells_no_conflict() {
        let base = vec![(0, 0, "A".to_string()), (0, 1, "B".to_string())];
        let ours = vec![(0, 0, "X".to_string())];
        let theirs = vec![(0, 1, "Y".to_string())];
        let result = XlsxDriver::merge_cells(&base, &ours, &theirs);
        assert!(result.is_some());
        let m = result.unwrap();
        assert_eq!(
            m.iter()
                .find(|(r, c, _)| *r == 0 && *c == 0)
                .map(|(_, _, v)| v.as_str()),
            Some("X")
        );
        assert_eq!(
            m.iter()
                .find(|(r, c, _)| *r == 0 && *c == 1)
                .map(|(_, _, v)| v.as_str()),
            Some("Y")
        );
    }

    #[test]
    fn test_merge_cells_conflict() {
        let base = vec![(0, 0, "A".to_string())];
        let ours = vec![(0, 0, "X".to_string())];
        let theirs = vec![(0, 0, "Y".to_string())];
        assert!(XlsxDriver::merge_cells(&base, &ours, &theirs).is_none());
    }
}

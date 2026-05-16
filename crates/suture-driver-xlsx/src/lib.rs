// SPDX-License-Identifier: MIT OR Apache-2.0
//! XLSX semantic driver — cell-level diff and merge for Excel spreadsheets.
//!
//! ## Architecture
//!
//! Real XLSX files store data in `xl/worksheets/sheetN.xml` with cell references
//! in A1 notation (e.g., `<c r="B3">`). Cell types are indicated by the `t` attribute:
//! - `t="s"` → shared string (index into `xl/sharedStrings.xml`)
//! - `t="inlineStr"` → inline string (embedded `<is><t>...</t></is>`)
//! - `t="n"` or absent → numeric value (from `<v>`)
//! - `t="b"` → boolean
//! - `t="str"` → formula string result
//!
//! This driver:
//! 1. Parses `xl/sharedStrings.xml` to build a string table
//! 2. Resolves cell references from A1 notation to (row, col) coordinates
//! 3. Performs cell-level diff and three-way merge

use std::collections::{BTreeMap, BTreeSet, HashMap};

use suture_driver::{DriverError, SemanticChange, SutureDriver};
use suture_ooxml::OoxmlDocument;

use std::fmt::Write;
/// Convert bytes to String, replacing invalid UTF-8 sequences with the Unicode replacement character.
/// This is safe for binary formats like OOXML (ZIP/XML) where the content should be valid UTF-8
/// per specification (ECMA-376, ISO 29500), but we defensively handle edge cases.
fn bytes_to_string_lossy(bytes: Vec<u8>) -> String {
    String::from_utf8_lossy(&bytes).into_owned()
}

type Cell = (usize, usize, String);
type SheetData = (String, Vec<Cell>);

#[derive(Debug, Clone, PartialEq, Eq)]
struct FormulaCell {
    row: usize,
    col: usize,
    formula: Option<String>,
    value: String,
}

pub struct XlsxDriver;

impl XlsxDriver {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Parse a column letter(s) to a 1-based column index.
    /// A=1, B=2, ..., Z=26, AA=27, AB=28, etc.
    fn col_from_a1(col_str: &str) -> usize {
        let mut col = 0usize;
        for ch in col_str.bytes() {
            col = col * 26 + (ch - b'A' + 1) as usize;
        }
        col
    }

    /// Parse A1 notation (e.g., "B3", "AA42") to (row, col) coordinates (1-based).
    fn parse_a1(ref_str: &str) -> Option<(usize, usize)> {
        let bytes = ref_str.as_bytes();
        let mut split = 0;
        while split < bytes.len() && bytes[split].is_ascii_alphabetic() {
            split += 1;
        }
        if split == 0 || split >= bytes.len() {
            return None;
        }
        let col_str = &ref_str[..split];
        let row_str = &ref_str[split..];
        let col = Self::col_from_a1(col_str);
        let row = row_str.parse::<usize>().ok()?;
        Some((row, col))
    }

    /// Extract an XML attribute value from a line of XML.
    fn extract_attr(xml_line: &str, attr_name: &str) -> Option<String> {
        let pattern = format!("{attr_name}=\"");
        let start = xml_line.find(&pattern)?;
        let start = start + pattern.len();
        let rest = &xml_line[start..];
        let end = rest.find('"')?;
        Some(rest[..end].to_string())
    }

    /// Parse the shared strings table from `xl/sharedStrings.xml`.
    /// Returns a vector of string values indexed by their position.
    fn parse_shared_strings(doc: &OoxmlDocument) -> Vec<String> {
        let mut strings = Vec::new();
        let Some(part) = doc.get_part("xl/sharedStrings.xml") else {
            return strings;
        };

        let mut in_si = false;
        let mut current_text = String::new();

        for line in part.content.lines() {
            let trimmed = line.trim();

            if !in_si && (trimmed.contains("<si>") || trimmed.contains("<si ")) {
                in_si = true;
                current_text.clear();
                // Don't continue — <t> might be on the same line
            }

            if in_si {
                if let Some(start) = trimmed.find("<t>") {
                    let after = &trimmed[start + 3..];
                    if let Some(end) = after.find("</t>") {
                        current_text = after[..end].to_string();
                    }
                }
                if trimmed.contains("</si>") {
                    strings.push(std::mem::take(&mut current_text));
                    in_si = false;
                }
            }
        }
        strings
    }

    /// Parse a worksheet XML to extract cells.
    fn parse_sheet_xml(xml: &str, shared_strings: &[String]) -> Vec<Cell> {
        let mut cells = Vec::new();
        let mut in_cell = false;
        let mut cell_ref = String::new();
        let mut cell_type = String::new();
        let mut cell_value = String::new();
        let mut in_inline_str = false;

        for line in xml.lines() {
            let trimmed = line.trim();

            // Look for all <c> tags on this line (there may be multiple cells per line)
            let mut search_from = 0;
            loop {
                // Find next <c on this line
                let c_pos = if in_cell {
                    // Already inside a cell — continue processing it
                    None
                } else {
                    let remaining = &trimmed[search_from..];
                    remaining
                        .find("<c ")
                        .or_else(|| remaining.find("<c>"))
                        .map(|pos| search_from + pos)
                };

                if !in_cell {
                    match c_pos {
                        Some(pos) => {
                            in_cell = true;
                            cell_ref.clear();
                            cell_type.clear();
                            cell_value.clear();
                            in_inline_str = false;

                            // Extract attributes from the <c> tag only
                            let c_tag = &trimmed[pos..];
                            let c_tag_end = c_tag.find('>').unwrap_or(c_tag.len());
                            let c_tag_only = &c_tag[..c_tag_end];

                            if let Some(r) = Self::extract_attr(c_tag_only, "r") {
                                cell_ref = r;
                            }
                            if let Some(t) = Self::extract_attr(c_tag_only, "t") {
                                cell_type = t;
                            }
                            search_from = pos + c_tag_end;
                        }
                        None => break, // No more cells on this line
                    }
                }

                if in_cell {
                    // Search only within the part of the line starting from this cell's <c> tag
                    let cell_region = &trimmed[search_from..];

                    // Extract value from <v>...</v>
                    if let Some(start) = cell_region.find("<v>") {
                        let after = &cell_region[start + 3..];
                        if let Some(end) = after.find("</v>") {
                            cell_value = after[..end].to_string();
                        }
                    }

                    // Detect inline string
                    if cell_region.contains("<is>") || cell_region.contains("<is ") {
                        in_inline_str = true;
                    }
                    if in_inline_str {
                        if let Some(start) = cell_region.find("<t>") {
                            let after = &cell_region[start + 3..];
                            if let Some(end) = after.find("</t>") {
                                cell_value = after[..end].to_string();
                            }
                        }
                        if cell_region.contains("</is>") {
                            in_inline_str = false;
                        }
                    }

                    // Cell end
                    if cell_region.contains("</c>") {
                        if let Some((row, col)) = Self::parse_a1(&cell_ref) {
                            let display_value = match cell_type.as_str() {
                                "s" => cell_value
                                    .parse::<usize>()
                                    .ok()
                                    .and_then(|idx| shared_strings.get(idx).cloned())
                                    .unwrap_or_else(|| cell_value.clone()),
                                "b" => match cell_value.as_str() {
                                    "1" | "true" => "TRUE".to_owned(),
                                    _ => "FALSE".to_owned(),
                                },
                                _ => cell_value.clone(),
                            };
                            if !display_value.is_empty() {
                                cells.push((row, col, display_value));
                            }
                        }
                        in_cell = false;
                        // Continue the loop to look for more <c> tags on this line
                    } else {
                        break; // Cell not closed yet — move to next line
                    }
                }
            }
        }
        cells
    }

    /// Parse a worksheet XML to extract cells with formula awareness.
    fn parse_sheet_xml_with_formulas(xml: &str, shared_strings: &[String]) -> Vec<FormulaCell> {
        let mut cells = Vec::new();
        let mut in_cell = false;
        let mut cell_ref = String::new();
        let mut cell_type = String::new();
        let mut cell_value = String::new();
        let mut cell_formula = String::new();
        let mut has_formula = false;
        let mut in_inline_str = false;

        for line in xml.lines() {
            let trimmed = line.trim();

            let mut search_from = 0;
            loop {
                let c_pos = if in_cell {
                    None
                } else {
                    let remaining = &trimmed[search_from..];
                    remaining
                        .find("<c ")
                        .or_else(|| remaining.find("<c>"))
                        .map(|pos| search_from + pos)
                };

                if !in_cell {
                    match c_pos {
                        Some(pos) => {
                            in_cell = true;
                            cell_ref.clear();
                            cell_type.clear();
                            cell_value.clear();
                            cell_formula.clear();
                            has_formula = false;
                            in_inline_str = false;

                            let c_tag = &trimmed[pos..];
                            let c_tag_end = c_tag.find('>').unwrap_or(c_tag.len());
                            let c_tag_only = &c_tag[..c_tag_end];

                            if let Some(r) = Self::extract_attr(c_tag_only, "r") {
                                cell_ref = r;
                            }
                            if let Some(t) = Self::extract_attr(c_tag_only, "t") {
                                cell_type = t;
                            }
                            search_from = pos + c_tag_end;
                        }
                        None => break,
                    }
                }

                if in_cell {
                    let cell_region = &trimmed[search_from..];

                    // Extract formula from <f>...</f>
                    if let Some(start) = cell_region.find("<f>") {
                        let after = &cell_region[start + 3..];
                        if let Some(end) = after.find("</f>") {
                            cell_formula = after[..end].to_string();
                            has_formula = true;
                        }
                    } else if let Some(start) = cell_region.find("<f ") {
                        let after = &cell_region[start + 3..];
                        if let Some(end) = after.find("</f>") {
                            cell_formula = after[..end].to_string();
                            has_formula = true;
                        }
                    }

                    if let Some(start) = cell_region.find("<v>") {
                        let after = &cell_region[start + 3..];
                        if let Some(end) = after.find("</v>") {
                            cell_value = after[..end].to_string();
                        }
                    }

                    if cell_region.contains("<is>") || cell_region.contains("<is ") {
                        in_inline_str = true;
                    }
                    if in_inline_str {
                        if let Some(start) = cell_region.find("<t>") {
                            let after = &cell_region[start + 3..];
                            if let Some(end) = after.find("</t>") {
                                cell_value = after[..end].to_string();
                            }
                        }
                        if cell_region.contains("</is>") {
                            in_inline_str = false;
                        }
                    }

                    if cell_region.contains("</c>") {
                        if let Some((row, col)) = Self::parse_a1(&cell_ref) {
                            let display_value = match cell_type.as_str() {
                                "s" => cell_value
                                    .parse::<usize>()
                                    .ok()
                                    .and_then(|idx| shared_strings.get(idx).cloned())
                                    .unwrap_or_else(|| cell_value.clone()),
                                "b" => match cell_value.as_str() {
                                    "1" | "true" => "TRUE".to_owned(),
                                    _ => "FALSE".to_owned(),
                                },
                                _ => cell_value.clone(),
                            };
                            if !display_value.is_empty() || has_formula {
                                cells.push(FormulaCell {
                                    row,
                                    col,
                                    formula: if has_formula {
                                        Some(cell_formula.clone())
                                    } else {
                                        None
                                    },
                                    value: display_value,
                                });
                            }
                        }
                        in_cell = false;
                    } else {
                        break;
                    }
                }
            }
        }
        cells
    }

    /// Formula-aware three-way merge of cells.
    ///
    /// When both sides modify the same cell's formula, flag as conflict.
    /// When one side changes formula and the other changes value, flag as conflict.
    /// When changes are in different cells, merge cleanly.
    fn merge_cells_formula_aware(
        base: &[FormulaCell],
        ours: &[FormulaCell],
        theirs: &[FormulaCell],
    ) -> Option<Vec<FormulaCell>> {
        let base_map: HashMap<(usize, usize), &FormulaCell> =
            base.iter().map(|c| ((c.row, c.col), c)).collect();
        let ours_map: HashMap<(usize, usize), &FormulaCell> =
            ours.iter().map(|c| ((c.row, c.col), c)).collect();
        let theirs_map: HashMap<(usize, usize), &FormulaCell> =
            theirs.iter().map(|c| ((c.row, c.col), c)).collect();

        let all_keys: BTreeSet<_> = base_map
            .keys()
            .chain(ours_map.keys())
            .chain(theirs_map.keys())
            .collect();
        let mut merged = Vec::new();

        for &(row, col) in all_keys {
            let b = base_map.get(&(row, col));
            let o = ours_map.get(&(row, col));
            let t = theirs_map.get(&(row, col));

            match (b, o, t) {
                (None, None, None) => unreachable!(),
                (None, Some(o), None) => merged.push((*o).clone()),
                (None, None, Some(t)) => merged.push((*t).clone()),
                (None, Some(o), Some(t)) => {
                    if o.formula == t.formula && o.value == t.value {
                        merged.push((*o).clone());
                    } else {
                        return None;
                    }
                }
                (Some(_), Some(o), None) => merged.push((*o).clone()),
                (Some(_), None, Some(t)) => merged.push((*t).clone()),
                (Some(_), None, None) => {}
                (Some(b), Some(o), Some(t)) => {
                    let ours_changed_formula = o.formula != b.formula;
                    let ours_changed_value = o.value != b.value;
                    let theirs_changed_formula = t.formula != b.formula;
                    let theirs_changed_value = t.value != b.value;

                    // Both changed formula in same cell -> conflict
                    if ours_changed_formula && theirs_changed_formula && o.formula != t.formula {
                        return None;
                    }

                    // One side changed formula, other changed value -> conflict
                    if ours_changed_formula && !theirs_changed_formula && theirs_changed_value {
                        return None;
                    }
                    if theirs_changed_formula && !ours_changed_formula && ours_changed_value {
                        return None;
                    }

                    // Clean merge: use standard three-way logic
                    if o.value == t.value && o.formula == t.formula {
                        merged.push((*o).clone());
                    } else if o.value == b.value && o.formula == b.formula {
                        merged.push((*t).clone());
                    } else if t.value == b.value && t.formula == b.formula {
                        merged.push((*o).clone());
                    } else {
                        return None;
                    }
                }
            }
        }
        Some(merged)
    }

    /// Parse all sheets from an XLSX document.
    #[allow(clippy::type_complexity)]
    fn parse_sheets(doc: &OoxmlDocument) -> Result<Vec<SheetData>, DriverError> {
        let shared_strings = Self::parse_shared_strings(doc);

        let mut sheet_files: Vec<String> = doc
            .parts
            .keys()
            .filter(|k| k.contains("worksheets/") && k.ends_with(".xml"))
            .cloned()
            .collect();
        sheet_files.sort();

        let mut sheets = Vec::new();
        for path in &sheet_files {
            let part = doc
                .get_part(path)
                .ok_or_else(|| DriverError::ParseError(format!("sheet part {path} missing")))?;
            let name = path.rsplit('/').next().unwrap_or("sheet");
            let name = name.strip_suffix(".xml").unwrap_or(name);
            let cells = Self::parse_sheet_xml(&part.content, &shared_strings);
            sheets.push((name.to_owned(), cells));
        }

        Ok(sheets)
    }

    fn diff_cells(
        base_cells: &[Cell],
        new_cells: &[Cell],
        sheet_name: &str,
    ) -> Vec<SemanticChange> {
        let base_map: HashMap<(usize, usize), &String> =
            base_cells.iter().map(|(r, c, v)| ((*r, *c), v)).collect();
        let new_map: HashMap<(usize, usize), &String> =
            new_cells.iter().map(|(r, c, v)| ((*r, *c), v)).collect();

        let mut changes = Vec::new();
        let all_keys: BTreeSet<_> = base_map.keys().chain(new_map.keys()).collect();

        for (row, col) in all_keys {
            let col_letter = col_to_letter(*col);
            let path = format!("/{sheet_name}/{col_letter}/{row}");
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

    #[cfg(test)]
    fn merge_cells(base: &[Cell], ours: &[Cell], theirs: &[Cell]) -> Option<Vec<Cell>> {
        let base_map: HashMap<(usize, usize), &String> =
            base.iter().map(|(r, c, v)| ((*r, *c), v)).collect();
        let ours_map: HashMap<(usize, usize), &String> =
            ours.iter().map(|(r, c, v)| ((*r, *c), v)).collect();
        let theirs_map: HashMap<(usize, usize), &String> =
            theirs.iter().map(|(r, c, v)| ((*r, *c), v)).collect();

        let all_keys: BTreeSet<_> = base_map
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
                (None | Some(_), None, Some(t)) => merged.push((row, col, t.to_owned())),
                (None, Some(o), Some(t)) => {
                    if o == t {
                        merged.push((row, col, o.to_owned()));
                    } else {
                        return None;
                    }
                }
                (None | Some(_), Some(o), None) => merged.push((row, col, o.to_owned())),
                (Some(_) | None, None, None) => {}
                (Some(b), Some(o), Some(t)) => {
                    if o == t {
                        merged.push((row, col, o.to_owned()));
                    } else if o == b {
                        merged.push((row, col, t.to_owned()));
                    } else if t == b {
                        merged.push((row, col, o.to_owned()));
                    } else {
                        return None;
                    }
                }
            }
        }
        Some(merged)
    }
}

/// Convert a 1-based column number to a column letter (A=1, Z=26, AA=27).
fn col_to_letter(col: usize) -> String {
    let mut n = col;
    let mut result = String::new();
    while n > 0 {
        n -= 1;
        result.insert(0, (b'A' + (n % 26) as u8) as char);
        n /= 26;
    }
    result
}

impl Default for XlsxDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl SutureDriver for XlsxDriver {
    fn name(&self) -> &'static str {
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
                .map_or(&[][..], |(_, c)| c.as_slice());
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
            return Ok("no changes".to_owned());
        }
        let lines: Vec<String> = changes
            .iter()
            .map(|c| match c {
                SemanticChange::Added { path, value } => format!("  ADDED     {path}: {value}"),
                SemanticChange::Removed { path, old_value } => {
                    format!("  REMOVED   {path}: {old_value}")
                }
                SemanticChange::Modified {
                    path,
                    old_value,
                    new_value,
                } => format!("  MODIFIED  {path}: {old_value} -> {new_value}"),
                SemanticChange::Moved {
                    old_path,
                    new_path,
                    value,
                } => format!("  MOVED     {old_path} -> {new_path}: {value}"),
            })
            .collect();
        Ok(lines.join("\n"))
    }

    fn merge(&self, base: &str, ours: &str, theirs: &str) -> Result<Option<String>, DriverError> {
        let bytes = self.merge_raw(base.as_bytes(), ours.as_bytes(), theirs.as_bytes())?;
        Ok(bytes.map(bytes_to_string_lossy))
    }

    fn merge_raw(
        &self,
        base: &[u8],
        ours: &[u8],
        theirs: &[u8],
    ) -> Result<Option<Vec<u8>>, DriverError> {
        let base_doc =
            OoxmlDocument::from_bytes(base).map_err(|e| DriverError::ParseError(e.to_string()))?;
        let ours_doc =
            OoxmlDocument::from_bytes(ours).map_err(|e| DriverError::ParseError(e.to_string()))?;
        let theirs_doc = OoxmlDocument::from_bytes(theirs)
            .map_err(|e| DriverError::ParseError(e.to_string()))?;

        let base_sheets = Self::parse_sheets_with_formulas(&base_doc)?;
        let ours_sheets = Self::parse_sheets_with_formulas(&ours_doc)?;
        let theirs_sheets = Self::parse_sheets_with_formulas(&theirs_doc)?;

        let all_names: BTreeSet<&str> = base_sheets
            .iter()
            .chain(ours_sheets.iter())
            .chain(theirs_sheets.iter())
            .map(|(n, _)| n.as_str())
            .collect();

        let mut merged_sheets: Vec<(String, Vec<FormulaCell>)> = Vec::new();
        for &name in &all_names {
            let base_cells = base_sheets
                .iter()
                .find(|(n, _)| n == name)
                .map_or(&[][..], |(_, c)| c.as_slice());
            let ours_cells = ours_sheets
                .iter()
                .find(|(n, _)| n == name)
                .map_or(&[][..], |(_, c)| c.as_slice());
            let theirs_cells = theirs_sheets
                .iter()
                .find(|(n, _)| n == name)
                .map_or(&[][..], |(_, c)| c.as_slice());

            match Self::merge_cells_formula_aware(base_cells, ours_cells, theirs_cells) {
                Some(cells) => merged_sheets.push((name.to_owned(), cells)),
                None => return Ok(None),
            }
        }

        let mut doc =
            OoxmlDocument::from_bytes(base).map_err(|e| DriverError::ParseError(e.to_string()))?;

        let mut name_to_path: HashMap<String, String> = HashMap::new();
        for path in doc.parts.keys() {
            if path.contains("worksheets/") && path.ends_with(".xml") {
                let sheet_name = path.rsplit('/').next().unwrap_or("sheet");
                let sheet_name = sheet_name.strip_suffix(".xml").unwrap_or(sheet_name);
                name_to_path.insert(sheet_name.to_owned(), path.clone());
            }
        }

        for (name, cells) in &merged_sheets {
            if let Some(path) = name_to_path.get(name)
                && let Some(part) = doc.parts.get_mut(path)
            {
                part.content = Self::rebuild_sheet_xml_with_formulas(&part.content, cells);
            }
        }

        let bytes = doc
            .to_bytes()
            .map_err(|e| DriverError::SerializationError(e.to_string()))?;
        Ok(Some(bytes))
    }

    fn diff_raw(
        &self,
        base: Option<&[u8]>,
        new_content: &[u8],
    ) -> Result<Vec<SemanticChange>, DriverError> {
        let new_doc = OoxmlDocument::from_bytes(new_content)
            .map_err(|e| DriverError::ParseError(e.to_string()))?;
        let new_sheets = Self::parse_sheets(&new_doc)?;

        #[allow(clippy::type_complexity)]
        let base_sheets: Vec<SheetData> = match base {
            None => Vec::new(),
            Some(b) => {
                let base_doc = OoxmlDocument::from_bytes(b)
                    .map_err(|e| DriverError::ParseError(e.to_string()))?;
                Self::parse_sheets(&base_doc)?
            }
        };

        let mut changes = Vec::new();
        for (name, cells) in &new_sheets {
            let base_cells = base_sheets
                .iter()
                .find(|(n, _)| n == name)
                .map_or(&[][..], |(_, c)| c.as_slice());
            changes.extend(Self::diff_cells(base_cells, cells, name));
        }
        Ok(changes)
    }
}

impl XlsxDriver {
    /// Rebuild a worksheet XML by replacing the `<sheetData>` section
    /// with cells from the merged result.
    ///
    /// This preserves everything outside `<sheetData>...</sheetData>`
    /// (column widths, sheet views, merge cells, etc.) and only replaces
    /// the actual cell data.
    #[cfg(test)]
    fn rebuild_sheet_xml(original_xml: &str, cells: &[Cell]) -> String {
        // Find <sheetData> and </sheetData> boundaries
        let data_start = match original_xml.find("<sheetData") {
            Some(pos) => {
                // Find the closing > of the opening tag
                let after = &original_xml[pos..];

                after.find('>').map_or(pos, |i| pos + i + 1)
            }
            None => return original_xml.to_owned(),
        };

        let Some(data_end) = original_xml.find("</sheetData>") else {
            return original_xml.to_owned();
        };

        // Build new sheetData content
        let mut rows: BTreeMap<usize, Vec<(usize, &String)>> = BTreeMap::new();
        for &(row, col, ref val) in cells {
            rows.entry(row).or_default().push((col, val));
        }

        let mut new_data = String::from("<sheetData>");
        for (row_num, cols) in &rows {
            let _ = write!(new_data, "<row r=\"{row_num}\">");
            for (col, val) in cols {
                let col_letter = col_to_letter(*col);
                let ref_str = format!("{col_letter}{row_num}");
                // Use inlineStr for all string values to avoid shared string table issues
                if val.parse::<f64>().is_ok() {
                    let _ = write!(new_data, "<c r=\"{ref_str}\"><v>{val}</v></c>");
                } else if *val == "TRUE" || *val == "FALSE" {
                    let bval = if *val == "TRUE" { "1" } else { "0" };
                    let _ = write!(new_data, "<c r=\"{ref_str}\" t=\"b\"><v>{bval}</v></c>");
                } else {
                    let _ = write!(
                        new_data,
                        "<c r=\"{ref_str}\" t=\"inlineStr\"><is><t>{val}</t></is></c>"
                    );
                }
            }
            new_data.push_str("</row>");
        }
        new_data.push_str("</sheetData>");

        // Reassemble: before sheetData + new sheetData + after sheetData
        let mut result = String::new();
        result.push_str(&original_xml[..data_start]);
        result.push_str(&new_data);
        result.push_str(&original_xml[data_end + "</sheetData>".len()..]);
        result
    }

    #[allow(clippy::type_complexity)]
    fn parse_sheets_with_formulas(
        doc: &OoxmlDocument,
    ) -> Result<Vec<(String, Vec<FormulaCell>)>, DriverError> {
        let shared_strings = Self::parse_shared_strings(doc);

        let mut sheet_files: Vec<String> = doc
            .parts
            .keys()
            .filter(|k| k.contains("worksheets/") && k.ends_with(".xml"))
            .cloned()
            .collect();
        sheet_files.sort();

        let mut sheets = Vec::new();
        for path in &sheet_files {
            let part = doc
                .get_part(path)
                .ok_or_else(|| DriverError::ParseError(format!("sheet part {path} missing")))?;
            let name = path.rsplit('/').next().unwrap_or("sheet");
            let name = name.strip_suffix(".xml").unwrap_or(name);
            let cells = Self::parse_sheet_xml_with_formulas(&part.content, &shared_strings);
            sheets.push((name.to_owned(), cells));
        }

        Ok(sheets)
    }

    fn rebuild_sheet_xml_with_formulas(original_xml: &str, cells: &[FormulaCell]) -> String {
        let data_start = match original_xml.find("<sheetData") {
            Some(pos) => {
                let after = &original_xml[pos..];
                after.find('>').map_or(pos, |i| pos + i + 1)
            }
            None => return original_xml.to_owned(),
        };

        let Some(data_end) = original_xml.find("</sheetData>") else {
            return original_xml.to_owned();
        };

        let mut rows: BTreeMap<usize, Vec<(usize, &FormulaCell)>> = BTreeMap::new();
        for cell in cells {
            rows.entry(cell.row).or_default().push((cell.col, cell));
        }

        let mut new_data = String::from("<sheetData>");
        for (row_num, cols) in &rows {
            let _ = write!(new_data, "<row r=\"{row_num}\">");
            for (_col, cell) in cols {
                let col_letter = col_to_letter(cell.col);
                let ref_str = format!("{col_letter}{row_num}");

                let mut cell_xml = format!("<c r=\"{ref_str}\"");
                if let Some(ref formula) = cell.formula {
                    let _ = write!(cell_xml, "><f>{formula}</f><v>{}</v></c>", cell.value);
                } else if cell.value.parse::<f64>().is_ok() {
                    let _ = write!(cell_xml, "><v>{}</v></c>", cell.value);
                } else if cell.value == "TRUE" || cell.value == "FALSE" {
                    let bval = if cell.value == "TRUE" { "1" } else { "0" };
                    let _ = write!(cell_xml, " t=\"b\"><v>{bval}</v></c>");
                } else {
                    let _ = write!(
                        cell_xml,
                        " t=\"inlineStr\"><is><t>{}</t></is></c>",
                        cell.value
                    );
                }
                new_data.push_str(&cell_xml);
            }
            new_data.push_str("</row>");
        }
        new_data.push_str("</sheetData>");

        let mut result = String::new();
        result.push_str(&original_xml[..data_start]);
        result.push_str(&new_data);
        result.push_str(&original_xml[data_end + "</sheetData>".len()..]);
        result
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
    fn test_col_from_a1() {
        assert_eq!(XlsxDriver::col_from_a1("A"), 1);
        assert_eq!(XlsxDriver::col_from_a1("B"), 2);
        assert_eq!(XlsxDriver::col_from_a1("Z"), 26);
        assert_eq!(XlsxDriver::col_from_a1("AA"), 27);
        assert_eq!(XlsxDriver::col_from_a1("AB"), 28);
        assert_eq!(XlsxDriver::col_from_a1("AZ"), 52);
        assert_eq!(XlsxDriver::col_from_a1("BA"), 53);
    }

    #[test]
    fn test_parse_a1() {
        assert_eq!(XlsxDriver::parse_a1("A1"), Some((1, 1)));
        assert_eq!(XlsxDriver::parse_a1("B3"), Some((3, 2)));
        assert_eq!(XlsxDriver::parse_a1("AA42"), Some((42, 27)));
        assert_eq!(XlsxDriver::parse_a1("Z1"), Some((1, 26)));
        assert_eq!(XlsxDriver::parse_a1("123"), None); // No column letters
        assert_eq!(XlsxDriver::parse_a1("ABC"), None); // No row number
    }

    #[test]
    fn test_col_to_letter() {
        assert_eq!(col_to_letter(1), "A");
        assert_eq!(col_to_letter(2), "B");
        assert_eq!(col_to_letter(26), "Z");
        assert_eq!(col_to_letter(27), "AA");
        assert_eq!(col_to_letter(28), "AB");
    }

    #[test]
    fn test_parse_shared_strings() {
        let xml = r#"<?xml version="1.0"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="3" uniqueCount="3">
  <si><t>Hello</t></si>
  <si><t>World</t></si>
  <si><t>Test</t></si>
</sst>"#;
        let mut doc_parts = std::collections::HashMap::new();
        doc_parts.insert(
            "xl/sharedStrings.xml".to_string(),
            suture_ooxml::OoxmlPart {
                path: "xl/sharedStrings.xml".to_string(),
                content: xml.to_string(),
                content_type: String::new(),
            },
        );
        let doc = OoxmlDocument {
            parts: doc_parts,
            binary_parts: HashMap::new(),
            content_types: String::new(),
            rels: HashMap::new(),
            part_rels: HashMap::new(),
        };
        let strings = XlsxDriver::parse_shared_strings(&doc);
        assert_eq!(strings.len(), 3);
        assert_eq!(strings[0], "Hello");
        assert_eq!(strings[1], "World");
        assert_eq!(strings[2], "Test");
    }

    #[test]
    fn test_parse_sheet_xml_with_shared_strings() {
        let xml = r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<sheetData>
<row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1"><v>42</v></c></row>
<row r="2"><c r="A2" t="s"><v>1</v></c></row>
</sheetData>
</worksheet>"#;
        let shared = vec!["Hello".to_string(), "World".to_string()];
        let cells = XlsxDriver::parse_sheet_xml(xml, &shared);
        assert_eq!(cells.len(), 3);
        assert_eq!(cells[0], (1, 1, "Hello".to_string()));
        assert_eq!(cells[1], (1, 2, "42".to_string()));
        assert_eq!(cells[2], (2, 1, "World".to_string()));
    }

    #[test]
    fn test_parse_sheet_xml_with_inline_strings() {
        let xml = r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<sheetData>
<row r="1"><c r="A1" t="inlineStr"><is><t>Direct text</t></is></c></row>
</sheetData>
</worksheet>"#;
        let cells = XlsxDriver::parse_sheet_xml(xml, &[]);
        assert_eq!(cells.len(), 1);
        assert_eq!(cells[0], (1, 1, "Direct text".to_string()));
    }

    #[test]
    fn test_parse_sheet_xml_with_booleans() {
        let xml = r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<sheetData>
<row r="1"><c r="A1" t="b"><v>1</v></c><c r="B1" t="b"><v>0</v></c></row>
</sheetData>
</worksheet>"#;
        let cells = XlsxDriver::parse_sheet_xml(xml, &[]);
        assert_eq!(cells.len(), 2);
        assert_eq!(cells[0], (1, 1, "TRUE".to_string()));
        assert_eq!(cells[1], (1, 2, "FALSE".to_string()));
    }

    #[test]
    fn test_diff_cells() {
        let base = vec![(1, 1, "A".to_string()), (1, 2, "B".to_string())];
        let new = vec![
            (1, 1, "X".to_string()),
            (1, 2, "B".to_string()),
            (2, 1, "C".to_string()),
        ];
        let changes = XlsxDriver::diff_cells(&base, &new, "Sheet1");
        assert!(
            changes
                .iter()
                .any(|c| matches!(c, SemanticChange::Modified { .. }))
        );
        assert!(
            changes
                .iter()
                .any(|c| matches!(c, SemanticChange::Added { value, .. } if value == "C"))
        );
    }

    #[test]
    fn test_merge_cells_no_conflict() {
        let base = vec![(1, 1, "A".to_string()), (1, 2, "B".to_string())];
        let ours = vec![(1, 1, "X".to_string())];
        let theirs = vec![(1, 2, "Y".to_string())];
        let result = XlsxDriver::merge_cells(&base, &ours, &theirs);
        assert!(result.is_some());
        let m = result.unwrap();
        assert_eq!(
            m.iter()
                .find(|(r, c, _)| *r == 1 && *c == 1)
                .map(|(_, _, v)| v.as_str()),
            Some("X")
        );
        assert_eq!(
            m.iter()
                .find(|(r, c, _)| *r == 1 && *c == 2)
                .map(|(_, _, v)| v.as_str()),
            Some("Y")
        );
    }

    #[test]
    fn test_merge_cells_conflict() {
        let base = vec![(1, 1, "A".to_string())];
        let ours = vec![(1, 1, "X".to_string())];
        let theirs = vec![(1, 1, "Y".to_string())];
        assert!(XlsxDriver::merge_cells(&base, &ours, &theirs).is_none());
    }

    #[test]
    fn test_rebuild_sheet_xml() {
        let original = r#"<?xml version="1.0"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<sheetFormatPr defaultColWidth="10"/>
<sheetData><row r="1"><c r="A1"><v>old</v></c></row></sheetData>
<sheetViews><sheetView tabSelected="1"/></sheetViews>
</worksheet>"#;

        let cells = vec![(1, 1, "new".to_string()), (2, 1, "added".to_string())];
        let rebuilt = XlsxDriver::rebuild_sheet_xml(original, &cells);

        assert!(rebuilt.contains("<sheetFormatPr")); // Preserved
        assert!(rebuilt.contains("<sheetViews>")); // Preserved
        assert!(rebuilt.contains("new")); // New cell value
        assert!(rebuilt.contains("added")); // Added cell value
        assert!(!rebuilt.contains("old")); // Old value replaced
    }

    // === Formula-aware merge tests ===

    fn fc(row: usize, col: usize, formula: &str, value: &str) -> FormulaCell {
        FormulaCell {
            row,
            col,
            formula: Some(formula.to_string()),
            value: value.to_string(),
        }
    }

    fn vc(row: usize, col: usize, value: &str) -> FormulaCell {
        FormulaCell {
            row,
            col,
            formula: None,
            value: value.to_string(),
        }
    }

    #[test]
    fn test_parse_sheet_xml_with_formulas() {
        let xml = r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<sheetData>
<row r="1"><c r="A1"><f>SUM(B1:B10)</f><v>100</v></c><c r="B1"><v>42</v></c></row>
</sheetData>
</worksheet>"#;
        let cells = XlsxDriver::parse_sheet_xml_with_formulas(xml, &[]);
        assert_eq!(cells.len(), 2);
        assert_eq!(cells[0].formula.as_deref(), Some("SUM(B1:B10)"));
        assert_eq!(cells[0].value, "100");
        assert_eq!(cells[1].formula, None);
        assert_eq!(cells[1].value, "42");
    }

    #[test]
    fn test_formula_conflict_both_change_same_cell_formula() {
        let base = vec![fc(1, 1, "SUM(A1:A10)", "100")];
        let ours = vec![fc(1, 1, "SUM(A1:A20)", "200")];
        let theirs = vec![fc(1, 1, "AVERAGE(A1:A10)", "50")];

        let result = XlsxDriver::merge_cells_formula_aware(&base, &ours, &theirs);
        assert!(
            result.is_none(),
            "both sides changing formula in same cell should conflict"
        );
    }

    #[test]
    fn test_formula_conflict_one_changes_formula_other_changes_value() {
        let base = vec![fc(1, 1, "SUM(A1:A10)", "100")];
        let ours = vec![fc(1, 1, "SUM(A1:A20)", "200")];
        let theirs = vec![fc(1, 1, "SUM(A1:A10)", "150")];

        let result = XlsxDriver::merge_cells_formula_aware(&base, &ours, &theirs);
        assert!(
            result.is_none(),
            "one side changing formula and other changing value should conflict"
        );
    }

    #[test]
    fn test_formula_clean_merge_different_cells() {
        let base = vec![fc(1, 1, "SUM(A1:A10)", "100"), vc(1, 2, "Label")];
        let ours = vec![fc(1, 1, "SUM(A1:A20)", "200"), vc(1, 2, "Label")];
        let theirs = vec![fc(1, 1, "SUM(A1:A10)", "100"), vc(1, 2, "Changed Label")];

        let result = XlsxDriver::merge_cells_formula_aware(&base, &ours, &theirs);
        assert!(
            result.is_some(),
            "changes in different cells should merge cleanly"
        );
        let merged = result.unwrap();
        let a1 = merged.iter().find(|c| c.row == 1 && c.col == 1).unwrap();
        assert_eq!(a1.formula.as_deref(), Some("SUM(A1:A20)"));
        assert_eq!(a1.value, "200");
        let b1 = merged.iter().find(|c| c.row == 1 && c.col == 2).unwrap();
        assert_eq!(b1.value, "Changed Label");
    }

    #[test]
    fn test_formula_both_same_formula_change_clean() {
        let base = vec![fc(1, 1, "SUM(A1:A10)", "100")];
        let ours = vec![fc(1, 1, "SUM(A1:A20)", "200")];
        let theirs = vec![fc(1, 1, "SUM(A1:A20)", "200")];

        let result = XlsxDriver::merge_cells_formula_aware(&base, &ours, &theirs);
        assert!(
            result.is_some(),
            "both sides making identical formula changes should merge cleanly"
        );
    }

    #[test]
    fn test_rebuild_sheet_xml_with_formulas() {
        let original = r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<sheetFormatPr defaultColWidth="10"/>
<sheetData><row r="1"><c r="A1"><v>old</v></c></row></sheetData>
<sheetViews><sheetView tabSelected="1"/></sheetViews>
</worksheet>"#;

        let cells = vec![fc(1, 1, "SUM(B1:B10)", "100"), vc(2, 1, "Header")];
        let rebuilt = XlsxDriver::rebuild_sheet_xml_with_formulas(original, &cells);

        assert!(rebuilt.contains("<sheetFormatPr"));
        assert!(rebuilt.contains("<f>SUM(B1:B10)</f>"));
        assert!(rebuilt.contains("<v>100</v>"));
        assert!(rebuilt.contains("Header"));
        assert!(!rebuilt.contains("old"));
    }
}

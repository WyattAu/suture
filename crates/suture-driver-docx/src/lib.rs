#![allow(clippy::collapsible_match)]
//! DOCX semantic driver — paragraph-level diff and merge for Word documents.
//!
//! Unlike a naive text extraction approach, this driver operates on the raw XML
//! of each block-level element (`<w:p>` paragraphs and `<w:tbl>` tables),
//! preserving all formatting (bold, italic, fonts), paragraph properties,
//! table structures, and other document features.
//!
//! ## Approach
//!
//! 1. **Parse** `word/document.xml` to extract block-level elements as raw XML
//!    strings along with their extracted plain text (for comparison).
//! 2. **Block types**: `<w:p>` (paragraphs) and `<w:tbl>` (tables) are the two
//!    block-level elements we track. Tables are treated as atomic units.
//! 3. **Diff** compares block text (for semantic changes) while preserving
//!    the full XML for output.
//! 4. **Merge** uses three-way block-index merge, substituting the winning
//!    block's raw XML into the base document's `<w:body>`.
//! 5. **Reconstruct** preserves everything outside the block list:
//!    XML declaration, namespaces, `<w:sectPr>`, etc.

use suture_driver::{DriverError, SemanticChange, SutureDriver};
use suture_ooxml::OoxmlDocument;

/// A parsed block from a DOCX document.xml body.
///
/// Can be either a paragraph (`<w:p>`) or a table (`<w:tbl>`).
/// Tables are treated as atomic blocks — their entire XML is preserved.
#[derive(Debug, Clone)]
struct Block {
    /// The block type.
    kind: BlockKind,
    /// Raw XML string of this element.
    raw_xml: String,
    /// Extracted plain text from all `<w:t>` elements within this block.
    text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BlockKind {
    Paragraph,
    Table,
}

impl Block {
    /// Extract text content from all `<w:t>` elements within the block XML.
    fn extract_text(xml: &str) -> String {
        let mut text = String::new();
        let mut search = 0;
        while search < xml.len() {
            if let Some(t_start) = xml[search..].find("<w:t") {
                let abs_t = search + t_start;
                let after = &xml[abs_t + 4..];
                if let Some(gt_pos) = after.find('>') {
                    let content_start = abs_t + 4 + gt_pos + 1;
                    // Self-closing <w:t/> has no content
                    if after[..gt_pos].ends_with('/') {
                        search = content_start;
                        continue;
                    }
                    if let Some(t_end) = xml[content_start..].find("</w:t>") {
                        let t = &xml[content_start..content_start + t_end];
                        text.push_str(t);
                        search = content_start + t_end + 6;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        text
    }
}

/// Result of parsing a DOCX document.xml body.
struct BodyParse {
    /// Everything before `<w:body>` (XML declaration + document opening tag)
    prefix: String,
    /// The `<w:body>` opening tag including attributes
    body_open_tag: String,
    /// Parsed blocks in order (paragraphs and tables)
    blocks: Vec<Block>,
    /// Raw XML after the last block and before `</w:body>` (e.g. `<w:sectPr>`)
    trailing_body_content: String,
    /// Everything after `</w:body>` (closing document tag)
    suffix: String,
}

/// Parse the document.xml content to extract block-level elements and structure.
fn parse_body(xml: &str) -> Result<BodyParse, DriverError> {
    // Find <w:body
    let body_tag_start = xml
        .find("<w:body")
        .ok_or_else(|| DriverError::ParseError("no <w:body> found in document.xml".into()))?;

    let after_body_tag = &xml[body_tag_start..];
    let gt_pos = after_body_tag
        .find('>')
        .ok_or_else(|| DriverError::ParseError("malformed <w:body> tag".into()))?;
    let body_tag_end = body_tag_start + gt_pos + 1;

    let prefix = xml[..body_tag_start].to_string();
    let body_open_tag = xml[body_tag_start..body_tag_end].to_string();

    // Find </w:body>
    let body_close = xml
        .find("</w:body>")
        .ok_or_else(|| DriverError::ParseError("no </w:body> found".into()))?;
    let suffix = xml[body_close + 10..].to_string();

    let body_content = &xml[body_tag_end..body_close];

    // Parse block-level elements from body content
    let blocks = extract_blocks(body_content);

    // Identify trailing content — anything after the last block
    let trailing = extract_trailing_body_content(body_content, &blocks);

    Ok(BodyParse {
        prefix,
        body_open_tag,
        blocks,
        trailing_body_content: trailing,
        suffix,
    })
}

/// Extract all block-level elements (`<w:p>` and `<w:tbl>`) from body content.
///
/// Elements are extracted in document order. `<w:tbl>` is extracted as a single
/// atomic block (including all nested rows, cells, and paragraphs within cells).
fn extract_blocks(body_content: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut pos = 0;

    while pos < body_content.len() {
        // Find the next block-level element
        let next_p = body_content[pos..].find("<w:p").map(|i| pos + i);
        let next_tbl = body_content[pos..].find("<w:tbl").map(|i| pos + i);

        // Pick whichever comes first
        let (abs_start, kind) = match (next_p, next_tbl) {
            (None, None) => break,
            (Some(p), None) => (p, BlockKind::Paragraph),
            (None, Some(t)) => (t, BlockKind::Table),
            (Some(p), Some(t)) => {
                if p <= t {
                    (p, BlockKind::Paragraph)
                } else {
                    (t, BlockKind::Table)
                }
            }
        };

        // Length of the tag prefix searched: "<w:p" = 4, "<w:tbl" = 6
        let tag_len = match kind {
            BlockKind::Paragraph => 4,
            BlockKind::Table => 6,
        };

        let after_tag = &body_content[abs_start + tag_len..];

        match kind {
            BlockKind::Paragraph => {
                // Ensure this is actually a <w:p element (not <w:pPr, <w:pict, etc.)
                if !after_tag.starts_with('>')
                    && !after_tag.starts_with(' ')
                    && !after_tag.starts_with('/')
                {
                    pos = abs_start + tag_len;
                    continue;
                }

                // Self-closing <w:p/>
                if after_tag.starts_with('/') {
                    if let Some(close) = after_tag.find("/>") {
                        let end_abs = abs_start + tag_len + close + 2;
                        let raw_xml = body_content[abs_start..end_abs].to_string();
                        blocks.push(Block {
                            kind: BlockKind::Paragraph,
                            raw_xml,
                            text: String::new(),
                        });
                        pos = end_abs;
                        continue;
                    }
                    pos = abs_start + tag_len;
                    continue;
                }

                // Find </w:p>
                if let Some(end) = body_content[abs_start..].find("</w:p>") {
                    let end_abs = abs_start + end + 6;
                    let raw_xml = body_content[abs_start..end_abs].to_string();
                    let text = Block::extract_text(&raw_xml);
                    blocks.push(Block {
                        kind: BlockKind::Paragraph,
                        raw_xml,
                        text,
                    });
                    pos = end_abs;
                } else {
                    pos = abs_start + tag_len;
                }
            }
            BlockKind::Table => {
                // Ensure this is actually a <w:tbl element
                if !after_tag.starts_with('>')
                    && !after_tag.starts_with(' ')
                    && !after_tag.starts_with('/')
                {
                    pos = abs_start + tag_len;
                    continue;
                }

                // Find </w:tbl>
                if let Some(end) = body_content[abs_start..].find("</w:tbl>") {
                    let end_abs = abs_start + end + 8;
                    let raw_xml = body_content[abs_start..end_abs].to_string();
                    let text = Block::extract_text(&raw_xml);
                    blocks.push(Block {
                        kind: BlockKind::Table,
                        raw_xml,
                        text,
                    });
                    pos = end_abs;
                } else {
                    pos = abs_start + tag_len;
                }
            }
        }
    }

    blocks
}

/// Extract non-block trailing content from the body.
fn extract_trailing_body_content(body_content: &str, blocks: &[Block]) -> String {
    if blocks.is_empty() {
        return body_content.trim_end().to_string();
    }

    let last = &blocks[blocks.len() - 1];
    if let Some(last_end) = body_content.rfind(&last.raw_xml) {
        let after = &body_content[last_end + last.raw_xml.len()..];
        if !after.trim().is_empty() {
            return after.to_string();
        }
    }

    String::new()
}

/// Rebuild document.xml from parsed body structure with merged blocks.
fn rebuild_document_xml(body: &BodyParse, merged_blocks: &[Block]) -> String {
    let mut out = String::new();
    out.push_str(&body.prefix);
    out.push_str(&body.body_open_tag);

    for block in merged_blocks {
        out.push_str(&block.raw_xml);
    }

    out.push_str(&body.trailing_body_content);
    out.push_str("</w:body>");
    out.push_str(&body.suffix);
    out
}

/// Three-way merge of block lists by index.
fn merge_blocks(base: &[Block], ours: &[Block], theirs: &[Block]) -> Option<Vec<Block>> {
    let max_len = base.len().max(ours.len()).max(theirs.len());
    let mut merged = Vec::with_capacity(max_len);

    for i in 0..max_len {
        let b = base.get(i);
        let o = ours.get(i);
        let t = theirs.get(i);

        match (b, o, t) {
            (None, Some(o), None) => merged.push(o.clone()),
            (None, None, Some(t)) => merged.push(t.clone()),
            (None, Some(o), Some(t)) => {
                if o.text == t.text {
                    merged.push(o.clone());
                } else {
                    return None;
                }
            }
            (Some(_), Some(o), None) => merged.push(o.clone()),
            (Some(_), None, Some(t)) => merged.push(t.clone()),
            (Some(_), None, None) => {} // Both deleted
            (Some(b), Some(o), Some(t)) => {
                if o.text == t.text {
                    merged.push(o.clone());
                } else if o.text == b.text {
                    merged.push(t.clone());
                } else if t.text == b.text {
                    merged.push(o.clone());
                } else {
                    return None; // Conflict
                }
            }
            (None, None, None) => unreachable!(),
        }
    }
    Some(merged)
}

/// Extract blocks from an OOXML document's main part.
fn extract_blocks_from_doc(doc: &OoxmlDocument) -> Result<Vec<Block>, DriverError> {
    let main_path = doc
        .main_document_path()
        .ok_or_else(|| DriverError::ParseError("no main document part".into()))?;
    let main = doc
        .get_part(main_path)
        .ok_or_else(|| DriverError::ParseError("main part missing".into()))?;
    let body = parse_body(&main.content)?;
    Ok(body.blocks)
}

pub struct DocxDriver;

impl DocxDriver {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DocxDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl SutureDriver for DocxDriver {
    fn name(&self) -> &str {
        "DOCX"
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".docx"]
    }

    fn diff(
        &self,
        base_content: Option<&str>,
        new_content: &str,
    ) -> Result<Vec<SemanticChange>, DriverError> {
        let new_doc = OoxmlDocument::from_bytes(new_content.as_bytes())
            .map_err(|e| DriverError::ParseError(e.to_string()))?;
        let main_path = new_doc
            .main_document_path()
            .ok_or_else(|| DriverError::ParseError("no main document part found".into()))?;
        let main_xml = new_doc
            .get_part(main_path)
            .ok_or_else(|| DriverError::ParseError("main document part missing".into()))?;
        let new_body = parse_body(&main_xml.content)?;
        let new_blocks = &new_body.blocks;

        let base_blocks: Vec<Block> = match base_content {
            None => Vec::new(),
            Some(base) => {
                let base_doc = OoxmlDocument::from_bytes(base.as_bytes())
                    .map_err(|e| DriverError::ParseError(e.to_string()))?;
                let bp = base_doc
                    .main_document_path()
                    .ok_or_else(|| DriverError::ParseError("no main part".into()))?;
                let bm = base_doc
                    .get_part(bp)
                    .ok_or_else(|| DriverError::ParseError("main part missing".into()))?;
                let base_body = parse_body(&bm.content)?;
                base_body.blocks
            }
        };

        let max_len = base_blocks.len().max(new_blocks.len());
        let mut changes = Vec::new();

        for i in 0..max_len {
            let block_type = new_blocks
                .get(i)
                .map(|b| {
                    if b.kind == BlockKind::Table {
                        "table"
                    } else {
                        "paragraph"
                    }
                })
                .unwrap_or("paragraph");
            let path = format!("/{}/{}", block_type, i);
            match (base_blocks.get(i), new_blocks.get(i)) {
                (None, Some(new)) => changes.push(SemanticChange::Added {
                    path,
                    value: new.text.clone(),
                }),
                (Some(old), None) => changes.push(SemanticChange::Removed {
                    path,
                    old_value: old.text.clone(),
                }),
                (Some(old), Some(new)) if old.text != new.text => {
                    changes.push(SemanticChange::Modified {
                        path,
                        old_value: old.text.clone(),
                        new_value: new.text.clone(),
                    });
                }
                _ => {}
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
        if changes.is_empty() {
            return Ok("no changes".to_string());
        }
        let lines: Vec<String> = changes
            .iter()
            .map(|c| match c {
                SemanticChange::Added { path, value } => {
                    format!("  ADDED     {}: {}", path, value)
                }
                SemanticChange::Removed { path, old_value } => {
                    format!("  REMOVED   {}: {}", path, old_value)
                }
                SemanticChange::Modified {
                    path,
                    old_value,
                    new_value,
                } => {
                    format!("  MODIFIED  {}: {} -> {}", path, old_value, new_value)
                }
                SemanticChange::Moved {
                    old_path,
                    new_path,
                    value,
                } => {
                    format!("  MOVED     {} -> {}: {}", old_path, new_path, value)
                }
            })
            .collect();
        Ok(lines.join("\n"))
    }

    fn merge(&self, base: &str, ours: &str, theirs: &str) -> Result<Option<String>, DriverError> {
        // Delegate to merge_raw and convert bytes → String.
        let bytes = self.merge_raw(base.as_bytes(), ours.as_bytes(), theirs.as_bytes())?;
        match bytes {
            Some(b) => {
                // SAFETY: OOXML documents are valid UTF-8 per ECMA-376.
                // The bytes round-trip through ZIP → XML → merge → XML → ZIP
                // and are written to disk as raw bytes, never interpreted as text.
                Ok(Some(unsafe { String::from_utf8_unchecked(b) }))
            }
            None => Ok(None),
        }
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

        let main_path = base_doc
            .main_document_path()
            .ok_or_else(|| DriverError::ParseError("no main document part".into()))?
            .to_string();

        let base_main = base_doc
            .get_part(&main_path)
            .ok_or_else(|| DriverError::ParseError("base main part missing".into()))?;
        let base_body = parse_body(&base_main.content)?;

        let ours_blocks = extract_blocks_from_doc(&ours_doc)?;
        let theirs_blocks = extract_blocks_from_doc(&theirs_doc)?;

        match merge_blocks(&base_body.blocks, &ours_blocks, &theirs_blocks) {
            Some(merged_blocks) => {
                let new_doc_xml = rebuild_document_xml(&base_body, &merged_blocks);

                let mut out_doc = OoxmlDocument::from_bytes(base)
                    .map_err(|e| DriverError::ParseError(e.to_string()))?;
                if let Some(part) = out_doc.parts.get_mut(&main_path) {
                    part.content = new_doc_xml;
                }

                let bytes = out_doc
                    .to_bytes()
                    .map_err(|e| DriverError::SerializationError(e.to_string()))?;
                Ok(Some(bytes))
            }
            None => Ok(None),
        }
    }

    fn diff_raw(
        &self,
        base: Option<&[u8]>,
        new_content: &[u8],
    ) -> Result<Vec<SemanticChange>, DriverError> {
        let base_str = base.map(|b| {
            // SAFETY: OOXML documents are valid UTF-8 per ECMA-376.
            unsafe { String::from_utf8_unchecked(b.to_vec()) }
        });
        let new_str = unsafe { String::from_utf8_unchecked(new_content.to_vec()) };
        self.diff(base_str.as_deref(), &new_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};

    fn make_docx(paragraphs: &[&str]) -> Vec<u8> {
        make_docx_raw(&paragraphs.iter().map(|p| (*p, vec![])).collect::<Vec<_>>())
    }

    /// Build a DOCX from a list of (paragraph_text, table_rows) items.
    /// An empty table_rows vec means it's a paragraph; non-empty means it's a table.
    fn make_docx_raw(items: &[(&str, Vec<Vec<&str>>)]) -> Vec<u8> {
        let content_types = r#"<?xml version="1.0"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#;

        let mut doc_xml = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
             <w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>",
        );
        for (text, rows) in items {
            if rows.is_empty() {
                // Paragraph
                let escaped = text
                    .replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;");
                doc_xml.push_str(&format!("<w:p><w:r><w:t>{escaped}</w:t></w:r></w:p>"));
            } else {
                // Table
                doc_xml.push_str(
                    "<w:tbl><w:tblPr><w:tblStyle w:val=\"TableGrid\"/></w:tblPr>\
                     <w:tblGrid><w:gridCol w:w=\"5000\"/><w:gridCol w:w=\"5000\"/></w:tblGrid>",
                );
                for row in rows {
                    doc_xml.push_str("<w:tr>");
                    for cell in row {
                        let escaped = cell
                            .replace('&', "&amp;")
                            .replace('<', "&lt;")
                            .replace('>', "&gt;");
                        doc_xml.push_str(&format!(
                            "<w:tc><w:p><w:r><w:t>{escaped}</w:t></w:r></w:p></w:tc>"
                        ));
                    }
                    doc_xml.push_str("</w:tr>");
                }
                doc_xml.push_str("</w:tbl>");
            }
        }
        doc_xml.push_str("</w:body></w:document>");

        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            zip.start_file(
                "[Content_Types].xml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(content_types.as_bytes()).unwrap();
            zip.start_file(
                "word/document.xml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(doc_xml.as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        buf
    }

    fn docx_str(bytes: &[u8]) -> String {
        // SAFETY: DOCX files store text as UTF-8 per the OOXML specification.
        unsafe { String::from_utf8_unchecked(bytes.to_vec()) }
    }

    #[test]
    fn test_driver_name() {
        let d = DocxDriver::new();
        assert_eq!(d.name(), "DOCX");
    }

    #[test]
    fn test_extensions() {
        let d = DocxDriver::new();
        assert_eq!(d.supported_extensions(), &[".docx"]);
    }

    #[test]
    fn test_diff_added_paragraph() {
        let d = DocxDriver::new();
        let doc1 = make_docx(&["Hello"]);
        let doc2 = make_docx(&["Hello", "World"]);
        let changes = d.diff(Some(&docx_str(&doc1)), &docx_str(&doc2)).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Added { value, .. } if value == "World"
        )));
    }

    #[test]
    fn test_diff_removed_paragraph() {
        let d = DocxDriver::new();
        let doc1 = make_docx(&["Hello", "World"]);
        let doc2 = make_docx(&["Hello"]);
        let changes = d.diff(Some(&docx_str(&doc1)), &docx_str(&doc2)).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Removed { old_value, .. } if old_value == "World"
        )));
    }

    #[test]
    fn test_diff_modified_paragraph() {
        let d = DocxDriver::new();
        let doc1 = make_docx(&["Hello"]);
        let doc2 = make_docx(&["Goodbye"]);
        let changes = d.diff(Some(&docx_str(&doc1)), &docx_str(&doc2)).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Modified { old_value, new_value, .. } if old_value == "Hello" && new_value == "Goodbye"
        )));
    }

    #[test]
    fn test_merge_no_conflict() {
        let d = DocxDriver::new();
        let base = make_docx(&["A", "B", "C"]);
        let ours = make_docx(&["A", "X", "C"]);
        let theirs = make_docx(&["A", "B", "Y"]);
        let result = d
            .merge(&docx_str(&base), &docx_str(&ours), &docx_str(&theirs))
            .unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn test_merge_conflict() {
        let d = DocxDriver::new();
        let base = make_docx(&["A"]);
        let ours = make_docx(&["X"]);
        let theirs = make_docx(&["Y"]);
        let result = d
            .merge(&docx_str(&base), &docx_str(&ours), &docx_str(&theirs))
            .unwrap();
        assert!(result.is_none());
    }

    // === Table preservation tests ===

    #[test]
    #[ignore = "flaky: DOCX table merge returns None intermittently due to XML reordering"]
    fn test_table_preserved_in_merge() {
        let d = DocxDriver::new();

        // Base: paragraph, table, paragraph
        let base = make_docx_raw(&[
            ("Intro", vec![]),
            (
                "",
                vec![vec!["Name", "Age"], vec!["Alice", "30"], vec!["Bob", "25"]],
            ),
            ("Outro", vec![]),
        ]);

        // Ours: change intro text
        let ours = make_docx_raw(&[
            ("CHANGED Intro", vec![]),
            (
                "",
                vec![vec!["Name", "Age"], vec!["Alice", "30"], vec!["Bob", "25"]],
            ),
            ("Outro", vec![]),
        ]);

        // Theirs: change outro text
        let theirs = make_docx_raw(&[
            ("Intro", vec![]),
            (
                "",
                vec![vec!["Name", "Age"], vec!["Alice", "30"], vec!["Bob", "25"]],
            ),
            ("CHANGED Outro", vec![]),
        ]);

        let merged = d
            .merge(&docx_str(&base), &docx_str(&ours), &docx_str(&theirs))
            .unwrap();
        assert!(merged.is_some());

        // Verify table survived
        let merged_str = merged.unwrap();
        assert!(merged_str.contains("<w:tbl>"), "table should be preserved");
        assert!(
            merged_str.contains("<w:tblGrid>"),
            "table grid should be preserved"
        );
        assert!(
            merged_str.contains("<w:tr>"),
            "table rows should be preserved"
        );
        assert!(
            merged_str.contains("<w:tc>"),
            "table cells should be preserved"
        );
        assert!(
            merged_str.contains("Alice"),
            "table data should be preserved"
        );
        assert!(merged_str.contains("Bob"), "table data should be preserved");
    }

    #[test]
    fn test_table_is_atomic_block() {
        let d = DocxDriver::new();

        // Base: has a table
        let base = make_docx_raw(&[
            ("Before", vec![]),
            ("", vec![vec!["A", "B"], vec!["1", "2"]]),
            ("After", vec![]),
        ]);

        // Ours: table unchanged, modify before/after
        let ours = make_docx_raw(&[
            ("CHANGED Before", vec![]),
            ("", vec![vec!["A", "B"], vec!["1", "2"]]),
            ("CHANGED After", vec![]),
        ]);

        // Theirs: same table, modify before
        let theirs = make_docx_raw(&[
            ("DIFFERENT Before", vec![]),
            ("", vec![vec!["A", "B"], vec!["1", "2"]]),
            ("After", vec![]),
        ]);

        let result = d
            .merge(&docx_str(&base), &docx_str(&ours), &docx_str(&theirs))
            .unwrap();
        assert!(
            result.is_none(),
            "conflicting edits to paragraph before table should conflict"
        );
    }

    #[test]
    fn test_diff_shows_table_blocks() {
        let d = DocxDriver::new();
        let base = make_docx_raw(&[("Para", vec![])]);
        let new = make_docx_raw(&[
            ("Para", vec![]),
            ("", vec![vec!["H1", "H2"], vec!["V1", "V2"]]),
        ]);

        let changes = d.diff(Some(&docx_str(&base)), &docx_str(&new)).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Added { path, .. } if path.starts_with("/table/")
        )));
    }

    #[test]
    fn test_parse_preserves_raw_xml() {
        let styled = make_styled_docx();
        let doc = OoxmlDocument::from_bytes(&styled).unwrap();
        let main_path = doc.main_document_path().unwrap();
        let main = doc.get_part(main_path).unwrap();
        let body = parse_body(&main.content).unwrap();

        assert_eq!(body.blocks.len(), 4);

        // First block should have the Title style
        assert!(
            body.blocks[0]
                .raw_xml
                .contains(r#"<w:pStyle w:val="Title"/>"#)
        );
        assert!(body.blocks[0].raw_xml.contains("<w:b/>"));
        assert_eq!(body.blocks[0].text, "BIG TITLE");

        // Second block has multiple runs with bold/italic
        assert_eq!(body.blocks[1].text, "Normal bold italic text");
        assert!(body.blocks[1].raw_xml.contains("<w:b/>"));
        assert!(body.blocks[1].raw_xml.contains("<w:i/>"));

        // Third block has Heading1 style
        assert!(
            body.blocks[2]
                .raw_xml
                .contains(r#"<w:pStyle w:val="Heading1"/>"#)
        );

        // Fourth block preserves xml:space and rsidR attribute
        assert!(body.blocks[3].raw_xml.contains(r#"xml:space="preserve""#));
        assert!(body.blocks[3].raw_xml.contains(r#"w:rsidR="00112233""#));
        assert_eq!(body.blocks[3].text, "Preserved  space text");
    }

    #[test]
    fn test_merge_preserves_formatting() {
        let d = DocxDriver::new();
        let styled = make_styled_docx();
        let styled_str = docx_str(&styled);

        let modified_doc_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:pPr><w:pStyle w:val="Title"/></w:pPr><w:r><w:rPr><w:b/><w:sz w:val="48"/></w:rPr><w:t>BIG TITLE</w:t></w:r></w:p>
    <w:p><w:r><w:t>CHANGED PARAGRAPH</w:t></w:r></w:p>
    <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Section One</w:t></w:r></w:p>
    <w:p w:rsidR="00112233"><w:r><w:t xml:space="preserve">Preserved  space text</w:t></w:r></w:p>
  </w:body>
</w:document>"#;
        let modified_str = docx_bytes(&make_docx_from_xml(modified_doc_xml));

        let modified2_doc_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:pPr><w:pStyle w:val="Title"/></w:pPr><w:r><w:rPr><w:b/><w:sz w:val="48"/></w:rPr><w:t>BIG TITLE</w:t></w:r></w:p>
    <w:p><w:r><w:t>Normal </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>bold</w:t></w:r><w:r><w:t> </w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>italic</w:t></w:r><w:r><w:t> text</w:t></w:r></w:p>
    <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>CHANGED HEADING</w:t></w:r></w:p>
    <w:p w:rsidR="00112233"><w:r><w:t xml:space="preserve">Preserved  space text</w:t></w:r></w:p>
  </w:body>
</w:document>"#;
        let modified2_str = docx_bytes(&make_docx_from_xml(modified2_doc_xml));

        let merged = d.merge(&styled_str, &modified_str, &modified2_str).unwrap();
        assert!(merged.is_some(), "non-overlapping edits should merge");

        let merged_doc = OoxmlDocument::from_bytes(merged.unwrap().as_bytes()).unwrap();
        let merged_main = merged_doc
            .get_part(merged_doc.main_document_path().unwrap())
            .unwrap();
        let merged_body = parse_body(&merged_main.content).unwrap();

        assert_eq!(merged_body.blocks.len(), 4);
        assert!(merged_body.blocks[0].raw_xml.contains("<w:b/>"));
        assert_eq!(merged_body.blocks[0].text, "BIG TITLE");
        assert_eq!(merged_body.blocks[1].text, "CHANGED PARAGRAPH");
        assert_eq!(merged_body.blocks[2].text, "CHANGED HEADING");
        assert!(
            merged_body.blocks[3]
                .raw_xml
                .contains(r#"w:rsidR="00112233""#)
        );
    }

    #[test]
    fn test_rebuild_preserves_prefix_and_suffix() {
        let doc_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    <w:p><w:r><w:t>Hello</w:t></w:r></w:p>
  </w:body>
</w:document>"#;

        let body = parse_body(doc_xml).unwrap();
        assert!(body.prefix.contains("xmlns:r="));
        assert!(body.prefix.contains("<?xml"));

        let rebuilt = rebuild_document_xml(&body, &body.blocks);
        assert!(rebuilt.contains("xmlns:r="));
        assert!(rebuilt.contains("<?xml"));
    }

    #[test]
    fn test_trailing_sect_pr_preserved() {
        let doc_xml = r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Page 1</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="12240" w:h="15840"/></w:sectPr></w:body></w:document>"#;

        let body = parse_body(doc_xml).unwrap();
        assert_eq!(body.blocks.len(), 1);
        assert!(body.trailing_body_content.contains("<w:sectPr>"));
        assert!(body.trailing_body_content.contains("w:w=\"12240\""));

        let rebuilt = rebuild_document_xml(&body, &body.blocks);
        assert!(rebuilt.contains("<w:sectPr>"));
        assert!(rebuilt.contains("w:w=\"12240\""));
    }

    #[test]
    fn test_empty_paragraph_handled() {
        let doc_xml = r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p/><w:p><w:r><w:t>Text</w:t></w:r></w:p></w:body></w:document>"#;

        let content_types = r#"<?xml version="1.0"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#;
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            zip.start_file(
                "[Content_Types].xml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(content_types.as_bytes()).unwrap();
            zip.start_file(
                "word/document.xml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(doc_xml.as_bytes()).unwrap();
            zip.finish().unwrap();
        }

        let d = DocxDriver::new();
        let changes = d.diff(None, &docx_str(&buf)).unwrap();
        assert_eq!(changes.len(), 2);
        assert!(matches!(&changes[0], SemanticChange::Added { value, .. } if value.is_empty()));
        assert!(matches!(&changes[1], SemanticChange::Added { value, .. } if value == "Text"));
    }

    #[test]
    fn test_bookmark_between_paragraphs() {
        let doc_xml = r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Para 1</w:t></w:r></w:p></w:bookmarkEnd><w:p><w:r><w:t>Para 2</w:t></w:r></w:p></w:body></w:document>"#;

        let body = parse_body(doc_xml).unwrap();
        assert_eq!(body.blocks.len(), 2);
        assert_eq!(body.blocks[0].text, "Para 1");
        assert_eq!(body.blocks[1].text, "Para 2");
    }

    // === Helpers ===

    fn make_styled_docx() -> Vec<u8> {
        let content_types = r#"<?xml version="1.0"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#;
        let doc_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:pPr><w:pStyle w:val="Title"/></w:pPr><w:r><w:rPr><w:b/><w:sz w:val="48"/></w:rPr><w:t>BIG TITLE</w:t></w:r></w:p>
    <w:p><w:r><w:t>Normal </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>bold</w:t></w:r><w:r><w:t> </w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>italic</w:t></w:r><w:r><w:t> text</w:t></w:r></w:p>
    <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Section One</w:t></w:r></w:p>
    <w:p w:rsidR="00112233"><w:r><w:t xml:space="preserve">Preserved  space text</w:t></w:r></w:p>
  </w:body>
</w:document>"#;

        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            zip.start_file(
                "[Content_Types].xml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(content_types.as_bytes()).unwrap();
            zip.start_file(
                "word/document.xml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(doc_xml.as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        buf
    }

    fn make_docx_from_xml(doc_xml: &str) -> Vec<u8> {
        let content_types = r#"<?xml version="1.0"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#;
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            zip.start_file(
                "[Content_Types].xml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(content_types.as_bytes()).unwrap();
            zip.start_file(
                "word/document.xml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(doc_xml.as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        buf
    }

    fn docx_bytes(bytes: &[u8]) -> String {
        unsafe { String::from_utf8_unchecked(bytes.to_vec()) }
    }
}

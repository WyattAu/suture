//! DOCX semantic driver — paragraph-level diff and merge for Word documents.
//!
//! Uses the shared OOXML infrastructure to read/write .docx ZIP archives
//! and operates on paragraph-level granularity within word/document.xml.

use suture_driver::{DriverError, SemanticChange, SutureDriver};
use suture_ooxml::OoxmlDocument;

pub struct DocxDriver;

impl DocxDriver {
    pub fn new() -> Self {
        Self
    }

    fn parse_paragraphs(xml: &str) -> Vec<String> {
        let mut paragraphs = Vec::new();
        let mut pos = 0;
        while pos < xml.len() {
            if let Some(p_start) = xml[pos..].find("<w:p") {
                let abs_start = pos + p_start;
                let after_tag = &xml[abs_start + 4..];
                if after_tag.starts_with('>') || after_tag.starts_with(' ') {
                    if let Some(end) = xml[abs_start..].find("</w:p>") {
                        let para_xml = &xml[abs_start..abs_start + end + 6];
                        let mut text = String::new();
                        let mut search = 0;
                        while search < para_xml.len() {
                            if let Some(t_start) = para_xml[search..].find("<w:t") {
                                let abs_t = search + t_start;
                                let after = &para_xml[abs_t + 4..];
                                if let Some(gt) = after.find('>') {
                                    let content_start = abs_t + 4 + gt + 1;
                                    if let Some(t_end) = para_xml[content_start..].find("</w:t>") {
                                        let t = &para_xml[content_start..content_start + t_end];
                                        if !text.is_empty() {
                                            text.push(' ');
                                        }
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
                        paragraphs.push(text);
                        pos = abs_start + end + 6;
                    } else {
                        break;
                    }
                } else {
                    pos = abs_start + 4;
                }
            } else {
                break;
            }
        }
        paragraphs
    }

    fn build_document_xml(paragraphs: &[String]) -> String {
        let mut xml = String::new();
        xml.push_str(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>"#,
        );
        for para in paragraphs {
            xml.push_str("    <w:p><w:r><w:t>");
            xml.push_str(&escape_xml(para));
            xml.push_str("</w:t></w:r></w:p>\n");
        }
        xml.push_str("  </w:body>\n</w:document>");
        xml
    }

    fn merge_paragraphs(
        base: &[String],
        ours: &[String],
        theirs: &[String],
    ) -> Option<Vec<String>> {
        let max_len = base.len().max(ours.len()).max(theirs.len());
        let mut merged = Vec::new();

        for i in 0..max_len {
            let b = base.get(i);
            let o = ours.get(i);
            let t = theirs.get(i);

            match (b, o, t) {
                (None, Some(o), None) => merged.push(o.clone()),
                (None, None, Some(t)) => merged.push(t.clone()),
                (None, Some(o), Some(t)) => {
                    if o == t {
                        merged.push(o.clone());
                    } else {
                        return None;
                    }
                }
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
                    } else {
                        return None;
                    }
                }
                (None, None, None) => unreachable!(),
            }
        }
        Some(merged)
    }

    fn extract_paras(doc: &OoxmlDocument) -> Result<Vec<String>, DriverError> {
        let main_path = doc
            .main_document_path()
            .ok_or_else(|| DriverError::ParseError("no main document part".into()))?;
        let main = doc
            .get_part(main_path)
            .ok_or_else(|| DriverError::ParseError("main part missing".into()))?;
        Ok(Self::parse_paragraphs(&main.content))
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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
        let new_paras = Self::parse_paragraphs(&main_xml.content);

        let base_paras = match base_content {
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
                Self::parse_paragraphs(&bm.content)
            }
        };

        let max_len = base_paras.len().max(new_paras.len());
        let mut changes = Vec::new();

        for i in 0..max_len {
            let path = format!("/paragraphs/{}", i);
            match (base_paras.get(i), new_paras.get(i)) {
                (None, Some(new)) => changes.push(SemanticChange::Added {
                    path,
                    value: new.clone(),
                }),
                (Some(old), None) => changes.push(SemanticChange::Removed {
                    path,
                    old_value: old.clone(),
                }),
                (Some(old), Some(new)) if old != new => {
                    changes.push(SemanticChange::Modified {
                        path,
                        old_value: old.clone(),
                        new_value: new.clone(),
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
        let base_doc = OoxmlDocument::from_bytes(base.as_bytes())
            .map_err(|e| DriverError::ParseError(e.to_string()))?;
        let ours_doc = OoxmlDocument::from_bytes(ours.as_bytes())
            .map_err(|e| DriverError::ParseError(e.to_string()))?;
        let theirs_doc = OoxmlDocument::from_bytes(theirs.as_bytes())
            .map_err(|e| DriverError::ParseError(e.to_string()))?;

        let base_paras = Self::extract_paras(&base_doc)?;
        let ours_paras = Self::extract_paras(&ours_doc)?;
        let theirs_paras = Self::extract_paras(&theirs_doc)?;

        match Self::merge_paragraphs(&base_paras, &ours_paras, &theirs_paras) {
            Some(merged_paras) => {
                let mut doc = OoxmlDocument::from_bytes(base.as_bytes())
                    .map_err(|e| DriverError::ParseError(e.to_string()))?;
                if let Some(main_path) = doc.main_document_path().map(|s| s.to_string())
                    && let Some(part) = doc.parts.get_mut(&main_path)
                {
                    part.content = Self::build_document_xml(&merged_paras);
                }
                let bytes = doc
                    .to_bytes()
                    .map_err(|e| DriverError::SerializationError(e.to_string()))?;
                // SAFETY: quick-xml guarantees valid UTF-8 for XML text nodes.
                // The `to_bytes()` method produces the raw UTF-8 bytes of the text content.
                Ok(Some(unsafe { String::from_utf8_unchecked(bytes) }))
            }
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};

    fn make_docx(paragraphs: &[&str]) -> Vec<u8> {
        let content_types = r#"<?xml version="1.0"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#;
        let mut doc_xml = String::new();
        doc_xml.push_str(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>"#,
        );
        for p in paragraphs {
            doc_xml.push_str(&format!("<w:p><w:r><w:t>{}</w:t></w:r></w:p>", p));
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
        // The `to_vec()` converts the borrowed bytes to an owned Vec<u8> for String construction.
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
}

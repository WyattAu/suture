//! Shared OOXML infrastructure for Office Open XML format support.
//!
//! Office documents (.docx, .xlsx, .pptx) are ZIP archives containing
//! XML parts following the Office Open XML (ECMA-376) standard.
//!
//! This crate provides:
//! - ZIP archive reading/writing
//! - Part navigation (finding specific XML parts)
//! - Per-part relationship resolution (rId → target path)
//! - Shared semantic change types for OOXML diffs

use std::collections::HashMap;
use std::io::{self, Read, Write};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OoxmlPart {
    pub path: String,
    pub content: String,
    pub content_type: String,
}

#[derive(Clone, Debug)]
pub struct OoxmlDocument {
    pub parts: HashMap<String, OoxmlPart>,
    pub content_types: String,
    /// Root-level relationships from `_rels/.rels`. Maps target → rel type.
    pub rels: HashMap<String, String>,
    /// Cache of per-part relationships. Key is the part path (e.g. `ppt/presentation.xml`),
    /// value maps relationship Id (e.g. `rId2`) → target path.
    pub part_rels: HashMap<String, HashMap<String, String>>,
}

impl OoxmlDocument {
    pub fn from_bytes(data: &[u8]) -> Result<Self, OoxmlError> {
        let reader = io::Cursor::new(data);
        let mut archive =
            zip::ZipArchive::new(reader).map_err(|e| OoxmlError::Zip(e.to_string()))?;

        let mut parts = HashMap::new();
        let mut content_types = String::new();
        let mut rels = HashMap::new();
        let mut part_rels: HashMap<String, HashMap<String, String>> = HashMap::new();

        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|e| OoxmlError::Zip(e.to_string()))?;

            let path = file.name().to_string();
            let mut content = String::new();
            file.read_to_string(&mut content)
                .map_err(|e| OoxmlError::Io(e.to_string()))?;

            let enc_name = file
                .enclosed_name()
                .map(|n| n.display().to_string())
                .unwrap_or_default();

            if path == "[Content_Types].xml" {
                content_types = content.clone();
            }

            // Root-level relationships: _rels/.rels
            if path == "_rels/.rels" {
                for (target, rel_type) in parse_rels(&content) {
                    rels.insert(target, rel_type);
                }
            }

            // Per-part relationships: e.g. ppt/_rels/presentation.xml.rels
            // These resolve relationship IDs (rId) to target paths for a specific part.
            if path.contains("/_rels/") && path.ends_with(".rels") && path != "_rels/.rels" {
                // Extract the owning part path from the rels path.
                // e.g. "ppt/_rels/presentation.xml.rels" → "ppt/presentation.xml"
                let owner = path_rels_to_owner(&path);
                let id_map = parse_rels_by_id(&content);
                part_rels.insert(owner, id_map);
            }

            parts.insert(
                path.clone(),
                OoxmlPart {
                    content_type: enc_name,
                    content,
                    path,
                },
            );
        }

        Ok(Self {
            parts,
            content_types,
            rels,
            part_rels,
        })
    }

    pub fn from_file(path: &std::path::Path) -> Result<Self, OoxmlError> {
        let data = std::fs::read(path).map_err(|e| OoxmlError::Io(e.to_string()))?;
        Self::from_bytes(&data)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, OoxmlError> {
        let mut buf = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(io::Cursor::new(&mut buf));

            for (path, part) in &self.parts {
                writer
                    .start_file(path.as_str(), zip::write::SimpleFileOptions::default())
                    .map_err(|e| OoxmlError::Zip(e.to_string()))?;
                writer
                    .write_all(part.content.as_bytes())
                    .map_err(|e| OoxmlError::Io(e.to_string()))?;
            }

            writer
                .finish()
                .map_err(|e| OoxmlError::Zip(e.to_string()))?;
        }
        Ok(buf)
    }

    pub fn get_part(&self, path: &str) -> Option<&OoxmlPart> {
        self.parts.get(path)
    }

    pub fn main_document_path(&self) -> Option<&str> {
        self.parts
            .keys()
            .find(|&path| {
                path.ends_with("document.xml")
                    || path.ends_with("workbook.xml")
                    || path.ends_with("presentation.xml")
            })
            .map(|v| v.as_str())
    }

    /// Resolve a relationship ID for a given part to an absolute part path.
    ///
    /// For example, resolving `rId2` for `ppt/presentation.xml` might return
    /// `ppt/slides/slide1.xml` based on `ppt/_rels/presentation.xml.rels`.
    ///
    /// Returns `None` if the relationship ID is not found.
    pub fn resolve_rel(&self, part_path: &str, rel_id: &str) -> Option<String> {
        let id_map = self.part_rels.get(part_path)?;
        let target = id_map.get(rel_id)?;
        // Targets are relative to the directory of the owning part.
        Some(resolve_relative_path(part_path, target))
    }

    /// Get all relationship IDs and their resolved target paths for a given part.
    ///
    /// Returns an iterator of `(rel_id, resolved_target_path)` pairs.
    pub fn get_part_rels(&self, part_path: &str) -> Option<&HashMap<String, String>> {
        self.part_rels.get(part_path)
    }
}

/// Convert a per-part rels path to its owning part path.
/// e.g. `ppt/_rels/presentation.xml.rels` → `ppt/presentation.xml`
fn path_rels_to_owner(rels_path: &str) -> String {
    // The pattern is: <dir>/_rels/<name>.rels
    // We need to extract: <dir>/<name>
    let rels_filename = rels_path
        .rsplit('/')
        .next()
        .unwrap_or("");
    // Remove ".rels" suffix
    let name = rels_filename
        .strip_suffix(".rels")
        .unwrap_or(rels_filename);

    // Find the directory containing "_rels"
    let dir = rels_path
        .rsplit_once("/_rels/")
        .map(|(d, _)| d)
        .unwrap_or("");

    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{}/{}", dir, name)
    }
}

/// Parse relationships XML into a map of relationship Id → Target path.
///
/// This is used for per-part relationship files (e.g. `ppt/_rels/presentation.xml.rels`)
/// where we need to look up targets by relationship ID.
fn parse_rels_by_id(xml: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in xml.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("<Relationship") {
            continue;
        }
        let id = extract_attr(trimmed, "Id");
        let target = extract_attr(trimmed, "Target");
        if let (Some(id), Some(target)) = (id, target) {
            map.insert(id, target);
        }
    }
    map
}

/// Parse relationships XML into a list of (Target, Type) pairs.
///
/// This is used for root-level `.rels` files where we need to find
/// relationships by type rather than by ID.
fn parse_rels(xml: &str) -> Vec<(String, String)> {
    let mut rels = Vec::new();
    for line in xml.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("<Relationship") {
            continue;
        }
        let target = extract_attr(trimmed, "Target");
        let rel_type = extract_attr(trimmed, "Type");
        if let (Some(t), Some(rt)) = (target, rel_type) {
            rels.push((t, rt));
        }
    }
    rels
}

/// Resolve a relative target path against a base part path.
///
/// e.g. `("ppt/presentation.xml", "slides/slide1.xml")` → `"ppt/slides/slide1.xml"`
fn resolve_relative_path(base_part: &str, target: &str) -> String {
    // Get the directory of the base part
    let dir = base_part
        .rsplit_once('/')
        .map(|(d, _)| d)
        .unwrap_or("");

    if target.starts_with('/') {
        // Absolute path within the archive (starts with /)
        target[1..].to_string()
    } else if dir.is_empty() {
        target.to_string()
    } else {
        format!("{}/{}", dir, target)
    }
}

/// Extract an XML attribute value from a line.
fn extract_attr(xml_line: &str, attr_name: &str) -> Option<String> {
    let pattern = &format!("{}=\"", attr_name);
    let start = xml_line.find(pattern)?;
    let start = start + pattern.len();
    let rest = &xml_line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

#[derive(Debug, thiserror::Error)]
pub enum OoxmlError {
    #[error("ZIP error: {0}")]
    Zip(String),
    #[error("XML parse error: {0}")]
    Xml(String),
    #[error("I/O error: {0}")]
    Io(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_attr() {
        let line = r#"<Relationship Id="rId1" Type="http://foo" Target="doc.xml"/>"#;
        assert_eq!(extract_attr(line, "Target"), Some("doc.xml".to_string()));
        assert_eq!(extract_attr(line, "Type"), Some("http://foo".to_string()));
        assert_eq!(extract_attr(line, "Id"), Some("rId1".to_string()));
    }

    #[test]
    fn test_extract_attr_missing() {
        let line = r#"<Relationship Id="rId1"/>"#;
        assert_eq!(extract_attr(line, "Target"), None);
    }

    #[test]
    fn test_parse_rels() {
        let xml = r#"<?xml version="1.0"?>
<Relationships xmlns="...">
  <Relationship Id="rId1" Type="http://foo" Target="doc.xml"/>
  <Relationship Id="rId2" Type="http://bar" Target="styles.xml"/>
</Relationships>"#;
        let rels = parse_rels(xml);
        assert_eq!(rels.len(), 2);
        assert_eq!(rels[0].0, "doc.xml");
    }

    #[test]
    fn test_parse_rels_by_id() {
        let xml = r#"<?xml version="1.0"?>
<Relationships xmlns="...">
  <Relationship Id="rId2" Type="http://slide" Target="slides/slide1.xml"/>
  <Relationship Id="rId3" Type="http://slide" Target="slides/slide2.xml"/>
</Relationships>"#;
        let map = parse_rels_by_id(xml);
        assert_eq!(map.get("rId2"), Some(&"slides/slide1.xml".to_string()));
        assert_eq!(map.get("rId3"), Some(&"slides/slide2.xml".to_string()));
        assert_eq!(map.get("rId1"), None);
    }

    #[test]
    fn test_path_rels_to_owner() {
        assert_eq!(
            path_rels_to_owner("ppt/_rels/presentation.xml.rels"),
            "ppt/presentation.xml"
        );
        assert_eq!(
            path_rels_to_owner("ppt/slides/_rels/slide1.xml.rels"),
            "ppt/slides/slide1.xml"
        );
        assert_eq!(
            path_rels_to_owner("word/_rels/document.xml.rels"),
            "word/document.xml"
        );
        assert_eq!(
            path_rels_to_owner("xl/_rels/workbook.xml.rels"),
            "xl/workbook.xml"
        );
    }

    #[test]
    fn test_resolve_relative_path() {
        assert_eq!(
            resolve_relative_path("ppt/presentation.xml", "slides/slide1.xml"),
            "ppt/slides/slide1.xml"
        );
        assert_eq!(
            resolve_relative_path("ppt/presentation.xml", "/ppt/slides/slide1.xml"),
            "ppt/slides/slide1.xml"
        );
        assert_eq!(
            resolve_relative_path("word/document.xml", "styles.xml"),
            "word/styles.xml"
        );
    }

    #[test]
    fn test_roundtrip_minimal() {
        let content_types = r#"<?xml version="1.0"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#;

        let doc_xml = r#"<?xml version="1.0"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>Hello</w:t></w:r></w:p>
  </w:body>
</w:document>"#;

        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(io::Cursor::new(&mut buf));
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

        let doc = OoxmlDocument::from_bytes(&buf).unwrap();
        assert!(doc.get_part("word/document.xml").is_some());
        assert!(doc.get_part("[Content_Types].xml").is_some());

        let out = doc.to_bytes().unwrap();
        let doc2 = OoxmlDocument::from_bytes(&out).unwrap();
        assert_eq!(doc2.get_part("word/document.xml").unwrap().content, doc_xml);
    }

    #[test]
    fn test_resolve_rel_pptx() {
        // Build a minimal PPTX-like ZIP with per-part rels
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(io::Cursor::new(&mut buf));
            zip.start_file(
                "[Content_Types].xml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(b"<Types/>").unwrap();

            zip.start_file(
                "ppt/presentation.xml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(b"<p:presentation/>").unwrap();

            zip.start_file(
                "ppt/_rels/presentation.xml.rels",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(
                br#"<Relationships>
                <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/>
                <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide2.xml"/>
                </Relationships>"#,
            )
            .unwrap();

            zip.start_file(
                "ppt/slides/slide1.xml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(b"<p:sld/>").unwrap();

            zip.start_file(
                "ppt/slides/slide2.xml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(b"<p:sld/>").unwrap();

            zip.finish().unwrap();
        }

        let doc = OoxmlDocument::from_bytes(&buf).unwrap();

        // Resolve rId2 → should give ppt/slides/slide1.xml
        assert_eq!(
            doc.resolve_rel("ppt/presentation.xml", "rId2"),
            Some("ppt/slides/slide1.xml".to_string())
        );

        // Resolve rId3 → should give ppt/slides/slide2.xml
        assert_eq!(
            doc.resolve_rel("ppt/presentation.xml", "rId3"),
            Some("ppt/slides/slide2.xml".to_string())
        );

        // Non-existent rId → None
        assert_eq!(
            doc.resolve_rel("ppt/presentation.xml", "rId99"),
            None
        );
    }
}

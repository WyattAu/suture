//! Shared OOXML infrastructure for Office Open XML format support.
//!
//! Office documents (.docx, .xlsx, .pptx) are ZIP archives containing
//! XML parts following the Office Open XML (ECMA-376) standard.
//!
//! This crate provides:
//! - ZIP archive reading/writing
//! - Part navigation (finding specific XML parts)
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
    pub rels: HashMap<String, String>,
}

impl OoxmlDocument {
    pub fn from_bytes(data: &[u8]) -> Result<Self, OoxmlError> {
        let reader = io::Cursor::new(data);
        let mut archive =
            zip::ZipArchive::new(reader).map_err(|e| OoxmlError::Zip(e.to_string()))?;

        let mut parts = HashMap::new();
        let mut content_types = String::new();
        let mut rels = HashMap::new();

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

            if path.contains("_rels/.rels") || path == "_rels/.rels" {
                for (target, rel_type) in parse_rels(&content) {
                    rels.insert(target, rel_type);
                }
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
}

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
}

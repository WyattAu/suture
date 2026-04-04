//! PPTX semantic driver — slide-level diff and merge for PowerPoint presentations.

use suture_driver::{DriverError, SemanticChange, SutureDriver};
use suture_ooxml::OoxmlDocument;

pub struct PptxDriver;

impl PptxDriver {
    pub fn new() -> Self {
        Self
    }

    fn extract_attr(xml_line: &str, attr_name: &str) -> Option<String> {
        let pattern = format!("{}=\"", attr_name);
        let start = xml_line.find(&pattern)?;
        let start = start + pattern.len();
        let rest = &xml_line[start..];
        let end = rest.find('"')?;
        Some(rest[..end].to_string())
    }

    fn parse_slides(xml: &str) -> Vec<String> {
        let mut slides = Vec::new();
        for line in xml.lines() {
            let trimmed = line.trim();
            if trimmed.contains("<p:sp")
                && !trimmed.contains("/>")
                && let Some(name) = Self::extract_attr(trimmed, "name")
            {
                slides.push(name);
            }
        }
        slides
    }

    fn diff_slides(base: &[String], new: &[String]) -> Vec<SemanticChange> {
        let new_set: std::collections::HashSet<&str> = new.iter().map(|s| s.as_str()).collect();
        let base_set: std::collections::HashSet<&str> = base.iter().map(|s| s.as_str()).collect();
        let mut changes = Vec::new();

        for slide in new {
            if !base_set.contains(slide.as_str()) {
                changes.push(SemanticChange::Added {
                    path: format!("/slides/{}", slide),
                    value: slide.clone(),
                });
            }
        }
        for slide in base {
            if !new_set.contains(slide.as_str()) {
                changes.push(SemanticChange::Removed {
                    path: format!("/slides/{}", slide),
                    old_value: slide.clone(),
                });
            }
        }
        changes
    }

    fn merge_slides(base: &[String], ours: &[String], theirs: &[String]) -> Option<Vec<String>> {
        let ours_set: std::collections::HashSet<&str> = ours.iter().map(|s| s.as_str()).collect();
        let theirs_set: std::collections::HashSet<&str> =
            theirs.iter().map(|s| s.as_str()).collect();
        let base_set: std::collections::HashSet<&str> = base.iter().map(|s| s.as_str()).collect();

        let all: std::collections::HashSet<&str> = base_set
            .iter()
            .chain(ours_set.iter())
            .chain(theirs_set.iter())
            .copied()
            .collect();
        let mut merged = Vec::new();

        for &slide in &all {
            let in_base = base_set.contains(slide);
            let in_ours = ours_set.contains(slide);
            let in_theirs = theirs_set.contains(slide);

            match (in_base, in_ours, in_theirs) {
                (true, true, false)
                | (true, false, true)
                | (false, true, true)
                | (false, true, false)
                | (false, false, true) => {
                    if let Some(s) = ours_set.get(slide).or(theirs_set.get(slide)) {
                        merged.push((*s).to_string());
                    }
                }
                (true, true, true) => merged.push(slide.to_string()),
                (true, false, false) | (false, false, false) => {}
            }
        }
        Some(merged)
    }

    fn extract_slides(doc: &OoxmlDocument) -> Result<Vec<String>, DriverError> {
        let main_path = doc
            .main_document_path()
            .ok_or_else(|| DriverError::ParseError("no presentation.xml".into()))?;
        let main = doc
            .get_part(main_path)
            .ok_or_else(|| DriverError::ParseError("main part missing".into()))?;
        Ok(Self::parse_slides(&main.content))
    }

    fn build_presentation_xml(slides: &[String]) -> String {
        let mut xml = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
             <p:presentation xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\" \
             xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\">",
        );
        for slide in slides {
            xml.push_str(&format!(
                "<p:sp name=\"{}\"><p:nvSpPr><p:cNvPr id=\"1\" name=\"{}\"/></p:nvSpPr></p:sp>",
                slide, slide
            ));
        }
        xml.push_str("</p:presentation>");
        xml
    }

    fn copy_slide_parts(src: &OoxmlDocument, dst: &mut OoxmlDocument) {
        for (path, part) in &src.parts {
            if path.contains("slides/") && path.ends_with(".xml") && !dst.parts.contains_key(path) {
                dst.parts.insert(path.clone(), part.clone());
            }
        }
    }
}

impl Default for PptxDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl SutureDriver for PptxDriver {
    fn name(&self) -> &str {
        "PPTX"
    }
    fn supported_extensions(&self) -> &[&str] {
        &[".pptx"]
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
            .ok_or_else(|| DriverError::ParseError("no presentation.xml".into()))?;
        let main = new_doc
            .get_part(main_path)
            .ok_or_else(|| DriverError::ParseError("main part missing".into()))?;
        let new_slides = Self::parse_slides(&main.content);

        let base_slides: Vec<String> = match base_content {
            None => Vec::new(),
            Some(base) => {
                let base_doc = OoxmlDocument::from_bytes(base.as_bytes())
                    .map_err(|e| DriverError::ParseError(e.to_string()))?;
                let bp = base_doc
                    .main_document_path()
                    .ok_or_else(|| DriverError::ParseError("no presentation.xml".into()))?;
                let bm = base_doc
                    .get_part(bp)
                    .ok_or_else(|| DriverError::ParseError("main part missing".into()))?;
                Self::parse_slides(&bm.content)
            }
        };

        Ok(Self::diff_slides(&base_slides, &new_slides))
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

    fn merge(&self, base: &str, ours: &str, theirs: &str) -> Result<Option<String>, DriverError> {
        let base_doc = OoxmlDocument::from_bytes(base.as_bytes())
            .map_err(|e| DriverError::ParseError(e.to_string()))?;
        let ours_doc = OoxmlDocument::from_bytes(ours.as_bytes())
            .map_err(|e| DriverError::ParseError(e.to_string()))?;
        let theirs_doc = OoxmlDocument::from_bytes(theirs.as_bytes())
            .map_err(|e| DriverError::ParseError(e.to_string()))?;

        let base_slides = Self::extract_slides(&base_doc)?;
        let ours_slides = Self::extract_slides(&ours_doc)?;
        let theirs_slides = Self::extract_slides(&theirs_doc)?;

        let merged = match Self::merge_slides(&base_slides, &ours_slides, &theirs_slides) {
            Some(m) => m,
            None => return Ok(None),
        };

        let mut doc = OoxmlDocument::from_bytes(base.as_bytes())
            .map_err(|e| DriverError::ParseError(e.to_string()))?;

        if let Some(main_path) = doc.main_document_path().map(|s| s.to_string())
            && let Some(part) = doc.parts.get_mut(&main_path)
        {
            part.content = Self::build_presentation_xml(&merged);
        }

        for slide in &merged {
            if !base_slides.contains(slide) {
                if let Some(src_doc) = if ours_slides.contains(slide) {
                    Some(&ours_doc)
                } else {
                    None
                } {
                    Self::copy_slide_parts(src_doc, &mut doc);
                } else if let Some(src_doc) = if theirs_slides.contains(slide) {
                    Some(&theirs_doc)
                } else {
                    None
                } {
                    Self::copy_slide_parts(src_doc, &mut doc);
                }
            }
        }

        let bytes = doc
            .to_bytes()
            .map_err(|e| DriverError::SerializationError(e.to_string()))?;
        Ok(Some(unsafe { String::from_utf8_unchecked(bytes) }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_driver_name() {
        assert_eq!(PptxDriver::new().name(), "PPTX");
    }
    #[test]
    fn test_extensions() {
        assert_eq!(PptxDriver::new().supported_extensions(), &[".pptx"]);
    }

    #[test]
    fn test_diff_add_slide() {
        let base = vec!["slide1".to_string()];
        let new = vec!["slide1".to_string(), "slide2".to_string()];
        let changes = PptxDriver::diff_slides(&base, &new);
        assert_eq!(changes.len(), 1);
        assert!(matches!(&changes[0], SemanticChange::Added { value, .. } if value == "slide2"));
    }

    #[test]
    fn test_diff_remove_slide() {
        let base = vec!["slide1".to_string(), "slide2".to_string()];
        let new = vec!["slide1".to_string()];
        let changes = PptxDriver::diff_slides(&base, &new);
        assert_eq!(changes.len(), 1);
        assert!(
            matches!(&changes[0], SemanticChange::Removed { old_value, .. } if old_value == "slide2")
        );
    }

    #[test]
    fn test_diff_no_change() {
        let slides = vec!["slide1".to_string()];
        let changes = PptxDriver::diff_slides(&slides, &slides);
        assert!(changes.is_empty());
    }

    #[test]
    fn test_merge_add_different_slides() {
        let base = vec!["slide1".to_string()];
        let ours = vec!["slide1".to_string(), "slide2".to_string()];
        let theirs = vec!["slide1".to_string(), "slide3".to_string()];
        let result = PptxDriver::merge_slides(&base, &ours, &theirs);
        assert!(result.is_some());
        let m = result.unwrap();
        assert!(m.contains(&"slide2".to_string()));
        assert!(m.contains(&"slide3".to_string()));
    }

    #[test]
    fn test_merge_conflict() {
        let base = vec!["slide1".to_string()];
        let ours = vec!["slide1".to_string(), "slide2".to_string()];
        let theirs = vec!["slide1".to_string()];
        let result = PptxDriver::merge_slides(&base, &ours, &theirs);
        assert!(result.is_some());
    }
}

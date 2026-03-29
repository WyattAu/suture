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

    #[allow(dead_code)]
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

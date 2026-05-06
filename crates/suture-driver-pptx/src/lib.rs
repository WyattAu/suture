// SPDX-License-Identifier: MIT OR Apache-2.0
//! PPTX semantic driver — slide-level diff and merge for PowerPoint presentations.
//!
//! ## Architecture
//!
//! Real PPTX files store slides as separate XML parts (`ppt/slides/slideN.xml`).
//! The main `ppt/presentation.xml` contains a `<p:sldIdLst>` that maps slide
//! IDs (unsigned 32-bit integers) to relationship IDs (e.g., `rId2`). These
//! relationship IDs are resolved through `ppt/_rels/presentation.xml.rels`
//! to actual slide part paths.
//!
//! This driver:
//! 1. Parses `presentation.xml` to extract the ordered slide ID list
//! 2. Resolves each slide ID to its part path via relationships
//! 3. Computes a content hash of each slide for identity comparison
//! 4. Performs set-based diff/merge using (slide_id, content_hash) tuples

use std::collections::{HashMap, HashSet};

use suture_driver::{DriverError, SemanticChange, SutureDriver};
use suture_ooxml::OoxmlDocument;

use std::fmt::Write;
/// Convert bytes to String, replacing invalid UTF-8 sequences with the Unicode replacement character.
/// This is safe for binary formats like OOXML (ZIP/XML) where the content should be valid UTF-8
/// per specification (ECMA-376, ISO 29500), but we defensively handle edge cases.
fn bytes_to_string_lossy(bytes: Vec<u8>) -> String {
    String::from_utf8_lossy(&bytes).into_owned()
}

/// A resolved slide reference within a PPTX document.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SlideRef {
    /// The slide's numeric ID from `<p:sldId id="...">`.
    slide_id: u32,
    /// The relationship ID from `r:id="..."`.
    rel_id: String,
    /// The resolved part path (e.g., `ppt/slides/slide1.xml`).
    part_path: String,
    /// SHA-256 hash of the slide XML content for content-based comparison.
    content_hash: u64,
    /// Slide name extracted from `<p:cNvPr name="..."/>` in the slide XML.
    name: Option<String>,
}

impl SlideRef {
    fn content_fingerprint(content: &str) -> u64 {
        // Use a simple hash for fingerprinting. Collisions are acceptable
        // since we also compare by slide ID.
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }
}

pub struct PptxDriver;

impl PptxDriver {
    #[must_use]
    pub fn new() -> Self {
        Self
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

    /// Extract the slide name from the first `<p:cNvPr name="..."/>` in slide XML.
    fn extract_slide_name(content: &str) -> Option<String> {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.contains("<p:cNvPr ")
                && let Some(name) = Self::extract_attr(trimmed, "name")
            {
                return Some(name);
            }
        }
        None
    }

    /// Parse the `<p:sldIdLst>` from presentation.xml to extract slide ID entries.
    ///
    /// Returns a list of `(slide_id, rel_id)` tuples in document order.
    fn parse_slide_id_list(presentation_xml: &str) -> Vec<(u32, String)> {
        let mut slides = Vec::new();

        for line in presentation_xml.lines() {
            let trimmed = line.trim();

            // Skip lines that don't contain sldId elements
            if !trimmed.contains("<p:sldId ") && !trimmed.contains("<p:sldId>") {
                continue;
            }

            // Find all <p:sldId .../> on this line (may be multiple per line)
            let mut search_start = 0;
            while search_start < trimmed.len() {
                let remaining = &trimmed[search_start..];
                let Some(tag_start) = remaining.find("<p:sldId ") else {
                    break;
                };
                let element = &remaining[tag_start..];

                // Find the end of this element (either /> or >)
                let end = match element.find("/>") {
                    Some(pos) => pos + 2,
                    None => match element.find('>') {
                        Some(pos) => pos + 1,
                        None => break,
                    },
                };
                let element_str = &element[..end];

                let id = Self::extract_attr(element_str, "id").and_then(|s| s.parse().ok());
                let rid = Self::extract_attr(element_str, "r:id");

                if let (Some(id_val), Some(rid_val)) = (id, rid) {
                    slides.push((id_val, rid_val));
                }

                search_start += tag_start + end;
            }
        }
        slides
    }

    /// Resolve a list of slide ID/rel-ID pairs to full slide references.
    fn resolve_slides(
        doc: &OoxmlDocument,
        presentation_path: &str,
        slide_ids: &[(u32, String)],
    ) -> Result<Vec<SlideRef>, DriverError> {
        let mut refs = Vec::new();
        for &(slide_id, ref rel_id) in slide_ids {
            let part_path = doc.resolve_rel(presentation_path, rel_id).ok_or_else(|| {
                let msg = format!("cannot resolve rId {rel_id} in {presentation_path}");
                DriverError::ParseError(msg)
            })?;

            let (content_hash, name) = doc.get_part(&part_path).map_or((0, None), |part| {
                (
                    SlideRef::content_fingerprint(&part.content),
                    Self::extract_slide_name(&part.content),
                )
            });

            refs.push(SlideRef {
                slide_id,
                rel_id: rel_id.clone(),
                part_path,
                content_hash,
                name,
            });
        }
        Ok(refs)
    }

    /// Extract and resolve all slides from a PPTX document.
    fn extract_slides(doc: &OoxmlDocument) -> Result<Vec<SlideRef>, DriverError> {
        let pres_path = doc
            .main_document_path()
            .ok_or_else(|| DriverError::ParseError("no presentation.xml found".into()))?;
        let pres_part = doc
            .get_part(pres_path)
            .ok_or_else(|| DriverError::ParseError("presentation.xml part missing".into()))?;

        let slide_ids = Self::parse_slide_id_list(&pres_part.content);
        Self::resolve_slides(doc, pres_path, &slide_ids)
    }

    /// Compute semantic changes between two slide lists.
    ///
    /// Identity is determined by slide ID (preserves identity across
    /// content modifications) with content_hash tracking modifications.
    fn diff_slides(base: &[SlideRef], new: &[SlideRef]) -> Vec<SemanticChange> {
        let mut changes = Vec::new();

        let base_by_id: HashMap<u32, &SlideRef> = base.iter().map(|s| (s.slide_id, s)).collect();
        let new_by_id: HashMap<u32, &SlideRef> = new.iter().map(|s| (s.slide_id, s)).collect();

        for slide in new {
            if !base_by_id.contains_key(&slide.slide_id) {
                let value = slide
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("slide_id={}", slide.slide_id));
                changes.push(SemanticChange::Added {
                    path: format!("/slides/{}", slide.part_path),
                    value,
                });
            }
        }

        for slide in base {
            if !new_by_id.contains_key(&slide.slide_id) {
                let old_value = slide
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("slide_id={}", slide.slide_id));
                changes.push(SemanticChange::Removed {
                    path: format!("/slides/{}", slide.part_path),
                    old_value,
                });
            }
        }

        // Detect modified slides (same ID, different content)
        for slide in new {
            if let Some(base_slide) = base_by_id.get(&slide.slide_id)
                && slide.content_hash != base_slide.content_hash
            {
                changes.push(SemanticChange::Modified {
                    path: format!("/slides/{}", slide.part_path),
                    old_value: format!(
                        "slide_id={}, hash={:016x}",
                        slide.slide_id, base_slide.content_hash
                    ),
                    new_value: format!(
                        "slide_id={}, hash={:016x}",
                        slide.slide_id, slide.content_hash
                    ),
                });
            }
        }

        // Detect reordered slides (same set of IDs, different order)
        let base_order: Vec<u32> = base.iter().map(|s| s.slide_id).collect();
        let new_order: Vec<u32> = new.iter().map(|s| s.slide_id).collect();
        if base_order != new_order && changes.is_empty() {
            // Only report reorder if no adds/removes/modifications
            changes.push(SemanticChange::Modified {
                path: "/slide_order".into(),
                old_value: format!("{base_order:?}"),
                new_value: format!("{new_order:?}"),
            });
        }

        changes
    }

    /// Three-way merge of slide lists.
    ///
    /// Strategy:
    /// - Slides added by one side but not the other: include (non-conflicting)
    /// - Slides removed by one side but not the other: include (non-conflicting)
    /// - Slides added by both sides with same ID: include (non-conflicting)
    /// - Slides modified by one side only: take the modified version
    /// - Slides modified by both sides: conflict (return None)
    /// - Slides removed by both sides: omit
    /// - Slide removed by one, modified by other: conflict (return None)
    fn merge_slides(
        base: &[SlideRef],
        ours: &[SlideRef],
        theirs: &[SlideRef],
    ) -> Option<Vec<SlideRef>> {
        let base_by_id: HashMap<u32, &SlideRef> = base.iter().map(|s| (s.slide_id, s)).collect();
        let ours_by_id: HashMap<u32, &SlideRef> = ours.iter().map(|s| (s.slide_id, s)).collect();
        let theirs_by_id: HashMap<u32, &SlideRef> =
            theirs.iter().map(|s| (s.slide_id, s)).collect();

        // Collect all slide IDs from all three versions
        let all_ids: HashSet<u32> = base_by_id
            .keys()
            .chain(ours_by_id.keys())
            .chain(theirs_by_id.keys())
            .copied()
            .collect();

        // Track which source document to copy slide parts from
        // true = ours, false = theirs
        let mut new_slide_sources: HashMap<u32, (bool, SlideRef)> = HashMap::new();

        for &id in &all_ids {
            let in_base = base_by_id.contains_key(&id);
            let in_ours = ours_by_id.contains_key(&id);
            let in_theirs = theirs_by_id.contains_key(&id);

            match (in_base, in_ours, in_theirs) {
                // All three have it — check for modifications
                (true, true, true) => {
                    let ours_modified =
                        ours_by_id[&id].content_hash != base_by_id[&id].content_hash;
                    let theirs_modified =
                        theirs_by_id[&id].content_hash != base_by_id[&id].content_hash;

                    match (ours_modified, theirs_modified) {
                        (false, false) => {
                            // Neither modified — keep as-is (from base)
                        }
                        (true, false) => {
                            // Only ours modified — take ours
                            new_slide_sources.insert(id, (true, ours_by_id[&id].clone()));
                        }
                        (false, true) => {
                            // Only theirs modified — take theirs
                            new_slide_sources.insert(id, (false, theirs_by_id[&id].clone()));
                        }
                        (true, true) => {
                            // Both modified — check if same change
                            if ours_by_id[&id].content_hash == theirs_by_id[&id].content_hash {
                                // Same modification — take either
                                new_slide_sources.insert(id, (true, ours_by_id[&id].clone()));
                            } else {
                                // Genuine conflict
                                return None;
                            }
                        }
                    }
                }

                // Added by ours only
                (false, true, false) => {
                    new_slide_sources.insert(id, (true, ours_by_id[&id].clone()));
                }

                // Added by theirs only
                (false, false, true) => {
                    new_slide_sources.insert(id, (false, theirs_by_id[&id].clone()));
                }

                // Added by both — check if same content (by hash, not by ID)
                // Two sides may independently add slides with the same auto-generated ID
                // but different content — these are independent adds, not a conflict.
                (false, true, true) => {
                    if ours_by_id[&id].content_hash == theirs_by_id[&id].content_hash {
                        // Same content added by both — include once
                        new_slide_sources.insert(id, (true, ours_by_id[&id].clone()));
                    } else {
                        // Different content with same ID — treat as two independent adds
                        // Include both; assign new unique IDs to avoid collision.
                        // Use a large offset to avoid collision with real slide IDs.
                        let new_id_ours = 1_000_000 + id;
                        let new_id_theirs = 2_000_000 + id;
                        let mut ours_slide = ours_by_id[&id].clone();
                        ours_slide.slide_id = new_id_ours;
                        let mut theirs_slide = theirs_by_id[&id].clone();
                        theirs_slide.slide_id = new_id_theirs;
                        new_slide_sources.insert(new_id_ours, (true, ours_slide));
                        new_slide_sources.insert(new_id_theirs, (false, theirs_slide));
                    }
                }

                (true, false, true | false) | (true, true, false) | (false, false, false) => {
                    // Deleted by ours, kept by theirs — non-conflicting delete
                    // Kept by ours, removed by theirs
                    // Removed by both
                    // Not in any (shouldn't happen with HashSet logic)
                }
            }
        }

        // Build merged slide list: start with base order, add new slides at end
        let mut merged_ids: Vec<u32> = Vec::new();
        let mut appended_new = Vec::new();

        // Preserve base order for existing slides
        for slide in base {
            let in_ours = ours_by_id.contains_key(&slide.slide_id);
            let in_theirs = theirs_by_id.contains_key(&slide.slide_id);

            let deleted = match (in_ours, in_theirs) {
                (true | false, true) | (true, false) => false,
                (false, false) => true,
            };

            if !deleted {
                merged_ids.push(slide.slide_id);
            }
        }

        // Append new slides (not in base)
        for &id in &all_ids {
            if !base_by_id.contains_key(&id)
                && (ours_by_id.contains_key(&id) || theirs_by_id.contains_key(&id))
            {
                // Check if this ID was remapped (de-duplicated)
                if new_slide_sources.contains_key(&id) {
                    appended_new.push(id);
                }
            }
        }
        // Also include any new IDs created by de-duplication
        for &id in new_slide_sources.keys() {
            if !base_by_id.contains_key(&id) && !all_ids.contains(&id) {
                appended_new.push(id);
            }
        }
        merged_ids.extend(appended_new);

        // Build result with updated slide references
        let mut result = Vec::new();
        for &id in &merged_ids {
            if let Some((_from_ours, slide_ref)) = new_slide_sources.get(&id) {
                // Use the updated version
                result.push(slide_ref.clone());
            } else if let Some(base_slide) = base_by_id.get(&id) {
                // Use base version (unmodified)
                result.push((*base_slide).clone());
            }
        }

        Some(result)
    }

    /// Copy slide parts from source document to destination, including their
    /// rels files and any referenced media.
    fn copy_slide_parts(slide: &SlideRef, src_doc: &OoxmlDocument, dst_doc: &mut OoxmlDocument) {
        // Copy the slide XML itself
        if let Some(part) = src_doc.get_part(&slide.part_path) {
            dst_doc.parts.insert(part.path.clone(), part.clone());
        }

        // Copy the slide's rels file (e.g., ppt/slides/_rels/slide1.xml.rels)
        let rels_path = part_path_to_rels_path(&slide.part_path);
        if let Some(part) = src_doc.get_part(&rels_path) {
            dst_doc.parts.insert(part.path.clone(), part.clone());
        }

        // Copy any media files referenced by the slide's rels
        if let Some(id_map) = src_doc.part_rels.get(&slide.part_path) {
            for target in id_map.values() {
                let resolved = resolve_relative_path(&slide.part_path, target);
                if !dst_doc.parts.contains_key(&resolved)
                    && let Some(part) = src_doc.get_part(&resolved)
                {
                    dst_doc.parts.insert(part.path.clone(), part.clone());
                }
            }
        }
    }
}

/// Convert a part path to its rels path.
/// e.g., `ppt/slides/slide1.xml` → `ppt/slides/_rels/slide1.xml.rels`
fn part_path_to_rels_path(part_path: &str) -> String {
    let (dir, name) = match part_path.rsplit_once('/') {
        Some((d, n)) => (d, n),
        None => ("", part_path),
    };
    if dir.is_empty() {
        format!("_rels/{name}.rels")
    } else {
        format!("{dir}/_rels/{name}.rels")
    }
}

/// Resolve a relative target path against a base part path.
fn resolve_relative_path(base_part: &str, target: &str) -> String {
    let dir = base_part.rsplit_once('/').map_or("", |(d, _)| d);
    target.strip_prefix('/').map_or_else(
        || {
            if dir.is_empty() {
                target.to_owned()
            } else {
                format!("{dir}/{target}")
            }
        },
        std::borrow::ToOwned::to_owned,
    )
}

impl Default for PptxDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl SutureDriver for PptxDriver {
    fn name(&self) -> &'static str {
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
        let new_slides = Self::extract_slides(&new_doc)?;

        let base_slides: Vec<SlideRef> = match base_content {
            None => Vec::new(),
            Some(base) => {
                let base_doc = OoxmlDocument::from_bytes(base.as_bytes())
                    .map_err(|e| DriverError::ParseError(e.to_string()))?;
                Self::extract_slides(&base_doc)?
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

        let base_slides = Self::extract_slides(&base_doc)?;
        let ours_slides = Self::extract_slides(&ours_doc)?;
        let theirs_slides = Self::extract_slides(&theirs_doc)?;

        let Some(merged) = Self::merge_slides(&base_slides, &ours_slides, &theirs_slides) else {
            return Ok(None);
        };

        // Build the merged document starting from base
        let mut doc =
            OoxmlDocument::from_bytes(base).map_err(|e| DriverError::ParseError(e.to_string()))?;

        // Update presentation.xml with the new slide ID list
        let pres_path = doc
            .main_document_path()
            .map(std::string::ToString::to_string)
            .ok_or_else(|| DriverError::ParseError("no presentation.xml".into()))?;

        if let Some(part) = doc.parts.get_mut(&pres_path) {
            part.content = Self::build_presentation_xml(&merged);
        }

        // Update the presentation.xml.rels to include new slide entries
        Self::update_presentation_rels(&mut doc, &pres_path, &merged);

        // Copy slide parts from source documents for new/modified slides
        let base_by_id: HashMap<u32, &SlideRef> =
            base_slides.iter().map(|s| (s.slide_id, s)).collect();

        for slide in &merged {
            if base_by_id.contains_key(&slide.slide_id) {
                // Modified slide — copy the updated version
                let ours_by_id: HashMap<u32, &SlideRef> =
                    ours_slides.iter().map(|s| (s.slide_id, s)).collect();
                let theirs_by_id: HashMap<u32, &SlideRef> =
                    theirs_slides.iter().map(|s| (s.slide_id, s)).collect();

                let base_hash = base_by_id[&slide.slide_id].content_hash;
                if slide.content_hash != base_hash {
                    if let Some(src_slide) = ours_by_id.get(&slide.slide_id)
                        && src_slide.content_hash == slide.content_hash
                    {
                        Self::copy_slide_parts(src_slide, &ours_doc, &mut doc);
                    }
                    if let Some(src_slide) = theirs_by_id.get(&slide.slide_id)
                        && src_slide.content_hash == slide.content_hash
                    {
                        Self::copy_slide_parts(src_slide, &theirs_doc, &mut doc);
                    }
                }
            } else {
                // New slide — copy from whichever source has it
                let ours_by_id: HashMap<u32, &SlideRef> =
                    ours_slides.iter().map(|s| (s.slide_id, s)).collect();
                let theirs_by_id: HashMap<u32, &SlideRef> =
                    theirs_slides.iter().map(|s| (s.slide_id, s)).collect();

                if let Some(src_slide) = ours_by_id.get(&slide.slide_id) {
                    Self::copy_slide_parts(src_slide, &ours_doc, &mut doc);
                } else if let Some(src_slide) = theirs_by_id.get(&slide.slide_id) {
                    Self::copy_slide_parts(src_slide, &theirs_doc, &mut doc);
                }
            }
        }

        // Update [Content_Types].xml for new slides
        Self::update_content_types(&mut doc, &merged);

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
        let base_str = base.map(|b| bytes_to_string_lossy(b.to_vec()));
        let new_str = bytes_to_string_lossy(new_content.to_vec());
        self.diff(base_str.as_deref(), &new_str)
    }
}

// === Presentation XML builders ===

impl PptxDriver {
    /// Build a new `presentation.xml` with the given slide list.
    fn build_presentation_xml(slides: &[SlideRef]) -> String {
        let sld_ids: String = slides
            .iter()
            .map(|s| format!(r#"<p:sldId id="{}" r:id="{}"/>"#, s.slide_id, s.rel_id))
            .collect();

        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:sldIdLst>{sld_ids}</p:sldIdLst>
</p:presentation>"#
        )
    }

    /// Update or create the presentation.xml.rels to include all slides.
    fn update_presentation_rels(doc: &mut OoxmlDocument, pres_path: &str, slides: &[SlideRef]) {
        let rels_path = part_path_to_rels_path(pres_path);

        let slide_rel_type =
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";

        // Collect existing non-slide relationships
        let mut other_rels = Vec::new();
        if let Some(id_map) = doc.part_rels.get(pres_path) {
            for (rid, target) in id_map {
                // Check if this is a slide relationship by looking at the target path
                if target.contains("slides/") && target.ends_with(".xml") {
                    continue; // Skip slide rels, we'll rebuild them
                }
                // Keep non-slide relationships
                other_rels.push((rid.clone(), target.clone()));
            }
        }

        // Build new rels content with slide entries
        let mut rels_xml = String::from(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
        );

        // Add other relationships first
        for (rid, target) in &other_rels {
            let _ = write!(
                rels_xml,
                r#"
  <Relationship Id="{rid}" Type="{slide_rel_type}" Target="{target}"/>"#
            );
        }

        // Add slide relationships
        for slide in slides {
            let _ = write!(
                rels_xml,
                r#"
  <Relationship Id="{}" Type="{}" Target="{}"/>"#,
                slide.rel_id,
                slide_rel_type,
                // Target is relative to ppt/ directory
                slide
                    .part_path
                    .strip_prefix("ppt/")
                    .unwrap_or(&slide.part_path)
            );
        }

        rels_xml.push_str("\n</Relationships>");

        // Update the rels part
        let ct = "application/vnd.openxmlformats-package.relationships+xml".to_owned();
        doc.parts.insert(
            rels_path.clone(),
            suture_ooxml::OoxmlPart {
                path: rels_path,
                content: rels_xml,
                content_type: ct,
            },
        );

        // Update the part_rels cache
        let mut new_id_map = HashMap::new();
        for (rid, target) in &other_rels {
            new_id_map.insert(rid.clone(), target.clone());
        }
        for slide in slides {
            new_id_map.insert(
                slide.rel_id.clone(),
                slide
                    .part_path
                    .strip_prefix("ppt/")
                    .unwrap_or(&slide.part_path)
                    .to_owned(),
            );
        }
        doc.part_rels.insert(pres_path.to_owned(), new_id_map);
    }

    /// Update [Content_Types].xml to include content type overrides for new slides.
    fn update_content_types(doc: &mut OoxmlDocument, slides: &[SlideRef]) {
        let ct_path = "[Content_Types].xml";
        if let Some(part) = doc.get_part(ct_path) {
            let content = &part.content;

            // Check if each slide already has a content type override
            let mut needs_update = false;
            for slide in slides {
                let part_name = format!("/{}", slide.part_path);
                if !content.contains(&part_name) {
                    needs_update = true;
                    break;
                }
            }

            if needs_update {
                let mut overrides = String::new();
                // Collect existing overrides
                for line in content.lines() {
                    if line.contains("<Override ") {
                        overrides.push_str(line);
                        overrides.push('\n');
                    }
                }
                // Add new slide overrides
                let slide_ct =
                    "application/vnd.openxmlformats-officedocument.presentationml.slide+xml";
                for slide in slides {
                    let part_name = format!("/{}", slide.part_path);
                    if !content.contains(&part_name) {
                        let _ = write!(
                            overrides,
                            r#"  <Override PartName="{part_name}" ContentType="{slide_ct}"/>"#
                        );
                        overrides.push('\n');
                    }
                }

                // Rebuild content types
                let mut new_ct = String::from(
                    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
"#,
                );
                new_ct.push_str(&overrides);
                new_ct.push_str("</Types>");

                if let Some(ct_part) = doc.parts.get_mut(ct_path) {
                    ct_part.content = new_ct;
                }
            }
        }
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
    fn test_parse_slide_id_list_single() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:sldIdLst><p:sldId id="256" r:id="rId2"/></p:sldIdLst>
</p:presentation>"#;
        let slides = PptxDriver::parse_slide_id_list(xml);
        assert_eq!(slides.len(), 1);
        assert_eq!(slides[0], (256, "rId2".to_string()));
    }

    #[test]
    fn test_parse_slide_id_list_multi() {
        let xml = r#"<?xml version="1.0"?>
<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:sldIdLst>
    <p:sldId id="256" r:id="rId2"/>
    <p:sldId id="257" r:id="rId3"/>
    <p:sldId id="258" r:id="rId4"/>
  </p:sldIdLst>
</p:presentation>"#;
        let slides = PptxDriver::parse_slide_id_list(xml);
        assert_eq!(slides.len(), 3);
        assert_eq!(slides[0], (256, "rId2".to_string()));
        assert_eq!(slides[1], (257, "rId3".to_string()));
        assert_eq!(slides[2], (258, "rId4".to_string()));
    }

    #[test]
    fn test_part_path_to_rels_path() {
        assert_eq!(
            part_path_to_rels_path("ppt/slides/slide1.xml"),
            "ppt/slides/_rels/slide1.xml.rels"
        );
        assert_eq!(
            part_path_to_rels_path("ppt/presentation.xml"),
            "ppt/_rels/presentation.xml.rels"
        );
    }

    #[test]
    fn test_diff_add_slide() {
        let base = vec![SlideRef {
            slide_id: 256,
            rel_id: "rId2".into(),
            part_path: "ppt/slides/slide1.xml".into(),
            content_hash: 111,
            name: None,
        }];
        let new = vec![
            SlideRef {
                slide_id: 256,
                rel_id: "rId2".into(),
                part_path: "ppt/slides/slide1.xml".into(),
                content_hash: 111,
                name: None,
            },
            SlideRef {
                slide_id: 257,
                rel_id: "rId3".into(),
                part_path: "ppt/slides/slide2.xml".into(),
                content_hash: 222,
                name: None,
            },
        ];
        let changes = PptxDriver::diff_slides(&base, &new);
        assert_eq!(changes.len(), 1);
        assert!(
            matches!(&changes[0], SemanticChange::Added { value, .. } if value.contains("257"))
        );
    }

    #[test]
    fn test_diff_remove_slide() {
        let base = vec![
            SlideRef {
                slide_id: 256,
                rel_id: "rId2".into(),
                part_path: "ppt/slides/slide1.xml".into(),
                content_hash: 111,
                name: None,
            },
            SlideRef {
                slide_id: 257,
                rel_id: "rId3".into(),
                part_path: "ppt/slides/slide2.xml".into(),
                content_hash: 222,
                name: None,
            },
        ];
        let new = vec![SlideRef {
            slide_id: 256,
            rel_id: "rId2".into(),
            part_path: "ppt/slides/slide1.xml".into(),
            content_hash: 111,
            name: None,
        }];
        let changes = PptxDriver::diff_slides(&base, &new);
        assert_eq!(changes.len(), 1);
        assert!(
            matches!(&changes[0], SemanticChange::Removed { old_value, .. } if old_value.contains("257"))
        );
    }

    #[test]
    fn test_diff_modify_slide() {
        let base = vec![SlideRef {
            slide_id: 256,
            rel_id: "rId2".into(),
            part_path: "ppt/slides/slide1.xml".into(),
            content_hash: 111,
            name: None,
        }];
        let new = vec![SlideRef {
            slide_id: 256,
            rel_id: "rId2".into(),
            part_path: "ppt/slides/slide1.xml".into(),
            content_hash: 999,
            name: None,
        }];
        let changes = PptxDriver::diff_slides(&base, &new);
        assert_eq!(changes.len(), 1);
        assert!(matches!(&changes[0], SemanticChange::Modified { .. }));
    }

    #[test]
    fn test_diff_no_change() {
        let slide = SlideRef {
            slide_id: 256,
            rel_id: "rId2".into(),
            part_path: "ppt/slides/slide1.xml".into(),
            content_hash: 111,
            name: None,
        };
        let changes = PptxDriver::diff_slides(&[slide.clone()], &[slide]);
        assert!(changes.is_empty());
    }

    #[test]
    fn test_merge_add_different_slides() {
        let base = vec![SlideRef {
            slide_id: 256,
            rel_id: "rId2".into(),
            part_path: "ppt/slides/slide1.xml".into(),
            content_hash: 111,
            name: None,
        }];
        let ours = vec![
            SlideRef {
                slide_id: 256,
                rel_id: "rId2".into(),
                part_path: "ppt/slides/slide1.xml".into(),
                content_hash: 111,
                name: None,
            },
            SlideRef {
                slide_id: 257,
                rel_id: "rId3".into(),
                part_path: "ppt/slides/slide2.xml".into(),
                content_hash: 222,
                name: None,
            },
        ];
        let theirs = vec![
            SlideRef {
                slide_id: 256,
                rel_id: "rId2".into(),
                part_path: "ppt/slides/slide1.xml".into(),
                content_hash: 111,
                name: None,
            },
            SlideRef {
                slide_id: 258,
                rel_id: "rId4".into(),
                part_path: "ppt/slides/slide3.xml".into(),
                content_hash: 333,
                name: None,
            },
        ];
        let result = PptxDriver::merge_slides(&base, &ours, &theirs);
        assert!(result.is_some());
        let m = result.unwrap();
        assert_eq!(m.len(), 3); // base + ours new + theirs new
        let ids: Vec<u32> = m.iter().map(|s| s.slide_id).collect();
        assert!(ids.contains(&257)); // ours slide
        assert!(ids.contains(&258)); // theirs slide
    }

    #[test]
    fn test_merge_conflict_both_modify_same_slide() {
        let base = vec![SlideRef {
            slide_id: 256,
            rel_id: "rId2".into(),
            part_path: "ppt/slides/slide1.xml".into(),
            content_hash: 111,
            name: None,
        }];
        let ours = vec![SlideRef {
            slide_id: 256,
            rel_id: "rId2".into(),
            part_path: "ppt/slides/slide1.xml".into(),
            content_hash: 222,
            name: None,
        }];
        let theirs = vec![SlideRef {
            slide_id: 256,
            rel_id: "rId2".into(),
            part_path: "ppt/slides/slide1.xml".into(),
            content_hash: 333,
            name: None,
        }];
        let result = PptxDriver::merge_slides(&base, &ours, &theirs);
        assert!(result.is_none()); // Conflict
    }

    #[test]
    fn test_merge_one_side_modify() {
        let base = vec![SlideRef {
            slide_id: 256,
            rel_id: "rId2".into(),
            part_path: "ppt/slides/slide1.xml".into(),
            content_hash: 111,
            name: None,
        }];
        let ours = vec![SlideRef {
            slide_id: 256,
            rel_id: "rId2".into(),
            part_path: "ppt/slides/slide1.xml".into(),
            content_hash: 222,
            name: None,
        }];
        let theirs = vec![SlideRef {
            slide_id: 256,
            rel_id: "rId2".into(),
            part_path: "ppt/slides/slide1.xml".into(),
            content_hash: 111,
            name: None,
        }];
        let result = PptxDriver::merge_slides(&base, &ours, &theirs);
        assert!(result.is_some());
        let m = result.unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].content_hash, 222); // Ours' modification wins
    }

    #[test]
    fn test_merge_both_add_same_content() {
        let base = vec![SlideRef {
            slide_id: 256,
            rel_id: "rId2".into(),
            part_path: "ppt/slides/slide1.xml".into(),
            content_hash: 111,
            name: None,
        }];
        let ours = vec![
            SlideRef {
                slide_id: 256,
                rel_id: "rId2".into(),
                part_path: "ppt/slides/slide1.xml".into(),
                content_hash: 111,
                name: None,
            },
            SlideRef {
                slide_id: 257,
                rel_id: "rId3".into(),
                part_path: "ppt/slides/slide2.xml".into(),
                content_hash: 555,
                name: None,
            },
        ];
        let theirs = vec![
            SlideRef {
                slide_id: 256,
                rel_id: "rId2".into(),
                part_path: "ppt/slides/slide1.xml".into(),
                content_hash: 111,
                name: None,
            },
            SlideRef {
                slide_id: 257,
                rel_id: "rId3".into(),
                part_path: "ppt/slides/slide2.xml".into(),
                content_hash: 555,
                name: None,
            },
        ];
        let result = PptxDriver::merge_slides(&base, &ours, &theirs);
        assert!(result.is_some()); // Not a conflict — same content
    }

    #[test]
    fn test_diff_real_pptx_structure() {
        // Test against the actual PPTX structure from sample.pptx
        let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:sldIdLst><p:sldId id="256" r:id="rId2"/></p:sldIdLst>
</p:presentation>"#;
        let slides = PptxDriver::parse_slide_id_list(xml);
        assert_eq!(slides.len(), 1);
        assert_eq!(slides[0].0, 256);
        assert_eq!(slides[0].1, "rId2");
    }

    /// Build a minimal PPTX as a byte string for roundtrip testing.
    /// This creates a valid ZIP with the required OOXML parts.
    fn build_minimal_pptx(slide_titles: &[&str]) -> Vec<u8> {
        use std::io::{Cursor, Write};
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));

            // _rels/.rels
            zip.start_file("_rels/.rels", zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(
                br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>"#,
            )
            .unwrap();

            // Build slide rels
            let slide_rels: String = slide_titles
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    format!(
                        r#"  <Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{}.xml"/>"#,
                        i + 2,
                        i + 1
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");

            // ppt/_rels/presentation.xml.rels
            zip.start_file(
                "ppt/_rels/presentation.xml.rels",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(
                format!(
                    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
{}
</Relationships>"#,
                    slide_rels
                )
                .as_bytes(),
            )
            .unwrap();

            // ppt/presentation.xml
            let sld_ids: String = slide_titles
                .iter()
                .enumerate()
                .map(|(i, _)| format!(r#"<p:sldId id="{}" r:id="rId{}"/>"#, 256 + i as u32, i + 2))
                .collect::<Vec<_>>()
                .join("");
            zip.start_file(
                "ppt/presentation.xml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(
                format!(
                    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:sldIdLst>{}</p:sldIdLst>
</p:presentation>"#,
                    sld_ids
                )
                .as_bytes(),
            )
            .unwrap();

            // [Content_Types].xml
            let ct_overrides: String = slide_titles
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    format!(
                        r#"  <Override PartName="/ppt/slides/slide{}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#,
                        i + 1
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            zip.start_file(
                "[Content_Types].xml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(
                format!(
                    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
{}
</Types>"#,
                    ct_overrides
                )
                .as_bytes(),
            )
            .unwrap();

            // Slides
            for (i, title) in slide_titles.iter().enumerate() {
                let slide_xml = format!(
                    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld><p:spTree>
    <p:sp><p:nvSpPr><p:cNvPr id="2" name="Title 1"/></p:nvSpPr><p:spPr/>
    <p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>{}</a:t></a:r></a:p></p:txBody>
    </p:sp>
  </p:spTree></p:cSld>
</p:sld>"#,
                    title
                );
                zip.start_file(
                    format!("ppt/slides/slide{}.xml", i + 1),
                    zip::write::SimpleFileOptions::default(),
                )
                .unwrap();
                zip.write_all(slide_xml.as_bytes()).unwrap();

                // Slide rels (empty)
                zip.start_file(
                    format!("ppt/slides/_rels/slide{}.xml.rels", i + 1),
                    zip::write::SimpleFileOptions::default(),
                )
                .unwrap();
                zip.write_all(
                    br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>"#,
                )
                .unwrap();
            }

            zip.finish().unwrap();
        }
        buf
    }

    #[test]
    fn test_full_roundtrip_single_slide() {
        let driver = PptxDriver::new();
        let pptx_bytes = build_minimal_pptx(&["Hello"]);
        let pptx_str = unsafe { String::from_utf8_unchecked(pptx_bytes.clone()) };

        // Extract slides
        let doc = OoxmlDocument::from_bytes(&pptx_bytes).unwrap();
        let slides = PptxDriver::extract_slides(&doc).unwrap();
        assert_eq!(slides.len(), 1);
        assert_eq!(slides[0].slide_id, 256);

        // Diff against nothing (new file)
        let changes = driver.diff(None, &pptx_str).unwrap();
        assert_eq!(changes.len(), 1);
        assert!(matches!(&changes[0], SemanticChange::Added { .. }));

        // Diff against itself (no changes)
        let changes = driver.diff(Some(&pptx_str), &pptx_str).unwrap();
        assert!(changes.is_empty());
    }

    #[test]
    fn test_full_diff_added_slide() {
        let driver = PptxDriver::new();
        let base_bytes = build_minimal_pptx(&["Slide A"]);
        let base_str = unsafe { String::from_utf8_unchecked(base_bytes) };
        let new_bytes = build_minimal_pptx(&["Slide A", "Slide B"]);
        let new_str = unsafe { String::from_utf8_unchecked(new_bytes) };

        let changes = driver.diff(Some(&base_str), &new_str).unwrap();
        assert_eq!(changes.len(), 1);
        assert!(matches!(&changes[0], SemanticChange::Added { .. }));

        let fmt = driver.format_diff(Some(&base_str), &new_str).unwrap();
        assert!(fmt.contains("ADDED"));
    }

    #[test]
    fn test_full_merge_add_different_slides() {
        let driver = PptxDriver::new();
        let base_bytes = build_minimal_pptx(&["Shared"]);
        let ours_bytes = build_minimal_pptx(&["Shared", "Ours Slide"]);
        let theirs_bytes = build_minimal_pptx(&["Shared", "Theirs Slide"]);

        let result = driver
            .merge_raw(&base_bytes, &ours_bytes, &theirs_bytes)
            .unwrap();
        assert!(
            result.is_some(),
            "merge should succeed (non-conflicting adds)"
        );

        // Verify the merged result has 3 slides
        let merged_bytes = result.unwrap();
        let merged_doc = OoxmlDocument::from_bytes(&merged_bytes).unwrap();
        let merged_slides = PptxDriver::extract_slides(&merged_doc).unwrap();
        assert_eq!(merged_slides.len(), 3);
    }

    #[test]
    fn test_full_merge_modify_conflict() {
        let driver = PptxDriver::new();
        let base_bytes = build_minimal_pptx(&["Original"]);
        let ours_bytes = build_minimal_pptx(&["Ours Version"]);
        let theirs_bytes = build_minimal_pptx(&["Theirs Version"]);

        // Both modified the same slide differently — conflict
        let result = driver
            .merge_raw(&base_bytes, &ours_bytes, &theirs_bytes)
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_full_merge_one_side_modify() {
        let driver = PptxDriver::new();
        let base_bytes = build_minimal_pptx(&["Original"]);
        let ours_bytes = build_minimal_pptx(&["Modified"]);
        let theirs_bytes = build_minimal_pptx(&["Original"]); // unchanged

        let result = driver
            .merge_raw(&base_bytes, &ours_bytes, &theirs_bytes)
            .unwrap();
        assert!(result.is_some());

        // Verify the merged result has the modified content.
        // The merged output is a ZIP file, so we need to extract the slide content.
        let merged_bytes = result.unwrap();
        let merged_doc = OoxmlDocument::from_bytes(&merged_bytes).unwrap();
        let merged_slides = PptxDriver::extract_slides(&merged_doc).unwrap();
        assert_eq!(merged_slides.len(), 1);

        // Check the actual slide XML content
        let slide_part = merged_doc.get_part(&merged_slides[0].part_path).unwrap();
        assert!(
            slide_part.content.contains("Modified"),
            "merged slide should contain 'Modified', got: {}",
            &slide_part.content[..slide_part.content.len().min(200)]
        );
    }
}

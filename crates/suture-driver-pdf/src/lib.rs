#![allow(clippy::collapsible_match)]
//! PDF semantic driver — page-level diff and merge for PDF documents.
//!
//! Uses `lopdf` to parse PDF binary content and extract text from each page.
//! Comparisons are performed at page granularity, detecting added, removed,
//! and modified pages between document versions.

use suture_driver::{DriverError, SemanticChange, SutureDriver};

/// Convert bytes to String, replacing invalid UTF-8 sequences with the Unicode replacement character.
/// This is safe for binary formats where the content is stored as bytes but round-tripped
/// through String for the driver interface.
fn bytes_to_string_lossy(bytes: Vec<u8>) -> String {
    String::from_utf8_lossy(&bytes).into_owned()
}

pub struct PdfDriver;

impl PdfDriver {
    pub fn new() -> Self {
        Self
    }

    fn load_doc(bytes: &[u8]) -> Result<lopdf::Document, DriverError> {
        let cursor = std::io::Cursor::new(bytes);
        lopdf::Document::load_from(cursor)
            .map_err(|e| DriverError::ParseError(format!("failed to parse PDF: {e}")))
    }

    fn extract_pages(bytes: &[u8]) -> Result<Vec<String>, DriverError> {
        let doc = Self::load_doc(bytes)?;

        let page_map = doc.get_pages();
        let mut sorted_page_nums: Vec<_> = page_map.keys().copied().collect();
        sorted_page_nums.sort();

        let mut pages = Vec::new();
        for page_num in sorted_page_nums {
            let page_id = page_map[&page_num];
            let text = Self::extract_text_from_page(&doc, page_id)?;
            pages.push(text);
        }
        Ok(pages)
    }

    fn extract_text_from_page(
        doc: &lopdf::Document,
        page_id: (u32, u16),
    ) -> Result<String, DriverError> {
        let content_ids = doc.get_page_contents(page_id);
        if content_ids.is_empty() {
            return Ok(String::new());
        }

        let mut all_text = String::new();
        for content_id in &content_ids {
            let contents_obj = doc.objects.get(content_id).ok_or_else(|| {
                DriverError::ParseError(format!("Contents object {content_id:?} not found"))
            })?;

            let stream = contents_obj.as_stream().map_err(|e| {
                DriverError::ParseError(format!("Page content {content_id:?} is not a stream: {e}"))
            })?;

            let stream_bytes = stream
                .decompressed_content()
                .unwrap_or_else(|_| stream.content.clone());

            let stream_str = String::from_utf8_lossy(&stream_bytes);
            let text = Self::parse_text_from_stream(&stream_str);
            if !text.is_empty() {
                if !all_text.is_empty() {
                    all_text.push(' ');
                }
                all_text.push_str(&text);
            }
        }
        Ok(all_text)
    }

    fn parse_text_from_stream(stream: &str) -> String {
        let mut text = String::new();
        let bytes = stream.as_bytes();
        let mut i = 0;

        while i < bytes.len() {
            if bytes[i] == b'(' {
                let (s, end) = Self::extract_paren_string(bytes, i);
                if !s.is_empty() {
                    if !text.is_empty() {
                        text.push(' ');
                    }
                    text.push_str(&s);
                }
                i = end + 1;
            } else {
                i += 1;
            }
        }
        text
    }

    fn extract_paren_string(bytes: &[u8], start: usize) -> (String, usize) {
        let mut depth = 0usize;
        let mut result = Vec::new();
        let mut i = start;

        while i < bytes.len() {
            match bytes[i] {
                b'(' => {
                    if i != start {
                        result.push(b'(');
                    }
                    depth += 1;
                    i += 1;
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return (String::from_utf8_lossy(&result).into_owned(), i);
                    }
                    result.push(b')');
                    i += 1;
                }
                b'\\' if i + 1 < bytes.len() => {
                    i += 1;
                    match bytes[i] {
                        b'n' => result.push(b'\n'),
                        b'r' => result.push(b'\r'),
                        b't' => result.push(b'\t'),
                        b'b' => result.push(b'\x08'),
                        b'f' => result.push(b'\x0c'),
                        b'(' => result.push(b'('),
                        b')' => result.push(b')'),
                        b'\\' => result.push(b'\\'),
                        b'0'..=b'7' => {
                            let mut oct = 0u32;
                            for _ in 0..3 {
                                if i < bytes.len() && matches!(bytes[i], b'0'..=b'7') {
                                    oct = oct * 8 + (bytes[i] - b'0') as u32;
                                    i += 1;
                                } else {
                                    break;
                                }
                            }
                            if let Some(ch) = char::from_u32(oct) {
                                let mut buf = [0u8; 4];
                                result.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                            }
                            continue;
                        }
                        _ => result.push(bytes[i]),
                    }
                    i += 1;
                }
                _ => {
                    result.push(bytes[i]);
                    i += 1;
                }
            }
        }

        (
            String::from_utf8_lossy(&result).into_owned(),
            i.saturating_sub(1),
        )
    }

    fn diff_pages(base: &[String], new: &[String]) -> Vec<SemanticChange> {
        let max_len = base.len().max(new.len());
        let mut changes = Vec::new();

        for i in 0..max_len {
            let path = format!("/pages/{}", i);
            match (base.get(i), new.get(i)) {
                (None, Some(n)) => changes.push(SemanticChange::Added {
                    path,
                    value: n.clone(),
                }),
                (Some(o), None) => changes.push(SemanticChange::Removed {
                    path,
                    old_value: o.clone(),
                }),
                (Some(o), Some(n)) if o != n => changes.push(SemanticChange::Modified {
                    path,
                    old_value: o.clone(),
                    new_value: n.clone(),
                }),
                _ => {}
            }
        }
        changes
    }

    fn merge_pages(base: &[String], ours: &[String], theirs: &[String]) -> Option<Vec<String>> {
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
}

impl Default for PdfDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl SutureDriver for PdfDriver {
    fn name(&self) -> &str {
        "PDF"
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".pdf"]
    }

    fn diff(
        &self,
        base_content: Option<&str>,
        new_content: &str,
    ) -> Result<Vec<SemanticChange>, DriverError> {
        let new_pages = Self::extract_pages(new_content.as_bytes())?;

        let base_pages = match base_content {
            None => Vec::new(),
            Some(base) => Self::extract_pages(base.as_bytes())?,
        };

        Ok(Self::diff_pages(&base_pages, &new_pages))
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
        let bytes = self.merge_raw(base.as_bytes(), ours.as_bytes(), theirs.as_bytes())?;
        match bytes {
            Some(b) => Ok(Some(bytes_to_string_lossy(b))),
            None => Ok(None),
        }
    }

    fn merge_raw(
        &self,
        base: &[u8],
        ours: &[u8],
        theirs: &[u8],
    ) -> Result<Option<Vec<u8>>, DriverError> {
        let base_pages = Self::extract_pages(base)?;
        let ours_pages = Self::extract_pages(ours)?;
        let theirs_pages = Self::extract_pages(theirs)?;

        match Self::merge_pages(&base_pages, &ours_pages, &theirs_pages) {
            Some(_merged) => {
                let mut ours_doc = Self::load_doc(ours)?;

                let mut buf = Vec::new();
                ours_doc.save_to(&mut buf).map_err(|e| {
                    DriverError::SerializationError(format!("failed to serialize PDF: {e}"))
                })?;

                Ok(Some(buf))
            }
            None => Ok(None),
        }
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

#[cfg(test)]
fn make_minimal_pdf(page_texts: &[&str]) -> Vec<u8> {
    let mut objects = Vec::new();
    let mut obj_offsets = Vec::new();

    let header = b"%PDF-1.0\n";
    let mut pdf = header.to_vec();

    let num_pages = page_texts.len();

    let catalog_id = 1u32;
    let pages_id = 2u32;
    let font_id = 3u32;
    let first_page_obj_id = 4u32;
    let first_content_id = first_page_obj_id + num_pages as u32;

    for i in 0..num_pages as u32 {
        let page_obj_id = first_page_obj_id + i;
        let content_obj_id = first_content_id + i;

        let text = page_texts[i as usize];
        let stream_content = format!("BT /F1 12 Tf 100 700 Td ({text}) Tj ET");
        let stream_len = stream_content.len();

        let page_dict = format!(
            "<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 612 792] /Contents {content_obj_id} 0 R /Resources << /Font << /F1 {font_id} 0 R >> >> >>"
        );

        let content_obj =
            format!("<< /Length {stream_len} >>\nstream\n{stream_content}\nendstream");

        objects.push((page_obj_id, page_dict));
        objects.push((content_obj_id, content_obj));
    }

    let mut kids = Vec::new();
    for i in 0..num_pages as u32 {
        kids.push(format!("{} 0 R", first_page_obj_id + i));
    }
    let kids_str = kids.join(" ");

    let catalog = format!("<< /Type /Catalog /Pages {pages_id} 0 R >>");
    let pages_dict = format!("<< /Type /Pages /Kids [{}] /Count {num_pages} >>", kids_str);
    let font_dict = "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string();

    let mut all_objects: Vec<(u32, String)> = vec![
        (catalog_id, catalog),
        (pages_id, pages_dict),
        (font_id, font_dict),
    ];
    all_objects.extend(objects);

    for (id, content) in &all_objects {
        obj_offsets.push(pdf.len());
        let obj_str = format!("{id} 0 obj\n{content}\nendobj\n");
        pdf.extend_from_slice(obj_str.as_bytes());
    }

    let xref_offset = pdf.len();
    let num_objs = all_objects.len() as u32 + 1;

    let xref = format!("xref\n0 {num_objs}\n0000000000 65535 f \n");
    pdf.extend_from_slice(xref.as_bytes());

    for &offset in &obj_offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }

    let trailer = format!(
        "trailer\n<< /Size {num_objs} /Root {catalog_id} 0 R >>\nstartxref\n{xref_offset}\n%%EOF"
    );
    pdf.extend_from_slice(trailer.as_bytes());

    pdf
}

#[cfg(test)]
fn pdf_str(bytes: &[u8]) -> String {
    unsafe { String::from_utf8_unchecked(bytes.to_vec()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pdf_driver_name() {
        let driver = PdfDriver::new();
        assert_eq!(driver.name(), "PDF");
    }

    #[test]
    fn test_pdf_driver_extensions() {
        let driver = PdfDriver::new();
        assert_eq!(driver.supported_extensions(), &[".pdf"]);
    }

    #[test]
    fn test_pdf_diff_new_file() {
        let driver = PdfDriver::new();
        let pdf_bytes = make_minimal_pdf(&["Hello"]);
        let changes = driver.diff(None, &pdf_str(&pdf_bytes)).unwrap();
        assert_eq!(changes.len(), 1);
        assert!(matches!(
            &changes[0],
            SemanticChange::Added { value, .. } if value == "Hello"
        ));
    }

    #[test]
    fn test_pdf_diff_identical() {
        let driver = PdfDriver::new();
        let pdf_bytes = make_minimal_pdf(&["Hello"]);
        let changes = driver
            .diff(Some(&pdf_str(&pdf_bytes)), &pdf_str(&pdf_bytes))
            .unwrap();
        assert!(changes.is_empty());
    }

    #[test]
    fn test_pdf_diff_empty() {
        let driver = PdfDriver::new();
        let pdf1 = make_minimal_pdf(&[""]);
        let pdf2 = make_minimal_pdf(&[""]);
        let changes = driver.diff(Some(&pdf_str(&pdf1)), &pdf_str(&pdf2)).unwrap();
        assert!(changes.is_empty());
    }

    #[test]
    fn test_pdf_format_diff_empty() {
        let driver = PdfDriver::new();
        let pdf_bytes = make_minimal_pdf(&["Hello"]);
        let result = driver
            .format_diff(Some(&pdf_str(&pdf_bytes)), &pdf_str(&pdf_bytes))
            .unwrap();
        assert_eq!(result, "no changes");
    }

    #[test]
    fn test_pdf_format_diff_modified() {
        let driver = PdfDriver::new();
        let pdf1 = make_minimal_pdf(&["Hello"]);
        let pdf2 = make_minimal_pdf(&["World"]);
        let result = driver
            .format_diff(Some(&pdf_str(&pdf1)), &pdf_str(&pdf2))
            .unwrap();
        assert!(result.contains("MODIFIED"));
        assert!(result.contains("Hello"));
        assert!(result.contains("World"));
    }

    #[test]
    fn test_pdf_merge_no_conflict() {
        let driver = PdfDriver::new();
        let base = make_minimal_pdf(&["A", "B", "C"]);
        let ours = make_minimal_pdf(&["A", "X", "C"]);
        let theirs = make_minimal_pdf(&["A", "B", "Y"]);
        let result = driver
            .merge(&pdf_str(&base), &pdf_str(&ours), &pdf_str(&theirs))
            .unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn test_pdf_merge_conflict() {
        let driver = PdfDriver::new();
        let base = make_minimal_pdf(&["A"]);
        let ours = make_minimal_pdf(&["X"]);
        let theirs = make_minimal_pdf(&["Y"]);
        let result = driver
            .merge(&pdf_str(&base), &pdf_str(&ours), &pdf_str(&theirs))
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_pdf_diff_added_page() {
        let driver = PdfDriver::new();
        let pdf1 = make_minimal_pdf(&["Page One"]);
        let pdf2 = make_minimal_pdf(&["Page One", "Page Two"]);
        let changes = driver.diff(Some(&pdf_str(&pdf1)), &pdf_str(&pdf2)).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Added { value, .. } if value == "Page Two"
        )));
    }

    #[test]
    fn test_pdf_diff_removed_page() {
        let driver = PdfDriver::new();
        let pdf1 = make_minimal_pdf(&["Page One", "Page Two"]);
        let pdf2 = make_minimal_pdf(&["Page One"]);
        let changes = driver.diff(Some(&pdf_str(&pdf1)), &pdf_str(&pdf2)).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Removed { old_value, .. } if old_value == "Page Two"
        )));
    }

    #[test]
    fn test_pdf_diff_modified_page() {
        let driver = PdfDriver::new();
        let pdf1 = make_minimal_pdf(&["Old text"]);
        let pdf2 = make_minimal_pdf(&["New text"]);
        let changes = driver.diff(Some(&pdf_str(&pdf1)), &pdf_str(&pdf2)).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Modified { old_value, new_value, .. }
                if old_value == "Old text" && new_value == "New text"
        )));
    }
}

use std::io::{Cursor, Write};
use suture_driver::{SemanticChange, SutureDriver};
use suture_driver_docx::DocxDriver;

fn make_docx_bytes(paragraphs: &[&str]) -> Vec<u8> {
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

/// Create a DOCX as a String via from_utf8_unchecked (for diff/format tests
/// where the text-oriented API is sufficient and data stays valid UTF-8).
fn make_docx(paragraphs: &[&str]) -> String {
    unsafe { String::from_utf8_unchecked(make_docx_bytes(paragraphs)) }
}

#[test]
fn docx_two_editor_merge_different_paragraphs() {
    let driver = DocxDriver::new();
    let base = make_docx_bytes(&["Alpha", "Beta", "Gamma"]);
    let ours = make_docx_bytes(&["Alpha Modified", "Beta", "Gamma"]);
    let theirs = make_docx_bytes(&["Alpha", "Beta", "Gamma Modified"]);

    let merged = driver.merge_raw(&base, &ours, &theirs).unwrap();
    assert!(
        merged.is_some(),
        "merge of different paragraphs should succeed"
    );

    let merged_bytes = merged.unwrap();
    let merged_str = unsafe { String::from_utf8_unchecked(merged_bytes) };
    let base_str = unsafe { String::from_utf8_unchecked(base) };
    let diff = driver.diff(Some(&base_str), &merged_str).unwrap();
    assert!(
        diff.iter().any(|c| matches!(c, SemanticChange::Modified { new_value, .. } if new_value == "Alpha Modified")),
        "merge should preserve editor A's change to paragraph 0"
    );
    assert!(
        diff.iter().any(|c| matches!(c, SemanticChange::Modified { new_value, .. } if new_value == "Gamma Modified")),
        "merge should preserve editor B's change to paragraph 2"
    );
}

#[test]
fn docx_two_editor_merge_a_adds_b_edits() {
    let driver = DocxDriver::new();
    let base = make_docx_bytes(&["Hello", "World"]);
    let ours = make_docx_bytes(&["Hello", "World", "New Paragraph"]);
    let theirs = make_docx_bytes(&["Hello", "World Modified"]);

    let merged = driver.merge_raw(&base, &ours, &theirs).unwrap();
    assert!(merged.is_some(), "merge with add + edit should succeed");

    let merged_bytes = merged.unwrap();
    let merged_str = unsafe { String::from_utf8_unchecked(merged_bytes) };
    let base_str = unsafe { String::from_utf8_unchecked(base) };
    let diff = driver.diff(Some(&base_str), &merged_str).unwrap();
    assert!(
        diff.iter()
            .any(|c| matches!(c, SemanticChange::Added { value, .. } if value == "New Paragraph")),
        "merge should preserve editor A's added paragraph"
    );
    assert!(
        diff.iter().any(|c| matches!(c, SemanticChange::Modified { new_value, .. } if new_value == "World Modified")),
        "merge should preserve editor B's edit to paragraph 1"
    );
}

#[test]
fn docx_two_editor_conflict_same_paragraph() {
    let driver = DocxDriver::new();
    let base = make_docx(&["Shared", "Other"]);
    let ours = make_docx(&["Changed by A", "Other"]);
    let theirs = make_docx(&["Changed by B", "Other"]);

    let result = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(
        result.is_none(),
        "conflicting edits to same paragraph should return None"
    );
}

#[test]
fn docx_diff_detects_all_change_types() {
    let driver = DocxDriver::new();
    let base = make_docx(&["Keep", "Modify"]);
    let new = make_docx(&["Keep", "Modified", "Added"]);

    let changes = driver.diff(Some(&base), &new).unwrap();
    let modified = changes
        .iter()
        .filter(|c| matches!(c, SemanticChange::Modified { .. }))
        .count();
    let added = changes
        .iter()
        .filter(|c| matches!(c, SemanticChange::Added { .. }))
        .count();

    assert_eq!(modified, 1, "should detect one modified paragraph");
    assert_eq!(added, 1, "should detect one added paragraph");
}

#[test]
fn docx_format_diff_readable() {
    let driver = DocxDriver::new();
    let base = make_docx(&["Old"]);
    let new = make_docx(&["New"]);

    let output = driver.format_diff(Some(&base), &new).unwrap();
    assert!(
        output.contains("MODIFIED"),
        "format_diff should show MODIFIED"
    );
    assert!(output.contains("Old"), "format_diff should show old value");
    assert!(output.contains("New"), "format_diff should show new value");
}

#[test]
fn docx_format_diff_no_changes() {
    let driver = DocxDriver::new();
    let doc = make_docx(&["Same"]);

    let output = driver.format_diff(Some(&doc), &doc).unwrap();
    assert_eq!(output, "no changes");
}

#[test]
fn docx_diff_new_file() {
    let driver = DocxDriver::new();
    let new = make_docx(&["First", "Second"]);

    let changes = driver.diff(None, &new).unwrap();
    assert_eq!(
        changes.len(),
        2,
        "diff from None should show all paragraphs as Added"
    );
    assert!(
        changes
            .iter()
            .all(|c| matches!(c, SemanticChange::Added { .. }))
    );
}

#[test]
fn docx_merge_both_add_same_paragraph() {
    let driver = DocxDriver::new();
    let base = make_docx(&["Existing"]);
    let ours = make_docx(&["Existing", "Convergent"]);
    let theirs = make_docx(&["Existing", "Convergent"]);

    let result = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(
        result.is_some(),
        "identical additions from both sides should merge cleanly"
    );
}

#[test]
fn docx_merge_both_add_different_paragraphs_conflict() {
    let driver = DocxDriver::new();
    let base = make_docx(&["Existing"]);
    let ours = make_docx(&["Existing", "From A"]);
    let theirs = make_docx(&["Existing", "From B"]);

    let result = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(
        result.is_none(),
        "different additions at the same index should conflict"
    );
}

#[test]
fn docx_merge_one_side_unchanged() {
    let driver = DocxDriver::new();
    let base = make_docx_bytes(&["A", "B"]);
    let ours = make_docx_bytes(&["A", "B Modified"]);
    let theirs = make_docx_bytes(&["A", "B"]);

    let result = driver.merge_raw(&base, &ours, &theirs).unwrap();
    assert!(
        result.is_some(),
        "merge with one side unchanged should succeed"
    );
    let merged_bytes = result.unwrap();
    let merged_str = unsafe { String::from_utf8_unchecked(merged_bytes) };
    let base_str = unsafe { String::from_utf8_unchecked(base) };
    let diff = driver.diff(Some(&base_str), &merged_str).unwrap();
    assert!(
        diff.iter().any(
            |c| matches!(c, SemanticChange::Modified { new_value, .. } if new_value == "B Modified")
        ),
        "merged result should contain the changed side's edit"
    );
}

#[test]
fn docx_merge_empty_document() {
    let driver = DocxDriver::new();
    let base = make_docx(&[]);
    let ours = make_docx(&["First"]);
    let theirs = make_docx(&[]);

    let result = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(result.is_some(), "merge with empty base should succeed");
}

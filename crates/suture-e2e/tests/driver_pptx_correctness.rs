use std::io::{Cursor, Write};
use suture_driver::{SemanticChange, SutureDriver};
use suture_driver_pptx::PptxDriver;

fn make_pptx(slide_names: &[&str]) -> String {
    let content_types = r#"<?xml version="1.0"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
</Types>"#;

    let mut pres_xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <p:presentation xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\" \
         xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\">\n",
    );
    for name in slide_names {
        pres_xml.push_str(&format!(
            "<p:sp name=\"{}\">\n<p:nvSpPr><p:cNvPr id=\"1\" name=\"{}\"/></p:nvSpPr></p:sp>\n",
            name, name
        ));
    }
    pres_xml.push_str("</p:presentation>");

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
            "ppt/presentation.xml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(pres_xml.as_bytes()).unwrap();
        zip.finish().unwrap();
    }
    unsafe { String::from_utf8_unchecked(buf) }
}

#[test]
fn pptx_two_editor_merge_different_slides_added() {
    let driver = PptxDriver::new();
    let base = make_pptx(&["Slide1"]);
    let ours = make_pptx(&["Slide1", "Slide2"]);
    let theirs = make_pptx(&["Slide1", "Slide3"]);

    let merged = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(
        merged.is_some(),
        "merge adding different slides should succeed"
    );
    assert!(
        !merged.unwrap().is_empty(),
        "merged content should not be empty"
    );
}

#[test]
fn pptx_two_editor_merge_a_adds_b_removes() {
    let driver = PptxDriver::new();
    let base = make_pptx(&["Keep", "Remove", "Stay"]);
    let ours = make_pptx(&["Keep", "Remove", "Stay", "NewSlide"]);
    let theirs = make_pptx(&["Keep", "Stay"]);

    let merged = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(merged.is_some(), "merge with add + remove should succeed");
    assert!(
        !merged.unwrap().is_empty(),
        "merged content should not be empty"
    );
}

#[test]
fn pptx_diff_detects_added_slide() {
    let driver = PptxDriver::new();
    let base = make_pptx(&["Slide1"]);
    let new = make_pptx(&["Slide1", "Slide2"]);

    let changes = driver.diff(Some(&base), &new).unwrap();
    assert_eq!(changes.len(), 1);
    assert!(matches!(&changes[0], SemanticChange::Added { value, .. } if value == "Slide2"));
}

#[test]
fn pptx_diff_detects_removed_slide() {
    let driver = PptxDriver::new();
    let base = make_pptx(&["Slide1", "Slide2"]);
    let new = make_pptx(&["Slide1"]);

    let changes = driver.diff(Some(&base), &new).unwrap();
    assert_eq!(changes.len(), 1);
    assert!(
        matches!(&changes[0], SemanticChange::Removed { old_value, .. } if old_value == "Slide2")
    );
}

#[test]
fn pptx_diff_no_changes() {
    let driver = PptxDriver::new();
    let doc = make_pptx(&["A", "B"]);

    let changes = driver.diff(Some(&doc), &doc).unwrap();
    assert!(changes.is_empty());
}

#[test]
fn pptx_format_diff() {
    let driver = PptxDriver::new();
    let base = make_pptx(&["Old"]);
    let new = make_pptx(&["Old", "New"]);

    let output = driver.format_diff(Some(&base), &new).unwrap();
    assert!(output.contains("ADDED"), "format_diff should show ADDED");
}

#[test]
fn pptx_format_diff_no_changes() {
    let driver = PptxDriver::new();
    let doc = make_pptx(&["Same"]);

    let output = driver.format_diff(Some(&doc), &doc).unwrap();
    assert_eq!(output, "no changes");
}

#[test]
fn pptx_diff_new_file() {
    let driver = PptxDriver::new();
    let new = make_pptx(&["Title", "Content"]);

    let changes = driver.diff(None, &new).unwrap();
    assert_eq!(changes.len(), 2);
    assert!(changes
        .iter()
        .all(|c| matches!(c, SemanticChange::Added { .. })));
}

#[test]
fn pptx_merge_one_side_unchanged() {
    let driver = PptxDriver::new();
    let base = make_pptx(&["S1"]);
    let ours = make_pptx(&["S1", "S2"]);
    let theirs = make_pptx(&["S1"]);

    let result = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(
        result.is_some(),
        "merge with theirs unchanged should succeed"
    );
}

#[test]
fn pptx_merge_empty_base() {
    let driver = PptxDriver::new();
    let base = make_pptx(&[]);
    let ours = make_pptx(&["FromA"]);
    let theirs = make_pptx(&["FromB"]);

    let result = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(
        result.is_some(),
        "different additions to empty base should succeed (set-based merge)"
    );
}

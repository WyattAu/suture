use std::io::{Cursor, Write};
use suture_driver::{SemanticChange, SutureDriver};
use suture_driver_pptx::PptxDriver;

fn make_pptx(slide_names: &[&str]) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));

        let ct_overrides: String = slide_names
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

        let slide_rels: String = slide_names
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

        let sld_ids: String = slide_names
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

        for (i, name) in slide_names.iter().enumerate() {
            let slide_xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld><p:spTree>
    <p:sp><p:nvSpPr><p:cNvPr id="2" name="{}"/></p:nvSpPr><p:spPr/>
    </p:sp>
  </p:spTree></p:cSld>
</p:sld>"#,
                name
            );
            zip.start_file(
                format!("ppt/slides/slide{}.xml", i + 1),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(slide_xml.as_bytes()).unwrap();
        }

        zip.finish().unwrap();
    }
    buf
}

#[test]
fn pptx_two_editor_merge_different_slides_added() {
    let driver = PptxDriver::new();
    let base = make_pptx(&["Slide1"]);
    let ours = make_pptx(&["Slide1", "Slide2"]);
    let theirs = make_pptx(&["Slide1", "Slide3"]);

    let merged = driver.merge_raw(&base, &ours, &theirs).unwrap();
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

    let merged = driver.merge_raw(&base, &ours, &theirs).unwrap();
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

    let changes = driver.diff_raw(Some(&base), &new).unwrap();
    assert_eq!(changes.len(), 1);
    assert!(matches!(&changes[0], SemanticChange::Added { value, .. } if value == "Slide2"));
}

#[test]
fn pptx_diff_detects_removed_slide() {
    let driver = PptxDriver::new();
    let base = make_pptx(&["Slide1", "Slide2"]);
    let new = make_pptx(&["Slide1"]);

    let changes = driver.diff_raw(Some(&base), &new).unwrap();
    assert_eq!(changes.len(), 1);
    assert!(
        matches!(&changes[0], SemanticChange::Removed { old_value, .. } if old_value == "Slide2")
    );
}

#[test]
fn pptx_diff_no_changes() {
    let driver = PptxDriver::new();
    let doc = make_pptx(&["A", "B"]);

    let changes = driver.diff_raw(Some(&doc), &doc).unwrap();
    assert!(changes.is_empty());
}

#[test]
fn pptx_format_diff() {
    let driver = PptxDriver::new();
    let base = make_pptx(&["Old"]);
    let new = make_pptx(&["Old", "New"]);

    let changes = driver.diff_raw(Some(&base), &new).unwrap();
    assert!(
        changes
            .iter()
            .any(|c| matches!(c, SemanticChange::Added { .. })),
        "should detect added slide"
    );
}

#[test]
fn pptx_format_diff_no_changes() {
    let driver = PptxDriver::new();
    let doc = make_pptx(&["Same"]);

    let changes = driver.diff_raw(Some(&doc), &doc).unwrap();
    assert!(changes.is_empty());
}

#[test]
fn pptx_diff_new_file() {
    let driver = PptxDriver::new();
    let new = make_pptx(&["Title", "Content"]);

    let changes = driver.diff_raw(None, &new).unwrap();
    assert_eq!(changes.len(), 2);
    assert!(
        changes
            .iter()
            .all(|c| matches!(c, SemanticChange::Added { .. }))
    );
}

#[test]
fn pptx_merge_one_side_unchanged() {
    let driver = PptxDriver::new();
    let base = make_pptx(&["S1"]);
    let ours = make_pptx(&["S1", "S2"]);
    let theirs = make_pptx(&["S1"]);

    let result = driver.merge_raw(&base, &ours, &theirs).unwrap();
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

    let result = driver.merge_raw(&base, &ours, &theirs).unwrap();
    assert!(
        result.is_some(),
        "different additions to empty base should succeed (set-based merge)"
    );
}

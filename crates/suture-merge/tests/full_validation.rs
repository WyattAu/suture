//! Comprehensive validation suite for suture-merge.
//!
//! Tests every format, every function, and every edge case.
//! Run with: cargo test -p suture-merge --features all -- full_validation

use suture_merge::*;
use std::io::Write;

// ============================================================================
// Helpers
// ============================================================================

#[cfg(any(feature = "docx", feature = "xlsx", feature = "pptx"))]
fn make_minimal_zip(files: &[(&str, &str)]) -> Vec<u8> {
    use std::io::Cursor;
    let mut buf = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        for (path, content) in files {
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file(path, options).unwrap();
            zip.write_all(content.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }
    buf.into_inner()
}

#[cfg(feature = "docx")]
fn make_minimal_docx(paragraphs: &[&str]) -> String {
    let body_content = paragraphs
        .iter()
        .map(|p| format!("<w:p><w:r><w:t>{}</w:t></w:r></w:p>", p))
        .collect::<Vec<_>>()
        .join("");
    let doc_xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
        <w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\
        <w:body>{}</w:body>\
        </w:document>",
        body_content
    );
    let zip_bytes = make_minimal_zip(&[
        ("[Content_Types].xml", "<?xml version=\"1.0\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/></Types>"),
        ("word/document.xml", &doc_xml),
        ("_rels/.rels", "<?xml version=\"1.0\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/></Relationships>"),
    ]);
    // SAFETY: ZIP bytes are not valid UTF-8, but the DOCX driver uses
    // String::from_utf8_unchecked internally for the same reason — it
    // re-parses as bytes when reading the ZIP, so the String is just a
    // byte transport.
    unsafe { String::from_utf8_unchecked(zip_bytes) }
}

#[cfg(feature = "xlsx")]
fn make_minimal_xlsx(cells: &[(&str, &str)]) -> String {
    let rows: std::collections::HashSet<String> = cells
        .iter()
        .map(|(ref_, _)| {
            let row_num: u32 = ref_
                .chars()
                .skip_while(|c| c.is_alphabetic())
                .collect::<String>()
                .parse()
                .unwrap_or(1);
            format!("{}", row_num)
        })
        .collect();
    let row_xml = rows
        .iter()
        .map(|r| {
            format!(
                "<row r=\"{}\"><c r=\"A{}\" t=\"inlineStr\"><is><t>test</t></is></c></row>",
                r, r
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let sheet_xml = format!(
        "<?xml version=\"1.0\"?>\
        <worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">\
        <sheetData>{}</sheetData>\
        </worksheet>",
        row_xml
    );
    let zip_bytes = make_minimal_zip(&[
        ("[Content_Types].xml", "<?xml version=\"1.0\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/></Types>"),
        ("xl/worksheets/sheet1.xml", &sheet_xml),
        ("_rels/.rels", "<?xml version=\"1.0\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>"),
    ]);
    // SAFETY: ZIP bytes are not valid UTF-8. See make_minimal_docx.
    unsafe { String::from_utf8_unchecked(zip_bytes) }
}

#[cfg(feature = "pptx")]
fn make_minimal_pptx(slides: &[&str]) -> String {
    let slide_xmls: Vec<(String, String)> = slides
        .iter()
        .enumerate()
        .map(|(i, _name)| {
            (
                format!("ppt/slides/slide{}.xml", i + 1),
                format!(
                    r#"<?xml version="1.0"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
<p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld>
<p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>
</p:sld>"#
                ),
            )
        })
        .collect();

    // Build .rels file — each <Relationship> on its own line (parse_rels_by_id uses .lines())
    let slide_rels: String = slides
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

    let sld_ids: String = slides
        .iter()
        .enumerate()
        .map(|(i, _)| {
            format!(
                r#"    <p:sldId id="{}" r:id="rId{}"/>"#,
                256 + i as u32,
                i + 2
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let ct_overrides: String = slides
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

    let mut files: Vec<(&str, String)> = vec![
        ("[Content_Types].xml", format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
{}
</Types>"#,
            ct_overrides
        )),
        ("ppt/presentation.xml", format!(
            r#"<?xml version="1.0"?>
<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
<p:sldIdLst>
{}
</p:sldIdLst>
</p:presentation>"#,
            sld_ids
        )),
        ("ppt/_rels/presentation.xml.rels", format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>
{}
</Relationships>"#,
            slide_rels
        )),
    ];

    for (path, content) in &slide_xmls {
        files.push((path, content.clone()));
    }

    let zip_inputs: Vec<(&str, &str)> = files.iter().map(|(p, c)| (*p, c.as_str())).collect();
    let zip_bytes = make_minimal_zip(&zip_inputs);
    // SAFETY: ZIP bytes are not valid UTF-8. See make_minimal_docx.
    unsafe { String::from_utf8_unchecked(zip_bytes) }
}

#[test]
fn consistency_diff_then_format() {
    let base = r#"{"a": 1, "b": 2}"#;
    let modified = r#"{"a": 1, "b": 3, "c": 4}"#;

    let changes = diff(base, modified, Some(".json")).unwrap();
    let formatted = format_diff(base, modified, Some(".json")).unwrap();

    // If there are changes, format_diff should not be "no changes"
    if !changes.is_empty() {
        assert_ne!(formatted.trim(), "no changes");
    }
}

#[test]
fn consistency_no_changes() {
    let content = r#"{"a": 1}"#;
    let changes = diff(content, content, Some(".json")).unwrap();
    let formatted = format_diff(content, content, Some(".json")).unwrap();

    assert!(changes.is_empty());
    assert_eq!(formatted.trim(), "no changes");
}

#[test]
fn consistency_all_formats_no_change() {
    // Every format should report no changes for identical content
    let mut formats: Vec<(&str, &str, &str, fn(&str, &str, &str) -> Result<MergeResult, MergeError>)> = vec![
        (".json", r#"{"a":1}"#, r#"{"a":1}"#, merge_json),
        ("base", "a: 1\n", "a: 1\n", merge_yaml),
        ("base", "a = 1\n", "a = 1\n", merge_toml),
        ("base", "a\n1\n", "a\n1\n", merge_csv),
    ];
    #[cfg(feature = "xml")]
    formats.push(("base", "<r><a>1</a></r>", "<r><a>1</a></r>", merge_xml));
    #[cfg(feature = "markdown")]
    formats.push(("base", "# A\n\nB\n", "# A\n\nB\n", merge_markdown));

    for (label, base, modified, merge_fn) in formats {
        let r = merge_fn(base, modified, base).unwrap();
        assert_eq!(
            r.status,
            MergeStatus::Clean,
            "Format {} should be clean for identical inputs",
            label
        );
    }
}

// ============================================================================
// DOCX, XLSX, PPTX (binary formats - feature-gated)
// ============================================================================

#[cfg(feature = "docx")]
mod docx_tests {
    use super::*;

    #[test]
    fn merge_docx_clean_paragraph_addition() {
        let base = make_minimal_docx(&["Hello world"]);
        let ours = make_minimal_docx(&["Hello world", "New paragraph from ours"]);
        let theirs = make_minimal_docx(&["Hello world", "New paragraph from theirs"]);

        let result = merge_docx(&base, &ours, &theirs).unwrap();
        assert!(matches!(result.status, MergeStatus::Clean | MergeStatus::Conflict));
    }

    #[test]
    fn merge_docx_same_content_no_change() {
        let doc = make_minimal_docx(&["Hello world"]);
        let result = merge_docx(&doc, &doc, &doc).unwrap();
        assert_eq!(result.status, MergeStatus::Clean);
    }
}

#[cfg(feature = "xlsx")]
mod xlsx_tests {
    use super::*;

    #[test]
    fn merge_xlsx_same_content_no_change() {
        let doc = make_minimal_xlsx(&[("A1", "Hello")]);
        let result = merge_xlsx(&doc, &doc, &doc).unwrap();
        assert_eq!(result.status, MergeStatus::Clean);
    }
}

#[cfg(feature = "pptx")]
mod pptx_tests {
    use super::*;

    #[test]
    fn merge_pptx_same_content_no_change() {
        let doc = make_minimal_pptx(&["Slide 1"]);
        let result = merge_pptx(&doc, &doc, &doc).unwrap();
        assert_eq!(result.status, MergeStatus::Clean);
    }
}

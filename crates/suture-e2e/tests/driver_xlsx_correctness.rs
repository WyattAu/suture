use std::io::{Cursor, Write as IoWrite};
use suture_driver::{SemanticChange, SutureDriver};
use suture_driver_xlsx::XlsxDriver;

use std::fmt::Write;
type Cell = (usize, usize, String);

/// Convert 0-based column index to A1 column letter(s).
fn col_to_letter(col: usize) -> String {
    let mut result = String::new();
    let mut c = col;
    loop {
        result.insert(0, char::from(b'A' + (c % 26) as u8));
        c /= 26;
        if c == 0 {
            break;
        }
        c -= 1;
    }
    result
}

fn make_xlsx(sheets: &[(&str, &[Cell])]) -> String {
    let content_types = r#"<?xml version="1.0"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
</Types>"#;

    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
        zip.start_file(
            "[Content_Types].xml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(content_types.as_bytes()).unwrap();

        for (sheet_name, cells) in sheets {
            let mut xml = String::from(
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
                 <worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">",
            );
            let mut rows: std::collections::BTreeMap<usize, Vec<(usize, &String)>> =
                std::collections::BTreeMap::new();
            for &(row, col, ref val) in *cells {
                rows.entry(row).or_default().push((col, val));
            }
            for (row_num, cols) in &rows {
                let _ = write!(xml, "<row r=\"{}\">\n", row_num);
                for (col, val) in cols {
                    let col_letter = col_to_letter(*col);
                    let _ = write!(xml, 
                        "<c r=\"{}{}\"><v>{}</v></c>\n",
                        col_letter, row_num, val
                    );
                }
                xml.push_str("</row>\n");
            }
            xml.push_str("</worksheet>");

            let path = format!("xl/worksheets/{}.xml", sheet_name);
            zip.start_file(&path, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(xml.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }
    unsafe { String::from_utf8_unchecked(buf) }
}

#[test]
fn xlsx_two_editor_merge_different_cells() {
    let driver = XlsxDriver::new();
    let base = make_xlsx(&[("sheet1", &[(1, 0, "A".into()), (2, 0, "B".into())])]);
    let ours = make_xlsx(&[("sheet1", &[(1, 0, "X".into()), (2, 0, "B".into())])]);
    let theirs = make_xlsx(&[("sheet1", &[(1, 0, "A".into()), (2, 0, "Y".into())])]);

    let merged = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(merged.is_some(), "merge of different cells should succeed");
}

#[test]
fn xlsx_two_editor_merge_a_adds_b_edits() {
    let driver = XlsxDriver::new();
    let base = make_xlsx(&[("sheet1", &[(1, 0, "Old".into())])]);
    let ours = make_xlsx(&[("sheet1", &[(1, 0, "Old".into()), (2, 0, "New".into())])]);
    let theirs = make_xlsx(&[("sheet1", &[(1, 0, "Changed".into())])]);

    let merged = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(merged.is_some(), "merge with add + edit should succeed");
}

#[test]
fn xlsx_two_editor_conflict_same_cell() {
    let driver = XlsxDriver::new();
    let base = make_xlsx(&[("sheet1", &[(1, 0, "Original".into())])]);
    let ours = make_xlsx(&[("sheet1", &[(1, 0, "From A".into())])]);
    let theirs = make_xlsx(&[("sheet1", &[(1, 0, "From B".into())])]);

    let result = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(
        result.is_none(),
        "conflicting cell edits should return None"
    );
}

#[test]
fn xlsx_diff_detects_cell_changes() {
    let driver = XlsxDriver::new();
    let base = make_xlsx(&[("sheet1", &[(1, 0, "A".into()), (2, 0, "B".into())])]);
    let new = make_xlsx(&[(
        "sheet1",
        &[(1, 0, "X".into()), (2, 0, "B".into()), (3, 0, "C".into())],
    )]);

    let changes = driver.diff(Some(&base), &new).unwrap();
    let modified = changes
        .iter()
        .filter(|c| matches!(c, SemanticChange::Modified { .. }))
        .count();
    let added = changes
        .iter()
        .filter(|c| matches!(c, SemanticChange::Added { .. }))
        .count();

    assert_eq!(modified, 1, "should detect one modified cell");
    assert_eq!(added, 1, "should detect one added cell");
}

#[test]
fn xlsx_diff_removed_cell() {
    let driver = XlsxDriver::new();
    let base = make_xlsx(&[("sheet1", &[(1, 0, "A".into()), (2, 0, "B".into())])]);
    let new = make_xlsx(&[("sheet1", &[(1, 0, "A".into())])]);

    let changes = driver.diff(Some(&base), &new).unwrap();
    assert!(
        changes
            .iter()
            .any(|c| matches!(c, SemanticChange::Removed { .. })),
        "should detect removed cell"
    );
}

#[test]
fn xlsx_format_diff() {
    let driver = XlsxDriver::new();
    let base = make_xlsx(&[("sheet1", &[(1, 0, "10".into())])]);
    let new = make_xlsx(&[("sheet1", &[(1, 0, "20".into())])]);

    let output = driver.format_diff(Some(&base), &new).unwrap();
    assert!(
        output.contains("MODIFIED"),
        "format_diff should show MODIFIED"
    );
}

#[test]
fn xlsx_format_diff_no_changes() {
    let driver = XlsxDriver::new();
    let doc = make_xlsx(&[("sheet1", &[(1, 0, "A".into())])]);

    let output = driver.format_diff(Some(&doc), &doc).unwrap();
    assert_eq!(output, "no changes");
}

#[test]
fn xlsx_merge_one_side_unchanged() {
    let driver = XlsxDriver::new();
    let base = make_xlsx(&[("sheet1", &[(1, 0, "A".into())])]);
    let ours = make_xlsx(&[("sheet1", &[(1, 0, "A".into())])]);
    let theirs = make_xlsx(&[("sheet1", &[(1, 0, "B".into())])]);

    let result = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(
        result.is_some(),
        "merge with ours unchanged should return theirs"
    );
}

#[test]
fn xlsx_diff_new_file() {
    let driver = XlsxDriver::new();
    let new = make_xlsx(&[("sheet1", &[(1, 0, "Val".into())])]);

    let changes = driver.diff(None, &new).unwrap();
    assert!(
        changes
            .iter()
            .all(|c| matches!(c, SemanticChange::Added { .. })),
        "diff from None should show all cells as Added"
    );
}

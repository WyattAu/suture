use suture_driver::{SemanticChange, SutureDriver};
use suture_driver_pdf::PdfDriver;

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

#[test]
fn pdf_text_extraction_accuracy() {
    let driver = PdfDriver::new();
    let pdf = make_minimal_pdf(&["Hello World", "Second Page"]);

    let changes = driver.diff_raw(None, &pdf).unwrap();
    assert_eq!(changes.len(), 2, "should extract text from both pages");

    assert!(
        matches!(&changes[0], SemanticChange::Added { value, .. } if value == "Hello World"),
        "first page text should be extracted accurately"
    );
    assert!(
        matches!(&changes[1], SemanticChange::Added { value, .. } if value == "Second Page"),
        "second page text should be extracted accurately"
    );
}

#[test]
fn pdf_page_level_diff_modifications() {
    let driver = PdfDriver::new();
    let base = make_minimal_pdf(&["Page One", "Page Two", "Page Three"]);
    let new = make_minimal_pdf(&["Page One", "Modified Two", "Page Three"]);

    let changes = driver.diff_raw(Some(&base), &new).unwrap();
    assert_eq!(changes.len(), 1, "should detect one modified page");
    assert!(
        matches!(&changes[0], SemanticChange::Modified { old_value, new_value, .. }
            if old_value == "Page Two" && new_value == "Modified Two"),
        "should correctly identify the modified page"
    );
}

#[test]
fn pdf_page_level_diff_addition() {
    let driver = PdfDriver::new();
    let base = make_minimal_pdf(&["Existing"]);
    let new = make_minimal_pdf(&["Existing", "New Page"]);

    let changes = driver.diff_raw(Some(&base), &new).unwrap();
    assert!(
        changes
            .iter()
            .any(|c| matches!(c, SemanticChange::Added { value, .. } if value == "New Page")),
        "should detect added page"
    );
}

#[test]
fn pdf_page_level_diff_removal() {
    let driver = PdfDriver::new();
    let base = make_minimal_pdf(&["Keep", "Remove Me"]);
    let new = make_minimal_pdf(&["Keep"]);

    let changes = driver.diff_raw(Some(&base), &new).unwrap();
    assert!(
        changes.iter().any(
            |c| matches!(c, SemanticChange::Removed { old_value, .. } if old_value == "Remove Me")
        ),
        "should detect removed page"
    );
}

#[test]
fn pdf_two_editor_merge_different_pages() {
    let driver = PdfDriver::new();
    let base = make_minimal_pdf(&["A", "B", "C"]);
    let ours = make_minimal_pdf(&["A Modified", "B", "C"]);
    let theirs = make_minimal_pdf(&["A", "B", "C Modified"]);

    let result = driver.merge_raw(&base, &ours, &theirs).unwrap();
    assert!(
        result.is_some(),
        "merge with changes to different pages should succeed"
    );
}

#[test]
fn pdf_two_editor_conflict_same_page() {
    let driver = PdfDriver::new();
    let base = make_minimal_pdf(&["Shared"]);
    let ours = make_minimal_pdf(&["Changed by A"]);
    let theirs = make_minimal_pdf(&["Changed by B"]);

    let result = driver.merge_raw(&base, &ours, &theirs).unwrap();
    assert!(
        result.is_none(),
        "conflicting changes to the same page should return None"
    );
}

#[test]
fn pdf_two_editor_a_adds_b_edits() {
    let driver = PdfDriver::new();
    let base = make_minimal_pdf(&["Hello"]);
    let ours = make_minimal_pdf(&["Hello", "New Page"]);
    let theirs = make_minimal_pdf(&["Hello Modified"]);

    let result = driver.merge_raw(&base, &ours, &theirs).unwrap();
    assert!(
        result.is_some(),
        "merge with one side adding a page and the other editing should succeed"
    );
}

#[test]
fn pdf_format_diff() {
    let driver = PdfDriver::new();
    let base = make_minimal_pdf(&["Old text"]);
    let new = make_minimal_pdf(&["New text"]);

    let changes = driver.diff_raw(Some(&base), &new).unwrap();
    assert!(
        changes.iter().any(|c| matches!(c, SemanticChange::Modified { old_value, new_value, .. } if old_value == "Old text" && new_value == "New text")),
        "should detect modified page"
    );
}

#[test]
fn pdf_format_diff_no_changes() {
    let driver = PdfDriver::new();
    let pdf = make_minimal_pdf(&["Same"]);

    let changes = driver.diff_raw(Some(&pdf), &pdf).unwrap();
    assert!(changes.is_empty());
}

#[test]
fn pdf_diff_empty_pages() {
    let driver = PdfDriver::new();
    let base = make_minimal_pdf(&[""]);
    let new = make_minimal_pdf(&[""]);

    let changes = driver.diff_raw(Some(&base), &new).unwrap();
    assert!(
        changes.is_empty(),
        "identical empty pages should produce no changes"
    );
}

#[test]
fn pdf_merge_one_side_unchanged() {
    let driver = PdfDriver::new();
    let base = make_minimal_pdf(&["Original"]);
    let ours = make_minimal_pdf(&["Original"]);
    let theirs = make_minimal_pdf(&["Changed"]);

    let result = driver.merge_raw(&base, &ours, &theirs).unwrap();
    assert!(
        result.is_some(),
        "merge with one side unchanged should succeed"
    );
}

#[test]
fn pdf_merge_multiple_page_changes() {
    let driver = PdfDriver::new();
    let base = make_minimal_pdf(&["P1", "P2", "P3", "P4"]);
    let ours = make_minimal_pdf(&["P1 Edited", "P2", "P3", "P4"]);
    let theirs = make_minimal_pdf(&["P1", "P2", "P3 Edited", "P4"]);

    let result = driver.merge_raw(&base, &ours, &theirs).unwrap();
    assert!(
        result.is_some(),
        "merge with non-overlapping multi-page edits should succeed"
    );
}

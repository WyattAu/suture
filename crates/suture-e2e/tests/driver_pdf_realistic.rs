use suture_driver::{SemanticChange, SutureDriver};
use suture_driver_pdf::PdfDriver;
use suture_e2e::fixtures::pdf;

#[test]
fn pdf_realistic_simple_parse_and_diff() {
    let driver = PdfDriver::new();
    let doc = pdf::simple();

    let changes = driver.diff_raw(None, &doc).unwrap();
    assert_eq!(changes.len(), 1, "simple pdf should have 1 page");
    assert!(
        matches!(&changes[0], SemanticChange::Added { value, .. } if value.contains("Hello")),
        "should extract page text"
    );
}

#[test]
fn pdf_realistic_simple_modify_and_diff() {
    let driver = PdfDriver::new();
    let base = pdf::simple();
    let modified = pdf::with_modified_page(pdf::SIMPLE_PAGES, 0, "MODIFIED: Hello, World!");

    let changes = driver.diff_raw(Some(&base), &modified).unwrap();
    assert_eq!(changes.len(), 1);
    assert!(
        matches!(&changes[0], SemanticChange::Modified { old_value, new_value, .. }
            if old_value.contains("Hello") && new_value.contains("MODIFIED")),
        "should detect page modification"
    );
}

#[test]
fn pdf_realistic_simple_merge_different_pages() {
    let driver = PdfDriver::new();
    let base = pdf::simple();
    let ours = pdf::with_modified_page(pdf::SIMPLE_PAGES, 0, "CHANGED BY A");
    let theirs = pdf::with_modified_page(pdf::SIMPLE_PAGES, 0, "CHANGED BY B");

    let result = driver.merge_raw(&base, &ours, &theirs).unwrap();
    assert!(result.is_none(), "same page edits should conflict");
}

#[test]
fn pdf_realistic_multi_page_parse() {
    let driver = PdfDriver::new();
    let doc = pdf::multi_page();

    let changes = driver.diff_raw(None, &doc).unwrap();
    assert!(
        changes.len() >= 12,
        "multi-page pdf should have at least 12 pages, got {}",
        changes.len()
    );
}

#[test]
fn pdf_realistic_multi_page_merge_different_pages() {
    let driver = PdfDriver::new();
    let base = pdf::multi_page();
    let ours = pdf::with_modified_page(pdf::MULTI_PAGE_PAGES, 0, "UPDATED TABLE OF CONTENTS");
    let theirs = pdf::with_modified_page(
        pdf::MULTI_PAGE_PAGES,
        4,
        "UPDATED: Security best practices.",
    );

    let merged = driver.merge_raw(&base, &ours, &theirs).unwrap();
    assert!(
        merged.is_some(),
        "multi-page: changes to different pages should merge"
    );
}

#[test]
fn pdf_realistic_multi_page_conflict_same_page() {
    let driver = PdfDriver::new();
    let base = pdf::multi_page();
    let ours = pdf::with_modified_page(pdf::MULTI_PAGE_PAGES, 2, "CHANGED BY EDITOR A");
    let theirs = pdf::with_modified_page(pdf::MULTI_PAGE_PAGES, 2, "CHANGED BY EDITOR B");

    let result = driver.merge_raw(&base, &ours, &theirs).unwrap();
    assert!(result.is_none(), "same page conflict should be detected");
}

#[test]
fn pdf_realistic_complex_parse() {
    let driver = PdfDriver::new();
    let doc = pdf::complex();

    let changes = driver.diff_raw(None, &doc).unwrap();
    assert!(
        changes.len() >= 8,
        "complex pdf should have at least 8 pages, got {}",
        changes.len()
    );
}

#[test]
fn pdf_realistic_complex_merge_sections() {
    let driver = PdfDriver::new();
    let base = pdf::complex();
    let ours = pdf::with_modified_page(pdf::COMPLEX_PAGES, 1, "UPDATED: Financial Highlights");
    let theirs = pdf::with_modified_page(pdf::COMPLEX_PAGES, 6, "UPDATED: Human Resources");

    let merged = driver.merge_raw(&base, &ours, &theirs).unwrap();
    assert!(
        merged.is_some(),
        "complex: changes to different sections should merge"
    );
}

#[test]
fn pdf_realistic_format_diff() {
    let driver = PdfDriver::new();
    let base = pdf::simple();
    let modified = pdf::with_modified_page(pdf::SIMPLE_PAGES, 0, "NEW TEXT");

    let changes = driver.diff_raw(Some(&base), &modified).unwrap();
    assert!(
        changes
            .iter()
            .any(|c| matches!(c, SemanticChange::Modified { .. })),
        "diff_raw should detect modifications"
    );
    assert!(
        changes.iter().any(|c| matches!(c, SemanticChange::Modified { old_value, .. } if old_value.contains("Hello"))),
        "should show old value with Hello"
    );
    assert!(
        changes.iter().any(|c| matches!(c, SemanticChange::Modified { new_value, .. } if new_value.contains("NEW TEXT"))),
        "should show new value with NEW TEXT"
    );
}

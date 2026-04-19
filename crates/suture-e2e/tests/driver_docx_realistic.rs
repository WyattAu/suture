use suture_driver::{SemanticChange, SutureDriver};
use suture_driver_docx::DocxDriver;
use suture_e2e::fixtures::docx;

#[test]
fn docx_realistic_simple_parse_and_diff() {
    let driver = DocxDriver::new();
    let doc = docx::simple();

    let changes = driver.diff(None, &doc).unwrap();
    assert_eq!(changes.len(), 1, "simple doc should have 1 paragraph");
    assert!(
        matches!(&changes[0], SemanticChange::Added { value, .. } if value.contains("simple document")),
        "should extract paragraph text"
    );
}

#[test]
fn docx_realistic_simple_modify_and_diff() {
    let driver = DocxDriver::new();
    let base = docx::simple();
    let modified = docx::with_modified_paragraph(
        docx::SIMPLE_PARAGRAPHS,
        0,
        "This is an UPDATED simple document.",
    );

    let changes = driver.diff(Some(&base), &modified).unwrap();
    assert_eq!(changes.len(), 1);
    assert!(
        matches!(&changes[0], SemanticChange::Modified { old_value, new_value, .. }
            if old_value.contains("simple document") && new_value.contains("UPDATED")),
        "should detect modification"
    );
}

#[test]
fn docx_realistic_multi_section_merge_different_paragraphs() {
    let driver = DocxDriver::new();
    let base = docx::multi_section();
    let ours = docx::with_modified_paragraph(
        docx::MULTI_SECTION_PARAGRAPHS,
        0,
        "MODIFIED TITLE: Quarterly Report Q4 2025",
    );
    let theirs = docx::with_modified_paragraph(
        docx::MULTI_SECTION_PARAGRAPHS,
        7,
        "We plan to expand into ASIA in Q1 2026.",
    );

    let merged = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(
        merged.is_some(),
        "changes to different paragraphs should merge"
    );

    let merged_str = merged.unwrap();
    let diff = driver.diff(Some(&base), &merged_str).unwrap();
    assert!(
        diff.iter().any(|c| matches!(c, SemanticChange::Modified { new_value, .. } if new_value.contains("MODIFIED TITLE"))),
        "should preserve editor A change"
    );
    assert!(
        diff.iter().any(|c| matches!(c, SemanticChange::Modified { new_value, .. } if new_value.contains("ASIA"))),
        "should preserve editor B change"
    );
}

#[test]
fn docx_realistic_multi_section_conflict_same_paragraph() {
    let driver = DocxDriver::new();
    let base = docx::multi_section();
    let ours = docx::with_modified_paragraph(
        docx::MULTI_SECTION_PARAGRAPHS,
        2,
        "CHANGED BY EDITOR A: Revenue increased by 50%.",
    );
    let theirs = docx::with_modified_paragraph(
        docx::MULTI_SECTION_PARAGRAPHS,
        2,
        "CHANGED BY EDITOR B: Revenue increased by 30%.",
    );

    let result = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(result.is_none(), "same paragraph edits should conflict");
}

#[test]
fn docx_realistic_styled_preserves_structure() {
    let driver = DocxDriver::new();
    let doc = docx::styled();

    let changes = driver.diff(None, &doc).unwrap();
    assert!(
        changes.len() >= 10,
        "styled doc should have at least 10 paragraphs, got {}",
        changes.len()
    );
    assert!(
        changes.iter().any(
            |c| matches!(c, SemanticChange::Added { value, .. } if value.contains("BRANDED REPORT"))
        ),
        "should detect branded title"
    );
}

#[test]
fn docx_realistic_complex_legal_document_merge() {
    let driver = DocxDriver::new();
    let base = docx::complex();
    let ours = docx::with_modified_paragraph(
        docx::COMPLEX_PARAGRAPHS,
        3,
        "1.1 The Company shall provide STRATEGIC consulting services.",
    );
    let theirs = docx::with_modified_paragraph(
        docx::COMPLEX_PARAGRAPHS,
        9,
        "3.1 Client shall pay Company a monthly retainer of $20,000.",
    );

    let merged = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(
        merged.is_some(),
        "legal doc: changes to different articles should merge"
    );
}

#[test]
fn docx_realistic_complex_legal_conflict() {
    let driver = DocxDriver::new();
    let base = docx::complex();
    let ours = docx::with_modified_paragraph(
        docx::COMPLEX_PARAGRAPHS,
        15,
        "5.1 Total liability shall not exceed $1,000,000.",
    );
    let theirs = docx::with_modified_paragraph(
        docx::COMPLEX_PARAGRAPHS,
        15,
        "5.1 Total liability shall not exceed $500,000.",
    );

    let result = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(
        result.is_none(),
        "legal doc: conflicting clause edits should conflict"
    );
}

#[test]
fn docx_realistic_long_document_performance() {
    let driver = DocxDriver::new();
    let doc = docx::long();

    let changes = driver.diff(None, &doc).unwrap();
    assert!(
        changes.len() >= 50,
        "long doc should have at least 50 paragraphs, got {}",
        changes.len()
    );

    let paras = docx::long_paragraphs();
    let modified = docx::with_modified_paragraph(
        &paras,
        0,
        "UPDATED: THE COMPREHENSIVE GUIDE TO MODERN SOFTWARE ARCHITECTURE - V2",
    );
    let diff = driver.diff(Some(&doc), &modified).unwrap();
    assert_eq!(
        diff.len(),
        1,
        "should detect single modification in long doc"
    );
}

#[test]
fn docx_realistic_long_merge_across_chapters() {
    let driver = DocxDriver::new();
    let base = docx::long();
    let paras = docx::long_paragraphs();
    let ours = docx::with_modified_paragraph(&paras, 3, "Section 1.1: UPDATED Foundation Concepts");
    let theirs =
        docx::with_modified_paragraph(&paras, 25, "Section 2.6: UPDATED Microservice Patterns");

    let merged = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(
        merged.is_some(),
        "long doc: changes in different chapters should merge"
    );
}

#[test]
fn docx_realistic_format_diff_multi_section() {
    let driver = DocxDriver::new();
    let base = docx::multi_section();
    let modified = docx::with_modified_paragraph(
        docx::MULTI_SECTION_PARAGRAPHS,
        4,
        "Total revenue reached $5.0 million, exceeding our target.",
    );

    let output = driver.format_diff(Some(&base), &modified).unwrap();
    assert!(output.contains("MODIFIED"));
}

#[test]
fn test_docx_two_editor_paragraph_conflict() {
    let driver = DocxDriver::new();
    let five_paras: &[&str] = &[
        "Introduction to the project",
        "Background and context",
        "Key findings from research",
        "Analysis and discussion",
        "Conclusions and next steps",
    ];
    let base = docx::with_modified_paragraph(five_paras, 0, five_paras[0]);
    assert!(base.starts_with("PK"), "base should be valid DOCX (ZIP)");

    let ours = docx::with_modified_paragraph(
        five_paras,
        2,
        "EDITOR A: Research findings have been comprehensively updated.",
    );
    let theirs = docx::with_modified_paragraph(
        five_paras,
        2,
        "EDITOR B: Research findings require further review.",
    );

    let result = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(
        result.is_none(),
        "both editors modifying the same paragraph should conflict"
    );
}

#[test]
fn test_docx_table_insertion_merge() {
    let driver = DocxDriver::new();
    let base = docx::multi_section();
    let ours = docx::with_modified_paragraph(
        docx::MULTI_SECTION_PARAGRAPHS,
        2,
        "[TABLE INSERTED BY EDITOR A]\nRevenue | Q1 | Q2 | Q3 | Q4\n$4.2M | $1.0M | $1.1M | $1.0M | $1.1M",
    );
    let theirs = docx::with_modified_paragraph(
        docx::MULTI_SECTION_PARAGRAPHS,
        4,
        "Total revenue reached $5.0 million, exceeding our target by 31%.",
    );

    let merged = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(
        merged.is_some(),
        "table insertion + text modification should merge cleanly"
    );

    let merged_str = merged.unwrap();
    assert!(
        merged_str.starts_with("PK"),
        "merged output should be valid DOCX (ZIP magic bytes)"
    );

    let diff = driver.diff(Some(&base), &merged_str).unwrap();
    assert!(
        diff.iter().any(|c| matches!(c, SemanticChange::Modified { new_value, .. } if new_value.contains("[TABLE INSERTED BY EDITOR A]"))),
        "should preserve editor A table insertion"
    );
    assert!(
        diff.iter().any(|c| matches!(c, SemanticChange::Modified { new_value, .. } if new_value.contains("$5.0 million"))),
        "should preserve editor B text modification"
    );
}

#[test]
fn test_docx_large_document_stress() {
    let driver = DocxDriver::new();
    let base = docx::long();
    let paras = docx::long_paragraphs();

    let changes = driver.diff(None, &base).unwrap();
    assert!(
        changes.len() >= 50,
        "large doc should have at least 50 paragraphs, got {}",
        changes.len()
    );

    let ours = docx::with_modified_paragraph(&paras, 5, "STRESS: Modified section 1.3");
    let theirs = docx::with_modified_paragraph(&paras, 30, "STRESS: Modified section 2.6");
    let another = docx::with_modified_paragraph(&paras, 45, "STRESS: Modified section 3.8");

    let _ = driver.diff(Some(&base), &ours).unwrap();
    let _ = driver.diff(Some(&base), &theirs).unwrap();
    let _ = driver.diff(Some(&base), &another).unwrap();

    let merge_result = driver.merge(&base, &ours, &theirs);
    assert!(
        merge_result.is_ok(),
        "large document merge should not error"
    );
    assert!(
        merge_result.unwrap().is_some(),
        "large doc: non-overlapping changes in different chapters should merge"
    );
}

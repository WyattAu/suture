use suture_driver::{SemanticChange, SutureDriver};
use suture_driver_pptx::PptxDriver;
use suture_e2e::fixtures::pptx;

#[test]
fn pptx_realistic_simple_parse_and_diff() {
    let driver = PptxDriver::new();
    let doc = pptx::simple();

    let changes = driver.diff(None, &doc).unwrap();
    assert_eq!(changes.len(), 3, "simple pptx should have 3 slides");
    assert!(
        changes
            .iter()
            .any(|c| matches!(c, SemanticChange::Added { value, .. } if value == "Title Slide")),
        "should detect title slide"
    );
}

#[test]
fn pptx_realistic_simple_add_slide_merge() {
    let driver = PptxDriver::new();
    let base = pptx::simple();

    let ours = pptx::make_from_slides(&[
        "Title Slide",
        "Content Slide",
        "Summary Slide",
        "New Appendix",
    ]);
    let theirs = pptx::make_from_slides(&[
        "Title Slide",
        "Content Slide",
        "Summary Slide",
        "References",
    ]);

    let merged = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(merged.is_some(), "adding different slides should merge");
    assert!(
        !merged.unwrap().is_empty(),
        "merged result should not be empty"
    );
}

#[test]
fn pptx_realistic_simple_conflict_same_slide_removed() {
    let driver = PptxDriver::new();
    let base = pptx::simple();

    let ours = pptx::make_from_slides(&["Title Slide", "Content Slide"]);
    let theirs = pptx::make_from_slides(&["Title Slide", "Content Slide"]);

    let merged = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(merged.is_some(), "both removing same slide should merge");
}

#[test]
fn pptx_realistic_multi_layout_parse() {
    let driver = PptxDriver::new();
    let doc = pptx::multi_layout();

    let changes = driver.diff(None, &doc).unwrap();
    assert!(
        changes.len() >= 12,
        "multi-layout pptx should have at least 12 slides, got {}",
        changes.len()
    );
}

#[test]
fn pptx_realistic_multi_layout_merge_different_adds() {
    let driver = PptxDriver::new();
    let base = pptx::multi_layout();

    let ours = pptx::make_from_slides(&[
        "Title",
        "Agenda",
        "Introduction",
        "Market Analysis",
        "Competitive Landscape",
        "Product Overview",
        "Technical Architecture",
        "Demo",
        "Pricing",
        "Customer Testimonials",
        "Roadmap",
        "Q&A",
        "ADDED BY A: Technical Deep Dive",
    ]);
    let theirs = pptx::make_from_slides(&[
        "Title",
        "Agenda",
        "Introduction",
        "Market Analysis",
        "Competitive Landscape",
        "Product Overview",
        "Technical Architecture",
        "Demo",
        "Pricing",
        "Customer Testimonials",
        "Roadmap",
        "Q&A",
        "ADDED BY B: Security Overview",
    ]);

    let merged = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(
        merged.is_some(),
        "multi-layout: different slide additions should merge"
    );
}

#[test]
fn pptx_realistic_styled_merge_add_and_unchanged() {
    let driver = PptxDriver::new();
    let base = pptx::styled();

    let ours = pptx::make_from_slides(&[
        "Acme Corp Annual Report 2025",
        "Executive Summary",
        "Financial Performance",
        "Product Roadmap",
        "Team Growth",
        "Customer Metrics",
        "Strategic Priorities 2026",
        "ADDED: Investor FAQ",
    ]);
    let theirs = pptx::make_from_slides(&[
        "Acme Corp Annual Report 2025",
        "Executive Summary",
        "Financial Performance",
        "Product Roadmap",
        "Team Growth",
        "Customer Metrics",
        "Strategic Priorities 2026",
    ]);

    let merged = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(merged.is_some(), "styled: add + unchanged should merge");
}

#[test]
fn pptx_realistic_styled_diff_detects_changes() {
    let driver = PptxDriver::new();
    let base = pptx::styled();
    let modified = pptx::make_from_slides(&[
        "Acme Corp Annual Report 2025",
        "Executive Summary",
        "Financial Performance",
        "Product Roadmap",
        "Team Growth",
        "Customer Metrics",
        "Strategic Priorities 2026",
        "New Slide",
    ]);

    let changes = driver.diff(Some(&base), &modified).unwrap();
    assert!(
        changes.iter().any(
            |c| matches!(c, SemanticChange::Removed { old_value, .. } if old_value == "Thank You")
        ),
        "should detect removed Thank You slide"
    );
    assert!(
        changes
            .iter()
            .any(|c| matches!(c, SemanticChange::Added { value, .. } if value == "New Slide")),
        "should detect added New Slide"
    );
}

#[test]
fn pptx_realistic_complex_project_parse() {
    let driver = PptxDriver::new();
    let doc = pptx::complex();

    let changes = driver.diff(None, &doc).unwrap();
    assert!(
        changes.len() >= 15,
        "complex pptx should have at least 15 slides, got {}",
        changes.len()
    );
}

#[test]
fn pptx_realistic_complex_merge_add_and_remove() {
    let driver = PptxDriver::new();
    let base = pptx::complex();

    let ours = pptx::make_from_slides(&[
        "Project Phoenix - Kickoff",
        "Agenda",
        "ADDED: Stakeholder Map",
        "Objectives and KPIs",
        "Team Structure",
    ]);
    let theirs = pptx::make_from_slides(&[
        "Project Phoenix - Kickoff",
        "Agenda",
        "Project Background",
        "Objectives and KPIs",
        "Team Structure",
    ]);

    let merged = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(merged.is_some(), "complex: add + remove should merge");
}

#[test]
fn pptx_realistic_format_diff() {
    let driver = PptxDriver::new();
    let base = pptx::simple();
    let new = pptx::make_from_slides(&[
        "Title Slide",
        "Content Slide",
        "Summary Slide",
        "Bonus Slide",
    ]);

    let output = driver.format_diff(Some(&base), &new).unwrap();
    assert!(output.contains("ADDED"), "format_diff should show ADDED");
}

#[test]
fn test_pptx_slide_reorder_merge() {
    let driver = PptxDriver::new();
    let base = pptx::make_from_slides(&["S1", "S2", "S3", "S4", "S5"]);
    let ours = pptx::make_from_slides(&["S1", "S3", "S2", "S4", "S5"]);
    let theirs = pptx::make_from_slides(&["S1", "S2", "S3", "S4 Updated", "S5"]);

    let merged = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(
        merged.is_some(),
        "slide reorder + non-overlapping text edit should produce a merge result"
    );

    let merged_str = merged.unwrap();
    assert!(
        merged_str.starts_with("PK"),
        "merged output should be valid PPTX (ZIP magic bytes)"
    );
}

#[test]
fn test_pptx_multi_slide_edit_merge() {
    let driver = PptxDriver::new();
    let base = pptx::complex();

    let base_names: &[&str] = &[
        "Project Phoenix - Kickoff",
        "Agenda",
        "Project Background",
        "Objectives and KPIs",
        "Team Structure",
        "Timeline Overview",
        "Technical Approach",
        "Risk Assessment",
        "Budget Allocation",
        "Milestone 1: Discovery",
        "Milestone 2: Design",
        "Milestone 3: Implementation",
        "Milestone 4: Testing",
        "Milestone 5: Launch",
        "Appendix: Technical Specs",
    ];

    let mut ours_names: Vec<String> = base_names.iter().map(|s| s.to_string()).collect();
    ours_names[1] = "Agenda - UPDATED BY A".to_string();
    ours_names[4] = "Team Structure - UPDATED BY A".to_string();
    ours_names[7] = "Risk Assessment - UPDATED BY A".to_string();

    let mut theirs_names: Vec<String> = base_names.iter().map(|s| s.to_string()).collect();
    theirs_names[2] = "Project Background - UPDATED BY B".to_string();
    theirs_names[5] = "Timeline Overview - UPDATED BY B".to_string();
    theirs_names[8] = "Budget Allocation - UPDATED BY B".to_string();

    let ours = pptx::make_from_slides(&ours_names);
    let theirs = pptx::make_from_slides(&theirs_names);

    let merged = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(
        merged.is_some(),
        "non-overlapping multi-slide edits should merge"
    );

    let merged_str = merged.unwrap();
    assert!(
        merged_str.starts_with("PK"),
        "merged output should be valid PPTX (ZIP magic bytes)"
    );
}

#[test]
fn test_pptx_large_deck_stress() {
    let driver = PptxDriver::new();
    let slides: Vec<String> = (1..=30).map(|i| format!("Slide {i}")).collect();
    let doc = pptx::make_from_slides(&slides);

    let changes = driver.diff(None, &doc).unwrap();
    assert!(
        changes.len() >= 30,
        "30-slide deck should have at least 30 slides, got {}",
        changes.len()
    );
}

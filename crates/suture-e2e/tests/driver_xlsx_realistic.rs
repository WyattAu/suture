use suture_driver::{SemanticChange, SutureDriver};
use suture_driver_xlsx::XlsxDriver;
use suture_e2e::fixtures::xlsx;

#[test]
fn xlsx_realistic_simple_parse_and_diff() {
    let driver = XlsxDriver::new();
    let base = xlsx::simple();

    let changes = driver.diff(None, &base).unwrap();
    assert!(
        changes.len() >= 4,
        "simple xlsx should have at least 4 cells, got {}",
        changes.len()
    );
    assert!(
        changes
            .iter()
            .all(|c| matches!(c, SemanticChange::Added { .. })),
        "all cells should be Added for new file"
    );
}

#[test]
fn xlsx_realistic_simple_modify_and_diff() {
    let driver = XlsxDriver::new();
    let base = xlsx::simple();
    let modified = xlsx::with_modified_cell(&xlsx::simple_sheets(), 0, 2, 0, "MODIFIED Alpha");

    let changes = driver.diff(Some(&base), &modified).unwrap();
    assert!(
        changes.iter().any(|c| matches!(c, SemanticChange::Modified { new_value, .. } if new_value == "MODIFIED Alpha")),
        "should detect cell modification"
    );
}

#[test]
fn xlsx_realistic_simple_merge_different_cells() {
    let driver = XlsxDriver::new();
    let base = xlsx::simple();
    let ours = xlsx::with_modified_cell(&xlsx::simple_sheets(), 0, 2, 1, "999");
    let theirs = xlsx::with_modified_cell(&xlsx::simple_sheets(), 0, 3, 1, "300");

    let merged = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(merged.is_some(), "changes to different cells should merge");
}

#[test]
fn xlsx_realistic_simple_conflict_same_cell() {
    let driver = XlsxDriver::new();
    let base = xlsx::simple();
    let ours = xlsx::with_modified_cell(&xlsx::simple_sheets(), 0, 2, 1, "999");
    let theirs = xlsx::with_modified_cell(&xlsx::simple_sheets(), 0, 2, 1, "888");

    let result = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(result.is_none(), "conflicting cell edits should conflict");
}

#[test]
fn xlsx_realistic_multi_sheet_structure_preserved() {
    let driver = XlsxDriver::new();
    let doc = xlsx::multi_sheet();

    let changes = driver.diff(None, &doc).unwrap();
    let sales_cells: Vec<_> = changes
        .iter()
        .filter(|c| matches!(c, SemanticChange::Added { path, .. } if path.starts_with("/Sales")))
        .collect();
    let emp_cells: Vec<_> = changes
        .iter()
        .filter(
            |c| matches!(c, SemanticChange::Added { path, .. } if path.starts_with("/Employees")),
        )
        .collect();
    let prod_cells: Vec<_> = changes
        .iter()
        .filter(
            |c| matches!(c, SemanticChange::Added { path, .. } if path.starts_with("/Products")),
        )
        .collect();

    assert!(
        sales_cells.len() >= 20,
        "Sales sheet should have many cells, got {}",
        sales_cells.len()
    );
    assert!(
        emp_cells.len() >= 10,
        "Employees sheet should have many cells, got {}",
        emp_cells.len()
    );
    assert!(
        prod_cells.len() >= 5,
        "Products sheet should have many cells, got {}",
        prod_cells.len()
    );
}

#[test]
fn xlsx_realistic_multi_sheet_merge_different_sheets() {
    let driver = XlsxDriver::new();
    let base = xlsx::multi_sheet();
    let ours = xlsx::with_modified_cell(&xlsx::multi_sheet_sheets(), 0, 2, 1, "99999");
    let theirs = xlsx::with_modified_cell(&xlsx::multi_sheet_sheets(), 1, 3, 3, "200000");

    let merged = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(merged.is_some(), "changes to different sheets should merge");
}

#[test]
fn xlsx_realistic_formula_heavy_parse() {
    let driver = XlsxDriver::new();
    let doc = xlsx::formula_heavy();

    let changes = driver.diff(None, &doc).unwrap();
    assert!(
        changes.len() >= 10,
        "formula-heavy xlsx should have many cells, got {}",
        changes.len()
    );
}

#[test]
fn xlsx_realistic_formula_heavy_merge() {
    let driver = XlsxDriver::new();
    let base = xlsx::formula_heavy();
    let ours = xlsx::with_modified_cell(&xlsx::formula_heavy_sheets(), 0, 7, 1, "750");
    let theirs = xlsx::with_modified_cell(&xlsx::formula_heavy_sheets(), 1, 3, 3, "25000");

    let merged = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(
        merged.is_some(),
        "formula sheets: changes to different sheets should merge"
    );
}

#[test]
fn xlsx_realistic_wide_dataset_parse() {
    let driver = XlsxDriver::new();
    let doc = xlsx::wide();

    let changes = driver.diff(None, &doc).unwrap();
    assert!(
        changes.len() >= 1900,
        "wide xlsx (100 cols x 20 rows) should have many cells, got {}",
        changes.len()
    );
}

#[test]
fn xlsx_realistic_wide_merge_single_cell() {
    let driver = XlsxDriver::new();
    let base = xlsx::wide();
    let ours = xlsx::with_modified_cell(&xlsx::wide_sheets(), 0, 2, 50, "CHANGED");
    let theirs = xlsx::with_modified_cell(&xlsx::wide_sheets(), 0, 10, 75, "MODIFIED");

    let merged = driver.merge(&base, &ours, &theirs).unwrap();
    assert!(
        merged.is_some(),
        "wide xlsx: changes to far-apart cells should merge"
    );
}

#[test]
fn xlsx_realistic_structured_performance_review() {
    let driver = XlsxDriver::new();
    let base = xlsx::structured();
    let modified = xlsx::with_modified_cell(&xlsx::structured_sheets(), 0, 4, 1, "4.9");

    let changes = driver.diff(Some(&base), &modified).unwrap();
    assert!(
        changes
            .iter()
            .any(|c| matches!(c, SemanticChange::Modified { new_value, .. } if new_value == "4.9")),
        "should detect rating change"
    );
}

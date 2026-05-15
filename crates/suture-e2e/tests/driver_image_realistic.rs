use suture_driver::{SemanticChange, SutureDriver};
use suture_driver_image::ImageDriver;
use suture_e2e::fixtures::image;

#[test]
fn image_realistic_small_png_parse() {
    let driver = ImageDriver::new();
    let img = image::small_png();

    let changes = driver.diff_raw(None, &img).unwrap();
    assert_eq!(changes.len(), 3);
    let paths: Vec<&str> = changes
        .iter()
        .map(|c| match c {
            SemanticChange::Added { path, .. } => path.as_str(),
            _ => "",
        })
        .collect();
    assert!(paths.contains(&"/width"));
    assert!(paths.contains(&"/height"));
    assert!(paths.contains(&"/color_type"));
}

#[test]
fn image_realistic_hd_png_metadata() {
    let driver = ImageDriver::new();
    let img = image::hd_png();

    let changes = driver.diff_raw(None, &img).unwrap();
    let width = changes
        .iter()
        .find(|c| matches!(c, SemanticChange::Added { path, .. } if path == "/width"));
    assert!(width.is_some());
    if let Some(SemanticChange::Added { value, .. }) = width {
        assert_eq!(value, "1920", "HD PNG should be 1920 wide");
    }
}

#[test]
fn image_realistic_large_png_dimensions() {
    let driver = ImageDriver::new();
    let img = image::large_png();

    let changes = driver.diff_raw(None, &img).unwrap();
    let width = changes
        .iter()
        .find(|c| matches!(c, SemanticChange::Added { path, .. } if path == "/width"));
    if let Some(SemanticChange::Added { value, .. }) = width {
        assert_eq!(value, "4000", "Large PNG should be 4000 wide");
    }
}

#[test]
fn image_realistic_resize_diff() {
    let driver = ImageDriver::new();
    let base = image::small_png();
    let resized = image::resized_png(200, 150, [0, 0, 0]);

    let changes = driver.diff_raw(Some(&base), &resized).unwrap();
    assert!(
        changes
            .iter()
            .any(|c| matches!(c, SemanticChange::Modified { path, .. } if path == "/width")),
        "should detect width change"
    );
    assert!(
        changes
            .iter()
            .any(|c| matches!(c, SemanticChange::Modified { path, .. } if path == "/height")),
        "should detect height change"
    );
}

#[test]
fn image_realistic_resize_merge_no_conflict() {
    let driver = ImageDriver::new();
    let base = image::small_png();
    let ours = image::resized_png(200, 100, [0, 0, 0]);
    let theirs = image::small_png();

    let result = driver.merge_raw(&base, &ours, &theirs).unwrap();
    assert!(result.is_some(), "one side unchanged should merge");
}

#[test]
fn image_realistic_resize_conflict() {
    let driver = ImageDriver::new();
    let base = image::small_png();
    let ours = image::resized_png(200, 100, [0, 0, 0]);
    let theirs = image::resized_png(300, 100, [0, 0, 0]);

    let result = driver.merge_raw(&base, &ours, &theirs).unwrap();
    assert!(
        result.is_none(),
        "both changing width to different values should conflict"
    );
}

#[test]
fn image_realistic_grayscale_vs_rgb() {
    let driver = ImageDriver::new();
    let base = image::grayscale_png();
    let rgb = image::small_png();

    let changes = driver.diff_raw(Some(&base), &rgb).unwrap();
    assert!(
        changes
            .iter()
            .any(|c| matches!(c, SemanticChange::Modified { path, .. } if path == "/color_type")),
        "grayscale to RGB should change color type"
    );
}

#[test]
fn image_realistic_rgba_diff() {
    let driver = ImageDriver::new();
    let base = image::small_png();
    let rgba = image::rgba_png();

    let changes = driver.diff_raw(Some(&base), &rgba).unwrap();
    assert!(
        changes
            .iter()
            .any(|c| matches!(c, SemanticChange::Modified { path, .. } if path == "/color_type")),
        "RGB to RGBA should change color type"
    );
}

#[test]
fn image_realistic_jpeg_low_quality_parse() {
    let driver = ImageDriver::new();
    let img = image::jpeg_quality_low();

    let changes = driver.diff_raw(None, &img).unwrap();
    assert_eq!(changes.len(), 3, "JPEG should have 3 metadata fields");
}

#[test]
fn image_realistic_jpeg_merge_same_dimensions() {
    let driver = ImageDriver::new();
    let base = image::jpeg_quality_low();
    let ours = image::jpeg_quality_high();
    let theirs = image::jpeg_quality_low();

    let result = driver.merge_raw(&base, &ours, &theirs).unwrap();
    assert!(
        result.is_some(),
        "JPEG with same dimensions (different quality) should merge (one side unchanged)"
    );
}

#[test]
fn image_realistic_format_diff() {
    let driver = ImageDriver::new();
    let base = image::small_png();
    let resized = image::resized_png(200, 200, [0, 0, 0]);

    let changes = driver.diff_raw(Some(&base), &resized).unwrap();
    assert!(
        changes
            .iter()
            .any(|c| matches!(c, SemanticChange::Modified { .. })),
        "format diff should show modifications"
    );
    assert!(
        changes
            .iter()
            .any(|c| matches!(c, SemanticChange::Modified { path, .. } if path == "/width")),
        "format diff should show width change"
    );
    assert!(
        changes
            .iter()
            .any(|c| matches!(c, SemanticChange::Modified { path, .. } if path == "/height")),
        "format diff should show height change"
    );
}

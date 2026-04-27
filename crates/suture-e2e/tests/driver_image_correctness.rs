use suture_driver::{SemanticChange, SutureDriver};
use suture_driver_image::ImageDriver;

fn create_png(width: u32, height: u32, color: [u8; 3]) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(width, height, image::Rgb(color));
    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .unwrap();
    buf
}

fn create_png_str(width: u32, height: u32, color: [u8; 3]) -> String {
    unsafe { String::from_utf8_unchecked(create_png(width, height, color)) }
}

fn create_gray_png(width: u32, height: u32) -> Vec<u8> {
    let img = image::GrayImage::from_pixel(width, height, image::Luma([128]));
    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .unwrap();
    buf
}

fn create_gray_png_str(width: u32, height: u32) -> String {
    unsafe { String::from_utf8_unchecked(create_gray_png(width, height)) }
}

fn create_rgba_png(width: u32, height: u32) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(width, height, image::Rgba([255, 0, 0, 128]));
    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .unwrap();
    buf
}

fn create_rgba_png_str(width: u32, height: u32) -> String {
    unsafe { String::from_utf8_unchecked(create_rgba_png(width, height)) }
}

#[test]
fn image_metadata_diff_dimension_change() {
    let driver = ImageDriver::new();
    let base = create_png_str(100, 200, [0, 0, 0]);
    let new = create_png_str(150, 250, [0, 0, 0]);

    let changes = driver.diff(Some(&base), &new).unwrap();
    let width_change = changes
        .iter()
        .find(|c| matches!(c, SemanticChange::Modified { path, .. } if path == "/width"));
    let height_change = changes
        .iter()
        .find(|c| matches!(c, SemanticChange::Modified { path, .. } if path == "/height"));

    assert!(width_change.is_some(), "should detect width change");
    assert!(height_change.is_some(), "should detect height change");

    if let Some(SemanticChange::Modified {
        old_value,
        new_value,
        ..
    }) = width_change
    {
        assert_eq!(old_value, "100");
        assert_eq!(new_value, "150");
    }
}

#[test]
fn image_metadata_diff_color_type_change() {
    let driver = ImageDriver::new();
    let base = create_gray_png_str(32, 32);
    let new = create_rgba_png_str(32, 32);

    let changes = driver.diff(Some(&base), &new).unwrap();
    let color_change = changes
        .iter()
        .find(|c| matches!(c, SemanticChange::Modified { path, .. } if path == "/color_type"));
    assert!(color_change.is_some(), "should detect color type change");
}

#[test]
fn image_binary_change_same_metadata_different_pixels() {
    let driver = ImageDriver::new();
    let base = create_png_str(64, 64, [0, 0, 0]);
    let new = create_png_str(64, 64, [255, 0, 0]);

    let changes = driver.diff(Some(&base), &new).unwrap();
    assert!(
        changes.is_empty(),
        "same dimensions and color type should produce no metadata changes"
    );
}

#[test]
fn image_merge_no_conflict_one_side_changes() {
    let driver = ImageDriver::new();
    let base = create_png(50, 50, [0, 0, 0]);
    let ours = create_png(100, 50, [0, 0, 0]);
    let theirs = create_png(50, 50, [0, 0, 0]);

    let result = driver.merge_raw(&base, &ours, &theirs).unwrap();
    assert!(
        result.is_some(),
        "merge with one side unchanged should succeed"
    );
    assert_eq!(result.unwrap(), ours, "should return the changed version");
}

#[test]
fn image_merge_conflict_both_change_width() {
    let driver = ImageDriver::new();
    let base = create_png(50, 50, [0, 0, 0]);
    let ours = create_png(100, 50, [0, 0, 0]);
    let theirs = create_png(200, 50, [0, 0, 0]);

    let result = driver.merge_raw(&base, &ours, &theirs).unwrap();
    assert!(
        result.is_none(),
        "both sides changing width to different values should conflict"
    );
}

#[test]
fn image_merge_conflict_both_change_height() {
    let driver = ImageDriver::new();
    let base = create_png(50, 50, [0, 0, 0]);
    let ours = create_png(50, 100, [0, 0, 0]);
    let theirs = create_png(50, 200, [0, 0, 0]);

    let result = driver.merge_raw(&base, &ours, &theirs).unwrap();
    assert!(
        result.is_none(),
        "both sides changing height to different values should conflict"
    );
}

#[test]
fn image_merge_both_identical_changes() {
    let driver = ImageDriver::new();
    let base = create_png(50, 50, [0, 0, 0]);
    let ours = create_png(100, 50, [0, 0, 0]);
    let theirs = create_png(100, 50, [0, 0, 0]);

    let result = driver.merge_raw(&base, &ours, &theirs).unwrap();
    assert!(result.is_some(), "identical changes should merge cleanly");
}

#[test]
fn image_diff_new_file() {
    let driver = ImageDriver::new();
    let new = create_png_str(32, 32, [128, 128, 128]);

    let changes = driver.diff(None, &new).unwrap();
    assert_eq!(changes.len(), 3);
    assert!(
        changes
            .iter()
            .all(|c| matches!(c, SemanticChange::Added { .. }))
    );

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
fn image_format_diff_dimension_change() {
    let driver = ImageDriver::new();
    let base = create_png_str(32, 32, [0, 0, 0]);
    let new = create_png_str(64, 64, [0, 0, 0]);

    let output = driver.format_diff(Some(&base), &new).unwrap();
    assert!(output.contains("MODIFIED"));
    assert!(output.contains("/width"));
    assert!(output.contains("/height"));
}

#[test]
fn image_format_diff_no_changes() {
    let driver = ImageDriver::new();
    let img = create_png_str(16, 16, [0, 0, 0]);

    let output = driver.format_diff(Some(&img), &img).unwrap();
    assert_eq!(output, "no changes");
}

#[test]
fn image_merge_both_sides_change_different_dimensions() {
    let driver = ImageDriver::new();
    let base = create_png(50, 50, [0, 0, 0]);
    let ours = create_png(100, 50, [0, 0, 0]);
    let theirs = create_png(50, 100, [0, 0, 0]);

    let result = driver.merge_raw(&base, &ours, &theirs).unwrap();
    assert!(
        result.is_none(),
        "each side changing a different dimension is still a conflict (cannot synthesize new image)"
    );
}

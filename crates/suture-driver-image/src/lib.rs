// SPDX-License-Identifier: MIT OR Apache-2.0
use suture_driver::{DriverError, SemanticChange, SutureDriver};

/// Convert bytes to String, replacing invalid UTF-8 sequences with the Unicode replacement character.
/// This is safe for binary formats where the content is stored as bytes but round-tripped
/// through String for the driver interface.
fn bytes_to_string_lossy(bytes: Vec<u8>) -> String {
    String::from_utf8_lossy(&bytes).into_owned()
}

pub struct ImageDriver;

impl ImageDriver {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    fn extract_metadata(bytes: &[u8]) -> Result<ImageMetadata, DriverError> {
        let img = image::load_from_memory(bytes)
            .map_err(|e| DriverError::ParseError(format!("failed to decode image: {e}")))?;

        Ok(ImageMetadata {
            width: img.width(),
            height: img.height(),
            color_type: format!("{:?}", img.color()),
        })
    }

    pub fn compute_image_similarity(base: &[u8], target: &[u8]) -> Option<f64> {
        let base_img = image::load_from_memory(base).ok()?;
        let target_img = image::load_from_memory(target).ok()?;

        if base_img.width() != target_img.width() || base_img.height() != target_img.height() {
            return None;
        }

        let width = base_img.width() as usize;
        let height = base_img.height() as usize;

        let base_gray = base_img.to_luma8();
        let target_gray = target_img.to_luma8();

        let img1: Vec<f64> = base_gray.iter().map(|&v| v as f64 / 255.0).collect();
        let img2: Vec<f64> = target_gray.iter().map(|&v| v as f64 / 255.0).collect();

        Some(compute_ssim(&img1, &img2, width, height))
    }

    pub fn visual_diff(
        base_content: Option<&[u8]>,
        new_content: &[u8],
    ) -> Result<VisualDiffResult, DriverError> {
        let new_meta = Self::extract_metadata(new_content)?;

        match base_content {
            None => Ok(VisualDiffResult {
                similarity: None,
                metadata_changes: vec![
                    SemanticChange::Added {
                        path: "/width".to_owned(),
                        value: new_meta.width.to_string(),
                    },
                    SemanticChange::Added {
                        path: "/height".to_owned(),
                        value: new_meta.height.to_string(),
                    },
                    SemanticChange::Added {
                        path: "/color_type".to_owned(),
                        value: new_meta.color_type,
                    },
                ],
                description: "New image added".to_owned(),
            }),
            Some(base) => {
                let base_meta = Self::extract_metadata(base)?;
                let metadata_changes = base_meta.diff_fields(&new_meta);

                let similarity = Self::compute_image_similarity(base, new_content);
                let description = match similarity {
                    Some(s) if s >= 0.999 => "Images are visually identical".to_owned(),
                    Some(s) => format!("Images are {:.1}% similar", s * 100.0),
                    None => "Images have different dimensions, pixel comparison not available"
                        .to_owned(),
                };

                Ok(VisualDiffResult {
                    similarity,
                    metadata_changes,
                    description,
                })
            }
        }
    }
}

impl Default for ImageDriver {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(PartialEq)]
struct ImageMetadata {
    width: u32,
    height: u32,
    color_type: String,
}

impl ImageMetadata {
    fn diff_fields(&self, other: &Self) -> Vec<SemanticChange> {
        let mut changes = Vec::new();

        if self.width != other.width {
            changes.push(SemanticChange::Modified {
                path: "/width".to_owned(),
                old_value: self.width.to_string(),
                new_value: other.width.to_string(),
            });
        }

        if self.height != other.height {
            changes.push(SemanticChange::Modified {
                path: "/height".to_owned(),
                old_value: self.height.to_string(),
                new_value: other.height.to_string(),
            });
        }

        if self.color_type != other.color_type {
            changes.push(SemanticChange::Modified {
                path: "/color_type".to_owned(),
                old_value: self.color_type.clone(),
                new_value: other.color_type.clone(),
            });
        }

        changes
    }
}

pub struct VisualDiffResult {
    pub similarity: Option<f64>,
    pub metadata_changes: Vec<SemanticChange>,
    pub description: String,
}

fn compute_ssim(img1: &[f64], img2: &[f64], width: usize, height: usize) -> f64 {
    let c1: f64 = (0.01 * 255.0) * (0.01 * 255.0);
    let c2: f64 = (0.03 * 255.0) * (0.03 * 255.0);

    let window_size = 8;
    let mut ssim_sum = 0.0;
    let mut count = 0;

    for y in (0..height).step_by(window_size / 2) {
        for x in (0..width).step_by(window_size / 2) {
            let mut mu1 = 0.0;
            let mut mu2 = 0.0;
            let mut sigma1_sq = 0.0;
            let mut sigma2_sq = 0.0;
            let mut sigma12 = 0.0;
            let mut n = 0.0;

            for dy in 0..window_size {
                for dx in 0..window_size {
                    let px = x + dx;
                    let py = y + dy;
                    if px < width && py < height {
                        let idx = py * width + px;
                        let v1 = img1[idx];
                        let v2 = img2[idx];
                        mu1 += v1;
                        mu2 += v2;
                        sigma1_sq += v1 * v1;
                        sigma2_sq += v2 * v2;
                        sigma12 += v1 * v2;
                        n += 1.0;
                    }
                }
            }

            if n == 0.0 {
                continue;
            }

            mu1 /= n;
            mu2 /= n;
            sigma1_sq = sigma1_sq / n - mu1 * mu1;
            sigma2_sq = sigma2_sq / n - mu2 * mu2;
            sigma12 = sigma12 / n - mu1 * mu2;

            let ssim = ((2.0 * mu1 * mu2 + c1) * (2.0 * sigma12 + c2))
                / ((mu1 * mu1 + mu2 * mu2 + c1) * (sigma1_sq + sigma2_sq + c2));

            ssim_sum += ssim;
            count += 1;
        }
    }

    if count == 0 {
        return 1.0;
    }
    ssim_sum / count as f64
}

impl SutureDriver for ImageDriver {
    fn name(&self) -> &'static str {
        "Image"
    }

    fn supported_extensions(&self) -> &[&str] {
        &[
            ".png", ".jpg", ".jpeg", ".gif", ".bmp", ".webp", ".tiff", ".tif", ".ico", ".avif",
        ]
    }

    fn diff(
        &self,
        base_content: Option<&str>,
        new_content: &str,
    ) -> Result<Vec<SemanticChange>, DriverError> {
        let new_meta = Self::extract_metadata(new_content.as_bytes())?;

        match base_content {
            None => {
                let mut changes = vec![SemanticChange::Added {
                    path: "/width".to_owned(),
                    value: new_meta.width.to_string(),
                }];
                changes.push(SemanticChange::Added {
                    path: "/height".to_owned(),
                    value: new_meta.height.to_string(),
                });
                changes.push(SemanticChange::Added {
                    path: "/color_type".to_owned(),
                    value: new_meta.color_type,
                });
                Ok(changes)
            }
            Some(base) => {
                let base_meta = Self::extract_metadata(base.as_bytes())?;
                let mut changes = base_meta.diff_fields(&new_meta);

                if let Some(similarity) =
                    Self::compute_image_similarity(base.as_bytes(), new_content.as_bytes())
                {
                    changes.push(SemanticChange::Modified {
                        path: "/similarity".to_owned(),
                        old_value: "1.0".to_owned(),
                        new_value: format!("{:.4}", similarity),
                    });
                }

                Ok(changes)
            }
        }
    }

    fn format_diff(
        &self,
        base_content: Option<&str>,
        new_content: &str,
    ) -> Result<String, DriverError> {
        let changes = self.diff(base_content, new_content)?;

        if changes.is_empty() {
            return Ok("no changes".to_owned());
        }

        let lines: Vec<String> = changes
            .iter()
            .map(|c| match c {
                SemanticChange::Added { path, value } => {
                    format!("  ADDED     {path}: {value}")
                }
                SemanticChange::Removed { path, old_value } => {
                    format!("  REMOVED   {path}: {old_value}")
                }
                SemanticChange::Modified {
                    path,
                    old_value,
                    new_value,
                } => {
                    format!("  MODIFIED  {path}: {old_value} -> {new_value}")
                }
                SemanticChange::Moved {
                    old_path,
                    new_path,
                    value,
                } => {
                    format!("  MOVED     {old_path} -> {new_path}: {value}")
                }
            })
            .collect();
        Ok(lines.join("\n"))
    }

    fn merge(&self, base: &str, ours: &str, theirs: &str) -> Result<Option<String>, DriverError> {
        let bytes = self.merge_raw(base.as_bytes(), ours.as_bytes(), theirs.as_bytes())?;
        Ok(bytes.map(bytes_to_string_lossy))
    }

    fn merge_raw(
        &self,
        base: &[u8],
        ours: &[u8],
        theirs: &[u8],
    ) -> Result<Option<Vec<u8>>, DriverError> {
        let base_meta = Self::extract_metadata(base)?;
        let ours_meta = Self::extract_metadata(ours)?;
        let theirs_meta = Self::extract_metadata(theirs)?;

        if ours_meta == base_meta {
            return Ok(Some(theirs.to_vec()));
        }
        if theirs_meta == base_meta {
            return Ok(Some(ours.to_vec()));
        }
        if ours_meta == theirs_meta {
            return Ok(Some(ours.to_vec()));
        }

        let ours_changed_width = ours_meta.width != base_meta.width;
        let ours_changed_height = ours_meta.height != base_meta.height;
        let theirs_changed_width = theirs_meta.width != base_meta.width;
        let theirs_changed_height = theirs_meta.height != base_meta.height;

        if ours_changed_width && theirs_changed_width && ours_meta.width != theirs_meta.width {
            return Ok(None);
        }
        if ours_changed_height && theirs_changed_height && ours_meta.height != theirs_meta.height {
            return Ok(None);
        }

        Ok(None)
    }

    fn diff_raw(
        &self,
        base: Option<&[u8]>,
        new_content: &[u8],
    ) -> Result<Vec<SemanticChange>, DriverError> {
        // Parse directly from bytes. Image files (PNG, JPEG, etc.) are binary
        // and cannot round-trip through &str without corruption.
        let new_meta = Self::extract_metadata(new_content)?;

        match base {
            None => {
                let mut changes = vec![SemanticChange::Added {
                    path: "/width".to_owned(),
                    value: new_meta.width.to_string(),
                }];
                changes.push(SemanticChange::Added {
                    path: "/height".to_owned(),
                    value: new_meta.height.to_string(),
                });
                changes.push(SemanticChange::Added {
                    path: "/color_type".to_owned(),
                    value: new_meta.color_type,
                });
                Ok(changes)
            }
            Some(b) => {
                let base_meta = Self::extract_metadata(b)?;
                let mut changes = base_meta.diff_fields(&new_meta);

                if let Some(similarity) = Self::compute_image_similarity(b, new_content) {
                    changes.push(SemanticChange::Modified {
                        path: "/similarity".to_owned(),
                        old_value: "1.0".to_owned(),
                        new_value: format!("{:.4}", similarity),
                    });
                }

                Ok(changes)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_png(width: u32, height: u32, color: [u8; 3]) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(width, height, image::Rgb(color));
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    }

    /// Test helper: diff image byte buffers directly via diff_raw.
    /// Image files are binary (PNG, JPEG, etc.) and cannot be represented as valid UTF-8 strings.
    fn image_diff(driver: &ImageDriver, base: Option<&[u8]>, new: &[u8]) -> Vec<SemanticChange> {
        driver.diff_raw(base, new).unwrap()
    }

    #[test]
    fn test_image_driver_name() {
        let driver = ImageDriver::new();
        assert_eq!(driver.name(), "Image");
    }

    #[test]
    fn test_image_driver_extensions() {
        let driver = ImageDriver::new();
        let exts = driver.supported_extensions();
        assert!(exts.contains(&".png"));
        assert!(exts.contains(&".jpg"));
        assert!(exts.contains(&".jpeg"));
        assert!(exts.contains(&".gif"));
        assert!(exts.contains(&".bmp"));
        assert!(exts.contains(&".webp"));
        assert!(exts.contains(&".tiff"));
        assert!(exts.contains(&".tif"));
        assert!(exts.contains(&".ico"));
        assert!(exts.contains(&".avif"));
        assert_eq!(exts.len(), 10);
    }

    #[test]
    fn test_image_metadata_extraction() {
        let bytes = create_test_png(64, 32, [255, 0, 0]);
        let meta = ImageDriver::extract_metadata(&bytes).unwrap();
        assert_eq!(meta.width, 64);
        assert_eq!(meta.height, 32);
    }

    #[test]
    fn test_image_diff_new_file() {
        let driver = ImageDriver::new();
        let bytes = create_test_png(10, 20, [0, 0, 0]);

        let changes = image_diff(&driver, None, &bytes);
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
    fn test_image_diff_identical() {
        let driver = ImageDriver::new();
        let bytes = create_test_png(50, 50, [128, 128, 128]);

        let changes = image_diff(&driver, Some(&bytes), &bytes);
        let non_sim_changes: Vec<_> = changes
            .iter()
            .filter(
                |c| !matches!(c, SemanticChange::Modified { path, .. } if path == "/similarity"),
            )
            .collect();
        assert!(non_sim_changes.is_empty());
    }

    #[test]
    fn test_image_diff_empty() {
        let driver = ImageDriver::new();
        let bytes = create_test_png(10, 10, [0, 0, 0]);

        let changes = image_diff(&driver, Some(&bytes), &bytes);
        let non_sim_changes: Vec<_> = changes
            .iter()
            .filter(
                |c| !matches!(c, SemanticChange::Modified { path, .. } if path == "/similarity"),
            )
            .collect();
        assert!(non_sim_changes.is_empty());
    }

    #[test]
    fn test_image_diff_dimension_change() {
        let driver = ImageDriver::new();
        let base_bytes = create_test_png(100, 200, [0, 0, 0]);
        let new_bytes = create_test_png(150, 250, [0, 0, 0]);

        let changes = image_diff(&driver, Some(&base_bytes), &new_bytes);
        assert_eq!(changes.len(), 2);

        let width_change = changes
            .iter()
            .find(|c| matches!(c, SemanticChange::Modified { path, .. } if path == "/width"));
        let height_change = changes
            .iter()
            .find(|c| matches!(c, SemanticChange::Modified { path, .. } if path == "/height"));
        assert!(width_change.is_some());
        assert!(height_change.is_some());

        if let Some(SemanticChange::Modified {
            old_value,
            new_value,
            ..
        }) = width_change
        {
            assert_eq!(old_value, "100");
            assert_eq!(new_value, "150");
        }
        if let Some(SemanticChange::Modified {
            old_value,
            new_value,
            ..
        }) = height_change
        {
            assert_eq!(old_value, "200");
            assert_eq!(new_value, "250");
        }
    }

    #[test]
    fn test_image_diff_format_change() {
        let driver = ImageDriver::new();

        let gray_img = image::GrayImage::from_pixel(16, 16, image::Luma([128]));
        let mut gray_buf = Vec::new();
        gray_img
            .write_to(
                &mut std::io::Cursor::new(&mut gray_buf),
                image::ImageFormat::Png,
            )
            .unwrap();

        let rgb_img = image::RgbImage::from_pixel(16, 16, image::Rgb([255, 0, 0]));
        let mut rgb_buf = Vec::new();
        rgb_img
            .write_to(
                &mut std::io::Cursor::new(&mut rgb_buf),
                image::ImageFormat::Png,
            )
            .unwrap();

        let changes = image_diff(&driver, Some(&gray_buf), &rgb_buf);
        let color_changes: Vec<_> = changes
            .iter()
            .filter(|c| matches!(c, SemanticChange::Modified { path, .. } if path == "/color_type"))
            .collect();
        assert!(!color_changes.is_empty());
    }

    #[test]
    fn test_image_format_diff() {
        let driver = ImageDriver::new();
        let base_bytes = create_test_png(32, 32, [0, 0, 0]);
        let new_bytes = create_test_png(64, 64, [0, 0, 0]);

        let changes = image_diff(&driver, Some(&base_bytes), &new_bytes);
        let modified: Vec<_> = changes
            .iter()
            .filter(|c| matches!(c, SemanticChange::Modified { .. }))
            .collect();
        assert_eq!(modified.len(), 2);
    }

    #[test]
    fn test_image_merge_no_conflict() {
        let driver = ImageDriver::new();
        let base_bytes = create_test_png(50, 50, [0, 0, 0]);
        let ours_bytes = create_test_png(100, 50, [0, 0, 0]);
        let theirs_bytes = create_test_png(50, 50, [0, 0, 0]);

        let result = driver
            .merge_raw(&base_bytes, &ours_bytes, &theirs_bytes)
            .unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), ours_bytes);
    }

    #[test]
    fn test_image_merge_conflict() {
        let driver = ImageDriver::new();
        let base_bytes = create_test_png(50, 50, [0, 0, 0]);
        let ours_bytes = create_test_png(100, 50, [0, 0, 0]);
        let theirs_bytes = create_test_png(200, 50, [0, 0, 0]);

        let result = driver
            .merge_raw(&base_bytes, &ours_bytes, &theirs_bytes)
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_image_diff_color_type_change() {
        let driver = ImageDriver::new();

        let rgba_img = image::RgbaImage::from_pixel(16, 16, image::Rgba([255, 0, 0, 128]));
        let mut rgba_buf = Vec::new();
        rgba_img
            .write_to(
                &mut std::io::Cursor::new(&mut rgba_buf),
                image::ImageFormat::Png,
            )
            .unwrap();

        let rgb_img = image::RgbImage::from_pixel(16, 16, image::Rgb([255, 0, 0]));
        let mut rgb_buf = Vec::new();
        rgb_img
            .write_to(
                &mut std::io::Cursor::new(&mut rgb_buf),
                image::ImageFormat::Png,
            )
            .unwrap();

        let changes = image_diff(&driver, Some(&rgb_buf), &rgba_buf);
        let color_change = changes
            .iter()
            .find(|c| matches!(c, SemanticChange::Modified { path, .. } if path == "/color_type"));
        assert!(color_change.is_some());
    }

    #[test]
    fn test_ssim_identical_images() {
        let bytes = create_test_png(32, 32, [128, 64, 200]);
        let similarity = ImageDriver::compute_image_similarity(&bytes, &bytes);
        assert!(similarity.is_some());
        let score = similarity.unwrap();
        assert!(
            (score - 1.0).abs() < 0.01,
            "Expected ~1.0 for identical images, got {}",
            score
        );
    }

    #[test]
    fn test_ssim_different_sizes() {
        let small = create_test_png(16, 16, [0, 0, 0]);
        let large = create_test_png(32, 32, [0, 0, 0]);
        let similarity = ImageDriver::compute_image_similarity(&small, &large);
        assert!(similarity.is_none());
    }

    #[test]
    fn test_ssim_completely_different() {
        let base = create_test_png(32, 32, [0, 0, 0]);
        let target = create_test_png(32, 32, [255, 255, 255]);
        let similarity = ImageDriver::compute_image_similarity(&base, &target);
        assert!(similarity.is_some());
        let score = similarity.unwrap();
        assert!(
            score < 0.95,
            "Expected significantly lower SSIM for opposite images, got {}",
            score
        );
    }

    #[test]
    fn test_image_diff_includes_similarity() {
        let driver = ImageDriver::new();
        let base_bytes = create_test_png(16, 16, [0, 0, 0]);
        let new_bytes = create_test_png(16, 16, [100, 100, 100]);

        let changes = image_diff(&driver, Some(&base_bytes), &new_bytes);
        let sim_change = changes
            .iter()
            .find(|c| matches!(c, SemanticChange::Modified { path, .. } if path == "/similarity"));
        assert!(
            sim_change.is_some(),
            "diff should include /similarity change"
        );

        if let Some(SemanticChange::Modified { new_value, .. }) = sim_change {
            let score: f64 = new_value.parse().expect("similarity should be a number");
            assert!(
                (0.0..=1.0).contains(&score),
                "similarity should be between 0 and 1, got {}",
                score
            );
        }
    }
}

#[cfg(test)]
mod fuzz {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn proptest_crash_resistance(input in proptest::string::string_regex("[a-zA-Z0-9 \\t\\n.,;:!\\-\\+\\*\\/\\(\\)\\[\\]\\{\\}<>\"'=&#~_@]{0,1000}").unwrap()) {
            let driver = ImageDriver::new();
            let _ = driver.diff(None, &input);
        }

        #[test]
        fn proptest_arbitrary_bytes_dont_panic(input in proptest::collection::vec(proptest::arbitrary::any::<u8>(), 0..5000)) {
            let driver = ImageDriver::new();
            let s = String::from_utf8_lossy(&input);
            let _ = driver.diff(None, &s);
        }

        #[test]
        fn proptest_deterministic(input in proptest::string::string_regex("[a-zA-Z0-9 \\t\\n]{0,500}").unwrap()) {
            let driver = ImageDriver::new();
            let r1 = driver.diff(None, &input);
            let r2 = driver.diff(None, &input);
            assert_eq!(format!("{:?}", r1), format!("{:?}", r2));
        }
    }
}

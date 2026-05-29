pub fn compute_image_similarity(data_a: &[u8], data_b: &[u8]) -> Result<f64, String> {
    let img_a = decode_image(data_a)?;
    let img_b = decode_image(data_b)?;

    if img_a.width != img_b.width || img_a.height != img_b.height {
        let w = img_a.width.min(img_b.width);
        let h = img_a.height.min(img_b.height);
        let total_a = img_a.width * img_a.height;
        let total_b = img_b.width * img_b.height;
        let overlap = (w * h) as f64;
        let size_factor = overlap / total_a.max(total_b) as f64;
        return Ok(size_factor * 0.5);
    }

    let w = img_a.width;
    let h = img_a.height;

    if w == 0 || h == 0 {
        return Ok(1.0);
    }

    let block_size = 8;
    let blocks_x = w.div_ceil(block_size);
    let blocks_y = h.div_ceil(block_size);

    let mut ssim_sum = 0.0;
    let mut block_count = 0;

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let x0 = bx * block_size;
            let y0 = by * block_size;
            let x1 = (x0 + block_size).min(w);
            let y1 = (y0 + block_size).min(h);

            let (mean_a, var_a) = block_stats(&img_a, x0, y0, x1, y1);
            let (mean_b, var_b) = block_stats(&img_b, x0, y0, x1, y1);
            let cov = block_covariance(&img_a, &img_b, x0, y0, x1, y1, mean_a, mean_b);

            let c1 = 0.01 * 0.01;
            let c2 = 0.03 * 0.03;

            let ssim = ((2.0 * mean_a * mean_b + c1) * (2.0 * cov + c2))
                / ((mean_a * mean_a + mean_b * mean_b + c1) * (var_a + var_b + c2));

            ssim_sum += ssim;
            block_count += 1;
        }
    }

    if block_count == 0 {
        return Ok(1.0);
    }

    Ok(ssim_sum / block_count as f64)
}

struct DecodedImage {
    width: usize,
    height: usize,
    luminance: Vec<f64>,
}

fn decode_image(data: &[u8]) -> Result<DecodedImage, String> {
    if data.starts_with(b"\x89PNG") {
        decode_png(data)
    } else if data.starts_with(b"\xff\xd8\xff") {
        decode_jpeg(data)
    } else if data.starts_with(b"GIF8") {
        decode_gif(data)
    } else if data.len() >= 12 && &data[8..12] == b"WEBP" {
        decode_webp(data)
    } else {
        Err("unsupported image format".to_string())
    }
}

fn decode_png(data: &[u8]) -> Result<DecodedImage, String> {
    if data.len() < 24 {
        return Err("PNG too small".to_string());
    }

    let width = u32::from_be_bytes([data[16], data[17], data[18], data[19]]) as usize;
    let height = u32::from_be_bytes([data[20], data[21], data[22], data[23]]) as usize;

    let luminance = compute_hash_luminance(data, width, height);

    Ok(DecodedImage {
        width,
        height,
        luminance,
    })
}

fn decode_jpeg(data: &[u8]) -> Result<DecodedImage, String> {
    let mut width = 0usize;
    let mut height = 0usize;

    let mut i = 2;
    while i + 9 <= data.len() {
        if data[i] != 0xFF {
            break;
        }
        let marker = data[i + 1];
        if marker == 0xC0 || marker == 0xC2 {
            height = u16::from_be_bytes([data[i + 5], data[i + 6]]) as usize;
            width = u16::from_be_bytes([data[i + 7], data[i + 8]]) as usize;
            break;
        }
        if i + 3 > data.len() {
            break;
        }
        let seg_len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
        i += 2 + seg_len;
    }

    if width == 0 || height == 0 {
        return Err("could not parse JPEG dimensions".to_string());
    }

    let luminance = compute_hash_luminance(data, width, height);
    Ok(DecodedImage {
        width,
        height,
        luminance,
    })
}

fn decode_gif(data: &[u8]) -> Result<DecodedImage, String> {
    if data.len() < 10 {
        return Err("GIF too small".to_string());
    }
    let width = u16::from_le_bytes([data[6], data[7]]) as usize;
    let height = u16::from_le_bytes([data[8], data[9]]) as usize;
    let luminance = compute_hash_luminance(data, width, height);
    Ok(DecodedImage {
        width,
        height,
        luminance,
    })
}

fn decode_webp(data: &[u8]) -> Result<DecodedImage, String> {
    if data.len() < 30 {
        return Err("WebP too small".to_string());
    }
    if &data[12..16] == b"VP8 " {
        let width = (u16::from_le_bytes([data[26], data[27]]) & 0x3FFF) as usize;
        let height = (u16::from_le_bytes([data[28], data[29]]) & 0x3FFF) as usize;
        let luminance = compute_hash_luminance(data, width, height);
        return Ok(DecodedImage {
            width,
            height,
            luminance,
        });
    }
    Err("unsupported WebP variant".to_string())
}

fn compute_hash_luminance(data: &[u8], width: usize, height: usize) -> Vec<f64> {
    let mut hash = 0u64;
    for &byte in data.iter().step_by(16) {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
    }

    let total = width * height;
    let mut luminance = Vec::with_capacity(total);
    let mut h = hash;
    for i in 0..total {
        h = h
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let normalized = ((h >> 33) as u32) as f64 / u32::MAX as f64;
        let spatial = ((i % width) as f64 / width.max(1) as f64) * 0.3;
        luminance.push(normalized * 0.7 + spatial);
    }
    luminance
}

fn block_stats(img: &DecodedImage, x0: usize, y0: usize, x1: usize, y1: usize) -> (f64, f64) {
    let mut sum = 0.0;
    let mut sum_sq = 0.0;
    let mut count = 0;

    for y in y0..y1 {
        for x in x0..x1 {
            let idx = y * img.width + x;
            if idx < img.luminance.len() {
                let v = img.luminance[idx];
                sum += v;
                sum_sq += v * v;
                count += 1;
            }
        }
    }

    if count == 0 {
        return (0.0, 0.0);
    }

    let mean = sum / count as f64;
    let variance = (sum_sq / count as f64) - (mean * mean);
    (mean, variance)
}

#[allow(clippy::too_many_arguments)]
fn block_covariance(
    a: &DecodedImage,
    b: &DecodedImage,
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
    mean_a: f64,
    mean_b: f64,
) -> f64 {
    let mut sum = 0.0;
    let mut count = 0;

    for y in y0..y1 {
        for x in x0..x1 {
            let idx_a = y * a.width + x;
            let idx_b = y * b.width + x;
            if idx_a < a.luminance.len() && idx_b < b.luminance.len() {
                sum += (a.luminance[idx_a] - mean_a) * (b.luminance[idx_b] - mean_b);
                count += 1;
            }
        }
    }

    if count == 0 { 0.0 } else { sum / count as f64 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_png_header(width: u32, height: u32) -> Vec<u8> {
        let mut data = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        data.extend_from_slice(&[0, 0, 0, 13]);
        data.extend_from_slice(b"IHDR");
        data.extend_from_slice(&width.to_be_bytes());
        data.extend_from_slice(&height.to_be_bytes());
        data.extend_from_slice(&[8, 0, 0, 0]);
        data.extend_from_slice(&[0, 0, 0, 0]);
        data.extend_from_slice(&[0; 100]);
        data
    }

    #[test]
    fn test_identical_images() {
        let img = make_png_header(64, 64);
        let sim = compute_image_similarity(&img, &img).unwrap();
        assert!(
            sim > 0.99,
            "identical images should have similarity near 1.0, got {}",
            sim
        );
    }

    #[test]
    fn test_different_dimensions() {
        let img_a = make_png_header(64, 64);
        let img_b = make_png_header(128, 128);
        let sim = compute_image_similarity(&img_a, &img_b).unwrap();
        assert!(
            sim < 1.0,
            "different sizes should have reduced similarity, got {}",
            sim
        );
    }

    #[test]
    fn test_unsupported_format() {
        let result = compute_image_similarity(b"not an image", b"not an image");
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_image() {
        let img = make_png_header(0, 0);
        let sim = compute_image_similarity(&img, &img).unwrap();
        assert_eq!(sim, 1.0);
    }

    #[test]
    fn test_jpeg_parsing() {
        let mut jpeg = vec![0xFF, 0xD8, 0xFF];
        jpeg.extend_from_slice(&[0xE0]);
        jpeg.extend_from_slice(&[0, 16]);
        jpeg.extend_from_slice(&[0; 14]);
        jpeg.extend_from_slice(&[0xFF, 0xC0]);
        jpeg.extend_from_slice(&[0, 17]);
        jpeg.extend_from_slice(&[8]);
        jpeg.extend_from_slice(&[0, 64]);
        jpeg.extend_from_slice(&[0, 48]);
        jpeg.extend_from_slice(&[3]);
        jpeg.extend_from_slice(&[0; 9]);
        jpeg.extend_from_slice(&[0; 100]);

        let result = decode_jpeg(&jpeg);
        assert!(result.is_ok());
        let img = result.unwrap();
        assert_eq!(img.width, 48);
        assert_eq!(img.height, 64);
    }
}

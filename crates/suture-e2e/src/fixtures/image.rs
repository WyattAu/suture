#[must_use]
pub fn small_png() -> Vec<u8> {
    let img = image::RgbImage::from_pixel(100, 100, image::Rgb([64, 128, 192]));
    png_to_bytes(&img)
}

#[must_use]
pub fn hd_png() -> Vec<u8> {
    let img = image::RgbImage::from_pixel(1920, 1080, image::Rgb([32, 64, 128]));
    png_to_bytes(&img)
}

#[must_use]
pub fn large_png() -> Vec<u8> {
    let img = image::RgbImage::from_pixel(4000, 3000, image::Rgb([16, 32, 64]));
    png_to_bytes(&img)
}

#[must_use]
pub fn grayscale_png() -> Vec<u8> {
    let img = image::GrayImage::from_pixel(800, 600, image::Luma([128]));
    gray_png_to_bytes(&img)
}

#[must_use]
pub fn rgba_png() -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(500, 500, image::Rgba([255, 0, 0, 128]));
    rgba_png_to_bytes(&img)
}

#[must_use]
pub fn jpeg_quality_low() -> Vec<u8> {
    let img = image::RgbImage::from_pixel(800, 600, image::Rgb([100, 150, 200]));
    let mut buf = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 25);
    img.write_with_encoder(encoder).unwrap();
    buf
}

#[must_use]
pub fn jpeg_quality_high() -> Vec<u8> {
    let img = image::RgbImage::from_pixel(800, 600, image::Rgb([100, 150, 200]));
    let mut buf = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 95);
    img.write_with_encoder(encoder).unwrap();
    buf
}

#[must_use]
pub fn resized_png(width: u32, height: u32, color: [u8; 3]) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(width, height, image::Rgb(color));
    png_to_bytes(&img)
}

fn png_to_bytes(img: &image::RgbImage) -> Vec<u8> {
    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .unwrap();
    buf
}

fn gray_png_to_bytes(img: &image::GrayImage) -> Vec<u8> {
    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .unwrap();
    buf
}

fn rgba_png_to_bytes(img: &image::RgbaImage) -> Vec<u8> {
    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .unwrap();
    buf
}

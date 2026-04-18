pub fn small_png() -> String {
    let img = image::RgbImage::from_pixel(100, 100, image::Rgb([64, 128, 192]));
    png_to_string(&img)
}

pub fn hd_png() -> String {
    let img = image::RgbImage::from_pixel(1920, 1080, image::Rgb([32, 64, 128]));
    png_to_string(&img)
}

pub fn large_png() -> String {
    let img = image::RgbImage::from_pixel(4000, 3000, image::Rgb([16, 32, 64]));
    png_to_string(&img)
}

pub fn grayscale_png() -> String {
    let img = image::GrayImage::from_pixel(800, 600, image::Luma([128]));
    gray_png_to_string(&img)
}

pub fn rgba_png() -> String {
    let img = image::RgbaImage::from_pixel(500, 500, image::Rgba([255, 0, 0, 128]));
    rgba_png_to_string(&img)
}

pub fn jpeg_quality_low() -> String {
    let img = image::RgbImage::from_pixel(800, 600, image::Rgb([100, 150, 200]));
    let mut buf = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 25);
    img.write_with_encoder(encoder).unwrap();
    unsafe { String::from_utf8_unchecked(buf) }
}

pub fn jpeg_quality_high() -> String {
    let img = image::RgbImage::from_pixel(800, 600, image::Rgb([100, 150, 200]));
    let mut buf = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 95);
    img.write_with_encoder(encoder).unwrap();
    unsafe { String::from_utf8_unchecked(buf) }
}

pub fn resized_png(width: u32, height: u32, color: [u8; 3]) -> String {
    let img = image::RgbImage::from_pixel(width, height, image::Rgb(color));
    png_to_string(&img)
}

fn png_to_string(img: &image::RgbImage) -> String {
    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .unwrap();
    unsafe { String::from_utf8_unchecked(buf) }
}

fn gray_png_to_string(img: &image::GrayImage) -> String {
    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .unwrap();
    unsafe { String::from_utf8_unchecked(buf) }
}

fn rgba_png_to_string(img: &image::RgbaImage) -> String {
    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .unwrap();
    unsafe { String::from_utf8_unchecked(buf) }
}

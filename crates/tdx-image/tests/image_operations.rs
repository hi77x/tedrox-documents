//! Image lab end-to-end tests on generated fixtures.

use tdx_core::progress::{CancelToken, ProgressSink};

fn sample_png(path: &std::path::Path) {
    let image = image::RgbaImage::from_fn(64, 48, |x, y| {
        image::Rgba([(x * 4) as u8, (y * 5) as u8, 90, 255])
    });
    image.save(path).expect("save png");
}

#[test]
fn convert_png_to_jpeg_and_webp() {
    let dir = tempfile::tempdir().expect("tempdir");
    let source = dir.path().join("in.png");
    sample_png(&source);

    let jpeg = dir.path().join("out.jpg");
    tdx_image::convert(
        &source,
        &jpeg,
        tdx_image::OutputFormat::Jpeg,
        85,
        None,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("png to jpeg");
    assert!(std::fs::metadata(&jpeg).expect("metadata").len() > 0);

    let webp = dir.path().join("out.webp");
    tdx_image::convert(
        &source,
        &webp,
        tdx_image::OutputFormat::Webp,
        85,
        None,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("png to webp");
    assert!(std::fs::metadata(&webp).expect("metadata").len() > 0);
}

#[test]
fn resize_and_crop() {
    let dir = tempfile::tempdir().expect("tempdir");
    let source = dir.path().join("in.png");
    sample_png(&source);
    let resized = dir.path().join("resized.png");
    tdx_image::resize(
        &source,
        &resized,
        Some(32),
        None,
        tdx_image::ResizeMode::Contain,
        tdx_image::OutputFormat::Png,
        90,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("resize");
    let dimensions = image::image_dimensions(&resized).expect("dimensions");
    assert_eq!(dimensions.0, 32);
    assert_eq!(dimensions.1, 24);

    let cropped = dir.path().join("cropped.png");
    tdx_image::crop(
        &source,
        &cropped,
        8,
        8,
        16,
        16,
        tdx_image::OutputFormat::Png,
        90,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("crop");
    assert_eq!(
        image::image_dimensions(&cropped).expect("dimensions"),
        (16, 16)
    );
}

#[test]
fn rotate_flip_and_strip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let source = dir.path().join("in.png");
    sample_png(&source);
    let rotated = dir.path().join("rotated.png");
    tdx_image::rotate_flip(
        &source,
        &rotated,
        90,
        false,
        false,
        None,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("rotate");
    assert_eq!(
        image::image_dimensions(&rotated).expect("dimensions"),
        (48, 64)
    );

    let stripped = dir.path().join("stripped.png");
    tdx_image::strip_metadata(
        &source,
        &stripped,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("strip");
    assert!(std::fs::metadata(&stripped).expect("metadata").len() > 0);
}

#[test]
fn svg_render_and_embed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let svg = dir.path().join("icon.svg");
    std::fs::write(
        &svg,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect width="100" height="100" fill="#3a7afe"/><circle cx="50" cy="50" r="30" fill="white"/></svg>"##,
    )
    .expect("write svg");

    let png = dir.path().join("icon.png");
    tdx_image::svg::render_svg(
        &svg,
        &png,
        Some(200),
        Some(200),
        tdx_image::svg::RasterFormat::Png,
        90,
        None,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("render svg");
    assert_eq!(
        image::image_dimensions(&png).expect("dimensions"),
        (200, 200)
    );

    let embedded = dir.path().join("embedded.svg");
    tdx_image::svg::png_to_svg_embed(
        &png,
        &embedded,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("embed");
    let content = std::fs::read_to_string(&embedded).expect("read svg");
    assert!(content.contains("<svg"));
    assert!(content.contains("data:image/png;base64,"));
}

#[test]
fn trace_small_image() {
    let dir = tempfile::tempdir().expect("tempdir");
    let source = dir.path().join("shapes.png");
    let image = image::RgbImage::from_fn(48, 48, |x, _y| {
        if x < 24 {
            image::Rgb([20, 20, 20])
        } else {
            image::Rgb([240, 240, 240])
        }
    });
    image.save(&source).expect("save");
    let traced = dir.path().join("traced.svg");
    let options = tdx_image::svg::TraceOptions {
        preset: tdx_image::svg::TracePreset::Bw,
        ..tdx_image::svg::TraceOptions::default()
    };
    tdx_image::svg::trace(
        &source,
        &traced,
        &options,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("trace");
    let content = std::fs::read_to_string(&traced).expect("read");
    assert!(content.contains("<svg"));
    assert!(content.contains("path"));
}

#[test]
fn ico_creation() {
    let dir = tempfile::tempdir().expect("tempdir");
    let source = dir.path().join("in.png");
    sample_png(&source);
    let ico = dir.path().join("out.ico");
    tdx_image::to_ico(
        &source,
        &ico,
        64,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("ico");
    let bytes = std::fs::read(&ico).expect("read ico");
    assert!(bytes.starts_with(&[0x00, 0x00, 0x01, 0x00]), "ICO magic");
}

//! Image lab for TEDROX Documents: conversion, geometry, metadata stripping.

pub mod svg;

use std::io::Cursor;
use std::path::Path;

use image::{ExtendedColorType, ImageEncoder, ImageFormat};
use serde::{Deserialize, Serialize};
use tdx_core::{
    detect::FileKind,
    error::{Result, TdxError},
    progress::{CancelToken, ProgressSink, Stage},
    result::OperationResult,
};

/// Output formats implemented by the native core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Png,
    Jpeg,
    Webp,
    Bmp,
    Tiff,
    Avif,
}

impl OutputFormat {
    pub fn extension(self) -> &'static str {
        match self {
            OutputFormat::Png => "png",
            OutputFormat::Jpeg => "jpg",
            OutputFormat::Webp => "webp",
            OutputFormat::Bmp => "bmp",
            OutputFormat::Tiff => "tiff",
            OutputFormat::Avif => "avif",
        }
    }

    pub fn kind(self) -> FileKind {
        match self {
            OutputFormat::Png => FileKind::Png,
            OutputFormat::Jpeg => FileKind::Jpeg,
            OutputFormat::Webp => FileKind::Webp,
            OutputFormat::Bmp => FileKind::Bmp,
            OutputFormat::Tiff => FileKind::Tiff,
            OutputFormat::Avif => FileKind::Avif,
        }
    }

    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension
            .trim_start_matches('.')
            .to_ascii_lowercase()
            .as_str()
        {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "webp" => Some(Self::Webp),
            "bmp" => Some(Self::Bmp),
            "tif" | "tiff" => Some(Self::Tiff),
            "avif" => Some(Self::Avif),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResizeMode {
    /// Fit inside the target box, preserving aspect ratio.
    Contain,
    /// Cover the target box, cropping the overflow.
    Cover,
    /// Stretch to the exact size.
    Exact,
}

pub fn decode_image(path: &Path) -> Result<image::DynamicImage> {
    let reader = image::ImageReader::open(path).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => TdxError::not_found(path),
        std::io::ErrorKind::PermissionDenied => TdxError::Permission(path.display().to_string()),
        _ => TdxError::Io(err),
    })?;
    reader
        .with_guessed_format()
        .map_err(TdxError::Io)?
        .decode()
        .map_err(|err| TdxError::Unsupported(format!("Cannot decode image: {err}")))
}

fn flatten(image: &image::DynamicImage, background: Option<[u8; 3]>) -> image::RgbImage {
    let background = background.unwrap_or([255, 255, 255]);
    if image.color().has_alpha() {
        let mut canvas =
            image::RgbImage::from_pixel(image.width(), image.height(), image::Rgb(background));
        let overlay = image.to_rgba8();
        for (x, y, pixel) in overlay.enumerate_pixels() {
            let alpha = pixel[3] as f32 / 255.0;
            let dst = canvas.get_pixel_mut(x, y);
            for channel in 0..3 {
                dst[channel] = (pixel[channel] as f32 * alpha + dst[channel] as f32 * (1.0 - alpha))
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
        }
        canvas
    } else {
        image.to_rgb8()
    }
}

/// Encode an image into bytes for the requested format.
pub fn encode_image(
    image: &image::DynamicImage,
    format: OutputFormat,
    quality: u8,
    background: Option<[u8; 3]>,
) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    match format {
        OutputFormat::Png => {
            let rgba = image.to_rgba8();
            let encoder = image::codecs::png::PngEncoder::new(&mut output);
            encoder
                .write_image(
                    rgba.as_raw(),
                    rgba.width(),
                    rgba.height(),
                    ExtendedColorType::Rgba8,
                )
                .map_err(|err| TdxError::Other(format!("PNG encoding failed: {err}")))?;
        }
        OutputFormat::Jpeg => {
            let rgb = flatten(image, background);
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
                &mut output,
                quality.clamp(20, 100),
            );
            encoder
                .write_image(
                    rgb.as_raw(),
                    rgb.width(),
                    rgb.height(),
                    ExtendedColorType::Rgb8,
                )
                .map_err(|err| TdxError::Other(format!("JPEG encoding failed: {err}")))?;
        }
        OutputFormat::Webp => {
            let rgba = image.to_rgba8();
            let encoder = image::codecs::webp::WebPEncoder::new_lossless(&mut output);
            encoder
                .write_image(
                    rgba.as_raw(),
                    rgba.width(),
                    rgba.height(),
                    ExtendedColorType::Rgba8,
                )
                .map_err(|err| TdxError::Other(format!("WebP encoding failed: {err}")))?;
        }
        OutputFormat::Bmp => {
            let rgb = flatten(image, background);
            rgb.write_to(&mut Cursor::new(&mut output), ImageFormat::Bmp)
                .map_err(|err| TdxError::Other(format!("BMP encoding failed: {err}")))?;
        }
        OutputFormat::Tiff => {
            let rgba = image.to_rgba8();
            rgba.write_to(&mut Cursor::new(&mut output), ImageFormat::Tiff)
                .map_err(|err| TdxError::Other(format!("TIFF encoding failed: {err}")))?;
        }
        OutputFormat::Avif => {
            let rgba = image.to_rgba8();
            rgba.write_to(&mut Cursor::new(&mut output), ImageFormat::Avif)
                .map_err(|err| TdxError::Other(format!("AVIF encoding failed: {err}")))?;
        }
    }
    Ok(output)
}

fn finish(
    input: &Path,
    output: &Path,
    bytes: &[u8],
    kind: FileKind,
    stats: impl FnOnce(&mut OperationResult),
) -> Result<OperationResult> {
    tdx_core::fsutil::atomic_write(output, |file| {
        std::io::Write::write_all(file, bytes).map_err(TdxError::Io)
    })?;
    let mut result = OperationResult::single(output.to_path_buf(), kind);
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    stats(&mut result);
    Ok(result.finalize())
}

/// Convert an image between bitmap formats.
pub fn convert(
    input: &Path,
    output: &Path,
    format: OutputFormat,
    quality: u8,
    background: Option<[u8; 3]>,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    sink.stage(Stage::Reading, 0.1);
    cancel.check()?;
    let image = decode_image(input)?;
    sink.stage(Stage::Processing, 0.6);
    let bytes = encode_image(&image, format, quality, background)?;
    sink.stage(Stage::Writing, 0.9);
    finish(input, output, &bytes, format.kind(), |result| {
        result.set_stat("width", image.width());
        result.set_stat("height", image.height());
        result.set_stat("format", format.extension());
    })
}

/// Resize an image.
#[allow(clippy::too_many_arguments)]
pub fn resize(
    input: &Path,
    output: &Path,
    width: Option<u32>,
    height: Option<u32>,
    mode: ResizeMode,
    format: OutputFormat,
    quality: u8,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    sink.stage(Stage::Reading, 0.1);
    cancel.check()?;
    let image = decode_image(input)?;
    let source_width = image.width();
    let source_height = image.height();
    let (target_width, target_height) = match (width, height) {
        (Some(w), Some(h)) => (w.max(1), h.max(1)),
        (Some(w), None) => {
            let w = w.max(1);
            (
                w,
                ((source_height as f32 * (w as f32 / source_width as f32)).round() as u32).max(1),
            )
        }
        (None, Some(h)) => {
            let h = h.max(1);
            (
                ((source_width as f32 * (h as f32 / source_height as f32)).round() as u32).max(1),
                h,
            )
        }
        (None, None) => {
            return Err(TdxError::InvalidInput(
                "Provide a target width or height".into(),
            ));
        }
    };

    sink.stage(Stage::Processing, 0.5);
    let resized = match mode {
        ResizeMode::Exact => image.resize_exact(
            target_width,
            target_height,
            image::imageops::FilterType::Lanczos3,
        ),
        ResizeMode::Contain => image.resize(
            target_width,
            target_height,
            image::imageops::FilterType::Lanczos3,
        ),
        ResizeMode::Cover => {
            let scaled = image.resize(
                target_width,
                target_height,
                image::imageops::FilterType::Lanczos3,
            );
            // resize() with Cover semantics would be resize_to_fill; use that for correct cropping.
            let _ = scaled;
            image.resize_to_fill(
                target_width,
                target_height,
                image::imageops::FilterType::Lanczos3,
            )
        }
    };
    let bytes = encode_image(&resized, format, quality, None)?;
    sink.stage(Stage::Writing, 0.9);
    finish(input, output, &bytes, format.kind(), |result| {
        result.set_stat("source_width", source_width);
        result.set_stat("source_height", source_height);
        result.set_stat("width", resized.width());
        result.set_stat("height", resized.height());
    })
}

/// Crop a rectangle.
#[allow(clippy::too_many_arguments)]
pub fn crop(
    input: &Path,
    output: &Path,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    format: OutputFormat,
    quality: u8,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    sink.stage(Stage::Reading, 0.1);
    cancel.check()?;
    let image = decode_image(input)?;
    if width == 0 || height == 0 {
        return Err(TdxError::InvalidInput(
            "Crop width and height must be positive".into(),
        ));
    }
    if x + width > image.width() || y + height > image.height() {
        return Err(TdxError::InvalidInput(format!(
            "The crop rectangle ({}x{} at {x},{y}) is outside the image ({}x{})",
            width,
            height,
            image.width(),
            image.height()
        )));
    }
    sink.stage(Stage::Processing, 0.5);
    let cropped = image.crop_imm(x, y, width, height);
    let bytes = encode_image(&cropped, format, quality, None)?;
    finish(input, output, &bytes, format.kind(), |result| {
        result.set_stat("width", cropped.width());
        result.set_stat("height", cropped.height());
    })
}

/// Rotate clockwise by 90/180/270 and/or flip.
#[allow(clippy::too_many_arguments)]
pub fn rotate_flip(
    input: &Path,
    output: &Path,
    degrees: i32,
    flip_horizontal: bool,
    flip_vertical: bool,
    format: Option<OutputFormat>,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    sink.stage(Stage::Reading, 0.1);
    cancel.check()?;
    let mut image = decode_image(input)?;
    let normalized = degrees.rem_euclid(360);
    if normalized % 90 != 0 {
        return Err(TdxError::InvalidInput(
            "Rotation must be a multiple of 90 degrees".into(),
        ));
    }
    image = match normalized {
        90 => image.rotate90(),
        180 => image.rotate180(),
        270 => image.rotate270(),
        _ => image,
    };
    if flip_horizontal {
        image = image.fliph();
    }
    if flip_vertical {
        image = image.flipv();
    }
    let format = match format {
        Some(format) => format,
        None => {
            let extension = input.extension().and_then(|e| e.to_str()).unwrap_or("png");
            OutputFormat::from_extension(extension).unwrap_or(OutputFormat::Png)
        }
    };
    let bytes = encode_image(&image, format, 92, None)?;
    finish(input, output, &bytes, format.kind(), |result| {
        result.set_stat("degrees", normalized);
        result.set_stat("width", image.width());
        result.set_stat("height", image.height());
    })
}

/// Re-encode an image without EXIF/text/auxiliary chunks.
pub fn strip_metadata(
    input: &Path,
    output: &Path,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    sink.stage(Stage::Reading, 0.1);
    cancel.check()?;
    let image = decode_image(input)?;
    let extension = input.extension().and_then(|e| e.to_str()).unwrap_or("png");
    let format = OutputFormat::from_extension(extension).unwrap_or(OutputFormat::Png);
    let bytes = encode_image(&image, format, 92, None)?;
    finish(input, output, &bytes, format.kind(), |result| {
        result.set_stat("width", image.width());
        result.set_stat("height", image.height());
    })
}

/// Create a Windows ICO icon.
pub fn to_ico(
    input: &Path,
    output: &Path,
    size: u32,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    sink.stage(Stage::Reading, 0.1);
    cancel.check()?;
    let image = decode_image(input)?;
    let size = size.clamp(16, 256);
    let square = image.resize_to_fill(size, size, image::imageops::FilterType::Lanczos3);
    let rgba = square.to_rgba8();
    let mut bytes = Vec::new();
    {
        let encoder = image::codecs::ico::IcoEncoder::new(&mut bytes);
        encoder
            .write_image(
                rgba.as_raw(),
                rgba.width(),
                rgba.height(),
                ExtendedColorType::Rgba8,
            )
            .map_err(|err| TdxError::Other(format!("ICO encoding failed: {err}")))?;
    }
    finish(input, output, &bytes, FileKind::Ico, |result| {
        result.set_stat("size", size);
    })
}

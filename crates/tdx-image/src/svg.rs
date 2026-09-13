//! SVG rasterization, raster→SVG embedding and vector tracing.

use std::path::Path;

use base64::Engine;
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg;
use serde::{Deserialize, Serialize};
use tdx_core::{
    detect::FileKind,
    error::{Result, TdxError},
    progress::{CancelToken, ProgressSink, Stage},
    result::OperationResult,
};

use crate::{encode_image, OutputFormat};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RasterFormat {
    Png,
    Jpeg,
    Webp,
}

impl RasterFormat {
    pub fn to_output_format(self) -> OutputFormat {
        match self {
            RasterFormat::Png => OutputFormat::Png,
            RasterFormat::Jpeg => OutputFormat::Jpeg,
            RasterFormat::Webp => OutputFormat::Webp,
        }
    }
}

/// Rasterize an SVG document.
#[allow(clippy::too_many_arguments)]
pub fn render_svg(
    input: &Path,
    output: &Path,
    width: Option<u32>,
    height: Option<u32>,
    format: RasterFormat,
    quality: u8,
    background: Option<[u8; 3]>,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    sink.stage(Stage::Reading, 0.1);
    cancel.check()?;
    let bytes = std::fs::read(input).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => TdxError::not_found(input),
        _ => TdxError::Io(err),
    })?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err(TdxError::InvalidInput(
            "SVG file is larger than 64 MB".into(),
        ));
    }
    let text = String::from_utf8(bytes)
        .map_err(|_| TdxError::Unsupported("The SVG file is not valid UTF-8".into()))?;

    let mut options = usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    let tree = usvg::Tree::from_str(&text, &options)
        .map_err(|err| TdxError::InvalidInput(format!("Invalid SVG: {err}")))?;

    let size = tree.size();
    let intrinsic_width = size.width();
    let intrinsic_height = size.height();
    if intrinsic_width <= 0.0 || intrinsic_height <= 0.0 {
        return Err(TdxError::InvalidInput(
            "The SVG has no intrinsic size".into(),
        ));
    }
    let (scale_x, scale_y) = match (width, height) {
        (Some(w), Some(h)) => (w as f32 / intrinsic_width, h as f32 / intrinsic_height),
        (Some(w), None) => {
            let scale = w as f32 / intrinsic_width;
            (scale, scale)
        }
        (None, Some(h)) => {
            let scale = h as f32 / intrinsic_height;
            (scale, scale)
        }
        (None, None) => (1.0, 1.0),
    };
    let pixel_width = (intrinsic_width * scale_x).round().max(1.0) as u32;
    let pixel_height = (intrinsic_height * scale_y).round().max(1.0) as u32;
    if pixel_width > 20_000 || pixel_height > 20_000 {
        return Err(TdxError::InvalidInput(
            "The requested output size is too large".into(),
        ));
    }

    sink.stage(Stage::Processing, 0.5);
    let mut pixmap = Pixmap::new(pixel_width, pixel_height)
        .ok_or_else(|| TdxError::Other("Cannot allocate the target bitmap".into()))?;
    resvg::render(
        &tree,
        Transform::from_scale(scale_x, scale_y),
        &mut pixmap.as_mut(),
    );

    let raw = pixmap.data().to_vec();
    let rgba = image::RgbaImage::from_raw(pixel_width, pixel_height, raw)
        .ok_or_else(|| TdxError::Other("Bitmap buffer size mismatch".into()))?;
    let dynamic = image::DynamicImage::ImageRgba8(rgba);
    sink.stage(Stage::Writing, 0.85);
    let encoded = encode_image(&dynamic, format.to_output_format(), quality, background)?;
    tdx_core::fsutil::atomic_write(output, |file| {
        std::io::Write::write_all(file, &encoded).map_err(TdxError::Io)
    })?;

    let mut result =
        OperationResult::single(output.to_path_buf(), format.to_output_format().kind());
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    result.set_stat("width", pixel_width);
    result.set_stat("height", pixel_height);
    Ok(result.finalize())
}

/// Wrap a raster image inside an SVG container (no tracing).
pub fn png_to_svg_embed(
    input: &Path,
    output: &Path,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    sink.stage(Stage::Reading, 0.1);
    cancel.check()?;
    let bytes = std::fs::read(input).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => TdxError::not_found(input),
        _ => TdxError::Io(err),
    })?;
    let (width, height) = image::image_dimensions(input)
        .map_err(|err| TdxError::Unsupported(format!("Cannot read image dimensions: {err}")))?;
    let extension = input
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_else(|| "png".to_string());
    let mime = match extension.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "gif" => "image/gif",
        _ => "image/png",
    };
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let svg = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n  <image width=\"{width}\" height=\"{height}\" xlink:href=\"data:{mime};base64,{encoded}\"/>\n</svg>\n"
    );
    sink.stage(Stage::Writing, 0.8);
    tdx_core::fsutil::atomic_write(output, |file| {
        std::io::Write::write_all(file, svg.as_bytes()).map_err(TdxError::Io)
    })?;
    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Svg);
    result.bytes_in = bytes.len() as u64;
    result.set_stat("width", width);
    result.set_stat("height", height);
    result.set_stat("mode", "embed");
    result.warn("Embed mode wraps the raster pixels in SVG; it is not vector tracing");
    Ok(result.finalize())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TracePreset {
    /// Black and white line art.
    Bw,
    /// Flat colors, posters and logos.
    Poster,
    /// Photographs, many color layers.
    Photo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceOptions {
    pub preset: TracePreset,
    pub filter_speckle: u32,
    pub color_precision: u8,
    pub corner_threshold: u16,
    pub length_threshold: f32,
    pub path_precision: u8,
}

impl Default for TraceOptions {
    fn default() -> Self {
        Self {
            preset: TracePreset::Poster,
            filter_speckle: 4,
            color_precision: 6,
            corner_threshold: 60,
            length_threshold: 4.0,
            path_precision: 2,
        }
    }
}

/// Trace a raster image into vector SVG using vtracer.
pub fn trace(
    input: &Path,
    output: &Path,
    options: &TraceOptions,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    sink.stage(Stage::Reading, 0.1);
    cancel.check()?;
    let preset = match options.preset {
        TracePreset::Bw => vtracer::Preset::Bw,
        TracePreset::Poster => vtracer::Preset::Poster,
        TracePreset::Photo => vtracer::Preset::Photo,
    };
    let mut config = vtracer::Config::from_preset(preset);
    config.filter_speckle = options.filter_speckle as usize;
    config.color_precision = options.color_precision as i32;
    config.corner_threshold = options.corner_threshold as i32;
    config.length_threshold = options.length_threshold as f64;
    config.path_precision = Some(options.path_precision as u32);

    let parent = output.parent().map(Path::to_path_buf).unwrap_or_default();
    std::fs::create_dir_all(&parent)?;
    let temp = parent.join(format!(".tdx-trace-{}.svg", uuid::Uuid::new_v4().simple()));
    sink.stage(Stage::Processing, 0.4);
    vtracer::convert_image_to_svg(input, &temp, config)
        .map_err(|err| TdxError::Other(format!("Tracing failed: {err}")))?;
    cancel.check()?;
    std::fs::rename(&temp, output).map_err(|err| {
        let _ = std::fs::remove_file(&temp);
        TdxError::Io(err)
    })?;

    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Svg);
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    result.set_stat("mode", "trace");
    result.set_stat(
        "preset",
        match options.preset {
            TracePreset::Bw => "bw",
            TracePreset::Poster => "poster",
            TracePreset::Photo => "photo",
        },
    );
    Ok(result.finalize())
}

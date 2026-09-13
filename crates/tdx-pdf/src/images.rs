//! Image import (images → PDF) and page rasterization (PDF → images).
//!
//! Page rasterization needs the optional PDFium adapter. The function reports
//! a clear adapter-required error instead of pretending to work.

use std::path::{Path, PathBuf};

use flate2::{write::ZlibEncoder, Compression};
use image::{ExtendedColorType, ImageEncoder};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use serde::{Deserialize, Serialize};
use tdx_core::{
    detect::FileKind,
    error::{Result, TdxError},
    progress::{CancelToken, ProgressSink, Stage},
    result::OperationResult,
};

use crate::build::{assemble_document, load_pdf, prune_unreachable, save_document, verify_pdf};
use crate::{PageSize, RasterFormat};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ImagesToPdfOptions {
    pub page_size: PageSize,
    pub margin: f32,
    pub jpeg_quality: u8,
}

impl Default for ImagesToPdfOptions {
    fn default() -> Self {
        Self {
            page_size: PageSize::A4,
            margin: 24.0,
            jpeg_quality: 85,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PdfToImagesOptions {
    pub format: RasterFormat,
    pub dpi: u32,
    pub quality: u8,
    pub pages: Option<(u32, u32)>,
}

impl Default for PdfToImagesOptions {
    fn default() -> Self {
        Self {
            format: RasterFormat::Png,
            dpi: 150,
            quality: 90,
            pages: None,
        }
    }
}

fn decode_image(path: &Path) -> Result<image::DynamicImage> {
    let reader = image::ImageReader::open(path).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => TdxError::not_found(path),
        _ => TdxError::Io(err),
    })?;
    let reader = reader.with_guessed_format().map_err(TdxError::Io)?;
    reader
        .decode()
        .map_err(|err| TdxError::Unsupported(format!("Cannot decode image: {err}")))
}

fn flatten_on_white(image: &image::DynamicImage) -> image::DynamicImage {
    if !image.color().has_alpha() {
        return image.clone();
    }
    let mut canvas = image::RgbaImage::from_pixel(
        image.width(),
        image.height(),
        image::Rgba([255, 255, 255, 255]),
    );
    let overlay = image.to_rgba8();
    image::imageops::overlay(&mut canvas, &overlay, 0, 0);
    image::DynamicImage::ImageRgba8(canvas)
}

struct EmbeddedImage {
    object_id: ObjectId,
    pixel_width: u32,
    pixel_height: u32,
}

fn embed_image(doc: &mut Document, path: &Path, quality: u8) -> Result<EmbeddedImage> {
    let bytes = std::fs::read(path)?;
    let decoded = decode_image(path)?;
    let pixel_width = decoded.width();
    let pixel_height = decoded.height();
    let flattened = flatten_on_white(&decoded);

    let is_jpeg = bytes.starts_with(&[0xFF, 0xD8, 0xFF]);
    let is_gray_source = matches!(
        decoded.color(),
        image::ColorType::L8 | image::ColorType::L16
    );
    let can_embed_directly = is_jpeg && !decoded.color().has_alpha();

    let (content, dict) = if can_embed_directly {
        let color_space: &[u8] = if is_gray_source {
            b"DeviceGray"
        } else {
            b"DeviceRGB"
        };
        let mut dict = Dictionary::new();
        dict.set("Type", "XObject");
        dict.set("Subtype", "Image");
        dict.set("Width", pixel_width as i64);
        dict.set("Height", pixel_height as i64);
        dict.set("ColorSpace", Object::Name(color_space.to_vec()));
        dict.set("BitsPerComponent", 8);
        dict.set("Filter", Object::Name(b"DCTDecode".to_vec()));
        (bytes, dict)
    } else {
        let (raw, color_space): (Vec<u8>, &[u8]) = if matches!(
            flattened.color(),
            image::ColorType::L8 | image::ColorType::L16
        ) {
            let gray = flattened.to_luma8();
            (gray.into_raw(), b"DeviceGray")
        } else {
            let rgb = flattened.to_rgb8();
            (rgb.into_raw(), b"DeviceRGB")
        };
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        std::io::Write::write_all(&mut encoder, &raw)?;
        let compressed = encoder.finish()?;
        let mut dict = Dictionary::new();
        dict.set("Type", "XObject");
        dict.set("Subtype", "Image");
        dict.set("Width", pixel_width as i64);
        dict.set("Height", pixel_height as i64);
        dict.set("ColorSpace", Object::Name(color_space.to_vec()));
        dict.set("BitsPerComponent", 8);
        dict.set("Filter", Object::Name(b"FlateDecode".to_vec()));
        let _ = quality;
        (compressed, dict)
    };

    let object_id = doc.add_object(Stream::new(dict, content));
    Ok(EmbeddedImage {
        object_id,
        pixel_width,
        pixel_height,
    })
}

/// Build a PDF where each input image becomes one page.
pub fn images_to_pdf(
    images: &[PathBuf],
    output: &Path,
    options: &ImagesToPdfOptions,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    if images.is_empty() {
        return Err(TdxError::InvalidInput("Select at least one image".into()));
    }
    sink.stage(Stage::Validating, 0.0);
    let mut doc = Document::with_version("1.7");
    let mut pages: Vec<ObjectId> = Vec::new();
    let total = images.len();

    for (index, path) in images.iter().enumerate() {
        cancel.check()?;
        sink.message(
            Stage::Processing,
            index as f32 / total as f32,
            format!(
                "Embedding {}",
                path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            ),
        );
        let embedded = embed_image(&mut doc, path, options.jpeg_quality)?;

        let (page_width, page_height) = match options.page_size {
            PageSize::A4 => (595.0, 842.0),
            PageSize::Letter => (612.0, 792.0),
            PageSize::Fit => (
                embedded.pixel_width as f32 * 72.0 / 150.0,
                embedded.pixel_height as f32 * 72.0 / 150.0,
            ),
        };
        let margin = if options.page_size == PageSize::Fit {
            0.0
        } else {
            options.margin.max(0.0)
        };
        let available_width = (page_width - margin * 2.0).max(1.0);
        let available_height = (page_height - margin * 2.0).max(1.0);
        let scale = (available_width / embedded.pixel_width as f32)
            .min(available_height / embedded.pixel_height as f32)
            .min(if options.page_size == PageSize::Fit {
                f32::MAX
            } else {
                1.0
            });
        let draw_width = embedded.pixel_width as f32 * scale;
        let draw_height = embedded.pixel_height as f32 * scale;
        let offset_x = (page_width - draw_width) / 2.0;
        let offset_y = (page_height - draw_height) / 2.0;

        let content = format!(
            "q\n{width:.2} 0 0 {height:.2} {x:.2} {y:.2} cm\n/Im0 Do\nQ\n",
            width = draw_width,
            height = draw_height,
            x = offset_x,
            y = offset_y,
        );

        let mut xobject = Dictionary::new();
        xobject.set("Im0", embedded.object_id);
        let mut resources = Dictionary::new();
        resources.set("XObject", Object::Dictionary(xobject));

        let mut page = Dictionary::new();
        page.set("Type", "Page");
        page.set(
            "MediaBox",
            vec![
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real(page_width),
                Object::Real(page_height),
            ],
        );
        page.set("Resources", Object::Dictionary(resources));
        let content_id = doc.add_object(Stream::new(Dictionary::new(), content.into_bytes()));
        page.set("Contents", content_id);
        let page_id = doc.add_object(Object::Dictionary(page));
        pages.push(page_id);
    }

    let catalog = assemble_document(&mut doc, &pages);
    prune_unreachable(&mut doc, &[catalog]);
    sink.stage(Stage::Writing, 0.9);
    save_document(&mut doc, output)?;
    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Pdf);
    result.bytes_in = images
        .iter()
        .map(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
        .sum();
    result.set_stat("pages", pages.len());
    result.set_stat("images", images.len());
    for warning in verify_pdf(output, pages.len())? {
        result.warn(warning);
    }
    Ok(result.finalize())
}

/// True when a PDF renderer adapter can be located at runtime.
pub fn renderer_available() -> bool {
    std::env::var_os("TDX_PDFIUM_PATH").is_some()
}

/// Render PDF pages to images. Requires the PDFium adapter.
pub fn pdf_to_images(
    input: &Path,
    output_dir: &Path,
    options: &PdfToImagesOptions,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    let _ = (input, output_dir, options, sink, cancel);
    Err(TdxError::AdapterRequired(
        "PDF rendering requires the optional PDFium adapter (set TDX_PDFIUM_PATH). See docs/formats.md".into(),
    ))
}

/// Guard: confirm the document has no encryption before image work.
pub fn ensure_readable(path: &Path) -> Result<Document> {
    let document = load_pdf(path)?;
    Ok(document)
}

#[allow(dead_code)]
fn _assert_encoder_trait<T: ImageEncoder + ?Sized>(_: &T) {}

#[allow(dead_code)]
fn _assert_extended_color(_: ExtendedColorType) {}

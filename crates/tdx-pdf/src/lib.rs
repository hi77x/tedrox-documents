//! PDF engine for TEDROX Documents.
//!
//! Implemented on top of the permissively licensed `lopdf` crate. No AGPL
//! engine is bundled; PDF rendering to images requires an optional PDFium
//! adapter that is documented in `docs/formats.md`.

pub mod annotate;
pub mod build;
pub mod compress;
pub mod forms;
pub mod images;
pub mod metadata;
pub mod organize;
pub mod pages;
pub mod selection;
pub mod watermark;

use std::path::Path;

use serde::{Deserialize, Serialize};
use tdx_core::error::{Result, TdxError};

pub use metadata::{MetadataUpdate, PdfMetadata};
pub use pages::SplitMode;
pub use selection::PageSelection;

/// Target page size for generated PDFs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageSize {
    A4,
    Letter,
    /// Page matches the image aspect ratio at 150 DPI.
    Fit,
}

impl PageSize {
    pub fn dimensions(self) -> (f32, f32) {
        match self {
            PageSize::A4 => (595.0, 842.0),
            PageSize::Letter => (612.0, 792.0),
            PageSize::Fit => (595.0, 842.0),
        }
    }
}

/// Output format when rendering pages or converting SVG.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RasterFormat {
    Png,
    Jpeg,
    Webp,
}

impl RasterFormat {
    pub fn extension(self) -> &'static str {
        match self {
            RasterFormat::Png => "png",
            RasterFormat::Jpeg => "jpg",
            RasterFormat::Webp => "webp",
        }
    }
}

/// Compression presets. Values are honest: they trade quality for size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompressPreset {
    /// 72 DPI, small pages, strong downsampling.
    Screen,
    /// 150 DPI, good default.
    Balanced,
    /// 300 DPI, mild compression.
    Print,
    Custom {
        quality: u8,
        max_dimension: u32,
    },
}

impl CompressPreset {
    pub fn jpeg_quality(self) -> u8 {
        match self {
            CompressPreset::Screen => 55,
            CompressPreset::Balanced => 72,
            CompressPreset::Print => 85,
            CompressPreset::Custom { quality, .. } => quality.clamp(20, 100),
        }
    }

    pub fn max_dimension(self) -> u32 {
        match self {
            CompressPreset::Screen => 1024,
            CompressPreset::Balanced => 1600,
            CompressPreset::Print => 2400,
            CompressPreset::Custom { max_dimension, .. } => max_dimension.clamp(256, 10_000),
        }
    }
}

/// Short structural summary of a PDF.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfInfo {
    pub page_count: u32,
    pub version: String,
    pub encrypted: bool,
    pub object_count: usize,
    pub file_size: u64,
}

/// Load a PDF, mapping engine errors to product errors.
pub fn load_pdf(path: &Path) -> Result<lopdf::Document> {
    match lopdf::Document::load(path) {
        Ok(document) => Ok(document),
        Err(lopdf::Error::Decryption(_)) => Err(TdxError::Encrypted),
        Err(lopdf::Error::IO(err)) if err.kind() == std::io::ErrorKind::NotFound => {
            Err(TdxError::not_found(path))
        }
        Err(lopdf::Error::IO(err)) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            Err(TdxError::Permission(path.display().to_string()))
        }
        Err(lopdf::Error::IO(err)) => Err(TdxError::Io(err)),
        Err(err) => Err(TdxError::Corrupt(err.to_string())),
    }
}

/// Load from memory with the same error mapping.
pub fn load_pdf_mem(bytes: &[u8]) -> Result<lopdf::Document> {
    match lopdf::Document::load_mem(bytes) {
        Ok(document) => Ok(document),
        Err(lopdf::Error::Decryption(_)) => Err(TdxError::Encrypted),
        Err(err) => Err(TdxError::Corrupt(err.to_string())),
    }
}

pub fn inspect(path: &Path) -> Result<PdfInfo> {
    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let document = load_pdf(path)?;
    Ok(PdfInfo {
        page_count: document.get_pages().len() as u32,
        version: document.version.clone(),
        encrypted: false,
        object_count: document.objects.len(),
        file_size,
    })
}

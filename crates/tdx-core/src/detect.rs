//! File type detection.
//!
//! Detection never relies on the extension alone. The priority order is:
//! 1. magic bytes (`infer`), 2. container inspection (OOXML inside ZIP),
//! 3. MIME hint, 4. extension, 5. loose text content sniffing.

use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Result, TdxError};

/// Broad product categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Pdf,
    Document,
    Spreadsheet,
    Image,
    Archive,
    Data,
    Unknown,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Pdf => "pdf",
            Category::Document => "document",
            Category::Spreadsheet => "spreadsheet",
            Category::Image => "image",
            Category::Archive => "archive",
            Category::Data => "data",
            Category::Unknown => "unknown",
        }
    }
}

/// Concrete file kinds the application understands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    Pdf,
    Docx,
    Doc,
    Odt,
    Rtf,
    Txt,
    Markdown,
    Html,
    Csv,
    Tsv,
    Xlsx,
    Xls,
    Ods,
    Json,
    Xml,
    Png,
    Jpeg,
    Webp,
    Gif,
    Bmp,
    Tiff,
    Svg,
    Ico,
    Avif,
    Zip,
    SevenZip,
    Tar,
    Gzip,
    Unknown,
}

/// How confident the detector is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    /// Verified by magic bytes.
    Magic,
    /// Verified by inspecting a container (for example OOXML inside ZIP).
    Container,
    /// Derived from the extension only.
    Extension,
    /// Derived from loose content sniffing.
    Content,
}

/// What a format can do in this product. Only set flags that are actually
/// implemented — see the capability matrix in the documentation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilitySet {
    pub open: bool,
    pub preview: bool,
    pub edit: bool,
    pub convert: bool,
    pub export: bool,
    pub merge: bool,
    pub split: bool,
    pub metadata: bool,
    pub batch: bool,
}

impl CapabilitySet {
    pub const fn new() -> Self {
        Self {
            open: false,
            preview: false,
            edit: false,
            convert: false,
            export: false,
            merge: false,
            split: false,
            metadata: false,
            batch: false,
        }
    }

    pub const fn open_preview() -> Self {
        Self {
            open: true,
            preview: true,
            ..Self::new()
        }
    }

    pub const fn full() -> Self {
        Self {
            open: true,
            preview: true,
            edit: true,
            convert: true,
            export: true,
            merge: true,
            split: true,
            metadata: true,
            batch: true,
        }
    }

    pub const fn with_edit(mut self) -> Self {
        self.edit = true;
        self
    }

    pub const fn with_convert(mut self) -> Self {
        self.convert = true;
        self.export = true;
        self
    }

    pub const fn with_split(mut self) -> Self {
        self.split = true;
        self
    }

    pub const fn with_merge(mut self) -> Self {
        self.merge = true;
        self
    }

    pub const fn with_metadata(mut self) -> Self {
        self.metadata = true;
        self
    }

    pub const fn with_batch(mut self) -> Self {
        self.batch = true;
        self
    }
}

impl FileKind {
    pub fn category(self) -> Category {
        match self {
            FileKind::Pdf => Category::Pdf,
            FileKind::Docx
            | FileKind::Doc
            | FileKind::Odt
            | FileKind::Rtf
            | FileKind::Txt
            | FileKind::Markdown
            | FileKind::Html => Category::Document,
            FileKind::Csv | FileKind::Tsv | FileKind::Xlsx | FileKind::Xls | FileKind::Ods => {
                Category::Spreadsheet
            }
            FileKind::Json | FileKind::Xml => Category::Data,
            FileKind::Png
            | FileKind::Jpeg
            | FileKind::Webp
            | FileKind::Gif
            | FileKind::Bmp
            | FileKind::Tiff
            | FileKind::Svg
            | FileKind::Ico
            | FileKind::Avif => Category::Image,
            FileKind::Zip | FileKind::SevenZip | FileKind::Tar | FileKind::Gzip => {
                Category::Archive
            }
            FileKind::Unknown => Category::Unknown,
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            FileKind::Pdf => "pdf",
            FileKind::Docx => "docx",
            FileKind::Doc => "doc",
            FileKind::Odt => "odt",
            FileKind::Rtf => "rtf",
            FileKind::Txt => "txt",
            FileKind::Markdown => "md",
            FileKind::Html => "html",
            FileKind::Csv => "csv",
            FileKind::Tsv => "tsv",
            FileKind::Xlsx => "xlsx",
            FileKind::Xls => "xls",
            FileKind::Ods => "ods",
            FileKind::Json => "json",
            FileKind::Xml => "xml",
            FileKind::Png => "png",
            FileKind::Jpeg => "jpg",
            FileKind::Webp => "webp",
            FileKind::Gif => "gif",
            FileKind::Bmp => "bmp",
            FileKind::Tiff => "tiff",
            FileKind::Svg => "svg",
            FileKind::Ico => "ico",
            FileKind::Avif => "avif",
            FileKind::Zip => "zip",
            FileKind::SevenZip => "7z",
            FileKind::Tar => "tar",
            FileKind::Gzip => "gz",
            FileKind::Unknown => "bin",
        }
    }

    pub fn mime(self) -> &'static str {
        match self {
            FileKind::Pdf => "application/pdf",
            FileKind::Docx => {
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            }
            FileKind::Doc => "application/msword",
            FileKind::Odt => "application/vnd.oasis.opendocument.text",
            FileKind::Rtf => "application/rtf",
            FileKind::Txt => "text/plain",
            FileKind::Markdown => "text/markdown",
            FileKind::Html => "text/html",
            FileKind::Csv => "text/csv",
            FileKind::Tsv => "text/tab-separated-values",
            FileKind::Xlsx => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            FileKind::Xls => "application/vnd.ms-excel",
            FileKind::Ods => "application/vnd.oasis.opendocument.spreadsheet",
            FileKind::Json => "application/json",
            FileKind::Xml => "application/xml",
            FileKind::Png => "image/png",
            FileKind::Jpeg => "image/jpeg",
            FileKind::Webp => "image/webp",
            FileKind::Gif => "image/gif",
            FileKind::Bmp => "image/bmp",
            FileKind::Tiff => "image/tiff",
            FileKind::Svg => "image/svg+xml",
            FileKind::Ico => "image/x-icon",
            FileKind::Avif => "image/avif",
            FileKind::Zip => "application/zip",
            FileKind::SevenZip => "application/x-7z-compressed",
            FileKind::Tar => "application/x-tar",
            FileKind::Gzip => "application/gzip",
            FileKind::Unknown => "application/octet-stream",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            FileKind::Pdf => "PDF",
            FileKind::Docx => "Word document (DOCX)",
            FileKind::Doc => "Legacy Word document (DOC)",
            FileKind::Odt => "OpenDocument text (ODT)",
            FileKind::Rtf => "Rich Text Format",
            FileKind::Txt => "Plain text",
            FileKind::Markdown => "Markdown",
            FileKind::Html => "HTML",
            FileKind::Csv => "CSV",
            FileKind::Tsv => "TSV",
            FileKind::Xlsx => "Excel workbook (XLSX)",
            FileKind::Xls => "Legacy Excel workbook (XLS)",
            FileKind::Ods => "OpenDocument spreadsheet (ODS)",
            FileKind::Json => "JSON",
            FileKind::Xml => "XML",
            FileKind::Png => "PNG image",
            FileKind::Jpeg => "JPEG image",
            FileKind::Webp => "WebP image",
            FileKind::Gif => "GIF image",
            FileKind::Bmp => "BMP image",
            FileKind::Tiff => "TIFF image",
            FileKind::Svg => "SVG image",
            FileKind::Ico => "ICO icon",
            FileKind::Avif => "AVIF image",
            FileKind::Zip => "ZIP archive",
            FileKind::SevenZip => "7z archive",
            FileKind::Tar => "TAR archive",
            FileKind::Gzip => "GZip archive",
            FileKind::Unknown => "Unknown file",
        }
    }

    pub fn from_extension(ext: &str) -> Option<Self> {
        let ext = ext.trim_start_matches('.').to_ascii_lowercase();
        Some(match ext.as_str() {
            "pdf" => FileKind::Pdf,
            "docx" => FileKind::Docx,
            "doc" => FileKind::Doc,
            "odt" => FileKind::Odt,
            "rtf" => FileKind::Rtf,
            "txt" | "text" | "log" => FileKind::Txt,
            "md" | "markdown" | "mdown" => FileKind::Markdown,
            "html" | "htm" => FileKind::Html,
            "csv" => FileKind::Csv,
            "tsv" | "tab" => FileKind::Tsv,
            "xlsx" => FileKind::Xlsx,
            "xls" => FileKind::Xls,
            "ods" => FileKind::Ods,
            "json" => FileKind::Json,
            "xml" => FileKind::Xml,
            "png" => FileKind::Png,
            "jpg" | "jpeg" | "jpe" => FileKind::Jpeg,
            "webp" => FileKind::Webp,
            "gif" => FileKind::Gif,
            "bmp" => FileKind::Bmp,
            "tif" | "tiff" => FileKind::Tiff,
            "svg" => FileKind::Svg,
            "ico" => FileKind::Ico,
            "avif" => FileKind::Avif,
            "zip" => FileKind::Zip,
            "7z" => FileKind::SevenZip,
            "tar" => FileKind::Tar,
            "gz" | "gzip" => FileKind::Gzip,
            _ => return None,
        })
    }

    /// Capabilities that are actually implemented today. Conservative on
    /// purpose: never advertise something untested.
    pub fn capabilities(self) -> CapabilitySet {
        match self {
            FileKind::Pdf => CapabilitySet {
                open: true,
                preview: true,
                edit: false,
                convert: true,
                export: true,
                merge: true,
                split: true,
                metadata: true,
                batch: true,
            },
            FileKind::Docx => CapabilitySet::open_preview().with_convert().with_edit(),
            FileKind::Doc => CapabilitySet::open_preview().with_convert(),
            FileKind::Odt | FileKind::Rtf => CapabilitySet::open_preview().with_convert(),
            FileKind::Txt => CapabilitySet::open_preview().with_convert().with_edit(),
            FileKind::Markdown => CapabilitySet::open_preview().with_convert().with_edit(),
            FileKind::Html => CapabilitySet::open_preview().with_convert(),
            FileKind::Csv | FileKind::Tsv => CapabilitySet::open_preview()
                .with_convert()
                .with_edit()
                .with_split()
                .with_batch(),
            FileKind::Xlsx | FileKind::Xls | FileKind::Ods => {
                CapabilitySet::open_preview().with_convert()
            }
            FileKind::Json | FileKind::Xml => CapabilitySet::open_preview().with_convert(),
            FileKind::Png
            | FileKind::Jpeg
            | FileKind::Webp
            | FileKind::Gif
            | FileKind::Bmp
            | FileKind::Tiff
            | FileKind::Ico
            | FileKind::Avif => CapabilitySet::open_preview()
                .with_convert()
                .with_edit()
                .with_batch(),
            FileKind::Svg => CapabilitySet::open_preview().with_convert(),
            FileKind::Zip => CapabilitySet::open_preview(),
            FileKind::SevenZip | FileKind::Tar | FileKind::Gzip => CapabilitySet::new(),
            FileKind::Unknown => CapabilitySet::new(),
        }
    }

    pub fn is_image(self) -> bool {
        self.category() == Category::Image && self != FileKind::Svg
    }

    pub fn is_text_based(self) -> bool {
        matches!(
            self,
            FileKind::Txt
                | FileKind::Markdown
                | FileKind::Html
                | FileKind::Csv
                | FileKind::Tsv
                | FileKind::Json
                | FileKind::Xml
                | FileKind::Svg
        )
    }
}

/// Result of a detection pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedFile {
    pub kind: FileKind,
    pub mime: String,
    pub size: u64,
    pub extension: Option<String>,
    pub confidence: Confidence,
    pub name: Option<String>,
}

impl DetectedFile {
    pub fn category(&self) -> Category {
        self.kind.category()
    }
}

fn kind_from_infer(extension: &str, mime: &str) -> Option<FileKind> {
    if let Some(kind) = FileKind::from_extension(extension) {
        return Some(kind);
    }
    Some(match mime {
        "application/pdf" => FileKind::Pdf,
        "application/msword" => FileKind::Doc,
        "application/rtf" => FileKind::Rtf,
        "application/vnd.ms-excel" => FileKind::Xls,
        "application/json" => FileKind::Json,
        "image/jpeg" => FileKind::Jpeg,
        "image/tiff" => FileKind::Tiff,
        _ => return None,
    })
}

fn contains_svg_marker(bytes: &[u8]) -> bool {
    String::from_utf8_lossy(bytes)
        .to_ascii_lowercase()
        .contains("<svg")
}

fn looks_like_ooxml(prefix: &[u8]) -> Option<FileKind> {
    // OOXML files are ZIP containers. We scan the first bytes for the
    // characteristic part names. This avoids a full ZIP parse in the detector.
    let text = String::from_utf8_lossy(prefix);
    if text.contains("word/document.xml") || text.contains("word\\document.xml") {
        return Some(FileKind::Docx);
    }
    if text.contains("xl/workbook.xml") || text.contains("xl\\workbook.xml") {
        return Some(FileKind::Xlsx);
    }
    if text.contains("mimetypeapplication/vnd.oasis.opendocument.text") {
        return Some(FileKind::Odt);
    }
    if text.contains("mimetypeapplication/vnd.oasis.opendocument.spreadsheet") {
        return Some(FileKind::Ods);
    }
    None
}

fn sniff_text(bytes: &[u8]) -> (FileKind, Confidence) {
    let sample_len = bytes.len().min(8192);
    let sample = &bytes[..sample_len];
    let text: String = match std::str::from_utf8(sample) {
        Ok(value) => value.to_string(),
        Err(_) => {
            let lossy = String::from_utf8_lossy(sample);
            if lossy
                .chars()
                .filter(|c| c.is_control() && *c != '\n' && *c != '\r' && *c != '\t')
                .count()
                > sample_len / 32
            {
                return (FileKind::Unknown, Confidence::Content);
            }
            lossy.into_owned()
        }
    };
    let trimmed = text.trim_start_matches('\u{feff}').trim_start();
    let lower = trimmed.to_ascii_lowercase();

    if lower.starts_with("<!doctype html") || lower.starts_with("<html") {
        return (FileKind::Html, Confidence::Content);
    }
    if lower.starts_with("<?xml") || lower.starts_with("<svg") {
        if lower.contains("<svg") {
            return (FileKind::Svg, Confidence::Content);
        }
        return (FileKind::Xml, Confidence::Content);
    }
    if (trimmed.starts_with('{') || trimmed.starts_with('['))
        && serde_json::from_str::<serde_json::Value>(trimmed).is_ok()
    {
        return (FileKind::Json, Confidence::Content);
    }
    if trimmed.starts_with("%PDF-") {
        return (FileKind::Pdf, Confidence::Magic);
    }
    if lower.starts_with("{\\rtf") {
        return (FileKind::Rtf, Confidence::Magic);
    }

    // Delimiter heuristic for delimited text.
    let lines: Vec<&str> = trimmed.lines().take(5).collect();
    if let Some(first_line) = lines.first() {
        let commas = first_line.matches(',').count();
        let tabs = first_line.matches('\t').count();
        let semicolons = first_line.matches(';').count();
        if tabs >= 2 && tabs >= commas {
            return (FileKind::Tsv, Confidence::Content);
        }
        if commas >= 1 {
            let consistent = lines
                .iter()
                .filter(|line| line.matches(',').count() == commas)
                .count();
            if consistent == lines.len() {
                return (FileKind::Csv, Confidence::Content);
            }
        }
        if semicolons >= 1 && commas == 0 {
            let consistent = lines
                .iter()
                .filter(|line| line.matches(';').count() == semicolons)
                .count();
            if consistent == lines.len() {
                return (FileKind::Csv, Confidence::Content);
            }
        }
        if first_line.starts_with("# ") || lower.contains("\n## ") {
            return (FileKind::Markdown, Confidence::Content);
        }
    }
    (FileKind::Txt, Confidence::Content)
}

/// Detect a file on disk.
pub fn detect(path: &Path) -> Result<DetectedFile> {
    let meta = std::fs::metadata(path).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => TdxError::not_found(path),
        std::io::ErrorKind::PermissionDenied => TdxError::Permission(path.display().to_string()),
        _ => TdxError::Io(err),
    })?;
    if !meta.is_file() {
        return Err(TdxError::InvalidInput(format!(
            "Not a regular file: {}",
            path.display()
        )));
    }
    let size = meta.len();
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.to_string());

    let mut head = [0u8; 8192];
    let read = std::fs::File::open(path)
        .and_then(|mut file| file.read(&mut head))
        .unwrap_or(0);
    let head = &head[..read];

    // 1. Magic bytes.
    if let Some(kind) = infer::get(head).and_then(|t| kind_from_infer(t.extension(), t.mime_type()))
    {
        let kind = match kind {
            FileKind::Zip => looks_like_ooxml(head).unwrap_or(FileKind::Zip),
            FileKind::Xml if contains_svg_marker(head) => FileKind::Svg,
            other => other,
        };
        return Ok(DetectedFile {
            kind,
            mime: kind.mime().to_string(),
            size,
            extension,
            confidence: Confidence::Magic,
            name,
        });
    }

    // 2. Container inspection for ZIP-like files without a strong magic match.
    if head.starts_with(b"PK\x03\x04") {
        if let Some(kind) = looks_like_ooxml(head) {
            return Ok(DetectedFile {
                kind,
                mime: kind.mime().to_string(),
                size,
                extension,
                confidence: Confidence::Container,
                name,
            });
        }
    }

    // 3. Extension mapping.
    if let Some(ext) = &extension {
        if let Some(kind) = FileKind::from_extension(ext) {
            return Ok(DetectedFile {
                kind,
                mime: kind.mime().to_string(),
                size,
                extension: Some(ext.clone()),
                confidence: Confidence::Extension,
                name,
            });
        }
    }

    // 4. Content sniffing.
    let (kind, confidence) = sniff_text(head);
    Ok(DetectedFile {
        kind,
        mime: kind.mime().to_string(),
        size,
        extension,
        confidence,
        name,
    })
}

/// Detect from an in-memory buffer (used by tests and the Android bridge).
pub fn detect_bytes(name_hint: Option<&str>, bytes: &[u8]) -> DetectedFile {
    let extension = name_hint.and_then(|name| {
        Path::new(name)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
    });
    let mut kind = None;
    let mut confidence = Confidence::Content;

    if let Some(t) = infer::get(bytes) {
        if let Some(candidate) = kind_from_infer(t.extension(), t.mime_type()) {
            kind = Some(match candidate {
                FileKind::Xml if contains_svg_marker(bytes) => FileKind::Svg,
                other => other,
            });
            confidence = Confidence::Magic;
        }
    }
    if matches!(kind, Some(FileKind::Zip)) || bytes.starts_with(b"PK\x03\x04") {
        if let Some(ooxml) = looks_like_ooxml(&bytes[..bytes.len().min(8192)]) {
            kind = Some(ooxml);
            confidence = Confidence::Container;
        }
    }
    if kind.is_none() {
        if let Some(ext) = &extension {
            if let Some(candidate) = FileKind::from_extension(ext) {
                kind = Some(candidate);
                confidence = Confidence::Extension;
            }
        }
    }
    if kind.is_none() {
        let (candidate, sniff_confidence) = sniff_text(bytes);
        kind = Some(candidate);
        confidence = sniff_confidence;
    }
    let kind = kind.unwrap_or(FileKind::Unknown);
    DetectedFile {
        kind,
        mime: kind.mime().to_string(),
        size: bytes.len() as u64,
        extension,
        confidence,
        name: name_hint.map(|n| n.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_markdown_extension() {
        let detected = detect_bytes(Some("notes.md"), b"# Hello\n\nWorld");
        assert_eq!(detected.kind, FileKind::Markdown);
    }

    #[test]
    fn detects_json_content() {
        let detected = detect_bytes(None, br#"{"a": 1}"#);
        assert_eq!(detected.kind, FileKind::Json);
    }

    #[test]
    fn detects_svg_content() {
        let detected = detect_bytes(
            None,
            br#"<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg"></svg>"#,
        );
        assert_eq!(detected.kind, FileKind::Svg);
    }

    #[test]
    fn detects_tsv_content() {
        let detected = detect_bytes(None, b"a\tb\tc\n1\t2\t3\n");
        assert_eq!(detected.kind, FileKind::Tsv);
    }

    #[test]
    fn detects_csv_content() {
        let detected = detect_bytes(None, b"name,age\nada,36\n");
        assert_eq!(detected.kind, FileKind::Csv);
    }

    #[test]
    fn detects_html_content() {
        let detected = detect_bytes(None, b"<!doctype html><html><body>x</body></html>");
        assert_eq!(detected.kind, FileKind::Html);
    }

    #[test]
    fn categories_are_sane() {
        assert_eq!(FileKind::Pdf.category(), Category::Pdf);
        assert_eq!(FileKind::Png.category(), Category::Image);
        assert!(FileKind::Png.is_image());
        assert!(!FileKind::Svg.is_image());
    }
}

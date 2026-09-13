//! Capability metadata. The registry is the single source of truth for what
//! the application can do; UI and CLI read it instead of hardcoding menus.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostClass {
    Light,
    Moderate,
    Heavy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlatformSupport {
    All,
    DesktopOnly,
    AndroidOnly,
}

#[derive(Debug, Clone, Serialize)]
pub struct OperationDescriptor {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub summary: &'static str,
    pub accepted: &'static [&'static str],
    pub outputs: &'static [&'static str],
    pub cost: CostClass,
    pub streaming: bool,
    pub cancellable: bool,
    pub platforms: PlatformSupport,
    pub experimental: bool,
}

#[derive(Debug, Clone, Default)]
pub struct OperationRegistry {
    operations: Vec<OperationDescriptor>,
}

macro_rules! op {
    ($id:expr, $name:expr, $category:expr, $summary:expr, $accepted:expr, $outputs:expr, $cost:expr, $streaming:expr, $cancellable:expr, $platforms:expr) => {
        OperationDescriptor {
            id: $id,
            name: $name,
            category: $category,
            summary: $summary,
            accepted: $accepted,
            outputs: $outputs,
            cost: $cost,
            streaming: $streaming,
            cancellable: $cancellable,
            platforms: $platforms,
            experimental: false,
        }
    };
}

impl OperationRegistry {
    pub fn builtin() -> Self {
        use CostClass::*;
        use PlatformSupport::*;

        let operations = vec![
            // PDF
            op!(
                "pdf.merge",
                "Merge PDFs",
                "pdf",
                "Combine multiple PDF files into one document.",
                &["pdf"],
                &["pdf"],
                Light,
                false,
                true,
                All
            ),
            op!(
                "pdf.split",
                "Split PDF",
                "pdf",
                "Split a PDF into individual pages or fixed ranges.",
                &["pdf"],
                &["pdf"],
                Light,
                false,
                true,
                All
            ),
            op!(
                "pdf.extract",
                "Extract pages",
                "pdf",
                "Create a new PDF from a page selection.",
                &["pdf"],
                &["pdf"],
                Light,
                false,
                true,
                All
            ),
            op!(
                "pdf.delete_pages",
                "Delete pages",
                "pdf",
                "Remove selected pages from a PDF.",
                &["pdf"],
                &["pdf"],
                Light,
                false,
                true,
                All
            ),
            op!(
                "pdf.reorder",
                "Reorder pages",
                "pdf",
                "Rewrite a PDF with a new page order.",
                &["pdf"],
                &["pdf"],
                Light,
                false,
                true,
                All
            ),
            op!(
                "pdf.reverse",
                "Reverse pages",
                "pdf",
                "Reverse the page order of a PDF.",
                &["pdf"],
                &["pdf"],
                Light,
                false,
                true,
                All
            ),
            op!(
                "pdf.rotate",
                "Rotate pages",
                "pdf",
                "Rotate all or selected pages by 90/180/270 degrees.",
                &["pdf"],
                &["pdf"],
                Light,
                false,
                true,
                All
            ),
            op!(
                "pdf.odd_even",
                "Extract odd/even pages",
                "pdf",
                "Extract only odd or only even pages.",
                &["pdf"],
                &["pdf"],
                Light,
                false,
                true,
                All
            ),
            op!(
                "pdf.metadata_view",
                "View PDF metadata",
                "pdf",
                "Read document properties and file statistics.",
                &["pdf"],
                &[],
                Light,
                false,
                true,
                All
            ),
            op!(
                "pdf.metadata_clean",
                "Privacy clean",
                "pdf",
                "Remove document information, XMP metadata, JavaScript and attachments.",
                &["pdf"],
                &["pdf"],
                Light,
                false,
                true,
                All
            ),
            op!(
                "pdf.metadata_set",
                "Edit PDF metadata",
                "pdf",
                "Change title, author, subject and keywords.",
                &["pdf"],
                &["pdf"],
                Light,
                false,
                true,
                All
            ),
            op!(
                "pdf.compress",
                "Compress PDF",
                "pdf",
                "Re-encode embedded images with a quality preset to reduce size.",
                &["pdf"],
                &["pdf"],
                Heavy,
                false,
                true,
                All
            ),
            op!(
                "pdf.watermark",
                "Watermark",
                "pdf",
                "Stamp a text watermark on every page.",
                &["pdf"],
                &["pdf"],
                Light,
                false,
                true,
                All
            ),
            op!(
                "pdf.images_to_pdf",
                "Images to PDF",
                "pdf",
                "Build a PDF from PNG, JPEG, WebP, BMP or TIFF images.",
                &["png", "jpg", "jpeg", "webp", "bmp", "tiff"],
                &["pdf"],
                Moderate,
                true,
                true,
                All
            ),
            op!(
                "pdf.to_images",
                "PDF to images",
                "pdf",
                "Render PDF pages to PNG or JPEG files.",
                &["pdf"],
                &["png", "jpg"],
                Heavy,
                false,
                true,
                All
            ),
            op!(
                "pdf.inspect",
                "Inspect PDF",
                "pdf",
                "Page count, version, encryption state and structure summary.",
                &["pdf"],
                &[],
                Light,
                true,
                true,
                All
            ),
            // Documents
            op!(
                "doc.create",
                "Create document",
                "documents",
                "Create a new DOCX document from plain text or Markdown.",
                &["md", "txt"],
                &["docx"],
                Light,
                false,
                true,
                All
            ),
            op!(
                "doc.extract_text",
                "Extract text",
                "documents",
                "Extract plain text from DOCX, Markdown, HTML, RTF or text files.",
                &["docx", "md", "html", "rtf", "txt"],
                &["txt"],
                Light,
                true,
                true,
                All
            ),
            op!(
                "doc.to_markdown",
                "Convert to Markdown",
                "documents",
                "Convert DOCX or HTML to Markdown (best effort).",
                &["docx", "html"],
                &["md"],
                Light,
                true,
                true,
                All
            ),
            op!(
                "doc.legacy_import",
                "Import legacy DOC",
                "documents",
                "Convert legacy .doc files through the optional LibreOffice adapter.",
                &["doc", "xls"],
                &["docx", "xlsx"],
                Heavy,
                false,
                true,
                DesktopOnly
            ),
            // Markdown / text to PDF
            op!(
                "convert.md_to_pdf",
                "Markdown to PDF",
                "convert",
                "Render Markdown to a clean, paginated PDF.",
                &["md"],
                &["pdf"],
                Moderate,
                false,
                true,
                All
            ),
            op!(
                "convert.txt_to_pdf",
                "Text to PDF",
                "convert",
                "Render plain text to a paginated PDF.",
                &["txt"],
                &["pdf"],
                Light,
                false,
                true,
                All
            ),
            op!(
                "convert.docx_to_pdf",
                "DOCX to PDF",
                "convert",
                "Convert DOCX to PDF through the native OOXML engine.",
                &["docx"],
                &["pdf"],
                Moderate,
                false,
                true,
                All
            ),
            op!(
                "convert.html_to_pdf",
                "HTML to PDF",
                "convert",
                "Render HTML to PDF (text-first rendering, no scripts).",
                &["html"],
                &["pdf"],
                Moderate,
                false,
                true,
                All
            ),
            // Spreadsheets
            op!(
                "sheet.inspect",
                "Inspect workbook",
                "sheets",
                "List sheets, dimensions and cell statistics.",
                &["xlsx", "xls", "ods", "csv", "tsv"],
                &[],
                Light,
                true,
                true,
                All
            ),
            op!(
                "sheet.csv_info",
                "CSV report",
                "csv",
                "Delimiter, encoding, columns, row count and type inference.",
                &["csv", "tsv"],
                &[],
                Light,
                true,
                true,
                All
            ),
            op!(
                "sheet.csv_to_xlsx",
                "CSV to XLSX",
                "csv",
                "Convert delimited text to an Excel workbook.",
                &["csv", "tsv"],
                &["xlsx"],
                Light,
                true,
                true,
                All
            ),
            op!(
                "sheet.xlsx_to_csv",
                "XLSX to CSV",
                "csv",
                "Export a worksheet to CSV.",
                &["xlsx", "ods"],
                &["csv"],
                Light,
                true,
                true,
                All
            ),
            op!(
                "sheet.csv_to_json",
                "CSV to JSON",
                "csv",
                "Convert CSV rows to a JSON array.",
                &["csv", "tsv"],
                &["json"],
                Light,
                true,
                true,
                All
            ),
            op!(
                "sheet.json_to_csv",
                "JSON to CSV",
                "csv",
                "Convert a JSON array of objects to CSV.",
                &["json"],
                &["csv"],
                Light,
                true,
                true,
                All
            ),
            op!(
                "sheet.csv_transform",
                "Transform CSV",
                "csv",
                "Filter, sort, deduplicate, trim, rename and select columns.",
                &["csv", "tsv"],
                &["csv"],
                Moderate,
                true,
                true,
                All
            ),
            op!(
                "sheet.csv_split",
                "Split CSV",
                "csv",
                "Split a CSV by row count or by a column value.",
                &["csv", "tsv"],
                &["csv"],
                Moderate,
                true,
                true,
                All
            ),
            op!(
                "sheet.csv_join",
                "Join CSV files",
                "csv",
                "Concatenate several CSV files with matching headers.",
                &["csv", "tsv"],
                &["csv"],
                Moderate,
                true,
                true,
                All
            ),
            // Images
            op!(
                "image.convert",
                "Convert image",
                "images",
                "Convert between PNG, JPEG, WebP, BMP, TIFF and AVIF.",
                &["png", "jpg", "jpeg", "webp", "bmp", "tiff", "avif", "gif", "ico"],
                &["png", "jpg", "webp", "bmp", "tiff", "avif"],
                Light,
                true,
                true,
                All
            ),
            op!(
                "image.resize",
                "Resize image",
                "images",
                "Resize with aspect-ratio preserving fit modes.",
                &["png", "jpg", "jpeg", "webp", "bmp", "tiff"],
                &["png", "jpg", "webp"],
                Light,
                true,
                true,
                All
            ),
            op!(
                "image.crop",
                "Crop image",
                "images",
                "Crop a rectangular region.",
                &["png", "jpg", "jpeg", "webp", "bmp", "tiff"],
                &["png", "jpg", "webp"],
                Light,
                true,
                true,
                All
            ),
            op!(
                "image.rotate_flip",
                "Rotate / flip",
                "images",
                "Rotate by 90/180/270 degrees and flip horizontally or vertically.",
                &["png", "jpg", "jpeg", "webp", "bmp", "tiff"],
                &["png", "jpg", "webp"],
                Light,
                true,
                true,
                All
            ),
            op!(
                "image.strip_metadata",
                "Strip image metadata",
                "images",
                "Remove EXIF and auxiliary chunks.",
                &["png", "jpg", "jpeg", "webp"],
                &["png", "jpg", "webp"],
                Light,
                true,
                true,
                All
            ),
            op!(
                "image.svg_render",
                "SVG to raster",
                "images",
                "Rasterize SVG to PNG, JPEG or WebP.",
                &["svg"],
                &["png", "jpg", "webp"],
                Moderate,
                false,
                true,
                All
            ),
            op!(
                "image.png_to_svg_embed",
                "PNG to SVG (embed)",
                "images",
                "Wrap a raster image inside an SVG container. Not vector tracing.",
                &["png", "jpg", "jpeg", "webp", "bmp"],
                &["svg"],
                Light,
                false,
                true,
                All
            ),
            op!(
                "image.png_to_svg_trace",
                "Raster to SVG (trace)",
                "images",
                "Vector tracing with color quantization and path simplification.",
                &["png", "jpg", "jpeg", "webp", "bmp"],
                &["svg"],
                Heavy,
                false,
                true,
                All
            ),
            op!(
                "image.to_ico",
                "Create ICO",
                "images",
                "Build a multi-size Windows icon from an image.",
                &["png", "jpg", "jpeg", "webp", "bmp"],
                &["ico"],
                Light,
                true,
                true,
                All
            ),
            // Archives and files
            op!(
                "archive.zip_create",
                "Create ZIP",
                "files",
                "Zip files and folders with safe path normalization.",
                &["*"],
                &["zip"],
                Moderate,
                true,
                true,
                All
            ),
            op!(
                "archive.zip_extract",
                "Extract ZIP",
                "files",
                "Extract an archive with zip-slip and bomb protection.",
                &["zip"],
                &["*"],
                Moderate,
                true,
                true,
                All
            ),
            op!(
                "archive.zip_list",
                "List archive",
                "files",
                "List archive entries, sizes and compression ratios.",
                &["zip"],
                &[],
                Light,
                true,
                true,
                All
            ),
            op!(
                "file.hash",
                "File hashes",
                "files",
                "Compute SHA-256 and BLAKE-less MD5-free content hashes.",
                &["*"],
                &[],
                Moderate,
                true,
                true,
                All
            ),
            op!(
                "file.batch_rename",
                "Batch rename",
                "files",
                "Rename many files with patterns and numbering.",
                &["*"],
                &["*"],
                Light,
                true,
                true,
                All
            ),
            op!(
                "file.duplicates",
                "Find duplicates",
                "files",
                "Group files by content hash to find duplicates.",
                &["*"],
                &[],
                Moderate,
                true,
                true,
                All
            ),
        ];
        Self { operations }
    }

    pub fn all(&self) -> &[OperationDescriptor] {
        &self.operations
    }

    pub fn get(&self, id: &str) -> Option<&OperationDescriptor> {
        self.operations.iter().find(|op| op.id == id)
    }

    pub fn by_category(&self, category: &str) -> Vec<&OperationDescriptor> {
        self.operations
            .iter()
            .filter(|op| op.category == category)
            .collect()
    }

    pub fn categories(&self) -> Vec<&'static str> {
        let mut categories: Vec<&'static str> =
            self.operations.iter().map(|op| op.category).collect();
        categories.sort_unstable();
        categories.dedup();
        categories
    }
}

impl OperationDescriptor {
    pub fn supports_extension(&self, extension: &str) -> bool {
        let extension = extension.trim_start_matches('.').to_ascii_lowercase();
        self.accepted
            .iter()
            .any(|item| *item == "*" || *item == extension)
    }

    pub fn platforms_include_android(&self) -> bool {
        self.platforms != PlatformSupport::DesktopOnly
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_no_duplicate_ids() {
        let registry = OperationRegistry::builtin();
        let mut ids: Vec<&str> = registry.all().iter().map(|op| op.id).collect();
        ids.sort_unstable();
        let mut deduped = ids.clone();
        deduped.dedup();
        assert_eq!(ids, deduped);
    }

    #[test]
    fn unknown_operation_is_none() {
        assert!(OperationRegistry::builtin()
            .get("pdf.nonexistent")
            .is_none());
    }
}

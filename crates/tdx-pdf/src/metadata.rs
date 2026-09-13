//! PDF metadata read/write and privacy cleaning.

use std::path::Path;

use lopdf::{Dictionary, Document, Object, StringFormat};
use serde::{Deserialize, Serialize};
use tdx_core::{
    detect::FileKind,
    error::{Result, TdxError},
    progress::{CancelToken, ProgressSink, Stage},
    result::OperationResult,
};

use crate::build::{load_pdf, save_document, verify_pdf};

/// Metadata fields surfaced by the viewer.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PdfMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Option<String>,
    pub creator: Option<String>,
    pub producer: Option<String>,
    pub creation_date: Option<String>,
    pub modification_date: Option<String>,
    pub page_count: u32,
    pub version: String,
    pub file_size: u64,
    pub encrypted: bool,
    pub has_xmp: bool,
    pub has_javascript: bool,
    pub has_embedded_files: bool,
    pub raw_entries: usize,
}

/// Fields that can be updated by the user.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetadataUpdate {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Option<String>,
}

fn decode_pdf_bytes(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    } else if bytes.starts_with(&[0xFF, 0xFE]) {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        bytes.iter().map(|byte| *byte as char).collect()
    }
}

fn encode_pdf_text(value: &str) -> Object {
    if value.is_ascii() {
        Object::String(value.as_bytes().to_vec(), StringFormat::Literal)
    } else {
        let mut bytes = vec![0xFE, 0xFF];
        for unit in value.encode_utf16() {
            bytes.extend_from_slice(&unit.to_be_bytes());
        }
        Object::String(bytes, StringFormat::Literal)
    }
}

fn object_text(object: &Object) -> Option<String> {
    match object {
        Object::String(bytes, _) => Some(decode_pdf_bytes(bytes)),
        Object::Name(name) => Some(String::from_utf8_lossy(name).to_string()),
        _ => None,
    }
}

fn info_dictionary(doc: &Document) -> Option<&Dictionary> {
    let info = doc.trailer.get(b"Info").ok()?;
    match info {
        Object::Reference(id) => doc.get_dictionary(*id).ok(),
        Object::Dictionary(dict) => Some(dict),
        _ => None,
    }
}

/// Read metadata without modifying the file.
pub fn read_metadata(path: &Path) -> Result<PdfMetadata> {
    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let document = load_pdf(path)?;
    let page_count = document.get_pages().len() as u32;
    let mut metadata = PdfMetadata {
        page_count,
        version: document.version.clone(),
        file_size,
        ..PdfMetadata::default()
    };

    if let Some(info) = info_dictionary(&document) {
        metadata.raw_entries = info.len();
        metadata.title = info.get(b"Title").ok().and_then(object_text);
        metadata.author = info.get(b"Author").ok().and_then(object_text);
        metadata.subject = info.get(b"Subject").ok().and_then(object_text);
        metadata.keywords = info.get(b"Keywords").ok().and_then(object_text);
        metadata.creator = info.get(b"Creator").ok().and_then(object_text);
        metadata.producer = info.get(b"Producer").ok().and_then(object_text);
        metadata.creation_date = info.get(b"CreationDate").ok().and_then(object_text);
        metadata.modification_date = info.get(b"ModDate").ok().and_then(object_text);
    }

    if let Ok(root) = document.trailer.get(b"Root").and_then(Object::as_reference) {
        if let Ok(catalog) = document.get_dictionary(root) {
            metadata.has_xmp = catalog.has(b"Metadata");
            if let Ok(names) = catalog.get(b"Names").and_then(Object::as_dict) {
                metadata.has_javascript = names.has(b"JavaScript");
                metadata.has_embedded_files = names.has(b"EmbeddedFiles");
            }
        }
    }
    Ok(metadata)
}

/// Write an Info dictionary with the given values (empty strings clear keys).
pub fn set_metadata(
    input: &Path,
    output: &Path,
    update: &MetadataUpdate,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    sink.stage(Stage::Reading, 0.0);
    cancel.check()?;
    let mut document = load_pdf(input)?;
    let page_count = document.get_pages().len() as u32;

    let info_id = match document
        .trailer
        .get(b"Info")
        .ok()
        .and_then(|obj| obj.as_reference().ok())
    {
        Some(id) => id,
        None => {
            let id = document.add_object(Dictionary::new());
            document.trailer.set("Info", id);
            id
        }
    };

    {
        let object = document
            .get_object_mut(info_id)
            .map_err(|_| TdxError::Corrupt("Info dictionary is missing".into()))?;
        let dict = object
            .as_dict_mut()
            .map_err(|_| TdxError::Corrupt("Info object is not a dictionary".into()))?;
        for (key, value) in [
            (b"Title".as_slice(), &update.title),
            (b"Author".as_slice(), &update.author),
            (b"Subject".as_slice(), &update.subject),
            (b"Keywords".as_slice(), &update.keywords),
        ] {
            match value {
                Some(text) if !text.trim().is_empty() => {
                    dict.set(key.to_vec(), encode_pdf_text(text))
                }
                Some(_) => {
                    dict.remove(key);
                }
                None => {}
            }
        }
        dict.set("Producer", encode_pdf_text("TEDROX Documents"));
    }

    sink.stage(Stage::Writing, 0.5);
    save_document(&mut document, output)?;
    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Pdf);
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    result.set_stat("pages", page_count);
    for warning in verify_pdf(output, page_count as usize)? {
        result.warn(warning);
    }
    Ok(result.finalize())
}

/// Remove identifying metadata, XMP, JavaScript, embedded files and actions.
pub fn privacy_clean(
    input: &Path,
    output: &Path,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    sink.stage(Stage::Reading, 0.0);
    cancel.check()?;
    let mut document = load_pdf(input)?;
    let page_count = document.get_pages().len() as u32;
    let mut removed_entries: u32 = 0;

    if document.trailer.remove(b"Info").is_some() {
        removed_entries += 1;
    }
    // Remove the file identifier as well; it can be used to correlate copies.
    document.trailer.remove(b"ID");

    let root = document
        .trailer
        .get(b"Root")
        .and_then(Object::as_reference)
        .ok();
    if let Some(root_id) = root {
        let names_id = document
            .get_dictionary(root_id)
            .ok()
            .and_then(|catalog| catalog.get(b"Names").and_then(Object::as_reference).ok());
        if let Ok(catalog) = document.get_dictionary_mut(root_id) {
            if catalog.remove(b"Metadata").is_some() {
                removed_entries += 1;
            }
            if catalog.remove(b"OpenAction").is_some() {
                removed_entries += 1;
            }
            if catalog.remove(b"AA").is_some() {
                removed_entries += 1;
            }
            if catalog.remove(b"PieceInfo").is_some() {
                removed_entries += 1;
            }
        }
        if let Some(names_id) = names_id {
            if let Ok(names) = document.get_dictionary_mut(names_id) {
                for key in [b"JavaScript".as_slice(), b"EmbeddedFiles".as_slice()] {
                    if names.remove(key).is_some() {
                        removed_entries += 1;
                    }
                }
            }
        }
    }

    // Remove page-level additional actions as well.
    let page_ids: Vec<_> = document.get_pages().values().copied().collect();
    for page_id in page_ids {
        if let Ok(page) = document.get_dictionary_mut(page_id) {
            if page.remove(b"AA").is_some() {
                removed_entries += 1;
            }
        }
    }

    sink.stage(Stage::Writing, 0.5);
    save_document(&mut document, output)?;
    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Pdf);
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    result.set_stat("pages", page_count);
    result.set_stat("removed_entries", removed_entries);
    result.warn("Annotations and page content are preserved. Redaction requires the dedicated redaction tool.");
    for warning in verify_pdf(output, page_count as usize)? {
        result.warn(warning);
    }
    Ok(result.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_and_decodes_unicode() {
        let object = encode_pdf_text("Отчёт");
        assert_eq!(object_text(&object).as_deref(), Some("Отчёт"));
        let ascii = encode_pdf_text("Report");
        assert_eq!(object_text(&ascii).as_deref(), Some("Report"));
    }
}

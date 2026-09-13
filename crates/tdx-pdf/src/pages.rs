//! Page-level PDF operations.

use std::path::{Path, PathBuf};

use lopdf::{Document, Object, ObjectId};
use serde::{Deserialize, Serialize};
use tdx_core::{
    detect::FileKind,
    error::{Result, TdxError},
    fsutil,
    progress::{CancelToken, ProgressSink, Stage},
    result::OperationResult,
};

use crate::build::{
    append_document, assemble_document, extract_document, load_pdf, page_dimensions,
    prune_unreachable, save_document, verify_pdf,
};
use crate::selection::PageSelection;

/// How a PDF should be split.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitMode {
    /// One file per page.
    EveryPage,
    /// Fixed-size chunks of `n` pages.
    Chunks(u32),
    /// Explicit ranges (inclusive, 1-based).
    Ranges(Vec<(u32, u32)>),
}

fn input_size(paths: &[PathBuf]) -> u64 {
    paths
        .iter()
        .map(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
        .sum()
}

/// Merge PDFs into a single file, preserving page order.
pub fn merge(
    inputs: &[PathBuf],
    output: &Path,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    if inputs.len() < 2 {
        return Err(TdxError::InvalidInput(
            "Select at least two PDF files to merge".into(),
        ));
    }
    sink.stage(Stage::Validating, 0.0);
    let mut target = Document::with_version("1.7");
    let mut pages: Vec<ObjectId> = Vec::new();
    let total = inputs.len();

    for (index, path) in inputs.iter().enumerate() {
        cancel.check()?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        sink.message(
            Stage::Reading,
            index as f32 / total as f32,
            format!("Reading {name}"),
        );
        let mut source = load_pdf(path)?;
        // Normalize encryption-free documents so references stay valid.
        source.renumber_objects();
        let imported = append_document(&mut target, &source)?;
        pages.extend(imported);
        sink.stage(Stage::Processing, (index + 1) as f32 / total as f32);
    }

    sink.message(Stage::Writing, 0.0, "Writing merged PDF");
    let catalog = assemble_document(&mut target, &pages);
    prune_unreachable(&mut target, &[catalog]);
    save_document(&mut target, output)?;

    sink.stage(Stage::Verifying, 0.5);
    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Pdf);
    result.bytes_in = input_size(inputs);
    for warning in verify_pdf(output, pages.len())? {
        result.warn(warning);
    }
    result.set_stat("pages", pages.len());
    result.set_stat("files", inputs.len());
    Ok(result.finalize())
}

/// Split a PDF into multiple files inside `output_dir`.
pub fn split(
    input: &Path,
    output_dir: &Path,
    mode: &SplitMode,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    sink.stage(Stage::Reading, 0.0);
    let source = load_pdf(input)?;
    let page_count = source.get_pages().len() as u32;
    if page_count == 0 {
        return Err(TdxError::Corrupt("The document has no pages".into()));
    }
    std::fs::create_dir_all(output_dir)?;
    let stem = fsutil::sanitize_file_stem(
        input
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("document"),
        "document",
    );

    let groups: Vec<(String, Vec<u32>)> = match mode {
        SplitMode::EveryPage => (1..=page_count)
            .map(|page| (format!("{stem}-page-{page:03}.pdf"), vec![page]))
            .collect(),
        SplitMode::Chunks(size) => {
            if *size == 0 {
                return Err(TdxError::InvalidInput(
                    "Chunk size must be at least 1 page".into(),
                ));
            }
            let mut groups = Vec::new();
            let mut start = 1u32;
            while start <= page_count {
                let end = (start + size - 1).min(page_count);
                groups.push((
                    format!("{stem}-{start:03}-{end:03}.pdf"),
                    (start..=end).collect(),
                ));
                start = end + 1;
            }
            groups
        }
        SplitMode::Ranges(ranges) => {
            let mut groups = Vec::new();
            for (start, end) in ranges {
                if *start == 0 || end < start {
                    return Err(TdxError::InvalidInput(format!(
                        "Invalid range {start}-{end}"
                    )));
                }
                if *start > page_count {
                    return Err(TdxError::InvalidInput(format!(
                        "Page {start} is outside this document (1-{page_count})"
                    )));
                }
                let end = (*end).min(page_count);
                groups.push((
                    format!("{stem}-{start:03}-{end:03}.pdf"),
                    (*start..=end).collect(),
                ));
            }
            groups
        }
    };

    let total = groups.len();
    let mut outputs = Vec::new();
    for (index, (name, pages)) in groups.iter().enumerate() {
        cancel.check()?;
        sink.message(
            Stage::Processing,
            index as f32 / total as f32,
            format!("Creating {name}"),
        );
        let mut document = extract_document(&source, pages)?;
        let path = output_dir.join(name);
        save_document(&mut document, &path)?;
        verify_pdf(&path, pages.len())?;
        outputs.push(path);
    }

    let mut result = OperationResult {
        outputs: outputs
            .into_iter()
            .map(|path| {
                let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                tdx_core::result::OutputArtifact {
                    path,
                    bytes,
                    kind: FileKind::Pdf,
                    label: None,
                }
            })
            .collect(),
        ..OperationResult::default()
    };
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    result.set_stat("pages", page_count);
    result.set_stat("files", total);
    Ok(result.finalize())
}

/// Create a new PDF containing only the selected pages.
pub fn extract_pages(
    input: &Path,
    output: &Path,
    selection: &PageSelection,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    sink.stage(Stage::Reading, 0.0);
    cancel.check()?;
    let source = load_pdf(input)?;
    let page_count = source.get_pages().len() as u32;
    let pages = selection.resolve(page_count)?;
    sink.stage(Stage::Processing, 0.3);
    let mut document = extract_document(&source, &pages)?;
    save_document(&mut document, output)?;
    let warnings = verify_pdf(output, pages.len())?;
    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Pdf);
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    result.set_stat("pages_in", page_count);
    result.set_stat("pages_out", pages.len());
    for warning in warnings {
        result.warn(warning);
    }
    Ok(result.finalize())
}

/// Remove the selected pages.
pub fn delete_pages(
    input: &Path,
    output: &Path,
    selection: &PageSelection,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    sink.stage(Stage::Reading, 0.0);
    cancel.check()?;
    let source = load_pdf(input)?;
    let page_count = source.get_pages().len() as u32;
    let remove = selection.resolve(page_count)?;
    let kept: Vec<u32> = (1..=page_count)
        .filter(|page| !remove.contains(page))
        .collect();
    if kept.is_empty() {
        return Err(TdxError::InvalidInput(
            "Deleting every page would produce an empty document".into(),
        ));
    }
    sink.stage(Stage::Processing, 0.3);
    let mut document = extract_document(&source, &kept)?;
    save_document(&mut document, output)?;
    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Pdf);
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    result.set_stat("pages_removed", remove.len());
    result.set_stat("pages_out", kept.len());
    for warning in verify_pdf(output, kept.len())? {
        result.warn(warning);
    }
    Ok(result.finalize())
}

/// Rewrite a PDF with an explicit page order (1-based page numbers).
pub fn reorder(
    input: &Path,
    output: &Path,
    order: &[u32],
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    sink.stage(Stage::Reading, 0.0);
    cancel.check()?;
    let source = load_pdf(input)?;
    let page_count = source.get_pages().len() as u32;
    if order.len() as u32 != page_count {
        return Err(TdxError::InvalidInput(format!(
            "The new order must list all {page_count} pages exactly once"
        )));
    }
    let mut seen = std::collections::BTreeSet::new();
    for page in order {
        if *page == 0 || *page > page_count || !seen.insert(*page) {
            return Err(TdxError::InvalidInput(format!(
                "Invalid page number in the new order: {page}"
            )));
        }
    }
    sink.stage(Stage::Processing, 0.3);
    let mut document = extract_document(&source, order)?;
    save_document(&mut document, output)?;
    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Pdf);
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    result.set_stat("pages", page_count);
    for warning in verify_pdf(output, order.len())? {
        result.warn(warning);
    }
    Ok(result.finalize())
}

/// Reverse page order.
pub fn reverse(
    input: &Path,
    output: &Path,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    sink.stage(Stage::Reading, 0.0);
    cancel.check()?;
    let source = load_pdf(input)?;
    let page_count = source.get_pages().len() as u32;
    let order: Vec<u32> = (1..=page_count).rev().collect();
    sink.stage(Stage::Processing, 0.3);
    let mut document = extract_document(&source, &order)?;
    save_document(&mut document, output)?;
    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Pdf);
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    result.set_stat("pages", page_count);
    for warning in verify_pdf(output, order.len())? {
        result.warn(warning);
    }
    Ok(result.finalize())
}

/// Rotate pages clockwise by `degrees` (90, 180 or 270).
pub fn rotate(
    input: &Path,
    output: &Path,
    degrees: i32,
    selection: Option<&PageSelection>,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    let normalized = degrees.rem_euclid(360);
    if normalized == 0 || normalized % 90 != 0 {
        return Err(TdxError::InvalidInput(
            "Rotation must be 90, 180 or 270 degrees clockwise".into(),
        ));
    }
    sink.stage(Stage::Reading, 0.0);
    cancel.check()?;
    let mut document = load_pdf(input)?;
    let pages = document.get_pages();
    let page_count = pages.len() as u32;
    let selected: Vec<u32> = match selection {
        Some(selection) => selection.resolve(page_count)?,
        None => (1..=page_count).collect(),
    };
    let page_ids: Vec<ObjectId> = selected
        .iter()
        .filter_map(|page| pages.get(page).copied())
        .collect();

    for (index, page_id) in page_ids.iter().enumerate() {
        cancel.check()?;
        if let Ok(Object::Dictionary(dict)) = document.get_object_mut(*page_id) {
            let current = dict
                .get(b"Rotate")
                .ok()
                .and_then(crate::build::object_to_f32)
                .unwrap_or(0.0) as i32;
            let next = (current + normalized).rem_euclid(360);
            dict.set("Rotate", Object::Integer(next as i64));
        }
        sink.stage(
            Stage::Processing,
            (index + 1) as f32 / page_ids.len() as f32,
        );
    }

    save_document(&mut document, output)?;
    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Pdf);
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    result.set_stat("pages_rotated", page_ids.len());
    result.set_stat("degrees", normalized);
    for warning in verify_pdf(output, page_count as usize)? {
        result.warn(warning);
    }
    Ok(result.finalize())
}

/// Keep only odd or only even pages.
pub fn extract_odd_even(
    input: &Path,
    output: &Path,
    keep_odd: bool,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    sink.stage(Stage::Reading, 0.0);
    cancel.check()?;
    let source = load_pdf(input)?;
    let page_count = source.get_pages().len() as u32;
    let pages: Vec<u32> = (1..=page_count)
        .filter(|page| (*page % 2 == 1) == keep_odd)
        .collect();
    if pages.is_empty() {
        return Err(TdxError::InvalidInput(if keep_odd {
            "This document has no odd pages".into()
        } else {
            "This document has no even pages".into()
        }));
    }
    sink.stage(Stage::Processing, 0.3);
    let mut document = extract_document(&source, &pages)?;
    save_document(&mut document, output)?;
    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Pdf);
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    result.set_stat("pages_out", pages.len());
    for warning in verify_pdf(output, pages.len())? {
        result.warn(warning);
    }
    Ok(result.finalize())
}

/// Insert all pages of `insert` into `base` after page `after` (0 = start).
pub fn insert(
    base: &Path,
    insert_file: &Path,
    output: &Path,
    after: u32,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    sink.stage(Stage::Reading, 0.0);
    cancel.check()?;
    let base_doc = load_pdf(base)?;
    let base_count = base_doc.get_pages().len() as u32;
    if after > base_count {
        return Err(TdxError::InvalidInput(format!(
            "Cannot insert after page {after}: the document has {base_count} pages"
        )));
    }
    let insert_doc = load_pdf(insert_file)?;
    let insert_count = insert_doc.get_pages().len() as u32;

    let mut target = Document::with_version("1.7");
    sink.stage(Stage::Processing, 0.2);
    let base_pages = append_document(&mut target, &base_doc)?;
    cancel.check()?;
    let inset_pages = append_document(&mut target, &insert_doc)?;

    let mut order: Vec<ObjectId> = Vec::new();
    order.extend(base_pages.iter().take(after as usize).copied());
    order.extend(inset_pages.iter().copied());
    order.extend(base_pages.iter().skip(after as usize).copied());
    let expected = order.len();
    let catalog = assemble_document(&mut target, &order);
    prune_unreachable(&mut target, &[catalog]);
    sink.stage(Stage::Writing, 0.6);
    save_document(&mut target, output)?;
    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Pdf);
    result.bytes_in = std::fs::metadata(base).map(|m| m.len()).unwrap_or(0)
        + std::fs::metadata(insert_file).map(|m| m.len()).unwrap_or(0);
    result.set_stat("pages_base", base_count);
    result.set_stat("pages_inserted", insert_count);
    result.set_stat("pages_out", expected);
    for warning in verify_pdf(output, expected)? {
        result.warn(warning);
    }
    Ok(result.finalize())
}

/// Copy selected pages to the end of the document (duplicate).
pub fn duplicate_pages(
    input: &Path,
    output: &Path,
    selection: &PageSelection,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    sink.stage(Stage::Reading, 0.0);
    cancel.check()?;
    let source = load_pdf(input)?;
    let page_count = source.get_pages().len() as u32;
    let duplicate = selection.resolve(page_count)?;
    let mut target = Document::with_version("1.7");
    let imported = append_document(&mut target, &source)?;
    let mut all: Vec<ObjectId> = imported.clone();
    for page in &duplicate {
        if let Some(page_id) = imported.get((*page - 1) as usize) {
            let cloned = clone_page(&mut target, *page_id)?;
            all.push(cloned);
        }
    }
    let catalog = assemble_document(&mut target, &all);
    prune_unreachable(&mut target, &[catalog]);
    save_document(&mut target, output)?;
    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Pdf);
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    result.set_stat("pages_in", page_count);
    result.set_stat("pages_out", all.len());
    for warning in verify_pdf(output, all.len())? {
        result.warn(warning);
    }
    Ok(result.finalize())
}

/// Deep-clone a single page object (and its reachable subgraph) inside a
/// document. Used by duplicate; the cloned page keeps shared resources.
fn clone_page(target: &mut Document, page_id: ObjectId) -> Result<ObjectId> {
    let object = target
        .objects
        .get(&page_id)
        .cloned()
        .ok_or_else(|| TdxError::Corrupt("Page object is missing".into()))?;
    let new_id = target.add_object(object);
    Ok(new_id)
}

/// Helper used by the CLI: total dimensions of the first page.
pub fn first_page_size(path: &Path) -> Result<(f32, f32)> {
    let document = load_pdf(path)?;
    let pages = document.get_pages();
    let Some((_, page_id)) = pages.iter().next() else {
        return Err(TdxError::Corrupt("The document has no pages".into()));
    };
    Ok(page_dimensions(&document, *page_id))
}

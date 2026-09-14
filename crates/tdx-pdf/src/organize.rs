//! Explicit page plans: reorder, delete, duplicate and rotate in a single pass.
//!
//! The interactive PDF workspace edits pages locally and then asks the engine
//! for one new document, which keeps the save atomic and verifiable.

use std::path::Path;

use lopdf::{Document, Object, ObjectId};
use serde::{Deserialize, Serialize};
use tdx_core::{
    detect::FileKind,
    error::{Result, TdxError},
    progress::{CancelToken, ProgressSink, Stage},
    result::OperationResult,
};

use crate::build::{
    append_document, assemble_document, load_pdf, prune_unreachable, save_document, verify_pdf,
};

/// One entry of an explicit page plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PagePlanEntry {
    /// 1-based page number in the source document.
    pub page: u32,
    /// Rotation added on top of the page's current rotation (multiple of 90).
    #[serde(default)]
    pub rotate: i32,
}

/// Apply an explicit page plan to a document.
///
/// The plan lists the pages of the output in order. A source page may appear
/// more than once (duplication) and may be missing entirely (deletion).
pub fn apply_plan(
    input: &Path,
    output: &Path,
    plan: &[PagePlanEntry],
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    if plan.is_empty() {
        return Err(TdxError::InvalidInput(
            "The document must keep at least one page".into(),
        ));
    }
    for entry in plan {
        if entry.rotate % 90 != 0 {
            return Err(TdxError::InvalidInput(
                "Rotation must be a multiple of 90 degrees".into(),
            ));
        }
    }

    sink.stage(Stage::Reading, 0.0);
    cancel.check()?;
    let source = load_pdf(input)?;
    let page_count = source.get_pages().len() as u32;
    if page_count == 0 {
        return Err(TdxError::Corrupt("The document has no pages".into()));
    }
    for entry in plan {
        if entry.page == 0 || entry.page > page_count {
            return Err(TdxError::InvalidInput(format!(
                "Page {} is outside this document (1-{page_count})",
                entry.page
            )));
        }
    }

    sink.stage(Stage::Processing, 0.2);
    let mut target = Document::with_version("1.7");
    let imported = append_document(&mut target, &source)?;

    let total = plan.len();
    let mut ordered: Vec<ObjectId> = Vec::with_capacity(total);
    for (index, entry) in plan.iter().enumerate() {
        cancel.check()?;
        let source_id = imported
            .get((entry.page - 1) as usize)
            .copied()
            .ok_or_else(|| TdxError::Corrupt(format!("Page {} is missing", entry.page)))?;
        let object = target
            .objects
            .get(&source_id)
            .cloned()
            .ok_or_else(|| TdxError::Corrupt("Page object is missing".into()))?;
        let Object::Dictionary(mut dict) = object else {
            return Err(TdxError::Corrupt("Page object is not a dictionary".into()));
        };
        dict.remove(b"Parent");
        if entry.rotate != 0 {
            let current = dict
                .get(b"Rotate")
                .ok()
                .and_then(crate::build::object_to_f32)
                .unwrap_or(0.0) as i32;
            let next = (current + entry.rotate).rem_euclid(360);
            if next == 0 {
                dict.remove(b"Rotate");
            } else {
                dict.set("Rotate", Object::Integer(next as i64));
            }
        }
        let new_id = target.add_object(Object::Dictionary(dict));
        ordered.push(new_id);
        sink.stage(
            Stage::Processing,
            0.2 + 0.6 * (index + 1) as f32 / total as f32,
        );
    }

    let catalog = assemble_document(&mut target, &ordered);
    prune_unreachable(&mut target, &[catalog]);
    sink.stage(Stage::Writing, 0.85);
    save_document(&mut target, output)?;

    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Pdf);
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    let bytes_out = std::fs::metadata(output).map(|m| m.len()).unwrap_or(0);
    result.bytes_out = bytes_out;
    result.set_stat("pages_in", page_count);
    result.set_stat("pages_out", ordered.len());
    sink.stage(Stage::Verifying, 0.95);
    for warning in verify_pdf(output, ordered.len())? {
        result.warn(warning);
    }
    Ok(result.finalize())
}

/// Combine several documents into one by explicit (document, page) references.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InsertPlanEntry {
    /// Index into the `inputs` slice.
    pub document: u32,
    /// 1-based page number inside that document.
    pub page: u32,
    #[serde(default)]
    pub rotate: i32,
}

/// Merge documents into one using an explicit interleaved page plan.
pub fn merge_plan(
    inputs: &[std::path::PathBuf],
    output: &Path,
    plan: &[InsertPlanEntry],
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    if inputs.is_empty() {
        return Err(TdxError::InvalidInput("Select at least one PDF".into()));
    }
    if plan.is_empty() {
        return Err(TdxError::InvalidInput(
            "The output would have no pages".into(),
        ));
    }

    sink.stage(Stage::Reading, 0.0);
    let mut sources: Vec<Document> = Vec::with_capacity(inputs.len());
    for path in inputs {
        cancel.check()?;
        sources.push(load_pdf(path)?);
    }

    let mut target = Document::with_version("1.7");
    let mut imported: Vec<Vec<ObjectId>> = Vec::with_capacity(sources.len());
    for source in &sources {
        cancel.check()?;
        imported.push(append_document(&mut target, source)?);
    }

    let total = plan.len();
    let mut ordered: Vec<ObjectId> = Vec::with_capacity(total);
    for (index, entry) in plan.iter().enumerate() {
        cancel.check()?;
        let pages = imported
            .get(entry.document as usize)
            .ok_or_else(|| TdxError::InvalidInput("Unknown source document".into()))?;
        let Some(&source_id) = pages.get((entry.page.max(1) - 1) as usize) else {
            return Err(TdxError::InvalidInput(format!(
                "Document {} has no page {}",
                entry.document + 1,
                entry.page
            )));
        };
        let mut dict = match target.objects.get(&source_id) {
            Some(Object::Dictionary(dict)) => dict.clone(),
            _ => return Err(TdxError::Corrupt("Page object is missing".into())),
        };
        dict.remove(b"Parent");
        if entry.rotate % 90 == 0 && entry.rotate != 0 {
            let current = dict
                .get(b"Rotate")
                .ok()
                .and_then(crate::build::object_to_f32)
                .unwrap_or(0.0) as i32;
            let next = (current + entry.rotate).rem_euclid(360);
            if next == 0 {
                dict.remove(b"Rotate");
            } else {
                dict.set("Rotate", Object::Integer(next as i64));
            }
        }
        ordered.push(target.add_object(Object::Dictionary(dict)));
        sink.stage(
            Stage::Processing,
            0.2 + 0.6 * (index + 1) as f32 / total as f32,
        );
    }

    let catalog = assemble_document(&mut target, &ordered);
    prune_unreachable(&mut target, &[catalog]);
    sink.stage(Stage::Writing, 0.85);
    save_document(&mut target, output)?;

    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Pdf);
    result.bytes_in = inputs
        .iter()
        .map(|path| std::fs::metadata(path).map(|m| m.len()).unwrap_or(0))
        .sum();
    result.bytes_out = std::fs::metadata(output).map(|m| m.len()).unwrap_or(0);
    result.set_stat("files", inputs.len());
    result.set_stat("pages_out", ordered.len());
    sink.stage(Stage::Verifying, 0.95);
    for warning in verify_pdf(output, ordered.len())? {
        result.warn(warning);
    }
    Ok(result.finalize())
}

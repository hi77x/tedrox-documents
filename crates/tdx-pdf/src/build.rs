//! Object-level document import and assembly.
//!
//! The merge/split family shares this machinery: every object of a source
//! document is copied into the target with fresh object ids and remapped
//! references. Inherited page attributes (`MediaBox`, `Resources`, `Rotate`,
//! `CropBox`) are resolved from the source page tree, because many PDFs keep
//! them on intermediate `Pages` nodes.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use tdx_core::error::{Result, TdxError};

pub(crate) use crate::load_pdf;

/// Import every object of `source` into `target`; returns new object ids of
/// the source pages in page order.
pub fn import_document(target: &mut Document, source: &Document) -> Result<Vec<ObjectId>> {
    let mut mapping: BTreeMap<ObjectId, ObjectId> = BTreeMap::new();
    let mut next = target.max_id.saturating_add(1);
    for &id in source.objects.keys() {
        mapping.insert(id, (next, id.1));
        next += 1;
    }
    for (id, object) in &source.objects {
        let new_id = mapping[id];
        target
            .objects
            .insert(new_id, remap_object(object, &mapping));
    }
    target.max_id = next.saturating_sub(1);

    let pages = source.get_pages();
    let mut new_pages = Vec::with_capacity(pages.len());
    for (_, page_id) in pages {
        let Some(&new_id) = mapping.get(&page_id) else {
            continue;
        };
        let inherited = inherited_attributes(source, page_id, &mapping);
        if let Some(Object::Dictionary(dict)) = target.objects.get_mut(&new_id) {
            for (key, value) in inherited {
                if !dict.has(&key) {
                    dict.set(key, value);
                }
            }
        }
        new_pages.push(new_id);
    }
    Ok(new_pages)
}

fn remap_object(object: &Object, mapping: &BTreeMap<ObjectId, ObjectId>) -> Object {
    match object {
        Object::Null => Object::Null,
        Object::Boolean(value) => Object::Boolean(*value),
        Object::Integer(value) => Object::Integer(*value),
        Object::Real(value) => Object::Real(*value),
        Object::Name(name) => Object::Name(name.clone()),
        Object::String(bytes, format) => Object::String(bytes.clone(), *format),
        Object::Array(items) => Object::Array(
            items
                .iter()
                .map(|item| remap_object(item, mapping))
                .collect(),
        ),
        Object::Dictionary(dict) => Object::Dictionary(remap_dictionary(dict, mapping)),
        Object::Stream(stream) => {
            let mut new_stream = Stream::new(
                remap_dictionary(&stream.dict, mapping),
                stream.content.clone(),
            );
            new_stream.allows_compression = stream.allows_compression;
            Object::Stream(new_stream)
        }
        Object::Reference(id) => Object::Reference(*mapping.get(id).unwrap_or(id)),
    }
}

fn remap_dictionary(dict: &Dictionary, mapping: &BTreeMap<ObjectId, ObjectId>) -> Dictionary {
    let mut out = Dictionary::new();
    for (key, value) in dict.iter() {
        out.set(key.clone(), remap_object(value, mapping));
    }
    out
}

const INHERITABLE_KEYS: [&[u8]; 4] = [b"MediaBox", b"CropBox", b"Resources", b"Rotate"];

fn inherited_attributes(
    source: &Document,
    page_id: ObjectId,
    mapping: &BTreeMap<ObjectId, ObjectId>,
) -> Vec<(Vec<u8>, Object)> {
    let mut collected: Vec<(Vec<u8>, Object)> = Vec::new();
    let mut visited: BTreeSet<ObjectId> = BTreeSet::new();
    let mut current = page_id;
    while visited.insert(current) {
        let Ok(object) = source.get_object(current) else {
            break;
        };
        let Ok(dict) = object.as_dict() else { break };
        for key in INHERITABLE_KEYS {
            if collected
                .iter()
                .any(|(existing, _)| existing.as_slice() == key)
            {
                continue;
            }
            if let Ok(value) = dict.get(key) {
                collected.push((key.to_vec(), remap_object(value, mapping)));
            }
        }
        match dict.get(b"Parent").and_then(Object::as_reference) {
            Ok(parent) => current = parent,
            Err(_) => break,
        }
    }
    collected
}

/// Build a fresh `Pages` tree and `Catalog` for `pages`, set `/Parent` on each
/// page and return the catalog object id.
pub fn assemble_document(target: &mut Document, pages: &[ObjectId]) -> ObjectId {
    let pages_id = target.new_object_id();
    let mut kids = Vec::with_capacity(pages.len());
    for page_id in pages {
        if let Some(Object::Dictionary(dict)) = target.objects.get_mut(page_id) {
            dict.set("Parent", pages_id);
            if !dict.has(b"MediaBox") {
                dict.set(
                    "MediaBox",
                    vec![
                        Object::Integer(0),
                        Object::Integer(0),
                        Object::Integer(595),
                        Object::Integer(842),
                    ],
                );
            }
        }
        kids.push(Object::Reference(*page_id));
    }
    let mut pages_dict = Dictionary::new();
    pages_dict.set("Type", "Pages");
    pages_dict.set("Kids", Object::Array(kids));
    pages_dict.set("Count", pages.len() as i64);
    target
        .objects
        .insert(pages_id, Object::Dictionary(pages_dict));

    let catalog_id = target.new_object_id();
    let mut catalog = Dictionary::new();
    catalog.set("Type", "Catalog");
    catalog.set("Pages", pages_id);
    target
        .objects
        .insert(catalog_id, Object::Dictionary(catalog));
    target.trailer.set("Root", catalog_id);
    catalog_id
}

/// Extract selected 1-based pages from a source document into a new document.
pub fn extract_document(source: &Document, pages: &[u32]) -> Result<Document> {
    let mut target = Document::with_version("1.7");
    let imported = import_document(&mut target, source)?;
    let selected: Vec<ObjectId> = pages
        .iter()
        .filter_map(|page| imported.get((*page).saturating_sub(1) as usize).copied())
        .collect();
    if selected.is_empty() {
        return Err(TdxError::InvalidInput("No pages selected".into()));
    }
    let catalog = assemble_document(&mut target, &selected);
    prune_unreachable(&mut target, &[catalog]);
    Ok(target)
}

/// Keep only objects reachable from `roots` (plus referenced info dictionaries).
pub fn prune_unreachable(doc: &mut Document, roots: &[ObjectId]) {
    let mut keep: BTreeSet<ObjectId> = BTreeSet::new();
    let mut stack: Vec<ObjectId> = roots.to_vec();
    if let Ok(info) = doc.trailer.get(b"Info").and_then(Object::as_reference) {
        stack.push(info);
    }
    while let Some(id) = stack.pop() {
        if !keep.insert(id) {
            continue;
        }
        if let Some(object) = doc.objects.get(&id) {
            collect_references(object, &mut stack);
        }
    }
    doc.objects.retain(|id, _| keep.contains(id));
}

fn collect_references(object: &Object, stack: &mut Vec<ObjectId>) {
    match object {
        Object::Reference(id) => stack.push(*id),
        Object::Array(items) => {
            for item in items {
                collect_references(item, stack);
            }
        }
        Object::Dictionary(dict) => {
            for (_, value) in dict.iter() {
                collect_references(value, stack);
            }
        }
        Object::Stream(stream) => {
            for (_, value) in stream.dict.iter() {
                collect_references(value, stack);
            }
        }
        _ => {}
    }
}

/// Save a document atomically to `output`.
pub fn save_document(doc: &mut Document, output: &Path) -> Result<()> {
    doc.renumber_objects();
    doc.compress();
    tdx_core::fsutil::atomic_write(output, |file| {
        doc.save_to(file)
            .map_err(|err| TdxError::Other(format!("Failed to write PDF: {err}")))
    })
}

/// Re-open the generated file and confirm the page count. Returns warnings for
/// non-fatal mismatches.
pub fn verify_pdf(path: &Path, expected_pages: usize) -> Result<Vec<String>> {
    let mut warnings = Vec::new();
    match load_pdf(path) {
        Ok(document) => {
            let actual = document.get_pages().len();
            if actual != expected_pages {
                warnings.push(format!(
                    "Output verification: expected {expected_pages} pages, found {actual}"
                ));
            }
        }
        Err(err) => {
            let _ = std::fs::remove_file(path);
            return Err(TdxError::Other(format!(
                "The output PDF failed verification and was removed: {err}"
            )));
        }
    }
    Ok(warnings)
}

/// Resolve a page `/MediaBox` to `(width, height)`.
pub fn page_dimensions(doc: &Document, page_id: ObjectId) -> (f32, f32) {
    let mut current = page_id;
    let mut visited = BTreeSet::new();
    while visited.insert(current) {
        let Ok(object) = doc.get_object(current) else {
            break;
        };
        let Ok(dict) = object.as_dict() else { break };
        if let Ok(media_box) = dict.get(b"MediaBox") {
            let media_box = resolve_array(doc, media_box);
            if let Some(values) = media_box {
                if values.len() == 4 {
                    let x0 = object_to_f32(&values[0]).unwrap_or(0.0);
                    let y0 = object_to_f32(&values[1]).unwrap_or(0.0);
                    let x1 = object_to_f32(&values[2]).unwrap_or(595.0);
                    let y1 = object_to_f32(&values[3]).unwrap_or(842.0);
                    return ((x1 - x0).abs(), (y1 - y0).abs());
                }
            }
        }
        match dict.get(b"Parent").and_then(Object::as_reference) {
            Ok(parent) => current = parent,
            Err(_) => break,
        }
    }
    (595.0, 842.0)
}

fn resolve_array(doc: &Document, object: &Object) -> Option<Vec<Object>> {
    match object {
        Object::Array(items) => Some(items.clone()),
        Object::Reference(id) => {
            let target = doc.get_object(*id).ok()?;
            target.as_array().ok().cloned()
        }
        _ => None,
    }
}

pub fn object_to_f32(object: &Object) -> Option<f32> {
    match object {
        Object::Integer(value) => Some(*value as f32),
        Object::Real(value) => Some(*value),
        _ => None,
    }
}

/// Copy every page from `source` into `target` and return the assembled
/// catalog. Used by merge and insert.
pub fn append_document(target: &mut Document, source: &Document) -> Result<Vec<ObjectId>> {
    import_document(target, source)
}

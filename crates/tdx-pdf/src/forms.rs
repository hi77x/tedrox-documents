//! AcroForm support: inspect existing form fields and fill them in.
//!
//! TEDROX never executes JavaScript, actions or launch targets from a PDF.
//! Filling writes only field values and asks the reader to regenerate
//! appearances, which is the behaviour users expect from a form filler.

use std::collections::BTreeMap;
use std::path::Path;

use lopdf::{Dictionary, Document, Object, ObjectId};
use serde::{Deserialize, Serialize};
use tdx_core::{
    detect::FileKind,
    error::{Result, TdxError},
    progress::{CancelToken, ProgressSink, Stage},
    result::OperationResult,
};

use crate::build::{load_pdf, save_document, verify_pdf};

/// One interactive form field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormField {
    /// Fully qualified field name (`person.address.city`).
    pub name: String,
    /// `text`, `checkbox`, `radio`, `choice`, `signature` or `unknown`.
    pub kind: String,
    /// Current value, when the field carries a simple string value.
    pub value: Option<String>,
    /// `/AS` appearance state for buttons (checkbox or radio).
    pub state: Option<String>,
    /// Choice options for list boxes and combo boxes.
    pub options: Vec<String>,
    pub read_only: bool,
    pub required: bool,
    pub multiline: bool,
}

fn field_kind(ft: &str, flags: i64) -> &'static str {
    match ft {
        "Tx" => "text",
        "Ch" => "choice",
        "Sig" => "signature",
        "Btn" => {
            if flags & 0x8000 != 0 {
                "radio"
            } else if flags & 0x10000 != 0 {
                "button"
            } else {
                "checkbox"
            }
        }
        _ => "unknown",
    }
}

fn string_value(object: &Object) -> Option<String> {
    match object {
        Object::String(bytes, _) => {
            if bytes.starts_with(&[0xFE, 0xFF]) {
                let units: Vec<u16> = bytes[2..]
                    .chunks_exact(2)
                    .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
                    .collect();
                String::from_utf16(&units).ok()
            } else {
                Some(String::from_utf8_lossy(bytes).to_string())
            }
        }
        Object::Name(name) => Some(String::from_utf8_lossy(name).to_string()),
        Object::Integer(value) => Some(value.to_string()),
        Object::Real(value) => Some(value.to_string()),
        _ => None,
    }
}

fn resolve<'a>(document: &'a Document, object: &'a Object) -> Option<&'a Object> {
    match object {
        Object::Reference(id) => document.get_object(*id).ok(),
        other => Some(other),
    }
}

fn acro_form(document: &Document) -> Option<&Dictionary> {
    let root = document.trailer.get(b"Root").ok()?.as_reference().ok()?;
    let catalog = document.get_object(root).ok()?.as_dict().ok()?;
    resolve(document, catalog.get(b"AcroForm").ok()?)?
        .as_dict()
        .ok()
}

fn acro_form_id(document: &Document) -> Option<ObjectId> {
    let root = document.trailer.get(b"Root").ok()?.as_reference().ok()?;
    let catalog = document.get_object(root).ok()?.as_dict().ok()?;
    match catalog.get(b"AcroForm") {
        Ok(Object::Reference(id)) => Some(*id),
        _ => None,
    }
}

fn collect_fields(
    document: &Document,
    nodes: &[Object],
    prefix: &str,
    out: &mut Vec<(String, ObjectId, Dictionary)>,
) {
    for node in nodes {
        let Some(object) = resolve(document, node) else {
            continue;
        };
        let Ok(dict) = object.as_dict() else { continue };
        let partial = dict
            .get(b"T")
            .ok()
            .and_then(string_value)
            .unwrap_or_default();
        let name = if prefix.is_empty() {
            partial.clone()
        } else if partial.is_empty() {
            prefix.to_string()
        } else {
            format!("{prefix}.{partial}")
        };
        let kids = dict
            .get(b"Kids")
            .ok()
            .and_then(|value| resolve(document, value))
            .and_then(|value| value.as_array().ok())
            .cloned()
            .unwrap_or_default();
        let has_field_kids = kids.iter().any(|kid| {
            resolve(document, kid)
                .and_then(|value| value.as_dict().ok())
                .map(|kid_dict| kid_dict.has(b"T") || kid_dict.has(b"Kids"))
                .unwrap_or(false)
        });
        if has_field_kids {
            collect_fields(document, &kids, &name, out);
            continue;
        }
        if let Object::Reference(id) = node {
            out.push((name, *id, dict.clone()));
        }
    }
}

/// Inspect the interactive form of a PDF.
pub fn list_fields(path: &Path) -> Result<Vec<FormField>> {
    let document = load_pdf(path)?;
    let Some(form) = acro_form(&document) else {
        return Ok(Vec::new());
    };
    let Ok(fields) = form.get(b"Fields") else {
        return Ok(Vec::new());
    };
    let nodes = resolve(&document, fields)
        .and_then(|value| value.as_array().ok())
        .cloned()
        .unwrap_or_default();
    let mut collected = Vec::new();
    collect_fields(&document, &nodes, "", &mut collected);

    let mut out = Vec::new();
    for (name, _id, dict) in collected {
        let (field_type, flags) = inherited_property(&document, b"FT", &dict)
            .and_then(string_value)
            .map(|ft| {
                let flags = dict
                    .get(b"Ff")
                    .ok()
                    .and_then(|value| value.as_i64().ok())
                    .unwrap_or(0);
                (ft, flags)
            })
            .unwrap_or_else(|| (String::new(), 0));
        let options = inherited_property(&document, b"Opt", &dict)
            .and_then(|value| resolve(&document, value).cloned())
            .and_then(|value| value.as_array().ok().cloned())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        let item = resolve(&document, item)?;
                        match item {
                            Object::Array(pair) => pair.first().and_then(string_value),
                            other => string_value(other),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        out.push(FormField {
            kind: field_kind(&field_type, flags).to_string(),
            value: inherited_property(&document, b"V", &dict).and_then(string_value),
            state: dict.get(b"AS").ok().and_then(string_value),
            options,
            read_only: flags & 1 != 0,
            required: flags & 2 != 0,
            multiline: flags & 0x1000 != 0,
            name,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

fn inherited_property<'a>(
    document: &'a Document,
    key: &[u8],
    dict: &'a Dictionary,
) -> Option<&'a Object> {
    if let Ok(value) = dict.get(key) {
        return Some(value);
    }
    let mut current = dict
        .get(b"Parent")
        .ok()
        .and_then(|value| value.as_reference().ok())?;
    let mut guard = 0;
    while guard < 32 {
        guard += 1;
        let parent = document.get_object(current).ok()?.as_dict().ok()?;
        if let Ok(value) = parent.get(key) {
            return Some(value);
        }
        current = match parent.get(b"Parent").and_then(|value| value.as_reference()) {
            Ok(id) => id,
            Err(_) => break,
        };
    }
    None
}

fn button_on_state(document: &Document, dict: &Dictionary) -> Option<String> {
    let ap = dict.get(b"AP").ok()?;
    let ap = resolve(document, ap)?;
    let normal = ap.as_dict().ok()?.get(b"N").ok()?;
    let normal = resolve(document, normal)?;
    let states = normal.as_dict().ok()?;
    states
        .iter()
        .map(|(key, _)| String::from_utf8_lossy(key).to_string())
        .find(|key| key != "Off")
}

/// Fill form fields. Unknown names are reported as warnings instead of failing.
pub fn fill_fields(
    input: &Path,
    output: &Path,
    values: &BTreeMap<String, String>,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    if values.is_empty() {
        return Err(TdxError::InvalidInput(
            "Provide at least one form field value".into(),
        ));
    }
    sink.stage(Stage::Reading, 0.0);
    cancel.check()?;
    let mut document = load_pdf(input)?;
    let Some(form_dict) = acro_form(&document) else {
        return Err(TdxError::InvalidInput(
            "This PDF does not contain an interactive form".into(),
        ));
    };
    let Ok(fields) = form_dict.get(b"Fields") else {
        return Err(TdxError::InvalidInput(
            "This PDF does not contain an interactive form".into(),
        ));
    };
    let nodes = resolve(&document, fields)
        .and_then(|value| value.as_array().ok())
        .cloned()
        .unwrap_or_default();
    let mut collected = Vec::new();
    collect_fields(&document, &nodes, "", &mut collected);
    let form_id = acro_form_id(&document);

    let known: Vec<String> = collected.iter().map(|(name, _, _)| name.clone()).collect();
    let mut applied = 0usize;
    let mut unknown: Vec<String> = Vec::new();
    for (name, id, dict) in &collected {
        let Some(value) = values.get(name) else {
            continue;
        };
        cancel.check()?;
        let kind = inherited_property(&document, b"FT", dict)
            .and_then(string_value)
            .unwrap_or_default();
        let on_state = if kind == "Btn" {
            button_on_state(&document, dict).unwrap_or_else(|| "Yes".to_string())
        } else {
            String::new()
        };
        if let Ok(Object::Dictionary(field)) = document.get_object_mut(*id) {
            match kind.as_str() {
                "Btn" => {
                    let truthy = matches!(
                        value.trim().to_ascii_lowercase().as_str(),
                        "1" | "true" | "yes" | "on" | "checked"
                    );
                    field.set(
                        "AS",
                        Object::Name(if truthy {
                            on_state.into_bytes()
                        } else {
                            b"Off".to_vec()
                        }),
                    );
                    field.set(
                        "V",
                        Object::Name(if truthy {
                            b"Yes".to_vec()
                        } else {
                            b"Off".to_vec()
                        }),
                    );
                }
                "Ch" => {
                    let index = value.trim().parse::<i64>().ok();
                    field.set(
                        "V",
                        match index {
                            Some(index) => Object::String(
                                index.to_string().into_bytes(),
                                lopdf::StringFormat::Literal,
                            ),
                            None => Object::String(
                                value.clone().into_bytes(),
                                lopdf::StringFormat::Literal,
                            ),
                        },
                    );
                }
                _ => {
                    field.set(
                        "V",
                        Object::String(value.clone().into_bytes(), lopdf::StringFormat::Literal),
                    );
                }
            }
            applied += 1;
        }
    }

    for name in values.keys() {
        if !known.iter().any(|field| field == name) {
            unknown.push(name.clone());
        }
    }

    if let Some(form_id) = form_id {
        if let Ok(Object::Dictionary(form)) = document.get_object_mut(form_id) {
            form.set("NeedAppearances", Object::Boolean(true));
        }
    }

    sink.stage(Stage::Writing, 0.7);
    save_document(&mut document, output)?;
    let page_count = document.get_pages().len() as usize;
    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Pdf);
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    result.bytes_out = std::fs::metadata(output).map(|m| m.len()).unwrap_or(0);
    result.set_stat("fields_filled", applied);
    if !unknown.is_empty() {
        result.warn(format!(
            "The following fields do not exist in this document: {}",
            unknown.join(", ")
        ));
    }
    for warning in verify_pdf(output, page_count)? {
        result.warn(warning);
    }
    Ok(result.finalize())
}

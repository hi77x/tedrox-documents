//! Text watermark stamping.

use std::path::Path;

use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use serde::{Deserialize, Serialize};
use tdx_core::{
    detect::FileKind,
    error::{Result, TdxError},
    progress::{CancelToken, ProgressSink, Stage},
    result::OperationResult,
};

use crate::build::{load_pdf, page_dimensions, save_document, verify_pdf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatermarkOptions {
    pub text: String,
    pub font_size: f32,
    pub opacity: f32,
    /// Counter-clockwise angle in degrees.
    pub angle: f32,
    pub color: (f32, f32, f32),
}

impl Default for WatermarkOptions {
    fn default() -> Self {
        Self {
            text: "CONFIDENTIAL".into(),
            font_size: 48.0,
            opacity: 0.15,
            angle: 45.0,
            color: (0.5, 0.5, 0.5),
        }
    }
}

fn escape_pdf_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\\' => out.push_str("\\\\"),
            '\n' | '\r' => out.push(' '),
            _ if ch.is_ascii() => out.push(ch),
            // Standard Type1 fonts only cover WinAnsi; non-Latin glyphs cannot
            // be rendered without embedding a font. Replace and warn.
            _ => out.push('?'),
        }
    }
    out
}

fn ensure_subdictionary(doc: &mut Document, owner_id: ObjectId, key: &[u8]) -> Result<()> {
    let target = doc
        .get_object_mut(owner_id)
        .map_err(|_| TdxError::Corrupt("Page object is missing".into()))?;
    let dict = target
        .as_dict_mut()
        .map_err(|_| TdxError::Corrupt("Page object is not a dictionary".into()))?;

    let existing = dict.get(key).ok().cloned();
    match existing {
        Some(Object::Reference(id)) => {
            let referenced = doc
                .get_object_mut(id)
                .map_err(|_| TdxError::Corrupt("Resources reference is broken".into()))?;
            if referenced.as_dict_mut().is_err() {
                return Err(TdxError::Corrupt(
                    "Resources entry is not a dictionary".into(),
                ));
            }
        }
        Some(Object::Dictionary(_)) => {}
        _ => {
            let id = doc.add_object(Dictionary::new());
            let target = doc.get_object_mut(owner_id).expect("page exists");
            target.as_dict_mut().expect("dict").set(key.to_vec(), id);
        }
    }
    Ok(())
}

fn subdictionary_id(doc: &mut Document, owner_id: ObjectId, key: &[u8]) -> Result<ObjectId> {
    let target = doc
        .get_object_mut(owner_id)
        .map_err(|_| TdxError::Corrupt("Page object is missing".into()))?;
    let dict = target
        .as_dict_mut()
        .map_err(|_| TdxError::Corrupt("Broken page".into()))?;
    match dict.get(key).ok().cloned() {
        Some(Object::Reference(id)) => Ok(id),
        Some(Object::Dictionary(_)) => {
            let inline = dict.remove(key).expect("present");
            let id = doc.add_object(inline);
            let target = doc.get_object_mut(owner_id).expect("page exists");
            target.as_dict_mut().expect("dict").set(key.to_vec(), id);
            Ok(id)
        }
        _ => Err(TdxError::Corrupt("Missing sub-dictionary".into())),
    }
}

/// Stamp a text watermark on every page.
pub fn watermark(
    input: &Path,
    output: &Path,
    options: &WatermarkOptions,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    if options.text.trim().is_empty() {
        return Err(TdxError::InvalidInput("Watermark text is empty".into()));
    }
    sink.stage(Stage::Reading, 0.0);
    cancel.check()?;
    let mut document = load_pdf(input)?;
    let page_ids: Vec<ObjectId> = document.get_pages().values().copied().collect();
    if page_ids.is_empty() {
        return Err(TdxError::Corrupt("The document has no pages".into()));
    }
    let page_count = page_ids.len();

    let mut font = Dictionary::new();
    font.set("Type", "Font");
    font.set("Subtype", "Type1");
    font.set("BaseFont", "Helvetica");
    font.set("Encoding", "WinAnsiEncoding");
    let font_id = document.add_object(font);

    let mut graphics = Dictionary::new();
    graphics.set("Type", "ExtGState");
    graphics.set("ca", Object::Real(options.opacity.clamp(0.02, 1.0)));
    graphics.set("CA", Object::Real(options.opacity.clamp(0.02, 1.0)));
    let graphics_id = document.add_object(graphics);

    let has_non_ascii = !options.text.is_ascii();
    let escaped = escape_pdf_text(&options.text);
    let angle_rad = options.angle.to_radians();
    let cos = angle_rad.cos();
    let sin = angle_rad.sin();
    let (r, g, b) = options.color;

    for (index, page_id) in page_ids.iter().enumerate() {
        cancel.check()?;
        let (page_width, page_height) = page_dimensions(&document, *page_id);
        ensure_subdictionary(&mut document, *page_id, b"Resources")?;
        let resources_id = subdictionary_id(&mut document, *page_id, b"Resources")?;

        ensure_subdictionary(&mut document, resources_id, b"Font")?;
        let font_dict_id = subdictionary_id(&mut document, resources_id, b"Font")?;
        document
            .get_dictionary_mut(font_dict_id)
            .map_err(|err| TdxError::Corrupt(err.to_string()))?
            .set("TDXWMFont", font_id);

        ensure_subdictionary(&mut document, resources_id, b"ExtGState")?;
        let graphics_dict_id = subdictionary_id(&mut document, resources_id, b"ExtGState")?;
        document
            .get_dictionary_mut(graphics_dict_id)
            .map_err(|err| TdxError::Corrupt(err.to_string()))?
            .set("TDXWMGS", graphics_id);

        let approximate_width = options.font_size * 0.55 * escaped.chars().count() as f32;
        let center_x = page_width / 2.0;
        let center_y = page_height / 2.0;
        let tx = center_x - (approximate_width / 2.0) * cos + (options.font_size / 2.0) * sin;
        let ty = center_y - (approximate_width / 2.0) * sin - (options.font_size / 2.0) * cos;

        let content = format!(
            "q\n/TDXWMGS gs\n{r:.3} {g:.3} {b:.3} rg\nBT\n/TDXWMFont {size:.1} Tf\n{cos:.5} {sin:.5} {neg_sin:.5} {cos:.5} {tx:.2} {ty:.2} Tm\n({text}) Tj\nET\nQ\n",
            size = options.font_size,
            neg_sin = -sin,
            text = escaped,
        );
        let stream_id = document.add_object(Stream::new(Dictionary::new(), content.into_bytes()));

        let page = document
            .get_object_mut(*page_id)
            .map_err(|_| TdxError::Corrupt("Page object is missing".into()))?;
        let dict = page
            .as_dict_mut()
            .map_err(|_| TdxError::Corrupt("Broken page".into()))?;
        match dict.get(b"Contents").ok().cloned() {
            Some(Object::Array(mut items)) => {
                items.push(Object::Reference(stream_id));
                dict.set("Contents", Object::Array(items));
            }
            Some(Object::Reference(existing)) => {
                dict.set(
                    "Contents",
                    Object::Array(vec![
                        Object::Reference(existing),
                        Object::Reference(stream_id),
                    ]),
                );
            }
            Some(other) => {
                dict.set(
                    "Contents",
                    Object::Array(vec![other, Object::Reference(stream_id)]),
                );
            }
            None => {
                dict.set("Contents", stream_id);
            }
        }
        sink.stage(Stage::Processing, (index + 1) as f32 / page_count as f32);
    }

    sink.stage(Stage::Writing, 0.9);
    save_document(&mut document, output)?;
    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Pdf);
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    result.set_stat("pages", page_count);
    if has_non_ascii {
        result.warn(
            "The built-in watermark font covers Latin characters only; unsupported characters were replaced with '?'",
        );
    }
    for warning in verify_pdf(output, page_count)? {
        result.warn(warning);
    }
    Ok(result.finalize())
}

//! Annotation writing.
//!
//! The workspace lets users place highlights, ink, shapes and notes on a PDF.
//! Everything is written as a real PDF annotation dictionary with an explicit
//! appearance stream, so the result opens the same way in other readers.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use serde::{Deserialize, Serialize};
use tdx_core::{
    detect::FileKind,
    error::{Result, TdxError},
    progress::{CancelToken, ProgressSink, Stage},
    result::OperationResult,
};

use crate::build::{load_pdf, save_document, verify_pdf};

/// Geometry of a text markup annotation (one axis-aligned rectangle per line).
pub type Quad = [f32; 4];

/// The kind of annotation to place.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AnnotationKind {
    /// Text highlight: quads mark the covered lines.
    Highlight { quads: Vec<Quad> },
    /// Underline below the text baseline.
    Underline { quads: Vec<Quad> },
    /// Strike through the middle of the text.
    StrikeOut { quads: Vec<Quad> },
    /// Outlined rectangle.
    Square,
    /// Outlined ellipse inscribed in the rectangle.
    Circle,
    /// Straight line from `from` to `to` (PDF user space points).
    Line { from: [f32; 2], to: [f32; 2] },
    /// Line with an arrow head at `to`.
    Arrow { from: [f32; 2], to: [f32; 2] },
    /// Free text drawn directly on the page.
    FreeText {
        text: String,
        #[serde(default = "default_font_size")]
        font_size: f32,
    },
    /// Sticky note shown by the viewer next to the page.
    Note { text: String },
    /// Freehand ink: one or more strokes in PDF user space.
    Ink { strokes: Vec<Vec<[f32; 2]>> },
}

fn default_font_size() -> f32 {
    12.0
}

/// A single annotation request from the workspace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnnotationInput {
    /// 1-based page number.
    pub page: u32,
    /// Bounding rectangle `[x0, y0, x1, y1]` in PDF user space.
    pub rect: Quad,
    /// Annotation geometry and payload.
    pub kind: AnnotationKind,
    /// RGB colour in 0..1 range.
    #[serde(default)]
    pub color: Option<[f32; 3]>,
    /// Constant opacity in 0..1.
    #[serde(default)]
    pub opacity: Option<f32>,
    /// Author shown by the reader.
    #[serde(default)]
    pub author: Option<String>,
    /// Comment text.
    #[serde(default)]
    pub contents: Option<String>,
}

fn normalize_rect(rect: Quad) -> Quad {
    let x0 = rect[0].min(rect[2]);
    let y0 = rect[1].min(rect[3]);
    let x1 = rect[0].max(rect[2]);
    let y1 = rect[1].max(rect[3]);
    [x0, y0, x1, y1]
}

fn quad_union(quads: &[Quad]) -> Quad {
    let mut union = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for quad in quads {
        let rect = normalize_rect(*quad);
        union[0] = union[0].min(rect[0]);
        union[1] = union[1].min(rect[1]);
        union[2] = union[2].max(rect[2]);
        union[3] = union[3].max(rect[3]);
    }
    if union[0] > union[2] || union[1] > union[3] {
        [0.0, 0.0, 1.0, 1.0]
    } else {
        union
    }
}

fn num(value: f32) -> String {
    let mut text = format!("{:.3}", value);
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    if text.is_empty() || text == "-0" {
        text = "0".to_string();
    }
    text
}

fn color_tuple(color: Option<[f32; 3]>, fallback: [f32; 3]) -> [f32; 3] {
    let value = color.unwrap_or(fallback);
    [
        value[0].clamp(0.0, 1.0),
        value[1].clamp(0.0, 1.0),
        value[2].clamp(0.0, 1.0),
    ]
}

fn opacity_value(opacity: Option<f32>) -> f32 {
    opacity.unwrap_or(1.0).clamp(0.05, 1.0)
}

/// PDF text strings: ASCII stays literal, anything else uses UTF-16BE hex.
fn encode_text_object(text: &str) -> Object {
    if text.is_ascii() {
        let mut escaped = String::with_capacity(text.len());
        for ch in text.chars() {
            match ch {
                '\\' => escaped.push_str("\\\\"),
                '(' => escaped.push_str("\\("),
                ')' => escaped.push_str("\\)"),
                other => escaped.push(other),
            }
        }
        Object::String(escaped.into_bytes(), lopdf::StringFormat::Literal)
    } else {
        let mut bytes = vec![0xFEu8, 0xFFu8];
        for unit in text.encode_utf16() {
            bytes.push((unit >> 8) as u8);
            bytes.push((unit & 0xFF) as u8);
        }
        Object::String(bytes, lopdf::StringFormat::Hexadecimal)
    }
}

fn is_latin1(text: &str) -> bool {
    text.chars().all(|ch| (ch as u32) <= 0xFF)
}

fn escape_show_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 4);
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\r' => {}
            other => out.push(other),
        }
    }
    out
}

fn ellipse_path(cx: f32, cy: f32, rx: f32, ry: f32) -> String {
    const K: f32 = 0.552_284_8;
    let ox = rx * K;
    let oy = ry * K;
    let mut path = String::new();
    let _ = write!(path, "{} {} m ", num(cx - rx), num(cy));
    let _ = write!(
        path,
        "{} {} {} {} {} {} c ",
        num(cx - rx),
        num(cy + oy),
        num(cx - ox),
        num(cy + ry),
        num(cx),
        num(cy + ry)
    );
    let _ = write!(
        path,
        "{} {} {} {} {} {} c ",
        num(cx + ox),
        num(cy + ry),
        num(cx + rx),
        num(cy + oy),
        num(cx + rx),
        num(cy)
    );
    let _ = write!(
        path,
        "{} {} {} {} {} {} c ",
        num(cx + rx),
        num(cy - oy),
        num(cx + ox),
        num(cy - ry),
        num(cx),
        num(cy - ry)
    );
    let _ = write!(
        path,
        "{} {} {} {} {} {} c h",
        num(cx - ox),
        num(cy - ry),
        num(cx - rx),
        num(cy - oy),
        num(cx - rx),
        num(cy)
    );
    path
}

fn shape_content(
    kind: &AnnotationKind,
    color: [f32; 3],
    opacity: f32,
    rect: Quad,
) -> ContentStream {
    let [x0, y0, x1, y1] = rect;
    let width = (x1 - x0).abs().max(1.0);
    let height = (y1 - y0).abs().max(1.0);
    let stroke = (width.min(height) * 0.02).clamp(1.0, 3.0);
    let mut content = ContentStream::default();
    content.ext_gstate(opacity);
    let r = num(color[0]);
    let g = num(color[1]);
    let b = num(color[2]);
    match kind {
        AnnotationKind::Highlight { quads } => {
            content.body.push_str(&format!("{r} {g} {b} rg\n"));
            for quad in quads {
                let [q0, q1, q2, q3] = normalize_rect(*quad);
                let _ = writeln!(
                    content.body,
                    "{} {} {} {} re f",
                    num(q0),
                    num(q1),
                    num(q2 - q0),
                    num(q3 - q1)
                );
            }
        }
        AnnotationKind::Underline { quads } => {
            content
                .body
                .push_str(&format!("{r} {g} {b} RG\n{} w\n", num(stroke)));
            for quad in quads {
                let [q0, q1, q2, _] = normalize_rect(*quad);
                let y = q1 + stroke * 0.5;
                let _ = writeln!(
                    content.body,
                    "{} {} m {} {} l S",
                    num(q0),
                    num(y),
                    num(q2),
                    num(y)
                );
            }
        }
        AnnotationKind::StrikeOut { quads } => {
            content
                .body
                .push_str(&format!("{r} {g} {b} RG\n{} w\n", num(stroke)));
            for quad in quads {
                let [q0, q1, q2, q3] = normalize_rect(*quad);
                let y = (q1 + q3) / 2.0;
                let _ = writeln!(
                    content.body,
                    "{} {} m {} {} l S",
                    num(q0),
                    num(y),
                    num(q2),
                    num(y)
                );
            }
        }
        AnnotationKind::Square => {
            content
                .body
                .push_str(&format!("{r} {g} {b} RG\n{} w\n", num(stroke)));
            let inset = stroke / 2.0;
            let _ = writeln!(
                content.body,
                "{} {} {} {} re S",
                num(x0 + inset),
                num(y0 + inset),
                num(width - stroke),
                num(height - stroke)
            );
        }
        AnnotationKind::Circle => {
            content
                .body
                .push_str(&format!("{r} {g} {b} RG\n{} w\n", num(stroke)));
            content.body.push_str(&ellipse_path(
                x0 + width / 2.0,
                y0 + height / 2.0,
                width / 2.0 - stroke / 2.0,
                height / 2.0 - stroke / 2.0,
            ));
            content.body.push_str(" S\n");
        }
        AnnotationKind::Line { from, to } | AnnotationKind::Arrow { from, to } => {
            content.body.push_str(&format!(
                "{r} {g} {b} RG\n{} w\n1 J\n{} {} m {} {} l S\n",
                num(stroke * 1.2),
                num(from[0]),
                num(from[1]),
                num(to[0]),
                num(to[1])
            ));
            if let AnnotationKind::Arrow { from, to } = kind {
                let dx = to[0] - from[0];
                let dy = to[1] - from[1];
                let len = (dx * dx + dy * dy).sqrt().max(0.001);
                let ux = dx / len;
                let uy = dy / len;
                let head = (len * 0.12).clamp(6.0, 18.0);
                let px = -uy;
                let py = ux;
                let hx = to[0] - ux * head;
                let hy = to[1] - uy * head;
                let _ = writeln!(
                    content.body,
                    "{} {} m {} {} l S",
                    num(hx + px * head * 0.45),
                    num(hy + py * head * 0.45),
                    num(to[0]),
                    num(to[1])
                );
                let _ = writeln!(
                    content.body,
                    "{} {} m {} {} l S",
                    num(hx - px * head * 0.45),
                    num(hy - py * head * 0.45),
                    num(to[0]),
                    num(to[1])
                );
            }
        }
        AnnotationKind::FreeText { text, font_size } => {
            let size = font_size.clamp(6.0, 96.0);
            let leading = size * 1.25;
            content.uses_font = true;
            content.body.push_str(&format!(
                "{r} {g} {b} rg\nBT\n/Helv {} Tf\n{} TL\n{} {} Td\n",
                num(size),
                num(leading),
                num(x0 + 4.0),
                num(y1 - leading)
            ));
            for (index, line) in text.lines().enumerate() {
                if index > 0 {
                    content.body.push_str("T*\n");
                }
                let _ = writeln!(content.body, "({}) Tj", escape_show_text(line));
            }
            content.body.push_str("ET\n");
        }
        AnnotationKind::Ink { strokes } => {
            content.body.push_str(&format!(
                "{r} {g} {b} RG\n{} w\n1 J\n1 j\n",
                num(stroke * 1.5)
            ));
            for stroke_points in strokes {
                if stroke_points.len() < 2 {
                    continue;
                }
                let _ = writeln!(
                    content.body,
                    "{} {} m",
                    num(stroke_points[0][0]),
                    num(stroke_points[0][1])
                );
                for point in &stroke_points[1..] {
                    let _ = writeln!(content.body, "{} {} l", num(point[0]), num(point[1]));
                }
                content.body.push_str("S\n");
            }
        }
        AnnotationKind::Note { .. } => {}
    }
    content
}

#[derive(Default)]
struct ContentStream {
    body: String,
    uses_font: bool,
}

impl ContentStream {
    fn ext_gstate(&mut self, opacity: f32) {
        if opacity < 0.999 {
            self.body.push_str("/GS0 gs\n");
        }
    }

    fn wrap(self, rect: Quad) -> String {
        let mut content = String::new();
        content.push_str("q\n1 0 0 1 ");
        content.push_str(&num(-rect[0]));
        content.push(' ');
        content.push_str(&num(-rect[1]));
        content.push_str(" cm\n");
        content.push_str(&self.body);
        content.push_str("Q\n");
        content
    }
}

fn subtype_name(kind: &AnnotationKind) -> &'static str {
    match kind {
        AnnotationKind::Highlight { .. } => "Highlight",
        AnnotationKind::Underline { .. } => "Underline",
        AnnotationKind::StrikeOut { .. } => "StrikeOut",
        AnnotationKind::Square => "Square",
        AnnotationKind::Circle => "Circle",
        AnnotationKind::Line { .. } => "Line",
        AnnotationKind::Arrow { .. } => "Line",
        AnnotationKind::FreeText { .. } => "FreeText",
        AnnotationKind::Note { .. } => "Text",
        AnnotationKind::Ink { .. } => "Ink",
    }
}

fn default_color(kind: &AnnotationKind) -> [f32; 3] {
    match kind {
        AnnotationKind::Highlight { .. } => [1.0, 0.92, 0.23],
        AnnotationKind::Note { .. } | AnnotationKind::FreeText { .. } => [0.95, 0.72, 0.12],
        AnnotationKind::Ink { .. } => [0.85, 0.15, 0.15],
        _ => [0.16, 0.44, 0.88],
    }
}

fn default_opacity(kind: &AnnotationKind) -> f32 {
    match kind {
        AnnotationKind::Highlight { .. } => 0.45,
        AnnotationKind::FreeText { .. } => 0.95,
        _ => 1.0,
    }
}

fn ext_gstate_object(opacity: f32) -> Object {
    let mut dict = Dictionary::new();
    dict.set("Type", "ExtGState");
    dict.set("ca", Object::Real(opacity));
    dict.set("CA", Object::Real(opacity));
    dict.set("BM", Object::Name(b"Multiply".to_vec()));
    Object::Dictionary(dict)
}

/// Add annotations to a document and write the result to `output`.
pub fn add_annotations(
    input: &Path,
    output: &Path,
    annotations: &[AnnotationInput],
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    if annotations.is_empty() {
        return Err(TdxError::InvalidInput(
            "There is nothing to write: no annotations were provided".into(),
        ));
    }

    sink.stage(Stage::Reading, 0.0);
    cancel.check()?;
    let mut document = load_pdf(input)?;
    let pages: BTreeMap<u32, ObjectId> = document.get_pages();
    if pages.is_empty() {
        return Err(TdxError::Corrupt("The document has no pages".into()));
    }
    let page_count = pages.len() as u32;

    let font_id = add_helvetica(&mut document);

    let total = annotations.len();
    for (index, annotation) in annotations.iter().enumerate() {
        cancel.check()?;
        if annotation.page == 0 || annotation.page > page_count {
            return Err(TdxError::InvalidInput(format!(
                "Page {} is outside this document (1-{page_count})",
                annotation.page
            )));
        }
        let color = color_tuple(annotation.color, default_color(&annotation.kind));
        let opacity = opacity_value(
            annotation
                .opacity
                .or(Some(default_opacity(&annotation.kind))),
        );
        let rect = match &annotation.kind {
            AnnotationKind::Highlight { quads }
            | AnnotationKind::Underline { quads }
            | AnnotationKind::StrikeOut { quads } => quad_union(quads),
            _ => normalize_rect(annotation.rect),
        };

        let mut dict = Dictionary::new();
        dict.set("Type", "Annot");
        dict.set("Subtype", subtype_name(&annotation.kind));
        dict.set(
            "Rect",
            vec![
                Object::Real(rect[0]),
                Object::Real(rect[1]),
                Object::Real(rect[2]),
                Object::Real(rect[3]),
            ],
        );
        dict.set(
            "C",
            vec![
                Object::Real(color[0]),
                Object::Real(color[1]),
                Object::Real(color[2]),
            ],
        );
        dict.set("F", Object::Integer(4));
        if let Some(author) = annotation.author.as_deref().filter(|text| !text.is_empty()) {
            dict.set("T", encode_text_object(author));
        }
        if let Some(contents) = annotation
            .contents
            .as_deref()
            .filter(|text| !text.is_empty())
        {
            dict.set("Contents", encode_text_object(contents));
        } else {
            match &annotation.kind {
                AnnotationKind::Note { text } => {
                    dict.set("Contents", encode_text_object(text));
                }
                AnnotationKind::FreeText { text, .. } => {
                    dict.set("Contents", encode_text_object(text));
                }
                _ => {}
            }
        }
        dict.set("CA", Object::Real(opacity));
        if let Some(page_id) = pages.get(&annotation.page) {
            dict.set("P", Object::Reference(*page_id));
        }

        match &annotation.kind {
            AnnotationKind::Highlight { quads } => {
                dict.set("QuadPoints", quad_points(quads));
                dict.set("BM", Object::Name(b"Multiply".to_vec()));
            }
            AnnotationKind::Underline { quads } | AnnotationKind::StrikeOut { quads } => {
                dict.set("QuadPoints", quad_points(quads));
            }
            AnnotationKind::Line { from, to } | AnnotationKind::Arrow { from, to } => {
                dict.set(
                    "L",
                    vec![
                        Object::Real(from[0]),
                        Object::Real(from[1]),
                        Object::Real(to[0]),
                        Object::Real(to[1]),
                    ],
                );
            }
            AnnotationKind::FreeText { font_size, .. } => {
                dict.set(
                    "DA",
                    Object::String(
                        format!("/Helv {} Tf 0 g", num(*font_size)).into_bytes(),
                        lopdf::StringFormat::Literal,
                    ),
                );
            }
            AnnotationKind::Note { .. } => {
                dict.set("Name", Object::Name(b"Comment".to_vec()));
            }
            AnnotationKind::Ink { strokes } => {
                let list = strokes
                    .iter()
                    .map(|stroke| {
                        Object::Array(
                            stroke
                                .iter()
                                .flat_map(|point| [Object::Real(point[0]), Object::Real(point[1])])
                                .collect(),
                        )
                    })
                    .collect();
                dict.set("InkList", Object::Array(list));
            }
            _ => {}
        }

        if !matches!(annotation.kind, AnnotationKind::Note { .. }) {
            let content = shape_content(&annotation.kind, color, opacity, rect);
            let mut appearance = Dictionary::new();
            appearance.set("Type", "XObject");
            appearance.set("Subtype", "Form");
            appearance.set(
                "BBox",
                vec![
                    Object::Real(0.0),
                    Object::Real(0.0),
                    Object::Real(rect[2] - rect[0]),
                    Object::Real(rect[3] - rect[1]),
                ],
            );
            let mut resources = Dictionary::new();
            if content.uses_font {
                let mut fonts = Dictionary::new();
                fonts.set("Helv", Object::Reference(font_id));
                resources.set("Font", Object::Dictionary(fonts));
            }
            if opacity < 0.999 {
                let mut states = Dictionary::new();
                states.set("GS0", ext_gstate_object(opacity));
                resources.set("ExtGState", Object::Dictionary(states));
            }
            appearance.set("Resources", Object::Dictionary(resources));

            let stream = Stream::new(appearance, content.wrap(rect).into_bytes());
            let appearance_id = document.add_object(Object::Stream(stream));
            let mut ap = Dictionary::new();
            ap.set("N", Object::Reference(appearance_id));
            dict.set("AP", Object::Dictionary(ap));
        }

        let annotation_id = document.add_object(Object::Dictionary(dict));
        if let Some(page_id) = pages.get(&annotation.page).copied() {
            attach_annotation(&mut document, page_id, annotation_id)?;
        }

        sink.stage(
            Stage::Processing,
            0.2 + 0.6 * (index + 1) as f32 / total as f32,
        );
    }

    sink.stage(Stage::Writing, 0.85);
    save_document(&mut document, output)?;
    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Pdf);
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    result.bytes_out = std::fs::metadata(output).map(|m| m.len()).unwrap_or(0);
    result.set_stat("annotations", total);
    result.set_stat("pages", page_count);
    if let Some(warning) = latin1_warning(annotations) {
        result.warn(warning);
    }
    sink.stage(Stage::Verifying, 0.95);
    for warning in verify_pdf(output, page_count as usize)? {
        result.warn(warning);
    }
    Ok(result.finalize())
}

fn quad_points(quads: &[Quad]) -> Object {
    let mut points = Vec::with_capacity(quads.len() * 8);
    for quad in quads {
        let [x0, y0, x1, y1] = normalize_rect(*quad);
        points.push(Object::Real(x0));
        points.push(Object::Real(y1));
        points.push(Object::Real(x1));
        points.push(Object::Real(y1));
        points.push(Object::Real(x0));
        points.push(Object::Real(y0));
        points.push(Object::Real(x1));
        points.push(Object::Real(y0));
    }
    Object::Array(points)
}

fn add_helvetica(document: &mut Document) -> ObjectId {
    let mut font = Dictionary::new();
    font.set("Type", "Font");
    font.set("Subtype", "Type1");
    font.set("BaseFont", "Helvetica");
    font.set("Encoding", "WinAnsiEncoding");
    document.add_object(Object::Dictionary(font))
}

fn attach_annotation(
    document: &mut Document,
    page_id: ObjectId,
    annotation_id: ObjectId,
) -> Result<()> {
    let existing = match document.get_object(page_id) {
        Ok(Object::Dictionary(dict)) => match dict.get(b"Annots") {
            Ok(Object::Array(items)) => items.clone(),
            Ok(Object::Reference(id)) => match document.get_object(*id) {
                Ok(Object::Array(items)) => items.clone(),
                _ => Vec::new(),
            },
            _ => Vec::new(),
        },
        _ => {
            return Err(TdxError::Corrupt("Page object is missing".into()));
        }
    };
    let mut items = existing;
    items.push(Object::Reference(annotation_id));
    if let Ok(Object::Dictionary(dict)) = document.get_object_mut(page_id) {
        dict.set("Annots", Object::Array(items));
    }
    Ok(())
}

/// Short description of an existing annotation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationSummary {
    pub page: u32,
    pub subtype: String,
    pub rect: Quad,
    pub author: Option<String>,
    pub contents: Option<String>,
}

/// Read annotation metadata without executing anything from the document.
pub fn list_annotations(path: &Path) -> Result<Vec<AnnotationSummary>> {
    let document = load_pdf(path)?;
    let pages = document.get_pages();
    let mut summaries = Vec::new();
    for (number, page_id) in pages {
        let Ok(Object::Dictionary(page)) = document.get_object(page_id) else {
            continue;
        };
        let Ok(annots) = page.get(b"Annots") else {
            continue;
        };
        let items = match annots {
            Object::Array(items) => items.clone(),
            Object::Reference(id) => match document.get_object(*id) {
                Ok(Object::Array(items)) => items.clone(),
                _ => continue,
            },
            _ => continue,
        };
        for item in items {
            let Ok(id) = item.as_reference() else {
                continue;
            };
            let Ok(Object::Dictionary(dict)) = document.get_object(id) else {
                continue;
            };
            let subtype = dict
                .get(b"Subtype")
                .ok()
                .and_then(|value| value.as_name().ok())
                .map(|name| String::from_utf8_lossy(name).to_string())
                .unwrap_or_else(|| "Unknown".to_string());
            let rect = dict
                .get(b"Rect")
                .ok()
                .and_then(|value| value.as_array().ok())
                .map(|values| {
                    let mut out = [0.0f32; 4];
                    for (index, value) in values.iter().take(4).enumerate() {
                        out[index] = crate::build::object_to_f32(value).unwrap_or(0.0);
                    }
                    out
                })
                .unwrap_or([0.0; 4]);
            summaries.push(AnnotationSummary {
                page: number,
                subtype,
                rect,
                author: read_text(dict, b"T"),
                contents: read_text(dict, b"Contents"),
            });
        }
    }
    Ok(summaries)
}

fn read_text(dict: &Dictionary, key: &[u8]) -> Option<String> {
    let value = dict.get(key).ok()?;
    let bytes = match value {
        Object::String(bytes, _) => bytes.clone(),
        _ => return None,
    };
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect();
        String::from_utf16(&units).ok()
    } else {
        Some(String::from_utf8_lossy(&bytes).to_string())
    }
}

/// Human-readable warning about annotation fidelity.
pub fn latin1_warning(annotations: &[AnnotationInput]) -> Option<String> {
    for annotation in annotations {
        let text = match &annotation.kind {
            AnnotationKind::FreeText { text, .. } | AnnotationKind::Note { text } => text.as_str(),
            _ => "",
        };
        if !text.is_empty() && !is_latin1(text) {
            return Some(
                "Text annotations use the built-in Helvetica font for their appearance; \
                 characters outside Latin-1 may not be visible in every reader."
                    .to_string(),
            );
        }
    }
    None
}

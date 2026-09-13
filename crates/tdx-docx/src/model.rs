//! Rich document model used by the desktop editor.
//!
//! The model is intentionally small but round-trips through real OOXML:
//! paragraphs, headings, bullet and numbered lists, quotes, alignment and text
//! runs with bold, italic, underline, size, font family and color.

use std::path::Path;

use serde::{Deserialize, Serialize};
use tdx_core::error::{Result, TdxError};

use crate::{
    build_package_from_document, document_xml, wrap_document, xml_attr, xml_escape, DocxOptions,
};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocModel {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub blocks: Vec<DocBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocBlock {
    pub kind: String,
    #[serde(default)]
    pub align: Option<String>,
    #[serde(default)]
    pub runs: Vec<DocRun>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocRun {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub underline: bool,
    #[serde(default)]
    pub size: Option<u16>,
    #[serde(default)]
    pub font: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
}

fn is_hex_color(value: &str) -> bool {
    value.len() == 6 && value.chars().all(|c| c.is_ascii_hexdigit())
}

fn align_value(align: &str) -> Option<&'static str> {
    match align {
        "left" => Some("left"),
        "center" => Some("center"),
        "right" => Some("right"),
        "justify" => Some("both"),
        _ => None,
    }
}

fn align_from_jc(jc: &str) -> Option<String> {
    match jc {
        "left" => Some("left".into()),
        "center" => Some("center".into()),
        "right" => Some("right".into()),
        "both" | "distribute" => Some("justify".into()),
        _ => None,
    }
}

fn style_id_for_kind(kind: &str) -> Option<&'static str> {
    match kind {
        "heading1" => Some("Heading1"),
        "heading2" => Some("Heading2"),
        "heading3" => Some("Heading3"),
        "bullet" | "numbered" => Some("ListParagraph"),
        "quote" => Some("Quote"),
        _ => None,
    }
}

fn run_xml(run: &DocRun) -> String {
    let mut properties = String::new();
    if run.bold {
        properties.push_str("<w:b/>");
    }
    if run.italic {
        properties.push_str("<w:i/>");
    }
    if run.underline {
        properties.push_str("<w:u w:val=\"single\"/>");
    }
    if let Some(size) = run.size {
        if size > 0 {
            properties.push_str(&format!("<w:sz w:val=\"{size}\"/>"));
        }
    }
    if let Some(font) = &run.font {
        let font = font.trim();
        if !font.is_empty() {
            let escaped = xml_escape(font);
            properties.push_str(&format!(
                "<w:rFonts w:ascii=\"{escaped}\" w:hAnsi=\"{escaped}\" w:cs=\"{escaped}\"/>"
            ));
        }
    }
    if let Some(color) = &run.color {
        let color = color.trim_start_matches('#');
        if is_hex_color(color) {
            properties.push_str(&format!("<w:color w:val=\"{color}\"/>"));
        }
    }
    let properties = if properties.is_empty() {
        String::new()
    } else {
        format!("<w:rPr>{properties}</w:rPr>")
    };
    format!(
        "<w:r>{properties}<w:t xml:space=\"preserve\">{}</w:t></w:r>",
        xml_escape(&run.text)
    )
}

fn paragraph_properties_xml(block: &DocBlock, numbered_prefix: Option<u32>) -> String {
    let mut inner = String::new();
    if let Some(style) = style_id_for_kind(&block.kind) {
        inner.push_str(&format!("<w:pStyle w:val=\"{style}\"/>"));
    }
    if let Some(align) = block.align.as_deref().and_then(align_value) {
        inner.push_str(&format!("<w:jc w:val=\"{align}\"/>"));
    }
    let _ = numbered_prefix;
    if inner.is_empty() {
        String::new()
    } else {
        format!("<w:pPr>{inner}</w:pPr>")
    }
}

fn block_xml(block: &DocBlock, numbered_index: Option<u32>) -> String {
    let runs: String = block.runs.iter().map(run_xml).collect();
    let properties = paragraph_properties_xml(block, numbered_index);
    if block.kind == "numbered" {
        let number = numbered_index.unwrap_or(1);
        let prefix = DocRun {
            text: format!("{number}. "),
            bold: false,
            italic: false,
            underline: false,
            size: None,
            font: None,
            color: None,
        };
        format!("<w:p>{properties}{}{runs}</w:p>", run_xml(&prefix))
    } else if block.kind == "bullet" {
        let prefix = DocRun {
            text: "\u{2022} ".into(),
            bold: false,
            italic: false,
            underline: false,
            size: None,
            font: None,
            color: None,
        };
        format!("<w:p>{properties}{}{runs}</w:p>", run_xml(&prefix))
    } else {
        format!("<w:p>{properties}{runs}</w:p>")
    }
}

/// Build the `word/document.xml` payload for a model.
pub fn document_from_model(model: &DocModel) -> String {
    let mut body = String::new();
    let mut numbered = 0u32;
    for (index, block) in model.blocks.iter().enumerate() {
        let numbered_index = if block.kind == "numbered" {
            numbered += 1;
            Some(numbered)
        } else {
            numbered = 0;
            None
        };
        let _ = index;
        body.push_str(&block_xml(block, numbered_index));
    }
    if model.blocks.is_empty() {
        body.push_str("<w:p><w:r><w:t xml:space=\"preserve\"></w:t></w:r></w:p>");
    }
    wrap_document(&body)
}

/// Save a model as a real DOCX file.
pub fn save_model(model: &DocModel, output: &Path) -> Result<()> {
    let document = document_from_model(model);
    let options = DocxOptions {
        title: model.title.clone(),
        author: None,
    };
    let bytes = build_package_from_document(&document, &options)?;
    tdx_core::fsutil::atomic_write(output, |file| {
        std::io::Write::write_all(file, &bytes).map_err(TdxError::Io)
    })
}

fn parse_run_properties(properties: &str) -> DocRun {
    let mut run = DocRun {
        bold: properties.contains("<w:b/>") || properties.contains("<w:b "),
        italic: properties.contains("<w:i/>") || properties.contains("<w:i "),
        underline: properties.contains("<w:u "),
        ..DocRun::default()
    };
    if let Some(index) = properties.find("<w:sz") {
        let tag_end = properties[index..].find('>').map(|end| index + end);
        if let Some(tag_end) = tag_end {
            if let Some(value) = xml_attr(&properties[index..tag_end], "w:val") {
                run.size = value.parse::<u16>().ok();
            }
        }
    }
    if let Some(index) = properties.find("<w:rFonts") {
        let tag_end = properties[index..].find('>').map(|end| index + end);
        if let Some(tag_end) = tag_end {
            run.font = xml_attr(&properties[index..tag_end], "w:ascii")
                .or_else(|| xml_attr(&properties[index..tag_end], "w:hAnsi"));
        }
    }
    if let Some(index) = properties.find("<w:color") {
        let tag_end = properties[index..].find('>').map(|end| index + end);
        if let Some(tag_end) = tag_end {
            if let Some(value) = xml_attr(&properties[index..tag_end], "w:val") {
                if is_hex_color(&value) {
                    run.color = Some(value);
                }
            }
        }
    }
    run
}

fn run_text(segment: &str) -> String {
    let mut text = String::new();
    let mut cursor = segment;
    while let Some(start) = cursor.find("<w:t") {
        let after = &cursor[start..];
        let Some(open_end) = after.find('>') else {
            break;
        };
        let content_start = open_end + 1;
        let Some(close) = after[content_start..].find("</w:t>") else {
            break;
        };
        text.push_str(&crate::xml_unescape(
            &after[content_start..content_start + close],
        ));
        cursor = &after[content_start + close..];
    }
    text
}

fn strip_number_prefix(text: &str) -> Option<(u32, String)> {
    let digits: String = text.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() || digits.len() > 3 {
        return None;
    }
    let rest = &text[digits.len()..];
    let rest = rest.strip_prefix(". ").or_else(|| rest.strip_prefix("."))?;
    let number: u32 = digits.parse().ok()?;
    Some((number, rest.trim_start().to_string()))
}

/// Parse paragraphs, runs and formatting from `word/document.xml`.
pub fn parse_model_xml(xml: &str) -> Vec<DocBlock> {
    let mut blocks = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<w:p") {
        let after_start = &rest[start..];
        let is_paragraph = after_start.starts_with("<w:p>") || after_start.starts_with("<w:p ");
        if !is_paragraph {
            if let Some(next) = after_start[4..].find("<w:p") {
                rest = &after_start[4 + next..];
                continue;
            }
            break;
        }
        let Some(end) = after_start.find("</w:p>") else {
            break;
        };
        let block = &after_start[..end];

        let mut kind = "paragraph".to_string();
        let mut align = None;
        if let Some(properties_start) = block.find("<w:pPr") {
            if let Some(properties_end) = block[properties_start..].find("</w:pPr>") {
                let properties = &block[properties_start..properties_start + properties_end];
                if let Some(index) = properties.find("<w:pStyle") {
                    let tag_end = properties[index..].find('>').map(|value| index + value);
                    if let Some(tag_end) = tag_end {
                        if let Some(style) = xml_attr(&properties[index..tag_end], "w:val") {
                            kind = match style.as_str() {
                                "Heading1" => "heading1".into(),
                                "Heading2" => "heading2".into(),
                                "Heading3" => "heading3".into(),
                                "ListParagraph" => "bullet".into(),
                                "Quote" => "quote".into(),
                                _ => "paragraph".into(),
                            };
                        }
                    }
                }
                if let Some(index) = properties.find("<w:jc") {
                    let tag_end = properties[index..].find('>').map(|value| index + value);
                    if let Some(tag_end) = tag_end {
                        if let Some(jc) = xml_attr(&properties[index..tag_end], "w:val") {
                            align = align_from_jc(&jc);
                        }
                    }
                }
            }
        }

        let mut runs: Vec<DocRun> = Vec::new();
        let mut cursor = block;
        while let Some(run_start) = cursor.find("<w:r>").or_else(|| cursor.find("<w:r ")) {
            let after_run = &cursor[run_start..];
            let Some(run_end) = after_run.find("</w:r>") else {
                break;
            };
            let run_segment = &after_run[..run_end];
            let mut run = DocRun::default();
            if let Some(properties_start) = run_segment.find("<w:rPr") {
                if let Some(properties_end) = run_segment[properties_start..].find("</w:rPr>") {
                    run = parse_run_properties(
                        &run_segment[properties_start..properties_start + properties_end],
                    );
                }
            }
            run.text = run_text(run_segment);
            runs.push(run);
            cursor = &after_run[run_end + "</w:r>".len()..];
        }

        if let Some(first) = runs.first_mut() {
            if let Some(stripped) = first.text.strip_prefix("\u{2022} ") {
                first.text = stripped.to_string();
            }
        }
        if runs.first().map(|run| run.text.is_empty()).unwrap_or(false) {
            runs.remove(0);
        }
        if kind == "bullet" || kind == "paragraph" {
            if let Some(first) = runs.first() {
                if let Some((_, stripped)) = strip_number_prefix(&first.text) {
                    kind = "numbered".into();
                    if let Some(first) = runs.first_mut() {
                        first.text = stripped;
                    }
                    if runs.first().map(|run| run.text.is_empty()).unwrap_or(false) {
                        runs.remove(0);
                    }
                }
            }
        }

        if runs.is_empty() {
            runs.push(DocRun::default());
        }
        blocks.push(DocBlock { kind, align, runs });
        rest = &after_start[end + "</w:p>".len()..];
    }
    blocks
}

fn read_core_title(input: &Path) -> Option<String> {
    let file = std::fs::File::open(input).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let mut core = archive.by_name("docProps/core.xml").ok()?;
    let mut text = String::new();
    std::io::Read::read_to_string(&mut core, &mut text).ok()?;
    let start = text.find("<dc:title>").map(|index| index + 10)?;
    let end = text.find("</dc:title>")?;
    if end <= start {
        return None;
    }
    let title = crate::xml_unescape(&text[start..end]);
    if title.trim().is_empty() {
        None
    } else {
        Some(title)
    }
}

/// Load a DOCX file into the editor model.
pub fn load_model(input: &Path) -> Result<DocModel> {
    let xml = document_xml(input)?;
    let blocks = parse_model_xml(&xml);
    Ok(DocModel {
        title: read_core_title(input).or_else(|| {
            input
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(|stem| stem.to_string())
        }),
        blocks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_formatting() {
        let model = DocModel {
            title: Some("Demo".into()),
            blocks: vec![
                DocBlock {
                    kind: "heading1".into(),
                    align: Some("center".into()),
                    runs: vec![DocRun {
                        text: "Title".into(),
                        bold: true,
                        ..DocRun::default()
                    }],
                },
                DocBlock {
                    kind: "paragraph".into(),
                    align: None,
                    runs: vec![
                        DocRun {
                            text: "plain ".into(),
                            ..DocRun::default()
                        },
                        DocRun {
                            text: "bold".into(),
                            bold: true,
                            italic: true,
                            underline: true,
                            size: Some(28),
                            font: Some("Georgia".into()),
                            color: Some("ff0000".into()),
                        },
                    ],
                },
                DocBlock {
                    kind: "bullet".into(),
                    align: None,
                    runs: vec![DocRun {
                        text: "item".into(),
                        ..DocRun::default()
                    }],
                },
                DocBlock {
                    kind: "numbered".into(),
                    align: None,
                    runs: vec![DocRun {
                        text: "first".into(),
                        ..DocRun::default()
                    }],
                },
            ],
        };
        let xml = document_from_model(&model);
        let parsed = parse_model_xml(&xml);
        assert_eq!(parsed.len(), 4);
        assert_eq!(parsed[0].kind, "heading1");
        assert_eq!(parsed[0].align.as_deref(), Some("center"));
        assert!(parsed[0].runs[0].bold);
        assert_eq!(parsed[1].runs[1].text, "bold");
        assert!(parsed[1].runs[1].italic && parsed[1].runs[1].underline);
        assert_eq!(parsed[1].runs[1].size, Some(28));
        assert_eq!(parsed[1].runs[1].font.as_deref(), Some("Georgia"));
        assert_eq!(parsed[1].runs[1].color.as_deref(), Some("ff0000"));
        assert_eq!(parsed[2].kind, "bullet");
        assert_eq!(parsed[2].runs[0].text, "item");
        assert_eq!(parsed[3].kind, "numbered");
        assert_eq!(parsed[3].runs[0].text, "first");
    }
}

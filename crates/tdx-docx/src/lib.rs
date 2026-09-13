//! DOCX reader/writer for TEDROX Documents.
//!
//! The writer produces a minimal, valid OOXML package. The reader extracts
//! text, headings and simple inline formatting. Unsupported OOXML parts are
//! preserved on disk: this crate never rewrites an existing file in place.

use std::io::{Cursor, Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use tdx_core::{
    detect::FileKind,
    error::{Result, TdxError},
    progress::{CancelToken, ProgressSink, Stage},
    result::OperationResult,
};
use zip::write::SimpleFileOptions;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocxOptions {
    pub title: Option<String>,
    pub author: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocxInfo {
    pub paragraphs: u64,
    pub words: u64,
    pub characters: u64,
    pub headings: u64,
    pub title: Option<String>,
    pub author: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockKind {
    Paragraph,
    Heading(u8),
    Bullet,
    Numbered,
}

#[derive(Debug, Clone)]
struct Block {
    kind: BlockKind,
    runs: Vec<Run>,
}

#[derive(Debug, Clone)]
struct Run {
    text: String,
    bold: bool,
    italic: bool,
}

fn xml_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn inline_runs(text: &str) -> Vec<Run> {
    // Minimal inline parser: **bold**, *italic*, `code`, [label](url).
    let mut runs = Vec::new();
    let mut current = String::new();
    let mut bold = false;
    let mut italic = false;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '*' => {
                let double = chars.peek() == Some(&'*');
                if double {
                    chars.next();
                    if bold && current.is_empty() {
                        bold = false;
                        continue;
                    }
                    if !current.is_empty() {
                        runs.push(Run {
                            text: std::mem::take(&mut current),
                            bold,
                            italic,
                        });
                    }
                    bold = !bold;
                } else {
                    if !current.is_empty() {
                        runs.push(Run {
                            text: std::mem::take(&mut current),
                            bold,
                            italic,
                        });
                    }
                    italic = !italic;
                }
            }
            '[' => {
                let mut label = String::new();
                let mut rest = chars.clone();
                while let Some(next) = rest.next() {
                    if next == ']' {
                        if rest.peek() == Some(&'(') {
                            rest.next();
                            let mut url = String::new();
                            for next in rest.by_ref() {
                                if next == ')' {
                                    break;
                                }
                                url.push(next);
                            }
                            current.push_str(&label);
                            if !url.is_empty() {
                                current.push_str(&format!(" ({url})"));
                            }
                            chars = rest;
                            break;
                        }
                        break;
                    }
                    label.push(next);
                }
                if label.is_empty() {
                    current.push('[');
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        runs.push(Run {
            text: current,
            bold,
            italic,
        });
    }
    runs
}

fn markdown_blocks(markdown: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut paragraph: Vec<String> = Vec::new();
    let flush = |paragraph: &mut Vec<String>, blocks: &mut Vec<Block>| {
        if paragraph.is_empty() {
            return;
        }
        let text = paragraph.join(" ");
        blocks.push(Block {
            kind: BlockKind::Paragraph,
            runs: inline_runs(&text),
        });
        paragraph.clear();
    };
    for line in markdown.lines() {
        let trimmed = line.trim_end();
        if trimmed.trim().is_empty() {
            flush(&mut paragraph, &mut blocks);
            continue;
        }
        let heading = trimmed
            .trim_start()
            .chars()
            .take_while(|c| *c == '#')
            .count();
        if heading > 0
            && heading <= 6
            && trimmed.trim_start().as_bytes().get(heading) == Some(&b' ')
        {
            flush(&mut paragraph, &mut blocks);
            let text = trimmed.trim_start()[heading + 1..].trim();
            blocks.push(Block {
                kind: BlockKind::Heading(heading as u8),
                runs: inline_runs(text),
            });
            continue;
        }
        let content = trimmed.trim_start();
        if let Some(rest) = content
            .strip_prefix("- ")
            .or_else(|| content.strip_prefix("* "))
            .or_else(|| content.strip_prefix("+ "))
        {
            flush(&mut paragraph, &mut blocks);
            blocks.push(Block {
                kind: BlockKind::Bullet,
                runs: inline_runs(rest),
            });
            continue;
        }
        if let Some((number, rest)) = split_numbered(content) {
            flush(&mut paragraph, &mut blocks);
            blocks.push(Block {
                kind: BlockKind::Numbered,
                runs: inline_runs(&format!("{number}. {rest}")),
            });
            continue;
        }
        paragraph.push(content.to_string());
    }
    flush(&mut paragraph, &mut blocks);
    if blocks.is_empty() {
        blocks.push(Block {
            kind: BlockKind::Paragraph,
            runs: vec![Run {
                text: String::new(),
                bold: false,
                italic: false,
            }],
        });
    }
    blocks
}

fn split_numbered(line: &str) -> Option<(String, String)> {
    let mut digits = String::new();
    let mut chars = line.chars();
    while let Some(ch) = chars.next() {
        if ch.is_ascii_digit() {
            digits.push(ch);
        } else if ch == '.' && !digits.is_empty() {
            let rest: String = chars.collect();
            let rest = rest.strip_prefix(' ').unwrap_or(&rest).to_string();
            return Some((digits, rest));
        } else {
            return None;
        }
    }
    None
}

fn run_xml(run: &Run) -> String {
    let mut properties = String::new();
    if run.bold {
        properties.push_str("<w:b/>");
    }
    if run.italic {
        properties.push_str("<w:i/>");
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

fn block_xml(block: &Block) -> String {
    let runs: String = block.runs.iter().map(run_xml).collect();
    match block.kind {
        BlockKind::Heading(level) => format!(
            "<w:p><w:pPr><w:pStyle w:val=\"Heading{}\"/></w:pPr>{runs}</w:p>",
            level.min(3)
        ),
        BlockKind::Bullet => format!(
            "<w:p><w:pPr><w:pStyle w:val=\"ListParagraph\"/></w:pPr><w:r><w:t xml:space=\"preserve\">\u{2022} </w:t></w:r>{runs}</w:p>"
        ),
        BlockKind::Numbered => format!(
            "<w:p><w:pPr><w:pStyle w:val=\"ListParagraph\"/></w:pPr>{runs}</w:p>"
        ),
        BlockKind::Paragraph => format!("<w:p>{runs}</w:p>"),
    }
}

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/><Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/></Types>"#;

const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/></Relationships>"#;

const DOCUMENT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#;

const STYLES_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:pPr><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/><w:sz w:val="36"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/><w:basedOn w:val="Normal"/><w:pPr><w:outlineLvl w:val="1"/></w:pPr><w:rPr><w:b/><w:sz w:val="30"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading3"><w:name w:val="heading 3"/><w:basedOn w:val="Normal"/><w:pPr><w:outlineLvl w:val="2"/></w:pPr><w:rPr><w:b/><w:sz w:val="26"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="ListParagraph"><w:name w:val="List Paragraph"/><w:basedOn w:val="Normal"/><w:pPr><w:ind w:left="720"/></w:pPr></w:style></w:styles>"#;

fn core_xml(options: &DocxOptions) -> String {
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let title = options.title.clone().unwrap_or_default();
    let author = options
        .author
        .clone()
        .unwrap_or_else(|| "TEDROX Documents".to_string());
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:title>{title}</dc:title><dc:creator>{author}</dc:creator><cp:lastModifiedBy>{author}</cp:lastModifiedBy><dcterms:created xsi:type="dcterms:W3CDTF">{now}</dcterms:created><dcterms:modified xsi:type="dcterms:W3CDTF">{now}</dcterms:modified></cp:coreProperties>"#,
        title = xml_escape(&title),
        author = xml_escape(&author),
    )
}

const APP_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"><Application>TEDROX Documents</Application></Properties>"#;

fn build_package(blocks: &[Block], options: &DocxOptions) -> Result<Vec<u8>> {
    let body: String = blocks.iter().map(block_xml).collect();
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="1134" w:bottom="1134" w:left="1134"/></w:sectPr></w:body></w:document>"#
    );

    let mut buffer = Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut buffer);
        let options_stored =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let options_deflate =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let mut write = |name: &str, data: &str, options: SimpleFileOptions| -> Result<()> {
            writer
                .start_file(name.to_string(), options)
                .map_err(|err| TdxError::Other(format!("ZIP error: {err}")))?;
            writer.write_all(data.as_bytes()).map_err(TdxError::Io)?;
            Ok(())
        };
        write("[Content_Types].xml", CONTENT_TYPES, options_deflate)?;
        write("_rels/.rels", ROOT_RELS, options_deflate)?;
        write("word/document.xml", &document, options_deflate)?;
        write("word/styles.xml", STYLES_XML, options_deflate)?;
        write(
            "word/_rels/document.xml.rels",
            DOCUMENT_RELS,
            options_deflate,
        )?;
        write("docProps/core.xml", &core_xml(options), options_deflate)?;
        write("docProps/app.xml", APP_XML, options_deflate)?;
        let _ = options_stored;
        writer
            .finish()
            .map_err(|err| TdxError::Other(format!("ZIP error: {err}")))?;
    }
    Ok(buffer.into_inner())
}

fn document_xml(input: &Path) -> Result<String> {
    let file = std::fs::File::open(input).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => TdxError::not_found(input),
        _ => TdxError::Io(err),
    })?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|err| TdxError::Corrupt(format!("This is not a DOCX package: {err}")))?;
    let mut document = archive
        .by_name("word/document.xml")
        .map_err(|_| TdxError::Corrupt("word/document.xml is missing".into()))?;
    let mut text = String::new();
    document
        .read_to_string(&mut text)
        .map_err(|err| TdxError::Corrupt(format!("Cannot read document.xml: {err}")))?;
    Ok(text)
}

fn xml_attr(tag: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')? + start;
    Some(tag[start..end].to_string())
}

fn parse_paragraphs(xml: &str) -> Vec<(Option<String>, String)> {
    let mut paragraphs = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<w:p") {
        let after_start = &rest[start..];
        let Some(end) = after_start.find("</w:p>") else {
            break;
        };
        let block = &after_start[..end];
        let style = block.find("<w:pStyle").and_then(|index| {
            let tag_end = block[index..].find('>')? + index;
            xml_attr(&block[index..tag_end], "w:val")
        });
        let mut text = String::new();
        let mut cursor = block;
        while let Some(text_start) = cursor.find("<w:t") {
            let after = &cursor[text_start..];
            let Some(open_end) = after.find('>') else {
                break;
            };
            let content_start = open_end + 1;
            let Some(close) = after[content_start..].find("</w:t>") else {
                break;
            };
            text.push_str(&xml_unescape(&after[content_start..content_start + close]));
            cursor = &after[content_start + close..];
        }
        // Runs separated by breaks keep a space.
        paragraphs.push((style, text));
        rest = &after_start[end + "</w:p>".len()..];
    }
    paragraphs
}

/// Extract plain text (one paragraph per line).
pub fn extract_text(input: &Path) -> Result<String> {
    let xml = document_xml(input)?;
    let paragraphs = parse_paragraphs(&xml);
    let mut out = String::new();
    for (_, text) in paragraphs {
        out.push_str(&text);
        out.push('\n');
    }
    Ok(out)
}

/// Extract markdown (headings from paragraph styles; bold/italic not preserved).
pub fn extract_markdown(input: &Path) -> Result<String> {
    let xml = document_xml(input)?;
    let paragraphs = parse_paragraphs(&xml);
    let mut out = String::new();
    for (style, text) in paragraphs {
        match style.as_deref() {
            Some("Heading1") => out.push_str(&format!("# {text}\n\n")),
            Some("Heading2") => out.push_str(&format!("## {text}\n\n")),
            Some("Heading3") => out.push_str(&format!("### {text}\n\n")),
            Some("ListParagraph") => out.push_str(&format!("- {text}\n")),
            _ => {
                out.push_str(&text);
                out.push('\n');
            }
        }
    }
    Ok(out)
}

/// Create a DOCX from Markdown text.
pub fn from_markdown(
    markdown: &str,
    output: &Path,
    options: &DocxOptions,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    sink.stage(Stage::Processing, 0.3);
    cancel.check()?;
    let blocks = markdown_blocks(markdown);
    let bytes = build_package(&blocks, options)?;
    sink.stage(Stage::Writing, 0.8);
    tdx_core::fsutil::atomic_write(output, |file| file.write_all(&bytes).map_err(TdxError::Io))?;
    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Docx);
    result.bytes_in = markdown.len() as u64;
    result.set_stat("paragraphs", blocks.len());
    Ok(result.finalize())
}

/// Create a DOCX from a Markdown or text file.
pub fn create_from_file(
    input: &Path,
    output: &Path,
    options: &DocxOptions,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    let text = std::fs::read_to_string(input).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => TdxError::not_found(input),
        std::io::ErrorKind::InvalidData => {
            TdxError::Unsupported("The input file is not UTF-8 text".into())
        }
        _ => TdxError::Io(err),
    })?;
    let text = text.trim_start_matches('\u{feff}').to_string();
    let is_markdown = input
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "md" | "markdown"))
        .unwrap_or(false);
    let markdown = if is_markdown {
        text
    } else {
        text.lines()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n\n")
    };
    let mut result = from_markdown(&markdown, output, options, sink, cancel)?;
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    Ok(result)
}

/// Summary statistics for a DOCX file.
pub fn inspect(input: &Path) -> Result<DocxInfo> {
    let xml = document_xml(input)?;
    let paragraphs = parse_paragraphs(&xml);
    let mut info = DocxInfo {
        paragraphs: paragraphs.len() as u64,
        words: 0,
        characters: 0,
        headings: 0,
        title: None,
        author: None,
    };
    for (style, text) in &paragraphs {
        info.words += text.split_whitespace().count() as u64;
        info.characters += text.chars().count() as u64;
        if style
            .as_deref()
            .map(|s| s.starts_with("Heading"))
            .unwrap_or(false)
        {
            info.headings += 1;
        }
    }
    // Try to read core properties.
    if let Ok(file) = std::fs::File::open(input) {
        if let Ok(mut archive) = zip::ZipArchive::new(file) {
            if let Ok(mut core) = archive.by_name("docProps/core.xml") {
                let mut text = String::new();
                if core.read_to_string(&mut text).is_ok() {
                    let title_start = text.find("<dc:title>").map(|i| i + 10);
                    let title_end = text.find("</dc:title>");
                    if let (Some(start), Some(end)) = (title_start, title_end) {
                        if end > start {
                            info.title = Some(xml_unescape(&text[start..end]));
                        }
                    }
                    let author_start = text.find("<dc:creator>").map(|i| i + 12);
                    let author_end = text.find("</dc:creator>");
                    if let (Some(start), Some(end)) = (author_start, author_end) {
                        if end > start {
                            info.author = Some(xml_unescape(&text[start..end]));
                        }
                    }
                }
            }
        }
    }
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_xml() {
        assert_eq!(xml_escape("a<b>&\"c\""), "a&lt;b&gt;&amp;&quot;c&quot;");
    }

    #[test]
    fn builds_markdown_blocks() {
        let blocks = markdown_blocks("# Title\n\nHello **world**\n\n- one\n- two\n");
        assert_eq!(blocks.len(), 4);
        assert_eq!(blocks[0].kind, BlockKind::Heading(1));
        assert_eq!(blocks[1].runs.len(), 2);
        assert!(blocks[1].runs[1].bold);
        assert_eq!(blocks[2].kind, BlockKind::Bullet);
    }
}

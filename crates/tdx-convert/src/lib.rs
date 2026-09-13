//! Conversion center routing for TEDROX Documents.
//!
//! Native conversions are implemented with printpdf (MIT) and system fonts.
//! Legacy binary Office formats are handled through an optional, clearly
//! documented LibreOffice adapter that is never bundled.

use std::io::Write;
use std::path::{Path, PathBuf};

use printpdf::*;
use serde::{Deserialize, Serialize};
use tdx_core::{
    detect::{detect, FileKind},
    error::{Result, TdxError},
    fsutil,
    progress::{CancelToken, ProgressSink, Stage},
    result::OperationResult,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextStyle {
    Title,
    Heading1,
    Heading2,
    Heading3,
    Body,
    Bullet,
}

impl TextStyle {
    fn size_pt(self) -> f32 {
        match self {
            TextStyle::Title => 24.0,
            TextStyle::Heading1 => 19.0,
            TextStyle::Heading2 => 15.0,
            TextStyle::Heading3 => 13.0,
            TextStyle::Body | TextStyle::Bullet => 11.0,
        }
    }

    fn is_heading(self) -> bool {
        matches!(
            self,
            TextStyle::Title | TextStyle::Heading1 | TextStyle::Heading2 | TextStyle::Heading3
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBlock {
    pub style: TextStyle,
    pub text: String,
}

fn system_font_candidates(bold: bool) -> Vec<PathBuf> {
    let windows = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    let mut candidates: Vec<PathBuf> = Vec::new();
    if bold {
        candidates.push(PathBuf::from(&windows).join("Fonts\\arialbd.ttf"));
        candidates.push(PathBuf::from(&windows).join("Fonts\\segoeuib.ttf"));
    } else {
        candidates.push(PathBuf::from(&windows).join("Fonts\\arial.ttf"));
        candidates.push(PathBuf::from(&windows).join("Fonts\\segoeui.ttf"));
        candidates.push(PathBuf::from(&windows).join("Fonts\\calibri.ttf"));
    }
    candidates.push(PathBuf::from(
        "/System/Library/Fonts/Supplemental/Arial.ttf",
    ));
    candidates.push(PathBuf::from("/Library/Fonts/Arial.ttf"));
    candidates.push(PathBuf::from(
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ));
    candidates.push(PathBuf::from(
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    ));
    candidates.push(PathBuf::from("/usr/share/fonts/TTF/DejaVuSans.ttf"));
    candidates
}

fn find_system_font(bold: bool) -> Option<PathBuf> {
    system_font_candidates(bold)
        .into_iter()
        .find(|path| path.exists())
}

/// True when text contains characters the built-in WinAnsi font cannot draw.
fn needs_external_font(text: &str) -> bool {
    text.chars()
        .any(|ch| !ch.is_ascii() && !('\u{00a0}'..='\u{00ff}').contains(&ch))
}

fn sanitize_for_builtin_font(text: &str) -> String {
    text.chars()
        .map(|ch| {
            if ch.is_ascii() || ('\u{00a0}'..='\u{00ff}').contains(&ch) {
                ch
            } else {
                '?'
            }
        })
        .collect()
}

fn wrap_line(text: &str, max_chars: usize) -> Vec<String> {
    if text.trim().is_empty() {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
        } else if current.chars().count() + 1 + word.chars().count() <= max_chars {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// Render text blocks into a paginated PDF using a system font when available.
pub fn render_blocks_to_pdf(
    blocks: &[TextBlock],
    output: &Path,
    title: &str,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    if blocks.is_empty() {
        return Err(TdxError::InvalidInput("There is no text to render".into()));
    }
    sink.stage(Stage::Processing, 0.2);
    let needs_external = blocks.iter().any(|block| needs_external_font(&block.text));
    let regular_font_path = find_system_font(false);
    let bold_font_path = find_system_font(true);
    let used_font = regular_font_path.clone();
    let fallback_builtin = regular_font_path.is_none();

    let page_width = 210.0f32;
    let page_height = 297.0f32;
    let margin = 18.0f32;
    let top = page_height - margin;
    let bottom = margin;
    let pt_to_mm = 25.4f32 / 72.0;

    let (doc, first_page, first_layer) =
        PdfDocument::new(title, Mm(page_width), Mm(page_height), "Content");
    // Built-in Helvetica keeps Latin-only documents tiny; a system TrueType
    // font is embedded only when the text needs characters outside WinAnsi.
    let regular = if needs_external {
        match &regular_font_path {
            Some(path) => doc
                .add_external_font(std::fs::File::open(path)?)
                .map_err(|err| TdxError::Other(format!("Cannot load system font: {err}")))?,
            None => doc
                .add_builtin_font(BuiltinFont::Helvetica)
                .map_err(|err| TdxError::Other(format!("Cannot load built-in font: {err}")))?,
        }
    } else {
        doc.add_builtin_font(BuiltinFont::Helvetica)
            .map_err(|err| TdxError::Other(format!("Cannot load built-in font: {err}")))?
    };
    let bold = if needs_external {
        match &bold_font_path {
            Some(path) => doc
                .add_external_font(std::fs::File::open(path)?)
                .map_err(|err| TdxError::Other(format!("Cannot load system font: {err}")))?,
            None => regular.clone(),
        }
    } else {
        doc.add_builtin_font(BuiltinFont::HelveticaBold)
            .map_err(|err| TdxError::Other(format!("Cannot load built-in font: {err}")))?
    };

    let mut page_index = first_page;
    let mut layer_index = first_layer;
    let mut y = top;
    let mut warnings: Vec<String> = Vec::new();
    if needs_external && fallback_builtin {
        warnings.push(
            "No system TrueType font was found; non-Latin characters were replaced with '?'".into(),
        );
    }

    let total = blocks.len();
    for (index, block) in blocks.iter().enumerate() {
        cancel.check()?;
        let size = block.style.size_pt();
        let line_height = size * pt_to_mm * 1.45;
        let max_chars = ((page_width - margin * 2.0) / (size * pt_to_mm * 0.52))
            .floor()
            .max(16.0) as usize;
        let indent = if block.style == TextStyle::Bullet {
            6.0
        } else {
            0.0
        };
        let font = if block.style.is_heading() {
            &bold
        } else {
            &regular
        };

        for line in wrap_line(&block.text, max_chars) {
            cancel.check()?;
            if y - line_height < bottom {
                let (next_page, next_layer) =
                    doc.add_page(Mm(page_width), Mm(page_height), "Content");
                page_index = next_page;
                layer_index = next_layer;
                y = top;
            }
            let layer = doc.get_page(page_index).get_layer(layer_index);
            let display = if fallback_builtin && needs_external_font(&line) {
                sanitize_for_builtin_font(&line)
            } else {
                line
            };
            layer.use_text(
                display,
                size,
                Mm(margin + indent),
                Mm(y - size * pt_to_mm),
                font,
            );
            y -= line_height;
        }
        y -= match block.style {
            TextStyle::Body => 2.2,
            TextStyle::Bullet => 0.8,
            _ => 3.2,
        };
        if index % 32 == 0 {
            sink.stage(Stage::Processing, 0.2 + 0.6 * (index as f32 / total as f32));
        }
    }

    sink.stage(Stage::Writing, 0.9);
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut writer = std::io::BufWriter::new(&mut cursor);
        doc.save(&mut writer)
            .map_err(|err| TdxError::Other(format!("Cannot write PDF: {err}")))?;
        writer.flush().map_err(TdxError::Io)?;
    }
    let bytes = cursor.into_inner();
    fsutil::atomic_write(output, |file| file.write_all(&bytes).map_err(TdxError::Io))?;

    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Pdf);
    result.warnings = warnings;
    result.set_stat("blocks", blocks.len());
    result.set_stat(
        "font",
        if needs_external {
            used_font
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "built-in Helvetica".into())
        } else {
            "built-in Helvetica".to_string()
        },
    );
    Ok(result.finalize())
}

fn strip_inline_markdown(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '*' | '_' | '`' => {}
            '[' => {
                let mut label = String::new();
                for next in chars.by_ref() {
                    if next == ']' {
                        break;
                    }
                    label.push(next);
                }
                out.push_str(&label);
                if chars.peek() == Some(&'(') {
                    let mut depth = 0;
                    for next in chars.by_ref() {
                        if next == '(' {
                            depth += 1;
                        } else if next == ')' {
                            if depth == 0 {
                                break;
                            }
                            depth -= 1;
                        }
                    }
                }
            }
            _ => out.push(ch),
        }
    }
    out
}

/// Parse Markdown into PDF text blocks (subset: headings, bullets, paragraphs).
pub fn markdown_to_blocks(markdown: &str) -> Vec<TextBlock> {
    let mut blocks = Vec::new();
    let mut paragraph: Vec<String> = Vec::new();
    let flush = |paragraph: &mut Vec<String>, blocks: &mut Vec<TextBlock>| {
        if paragraph.is_empty() {
            return;
        }
        let text = strip_inline_markdown(&paragraph.join(" "));
        if !text.trim().is_empty() {
            blocks.push(TextBlock {
                style: TextStyle::Body,
                text,
            });
        }
        paragraph.clear();
    };
    for line in markdown.lines() {
        let trimmed = line.trim_end();
        if trimmed.trim().is_empty() {
            flush(&mut paragraph, &mut blocks);
            continue;
        }
        let content = trimmed.trim_start();
        let hashes = content.chars().take_while(|c| *c == '#').count();
        if hashes > 0 && hashes <= 6 && content.as_bytes().get(hashes) == Some(&b' ') {
            flush(&mut paragraph, &mut blocks);
            let text = strip_inline_markdown(content[hashes + 1..].trim());
            let style = match hashes {
                1 => TextStyle::Title,
                2 => TextStyle::Heading1,
                3 => TextStyle::Heading2,
                _ => TextStyle::Heading3,
            };
            blocks.push(TextBlock { style, text });
            continue;
        }
        if let Some(rest) = content
            .strip_prefix("- ")
            .or_else(|| content.strip_prefix("* "))
            .or_else(|| content.strip_prefix("+ "))
        {
            flush(&mut paragraph, &mut blocks);
            blocks.push(TextBlock {
                style: TextStyle::Bullet,
                text: strip_inline_markdown(rest),
            });
            continue;
        }
        paragraph.push(content.to_string());
    }
    flush(&mut paragraph, &mut blocks);
    if blocks.is_empty() {
        blocks.push(TextBlock {
            style: TextStyle::Body,
            text: String::new(),
        });
    }
    blocks
}

fn strip_html_to_blocks(html: &str) -> (Vec<TextBlock>, String) {
    let mut blocks = Vec::new();
    let mut plain = String::new();
    let mut rest: &str = html;
    let mut current_style = TextStyle::Body;
    let mut current = String::new();
    let push =
        |style: TextStyle, text: &mut String, blocks: &mut Vec<TextBlock>, plain: &mut String| {
            let decoded = decode_entities(text.trim());
            if !decoded.is_empty() {
                blocks.push(TextBlock {
                    style,
                    text: decoded.clone(),
                });
                plain.push_str(&decoded);
                plain.push('\n');
            }
            text.clear();
        };
    loop {
        let Some(open) = rest.find('<') else {
            current.push_str(rest);
            break;
        };
        current.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('>') else {
            current.push_str(&rest[open..]);
            break;
        };
        let tag = rest[open + 1..open + close].trim().to_ascii_lowercase();
        if tag.starts_with("script") || tag.starts_with("style") {
            let close_tag = if tag.starts_with("script") {
                "</script>"
            } else {
                "</style>"
            };
            let remainder = &rest[open + close + 1..];
            let lower_remainder = remainder.to_ascii_lowercase();
            if let Some(position) = lower_remainder.find(close_tag) {
                rest = &remainder[position + close_tag.len()..];
                continue;
            }
            break;
        }
        if tag.starts_with("h1") {
            push(current_style, &mut current, &mut blocks, &mut plain);
            current_style = TextStyle::Title;
        } else if tag.starts_with("h2") {
            push(current_style, &mut current, &mut blocks, &mut plain);
            current_style = TextStyle::Heading1;
        } else if tag.starts_with("h3") {
            push(current_style, &mut current, &mut blocks, &mut plain);
            current_style = TextStyle::Heading2;
        } else if tag.starts_with("h4") || tag.starts_with("h5") || tag.starts_with("h6") {
            push(current_style, &mut current, &mut blocks, &mut plain);
            current_style = TextStyle::Heading3;
        } else if tag.starts_with("li") {
            push(current_style, &mut current, &mut blocks, &mut plain);
            current_style = TextStyle::Bullet;
        } else if tag.starts_with("p")
            || tag.starts_with("br")
            || tag.starts_with("div")
            || tag.starts_with("/p")
            || tag.starts_with("/li")
            || tag.starts_with("/h")
        {
            push(current_style, &mut current, &mut blocks, &mut plain);
            current_style = TextStyle::Body;
        }
        rest = &rest[open + close + 1..];
    }
    push(current_style, &mut current, &mut blocks, &mut plain);
    if blocks.is_empty() {
        blocks.push(TextBlock {
            style: TextStyle::Body,
            text: String::new(),
        });
    }
    (blocks, plain)
}

fn decode_entities(text: &str) -> String {
    text.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&mdash;", "—")
        .replace("&ndash;", "–")
}

/// Markdown → PDF.
pub fn markdown_to_pdf(
    input: &Path,
    output: &Path,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    let text = read_text(input)?;
    let blocks = markdown_to_blocks(&text);
    let title = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Document");
    let mut result = render_blocks_to_pdf(&blocks, output, title, sink, cancel)?;
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    Ok(result.finalize())
}

/// Plain text → PDF.
pub fn txt_to_pdf(
    input: &Path,
    output: &Path,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    let text = read_text(input)?;
    let blocks: Vec<TextBlock> = text
        .lines()
        .map(|line| TextBlock {
            style: TextStyle::Body,
            text: line.to_string(),
        })
        .collect();
    let title = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Document");
    let mut result = render_blocks_to_pdf(&blocks, output, title, sink, cancel)?;
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    Ok(result.finalize())
}

/// HTML → PDF (text-first rendering; scripts and styles are ignored).
pub fn html_to_pdf(
    input: &Path,
    output: &Path,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    let html = read_text(input)?;
    let (blocks, _) = strip_html_to_blocks(&html);
    let title = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Document");
    let mut result = render_blocks_to_pdf(&blocks, output, title, sink, cancel)?;
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    result.warn("HTML rendering is text-first: images, CSS layout and scripts are not reproduced");
    Ok(result.finalize())
}

/// DOCX → PDF through markdown extraction.
pub fn docx_to_pdf(
    input: &Path,
    output: &Path,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    sink.stage(Stage::Reading, 0.05);
    let markdown = tdx_docx::extract_markdown(input)?;
    let blocks = markdown_to_blocks(&markdown);
    let title = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Document");
    let mut result = render_blocks_to_pdf(&blocks, output, title, sink, cancel)?;
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    result
        .warn("DOCX to PDF preserves text structure; complex layout and images are not reproduced");
    Ok(result.finalize())
}

/// Markdown → HTML.
pub fn markdown_to_html_file(input: &Path, output: &Path) -> Result<OperationResult> {
    let markdown = read_text(input)?;
    let html = markdown_to_html(
        &markdown,
        input
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Document"),
    );
    fsutil::atomic_write(output, |file| {
        file.write_all(html.as_bytes()).map_err(TdxError::Io)
    })?;
    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Html);
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    Ok(result.finalize())
}

/// Convert Markdown source into a standalone HTML document.
pub fn markdown_to_html(markdown: &str, title: &str) -> String {
    let mut body = String::new();
    let mut in_list = false;
    let mut paragraph: Vec<String> = Vec::new();
    let flush_paragraph = |paragraph: &mut Vec<String>, body: &mut String, in_list: &mut bool| {
        if !paragraph.is_empty() {
            if *in_list {
                body.push_str("</ul>\n");
                *in_list = false;
            }
            let text = paragraph.join(" ");
            body.push_str(&format!("<p>{}</p>\n", escape_html(&text)));
            paragraph.clear();
        }
    };
    for line in markdown.lines() {
        let trimmed = line.trim_end();
        if trimmed.trim().is_empty() {
            flush_paragraph(&mut paragraph, &mut body, &mut in_list);
            continue;
        }
        let content = trimmed.trim_start();
        let hashes = content.chars().take_while(|c| *c == '#').count();
        if hashes > 0 && hashes <= 6 && content.as_bytes().get(hashes) == Some(&b' ') {
            flush_paragraph(&mut paragraph, &mut body, &mut in_list);
            let level = hashes;
            body.push_str(&format!(
                "<h{level}>{}</h{level}>\n",
                escape_html(content[hashes + 1..].trim())
            ));
            continue;
        }
        if let Some(rest) = content
            .strip_prefix("- ")
            .or_else(|| content.strip_prefix("* "))
        {
            flush_paragraph(&mut paragraph, &mut body, &mut in_list);
            if !in_list {
                body.push_str("<ul>\n");
                in_list = true;
            }
            body.push_str(&format!("<li>{}</li>\n", escape_html(rest)));
            continue;
        }
        paragraph.push(content.to_string());
    }
    flush_paragraph(&mut paragraph, &mut body, &mut in_list);

    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{title}</title>\n<style>\nbody{{font-family:system-ui,-apple-system,'Segoe UI',Roboto,sans-serif;max-width:46rem;margin:3rem auto;padding:0 1.25rem;line-height:1.6;color:#1c1e21}}\nh1,h2,h3{{line-height:1.25}}\ncode{{background:#f2f3f5;padding:.1em .35em;border-radius:.25em}}\n</style>\n</head>\n<body>\n{body}</body>\n</html>\n",
        title = escape_html(title),
        body = body,
    )
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn read_text(path: &Path) -> Result<String> {
    let text = std::fs::read_to_string(path).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => TdxError::not_found(path),
        std::io::ErrorKind::InvalidData => {
            TdxError::Unsupported("The file is not UTF-8 text".into())
        }
        _ => TdxError::Io(err),
    })?;
    Ok(text.trim_start_matches('\u{feff}').to_string())
}

/// Locate the optional LibreOffice adapter.
pub fn find_libreoffice() -> Option<PathBuf> {
    if let Ok(output) = std::process::Command::new("soffice")
        .arg("--version")
        .output()
    {
        if output.status.success() {
            return Some(PathBuf::from("soffice"));
        }
    }
    let candidates = [
        r"C:\Program Files\LibreOffice\program\soffice.exe",
        r"C:\Program Files (x86)\LibreOffice\program\soffice.exe",
        "/usr/bin/soffice",
        "/usr/local/bin/soffice",
        "/Applications/LibreOffice.app/Contents/MacOS/soffice",
    ];
    candidates
        .iter()
        .map(PathBuf::from)
        .find(|path| path.exists())
}

/// Convert a legacy .doc/.xls file through LibreOffice when it is installed.
pub fn legacy_import(input: &Path, output: &Path) -> Result<OperationResult> {
    let Some(soffice) = find_libreoffice() else {
        return Err(TdxError::AdapterRequired(
            "LibreOffice was not found. Install it or enable the optional import adapter in Settings".into(),
        ));
    };
    let target_format = match output
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("docx") => "docx",
        Some("xlsx") => "xlsx",
        _ => {
            return Err(TdxError::InvalidInput(
                "Legacy import target must be DOCX or XLSX".into(),
            ))
        }
    };
    let output_dir = output.parent().map(Path::to_path_buf).unwrap_or_default();
    let status = std::process::Command::new(soffice)
        .arg("--headless")
        .arg("--norestore")
        .arg("--convert-to")
        .arg(target_format)
        .arg("--outdir")
        .arg(&output_dir)
        .arg(input)
        .status()
        .map_err(TdxError::Io)?;
    if !status.success() {
        return Err(TdxError::Other("LibreOffice conversion failed".into()));
    }
    let produced = output_dir.join(format!(
        "{}.{}",
        input
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("document"),
        target_format
    ));
    if produced != output && produced.exists() {
        std::fs::rename(&produced, output).map_err(TdxError::Io)?;
    }
    if !output.exists() {
        return Err(TdxError::Other(
            "LibreOffice finished but the output file was not created".into(),
        ));
    }
    let mut result = OperationResult::single(
        output.to_path_buf(),
        if target_format == "docx" {
            FileKind::Docx
        } else {
            FileKind::Xlsx
        },
    );
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    result.warn("Converted through the external LibreOffice adapter (not part of the MIT core)");
    Ok(result.finalize())
}

/// Auto-dispatch a conversion based on detected input and requested output.
pub fn convert_file(
    input: &Path,
    output: &Path,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    let detected = detect(input)?;
    let target = output
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .ok_or_else(|| TdxError::InvalidInput("The output path needs a file extension".into()))?;

    match (detected.kind, target.as_str()) {
        (FileKind::Markdown, "pdf") => markdown_to_pdf(input, output, sink, cancel),
        (FileKind::Txt, "pdf") => txt_to_pdf(input, output, sink, cancel),
        (FileKind::Html, "pdf") => html_to_pdf(input, output, sink, cancel),
        (FileKind::Docx, "pdf") => docx_to_pdf(input, output, sink, cancel),
        (FileKind::Markdown, "html") | (FileKind::Txt, "html") => {
            markdown_to_html_file(input, output)
        }
        (FileKind::Markdown | FileKind::Txt, "docx") => tdx_docx::create_from_file(
            input,
            output,
            &tdx_docx::DocxOptions::default(),
            sink,
            cancel,
        ),
        (FileKind::Docx, "txt") => {
            let text = tdx_docx::extract_text(input)?;
            fsutil::atomic_write(output, |file| {
                file.write_all(text.as_bytes()).map_err(TdxError::Io)
            })?;
            let mut result = OperationResult::single(output.to_path_buf(), FileKind::Txt);
            result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
            Ok(result.finalize())
        }
        (FileKind::Docx, "md") => {
            let text = tdx_docx::extract_markdown(input)?;
            fsutil::atomic_write(output, |file| {
                file.write_all(text.as_bytes()).map_err(TdxError::Io)
            })?;
            let mut result = OperationResult::single(output.to_path_buf(), FileKind::Markdown);
            result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
            Ok(result.finalize())
        }
        (FileKind::Docx, "html") => {
            let markdown = tdx_docx::extract_markdown(input)?;
            let html = markdown_to_html(
                &markdown,
                input
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Document"),
            );
            fsutil::atomic_write(output, |file| {
                file.write_all(html.as_bytes()).map_err(TdxError::Io)
            })?;
            let mut result = OperationResult::single(output.to_path_buf(), FileKind::Html);
            result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
            Ok(result.finalize())
        }
        (FileKind::Html, "txt") => {
            let html = read_text(input)?;
            let (_, plain) = strip_html_to_blocks(&html);
            fsutil::atomic_write(output, |file| {
                file.write_all(plain.as_bytes()).map_err(TdxError::Io)
            })?;
            let mut result = OperationResult::single(output.to_path_buf(), FileKind::Txt);
            result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
            Ok(result.finalize())
        }
        (FileKind::Doc, "docx") | (FileKind::Xls, "xlsx") => legacy_import(input, output),
        (kind, other) => Err(TdxError::Unsupported(format!(
            "{} → {other} is not supported by the native core",
            kind.display_name()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_blocks_are_parsed() {
        let blocks = markdown_to_blocks("# Title\n\nSome **bold** text\n\n- item one\n");
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].style, TextStyle::Title);
        assert_eq!(blocks[1].text, "Some bold text");
        assert_eq!(blocks[2].style, TextStyle::Bullet);
    }

    #[test]
    fn html_is_stripped() {
        let (blocks, plain) = strip_html_to_blocks("<h1>Hello</h1><p>World &amp; more</p>");
        assert_eq!(blocks[0].style, TextStyle::Title);
        assert!(plain.contains("World & more"));
    }

    #[test]
    fn markdown_to_html_escapes() {
        let html = markdown_to_html("# A < B", "T");
        assert!(html.contains("A &lt; B"));
    }
}

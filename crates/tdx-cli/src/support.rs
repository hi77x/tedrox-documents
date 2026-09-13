//! Shared CLI helpers: output formatting, progress, cancellation, tools list.

use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use serde_json::json;
use tdx_core::{
    error::Result,
    fsutil,
    progress::{CancelToken, ProgressEvent, ProgressSink},
    result::OperationResult,
    OperationRegistry, TdxError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Human,
    Json,
}

pub struct Ctx {
    pub format: OutputFormat,
    pub quiet: bool,
    pub cancel: CancelToken,
}

impl Ctx {
    pub fn new(format: OutputFormat, quiet: bool) -> std::result::Result<Self, TdxError> {
        let cancel = CancelToken::new();
        let handler_token = cancel.clone();
        let _ = ctrlc::set_handler(move || {
            handler_token.cancel();
            eprintln!("\nCancellation requested…");
        });
        Ok(Self {
            format,
            quiet,
            cancel,
        })
    }

    fn json_mode(&self) -> bool {
        self.format == OutputFormat::Json
    }

    fn progress_enabled(&self) -> bool {
        !self.quiet && !self.json_mode() && std::io::stderr().is_terminal()
    }

    pub fn execute(
        &self,
        _operation: &str,
        _inputs: &[PathBuf],
        run: impl FnOnce(&ProgressSink, &CancelToken) -> Result<OperationResult>,
    ) -> Result<OperationResult> {
        let sink = if self.progress_enabled() {
            ProgressSink::new(|event| print_progress(&event))
        } else {
            ProgressSink::default()
        };
        let started = Instant::now();
        let mut result = run(&sink, &self.cancel)?;
        if result.duration_ms == 0 {
            result.duration_ms = started.elapsed().as_millis() as u64;
        }
        if self.progress_enabled() {
            eprint!("\r\x1b[2K");
        }
        Ok(result)
    }

    pub fn report_error(&self, err: &TdxError) {
        if self.json_mode() {
            let payload = json!({
                "ok": false,
                "error": {
                    "category": err.category().as_str(),
                    "message": err.to_string(),
                }
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".into())
            );
        } else {
            eprintln!("Error: {err}");
            if let Some(hint) = hint_for(err) {
                eprintln!("Hint: {hint}");
            }
        }
    }
}

fn hint_for(err: &TdxError) -> Option<&'static str> {
    match err {
        TdxError::AdapterRequired(_) => {
            Some("Optional adapters are documented in docs/formats.md. Install LibreOffice for legacy Office files.")
        }
        TdxError::Encrypted => Some("Remove the password with the original tool, then retry."),
        TdxError::InsufficientSpace { .. } => Some("Free disk space or choose another output location."),
        _ => None,
    }
}

fn print_progress(event: &ProgressEvent) {
    let percent = (event.progress * 100.0).clamp(0.0, 100.0);
    let message = event.message.as_deref().unwrap_or("");
    eprint!(
        "\r\x1b[2K[{:<10}] {:>3.0}%  {}",
        event.stage.as_str(),
        percent,
        message
    );
}

/// Print an operation result in the selected format.
pub fn print_result(ctx: &Ctx, operation: &str, result: &OperationResult) {
    if ctx.format == OutputFormat::Json {
        let payload = json!({
            "ok": true,
            "operation": operation,
            "outputs": result.outputs.iter().map(|output| json!({
                "path": output.path.display().to_string(),
                "bytes": output.bytes,
                "kind": output.kind,
                "label": output.label,
            })).collect::<Vec<_>>(),
            "bytesIn": result.bytes_in,
            "bytesOut": result.bytes_out,
            "durationMs": result.duration_ms,
            "warnings": result.warnings,
            "stats": result.stats,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".into())
        );
        return;
    }

    if !ctx.quiet {
        println!(
            "Done in {} ({} in, {} out)",
            fsutil::format_duration_ms(result.duration_ms),
            fsutil::format_bytes(result.bytes_in),
            fsutil::format_bytes(result.bytes_out)
        );
        for output in &result.outputs {
            let label = output
                .label
                .as_deref()
                .map(|l| format!(" [{l}]"))
                .unwrap_or_default();
            println!(
                "  → {}{}  {}",
                output.path.display(),
                label,
                fsutil::format_bytes(output.bytes)
            );
        }
        for (key, value) in &result.stats {
            let rendered = match value {
                serde_json::Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            println!("  {key}: {rendered}");
        }
        for warning in &result.warnings {
            println!("Warning: {warning}");
        }
    }
}

/// `tdx-doc tools`
pub fn run_tools(ctx: &Ctx, category: Option<&str>) -> Result<ExitCode> {
    let registry = OperationRegistry::builtin();
    let operations: Vec<_> = match category {
        Some(category) => registry.by_category(category),
        None => registry.all().iter().collect(),
    };
    if operations.is_empty() {
        return Err(TdxError::InvalidInput(format!(
            "Unknown category: {}",
            category.unwrap_or_default()
        )));
    }
    if ctx.format == OutputFormat::Json {
        let payload = serde_json::to_string_pretty(&operations)
            .map_err(|err| TdxError::Other(err.to_string()))?;
        println!("{payload}");
        return Ok(ExitCode::SUCCESS);
    }
    let mut current = "";
    for operation in operations {
        if operation.category != current {
            if !current.is_empty() {
                println!();
            }
            current = operation.category;
            println!("{}", current.to_uppercase());
        }
        let experimental = if operation.experimental {
            " (experimental)"
        } else {
            ""
        };
        println!("  {:<22} {}{}", operation.id, operation.name, experimental);
        println!("  {:<22} {}", "", operation.summary);
    }
    Ok(ExitCode::SUCCESS)
}

pub fn parse_delimiter(value: &str) -> Result<char> {
    match value {
        "\\t" | "tab" | "tab-separated" => Ok('\t'),
        ";" => Ok(';'),
        "|" => Ok('|'),
        "," => Ok(','),
        other => other
            .chars()
            .next()
            .filter(|_| other.chars().count() == 1)
            .ok_or_else(|| TdxError::InvalidInput(format!("Invalid delimiter: {other}"))),
    }
}

pub fn parse_background(value: &str) -> Result<[u8; 3]> {
    let parts: Vec<&str> = value.split(',').collect();
    if parts.len() != 3 {
        return Err(TdxError::InvalidInput("Background must be R,G,B".into()));
    }
    let mut rgb = [255u8; 3];
    for (index, part) in parts.iter().enumerate() {
        rgb[index] = part.trim().parse::<u8>().map_err(|_| {
            TdxError::InvalidInput("Background must be R,G,B with values 0-255".into())
        })?;
    }
    Ok(rgb)
}

pub fn parse_page_list(value: &str) -> Result<Vec<u32>> {
    let mut pages = Vec::new();
    for token in value.split(',') {
        pages.push(
            token
                .trim()
                .parse::<u32>()
                .map_err(|_| TdxError::InvalidInput(format!("Invalid page order: {value}")))?,
        );
    }
    Ok(pages)
}

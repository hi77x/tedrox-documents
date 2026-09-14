//! Tauri 2 desktop shell for TEDROX Documents.
//!
//! The shell is intentionally thin: every command delegates to the shared
//! `tdx-*` engine crates. No document logic lives here.

use std::path::PathBuf;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{Emitter, Manager};
use tdx_core::{
    error::TdxError,
    progress::{CancelToken, ProgressSink, Stage},
    result::OperationResult,
    OperationRegistry,
};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProgressMsg {
    stage: String,
    progress: f32,
    message: Option<String>,
    done: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct OpOutput {
    path: String,
    bytes: u64,
    kind: String,
    label: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct OpResult {
    outputs: Vec<OpOutput>,
    bytes_in: u64,
    bytes_out: u64,
    duration_ms: u64,
    warnings: Vec<String>,
    stats: serde_json::Value,
}

impl From<OperationResult> for OpResult {
    fn from(result: OperationResult) -> Self {
        let stats = serde_json::to_value(&result.stats).unwrap_or(serde_json::Value::Null);
        Self {
            outputs: result
                .outputs
                .into_iter()
                .map(|output| OpOutput {
                    path: output.path.display().to_string(),
                    bytes: output.bytes,
                    kind: format!("{:?}", output.kind).to_lowercase(),
                    label: output.label,
                })
                .collect(),
            bytes_in: result.bytes_in,
            bytes_out: result.bytes_out,
            duration_ms: result.duration_ms,
            warnings: result.warnings,
            stats,
        }
    }
}

fn sink(channel: &Channel<ProgressMsg>) -> ProgressSink {
    let channel = channel.clone();
    ProgressSink::new(move |event| {
        let _ = channel.send(ProgressMsg {
            stage: event.stage.as_str().to_string(),
            progress: event.progress,
            message: event.message.clone(),
            done: false,
        });
    })
}

fn done(channel: &Channel<ProgressMsg>) {
    let _ = channel.send(ProgressMsg {
        stage: Stage::Completed.as_str().to_string(),
        progress: 1.0,
        message: None,
        done: true,
    });
}

fn fail(channel: &Channel<ProgressMsg>, err: &TdxError) {
    let _ = channel.send(ProgressMsg {
        stage: "failed".to_string(),
        progress: 1.0,
        message: Some(err.to_string()),
        done: true,
    });
}

fn token() -> CancelToken {
    CancelToken::new()
}

fn prepare_output(path: String) -> PathBuf {
    PathBuf::from(path)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FileInfo {
    path: String,
    name: String,
    kind: String,
    mime: String,
    category: String,
    size: u64,
    confidence: String,
}

#[tauri::command]
async fn inspect_files(paths: Vec<String>) -> Result<Vec<FileInfo>, String> {
    let mut items = Vec::new();
    for path in paths {
        let detected =
            tdx_core::detect::detect(std::path::Path::new(&path)).map_err(|e| e.to_string())?;
        let category = detected.category().as_str().to_string();
        items.push(FileInfo {
            path: path.clone(),
            name: detected.name.unwrap_or_else(|| path.clone()),
            kind: format!("{:?}", detected.kind).to_lowercase(),
            mime: detected.mime,
            category,
            size: detected.size,
            confidence: format!("{:?}", detected.confidence).to_lowercase(),
        });
    }
    Ok(items)
}

#[tauri::command]
async fn list_tools() -> Result<serde_json::Value, String> {
    let registry = OperationRegistry::builtin();
    serde_json::to_value(registry.all()).map_err(|e| e.to_string())
}

#[tauri::command]
async fn suggest_output(
    input: String,
    suffix: String,
    extension: String,
) -> Result<String, String> {
    let path = tdx_core::fsutil::default_output(std::path::Path::new(&input), &suffix, &extension);
    Ok(path.display().to_string())
}

#[tauri::command]
async fn pdf_merge(
    files: Vec<String>,
    output: String,
    on_progress: Channel<ProgressMsg>,
) -> Result<OpResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let inputs: Vec<PathBuf> = files.iter().map(PathBuf::from).collect();
        let outcome = tdx_pdf::pages::merge(
            &inputs,
            &prepare_output(output),
            &sink(&on_progress),
            &token(),
        );
        match outcome {
            Ok(result) => {
                done(&on_progress);
                Ok(OpResult::from(result.finalize()))
            }
            Err(err) => {
                fail(&on_progress, &err);
                Err(err.to_string())
            }
        }
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
async fn pdf_split(
    file: String,
    output_dir: String,
    chunk: u32,
    on_progress: Channel<ProgressMsg>,
) -> Result<OpResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mode = if chunk > 1 {
            tdx_pdf::SplitMode::Chunks(chunk)
        } else {
            tdx_pdf::SplitMode::EveryPage
        };
        match tdx_pdf::pages::split(
            std::path::Path::new(&file),
            std::path::Path::new(&output_dir),
            &mode,
            &sink(&on_progress),
            &token(),
        ) {
            Ok(result) => {
                done(&on_progress);
                Ok(OpResult::from(result.finalize()))
            }
            Err(err) => {
                fail(&on_progress, &err);
                Err(err.to_string())
            }
        }
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
async fn pdf_extract(
    file: String,
    pages: String,
    output: String,
    on_progress: Channel<ProgressMsg>,
) -> Result<OpResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let selection = match tdx_pdf::PageSelection::parse(&pages) {
            Ok(selection) => selection,
            Err(err) => {
                fail(&on_progress, &err);
                return Err(err.to_string());
            }
        };
        match tdx_pdf::pages::extract_pages(
            std::path::Path::new(&file),
            &prepare_output(output),
            &selection,
            &sink(&on_progress),
            &token(),
        ) {
            Ok(result) => {
                done(&on_progress);
                Ok(OpResult::from(result.finalize()))
            }
            Err(err) => {
                fail(&on_progress, &err);
                Err(err.to_string())
            }
        }
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
async fn pdf_clean(
    file: String,
    output: String,
    on_progress: Channel<ProgressMsg>,
) -> Result<OpResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        match tdx_pdf::metadata::privacy_clean(
            std::path::Path::new(&file),
            &prepare_output(output),
            &sink(&on_progress),
            &token(),
        ) {
            Ok(result) => {
                done(&on_progress);
                Ok(OpResult::from(result.finalize()))
            }
            Err(err) => {
                fail(&on_progress, &err);
                Err(err.to_string())
            }
        }
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
async fn pdf_compress(
    file: String,
    output: String,
    preset: String,
    quality: u8,
    max_dimension: u32,
    on_progress: Channel<ProgressMsg>,
) -> Result<OpResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let preset = match preset.as_str() {
            "screen" => tdx_pdf::CompressPreset::Screen,
            "print" => tdx_pdf::CompressPreset::Print,
            "custom" => tdx_pdf::CompressPreset::Custom {
                quality,
                max_dimension,
            },
            _ => tdx_pdf::CompressPreset::Balanced,
        };
        match tdx_pdf::compress::compress(
            std::path::Path::new(&file),
            &prepare_output(output),
            preset,
            &sink(&on_progress),
            &token(),
        ) {
            Ok(result) => {
                done(&on_progress);
                Ok(OpResult::from(result.finalize()))
            }
            Err(err) => {
                fail(&on_progress, &err);
                Err(err.to_string())
            }
        }
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
async fn pdf_metadata(file: String) -> Result<serde_json::Value, String> {
    let metadata =
        tdx_pdf::metadata::read_metadata(std::path::Path::new(&file)).map_err(|e| e.to_string())?;
    serde_json::to_value(metadata).map_err(|err| err.to_string())
}

#[tauri::command]
async fn pdf_info(file: String) -> Result<serde_json::Value, String> {
    let info = tdx_pdf::inspect(std::path::Path::new(&file)).map_err(|e| e.to_string())?;
    serde_json::to_value(info).map_err(|err| err.to_string())
}

#[tauri::command]
async fn pdf_apply_plan(
    file: String,
    output: String,
    plan: Vec<tdx_pdf::organize::PagePlanEntry>,
    on_progress: Channel<ProgressMsg>,
) -> Result<OpResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        match tdx_pdf::organize::apply_plan(
            std::path::Path::new(&file),
            &prepare_output(output),
            &plan,
            &sink(&on_progress),
            &token(),
        ) {
            Ok(result) => {
                done(&on_progress);
                Ok(OpResult::from(result))
            }
            Err(err) => {
                fail(&on_progress, &err);
                Err(err.to_string())
            }
        }
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
async fn pdf_add_annotations(
    file: String,
    output: String,
    annotations: Vec<tdx_pdf::annotate::AnnotationInput>,
    on_progress: Channel<ProgressMsg>,
) -> Result<OpResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        match tdx_pdf::annotate::add_annotations(
            std::path::Path::new(&file),
            &prepare_output(output),
            &annotations,
            &sink(&on_progress),
            &token(),
        ) {
            Ok(result) => {
                done(&on_progress);
                Ok(OpResult::from(result))
            }
            Err(err) => {
                fail(&on_progress, &err);
                Err(err.to_string())
            }
        }
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
async fn pdf_list_annotations(file: String) -> Result<serde_json::Value, String> {
    let annotations = tdx_pdf::annotate::list_annotations(std::path::Path::new(&file))
        .map_err(|e| e.to_string())?;
    serde_json::to_value(annotations).map_err(|err| err.to_string())
}

#[tauri::command]
async fn pdf_form_fields(file: String) -> Result<serde_json::Value, String> {
    let fields =
        tdx_pdf::forms::list_fields(std::path::Path::new(&file)).map_err(|e| e.to_string())?;
    serde_json::to_value(fields).map_err(|err| err.to_string())
}

#[tauri::command]
async fn pdf_fill_form(
    file: String,
    output: String,
    values: std::collections::BTreeMap<String, String>,
    on_progress: Channel<ProgressMsg>,
) -> Result<OpResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        match tdx_pdf::forms::fill_fields(
            std::path::Path::new(&file),
            &prepare_output(output),
            &values,
            &sink(&on_progress),
            &token(),
        ) {
            Ok(result) => {
                done(&on_progress);
                Ok(OpResult::from(result))
            }
            Err(err) => {
                fail(&on_progress, &err);
                Err(err.to_string())
            }
        }
    })
    .await
    .map_err(|err| err.to_string())?
}

/// Read a file as raw bytes for in-process viewers.
#[tauri::command]
async fn read_binary(path: String) -> Result<tauri::ipc::Response, String> {
    let bytes = std::fs::read(&path).map_err(|err| format!("{}: {err}", path))?;
    Ok(tauri::ipc::Response::new(bytes))
}

/// Write raw bytes (rendered pages, exports) without going through JSON.
#[tauri::command]
async fn write_binary(path: String, bytes: Vec<u8>) -> Result<u64, String> {
    tdx_core::fsutil::atomic_write(std::path::Path::new(&path), |file| {
        use std::io::Write;
        file.write_all(&bytes)
            .map_err(|err| tdx_core::error::TdxError::Io(err))
    })
    .map_err(|err| err.to_string())?;
    Ok(std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0))
}

/// Report the size of a file without loading it.
#[tauri::command]
async fn file_size(path: String) -> Result<u64, String> {
    std::fs::metadata(&path)
        .map(|meta| meta.len())
        .map_err(|err| err.to_string())
}

/// Remove a source file. Only ever called after an explicit opt-in in the UI
/// and after the replacement output has been written and verified.
#[tauri::command]
async fn delete_file(path: String) -> Result<(), String> {
    let target = std::path::Path::new(&path);
    if !target.is_file() {
        return Err("Not a regular file".into());
    }
    std::fs::remove_file(target).map_err(|err| err.to_string())
}

#[tauri::command]
async fn pdf_watermark(
    file: String,
    output: String,
    text: String,
    opacity: f32,
    on_progress: Channel<ProgressMsg>,
) -> Result<OpResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let options = tdx_pdf::watermark::WatermarkOptions {
            text,
            opacity,
            ..tdx_pdf::watermark::WatermarkOptions::default()
        };
        match tdx_pdf::watermark::watermark(
            std::path::Path::new(&file),
            &prepare_output(output),
            &options,
            &sink(&on_progress),
            &token(),
        ) {
            Ok(result) => {
                done(&on_progress);
                Ok(OpResult::from(result.finalize()))
            }
            Err(err) => {
                fail(&on_progress, &err);
                Err(err.to_string())
            }
        }
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
async fn image_convert(
    file: String,
    to: String,
    output: String,
    quality: u8,
    on_progress: Channel<ProgressMsg>,
) -> Result<OpResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let format = match tdx_image::OutputFormat::from_extension(&to) {
            Some(format) => format,
            None => {
                let err = TdxError::InvalidInput(format!("Unsupported image format: {to}"));
                fail(&on_progress, &err);
                return Err(err.to_string());
            }
        };
        match tdx_image::convert(
            std::path::Path::new(&file),
            &prepare_output(output),
            format,
            quality,
            None,
            &sink(&on_progress),
            &token(),
        ) {
            Ok(result) => {
                done(&on_progress);
                Ok(OpResult::from(result.finalize()))
            }
            Err(err) => {
                fail(&on_progress, &err);
                Err(err.to_string())
            }
        }
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
async fn image_resize(
    file: String,
    output: String,
    width: Option<u32>,
    height: Option<u32>,
    mode: String,
    on_progress: Channel<ProgressMsg>,
) -> Result<OpResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mode = match mode.as_str() {
            "cover" => tdx_image::ResizeMode::Cover,
            "exact" => tdx_image::ResizeMode::Exact,
            _ => tdx_image::ResizeMode::Contain,
        };
        let format = std::path::Path::new(&output)
            .extension()
            .and_then(|ext| ext.to_str())
            .and_then(tdx_image::OutputFormat::from_extension)
            .unwrap_or(tdx_image::OutputFormat::Png);
        match tdx_image::resize(
            std::path::Path::new(&file),
            &prepare_output(output),
            width,
            height,
            mode,
            format,
            92,
            &sink(&on_progress),
            &token(),
        ) {
            Ok(result) => {
                done(&on_progress);
                Ok(OpResult::from(result.finalize()))
            }
            Err(err) => {
                fail(&on_progress, &err);
                Err(err.to_string())
            }
        }
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
async fn csv_to_xlsx(
    file: String,
    output: String,
    on_progress: Channel<ProgressMsg>,
) -> Result<OpResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let options = tdx_sheet::csv_ops::CsvToXlsxOptions::default();
        match tdx_sheet::csv_ops::csv_to_xlsx(
            std::path::Path::new(&file),
            &prepare_output(output),
            &options,
            &sink(&on_progress),
            &token(),
        ) {
            Ok(result) => {
                done(&on_progress);
                Ok(OpResult::from(result.finalize()))
            }
            Err(err) => {
                fail(&on_progress, &err);
                Err(err.to_string())
            }
        }
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
async fn xlsx_to_csv(
    file: String,
    output: String,
    on_progress: Channel<ProgressMsg>,
) -> Result<OpResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let options = tdx_sheet::csv_ops::XlsxToCsvOptions::default();
        match tdx_sheet::csv_ops::xlsx_to_csv(
            std::path::Path::new(&file),
            &prepare_output(output),
            &options,
            &sink(&on_progress),
            &token(),
        ) {
            Ok(result) => {
                done(&on_progress);
                Ok(OpResult::from(result.finalize()))
            }
            Err(err) => {
                fail(&on_progress, &err);
                Err(err.to_string())
            }
        }
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
async fn doc_extract(file: String, output: String, markdown: bool) -> Result<String, String> {
    let path = std::path::Path::new(&file);
    let text = if markdown {
        tdx_docx::extract_markdown(path)
    } else {
        tdx_docx::extract_text(path)
    }
    .map_err(|err| err.to_string())?;
    let target = std::path::Path::new(&output);
    tdx_core::fsutil::atomic_write(target, |handle| {
        use std::io::Write;
        handle.write_all(text.as_bytes()).map_err(TdxError::Io)
    })
    .map_err(|err| err.to_string())?;
    Ok(output)
}

#[tauri::command]
async fn doc_from_markdown(
    file: String,
    output: String,
    title: Option<String>,
    on_progress: Channel<ProgressMsg>,
) -> Result<OpResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let options = tdx_docx::DocxOptions {
            title,
            author: None,
        };
        match tdx_docx::create_from_file(
            std::path::Path::new(&file),
            &prepare_output(output),
            &options,
            &sink(&on_progress),
            &token(),
        ) {
            Ok(result) => {
                done(&on_progress);
                Ok(OpResult::from(result.finalize()))
            }
            Err(err) => {
                fail(&on_progress, &err);
                Err(err.to_string())
            }
        }
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
async fn markdown_to_html(file: String, output: String) -> Result<String, String> {
    let result = tdx_convert::markdown_to_html_file(
        std::path::Path::new(&file),
        std::path::Path::new(&output),
    )
    .map_err(|err| err.to_string())?;
    Ok(result.finalize().outputs[0].path.display().to_string())
}

#[tauri::command]
async fn image_crop(
    file: String,
    output: String,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    on_progress: Channel<ProgressMsg>,
) -> Result<OpResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let format = std::path::Path::new(&output)
            .extension()
            .and_then(|ext| ext.to_str())
            .and_then(tdx_image::OutputFormat::from_extension)
            .unwrap_or(tdx_image::OutputFormat::Png);
        match tdx_image::crop(
            std::path::Path::new(&file),
            &prepare_output(output),
            x,
            y,
            width,
            height,
            format,
            92,
            &sink(&on_progress),
            &token(),
        ) {
            Ok(result) => {
                done(&on_progress);
                Ok(OpResult::from(result.finalize()))
            }
            Err(err) => {
                fail(&on_progress, &err);
                Err(err.to_string())
            }
        }
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
async fn image_rotate(
    file: String,
    output: String,
    degrees: i32,
    flip_horizontal: bool,
    flip_vertical: bool,
    on_progress: Channel<ProgressMsg>,
) -> Result<OpResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let format = std::path::Path::new(&output)
            .extension()
            .and_then(|ext| ext.to_str())
            .and_then(tdx_image::OutputFormat::from_extension);
        match tdx_image::rotate_flip(
            std::path::Path::new(&file),
            &prepare_output(output),
            degrees,
            flip_horizontal,
            flip_vertical,
            format,
            &sink(&on_progress),
            &token(),
        ) {
            Ok(result) => {
                done(&on_progress);
                Ok(OpResult::from(result.finalize()))
            }
            Err(err) => {
                fail(&on_progress, &err);
                Err(err.to_string())
            }
        }
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
async fn image_strip_metadata(
    file: String,
    output: String,
    on_progress: Channel<ProgressMsg>,
) -> Result<OpResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        match tdx_image::strip_metadata(
            std::path::Path::new(&file),
            &prepare_output(output),
            &sink(&on_progress),
            &token(),
        ) {
            Ok(result) => {
                done(&on_progress);
                Ok(OpResult::from(result.finalize()))
            }
            Err(err) => {
                fail(&on_progress, &err);
                Err(err.to_string())
            }
        }
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
async fn convert_auto(
    file: String,
    output: String,
    on_progress: Channel<ProgressMsg>,
) -> Result<OpResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        match tdx_convert::convert_file(
            std::path::Path::new(&file),
            &prepare_output(output),
            &sink(&on_progress),
            &token(),
        ) {
            Ok(result) => {
                done(&on_progress);
                Ok(OpResult::from(result.finalize()))
            }
            Err(err) => {
                fail(&on_progress, &err);
                Err(err.to_string())
            }
        }
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
async fn docx_open(path: String) -> Result<serde_json::Value, String> {
    let model =
        tdx_docx::model::load_model(std::path::Path::new(&path)).map_err(|err| err.to_string())?;
    serde_json::to_value(model).map_err(|err| err.to_string())
}

#[tauri::command]
async fn docx_save(path: String, model: serde_json::Value) -> Result<serde_json::Value, String> {
    let model: tdx_docx::model::DocModel =
        serde_json::from_value(model).map_err(|err| err.to_string())?;
    tdx_docx::model::save_model(&model, std::path::Path::new(&path))
        .map_err(|err| err.to_string())?;
    Ok(serde_json::json!({ "path": path }))
}

#[tauri::command]
async fn sheet_open(path: String) -> Result<serde_json::Value, String> {
    let model = tdx_sheet::model::load_workbook(std::path::Path::new(&path))
        .map_err(|err| err.to_string())?;
    serde_json::to_value(model).map_err(|err| err.to_string())
}

#[tauri::command]
async fn sheet_save(path: String, model: serde_json::Value) -> Result<serde_json::Value, String> {
    let model: tdx_sheet::model::WorkbookModel =
        serde_json::from_value(model).map_err(|err| err.to_string())?;
    tdx_sheet::model::save_workbook(&model, std::path::Path::new(&path))
        .map_err(|err| err.to_string())?;
    Ok(serde_json::json!({ "path": path }))
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            if let Some(path) = std::env::args().nth(1) {
                if std::path::Path::new(&path).is_file() {
                    let window = app.get_webview_window("main").expect("main window exists");
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(1200));
                        let _ = window.emit("open-file", path);
                    });
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            inspect_files,
            list_tools,
            suggest_output,
            docx_open,
            docx_save,
            sheet_open,
            sheet_save,
            pdf_merge,
            pdf_split,
            pdf_extract,
            pdf_clean,
            pdf_compress,
            pdf_metadata,
            pdf_info,
            pdf_watermark,
            pdf_apply_plan,
            pdf_add_annotations,
            pdf_list_annotations,
            pdf_form_fields,
            pdf_fill_form,
            read_binary,
            write_binary,
            file_size,
            delete_file,
            image_convert,
            image_resize,
            csv_to_xlsx,
            xlsx_to_csv,
            convert_auto,
            doc_extract,
            doc_from_markdown,
            markdown_to_html,
            image_crop,
            image_rotate,
            image_strip_metadata
        ])
        .run(tauri::generate_context!())
        .expect("error while running TEDROX Documents");
}

//! Archive operations for TEDROX Documents: safe ZIP creation and extraction.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tdx_core::{
    detect::FileKind,
    error::{Result, TdxError},
    fsutil,
    progress::{CancelToken, ProgressSink, Stage},
    result::OperationResult,
};
use zip::write::SimpleFileOptions;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZipEntry {
    pub name: String,
    pub size: u64,
    pub compressed_size: u64,
    pub is_directory: bool,
}

fn open_zip(path: &Path) -> Result<zip::ZipArchive<std::fs::File>> {
    let file = std::fs::File::open(path).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => TdxError::not_found(path),
        std::io::ErrorKind::PermissionDenied => TdxError::Permission(path.display().to_string()),
        _ => TdxError::Io(err),
    })?;
    zip::ZipArchive::new(file)
        .map_err(|err| TdxError::Corrupt(format!("Not a valid ZIP archive: {err}")))
}

/// List archive entries.
pub fn list(input: &Path) -> Result<Vec<ZipEntry>> {
    let mut archive = open_zip(input)?;
    let mut entries = Vec::with_capacity(archive.len());
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|err| TdxError::Corrupt(format!("Cannot read ZIP entry: {err}")))?;
        entries.push(ZipEntry {
            name: entry.name().to_string(),
            size: entry.size(),
            compressed_size: entry.compressed_size(),
            is_directory: entry.is_dir(),
        });
    }
    Ok(entries)
}

fn collect_files(root: &Path, base: &Path, files: &mut Vec<(PathBuf, String)>) -> Result<()> {
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".DS_Store" || name == "Thumbs.db" {
            continue;
        }
        if path.is_dir() {
            collect_files(&path, base, files)?;
        } else if path.is_file() {
            let relative = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            files.push((path.clone(), relative));
        }
    }
    Ok(())
}

/// Create a ZIP archive from files and/or directories.
pub fn create(
    inputs: &[PathBuf],
    output: &Path,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    if inputs.is_empty() {
        return Err(TdxError::InvalidInput(
            "Select at least one file to archive".into(),
        ));
    }
    sink.stage(Stage::Validating, 0.0);
    let mut files: Vec<(PathBuf, String)> = Vec::new();
    for input in inputs {
        if input.is_dir() {
            let base = input.parent().map(Path::to_path_buf).unwrap_or_default();
            collect_files(input, &base, &mut files)?;
        } else if input.is_file() {
            let name = input
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "file".into());
            files.push((input.clone(), name));
        } else {
            return Err(TdxError::not_found(input));
        }
    }
    if files.is_empty() {
        return Err(TdxError::InvalidInput("Nothing to archive".into()));
    }

    let total = files.len();
    let mut buffer = std::io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut buffer);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (index, (path, name)) in files.iter().enumerate() {
            cancel.check()?;
            sink.message(
                Stage::Writing,
                index as f32 / total as f32,
                format!("Adding {name}"),
            );
            writer
                .start_file(name.clone(), options)
                .map_err(|err| TdxError::Other(format!("ZIP error: {err}")))?;
            let mut file = std::fs::File::open(path)?;
            std::io::copy(&mut file, &mut writer)?;
        }
        writer
            .finish()
            .map_err(|err| TdxError::Other(format!("ZIP error: {err}")))?;
    }
    let bytes = buffer.into_inner();
    sink.stage(Stage::Writing, 0.9);
    fsutil::atomic_write(output, |file| file.write_all(&bytes).map_err(TdxError::Io))?;
    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Zip);
    result.bytes_in = inputs
        .iter()
        .map(|path| {
            std::fs::metadata(path)
                .map(|meta| if meta.is_file() { meta.len() } else { 0 })
                .unwrap_or(0)
        })
        .sum();
    result.set_stat("files", total);
    Ok(result.finalize())
}

const MAX_ENTRIES: usize = 200_000;
const MAX_TOTAL_UNCOMPRESSED: u64 = 8 * 1024 * 1024 * 1024;
const MAX_RATIO: u64 = 2000;

/// Extract a ZIP archive with zip-slip and decompression-bomb protection.
pub fn extract(
    input: &Path,
    output_dir: &Path,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    sink.stage(Stage::Validating, 0.0);
    std::fs::create_dir_all(output_dir)?;
    let mut archive = open_zip(input)?;
    if archive.len() > MAX_ENTRIES {
        return Err(TdxError::InvalidInput(format!(
            "Archive contains more than {MAX_ENTRIES} entries"
        )));
    }
    let mut total_uncompressed: u64 = 0;
    let mut extracted: u64 = 0;
    let mut output_paths: Vec<PathBuf> = Vec::new();
    let total_entries = archive.len();

    for index in 0..total_entries {
        cancel.check()?;
        let mut entry = archive
            .by_index(index)
            .map_err(|err| TdxError::Corrupt(format!("Cannot read ZIP entry: {err}")))?;
        let name = entry.name().to_string();
        let relative = PathBuf::from(name.replace('\\', "/"));
        if relative.components().next().is_none() {
            continue;
        }
        let target = fsutil::safe_join(output_dir, &relative)?;

        if entry.is_dir() {
            std::fs::create_dir_all(&target)?;
            continue;
        }
        let size = entry.size();
        let compressed = entry.compressed_size().max(1);
        if size > MAX_TOTAL_UNCOMPRESSED {
            return Err(TdxError::InvalidInput(format!(
                "Archive entry is too large to extract safely: {name}"
            )));
        }
        if size / compressed > MAX_RATIO && size > 64 * 1024 * 1024 {
            return Err(TdxError::InvalidInput(format!(
                "Suspicious compression ratio detected in entry: {name}"
            )));
        }
        total_uncompressed = total_uncompressed.saturating_add(size);
        if total_uncompressed > MAX_TOTAL_UNCOMPRESSED {
            return Err(TdxError::InvalidInput(
                "Archive expands beyond the 8 GB safety limit".into(),
            ));
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::File::create(&target)?;
        std::io::copy(&mut entry, &mut file)?;
        extracted += 1;
        output_paths.push(target);
        sink.counts(Stage::Writing, index as u64 + 1, total_entries as u64);
    }

    let mut result = OperationResult::empty();
    result.outputs =
        vec![
            tdx_core::result::OutputArtifact::new(output_dir.to_path_buf(), FileKind::Unknown)
                .with_label("extracted directory"),
        ];
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    result.set_stat("entries", extracted);
    result.set_stat("output_dir", output_dir.display().to_string());
    Ok(result.finalize())
}

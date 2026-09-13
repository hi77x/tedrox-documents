//! Archive, hashing, duplicate detection and batch rename.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::json;
use sha2::{Digest, Sha256};
use tdx_core::error::{Result, TdxError};

use crate::support::{Ctx, OutputFormat};
use crate::{run_and_report, ArchiveArgs, ArchiveCommand, FileArgs, FileCommand, InspectArgs};

pub fn run_archive(ctx: &Ctx, args: ArchiveArgs) -> Result<ExitCode> {
    match args.command {
        ArchiveCommand::Zip { items, output } => {
            run_and_report(ctx, "archive.zip_create", &items, |sink, cancel| {
                tdx_archive::create(&items, &output, sink, cancel)
            })
        }
        ArchiveCommand::Unzip { file, out_dir } => run_and_report(
            ctx,
            "archive.zip_extract",
            std::slice::from_ref(&file),
            |sink, cancel| tdx_archive::extract(&file, &out_dir, sink, cancel),
        ),
        ArchiveCommand::List { file } => {
            let entries = tdx_archive::list(&file)?;
            if ctx.format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&entries).unwrap_or_else(|_| "[]".into())
                );
            } else {
                println!("{:<52} {:>12} {:>12}", "ENTRY", "SIZE", "COMPRESSED");
                for entry in &entries {
                    let name = if entry.name.len() > 50 {
                        format!("…{}", &entry.name[entry.name.len() - 49..])
                    } else {
                        entry.name.clone()
                    };
                    println!(
                        "{:<52} {:>12} {:>12}",
                        name,
                        tdx_core::fsutil::format_bytes(entry.size),
                        tdx_core::fsutil::format_bytes(entry.compressed_size)
                    );
                }
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => TdxError::not_found(path),
        _ => TdxError::Io(err),
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 256 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub fn run_file(ctx: &Ctx, args: FileArgs) -> Result<ExitCode> {
    match args.command {
        FileCommand::Hash { files } => {
            let mut entries = Vec::new();
            for path in &files {
                let hash = sha256_file(path)?;
                entries.push((path.clone(), hash));
            }
            if ctx.format == OutputFormat::Json {
                let payload: Vec<_> = entries
                    .iter()
                    .map(|(path, hash)| json!({ "path": path.display().to_string(), "sha256": hash }))
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "[]".into())
                );
            } else {
                for (path, hash) in &entries {
                    println!("{hash}  {}", path.display());
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        FileCommand::Duplicates { paths } => {
            let mut files: Vec<PathBuf> = Vec::new();
            for path in &paths {
                if path.is_dir() {
                    collect_files(path, &mut files)?;
                } else if path.is_file() {
                    files.push(path.clone());
                } else {
                    return Err(TdxError::not_found(path));
                }
            }
            let mut groups: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
            for file in &files {
                let hash = sha256_file(file)?;
                groups.entry(hash).or_default().push(file.clone());
            }
            let duplicates: Vec<(String, Vec<PathBuf>)> = groups
                .into_iter()
                .filter(|(_, files)| files.len() > 1)
                .collect();
            if ctx.format == OutputFormat::Json {
                let payload: Vec<_> = duplicates
                    .iter()
                    .map(|(hash, files)| {
                        json!({
                            "sha256": hash,
                            "files": files.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "[]".into())
                );
            } else if duplicates.is_empty() {
                println!("No duplicate files found ({} files scanned)", files.len());
            } else {
                for (hash, files) in &duplicates {
                    println!("{}", &hash[..16]);
                    for file in files {
                        println!("  {}", file.display());
                    }
                }
                println!(
                    "\n{} duplicate group(s) across {} files",
                    duplicates.len(),
                    files.len()
                );
            }
            Ok(ExitCode::SUCCESS)
        }
        FileCommand::Rename {
            files,
            prefix,
            suffix,
            replace,
            start_number,
            apply,
        } => {
            let mut plans: Vec<(PathBuf, PathBuf)> = Vec::new();
            for (index, file) in files.iter().enumerate() {
                let stem = file.file_stem().and_then(|s| s.to_str()).ok_or_else(|| {
                    TdxError::InvalidInput(format!("Invalid file name: {}", file.display()))
                })?;
                let extension = file
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_string());
                let mut new_stem = stem.to_string();
                if let Some(replacement) = &replace {
                    let (from, to) = replacement.split_once('=').ok_or_else(|| {
                        TdxError::InvalidInput("Replace format is from=to".into())
                    })?;
                    new_stem = new_stem.replace(from, to);
                }
                if let Some(prefix) = &prefix {
                    new_stem = format!("{prefix}{new_stem}");
                }
                if let Some(suffix) = &suffix {
                    new_stem = format!("{new_stem}{suffix}");
                }
                if let Some(start) = start_number {
                    new_stem = format!("{new_stem}-{:03}", start + index as u32);
                }
                let new_name = match &extension {
                    Some(extension) => format!("{new_stem}.{extension}"),
                    None => new_stem,
                };
                let parent = file.parent().map(Path::to_path_buf).unwrap_or_default();
                plans.push((file.clone(), parent.join(new_name)));
            }
            if ctx.format == OutputFormat::Json {
                let payload: Vec<_> = plans
                    .iter()
                    .map(|(from, to)| json!({ "from": from.display().to_string(), "to": to.display().to_string() }))
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "[]".into())
                );
            } else {
                for (from, to) in &plans {
                    println!("{}  →  {}", from.display(), to.display());
                }
                if !apply {
                    println!("\nDry run. Pass --apply to rename.");
                }
            }
            if apply {
                for (from, to) in &plans {
                    if from != to && to.exists() {
                        return Err(TdxError::InvalidInput(format!(
                            "Refusing to overwrite existing file: {}",
                            to.display()
                        )));
                    }
                }
                let mut renamed = 0u32;
                for (from, to) in &plans {
                    if from != to {
                        std::fs::rename(from, to)?;
                        renamed += 1;
                    }
                }
                if ctx.format != OutputFormat::Json {
                    println!("Renamed {renamed} file(s)");
                }
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn collect_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

pub fn run_inspect(ctx: &Ctx, args: InspectArgs) -> Result<ExitCode> {
    let detected = tdx_core::detect::detect(&args.file)?;
    let capabilities = detected.kind.capabilities();
    if ctx.format == OutputFormat::Json {
        let payload = json!({
            "kind": detected.kind,
            "mime": detected.mime,
            "category": detected.category(),
            "size": detected.size,
            "confidence": detected.confidence,
            "capabilities": capabilities,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".into())
        );
    } else {
        println!("File:        {}", args.file.display());
        println!("Type:        {}", detected.kind.display_name());
        println!("MIME:        {}", detected.mime);
        println!("Category:    {}", detected.category().as_str());
        println!(
            "Size:        {}",
            tdx_core::fsutil::format_bytes(detected.size)
        );
        println!("Detected by: {:?}", detected.confidence);
        println!("Capabilities: open={} preview={} edit={} convert={} merge={} split={} metadata={} batch={}",
            capabilities.open, capabilities.preview, capabilities.edit, capabilities.convert,
            capabilities.merge, capabilities.split, capabilities.metadata, capabilities.batch);
    }
    Ok(ExitCode::SUCCESS)
}

//! Filesystem helpers: atomic writes, unique output names, disk space checks.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use crate::error::{Result, TdxError};

/// Write a file atomically: write to a temporary sibling, flush, then rename.
/// The destination is never modified until the new content is fully written.
pub fn atomic_write<F>(path: &Path, write: F) -> Result<()>
where
    F: FnOnce(&mut File) -> Result<()>,
{
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf);
    if let Some(parent) = &parent {
        fs::create_dir_all(parent)?;
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            TdxError::InvalidInput(format!("Invalid output path: {}", path.display()))
        })?;
    let temp_name = format!(".{file_name}.tdx-tmp-{}", uuid::Uuid::new_v4().simple());
    let temp_path = match &parent {
        Some(parent) => parent.join(temp_name),
        None => PathBuf::from(temp_name),
    };

    let result = (|| -> Result<()> {
        let mut file = File::create(&temp_path)?;
        write(&mut file)?;
        file.flush()?;
        file.sync_all()?;
        Ok(())
    })();

    if let Err(err) = result {
        let _ = fs::remove_file(&temp_path);
        return Err(err);
    }

    if let Err(err) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(TdxError::Io(err));
    }
    Ok(())
}

/// Return `path` if it does not exist, otherwise append ` (1)`, ` (2)`… before
/// the extension until a free name is found.
pub fn unique_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let extension = path.extension().and_then(|e| e.to_str());
    for index in 1..10_000 {
        let name = match extension {
            Some(ext) => format!("{stem} ({index}).{ext}"),
            None => format!("{stem} ({index})"),
        };
        let candidate = parent.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    path.to_path_buf()
}

/// Build a default output path next to the input file:
/// `report.pdf` + suffix `merged` + extension `pdf` → `report-merged.pdf`.
pub fn default_output(input: &Path, suffix: &str, extension: &str) -> PathBuf {
    let parent = input.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let suffix = suffix.trim_matches('-');
    let name = if suffix.is_empty() {
        format!("{stem}.{extension}")
    } else {
        format!("{stem}-{suffix}.{extension}")
    };
    parent.join(name)
}

/// Free bytes available on the volume containing `path` (falls back to its
/// closest existing parent).
pub fn free_space(path: &Path) -> std::io::Result<u64> {
    let mut probe = path.to_path_buf();
    while !probe.exists() {
        match probe.parent() {
            Some(parent) => probe = parent.to_path_buf(),
            None => break,
        }
    }
    fs4::available_space(&probe)
}

/// Best-effort pre-flight disk space check. `expected` is the input size scaled
/// by an estimated expansion factor.
pub fn ensure_free_space(path: &Path, expected_bytes: u64) -> Result<()> {
    let available = match free_space(path) {
        Ok(value) => value,
        Err(_) => return Ok(()),
    };
    // Keep a safety margin of 64 MB on top of the estimate.
    let needed = expected_bytes.saturating_add(64 * 1024 * 1024);
    if available < needed {
        return Err(TdxError::InsufficientSpace {
            needed_mb: needed / (1024 * 1024),
            available_mb: available / (1024 * 1024),
        });
    }
    Ok(())
}

/// Reject paths that escape `base` after normalization (zip-slip protection).
/// Backslashes are treated as separators on every platform so Windows-style
/// traversal cannot slip through on Unix.
pub fn is_safe_relative(relative: &Path) -> bool {
    let text = relative.to_string_lossy();
    if text.contains(':') {
        return false;
    }
    let unified = text.replace('\\', "/");
    let path = Path::new(&unified);
    if path.is_absolute() {
        return false;
    }
    let mut depth: i64 = 0;
    for component in path.components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    true
}

/// Join a relative archive path onto a base directory, rejecting escapes.
pub fn safe_join(base: &Path, relative: &Path) -> Result<PathBuf> {
    if !is_safe_relative(relative) {
        return Err(TdxError::InvalidInput(format!(
            "Archive entry would escape the target directory: {}",
            relative.display()
        )));
    }
    Ok(base.join(relative))
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

pub fn format_duration_ms(duration_ms: u64) -> String {
    if duration_ms < 1000 {
        format!("{duration_ms} ms")
    } else if duration_ms < 60_000 {
        format!("{:.2} s", duration_ms as f64 / 1000.0)
    } else {
        let minutes = duration_ms / 60_000;
        let seconds = (duration_ms % 60_000) / 1000;
        format!("{minutes}m {seconds}s")
    }
}

/// Sanitize a user-provided file stem so it can be used in output names.
pub fn sanitize_file_stem(value: &str, fallback: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_alphanumeric() || matches!(ch, '-' | '_' | ' ' | '.') {
            out.push(ch);
        }
    }
    let trimmed = out.trim().trim_matches('.').trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.chars().take(120).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_zip_slip_paths() {
        assert!(is_safe_relative(Path::new("a/b/c.txt")));
        assert!(is_safe_relative(Path::new("./a.txt")));
        assert!(!is_safe_relative(Path::new("../evil.txt")));
        assert!(!is_safe_relative(Path::new("a/../../evil.txt")));
        assert!(!is_safe_relative(Path::new("C:\\Windows\\evil.txt")));
        assert!(!is_safe_relative(Path::new("/etc/passwd")));
    }

    #[test]
    fn formats_bytes() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(2048), "2.0 KB");
        assert_eq!(format_bytes(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn sanitizes_stems() {
        assert_eq!(sanitize_file_stem("my report!", "out"), "my report");
        assert_eq!(sanitize_file_stem("///", "out"), "out");
    }
}

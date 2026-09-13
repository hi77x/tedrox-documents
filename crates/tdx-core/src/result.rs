//! Structured result of a completed operation.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::detect::FileKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputArtifact {
    pub path: PathBuf,
    pub bytes: u64,
    pub kind: FileKind,
    pub label: Option<String>,
}

impl OutputArtifact {
    pub fn new(path: impl Into<PathBuf>, kind: FileKind) -> Self {
        let path = path.into();
        let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        Self {
            path,
            bytes,
            kind,
            label: None,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OperationResult {
    pub outputs: Vec<OutputArtifact>,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub duration_ms: u64,
    pub warnings: Vec<String>,
    #[serde(default)]
    pub stats: BTreeMap<String, serde_json::Value>,
}

impl OperationResult {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn single(path: impl Into<PathBuf>, kind: FileKind) -> Self {
        Self {
            outputs: vec![OutputArtifact::new(path, kind)],
            ..Self::default()
        }
    }

    pub fn with_input_bytes(mut self, bytes: u64) -> Self {
        self.bytes_in = bytes;
        self
    }

    pub fn finalize(mut self) -> Self {
        self.bytes_out = self.outputs.iter().map(|o| o.bytes).sum();
        self
    }

    pub fn warn(&mut self, message: impl Into<String>) {
        self.warnings.push(message.into());
    }

    pub fn set_stat(&mut self, key: impl Into<String>, value: impl Serialize) {
        if let Ok(value) = serde_json::to_value(value) {
            self.stats.insert(key.into(), value);
        }
    }

    pub fn first_path(&self) -> Option<&Path> {
        self.outputs.first().map(|o| o.path.as_path())
    }
}

/// Measures wall-clock duration for an operation.
pub struct Stopwatch {
    started: Instant,
}

impl Stopwatch {
    pub fn start() -> Self {
        Self {
            started: Instant::now(),
        }
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    pub fn stop(self, result: &mut OperationResult) {
        result.duration_ms = self.elapsed_ms();
    }
}

/// Sum of file sizes; used to fill `bytes_in` without loading files.
pub fn total_size(paths: &[PathBuf]) -> u64 {
    paths
        .iter()
        .map(|path| std::fs::metadata(path).map(|m| m.len()).unwrap_or(0))
        .sum()
}

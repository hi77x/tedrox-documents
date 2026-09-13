//! Progress events and cooperative cancellation.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use serde::{Deserialize, Serialize};

use crate::error::{Result, TdxError};

/// Logical stage of a job. Mirrors the states documented in the job runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Validating,
    Reading,
    Processing,
    Writing,
    Verifying,
    Completed,
}

impl Stage {
    pub fn as_str(self) -> &'static str {
        match self {
            Stage::Validating => "validating",
            Stage::Reading => "reading",
            Stage::Processing => "processing",
            Stage::Writing => "writing",
            Stage::Verifying => "verifying",
            Stage::Completed => "completed",
        }
    }

    pub fn is_terminal(self) -> bool {
        self == Stage::Completed
    }
}

/// A single progress tick. `progress` is `0.0..=1.0` for the current stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressEvent {
    pub stage: Stage,
    pub progress: f32,
    pub message: Option<String>,
    pub current: Option<u64>,
    pub total: Option<u64>,
}

impl ProgressEvent {
    pub fn new(stage: Stage, progress: f32) -> Self {
        Self {
            stage,
            progress: progress.clamp(0.0, 1.0),
            message: None,
            current: None,
            total: None,
        }
    }

    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    pub fn with_counts(mut self, current: u64, total: u64) -> Self {
        self.current = Some(current);
        self.total = Some(total);
        self
    }
}

type SinkFn = Arc<dyn Fn(ProgressEvent) + Send + Sync + 'static>;

/// Cheap cloneable progress sink. The default sink discards events.
#[derive(Clone, Default)]
pub struct ProgressSink {
    inner: Option<SinkFn>,
}

impl ProgressSink {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(ProgressEvent) + Send + Sync + 'static,
    {
        Self {
            inner: Some(Arc::new(f)),
        }
    }

    pub fn is_active(&self) -> bool {
        self.inner.is_some()
    }

    pub fn emit(&self, event: ProgressEvent) {
        if let Some(f) = &self.inner {
            f(event);
        }
    }

    pub fn stage(&self, stage: Stage, progress: f32) {
        self.emit(ProgressEvent::new(stage, progress));
    }

    pub fn message(&self, stage: Stage, progress: f32, message: impl Into<String>) {
        self.emit(ProgressEvent::new(stage, progress).with_message(message));
    }

    pub fn counts(&self, stage: Stage, current: u64, total: u64) {
        let progress = if total == 0 {
            0.0
        } else {
            current as f32 / total as f32
        };
        self.emit(ProgressEvent::new(stage, progress).with_counts(current, total));
    }
}

/// Cooperative cancellation token shared between the UI and the engine.
#[derive(Clone, Default)]
pub struct CancelToken {
    flag: Arc<AtomicBool>,
}

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    /// Returns an error when the token has been cancelled. Engines call this
    /// between page/image/row units of work.
    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(TdxError::Cancelled)
        } else {
            Ok(())
        }
    }
}

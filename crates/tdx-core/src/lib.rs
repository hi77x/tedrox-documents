//! Core types, file detection, and utilities for TEDROX Documents.
//!
//! This crate is UI-agnostic: desktop, CLI, Android and future web shells all
//! consume it. It contains no document processing logic itself — that lives in
//! the dedicated `tdx-*` engine crates.

pub mod detect;
pub mod error;
pub mod fsutil;
pub mod ops;
pub mod progress;
pub mod result;

pub use detect::{detect, detect_bytes, CapabilitySet, Category, DetectedFile, FileKind};
pub use error::{ErrorCategory, Result, TdxError};
pub use ops::{CostClass, OperationDescriptor, OperationRegistry, PlatformSupport};
pub use progress::{CancelToken, ProgressEvent, ProgressSink, Stage};
pub use result::{OperationResult, OutputArtifact};

/// Product name used in user-agent strings and generated metadata.
pub const PRODUCT_NAME: &str = "TEDROX Documents";

/// Machine-readable product id.
pub const PRODUCT_ID: &str = "space.tedrox.documents";

/// Semantic version of the core crates.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

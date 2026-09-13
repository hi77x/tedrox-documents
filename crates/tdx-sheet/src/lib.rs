//! Spreadsheet and CSV workbench for TEDROX Documents.

pub mod csv_ops;
pub mod xlsx_ops;

use std::path::Path;

use serde::{Deserialize, Serialize};
use tdx_core::{
    detect::{detect, FileKind},
    error::{Result, TdxError},
};

/// One worksheet summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetSummary {
    pub name: String,
    pub rows: u64,
    pub columns: u64,
    pub has_data: bool,
    pub used_range: Option<String>,
}

/// Workbook-level inspection result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkbookInfo {
    pub kind: FileKind,
    pub file_size: u64,
    pub sheets: Vec<SheetSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColumnType {
    Empty,
    Boolean,
    Integer,
    Float,
    DateTime,
    Text,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvInfo {
    pub delimiter: char,
    pub encoding: String,
    pub has_header: bool,
    pub columns: Vec<String>,
    pub column_types: Vec<ColumnType>,
    pub row_count: u64,
    pub sampled_rows: u64,
    pub file_size: u64,
    pub warnings: Vec<String>,
}

/// Inspect any supported tabular file.
pub fn inspect(path: &Path) -> Result<serde_json::Value> {
    let detected = detect(path)?;
    match detected.kind {
        FileKind::Csv | FileKind::Tsv => {
            let info = csv_ops::csv_info(path)?;
            Ok(serde_json::to_value(info).unwrap_or_default())
        }
        FileKind::Xlsx | FileKind::Xls | FileKind::Ods => {
            let info = xlsx_ops::workbook_info(path)?;
            Ok(serde_json::to_value(info).unwrap_or_default())
        }
        other => Err(TdxError::Unsupported(format!(
            "{} is not a supported spreadsheet format",
            other.display_name()
        ))),
    }
}

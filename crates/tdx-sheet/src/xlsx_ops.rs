//! XLSX/XLS/ODS reading through calamine and XLSX writing through
//! rust_xlsxwriter.

use std::fs::File;
use std::path::Path;

use calamine::{open_workbook_auto, Data, Reader};
use rust_xlsxwriter::{Format, Workbook};
use tdx_core::{
    error::{Result, TdxError},
    progress::{CancelToken, ProgressSink, Stage},
};

use crate::{SheetSummary, WorkbookInfo};

fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(text) => text.clone(),
        Data::Int(value) => value.to_string(),
        Data::Float(value) => {
            if value.fract() == 0.0 && value.abs() < 1e15 {
                format!("{}", *value as i64)
            } else {
                format!("{value}")
            }
        }
        Data::Bool(value) => value.to_string(),
        Data::DateTime(_) | Data::DateTimeIso(_) | Data::DurationIso(_) | Data::Error(_) => {
            cell.to_string()
        }
    }
}

/// Inspect a workbook without modifying it.
pub fn workbook_info(path: &Path) -> Result<WorkbookInfo> {
    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let mut workbook = open_workbook_auto(path)
        .map_err(|err| TdxError::Corrupt(format!("Cannot open workbook: {err}")))?;
    let names = workbook.sheet_names().to_vec();
    if names.is_empty() {
        return Err(TdxError::Corrupt("The workbook contains no sheets".into()));
    }
    let mut sheets = Vec::new();
    for name in names {
        let (rows, columns, used_range) = match workbook.worksheet_range(&name) {
            Ok(range) => {
                let (height, width) = range.get_size();
                let used = if height > 0 && width > 0 {
                    Some(format!("R1C1:R{height}C{width}"))
                } else {
                    None
                };
                (height as u64, width as u64, used)
            }
            Err(_) => (0, 0, None),
        };
        sheets.push(SheetSummary {
            name,
            rows,
            columns,
            has_data: rows > 0 && columns > 0,
            used_range,
        });
    }
    let detected = tdx_core::detect::detect(path)?;
    Ok(WorkbookInfo {
        kind: detected.kind,
        file_size,
        sheets,
    })
}

fn write_cell(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    row: u32,
    column: u16,
    value: &str,
) -> Result<()> {
    let trimmed = value.trim();
    let looks_numeric = !trimmed.is_empty()
        && trimmed.parse::<f64>().is_ok()
        && !(trimmed.len() > 1 && trimmed.starts_with('0') && !trimmed.contains(['.', ',']));
    if looks_numeric {
        if let Ok(number) = trimmed.parse::<f64>() {
            worksheet
                .write_number(row, column, number)
                .map_err(|err| TdxError::Other(format!("Cannot write XLSX cell: {err}")))?;
            return Ok(());
        }
    }
    worksheet
        .write_string(row, column, value)
        .map_err(|err| TdxError::Other(format!("Cannot write XLSX cell: {err}")))?;
    Ok(())
}

/// Stream CSV records into a new workbook.
pub fn write_records_to_workbook(
    reader: &mut csv::Reader<File>,
    sheet_name: &str,
    has_header: bool,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<Workbook> {
    let mut workbook = Workbook::new();
    let header_format = Format::new().set_bold();
    let worksheet = workbook.add_worksheet();
    worksheet
        .set_name(sheet_name)
        .map_err(|err| TdxError::Other(format!("Cannot set sheet name: {err}")))?;

    let mut row_index: u32 = 0;
    if has_header {
        let headers: Vec<String> = reader
            .headers()
            .map_err(|err| TdxError::Corrupt(format!("Cannot read CSV headers: {err}")))?
            .iter()
            .map(|value| value.to_string())
            .collect();
        for (column, header) in headers.iter().enumerate() {
            worksheet
                .write_string_with_format(0, column as u16, header, &header_format)
                .map_err(|err| TdxError::Other(format!("Cannot write XLSX cell: {err}")))?;
            let width = (header.chars().count() as f64 * 1.2).clamp(8.0, 48.0);
            worksheet
                .set_column_width(column as u16, width)
                .map_err(|err| TdxError::Other(format!("Cannot set column width: {err}")))?;
        }
        row_index = 1;
    }

    for record in reader.records() {
        cancel.check()?;
        let record = record.map_err(|err| TdxError::Corrupt(format!("Cannot read CSV: {err}")))?;
        for (column, value) in record.iter().enumerate() {
            write_cell(worksheet, row_index, column as u16, value)?;
        }
        row_index += 1;
        if row_index % 512 == 0 {
            sink.stage(Stage::Processing, 0.5);
        }
    }
    sink.stage(Stage::Writing, 0.8);
    Ok(workbook)
}

/// Read one worksheet and render it as CSV bytes.
pub fn read_sheet_as_csv(
    input: &Path,
    sheet: Option<&str>,
    delimiter: char,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<(Vec<u8>, String, u64)> {
    let mut workbook = open_workbook_auto(input)
        .map_err(|err| TdxError::Corrupt(format!("Cannot open workbook: {err}")))?;
    let names = workbook.sheet_names().to_vec();
    let sheet_name = match sheet {
        Some(name) => {
            if !names.iter().any(|existing| existing == name) {
                return Err(TdxError::InvalidInput(format!("Sheet not found: {name}")));
            }
            name.to_string()
        }
        None => names
            .first()
            .cloned()
            .ok_or_else(|| TdxError::Corrupt("The workbook contains no sheets".into()))?,
    };
    let range = workbook
        .worksheet_range(&sheet_name)
        .map_err(|err| TdxError::Corrupt(format!("Cannot read sheet {sheet_name}: {err}")))?;

    let mut writer = csv::WriterBuilder::new()
        .delimiter(delimiter as u8)
        .from_writer(Vec::new());
    let mut rows: u64 = 0;
    for row in range.rows() {
        cancel.check()?;
        let record: Vec<String> = row.iter().map(cell_to_string).collect();
        writer
            .write_record(&record)
            .map_err(|err| TdxError::Other(format!("Cannot write CSV: {err}")))?;
        rows += 1;
        if rows % 1024 == 0 {
            sink.stage(Stage::Processing, 0.5);
        }
    }
    let bytes = writer
        .into_inner()
        .map_err(|err| TdxError::Other(format!("Cannot finish CSV: {err}")))?;
    Ok((bytes, sheet_name, rows))
}

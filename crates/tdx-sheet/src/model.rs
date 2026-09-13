//! Editable workbook model used by the desktop spreadsheet editor.

use std::path::Path;

use calamine::{open_workbook_auto, Data, Reader};
use rust_xlsxwriter::{Format, Workbook};
use serde::{Deserialize, Serialize};
use tdx_core::error::{Result, TdxError};

const MAX_CELLS_PER_SHEET: usize = 40_000;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkbookModel {
    #[serde(default)]
    pub sheets: Vec<SheetModel>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SheetModel {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub cells: Vec<CellModel>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CellModel {
    pub row: u32,
    pub col: u16,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub formula: Option<String>,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
}

fn cell_to_text(cell: &Data) -> String {
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
        _ => cell.to_string(),
    }
}

fn sanitize_sheet_name(name: &str, fallback: &str) -> String {
    let mut cleaned: String = name
        .chars()
        .filter(|c| !matches!(c, '\\' | '/' | '?' | '*' | '[' | ']' | ':'))
        .collect();
    if cleaned.trim().is_empty() {
        cleaned = fallback.to_string();
    }
    cleaned.chars().take(31).collect()
}

/// Load CSV/TSV into a single-sheet workbook.
fn load_delimited(input: &Path, delimiter: Option<char>) -> Result<WorkbookModel> {
    let options = crate::csv_ops::CsvOptions {
        delimiter,
        has_header: Some(false),
    };
    let mut session = crate::csv_ops::open_session(input, &options)?;
    let mut cells = Vec::new();
    let mut rows: u32 = 0;
    for record in session.reader.records() {
        let record = record.map_err(|err| TdxError::Corrupt(format!("Cannot read CSV: {err}")))?;
        if cells.len() >= MAX_CELLS_PER_SHEET {
            break;
        }
        for (index, value) in record.iter().enumerate() {
            if value.is_empty() {
                continue;
            }
            let col = index.min(u16::MAX as usize) as u16;
            cells.push(CellModel {
                row: rows,
                col,
                value: value.to_string(),
                formula: None,
                bold: false,
                italic: false,
            });
        }
        rows += 1;
    }
    Ok(WorkbookModel {
        sheets: vec![SheetModel {
            name: "Sheet1".into(),
            cells,
        }],
    })
}

fn load_spreadsheet(input: &Path) -> Result<WorkbookModel> {
    let mut workbook = open_workbook_auto(input)
        .map_err(|err| TdxError::Corrupt(format!("Cannot open workbook: {err}")))?;
    let names = workbook.sheet_names().to_vec();
    if names.is_empty() {
        return Err(TdxError::Corrupt("The workbook contains no sheets".into()));
    }
    let mut sheets = Vec::new();
    for name in names {
        let Ok(range) = workbook.worksheet_range(&name) else {
            sheets.push(SheetModel {
                name: sanitize_sheet_name(&name, "Sheet"),
                cells: Vec::new(),
            });
            continue;
        };
        let mut cells = Vec::new();
        'outer: for (row, data_row) in range.rows().enumerate() {
            for (col, data) in data_row.iter().enumerate() {
                if cells.len() >= MAX_CELLS_PER_SHEET {
                    break 'outer;
                }
                if matches!(data, Data::Empty) {
                    continue;
                }
                cells.push(CellModel {
                    row: row as u32,
                    col: col as u16,
                    value: cell_to_text(data),
                    formula: None,
                    bold: false,
                    italic: false,
                });
            }
        }
        sheets.push(SheetModel {
            name: sanitize_sheet_name(&name, "Sheet"),
            cells,
        });
    }
    Ok(WorkbookModel { sheets })
}

/// Load any supported spreadsheet (CSV, TSV, XLSX, XLS, ODS) into the editor.
pub fn load_workbook(input: &Path) -> Result<WorkbookModel> {
    let detected = tdx_core::detect::detect(input)?;
    match detected.kind {
        tdx_core::FileKind::Csv => load_delimited(input, None),
        tdx_core::FileKind::Tsv => load_delimited(input, Some('\t')),
        tdx_core::FileKind::Xlsx | tdx_core::FileKind::Xls | tdx_core::FileKind::Ods => {
            load_spreadsheet(input)
        }
        other => Err(TdxError::Unsupported(format!(
            "{} is not a supported spreadsheet",
            other.display_name()
        ))),
    }
}

fn format_for(bold: bool, italic: bool) -> Option<Format> {
    if !bold && !italic {
        return None;
    }
    let mut format = Format::new();
    if bold {
        format = format.set_bold();
    }
    if italic {
        format = format.set_italic();
    }
    Some(format)
}

fn save_xlsx(model: &WorkbookModel, output: &Path) -> Result<()> {
    let mut workbook = Workbook::new();
    if model.sheets.is_empty() {
        workbook.add_worksheet();
    }
    for (index, sheet) in model.sheets.iter().enumerate() {
        let name = sanitize_sheet_name(&sheet.name, &format!("Sheet{}", index + 1));
        let worksheet = workbook.add_worksheet();
        let _ = worksheet.set_name(&name);
        for cell in &sheet.cells {
            let row = cell.row;
            let col = cell.col;
            let format = format_for(cell.bold, cell.italic);
            if let Some(formula) = &cell.formula {
                let _ = worksheet.write_formula(row, col, formula.as_str());
                continue;
            }
            let value = cell.value.trim();
            let numeric = !value.is_empty()
                && value.parse::<f64>().is_ok()
                && !(value.len() > 1 && value.starts_with('0') && !value.contains('.'));
            match format {
                Some(format) => {
                    if numeric {
                        if let Ok(number) = value.parse::<f64>() {
                            let _ = worksheet.write_number_with_format(row, col, number, &format);
                        }
                    } else {
                        let _ = worksheet.write_string_with_format(row, col, &cell.value, &format);
                    }
                }
                None => {
                    if numeric {
                        if let Ok(number) = value.parse::<f64>() {
                            let _ = worksheet.write_number(row, col, number);
                        }
                    } else {
                        let _ = worksheet.write_string(row, col, &cell.value);
                    }
                }
            }
        }
    }
    let directory = output.parent().map(Path::to_path_buf).unwrap_or_default();
    std::fs::create_dir_all(&directory)?;
    let temp = directory.join(format!(".tdx-sheet-{}.xlsx", uuid::Uuid::new_v4().simple()));
    workbook
        .save(&temp)
        .map_err(|err| TdxError::Other(format!("Cannot write XLSX: {err}")))?;
    std::fs::rename(&temp, output).map_err(|err| {
        let _ = std::fs::remove_file(&temp);
        TdxError::Io(err)
    })
}

fn save_delimited(model: &WorkbookModel, output: &Path, delimiter: char) -> Result<()> {
    let sheet = model.sheets.first().cloned().unwrap_or_default();
    let mut max_row = 0u32;
    let mut max_col = 0u16;
    for cell in &sheet.cells {
        max_row = max_row.max(cell.row);
        max_col = max_col.max(cell.col);
    }
    let width = (max_col as usize + 1).max(1);
    let height = (max_row as usize + 1).max(1);
    let mut grid: Vec<Vec<String>> = vec![vec![String::new(); width]; height];
    for cell in &sheet.cells {
        if let Some(row) = grid.get_mut(cell.row as usize) {
            if let Some(slot) = row.get_mut(cell.col as usize) {
                *slot = cell.value.clone();
            }
        }
    }
    let mut buffer = Vec::new();
    {
        let mut writer = csv::WriterBuilder::new()
            .delimiter(delimiter as u8)
            .from_writer(&mut buffer);
        for row in &grid {
            writer
                .write_record(row)
                .map_err(|err| TdxError::Other(format!("Cannot write CSV: {err}")))?;
        }
        writer
            .flush()
            .map_err(|err| TdxError::Other(format!("Cannot write CSV: {err}")))?;
    }
    tdx_core::fsutil::atomic_write(output, |file| {
        std::io::Write::write_all(file, &buffer).map_err(TdxError::Io)
    })
}

/// Save the editor model as XLSX or CSV depending on the extension.
pub fn save_workbook(model: &WorkbookModel, output: &Path) -> Result<()> {
    let extension = output
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_else(|| "xlsx".into());
    match extension.as_str() {
        "csv" => save_delimited(model, output, ','),
        "tsv" => save_delimited(model, output, '\t'),
        _ => save_xlsx(model, output),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_csv_model() {
        let dir = tempfile::tempdir().expect("tempdir");
        let input = dir.path().join("data.csv");
        std::fs::write(&input, "name,score\nAda,10\nBob,\n").expect("write");
        let model = load_workbook(&input).expect("load");
        assert_eq!(model.sheets.len(), 1);
        assert!(model.sheets[0].cells.iter().any(|cell| cell.value == "Ada"));

        let output = dir.path().join("out.csv");
        save_workbook(&model, &output).expect("save");
        let text = std::fs::read_to_string(&output).expect("read");
        assert!(text.contains("Ada,10"));
    }

    #[test]
    fn round_trips_xlsx_with_formatting() {
        let dir = tempfile::tempdir().expect("tempdir");
        let model = WorkbookModel {
            sheets: vec![SheetModel {
                name: "Budget".into(),
                cells: vec![
                    CellModel {
                        row: 0,
                        col: 0,
                        value: "Total".into(),
                        formula: None,
                        bold: true,
                        italic: false,
                    },
                    CellModel {
                        row: 1,
                        col: 0,
                        value: "100".into(),
                        formula: None,
                        bold: false,
                        italic: false,
                    },
                    CellModel {
                        row: 2,
                        col: 0,
                        value: "150".into(),
                        formula: Some("=SUM(A2:A2)".into()),
                        bold: false,
                        italic: true,
                    },
                ],
            }],
        };
        let output = dir.path().join("book.xlsx");
        save_workbook(&model, &output).expect("save");
        let reloaded = load_workbook(&output).expect("reload");
        assert_eq!(reloaded.sheets[0].name, "Budget");
        assert!(reloaded.sheets[0]
            .cells
            .iter()
            .any(|cell| cell.value == "Total"));
    }
}

//! CSV workbench: detection, conversion, transformation, split and join.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tdx_core::{
    detect::FileKind,
    error::{Result, TdxError},
    fsutil,
    progress::{CancelToken, ProgressSink, Stage},
    result::{OperationResult, OutputArtifact},
};

use crate::{ColumnType, CsvInfo};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CsvOptions {
    pub delimiter: Option<char>,
    pub has_header: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvToXlsxOptions {
    pub delimiter: Option<char>,
    pub sheet_name: String,
}

impl Default for CsvToXlsxOptions {
    fn default() -> Self {
        Self {
            delimiter: None,
            sheet_name: "Sheet1".into(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct XlsxToCsvOptions {
    pub sheet: Option<String>,
    pub delimiter: Option<char>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonOptions {
    pub delimiter: Option<char>,
    pub pretty: bool,
    pub infer_types: bool,
}

impl Default for JsonOptions {
    fn default() -> Self {
        Self {
            delimiter: None,
            pretty: true,
            infer_types: true,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FilterRule {
    pub column: String,
    pub contains: Option<String>,
    pub equals: Option<String>,
    pub numeric_greater: Option<f64>,
    pub numeric_less: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransformOptions {
    pub delimiter: Option<char>,
    pub output_delimiter: Option<char>,
    pub trim: bool,
    pub drop_empty_rows: bool,
    pub deduplicate: bool,
    pub select_columns: Option<Vec<String>>,
    pub rename_columns: Vec<(String, String)>,
    pub sort_by: Option<String>,
    pub sort_descending: bool,
    pub filter: Option<FilterRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitMode {
    ByRows(u64),
    ByField(String),
}

/// A prepared input: original UTF-8 file or a transcoded temporary copy.
struct PreparedInput {
    path: PathBuf,
    temporary: bool,
    encoding: String,
}

impl Drop for PreparedInput {
    fn drop(&mut self) {
        if self.temporary {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn read_sample(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let mut file = File::open(path).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => TdxError::not_found(path),
        std::io::ErrorKind::PermissionDenied => TdxError::Permission(path.display().to_string()),
        _ => TdxError::Io(err),
    })?;
    let mut buffer = vec![0u8; limit];
    let read = file.read(&mut buffer)?;
    buffer.truncate(read);
    Ok(buffer)
}

fn detect_encoding(sample: &[u8]) -> &'static encoding_rs::Encoding {
    if sample.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return encoding_rs::UTF_8;
    }
    if sample.starts_with(&[0xFF, 0xFE]) {
        return encoding_rs::UTF_16LE;
    }
    if sample.starts_with(&[0xFE, 0xFF]) {
        return encoding_rs::UTF_16BE;
    }
    if std::str::from_utf8(sample).is_ok() {
        return encoding_rs::UTF_8;
    }
    let mut detector = chardetng::EncodingDetector::new(chardetng::Iso2022JpDetection::Deny);
    detector.feed(sample, true);
    detector.guess(None, chardetng::Utf8Detection::Allow)
}

fn transcode_to_utf8(input: &Path, encoding: &'static encoding_rs::Encoding) -> Result<PathBuf> {
    let temp = std::env::temp_dir().join(format!("tdx-csv-{}.utf8", uuid::Uuid::new_v4().simple()));
    let reader = File::open(input)?;
    let mut reader = BufReader::new(reader);
    let writer = File::create(&temp)?;
    let mut writer = BufWriter::new(writer);
    let mut decoder = encoding.new_decoder();
    let mut input_buffer = [0u8; 64 * 1024];
    let mut text = String::with_capacity(128 * 1024);
    loop {
        let read = reader.read(&mut input_buffer)?;
        let last = read == 0;
        let (_result, _consumed, _finished) =
            decoder.decode_to_string(&input_buffer[..read], &mut text, last);
        writer.write_all(text.as_bytes())?;
        text.clear();
        if last {
            break;
        }
    }
    writer.flush()?;
    Ok(temp)
}

fn prepare_input(path: &Path) -> Result<PreparedInput> {
    let sample = read_sample(path, 256 * 1024)?;
    let encoding = detect_encoding(&sample);
    let encoding_name = encoding.name().to_string();
    if encoding == encoding_rs::UTF_8 && !sample.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Ok(PreparedInput {
            path: path.to_path_buf(),
            temporary: false,
            encoding: encoding_name,
        });
    }
    let transcoded = transcode_to_utf8(path, encoding)?;
    Ok(PreparedInput {
        path: transcoded,
        temporary: true,
        encoding: encoding_name,
    })
}

/// Best-effort delimiter detection over the first 20 non-empty lines.
pub fn detect_delimiter(text: &str) -> char {
    let candidates = [',', ';', '\t', '|'];
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(20)
        .collect();
    if lines.is_empty() {
        return ',';
    }
    let mut best = (',', 0.0f64);
    for candidate in candidates {
        let counts: Vec<usize> = lines
            .iter()
            .map(|line| line.matches(candidate).count())
            .collect();
        let header_count = counts.first().copied().unwrap_or(0);
        if header_count == 0 {
            continue;
        }
        let consistent = counts
            .iter()
            .filter(|count| **count == header_count)
            .count() as f64;
        let score = header_count as f64 * (consistent / counts.len() as f64);
        if score > best.1 {
            best = (candidate, score);
        }
    }
    best.0
}

fn header_heuristic(records: &[Vec<String>]) -> bool {
    if records.len() < 2 {
        return true;
    }
    let first = &records[0];
    if first.is_empty() {
        return true;
    }
    let first_has_numbers = first.iter().any(|value| {
        let trimmed = value.trim();
        !trimmed.is_empty() && trimmed.parse::<f64>().is_ok()
    });
    let first_all_filled = first.iter().all(|value| !value.trim().is_empty());
    let unique: HashSet<&String> = first.iter().collect();
    let unique_ok = unique.len() == first.len();
    let second_has_numbers = records[1].iter().any(|value| {
        let trimmed = value.trim();
        !trimmed.is_empty() && trimmed.parse::<f64>().is_ok()
    });
    // Typical header: text-only unique labels; data below is numeric or longer.
    if !first_has_numbers
        && first_all_filled
        && unique_ok
        && (second_has_numbers || first.len() > 1)
    {
        return true;
    }
    !first_has_numbers && second_has_numbers && unique_ok
}

pub(crate) struct Session {
    pub(crate) reader: csv::Reader<File>,
    delimiter: char,
    has_header: bool,
    encoding: String,
    _input: PreparedInput,
}

pub(crate) fn open_session(path: &Path, options: &CsvOptions) -> Result<Session> {
    let input = prepare_input(path)?;
    let sample = read_sample(&input.path, 128 * 1024)?;
    let sample_text = String::from_utf8_lossy(&sample);
    let delimiter = options
        .delimiter
        .unwrap_or_else(|| detect_delimiter(&sample_text));
    let has_header = match options.has_header {
        Some(value) => value,
        None => {
            let mut probe = csv::ReaderBuilder::new()
                .delimiter(delimiter as u8)
                .has_headers(false)
                .flexible(true)
                .from_path(&input.path)
                .map_err(|err| TdxError::Corrupt(format!("Cannot read CSV: {err}")))?;
            let records: Vec<Vec<String>> = probe
                .records()
                .take(5)
                .filter_map(|record| record.ok())
                .map(|record| record.iter().map(|value| value.to_string()).collect())
                .collect();
            header_heuristic(&records)
        }
    };
    let reader = csv::ReaderBuilder::new()
        .delimiter(delimiter as u8)
        .has_headers(has_header)
        .flexible(true)
        .from_path(&input.path)
        .map_err(|err| TdxError::Corrupt(format!("Cannot read CSV: {err}")))?;
    let encoding = input.encoding.clone();
    Ok(Session {
        reader,
        delimiter,
        has_header,
        encoding,
        _input: input,
    })
}

fn infer_column_type(values: &[String]) -> ColumnType {
    let mut saw_value = false;
    let mut all_int = true;
    let mut all_float = true;
    let mut all_bool = true;
    let mut all_empty = true;
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        saw_value = true;
        all_empty = false;
        if trimmed.parse::<i64>().is_err() {
            all_int = false;
        }
        if trimmed.parse::<f64>().is_err() {
            all_float = false;
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower != "true" && lower != "false" && lower != "yes" && lower != "no" {
            all_bool = false;
        }
    }
    if !saw_value || all_empty {
        return ColumnType::Empty;
    }
    if all_bool {
        return ColumnType::Boolean;
    }
    if all_int {
        return ColumnType::Integer;
    }
    if all_float {
        return ColumnType::Float;
    }
    ColumnType::Text
}

/// Detailed CSV report.
pub fn csv_info(path: &Path) -> Result<CsvInfo> {
    let mut session = open_session(path, &CsvOptions::default())?;
    let headers: Vec<String> = if session.has_header {
        session
            .reader
            .headers()
            .map_err(|err| TdxError::Corrupt(format!("Cannot read CSV headers: {err}")))?
            .iter()
            .map(|value| value.to_string())
            .collect()
    } else {
        let first = session.reader.records().next();
        match first {
            Some(Ok(record)) => record
                .iter()
                .enumerate()
                .map(|(index, _)| format!("Column {}", index + 1))
                .collect(),
            _ => Vec::new(),
        }
    };
    let column_count = headers.len();
    let mut samples: Vec<Vec<String>> = vec![Vec::new(); column_count];
    let mut row_count: u64 = 0;
    for record in session.reader.records() {
        let record = record.map_err(|err| TdxError::Corrupt(format!("Cannot read CSV: {err}")))?;
        if row_count < 1000 {
            for (index, value) in record.iter().enumerate() {
                if index < column_count {
                    samples[index].push(value.to_string());
                }
            }
        }
        row_count += 1;
    }
    let warnings = if session.encoding != "UTF-8" {
        vec![format!("Input decoded from {}", session.encoding)]
    } else {
        Vec::new()
    };
    Ok(CsvInfo {
        delimiter: session.delimiter,
        encoding: session.encoding.clone(),
        has_header: session.has_header,
        columns: headers,
        column_types: samples
            .iter()
            .map(|values| infer_column_type(values))
            .collect(),
        row_count,
        sampled_rows: row_count.min(1000),
        file_size: std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
        warnings,
    })
}

fn infer_json_value(value: &str, infer: bool) -> serde_json::Value {
    if !infer {
        return serde_json::Value::String(value.to_string());
    }
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return serde_json::Value::Null;
    }
    if trimmed.eq_ignore_ascii_case("true") {
        return serde_json::Value::Bool(true);
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return serde_json::Value::Bool(false);
    }
    if let Ok(integer) = trimmed.parse::<i64>() {
        if trimmed.len() < 19 {
            return serde_json::Value::Number(integer.into());
        }
    }
    if let Ok(float) = trimmed.parse::<f64>() {
        if let Some(number) = serde_json::Number::from_f64(float) {
            return serde_json::Value::Number(number);
        }
    }
    serde_json::Value::String(value.to_string())
}

fn write_output_bytes(output: &Path, bytes: &[u8]) -> Result<()> {
    fsutil::atomic_write(output, |file| file.write_all(bytes).map_err(TdxError::Io))
}

/// Convert CSV to XLSX.
pub fn csv_to_xlsx(
    input: &Path,
    output: &Path,
    options: &CsvToXlsxOptions,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    sink.stage(Stage::Reading, 0.05);
    let mut session = open_session(
        input,
        &CsvOptions {
            delimiter: options.delimiter,
            has_header: None,
        },
    )?;
    let sheet_name = if options.sheet_name.trim().is_empty() {
        "Sheet1".to_string()
    } else {
        options.sheet_name.clone()
    };
    let mut workbook = crate::xlsx_ops::write_records_to_workbook(
        &mut session.reader,
        &sheet_name,
        session.has_header,
        sink,
        cancel,
    )?;
    let temp = output.with_extension(format!("tdx-{}.xlsx", uuid::Uuid::new_v4().simple()));
    workbook
        .save(&temp)
        .map_err(|err| TdxError::Other(format!("Cannot write XLSX: {err}")))?;
    std::fs::rename(&temp, output).map_err(|err| {
        let _ = std::fs::remove_file(&temp);
        TdxError::Io(err)
    })?;
    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Xlsx);
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    result.set_stat("sheet", sheet_name);
    Ok(result.finalize())
}

/// Convert a worksheet to CSV.
pub fn xlsx_to_csv(
    input: &Path,
    output: &Path,
    options: &XlsxToCsvOptions,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    sink.stage(Stage::Reading, 0.1);
    let delimiter = options.delimiter.unwrap_or(',');
    let (bytes, sheet_name, rows) = crate::xlsx_ops::read_sheet_as_csv(
        input,
        options.sheet.as_deref(),
        delimiter,
        sink,
        cancel,
    )?;
    sink.stage(Stage::Writing, 0.9);
    write_output_bytes(output, &bytes)?;
    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Csv);
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    result.set_stat("sheet", sheet_name);
    result.set_stat("rows", rows);
    Ok(result.finalize())
}

/// Convert CSV to a JSON array.
pub fn csv_to_json(
    input: &Path,
    output: &Path,
    options: &JsonOptions,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    sink.stage(Stage::Reading, 0.05);
    let mut session = open_session(
        input,
        &CsvOptions {
            delimiter: options.delimiter,
            has_header: None,
        },
    )?;
    let headers: Vec<String> = session
        .reader
        .headers()
        .map_err(|err| TdxError::Corrupt(format!("Cannot read CSV headers: {err}")))?
        .iter()
        .map(|value| value.to_string())
        .collect();
    if headers.is_empty() {
        return Err(TdxError::InvalidInput("The CSV file has no columns".into()));
    }

    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(b"[\n");
    let mut first = true;
    for record in session.reader.records() {
        cancel.check()?;
        let record = record.map_err(|err| TdxError::Corrupt(format!("Cannot read CSV: {err}")))?;
        let mut object = serde_json::Map::new();
        for (index, header) in headers.iter().enumerate() {
            let value = record.get(index).unwrap_or("");
            object.insert(header.clone(), infer_json_value(value, options.infer_types));
        }
        if !first {
            out.extend_from_slice(b",\n");
        }
        first = false;
        let serialized = if options.pretty {
            serde_json::to_string_pretty(&serde_json::Value::Object(object))
        } else {
            serde_json::to_string(&serde_json::Value::Object(object))
        }
        .map_err(|err| TdxError::Other(err.to_string()))?;
        out.extend_from_slice(serialized.as_bytes());
    }
    out.extend_from_slice(b"\n]\n");
    sink.stage(Stage::Writing, 0.9);
    write_output_bytes(output, &out)?;
    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Json);
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    result.set_stat("columns", headers.len());
    Ok(result.finalize())
}

/// Convert a JSON array of objects into CSV.
pub fn json_to_csv(
    input: &Path,
    output: &Path,
    options: &JsonOptions,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    sink.stage(Stage::Reading, 0.1);
    cancel.check()?;
    let bytes = std::fs::read(input).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => TdxError::not_found(input),
        _ => TdxError::Io(err),
    })?;
    if bytes.len() > 256 * 1024 * 1024 {
        return Err(TdxError::InvalidInput(
            "JSON file is larger than 256 MB".into(),
        ));
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|err| TdxError::InvalidInput(format!("Invalid JSON: {err}")))?;
    let rows: Vec<&serde_json::Map<String, serde_json::Value>> = match &value {
        serde_json::Value::Array(items) => items
            .iter()
            .map(|item| {
                item.as_object().ok_or_else(|| {
                    TdxError::InvalidInput("JSON array must contain objects only".into())
                })
            })
            .collect::<Result<Vec<_>>>()?,
        serde_json::Value::Object(object) => vec![object],
        _ => {
            return Err(TdxError::InvalidInput(
                "JSON to CSV expects an array of objects or one object".into(),
            ))
        }
    };
    if rows.is_empty() {
        return Err(TdxError::InvalidInput("The JSON array is empty".into()));
    }
    let mut headers: Vec<String> = Vec::new();
    for row in &rows {
        for key in row.keys() {
            if !headers.iter().any(|header| header == key) {
                headers.push(key.clone());
            }
        }
    }
    let delimiter = options.delimiter.unwrap_or(',');
    let mut writer = csv::WriterBuilder::new()
        .delimiter(delimiter as u8)
        .from_writer(Vec::new());
    writer
        .write_record(&headers)
        .map_err(|err| TdxError::Other(err.to_string()))?;
    for row in &rows {
        cancel.check()?;
        let record: Vec<String> = headers
            .iter()
            .map(|header| match row.get(header) {
                Some(serde_json::Value::Null) | None => String::new(),
                Some(serde_json::Value::String(text)) => text.clone(),
                Some(other) => other.to_string(),
            })
            .collect();
        writer
            .write_record(&record)
            .map_err(|err| TdxError::Other(err.to_string()))?;
    }
    let bytes = writer
        .into_inner()
        .map_err(|err| TdxError::Other(err.to_string()))?;
    sink.stage(Stage::Writing, 0.9);
    write_output_bytes(output, &bytes)?;
    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Csv);
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    result.set_stat("columns", headers.len());
    result.set_stat("rows", rows.len());
    Ok(result.finalize())
}

struct WritableTemp {
    final_path: PathBuf,
    temp_path: PathBuf,
    writer: Option<csv::Writer<File>>,
}

impl WritableTemp {
    fn create(final_path: PathBuf, delimiter: char) -> Result<Self> {
        if let Some(parent) = final_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temp_path = final_path.with_file_name(format!(
            ".{}.tdx-tmp-{}",
            final_path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| "output.csv".into()),
            uuid::Uuid::new_v4().simple()
        ));
        let writer = csv::WriterBuilder::new()
            .delimiter(delimiter as u8)
            .from_path(&temp_path)
            .map_err(|err| TdxError::Other(format!("Cannot write CSV: {err}")))?;
        Ok(Self {
            final_path,
            temp_path,
            writer: Some(writer),
        })
    }

    fn writer(&mut self) -> &mut csv::Writer<File> {
        self.writer.as_mut().expect("writer available")
    }

    fn commit(mut self) -> Result<()> {
        let mut writer = self.writer.take().expect("writer available");
        writer
            .flush()
            .map_err(|err| TdxError::Other(err.to_string()))?;
        drop(writer);
        std::fs::rename(&self.temp_path, &self.final_path).map_err(|err| {
            let _ = std::fs::remove_file(&self.temp_path);
            TdxError::Io(err)
        })
    }
}

impl Drop for WritableTemp {
    fn drop(&mut self) {
        if self.writer.is_some() {
            let _ = std::fs::remove_file(&self.temp_path);
        }
    }
}

/// Apply transformations and write a new CSV.
pub fn transform(
    input: &Path,
    output: &Path,
    options: &TransformOptions,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    sink.stage(Stage::Reading, 0.05);
    let mut session = open_session(
        input,
        &CsvOptions {
            delimiter: options.delimiter,
            has_header: Some(true),
        },
    )?;
    let headers: Vec<String> = session
        .reader
        .headers()
        .map_err(|err| TdxError::Corrupt(format!("Cannot read CSV headers: {err}")))?
        .iter()
        .map(|value| value.to_string())
        .collect();
    if headers.is_empty() {
        return Err(TdxError::InvalidInput("The CSV file has no columns".into()));
    }
    let delimiter = options
        .output_delimiter
        .or(options.delimiter)
        .unwrap_or(',');

    // Rename columns.
    let renamed: Vec<String> = headers
        .iter()
        .map(|header| {
            options
                .rename_columns
                .iter()
                .find(|(from, _)| from == header)
                .map(|(_, to)| to.clone())
                .unwrap_or_else(|| header.clone())
        })
        .collect();

    // Column selection.
    let indices: Vec<usize> = match &options.select_columns {
        Some(selected) => {
            let mut indices = Vec::new();
            for name in selected {
                let index = renamed
                    .iter()
                    .position(|header| header == name)
                    .or_else(|| headers.iter().position(|header| header == name))
                    .ok_or_else(|| TdxError::InvalidInput(format!("Column not found: {name}")))?;
                indices.push(index);
            }
            indices
        }
        None => (0..headers.len()).collect(),
    };

    let filter_index = match &options.filter {
        Some(rule) => Some(
            headers
                .iter()
                .position(|header| header == &rule.column)
                .ok_or_else(|| {
                    TdxError::InvalidInput(format!("Filter column not found: {}", rule.column))
                })?,
        ),
        None => None,
    };
    let sort_index = match &options.sort_by {
        Some(name) => Some(
            renamed
                .iter()
                .position(|header| header == name)
                .or_else(|| headers.iter().position(|header| header == name))
                .ok_or_else(|| TdxError::InvalidInput(format!("Sort column not found: {name}")))?,
        ),
        None => None,
    };

    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut seen: HashSet<u64> = HashSet::new();
    for record in session.reader.records() {
        cancel.check()?;
        let record = record.map_err(|err| TdxError::Corrupt(format!("Cannot read CSV: {err}")))?;
        let mut values: Vec<String> = record.iter().map(|value| value.to_string()).collect();
        values.resize(headers.len(), String::new());
        if options.trim {
            for value in values.iter_mut() {
                *value = value.trim().to_string();
            }
        }
        if options.drop_empty_rows && values.iter().all(|value| value.trim().is_empty()) {
            continue;
        }
        if let (Some(rule), Some(index)) = (&options.filter, filter_index) {
            let value = values.get(index).cloned().unwrap_or_default();
            if let Some(contains) = &rule.contains {
                if !value.contains(contains) {
                    continue;
                }
            }
            if let Some(equals) = &rule.equals {
                if &value != equals {
                    continue;
                }
            }
            if let Some(minimum) = rule.numeric_greater {
                match value.trim().parse::<f64>() {
                    Ok(number) if number > minimum => {}
                    _ => continue,
                }
            }
            if let Some(maximum) = rule.numeric_less {
                match value.trim().parse::<f64>() {
                    Ok(number) if number < maximum => {}
                    _ => continue,
                }
            }
        }
        if options.deduplicate {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            values.hash(&mut hasher);
            if !seen.insert(hasher.finish()) {
                continue;
            }
        }
        rows.push(values);
    }

    if let Some(index) = sort_index {
        rows.sort_by(|left, right| {
            let a = left.get(index).map(String::as_str).unwrap_or("");
            let b = right.get(index).map(String::as_str).unwrap_or("");
            match (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
                (Ok(x), Ok(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
                _ => a.to_lowercase().cmp(&b.to_lowercase()),
            }
        });
        if options.sort_descending {
            rows.reverse();
        }
    }

    sink.stage(Stage::Writing, 0.7);
    let mut target = WritableTemp::create(output.to_path_buf(), delimiter)?;
    let header_record: Vec<&str> = indices
        .iter()
        .map(|index| renamed[*index].as_str())
        .collect();
    target
        .writer()
        .write_record(&header_record)
        .map_err(|err| TdxError::Other(err.to_string()))?;
    for row in &rows {
        let record: Vec<&str> = indices
            .iter()
            .map(|index| row.get(*index).map(String::as_str).unwrap_or(""))
            .collect();
        target
            .writer()
            .write_record(&record)
            .map_err(|err| TdxError::Other(err.to_string()))?;
    }
    target.commit()?;

    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Csv);
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    result.set_stat("rows", rows.len());
    result.set_stat("columns", indices.len());
    Ok(result.finalize())
}

/// Split a CSV by row count or by a column value.
pub fn split(
    input: &Path,
    output_dir: &Path,
    mode: &SplitMode,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    sink.stage(Stage::Reading, 0.05);
    std::fs::create_dir_all(output_dir)?;
    let stem = fsutil::sanitize_file_stem(
        input.file_stem().and_then(|s| s.to_str()).unwrap_or("data"),
        "data",
    );
    let mut session = open_session(input, &CsvOptions::default())?;
    let headers: Vec<String> = session
        .reader
        .headers()
        .map_err(|err| TdxError::Corrupt(format!("Cannot read CSV headers: {err}")))?
        .iter()
        .map(|value| value.to_string())
        .collect();
    let delimiter = session.delimiter;

    let mut outputs: Vec<PathBuf> = Vec::new();
    match mode {
        SplitMode::ByRows(rows_per_file) => {
            let rows_per_file = (*rows_per_file).max(1);
            let mut part = 1u64;
            let mut in_part = 0u64;
            let mut target: Option<WritableTemp> = None;
            for record in session.reader.records() {
                cancel.check()?;
                let record =
                    record.map_err(|err| TdxError::Corrupt(format!("Cannot read CSV: {err}")))?;
                if in_part == 0 {
                    let path =
                        fsutil::unique_path(&output_dir.join(format!("{stem}-part-{part:03}.csv")));
                    let mut new_target = WritableTemp::create(path.clone(), delimiter)?;
                    new_target
                        .writer()
                        .write_record(&headers)
                        .map_err(|err| TdxError::Other(err.to_string()))?;
                    outputs.push(path);
                    target = Some(new_target);
                    part += 1;
                }
                target
                    .as_mut()
                    .expect("target")
                    .writer()
                    .write_record(record.iter())
                    .map_err(|err| TdxError::Other(err.to_string()))?;
                in_part += 1;
                if in_part >= rows_per_file {
                    if let Some(finished) = target.take() {
                        finished.commit()?;
                    }
                    in_part = 0;
                }
            }
            if let Some(finished) = target.take() {
                finished.commit()?;
            }
        }
        SplitMode::ByField(column) => {
            let index = headers
                .iter()
                .position(|header| header == column)
                .ok_or_else(|| TdxError::InvalidInput(format!("Column not found: {column}")))?;
            let mut groups: HashMap<String, WritableTemp> = HashMap::new();
            for record in session.reader.records() {
                cancel.check()?;
                let record =
                    record.map_err(|err| TdxError::Corrupt(format!("Cannot read CSV: {err}")))?;
                let value = record.get(index).unwrap_or("").trim().to_string();
                let key = if value.is_empty() {
                    "empty".to_string()
                } else {
                    value
                };
                if !groups.contains_key(&key) {
                    if groups.len() >= 96 {
                        return Err(TdxError::InvalidInput(
                            "This column has more than 96 distinct values; splitting by field is limited to 96 groups"
                                .into(),
                        ));
                    }
                    let file_stem = fsutil::sanitize_file_stem(&key, "group");
                    let path =
                        fsutil::unique_path(&output_dir.join(format!("{stem}-{file_stem}.csv")));
                    let mut target = WritableTemp::create(path.clone(), delimiter)?;
                    target
                        .writer()
                        .write_record(&headers)
                        .map_err(|err| TdxError::Other(err.to_string()))?;
                    outputs.push(path);
                    groups.insert(key.clone(), target);
                }
                groups
                    .get_mut(&key)
                    .expect("group")
                    .writer()
                    .write_record(record.iter())
                    .map_err(|err| TdxError::Other(err.to_string()))?;
            }
            for (_, target) in groups {
                target.commit()?;
            }
        }
    }

    if outputs.is_empty() {
        return Err(TdxError::InvalidInput(
            "The CSV file has no data rows to split".into(),
        ));
    }
    let artifacts: Vec<OutputArtifact> = outputs
        .iter()
        .map(|path| OutputArtifact::new(path.clone(), FileKind::Csv))
        .collect();
    let mut result = OperationResult {
        outputs: artifacts,
        ..OperationResult::default()
    };
    result.bytes_in = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    result.set_stat("files", outputs.len());
    Ok(result.finalize())
}

/// Concatenate CSV files with the same structure.
pub fn join(
    inputs: &[PathBuf],
    output: &Path,
    delimiter: Option<char>,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    if inputs.len() < 2 {
        return Err(TdxError::InvalidInput(
            "Select at least two CSV files to join".into(),
        ));
    }
    sink.stage(Stage::Reading, 0.05);
    let output_delimiter = delimiter.unwrap_or(',');
    let mut target = WritableTemp::create(output.to_path_buf(), output_delimiter)?;
    let mut header_written = false;
    let mut header_reference: Vec<String> = Vec::new();
    let mut total_rows: u64 = 0;
    let mut warnings: Vec<String> = Vec::new();
    let total = inputs.len();

    for (index, path) in inputs.iter().enumerate() {
        cancel.check()?;
        sink.stage(Stage::Processing, index as f32 / total as f32);
        // Joining assumes the first row is a header so it is written once.
        let mut session = open_session(
            path,
            &CsvOptions {
                delimiter: None,
                has_header: Some(true),
            },
        )?;
        let headers: Vec<String> = session
            .reader
            .headers()
            .map_err(|err| TdxError::Corrupt(format!("Cannot read CSV headers: {err}")))?
            .iter()
            .map(|value| value.to_string())
            .collect();
        if !header_written {
            header_reference = headers.clone();
            target
                .writer()
                .write_record(&headers)
                .map_err(|err| TdxError::Other(err.to_string()))?;
            header_written = true;
        } else if headers != header_reference {
            warnings.push(format!(
                "{} has a different header row; its rows were appended positionally",
                path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            ));
        }
        for record in session.reader.records() {
            let record =
                record.map_err(|err| TdxError::Corrupt(format!("Cannot read CSV: {err}")))?;
            target
                .writer()
                .write_record(record.iter())
                .map_err(|err| TdxError::Other(err.to_string()))?;
            total_rows += 1;
        }
    }
    target.commit()?;
    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Csv);
    result.bytes_in = inputs
        .iter()
        .map(|path| std::fs::metadata(path).map(|m| m.len()).unwrap_or(0))
        .sum();
    result.set_stat("rows", total_rows);
    result.set_stat("files", inputs.len());
    result.warnings = warnings;
    Ok(result.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_common_delimiters() {
        assert_eq!(detect_delimiter("a,b,c\n1,2,3\n"), ',');
        assert_eq!(detect_delimiter("a;b;c\n1;2;3\n"), ';');
        assert_eq!(detect_delimiter("a\tb\tc\n1\t2\t3\n"), '\t');
    }

    #[test]
    fn infers_types() {
        assert_eq!(
            infer_column_type(&["1".into(), "2".into()]),
            ColumnType::Integer
        );
        assert_eq!(
            infer_column_type(&["1.5".into(), "2".into()]),
            ColumnType::Float
        );
        assert_eq!(
            infer_column_type(&["true".into(), "false".into()]),
            ColumnType::Boolean
        );
        assert_eq!(infer_column_type(&["hello".into()]), ColumnType::Text);
        assert_eq!(infer_column_type(&[String::new()]), ColumnType::Empty);
    }

    #[test]
    fn infers_json_scalars() {
        assert!(infer_json_value("42", true).is_i64());
        assert!(infer_json_value("true", true).is_boolean());
        assert!(infer_json_value("", true).is_null());
        assert!(infer_json_value("abc", true).is_string());
    }
}

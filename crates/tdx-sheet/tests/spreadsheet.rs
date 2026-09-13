//! CSV and XLSX integration tests.

use tdx_core::progress::{CancelToken, ProgressSink};

fn write_csv(path: &std::path::Path, content: &str) {
    std::fs::write(path, content).expect("write csv");
}

#[test]
fn csv_info_detects_structure() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("people.csv");
    write_csv(&path, "name,age,city\nAda,36,London\nGrace,45,New York\n");
    let info = tdx_sheet::csv_ops::csv_info(&path).expect("info");
    assert_eq!(info.delimiter, ',');
    assert!(info.has_header);
    assert_eq!(info.row_count, 2);
    assert_eq!(info.columns, vec!["name", "age", "city"]);
    assert_eq!(info.column_types[1], tdx_sheet::ColumnType::Integer);
}

#[test]
fn csv_xlsx_roundtrip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let csv = dir.path().join("data.csv");
    write_csv(&csv, "id,label,amount\n1,alpha,10.5\n2,beta,20\n");
    let xlsx = dir.path().join("data.xlsx");
    let options = tdx_sheet::csv_ops::CsvToXlsxOptions::default();
    tdx_sheet::csv_ops::csv_to_xlsx(
        &csv,
        &xlsx,
        &options,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("csv to xlsx");
    assert!(xlsx.exists());

    let info = tdx_sheet::xlsx_ops::workbook_info(&xlsx).expect("workbook info");
    assert_eq!(info.sheets.len(), 1);
    assert_eq!(info.sheets[0].rows, 3, "header + two rows");

    let back = dir.path().join("back.csv");
    let csv_options = tdx_sheet::csv_ops::XlsxToCsvOptions::default();
    tdx_sheet::csv_ops::xlsx_to_csv(
        &xlsx,
        &back,
        &csv_options,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("xlsx to csv");
    let content = std::fs::read_to_string(&back).expect("read");
    assert!(content.contains("alpha"));
    assert!(content.contains("20"));
}

#[test]
fn csv_json_roundtrip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let csv = dir.path().join("data.csv");
    write_csv(&csv, "id,active\n1,true\n2,false\n");
    let json = dir.path().join("data.json");
    tdx_sheet::csv_ops::csv_to_json(
        &csv,
        &json,
        &tdx_sheet::csv_ops::JsonOptions::default(),
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("csv to json");
    let value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&json).expect("read")).expect("parse");
    assert_eq!(value[0]["active"], serde_json::Value::Bool(true));

    let back = dir.path().join("back.csv");
    tdx_sheet::csv_ops::json_to_csv(
        &json,
        &back,
        &tdx_sheet::csv_ops::JsonOptions::default(),
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("json to csv");
    let content = std::fs::read_to_string(&back).expect("read");
    assert!(content.contains("id,active"));
}

#[test]
fn transform_filters_dedupes_and_selects() {
    let dir = tempfile::tempdir().expect("tempdir");
    let csv = dir.path().join("data.csv");
    write_csv(
        &csv,
        "name,team,score\nAda,core,10\nBob,core,20\nAda,core,10\nCara,infra,30\n",
    );
    let out = dir.path().join("out.csv");
    let options = tdx_sheet::csv_ops::TransformOptions {
        trim: true,
        deduplicate: true,
        select_columns: Some(vec!["name".into(), "score".into()]),
        filter: Some(tdx_sheet::csv_ops::FilterRule {
            column: "team".into(),
            equals: Some("core".into()),
            ..Default::default()
        }),
        ..Default::default()
    };
    tdx_sheet::csv_ops::transform(
        &csv,
        &out,
        &options,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("transform");
    let content = std::fs::read_to_string(&out).expect("read");
    assert!(content.starts_with("name,score"));
    assert!(content.contains("Ada,10"));
    assert!(!content.contains("Cara"));
}

#[test]
fn transform_sorts_numerically() {
    let dir = tempfile::tempdir().expect("tempdir");
    let csv = dir.path().join("data.csv");
    write_csv(&csv, "name,score\nAda,9\nBob,100\nCara,20\n");
    let out = dir.path().join("sorted.csv");
    let options = tdx_sheet::csv_ops::TransformOptions {
        sort_by: Some("score".into()),
        sort_descending: true,
        ..Default::default()
    };
    tdx_sheet::csv_ops::transform(
        &csv,
        &out,
        &options,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("sort");
    let content = std::fs::read_to_string(&out).expect("read");
    let bob = content.find("Bob").expect("bob");
    let cara = content.find("Cara").expect("cara");
    let ada = content.find("Ada").expect("ada");
    assert!(bob < cara && cara < ada);
}

#[test]
fn split_by_rows_and_field() {
    let dir = tempfile::tempdir().expect("tempdir");
    let csv = dir.path().join("data.csv");
    write_csv(
        &csv,
        "name,team\nAda,core\nBob,core\nCara,infra\nDan,infra\n",
    );
    let by_rows = dir.path().join("rows");
    tdx_sheet::csv_ops::split(
        &csv,
        &by_rows,
        &tdx_sheet::csv_ops::SplitMode::ByRows(2),
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("split rows");
    assert_eq!(std::fs::read_dir(&by_rows).expect("dir").count(), 2);

    let by_field = dir.path().join("fields");
    tdx_sheet::csv_ops::split(
        &csv,
        &by_field,
        &tdx_sheet::csv_ops::SplitMode::ByField("team".into()),
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("split field");
    assert_eq!(std::fs::read_dir(&by_field).expect("dir").count(), 2);
}

#[test]
fn join_concatenates_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let first = dir.path().join("a.csv");
    let second = dir.path().join("b.csv");
    write_csv(&first, "name\nAda\n");
    write_csv(&second, "name\nBob\n");
    let out = dir.path().join("joined.csv");
    tdx_sheet::csv_ops::join(
        &[first, second],
        &out,
        None,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("join");
    let content = std::fs::read_to_string(&out).expect("read");
    assert_eq!(content.matches("name").count(), 1, "header written once");
    assert!(content.contains("Ada") && content.contains("Bob"));
}

#[test]
fn windows_1251_input_is_decoded() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("legacy.csv");
    // "имя,город\nАда,Москва\n" encoded as windows-1251.
    let text = "имя,город\nАда,Москва\n";
    let (bytes, _, _) = encoding_rs::WINDOWS_1251.encode(text);
    std::fs::write(&path, bytes.as_ref()).expect("write");
    let info = tdx_sheet::csv_ops::csv_info(&path).expect("info");
    assert_eq!(info.columns[0], "имя");
    assert_eq!(info.row_count, 1);
}

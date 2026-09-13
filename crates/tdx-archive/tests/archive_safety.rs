//! ZIP creation, extraction and safety tests.

use std::io::Write;

use tdx_core::progress::{CancelToken, ProgressSink};

#[test]
fn create_list_extract_roundtrip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let payload = dir.path().join("payload");
    std::fs::create_dir_all(payload.join("nested")).expect("mkdir");
    std::fs::write(payload.join("a.txt"), "alpha").expect("write");
    std::fs::write(payload.join("nested/b.txt"), "beta").expect("write");

    let archive = dir.path().join("bundle.zip");
    tdx_archive::create(
        std::slice::from_ref(&payload),
        &archive,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("create zip");
    let entries = tdx_archive::list(&archive).expect("list");
    assert!(entries.iter().any(|entry| entry.name.ends_with("a.txt")));
    assert!(entries
        .iter()
        .any(|entry| entry.name.ends_with("nested/b.txt")));

    let target = dir.path().join("extracted");
    tdx_archive::extract(
        &archive,
        &target,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("extract");
    let extracted = std::fs::read_to_string(target.join("payload/nested/b.txt")).expect("read");
    assert_eq!(extracted, "beta");
}

#[test]
fn zip_slip_entries_are_rejected() {
    let dir = tempfile::tempdir().expect("tempdir");
    let archive_path = dir.path().join("evil.zip");
    {
        let file = std::fs::File::create(&archive_path).expect("create");
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("../evil.txt", zip::write::SimpleFileOptions::default())
            .expect("start entry");
        writer.write_all(b"malicious").expect("write");
        writer.finish().expect("finish");
    }
    let target = dir.path().join("out");
    let error = tdx_archive::extract(
        &archive_path,
        &target,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect_err("zip slip must fail");
    assert!(!target.parent().expect("parent").join("evil.txt").exists());
    assert_eq!(error.category(), tdx_core::ErrorCategory::InvalidInput);
}

#[test]
fn missing_archive_reports_not_found() {
    let dir = tempfile::tempdir().expect("tempdir");
    let error = tdx_archive::list(&dir.path().join("missing.zip")).expect_err("missing");
    assert_eq!(error.category(), tdx_core::ErrorCategory::NotFound);
}

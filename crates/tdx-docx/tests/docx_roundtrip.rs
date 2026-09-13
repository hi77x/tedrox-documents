//! DOCX round-trip tests.

use tdx_core::progress::{CancelToken, ProgressSink};

#[test]
fn markdown_to_docx_roundtrip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let markdown = dir.path().join("notes.md");
    std::fs::write(
        &markdown,
        "# Project notes\n\nSome **bold** text and *italics*.\n\n- first item\n- second item\n\n## Details\n\nFinal paragraph.\n",
    )
    .expect("write md");
    let docx = dir.path().join("notes.docx");
    let options = tdx_docx::DocxOptions {
        title: Some("Project notes".into()),
        author: Some("Tester".into()),
    };
    tdx_docx::create_from_file(
        &markdown,
        &docx,
        &options,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("create docx");
    assert!(docx.exists());

    let text = tdx_docx::extract_text(&docx).expect("extract text");
    assert!(text.contains("Project notes"));
    assert!(text.contains("first item"));
    assert!(text.contains("Final paragraph."));

    let markdown_back = tdx_docx::extract_markdown(&docx).expect("extract markdown");
    assert!(markdown_back.starts_with("# Project notes"));

    let info = tdx_docx::inspect(&docx).expect("inspect");
    assert!(info.words > 5);
    assert!(info.headings >= 2);
    assert_eq!(info.title.as_deref(), Some("Project notes"));
}

#[test]
fn plain_text_creates_docx() {
    let dir = tempfile::tempdir().expect("tempdir");
    let text_file = dir.path().join("plain.txt");
    std::fs::write(&text_file, "Line one\nLine two\nLine three\n").expect("write");
    let docx = dir.path().join("plain.docx");
    tdx_docx::create_from_file(
        &text_file,
        &docx,
        &tdx_docx::DocxOptions::default(),
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("create");
    let text = tdx_docx::extract_text(&docx).expect("extract");
    assert!(text.contains("Line one"));
    assert!(text.contains("Line three"));
}

#[test]
fn unicode_paragraphs_survive() {
    let dir = tempfile::tempdir().expect("tempdir");
    let docx = dir.path().join("ru.docx");
    tdx_docx::from_markdown(
        "# Отчёт\n\nТекст на русском языке.\n",
        &docx,
        &tdx_docx::DocxOptions::default(),
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("create");
    let text = tdx_docx::extract_text(&docx).expect("extract");
    assert!(text.contains("Отчёт"));
    assert!(text.contains("русском"));
}

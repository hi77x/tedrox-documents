//! Conversion center tests.

use tdx_core::progress::{CancelToken, ProgressSink};

#[test]
fn markdown_to_pdf_produces_valid_pdf() {
    let dir = tempfile::tempdir().expect("tempdir");
    let markdown = dir.path().join("report.md");
    std::fs::write(
        &markdown,
        "# Quarterly report\n\nRevenue grew by 12%.\n\n## Highlights\n\n- New customers\n- Lower costs\n",
    )
    .expect("write");
    let pdf = dir.path().join("report.pdf");
    tdx_convert::markdown_to_pdf(
        &markdown,
        &pdf,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("md to pdf");
    let info = tdx_pdf::inspect(&pdf).expect("inspect pdf");
    assert!(info.page_count >= 1);
    assert!(info.file_size > 500);
}

#[test]
fn unicode_markdown_to_pdf_uses_system_font() {
    let dir = tempfile::tempdir().expect("tempdir");
    let markdown = dir.path().join("ru.md");
    std::fs::write(&markdown, "# Отчёт\n\nТекст на русском языке.\n").expect("write");
    let pdf = dir.path().join("ru.pdf");
    tdx_convert::markdown_to_pdf(
        &markdown,
        &pdf,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("md to pdf");
    assert!(pdf.exists());
}

#[test]
fn html_to_pdf_and_txt() {
    let dir = tempfile::tempdir().expect("tempdir");
    let html = dir.path().join("page.html");
    std::fs::write(
        &html,
        "<html><body><h1>Title</h1><p>Body text</p><ul><li>Item</li></ul></body></html>",
    )
    .expect("write");
    let pdf = dir.path().join("page.pdf");
    tdx_convert::html_to_pdf(&html, &pdf, &ProgressSink::default(), &CancelToken::new())
        .expect("html to pdf");
    assert!(tdx_pdf::inspect(&pdf).expect("inspect").page_count >= 1);

    let txt = dir.path().join("page.txt");
    let result =
        tdx_convert::convert_file(&html, &txt, &ProgressSink::default(), &CancelToken::new())
            .expect("html to txt");
    assert!(result.outputs[0].path.exists());
    let content = std::fs::read_to_string(&txt).expect("read");
    assert!(content.contains("Body text"));
}

#[test]
fn docx_to_pdf_and_html() {
    let dir = tempfile::tempdir().expect("tempdir");
    let docx = dir.path().join("doc.docx");
    tdx_docx::from_markdown(
        "# Heading\n\nParagraph with content.\n",
        &docx,
        &tdx_docx::DocxOptions::default(),
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("create docx");

    let pdf = dir.path().join("doc.pdf");
    tdx_convert::docx_to_pdf(&docx, &pdf, &ProgressSink::default(), &CancelToken::new())
        .expect("docx to pdf");
    assert!(tdx_pdf::inspect(&pdf).expect("inspect").page_count >= 1);

    let html = dir.path().join("doc.html");
    tdx_convert::convert_file(&docx, &html, &ProgressSink::default(), &CancelToken::new())
        .expect("docx to html");
    let content = std::fs::read_to_string(&html).expect("read");
    assert!(content.contains("<h1>Heading</h1>"));
}

#[test]
fn unsupported_conversion_is_honest() {
    let dir = tempfile::tempdir().expect("tempdir");
    let source = dir.path().join("source.png");
    let image = image::RgbImage::from_pixel(8, 8, image::Rgb([1, 2, 3]));
    image.save(&source).expect("save");
    let error = tdx_convert::convert_file(
        &source,
        &dir.path().join("out.xyz"),
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect_err("unsupported");
    assert_eq!(error.category(), tdx_core::ErrorCategory::Unsupported);
}

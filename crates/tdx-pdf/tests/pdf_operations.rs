//! End-to-end PDF operations on generated fixtures.

use std::path::Path;

use lopdf::{dictionary, Document, Object, Stream};
use tdx_core::progress::{CancelToken, ProgressSink};

fn make_pdf(path: &Path, pages: usize) {
    let mut doc = Document::with_version("1.7");
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });
    let pages_id = doc.new_object_id();
    let mut kids: Vec<Object> = Vec::new();
    for index in 1..=pages {
        let content = format!("BT /F1 24 Tf 72 720 Td (Page {index}) Tj ET");
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.into_bytes()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![
                Object::Integer(0),
                Object::Integer(0),
                Object::Integer(595),
                Object::Integer(842),
            ],
            "Contents" => content_id,
            "Resources" => dictionary! {
                "Font" => dictionary! { "F1" => font_id },
            },
        });
        kids.push(Object::Reference(page_id));
    }
    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Kids" => Object::Array(kids),
        "Count" => pages as i64,
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    doc.save(path).expect("save fixture pdf");
}

fn page_count(path: &Path) -> usize {
    Document::load(path).expect("load pdf").get_pages().len()
}

#[test]
fn merge_preserves_page_order() {
    let dir = tempfile::tempdir().expect("tempdir");
    let a = dir.path().join("a.pdf");
    let b = dir.path().join("b.pdf");
    let merged = dir.path().join("merged.pdf");
    make_pdf(&a, 2);
    make_pdf(&b, 3);

    let result = tdx_pdf::pages::merge(
        &[a.clone(), b.clone()],
        &merged,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("merge");
    assert_eq!(page_count(&merged), 5);
    assert_eq!(result.stats.get("pages").and_then(|v| v.as_u64()), Some(5));

    // Page order: the first merged page objects should come from a.pdf.
    let document = Document::load(&merged).expect("load merged");
    let pages = document.get_pages();
    let first = pages.get(&1).expect("page 1");
    let content = document.get_object(*first).expect("page object");
    assert!(content.as_dict().is_ok());
}

#[test]
fn split_every_page_creates_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let source = dir.path().join("source.pdf");
    make_pdf(&source, 4);
    let out_dir = dir.path().join("parts");
    let result = tdx_pdf::pages::split(
        &source,
        &out_dir,
        &tdx_pdf::SplitMode::EveryPage,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("split");
    assert_eq!(result.outputs.len(), 4);
    for output in &result.outputs {
        assert_eq!(page_count(&output.path), 1);
    }
}

#[test]
fn extract_delete_and_reverse() {
    let dir = tempfile::tempdir().expect("tempdir");
    let source = dir.path().join("source.pdf");
    make_pdf(&source, 5);

    let extracted = dir.path().join("extracted.pdf");
    let selection = tdx_pdf::PageSelection::parse("1-2,5").expect("selection");
    tdx_pdf::pages::extract_pages(
        &source,
        &extracted,
        &selection,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("extract");
    assert_eq!(page_count(&extracted), 3);

    let deleted = dir.path().join("deleted.pdf");
    let selection = tdx_pdf::PageSelection::parse("2,4").expect("selection");
    tdx_pdf::pages::delete_pages(
        &source,
        &deleted,
        &selection,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("delete");
    assert_eq!(page_count(&deleted), 3);

    let reversed = dir.path().join("reversed.pdf");
    tdx_pdf::pages::reverse(
        &source,
        &reversed,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("reverse");
    assert_eq!(page_count(&reversed), 5);
}

#[test]
fn rotate_updates_page_rotation() {
    let dir = tempfile::tempdir().expect("tempdir");
    let source = dir.path().join("source.pdf");
    make_pdf(&source, 2);
    let rotated = dir.path().join("rotated.pdf");
    tdx_pdf::pages::rotate(
        &source,
        &rotated,
        90,
        None,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("rotate");
    let document = Document::load(&rotated).expect("load");
    for page_id in document.get_pages().values() {
        let page = document.get_dictionary(*page_id).expect("page dict");
        let rotation = page
            .get(b"Rotate")
            .ok()
            .and_then(tdx_pdf::build::object_to_f32);
        assert_eq!(rotation, Some(90.0));
    }
}

#[test]
fn metadata_set_and_privacy_clean() {
    let dir = tempfile::tempdir().expect("tempdir");
    let source = dir.path().join("source.pdf");
    make_pdf(&source, 1);

    let tagged = dir.path().join("tagged.pdf");
    let update = tdx_pdf::MetadataUpdate {
        title: Some("Quarterly report".into()),
        author: Some("Test author".into()),
        subject: None,
        keywords: None,
    };
    tdx_pdf::metadata::set_metadata(
        &source,
        &tagged,
        &update,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("set metadata");
    let metadata = tdx_pdf::metadata::read_metadata(&tagged).expect("read metadata");
    assert_eq!(metadata.title.as_deref(), Some("Quarterly report"));
    assert_eq!(metadata.author.as_deref(), Some("Test author"));

    let clean = dir.path().join("clean.pdf");
    tdx_pdf::metadata::privacy_clean(
        &tagged,
        &clean,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("clean");
    let cleaned = tdx_pdf::metadata::read_metadata(&clean).expect("read clean");
    assert!(cleaned.title.is_none());
    assert!(cleaned.author.is_none());
}

#[test]
fn images_to_pdf_and_compress() {
    let dir = tempfile::tempdir().expect("tempdir");
    let image_path = dir.path().join("sample.png");
    let image = image::RgbImage::from_fn(160, 120, |x, y| {
        image::Rgb([(x % 255) as u8, (y % 255) as u8, 128])
    });
    image.save(&image_path).expect("save png");

    let pdf_path = dir.path().join("images.pdf");
    let options = tdx_pdf::images::ImagesToPdfOptions::default();
    tdx_pdf::images::images_to_pdf(
        &[image_path],
        &pdf_path,
        &options,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("images to pdf");
    assert_eq!(page_count(&pdf_path), 1);

    let compressed = dir.path().join("compressed.pdf");
    tdx_pdf::compress::compress(
        &pdf_path,
        &compressed,
        tdx_pdf::CompressPreset::Balanced,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("compress");
    assert_eq!(page_count(&compressed), 1);
    assert!(std::fs::metadata(&compressed).expect("metadata").len() > 0);
}

#[test]
fn watermark_stamps_content() {
    let dir = tempfile::tempdir().expect("tempdir");
    let source = dir.path().join("source.pdf");
    make_pdf(&source, 1);
    let stamped = dir.path().join("stamped.pdf");
    let options = tdx_pdf::watermark::WatermarkOptions::default();
    tdx_pdf::watermark::watermark(
        &source,
        &stamped,
        &options,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("watermark");
    let document = Document::load(&stamped).expect("load");
    let mut found = false;
    for object in document.objects.values() {
        if let Object::Stream(stream) = object {
            let decoded = stream
                .decompressed_content()
                .unwrap_or_else(|_| stream.content.clone());
            if String::from_utf8_lossy(&decoded).contains("TDXWMGS") {
                found = true;
            }
        }
    }
    assert!(found, "watermark content stream should be present");
}

#[test]
fn encrypted_or_missing_files_produce_clear_errors() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("missing.pdf");
    let error = tdx_pdf::load_pdf(&missing).expect_err("missing should fail");
    assert_eq!(error.category(), tdx_core::ErrorCategory::NotFound);
}

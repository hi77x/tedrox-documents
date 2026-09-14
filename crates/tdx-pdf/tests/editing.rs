//! Interactive editing operations: page plans and annotations.

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

fn load(path: &Path) -> Document {
    Document::load(path).expect("load pdf")
}

fn rotation_of(document: &Document, page: u32) -> i64 {
    let pages = document.get_pages();
    let page_id = pages.get(&page).copied().expect("page exists");
    document
        .get_object(page_id)
        .and_then(Object::as_dict)
        .ok()
        .and_then(|dict| dict.get(b"Rotate").ok())
        .and_then(|value| value.as_i64().ok())
        .unwrap_or(0)
}

#[test]
fn page_plan_reorders_deletes_duplicates_and_rotates() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input = dir.path().join("input.pdf");
    let output = dir.path().join("output.pdf");
    make_pdf(&input, 3);

    let plan = vec![
        tdx_pdf::organize::PagePlanEntry {
            page: 3,
            rotate: 90,
        },
        tdx_pdf::organize::PagePlanEntry { page: 1, rotate: 0 },
        tdx_pdf::organize::PagePlanEntry {
            page: 3,
            rotate: 180,
        },
        tdx_pdf::organize::PagePlanEntry {
            page: 2,
            rotate: -90,
        },
    ];
    let result = tdx_pdf::organize::apply_plan(
        &input,
        &output,
        &plan,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("apply plan");

    assert_eq!(
        result.stats.get("pages_out").and_then(|v| v.as_u64()),
        Some(4)
    );
    let document = load(&output);
    assert_eq!(document.get_pages().len(), 4);
    assert_eq!(rotation_of(&document, 1), 90);
    assert_eq!(rotation_of(&document, 2), 0);
    assert_eq!(rotation_of(&document, 3), 180);
    assert_eq!(rotation_of(&document, 4), 270);
}

#[test]
fn page_plan_rejects_out_of_range_pages() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input = dir.path().join("input.pdf");
    let output = dir.path().join("output.pdf");
    make_pdf(&input, 2);

    let plan = vec![tdx_pdf::organize::PagePlanEntry { page: 9, rotate: 0 }];
    let error = tdx_pdf::organize::apply_plan(
        &input,
        &output,
        &plan,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect_err("out of range page must fail");
    assert!(error.to_string().contains("outside this document"));
}

#[test]
fn annotations_round_trip_through_the_document() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input = dir.path().join("input.pdf");
    let output = dir.path().join("annotated.pdf");
    make_pdf(&input, 2);

    let annotations = vec![
        tdx_pdf::annotate::AnnotationInput {
            page: 1,
            rect: [72.0, 700.0, 300.0, 720.0],
            kind: tdx_pdf::annotate::AnnotationKind::Highlight {
                quads: vec![[72.0, 700.0, 300.0, 720.0]],
            },
            color: Some([1.0, 0.9, 0.2]),
            opacity: Some(0.5),
            author: Some("TEDROX".into()),
            contents: Some("Highlighted".into()),
        },
        tdx_pdf::annotate::AnnotationInput {
            page: 2,
            rect: [80.0, 640.0, 240.0, 700.0],
            kind: tdx_pdf::annotate::AnnotationKind::FreeText {
                text: "Reviewed".into(),
                font_size: 14.0,
            },
            color: Some([0.1, 0.3, 0.8]),
            opacity: None,
            author: None,
            contents: None,
        },
        tdx_pdf::annotate::AnnotationInput {
            page: 2,
            rect: [90.0, 500.0, 300.0, 560.0],
            kind: tdx_pdf::annotate::AnnotationKind::Ink {
                strokes: vec![vec![[90.0, 500.0], [140.0, 540.0], [200.0, 510.0]]],
            },
            color: None,
            opacity: None,
            author: None,
            contents: None,
        },
        tdx_pdf::annotate::AnnotationInput {
            page: 2,
            rect: [100.0, 400.0, 260.0, 460.0],
            kind: tdx_pdf::annotate::AnnotationKind::Note {
                text: "Check this".into(),
            },
            color: None,
            opacity: None,
            author: Some("Reviewer".into()),
            contents: None,
        },
    ];

    let result = tdx_pdf::annotate::add_annotations(
        &input,
        &output,
        &annotations,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("annotate");
    assert_eq!(
        result.stats.get("annotations").and_then(|v| v.as_u64()),
        Some(4)
    );
    assert!(result.warnings.is_empty());

    let document = load(&output);
    assert_eq!(document.get_pages().len(), 2);
    let summaries = tdx_pdf::annotate::list_annotations(&output).expect("list annotations");
    assert_eq!(summaries.len(), 4);
    assert_eq!(summaries.iter().filter(|item| item.page == 2).count(), 3);
    assert!(summaries
        .iter()
        .any(|item| item.subtype == "Highlight" && item.author.as_deref() == Some("TEDROX")));
    assert!(summaries
        .iter()
        .any(|item| item.subtype == "FreeText" && item.contents.as_deref() == Some("Reviewed")));
}

#[test]
fn merge_plan_interleaves_documents() {
    let dir = tempfile::tempdir().expect("tempdir");
    let a = dir.path().join("a.pdf");
    let b = dir.path().join("b.pdf");
    let output = dir.path().join("interleaved.pdf");
    make_pdf(&a, 2);
    make_pdf(&b, 2);

    let plan = vec![
        tdx_pdf::organize::InsertPlanEntry {
            document: 0,
            page: 1,
            rotate: 0,
        },
        tdx_pdf::organize::InsertPlanEntry {
            document: 1,
            page: 1,
            rotate: 0,
        },
        tdx_pdf::organize::InsertPlanEntry {
            document: 0,
            page: 2,
            rotate: 0,
        },
        tdx_pdf::organize::InsertPlanEntry {
            document: 1,
            page: 2,
            rotate: 0,
        },
        tdx_pdf::organize::InsertPlanEntry {
            document: 1,
            page: 2,
            rotate: 0,
        },
    ];
    tdx_pdf::organize::merge_plan(
        &[a, b],
        &output,
        &plan,
        &ProgressSink::default(),
        &CancelToken::new(),
    )
    .expect("merge plan");
    assert_eq!(load(&output).get_pages().len(), 5);
}

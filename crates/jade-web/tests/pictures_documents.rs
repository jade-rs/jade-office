//! 挿入 → 画像 on the browser build's Word document, as the native app does
//! it too: a picture in line at the caret at its own size, moved, given a
//! wrap, each one undo step, and saved into the package.

mod common;
use common::{contract, typed};
use jade_docx_editor::word_document::{FloatingObjectKind, FloatingObjectTarget};
use jade_web::editing::WordSession;
use jade_web::word::picture::{PlacedObject, WordPicture, Wrap};

/// A 2×1 PNG.
fn png() -> Vec<u8> {
    let mut bytes = Vec::new();
    image::RgbaImage::from_raw(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 255])
        .unwrap()
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    bytes
}

fn image_runs(document: &jade_web::editing::EditableDocument, paragraph: usize) -> Vec<usize> {
    document
        .word_document()
        .paragraph(paragraph)
        .map(|paragraph| {
            paragraph
                .runs
                .iter()
                .enumerate()
                .filter(|(_, run)| run.image.is_some())
                .map(|(index, _)| index)
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_picture_goes_in_at_the_caret_and_wraps() {
    let mut document = contract();
    let paragraph = typed(&mut document, "here ");
    document.place_caret(paragraph, "here".len());
    document.insert_picture(png()).unwrap();
    let runs = image_runs(&document, paragraph);
    assert_eq!(runs.len(), 1);
    let saved = document.export_docx().unwrap();
    let package = jade_opc::OpcPackage::open(&saved).unwrap();
    assert!(package
        .part_names()
        .iter()
        .any(|name| name.starts_with("word/media/")));

    let target = FloatingObjectTarget {
        paragraph_index: paragraph,
        run_index: runs[0],
        kind: FloatingObjectKind::Image,
    };
    let object = document
        .object_region(target)
        .expect("the picture is on a page");
    assert!(object.is_picture());
    assert_eq!((object.width.round(), object.height.round()), (2.0, 1.0));
    document.set_picture_wrap(object, Wrap::Square).unwrap();
    let image = document.word_document().paragraph(paragraph).unwrap().runs[runs[0]]
        .image
        .clone()
        .unwrap();
    assert_eq!(
        image.layout.wrap,
        jade_docx_editor::document::WrapMode::Square
    );

    let moved: PlacedObject = document.object_region(target).unwrap();
    document
        .place_object(moved, (moved.x + 40.0, moved.y + 20.0, 20.0, 10.0))
        .unwrap();
    let placed = document.object_region(target).unwrap();
    assert_eq!((placed.width.round(), placed.height.round()), (20.0, 10.0));

    document.undo().unwrap();
    document.undo().unwrap();
    document.undo().unwrap();
    assert!(image_runs(&document, paragraph).is_empty());
}

//! 挿入 › オブジェクト on the browser build's Word document: a file goes in
//! as an OLE package shown as its icon (or a Word document as its first
//! page), is drawn, survives a save, and hands back the file it holds.

mod common;
use common::{contract, open_bytes};
use jade_web::editing::WordSession;
use jade_web::word::ole_objects::OBJECT_ICON_SIZE;

fn object_run(document: &jade_web::editing::EditableDocument, paragraph: usize) -> usize {
    document
        .word_document()
        .paragraph(paragraph)
        .unwrap()
        .runs
        .iter()
        .position(|run| {
            run.image
                .as_ref()
                .is_some_and(|image| image.ole_program_id.is_some())
        })
        .expect("the object's run")
}

#[test]
fn a_file_is_embedded_drawn_saved_and_handed_back() {
    let mut document = contract();
    let paragraph = (0..document.paragraph_count())
        .find(|&index| document.cell_id_for_paragraph(index).is_none())
        .unwrap();
    document.place_caret(paragraph, 0);
    let before = document.page(0).unwrap().rgba_premultiplied();
    document
        .insert_object_file("見積書.csv", b"a,b\n1,2\n".to_vec(), true)
        .unwrap();
    assert!(
        document.page(0).unwrap().rgba_premultiplied() != before,
        "the icon is drawn"
    );
    let run = object_run(&document, paragraph);
    let image = document.word_document().paragraph(paragraph).unwrap().runs[run]
        .image
        .clone()
        .unwrap();
    assert_eq!(image.width_px, Some(OBJECT_ICON_SIZE.0));
    let file = document.object_file(paragraph, run).unwrap();
    assert_eq!(
        (file.name.as_str(), file.bytes.as_slice()),
        ("見積書.csv", &b"a,b\n1,2\n"[..])
    );

    let saved = document.export_docx().unwrap();
    let reopened = open_bytes(&saved);
    let run = object_run(&reopened, paragraph);
    assert_eq!(
        reopened.object_file(paragraph, run).unwrap().name,
        "見積書.csv"
    );
}

#[test]
fn a_word_document_shows_its_first_page() {
    let mut document = contract();
    let bytes = std::fs::read(common::core().join("contract.docx")).unwrap();
    document.place_caret(0, 0);
    document
        .insert_object_file("contract.docx", bytes, false)
        .unwrap();
    let run = object_run(&document, 0);
    let image = document.word_document().paragraph(0).unwrap().runs[run]
        .image
        .clone()
        .unwrap();
    assert!(
        image.height_px.unwrap() > image.width_px.unwrap(),
        "a portrait page"
    );
}

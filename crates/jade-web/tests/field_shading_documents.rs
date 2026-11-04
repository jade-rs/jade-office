//! Word 環境設定 › 表示 › フィールドの網かけ: a field's text is drawn on
//! Word's grey always, while the caret is in it, or never — on the page
//! only, never in the saved document.

mod common;

use jade_office_model::word::field_shading::{shading_for, FieldShadingMode};
use jade_office_model::word::fields::{FieldHost, WordFields};
use jade_web::editing::EditableDocument;
use jade_web::word::new::{blank_document, Blank};

fn page_pixels(document: &EditableDocument) -> Vec<u8> {
    document.page(0).expect("first page").rgba_premultiplied()
}

fn shade(document: &mut EditableDocument, mode: FieldShadingMode) -> bool {
    let shading = shading_for(&*document, mode);
    document.set_field_shading(shading).unwrap()
}

#[test]
fn field_shading_follows_the_setting_and_the_caret() {
    let mut document = common::open_bytes(&blank_document(Blank::English));
    document.place_caret(0, 0);
    document.insert_text("Author: ").unwrap();
    document.place_caret(0, "Author: ".len());
    document
        .insert_field_code("QUOTE \"Kanna\"", &FieldHost::default())
        .unwrap();
    let plain = page_pixels(&document);

    // 常に表示する: the field is on grey wherever the caret is.
    document.place_caret(0, 0);
    assert!(shade(&mut document, FieldShadingMode::Always));
    let shaded = page_pixels(&document);
    assert!(plain != shaded, "the field is drawn on grey");
    assert!(!shade(&mut document, FieldShadingMode::Always), "unchanged");

    // 選択時に表示する: only while the caret is in the field.
    assert!(shade(&mut document, FieldShadingMode::WhenSelected));
    assert!(page_pixels(&document) == plain, "the caret is outside it");
    document.place_caret(0, "Author: ".len());
    assert!(shade(&mut document, FieldShadingMode::WhenSelected));
    assert!(page_pixels(&document) == shaded, "the caret is in it");

    // 表示しない.
    assert!(shade(&mut document, FieldShadingMode::Never));
    assert!(page_pixels(&document) == plain);

    // A view setting: nothing of it is saved.
    shade(&mut document, FieldShadingMode::Always);
    let saved = common::open_bytes(&document.export_docx().unwrap());
    assert!(page_pixels(&saved) == plain);
}

/// トリムマーク and 文字列の範囲: the corner marks go and the dotted line
/// round the text area comes, on the page only.
#[test]
fn crop_marks_and_text_boundaries_are_view_settings() {
    let mut document = common::open_bytes(&blank_document(Blank::English));
    assert_eq!(document.page_guides(), (true, false), "Word's defaults");
    let marked = page_pixels(&document);
    assert!(document.set_page_guides(false, false).unwrap());
    let bare = page_pixels(&document);
    assert!(marked != bare, "the crop marks are drawn");
    assert!(document.set_page_guides(false, true).unwrap());
    let bounded = page_pixels(&document);
    assert!(
        bounded != bare && bounded != marked,
        "the text area is outlined"
    );
    assert!(!document.set_page_guides(false, true).unwrap(), "unchanged");
    let saved = common::open_bytes(&document.export_docx().unwrap());
    assert_eq!(saved.page_guides(), (true, false));
    assert!(page_pixels(&saved) == marked);
}

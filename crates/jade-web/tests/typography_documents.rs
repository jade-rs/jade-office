//! Word 2010 typography from the フォント dialog's 詳細設定 tab — ligatures,
//! figure styles — changes what the page draws, not just what is saved:
//! the text is shaped with the font's OpenType features, so `ff` becomes the
//! font's ligature glyph.

mod common;

use jade_office_model::word::font::WordCharacters;

fn page_pixels(document: &jade_web::editing::EditableDocument) -> Vec<u8> {
    document.page(0).expect("first page").rgba_premultiplied()
}

#[test]
fn ligatures_from_the_font_dialog_are_drawn() {
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "office affirms ");
    document.select((paragraph, 0), (paragraph, 14)).unwrap();
    let before = page_pixels(&document);

    let mut dialog = document.font_dialog();
    assert_eq!(dialog.format.ligatures.as_deref(), Some("none"));
    dialog.format.ligatures = Some("standard".to_string());
    document.apply_font_dialog(&dialog).unwrap();

    // The dialog reads the setting back.
    assert_eq!(
        document.font_dialog().format.ligatures.as_deref(),
        Some("standard")
    );
    let joined = page_pixels(&document);
    assert!(before != joined, "the ff ligatures are drawn");

    document.undo().unwrap();
    assert!(
        before == page_pixels(&document),
        "undo draws the letters again"
    );
}

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

/// レイアウト ▸ ハイフネーション ▸ 自動 breaks long English words at line
/// ends with a hyphen the text does not have; なし, or the paragraph's
/// 段落の自動ハイフネーションを行わない, takes them away again.
#[test]
fn automatic_hyphenation_redraws_the_page() {
    use jade_office_model::word::hyphenation::WordHyphenation;
    use jade_office_model::word::paragraph_options::{ParagraphSwitch, WordParagraphOptions};
    use jade_web::word::new::{blank_document, Blank};

    let mut document = common::open_bytes(&blank_document(Blank::English));
    document.place_caret(0, 0);
    let text = "Automatic hyphenation divides considerable international responsibilities \
        between representatives of organizations, documentation departments and \
        administrative committees, characteristically accommodating extraordinary \
        circumstances whenever practicable, notwithstanding unpredictable interruptions.";
    document.insert_text(text).unwrap();
    let plain = page_pixels(&document);

    document.set_auto_hyphenation(true).unwrap();
    let hyphenated = page_pixels(&document);
    assert!(plain != hyphenated, "the lines break inside words");
    assert_eq!(document.paragraph_text(0).as_deref(), Some(text));

    document.select((0, 0), (0, 1)).unwrap();
    document
        .set_paragraph_switch(ParagraphSwitch::DontHyphenate, true)
        .unwrap();
    assert!(page_pixels(&document) == plain, "the paragraph opts out");
    document.undo().unwrap();
    assert!(page_pixels(&document) == hyphenated);

    document.set_auto_hyphenation(false).unwrap();
    assert!(page_pixels(&document) == plain);
}

/// An ADVANCE field from the フィールド dialog moves the text after it, and
/// still does once the document is saved and opened again.
#[test]
fn an_advance_field_moves_the_text_after_it() {
    use jade_office_model::word::fields::{FieldHost, WordFields};
    use jade_web::word::new::{blank_document, Blank};

    let mut document = common::open_bytes(&blank_document(Blank::English));
    document.place_caret(0, 0);
    document.insert_text("Advance this").unwrap();
    let plain = page_pixels(&document);

    document.place_caret(0, "Advance".len());
    document
        .insert_field_code("ADVANCE \\r 40 \\d 6", &FieldHost::default())
        .unwrap();
    assert_eq!(document.paragraph_text(0).as_deref(), Some("Advance this"));
    let moved = page_pixels(&document);
    assert!(plain != moved, "the words after the field move");

    let reopened = common::open_bytes(&document.export_docx().unwrap());
    assert!(
        page_pixels(&reopened) == moved,
        "the saved field moves them too"
    );
}

//! デザイン › 透かし on the browser build's Word document — the commands the
//! native app runs too: a gallery entry drawn behind the text, the
//! ユーザー設定の透かし dialog's look (色, 半透明, レイアウト, サイズ) changing
//! what is drawn and read back after saving, and 透かしの削除.

mod common;
use common::{contract, open_bytes};
use jade_web::word::watermark::{WordWatermark, WATERMARK_PRESETS};

fn page_pixels(document: &jade_web::editing::EditableDocument) -> Vec<u8> {
    document.page(0).expect("first page").rgba_premultiplied()
}

/// The same picture, but for the odd antialiased pixel a repaint in place
/// leaves a shade off at the page's edge.
fn alike(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len()
        && a.chunks_exact(4)
            .zip(b.chunks_exact(4))
            .filter(|(a, b)| a != b)
            .count()
            < 500
}

#[test]
fn a_watermark_is_drawn_styled_saved_and_removed() {
    let mut document = contract();
    let plain = page_pixels(&document);

    let diagonal = WATERMARK_PRESETS
        .iter()
        .position(|preset| preset.text == "社外秘" && !preset.horizontal)
        .unwrap();
    document.insert_watermark_preset(diagonal).unwrap();
    assert_eq!(document.watermark_preset(), Some(diagonal));
    let silver = page_pixels(&document);
    assert!(silver != plain, "the watermark is drawn behind the text");

    // ユーザー設定の透かし: red, opaque, horizontal, 72 pt.
    let mut dialog = document.watermark_dialog();
    assert!(dialog.text_on);
    assert_eq!(dialog.text, "社外秘");
    dialog.color = "#FF0000".into();
    dialog.semitransparent = false;
    dialog.horizontal = true;
    dialog.set_size_text("72").unwrap();
    document.apply_watermark_dialog(&dialog).unwrap();
    let red = page_pixels(&document);
    assert!(red != silver, "the new look is drawn");
    assert!(
        red.chunks_exact(4)
            .any(|pixel| pixel[0] > 200 && pixel[1] < 60 && pixel[2] < 60),
        "opaque red text is on the page"
    );

    // Saved and opened again, it is drawn the same and the dialog reads it.
    let reopened = open_bytes(&document.export_docx().unwrap());
    assert!(
        alike(&page_pixels(&reopened), &red),
        "the saved watermark is drawn the same"
    );
    let back = reopened.watermark_dialog();
    assert_eq!(back.color, "#FF0000");
    assert!(!back.semitransparent && back.horizontal);
    assert_eq!(back.size, Some(72.0));

    document.undo().unwrap();
    assert!(
        alike(&page_pixels(&document), &silver),
        "undo draws the gallery look again"
    );

    document.remove_watermark().unwrap();
    assert!(document.watermark().is_none());
    assert!(
        alike(&page_pixels(&document), &plain),
        "透かしの削除 leaves the page plain"
    );
}

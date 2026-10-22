//! What an edit repaints — the edited page's band first, then on down — is
//! what opening the saved document draws: after a paste with italics and
//! after a picture goes in, and after typing above or below either, every
//! page's pixels match a fresh render of the same document.

mod common;
use common::contract;
use jade_web::editing::EditableDocument;
use jade_web::word::clipboard::{PasteMode, RichClip, WordClipboard};
use jade_web::word::picture::WordPicture;

/// A 40×30 grey PNG.
fn png() -> Vec<u8> {
    let mut bytes = Vec::new();
    image::RgbaImage::from_raw(40, 30, vec![200u8; 40 * 30 * 4])
        .unwrap()
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    bytes
}

/// Each page of `document` as repainted, against a fresh render of it saved.
fn assert_repaint_matches_a_fresh_render(label: &str, document: &mut EditableDocument) {
    let fresh = common::open_bytes(&document.export_docx().unwrap());
    assert_eq!(document.page_count(), fresh.page_count(), "{label}");
    for page in 0..document.page_count() {
        let repainted = document.page(page).unwrap().rgba_premultiplied();
        let drawn = fresh.page(page).unwrap().rgba_premultiplied();
        let differing = repainted
            .chunks(4)
            .zip(drawn.chunks(4))
            .filter(|(a, b)| a != b)
            .count();
        assert_eq!(
            differing, 0,
            "{label}: page {page} differs in {differing} px"
        );
    }
}

/// A body paragraph on the first page with some text, past the title.
fn body_paragraph(document: &EditableDocument) -> usize {
    (8..40)
        .find(|&index| {
            document.is_body_paragraph(index)
                && document
                    .paragraph_text(index)
                    .is_some_and(|text| text.chars().count() > 5)
        })
        .unwrap()
}

#[test]
fn a_paste_with_italics_repaints_as_a_fresh_render_draws() {
    let clip = RichClip::from_html(
        "<p><i>Italic words</i> and <b>bold</b></p><p>Second <i>slanted</i> line</p>",
    );
    for typed_at in [None, Some(-2i64), Some(3)] {
        let mut document = contract();
        let paragraph = body_paragraph(&document);
        document.place_caret(paragraph, 0);
        document.paste_as(&clip, PasteMode::KeepSource).unwrap();
        if let Some(delta) = typed_at {
            document.place_caret((paragraph as i64 + delta) as usize, 0);
            document.insert_text("y").unwrap();
        }
        assert_repaint_matches_a_fresh_render(&format!("paste, then {typed_at:?}"), &mut document);
    }
}

#[test]
fn a_picture_repaints_as_a_fresh_render_draws() {
    for typed_at in [None, Some(-2i64), Some(3)] {
        let mut document = contract();
        let paragraph = body_paragraph(&document);
        document.place_caret(paragraph, 0);
        document.insert_picture(png()).unwrap();
        if let Some(delta) = typed_at {
            document.place_caret((paragraph as i64 + delta) as usize, 0);
            document.insert_text("y").unwrap();
        }
        assert_repaint_matches_a_fresh_render(
            &format!("picture, then {typed_at:?}"),
            &mut document,
        );
    }
}

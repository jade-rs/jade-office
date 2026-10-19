//! BAHTTEXT, typed into a cell, writes the amount in Thai words as Excel
//! does — over a cell's number too, as on an invoice.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::sheet::{CellRef, Workbook};
use jade_office_model::tiny::TinySkiaBackend;

fn book() -> Workbook<TinySkiaBackend> {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let directory = core.join("fonts/Noto_Sans_JP/static");
    let backend = TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        None,
    ))
    .unwrap();
    let bytes = std::fs::read(core.join("public/fixtures/grid-operations-workbook.xlsx")).unwrap();
    Workbook::open(&bytes, "UTC", backend).unwrap()
}

#[test]
fn an_amount_reads_in_thai_words_and_follows_its_cell() {
    let mut book = book();
    // Microsoft's own example.
    let typed = CellRef::new(8, 10);
    book.set_cell(typed, "=BAHTTEXT(1234)").unwrap();
    assert_eq!(book.display_text(typed), "หนึ่งพันสองร้อยสามสิบสี่บาทถ้วน");

    let amount = CellRef::new(8, 11);
    let words = CellRef::new(9, 11);
    book.set_cell(amount, "1250.75").unwrap();
    book.set_cell(words, "=BAHTTEXT(I12)").unwrap();
    assert_eq!(book.display_text(words), "หนึ่งพันสองร้อยห้าสิบบาทเจ็ดสิบห้าสตางค์");
    book.set_cell(amount, "21").unwrap();
    assert_eq!(book.display_text(words), "ยี่สิบเอ็ดบาทถ้วน");
}

/// Thai is drawn in Thai glyphs, falling back from the Japanese face to the
/// bundled Noto Sans Thai as a browser falls back: after Latin in one run,
/// it measures as Noto Sans Thai's own glyphs, not as the Japanese face's
/// missing-glyph boxes — on both backends.
fn falls_back<B: jade_office_model::Backend>(backend: impl Fn(&[jade_office_model::Face]) -> B) {
    use jade_office_model::{thai_face, THAI_FONT};
    use jade_painter::Painter;

    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let japanese = Arc::new(
        std::fs::read(core.join("fonts/Noto_Sans_JP/static/NotoSansJP-Regular.ttf")).unwrap(),
    );
    let thai = Arc::new(std::fs::read(core.join("fonts").join(THAI_FONT)).unwrap());
    let width = |faces: &[jade_office_model::Face], font: &str, text: &str| {
        let painter = backend(faces).surface(1, 1).unwrap();
        painter.set_font(font);
        painter.measure_text_width(text).ok().unwrap()
    };
    let mut faces = aliased_faces(Arc::clone(&japanese), None);
    let latin = width(&faces, "16px Noto Sans JP", "Total ");
    let boxes = width(&faces, "16px Noto Sans JP", "Total บาท");
    faces.push(thai_face(Arc::clone(&thai)));
    let drawn = width(&faces, "16px Noto Sans JP", "Total บาท");
    let own = width(&[thai_face(thai)], "16px Noto Sans Thai", "บาท");
    assert!(
        (drawn - (latin + own)).abs() < 0.5,
        "{drawn} vs {latin} + {own}"
    );
    assert!((drawn - boxes).abs() > 1.0, "{drawn} vs boxes {boxes}");
}

#[test]
fn thai_falls_back_to_noto_sans_thai() {
    falls_back(|faces| TinySkiaBackend::new(faces).unwrap());
}

#[cfg(feature = "skia")]
#[test]
fn thai_falls_back_to_noto_sans_thai_on_skia() {
    falls_back(|faces| jade_office_model::skia::SkiaBackend::new(faces).unwrap());
}

//! Thai typed into a Word document in the browser build is laid out in Noto
//! Sans Thai's glyphs, as Chromium falls back to a font that has them — not
//! at the Japanese face's missing-glyph widths.

use std::path::PathBuf;

use jade_docx_layout::layout_cache::TextMeasurer;
use jade_web::document::{Document, ShapingMeasurer};
use jade_web::editing::EditableDocument;

fn core() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"))
}

fn font(path: &str) -> Vec<u8> {
    std::fs::read(core().join("fonts").join(path)).unwrap()
}

/// How far the caret moves when `text` is typed at the start of the first
/// paragraph with text.
fn advance(measurer: std::rc::Rc<ShapingMeasurer>, text: &str) -> f64 {
    let regular = font("Noto_Sans_JP/static/NotoSansJP-Regular.ttf");
    let fonts = jade_web::backend(regular, None).unwrap().fonts();
    let bytes = std::fs::read(core().join("contract.docx")).unwrap();
    let mut document = EditableDocument::open(&bytes, 1.0, fonts, measurer).unwrap();
    let paragraph = (0..document.paragraph_count())
        .find(|&index| {
            document
                .paragraph_text(index)
                .is_some_and(|text| !text.trim().is_empty())
        })
        .unwrap();
    let before = document.place_caret(paragraph, 0).unwrap().x;
    document.insert_text(text).unwrap();
    document.caret_rect().unwrap().x - before
}

#[test]
fn thai_is_measured_in_the_thai_face() {
    let regular = font("Noto_Sans_JP/static/NotoSansJP-Regular.ttf");
    let thai = font(jade_office_model::THAI_FONT);
    let with_thai = || jade_web::document::measurer_with(&regular, None, Some(&thai)).unwrap();
    let without = || jade_web::document::measurer(&regular, None).unwrap();
    let drawn = advance(with_thai(), "บาท");
    let boxes = advance(without(), "บาท");
    // The paragraph's alignment and size scale both alike, so their ratio
    // is the Thai glyphs' width over the missing-glyph boxes', at any size.
    let descriptor = "10.50pt 'Noto Sans JP'";
    let expected = with_thai().measure_text("บาท", descriptor).unwrap()
        / without().measure_text("บาท", descriptor).unwrap();
    assert!(
        (drawn / boxes - expected).abs() < 0.01,
        "{drawn} / {boxes} vs {expected}"
    );
    assert!(expected < 0.9, "Thai measures as boxes: {expected}");
}

#[test]
fn word_paints_a_thai_tone_mark_above_its_upper_vowel() {
    let regular = font("Noto_Sans_JP/static/NotoSansJP-Regular.ttf");
    let thai = font(jade_office_model::THAI_FONT);
    let (backend, measurer) = jade_web::bundled_fonts(regular, None, Some(thai), None).unwrap();
    let source = std::fs::read(core().join("contract.docx")).unwrap();
    let top_of_ink = |text| {
        let mut package = jade_opc::OpcPackage::open(&source).unwrap();
        package.replace(
            "word/document.xml",
            format!(
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                    <w:body>
                      <w:p><w:pPr><w:spacing w:line="1200" w:lineRule="exact"/></w:pPr></w:p>
                      <w:p><w:r><w:rPr>
                      <w:rFonts w:ascii="Noto Sans JP" w:hAnsi="Noto Sans JP" w:cs="Noto Sans JP"/>
                      <w:sz w:val="90"/><w:szCs w:val="90"/>
                    </w:rPr><w:t>{text}</w:t></w:r></w:p><w:sectPr/></w:body>
                </w:document>"#
            )
            .into_bytes(),
        );
        let doc = Document::open(
            &package.write().unwrap(),
            1.0,
            backend.fonts(),
            measurer.clone(),
        )
        .unwrap();
        let page = doc.page(0).unwrap();
        page.rgba_premultiplied()
            .chunks_exact(page.width() as usize * 4)
            .position(|row| row.chunks_exact(4).any(|pixel| pixel[0] < 128))
            .unwrap()
    };
    let bare = top_of_ink("ยี");
    let marked = top_of_ink("ยี่");
    assert!(marked + 4 < bare, "marked top {marked}, vowel top {bare}");
}

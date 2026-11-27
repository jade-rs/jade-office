use std::path::Path;

use jade_docx_layout::layout_cache::TextMeasurer;
use jade_web::editing::EditableDocument;

#[test]
fn word_editor_measures_and_renders_hangul_with_the_bundled_fallback() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let regular =
        std::fs::read(root.join("jade-core/fonts/Noto_Sans_JP/static/NotoSansJP-Regular.ttf"))
            .unwrap();
    let korean = std::fs::read(root.join("fonts").join(jade_office_model::KOREAN_FONT)).unwrap();
    let docx = std::fs::read(root.join("jade-core/contract.docx")).unwrap();
    let text = "한글 각 한";
    let render_edit = |korean| {
        let (backend, measurer) =
            jade_web::bundled_fonts(regular.clone(), None, None, korean).unwrap();
        let mut document =
            EditableDocument::open(&docx, 1.0, backend.fonts(), measurer.clone()).unwrap();
        let paragraph = (0..document.paragraph_count())
            .find(|&index| {
                document
                    .paragraph_text(index)
                    .is_some_and(|text| !text.trim().is_empty())
            })
            .unwrap();
        let before = document.place_caret(paragraph, 0).unwrap().x;
        document.insert_text(text).unwrap();
        let advance = document.caret_rect().unwrap().x - before;
        assert!(document
            .paragraph_text(paragraph)
            .unwrap()
            .starts_with(text));
        (
            advance,
            document.page(0).unwrap().rgba_premultiplied(),
            measurer,
        )
    };

    let (advance, pixels, measurer) = render_edit(Some(korean));
    let (boxes_advance, boxes, no_fallback) = render_edit(None);
    let descriptor = "10.50pt 'Noto Sans JP'";
    let width = measurer.measure_text(text, descriptor).unwrap();
    let boxes_width = no_fallback.measure_text(text, descriptor).unwrap();
    assert!(!measurer.uncovered_characters().contains(&'한'));
    assert!(no_fallback.uncovered_characters().contains(&'한'));
    assert!(
        (width - boxes_width).abs() > 1.0,
        "{width} vs {boxes_width}"
    );
    assert!(
        (advance - boxes_advance).abs() > 1.0,
        "{advance} vs {boxes_advance}"
    );
    assert_ne!(
        pixels, boxes,
        "Hangul should paint differently from missing glyphs"
    );
}

//! Arabic shaping at the normal Word rendering and editing entry points.

use std::path::Path;

use jade_docx_layout::line_metrics::{set_word_layout_platform, WordLayoutPlatform};
use jade_painter::Painter;
use jade_tiny_skia_painter::TinySkiaPainter;
use jade_web::document::{full_layout_debug, Document};
use jade_web::editing::EditableDocument;

const ARABIC: &str = "سلام";

fn core() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"))
}

fn fonts() -> (
    jade_web::TinySkiaBackend,
    std::rc::Rc<jade_web::document::ShapingMeasurer>,
) {
    let directory = core().join("fonts");
    let regular =
        std::fs::read(directory.join("Noto_Sans_JP/static/NotoSansJP-Regular.ttf")).unwrap();
    let arabic = std::fs::read(directory.join(jade_office_model::ARABIC_FONT)).unwrap();
    jade_web::bundled_fonts_with_arabic(regular, None, None, None, Some(arabic)).unwrap()
}

fn docx(text: &str, bidi: bool) -> Vec<u8> {
    let donor = std::fs::read(core().join("contract.docx")).unwrap();
    let mut package = jade_opc::OpcPackage::open(&donor).unwrap();
    let (paragraph, run) = if bidi {
        ("<w:pPr><w:bidi/></w:pPr>", "<w:rtl/>")
    } else {
        ("", "")
    };
    let xml = format!(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:body><w:p>{paragraph}<w:r><w:rPr>{run}
              <w:rFonts w:ascii="Noto Sans JP" w:hAnsi="Noto Sans JP" w:cs="Noto Sans JP"/>
              <w:sz w:val="60"/><w:szCs w:val="60"/>
            </w:rPr><w:t>{text}</w:t></w:r></w:p><w:sectPr/></w:body>
        </w:document>"#
    );
    package.replace("word/document.xml", xml.into_bytes());
    package.write().unwrap()
}

#[test]
fn a_windows_word_rtl_paragraph_paints_joined_arabic_and_saves_logical_text() {
    check_rtl_word_paragraph(WordLayoutPlatform::Windows);
}

#[test]
fn a_mac_word_rtl_paragraph_paints_joined_arabic_and_saves_logical_text() {
    check_rtl_word_paragraph(WordLayoutPlatform::Mac);
}

fn check_rtl_word_paragraph(platform: WordLayoutPlatform) {
    set_word_layout_platform(platform);
    let (backend, measurer) = fonts();
    let bytes = docx(ARABIC, true);
    let preview = Document::open(&bytes, 1.0, backend.fonts(), measurer.clone()).unwrap();
    let page = preview.page(0).unwrap();
    let layout = full_layout_debug(&bytes, backend.fonts(), measurer.clone()).unwrap();
    let line = layout
        .body_lines
        .iter()
        .find(|line| line.text == ARABIC)
        .unwrap();
    assert_eq!(line.fragments.len(), 1);
    let fragment = &line.fragments[0];
    let mut positions: Vec<_> = fragment
        .glyphs
        .iter()
        .map(|glyph| {
            (
                glyph.start,
                fragment.x + glyph.x - 2.0 * line.report_x_offset,
            )
        })
        .collect();
    positions.sort_by_key(|&(byte, _)| byte);
    assert!(positions.windows(2).all(|pair| pair[0].1 > pair[1].1));

    let expected = TinySkiaPainter::new(page.width(), page.height(), backend.fonts()).unwrap();
    expected.set_font("40px 'Noto Sans Arabic'");
    expected.set_fill_style_str("#000000");
    assert!(expected
        .try_fill_shaped_text(ARABIC, &positions, fragment.baseline)
        .unwrap());
    let ink = |painter: &TinySkiaPainter| {
        painter
            .rgba_premultiplied()
            .chunks_exact(4)
            .map(|pixel| pixel[0] < 64)
            .collect::<Vec<_>>()
    };
    let actual_ink = ink(&page);
    assert!(actual_ink.iter().any(|&black| black));
    let expected_ink = ink(&expected);
    let bounds = |pixels: &[bool]| {
        let (mut left, mut top, mut right, mut bottom, mut count) =
            (page.width(), page.height(), 0, 0, 0);
        for (i, &black) in pixels.iter().enumerate() {
            if black {
                let x = i as u32 % page.width();
                let y = i as u32 / page.width();
                left = left.min(x);
                top = top.min(y);
                right = right.max(x);
                bottom = bottom.max(y);
                count += 1;
            }
        }
        (left, top, right, bottom, count)
    };
    assert!(
        actual_ink == expected_ink,
        "DOCX ink {:?}, shaped ink {:?}, positions {positions:?}, baseline {}",
        bounds(&actual_ink),
        bounds(&expected_ink),
        fragment.baseline,
    );

    let mut editor =
        EditableDocument::open(&bytes, 1.0, backend.fonts(), measurer.clone()).unwrap();
    assert_eq!(editor.paragraph_text(0).as_deref(), Some(ARABIC));
    assert!(editor.place_caret(0, ARABIC.len()).is_some());
    editor.insert_text(" عالم").unwrap();
    let saved = editor.export_docx().unwrap();
    let reopened = EditableDocument::open(&saved, 1.0, backend.fonts(), measurer).unwrap();
    assert_eq!(reopened.paragraph_text(0).as_deref(), Some("سلام عالم"));
}

#[test]
fn mixed_word_text_places_digits_left_of_the_rtl_word() {
    let (backend, measurer) = fonts();
    let text = "abc عربي 123";
    let layout = full_layout_debug(&docx(text, false), backend.fonts(), measurer).unwrap();
    let line = layout
        .body_lines
        .iter()
        .find(|line| line.text == text)
        .unwrap();
    let mut glyphs: Vec<_> = line
        .fragments
        .iter()
        .flat_map(|fragment| {
            fragment
                .glyphs
                .iter()
                .map(|glyph| (fragment.x + glyph.x, glyph.text.as_str()))
        })
        .collect();
    glyphs.sort_by(|(left, _), (right, _)| left.total_cmp(right));
    let visible = glyphs
        .iter()
        .map(|&(_, text)| text.trim())
        .collect::<String>();
    assert_eq!(visible, "abc123يبرع");
}

//! デザイン › ページの色 › 塗りつぶし効果: a gradient, texture, pattern or
//! picture behind every page — drawn on the page, saved as Word's VML
//! background, read back, and taken off by a plain page colour.

mod common;

use jade_web::editing::EditableDocument;
use jade_web::word::page_fill::{FillTab, GradientStyle, Pattern, Texture, WordPageFill};
use jade_web::word::page_setup::WordPageSetup;

fn part(bytes: &[u8], name: &str) -> String {
    let package = jade_opc::OpcPackage::open(bytes).unwrap();
    package.part_str(name).unwrap().unwrap()
}

/// The page's colour at a point a fraction of the way across and down —
/// in the margin, clear of the text.
fn at(document: &EditableDocument, x: f32, y: f32) -> [u8; 3] {
    let page = document.page(0).expect("first page");
    let (width, height) = (page.width(), page.height());
    let pixels = page.rgba_premultiplied();
    let px = ((width as f32 * x) as u32).min(width - 1);
    let py = ((height as f32 * y) as u32).min(height - 1);
    let at = ((py * width + px) * 4) as usize;
    [pixels[at], pixels[at + 1], pixels[at + 2]]
}

fn near(color: [u8; 3], expected: [u8; 3]) -> bool {
    color
        .iter()
        .zip(expected)
        .all(|(a, b)| (i32::from(*a) - i32::from(b)).abs() <= 24)
}

#[test]
fn a_gradient_runs_down_the_page_and_is_saved() {
    let mut document = common::contract();
    let mut dialog = document.page_fill_dialog();
    assert_eq!(dialog.tab, FillTab::Gradient);
    dialog.color = "FFFFFF".to_string();
    dialog.color2 = "0070C0".to_string();
    dialog.style = GradientStyle::Horizontal;
    dialog.variant = 0;
    document.apply_page_fill(&dialog).unwrap();

    // 横: white at the top, blue at the bottom, the same across.
    let top = at(&document, 0.02, 0.01);
    let bottom = at(&document, 0.02, 0.99);
    assert!(near(top, [0xFF, 0xFF, 0xFF]), "{top:?}");
    assert!(near(bottom, [0x00, 0x70, 0xC0]), "{bottom:?}");
    assert!(near(at(&document, 0.98, 0.99), bottom));

    let bytes = document.export_docx().unwrap();
    let xml = part(&bytes, "word/document.xml");
    assert!(xml.contains("<v:background"), "{xml}");
    assert!(xml.contains(r#"type="gradient""#), "{xml}");
    assert!(xml.contains(r##"color2="#0070c0""##), "{xml}");
    let settings = part(&bytes, "word/settings.xml");
    assert!(settings.contains("w:displayBackgroundShape"), "{settings}");

    // Opened again, the dialog is on the same gradient.
    let reopened = common::open_bytes(&bytes);
    let again = reopened.page_fill_dialog();
    assert_eq!(again.tab, FillTab::Gradient);
    assert_eq!(again.color2, "0070C0");
    assert!(near(at(&reopened, 0.02, 0.99), bottom));

    // A plain page colour takes its place, and undo brings it back.
    document.set_page_color(Some("FFF9E6")).unwrap();
    assert!(document.page_fill().is_none());
    assert!(near(at(&document, 0.02, 0.99), [0xFF, 0xF9, 0xE6]));
    document.undo().unwrap();
    assert!(document.page_fill().is_some());
}

#[test]
fn a_pattern_and_a_texture_tile_the_page() {
    let mut document = common::contract();
    let mut dialog = document.page_fill_dialog();
    dialog.tab = FillTab::Pattern;
    dialog.pattern = Pattern::Percent50;
    dialog.foreground = "C00000".to_string();
    dialog.background = "FFFFFF".to_string();
    document.apply_page_fill(&dialog).unwrap();
    // 50 %: red and white side by side in the margin.
    let row: Vec<[u8; 3]> = (0..16)
        .map(|step| at(&document, 0.01 + step as f32 * 0.0015, 0.01))
        .collect();
    assert!(
        row.iter().any(|color| color[0] > 150 && color[1] < 120),
        "{row:?}"
    );
    assert!(row.iter().any(|color| color[1] > 200), "{row:?}");
    let bytes = document.export_docx().unwrap();
    let xml = part(&bytes, "word/document.xml");
    assert!(xml.contains(r#"type="pattern""#), "{xml}");
    assert!(xml.contains(r#"o:title="50%""#), "{xml}");
    let rels = part(&bytes, "word/_rels/document.xml.rels");
    assert!(rels.contains("media/"), "the pattern is a part: {rels}");

    let mut dialog = document.page_fill_dialog();
    assert_eq!(dialog.tab, FillTab::Pattern);
    assert_eq!(dialog.pattern, Pattern::Percent50);
    dialog.tab = FillTab::Texture;
    dialog.set_texture(Texture::Denim);
    document.apply_page_fill(&dialog).unwrap();
    let denim = at(&document, 0.02, 0.5);
    assert!(denim[2] > denim[0] + 30, "denim is blue: {denim:?}");
    let xml = part(&document.export_docx().unwrap(), "word/document.xml");
    assert!(xml.contains(r#"type="tile""#), "{xml}");
    assert!(xml.contains(r#"o:title="Denim""#), "{xml}");
}

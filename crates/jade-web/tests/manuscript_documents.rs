//! レイアウト › 原稿用紙設定 on the browser build's Word document — the
//! commands the native app runs too: マス目付き 20 × 20 draws its squares in
//! the 罫線の色 behind the text and lays the text on a `snapToChars` grid,
//! the grid is saved as Word writes it and read back, 下線付き draws rules
//! instead, and なし takes it away.

mod common;
use common::{contract, open_bytes};
use jade_web::editing::{EditableDocument, WordSession};
use jade_web::word::manuscript::{ManuscriptStyle, WordManuscript};

fn page_pixels(document: &EditableDocument) -> Vec<u8> {
    document.page(0).expect("first page").rgba_premultiplied()
}

/// Greenish pixels: the 罫線の色 the test picks, antialiased or not.
fn green_pixels(pixels: &[u8]) -> usize {
    pixels
        .chunks_exact(4)
        .filter(|pixel| i32::from(pixel[1]) > i32::from(pixel[0]) + 20 && pixel[1] > pixel[2])
        .count()
}

fn alike(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len()
        && a.chunks_exact(4)
            .zip(b.chunks_exact(4))
            .filter(|(a, b)| a != b)
            .count()
            < 500
}

#[test]
fn manuscript_paper_is_drawn_saved_and_removed() {
    let mut document = contract();
    let plain = page_pixels(&document);
    assert_eq!(green_pixels(&plain), 0);

    let mut dialog = document.manuscript_dialog();
    assert_eq!(dialog.style, None);
    dialog.set_style(Some(ManuscriptStyle::Squares));
    dialog.color = "#00B050".into();
    document.apply_manuscript_dialog(&dialog).unwrap();
    let squares = page_pixels(&document);
    let green = green_pixels(&squares);
    assert!(
        green > 2000,
        "the squares are drawn in the 罫線の色 ({green})"
    );

    let layout = document.word_document().section_layout(None);
    assert_eq!(layout.doc_grid.grid_type, "snapToChars");
    assert!((layout.margin_left - 85.0).abs() < 0.1);
    assert!((layout.margin_top - 72.0).abs() < 0.1);

    // Saved as Word writes it: a `Genko:` group in the header, read back.
    let bytes = document.export_docx().unwrap();
    let reopened = open_bytes(&bytes);
    let grid = reopened.manuscript_grid().expect("the grid is read back");
    assert_eq!(grid.style, ManuscriptStyle::Squares);
    assert_eq!((grid.chars, grid.lines), (20, 20));
    assert_eq!(grid.color, "#00B050");
    assert!(
        alike(&page_pixels(&reopened), &squares),
        "the saved grid is drawn the same"
    );

    // 下線付き draws rules rather than squares.
    let mut dialog = document.manuscript_dialog();
    assert_eq!(dialog.style, Some(ManuscriptStyle::Squares));
    dialog.set_style(Some(ManuscriptStyle::Lines));
    document.apply_manuscript_dialog(&dialog).unwrap();
    let lines = green_pixels(&page_pixels(&document));
    assert!(
        lines > 200 && lines < green,
        "underlines, fewer than squares ({lines})"
    );

    // なし takes it away and gives the text its line grid back.
    let mut dialog = document.manuscript_dialog();
    dialog.set_style(None);
    document.apply_manuscript_dialog(&dialog).unwrap();
    assert!(document.manuscript_grid().is_none());
    assert_eq!(green_pixels(&page_pixels(&document)), 0);
    assert_eq!(
        document
            .word_document()
            .section_layout(None)
            .doc_grid
            .grid_type,
        "lines"
    );

    // Undo brings the rules back.
    document.undo().unwrap();
    assert_eq!(
        document.manuscript_grid().map(|grid| grid.style),
        Some(ManuscriptStyle::Lines)
    );
}

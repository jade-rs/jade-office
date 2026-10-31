//! The ribbon's Top, Middle and Bottom Align and Increase and Decrease
//! Indent, as Excel's: one step at a time from the active cell.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::sheet::{span, CellRef, HorizontalAlignment, VerticalAlignment, Workbook};
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
fn vertical_alignment_toggles_back_to_bottom() {
    let mut book = book();
    let at = CellRef::new(10, 30);
    let range = span(at, at);
    assert!(book.valign(range, at, VerticalAlignment::Top).unwrap());
    assert_eq!(book.format_at(at).vertical, Some(VerticalAlignment::Top));
    book.valign(range, at, VerticalAlignment::Center).unwrap();
    assert_eq!(book.format_at(at).vertical, Some(VerticalAlignment::Center));
    book.valign(range, at, VerticalAlignment::Center).unwrap();
    assert_eq!(book.format_at(at).vertical, Some(VerticalAlignment::Bottom));
}

#[test]
fn indent_steps_up_and_down_and_moves_text_left() {
    let mut book = book();
    let at = CellRef::new(10, 30);
    let range = span(at, at);
    book.align(range, at, HorizontalAlignment::Center).unwrap();
    assert!(book.indent(range, at, true).unwrap());
    assert_eq!(book.format_at(at).indent, Some(1));
    assert_eq!(
        book.format_at(at).horizontal,
        Some(HorizontalAlignment::Left)
    );
    book.indent(range, at, true).unwrap();
    assert_eq!(book.format_at(at).indent, Some(2));
    book.indent(range, at, false).unwrap();
    book.indent(range, at, false).unwrap();
    assert_eq!(book.format_at(at).indent.unwrap_or(0), 0);
    // Nothing below none.
    assert!(!book.indent(range, at, false).unwrap());
}

#[test]
fn the_font_box_sets_the_family() {
    use jade_office_model::sheet::SHEET_FONTS;
    let mut book = book();
    let at = CellRef::new(10, 30);
    assert!(book.set_font_name(span(at, at), SHEET_FONTS[2]).unwrap());
    assert_eq!(book.format_at(at).font_name.as_deref(), Some("メイリオ"));
    assert!(book.undo().unwrap());
    assert_ne!(book.format_at(at).font_name.as_deref(), Some("メイリオ"));
}

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

#[test]
fn the_colour_gallery_has_the_theme_and_its_shades() {
    let book = book();
    let palette = book.theme_palette();
    assert_eq!(palette.len(), 60);
    assert!(palette.iter().all(|color| color.len() == 6));
    // Under each theme colour, its lighter shades get lighter.
    let red = |hex: &str| u8::from_str_radix(&hex[..2], 16).unwrap();
    let accent = 4;
    assert!(red(&palette[10 + accent]) > red(&palette[20 + accent]));
    assert!(red(&palette[accent]) > red(&palette[40 + accent]));
}

#[test]
fn merge_across_merges_each_row_on_its_own() {
    let mut book = book();
    for row in 30..33 {
        book.set_cell(CellRef::new(10, row), &format!("row {row}"))
            .unwrap();
    }
    let range = span(CellRef::new(10, 30), CellRef::new(12, 32));
    assert!(book.merge_across(range).unwrap());
    let saved = jade_xlsx_reader::read_xlsx(&book.save_bytes().unwrap()).unwrap();
    let merges = &saved.sheets[book.sheet_index()].merges;
    for row in 30..33 {
        assert!(merges.iter().any(
            |merge| merge.start == CellRef::new(10, row) && merge.end == CellRef::new(12, row)
        ));
    }
    assert_eq!(book.display_text(CellRef::new(10, 31)), "row 31");
    // One undo takes all three away.
    assert!(book.undo().unwrap());
    let saved = jade_xlsx_reader::read_xlsx(&book.save_bytes().unwrap()).unwrap();
    assert!(saved.sheets[book.sheet_index()]
        .merges
        .iter()
        .all(|merge| merge.start.row < 30 || merge.start.row > 32));
    // One column: nothing to merge.
    assert!(!book
        .merge_across(span(CellRef::new(10, 30), CellRef::new(10, 32)))
        .unwrap());
}

#[test]
fn convert_to_number_turns_numbers_stored_as_text_into_numbers() {
    let mut book = book();
    let (a, b, c) = (
        CellRef::new(10, 30),
        CellRef::new(10, 31),
        CellRef::new(10, 32),
    );
    book.set_cell(a, "'00123").unwrap();
    book.set_cell(b, "'1,234.5").unwrap();
    book.set_cell(c, "'abc").unwrap();
    assert!(book.is_number_as_text(a));
    assert!(!book.is_number_as_text(c));
    assert!(book.convert_to_number(span(a, c)).unwrap());
    assert_eq!(book.number(a), Some(123.0));
    assert_eq!(book.number(b), Some(1234.5));
    assert_eq!(book.display_text(c), "abc");
    assert!(!book.is_number_as_text(a));
    assert!(!book.convert_to_number(span(a, c)).unwrap());
    assert!(book.undo().unwrap());
    assert!(book.is_number_as_text(a));
}

#[test]
fn trace_dependents_finds_the_formulas_reading_a_cell() {
    let mut book = book();
    let source = CellRef::new(10, 30);
    book.set_cell(source, "5").unwrap();
    book.set_cell(CellRef::new(11, 30), "=K31*2").unwrap();
    book.set_cell(CellRef::new(12, 31), "=SUM(J31:K32)")
        .unwrap();
    book.set_cell(CellRef::new(13, 31), "=J1").unwrap();
    assert_eq!(
        book.dependents(source),
        vec![CellRef::new(11, 30), CellRef::new(12, 31)]
    );
    assert!(book.dependents(CellRef::new(20, 50)).is_empty());
}

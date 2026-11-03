//! Excel's View tab on a sheet: Gridlines and Headings shown or hidden,
//! the zoom (Zoom, Zoom to Selection) and Freeze Top Row / First Column,
//! each saved with the sheet so the file opens the same way.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::sheet::{span, CellRef, Freeze, Workbook};
use jade_office_model::tiny::TinySkiaBackend;

fn core() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"))
}

fn open(bytes: &[u8]) -> Workbook<TinySkiaBackend> {
    let directory = core().join("fonts/Noto_Sans_JP/static");
    let backend = TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        None,
    ))
    .unwrap();
    Workbook::open(bytes, "UTC", backend).unwrap()
}

fn book() -> Workbook<TinySkiaBackend> {
    open(&std::fs::read(core().join("public/fixtures/grid-operations-workbook.xlsx")).unwrap())
}

fn sheet_xml(bytes: &[u8]) -> String {
    let package = jade_opc::OpcPackage::open(bytes).unwrap();
    package
        .part_str("xl/worksheets/sheet1.xml")
        .unwrap()
        .unwrap()
}

#[test]
fn gridlines_hide_and_show_again_saved_with_the_sheet() {
    let mut book = book();
    assert!(book.show_grid_lines());
    assert!(book.set_show_grid_lines(false).unwrap());
    assert!(!book.show_grid_lines());
    assert!(!book.set_show_grid_lines(false).unwrap());
    let saved = book.save_bytes().unwrap();
    assert!(sheet_xml(&saved).contains(r#"showGridLines="0""#));
    assert!(!open(&saved).show_grid_lines());
    // One undo step.
    assert!(book.undo().unwrap());
    assert!(book.show_grid_lines());
}

#[test]
fn headings_hide_and_the_grid_loses_its_header_bands() {
    let mut book = book();
    assert!(book.show_headings());
    let shown = book.header_metrics();
    assert!(shown.row_width > 0.0 && shown.column_height > 0.0);
    assert!(book.set_show_headings(false).unwrap());
    assert!(!book.show_headings());
    let hidden = book.header_metrics();
    assert_eq!((hidden.row_width, hidden.column_height), (0.0, 0.0));
    let saved = book.save_bytes().unwrap();
    assert!(sheet_xml(&saved).contains(r#"showRowColHeaders="0""#));
    assert!(!open(&saved).show_headings());
    assert!(book.set_show_headings(true).unwrap());
    assert!(!sheet_xml(&book.save_bytes().unwrap()).contains("showRowColHeaders"));
}

#[test]
fn the_zoom_a_sheet_was_shown_at_is_saved_with_it() {
    let mut book = book();
    assert_eq!(book.sheet_zoom(), 1.0);
    assert!(!book.is_dirty());
    assert!(book.set_sheet_zoom(1.5));
    assert_eq!(book.sheet_zoom(), 1.5);
    // A change to save, as zooming is in Excel, but not an undo step.
    assert!(book.is_dirty());
    assert!(!book.undo().unwrap());
    assert!(!book.set_sheet_zoom(1.501));
    let saved = book.save_bytes().unwrap();
    assert!(sheet_xml(&saved).contains(r#"zoomScale="150" zoomScaleNormal="150""#));
    let mut reopened = open(&saved);
    assert_eq!(reopened.sheet_zoom(), 1.5);
    // Back to 100%: written as Excel writes it, with no zoom at all.
    assert!(reopened.set_sheet_zoom(1.0));
    assert!(!sheet_xml(&reopened.save_bytes().unwrap()).contains("zoomScale"));
    // Held to Excel's 10–400%.
    assert!(book.set_sheet_zoom(0.01));
    assert_eq!(book.sheet_zoom(), 0.1);
    assert!(book.set_sheet_zoom(9.0));
    assert_eq!(book.sheet_zoom(), 4.0);
}

#[test]
fn each_sheet_keeps_its_own_zoom() {
    let mut book = book();
    if book.sheet_names().len() < 2 {
        book.add_sheet().unwrap();
    }
    book.select_sheet(0).unwrap();
    book.set_sheet_zoom(0.75);
    book.select_sheet(1).unwrap();
    assert_eq!(book.sheet_zoom(), 1.0);
    book.set_sheet_zoom(2.0);
    book.select_sheet(0).unwrap();
    assert_eq!(book.sheet_zoom(), 0.75);
    let mut reopened = open(&book.save_bytes().unwrap());
    reopened.select_sheet(0).unwrap();
    assert_eq!(reopened.sheet_zoom(), 0.75);
    reopened.select_sheet(1).unwrap();
    assert_eq!(reopened.sheet_zoom(), 2.0);
}

#[test]
fn zoom_to_selection_fills_the_window_with_the_range() {
    let book = book();
    let metrics = book.header_metrics();
    // A1:B2 of the default 64 × 20 px cells in a 1000 × 600 px grid: the
    // two rows fill the height first.
    let (x, y, w, h) = book.cell_rect(CellRef::new(1, 1), (0.0, 0.0)).unwrap();
    let width = x + w - metrics.row_width;
    let height = y + h - metrics.column_height;
    let zoom = book.zoom_to_selection(
        span(CellRef::new(0, 0), CellRef::new(1, 1)),
        (1000.0, 600.0),
    );
    let expected =
        (1000.0 / (width + metrics.row_width)).min(600.0 / (height + metrics.column_height));
    assert_eq!(zoom, ((expected.min(4.0) * 100.0).round() / 100.0) as f32);
    // A range far bigger than the window: as small as Excel goes.
    let all = span(CellRef::new(0, 0), CellRef::new(500, 5000));
    let mut big = book;
    big.reach(all.end);
    assert_eq!(big.zoom_to_selection(all, (1000.0, 600.0)), 0.1);
    // And the range is scrolled to the top left of the part that scrolls,
    // where the first unfrozen cell is with nothing scrolled.
    assert_eq!(big.scroll_to_corner(CellRef::new(0, 0)), (0.0, 0.0));
    let (columns, rows) = big.frozen_panes().unwrap_or((0, 0));
    let corner = big
        .cell_rect(CellRef::new(columns, rows), (0.0, 0.0))
        .unwrap();
    let at = CellRef::new(columns + 3, rows + 4);
    let scroll = big.scroll_to_corner(at);
    let shown = big.cell_rect(at, scroll).unwrap();
    assert_eq!((shown.0, shown.1), (corner.0, corner.1));
}

#[test]
fn freeze_top_row_and_first_column() {
    let mut book = book();
    assert!(book.freeze_panes(Freeze::TopRow).unwrap());
    assert_eq!(book.frozen_panes(), Some((0, 1)));
    assert!(book.freeze_panes(Freeze::FirstColumn).unwrap());
    assert_eq!(book.frozen_panes(), Some((1, 0)));
    let saved = book.save_bytes().unwrap();
    assert!(sheet_xml(&saved).contains(r#"xSplit="1""#));
    assert_eq!(open(&saved).frozen_panes(), Some((1, 0)));
}

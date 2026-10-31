//! Typing lays out again only the rows of the cells it changed — the value
//! typed and the formulas reading it — rather than the whole sheet; every
//! test build checks the result against a full layout.

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
    let bytes = std::fs::read(core.join("public/fixtures/large-workbook.xlsx")).unwrap();
    Workbook::open(&bytes, "UTC", backend).unwrap()
}

#[test]
fn a_typed_value_repaints_its_row_and_the_rows_its_formulas_are_in() {
    let mut book = book();
    let at = CellRef::new(20, 3);
    // The first write after opening lays the sheet out in full.
    book.set_cell(at, "1").unwrap();
    book.set_cell(CellRef::new(21, 3), "=U4*2").unwrap();
    let _ = book.take_changes();
    let revision = book.layout_revision();

    book.set_cell(at, "21").unwrap();
    assert_eq!(book.display_text(CellRef::new(21, 3)), "42");
    assert!(book.layout_revision() > revision);
    let changes = book.take_changes().expect("the rows changed");
    let width = book.sheet_size().0 - book.header_metrics().row_width;
    // One band across the sheet, for row 4, which holds both cells.
    assert!(!changes.is_empty());
    assert!(changes
        .iter()
        .all(|(x, _, w, _)| *x == 0.0 && (*w - width).abs() < 0.01));
    let (_, top, _, height) = book.cell_rect(at, (0.0, 0.0)).unwrap();
    let column_band = book.header_metrics().column_height;
    assert!(changes
        .iter()
        .all(|(_, y, _, h)| (*y + column_band - top).abs() < 0.01 && (*h - height).abs() < 0.01));
}

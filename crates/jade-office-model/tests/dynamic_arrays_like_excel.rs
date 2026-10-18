//! Dynamic arrays as in Excel 365: a formula that returns several values
//! spills them into the cells beside it, the spill can be read whole with
//! `#`, and a value typed in its way blocks it with `#SPILL!`.

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

fn shown(book: &Workbook<TinySkiaBackend>, cells: &[&str]) -> Vec<String> {
    cells
        .iter()
        .map(|cell| book.display_text(CellRef::parse(cell).unwrap()))
        .collect()
}

#[test]
fn a_spilling_formula_fills_its_cells_and_a_value_in_its_way_blocks_it() {
    let mut book = book();
    let at = |cell: &str| CellRef::parse(cell).unwrap();
    book.set_cell(at("J1"), "=SEQUENCE(3)").unwrap();
    assert_eq!(shown(&book, &["J1", "J2", "J3", "J4"]), ["1", "2", "3", ""]);
    book.set_cell(at("L1"), "=SUM(J1#)").unwrap();
    assert_eq!(shown(&book, &["L1"]), ["6"]);

    book.set_cell(at("J2"), "x").unwrap();
    assert_eq!(
        shown(&book, &["J1", "J2", "J3", "L1"]),
        ["#SPILL!", "x", "", "#SPILL!"]
    );
    assert!(book.undo().unwrap());
    assert_eq!(
        shown(&book, &["J1", "J2", "J3", "L1"]),
        ["1", "2", "3", "6"]
    );
}

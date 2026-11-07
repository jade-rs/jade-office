//! Dynamic arrays as in Excel 365: a formula that returns several values
//! spills them into the cells beside it, the spill can be read whole with
//! `#`, and a value typed in its way blocks it with `#SPILL!`.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::sheet::{span, CellRef, Workbook};
use jade_office_model::tiny::TinySkiaBackend;

fn book() -> Workbook<TinySkiaBackend> {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let bytes = std::fs::read(core.join("public/fixtures/grid-operations-workbook.xlsx")).unwrap();
    open(&bytes)
}

fn open(bytes: &[u8]) -> Workbook<TinySkiaBackend> {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let directory = core.join("fonts/Noto_Sans_JP/static");
    let backend = TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        None,
    ))
    .unwrap();
    Workbook::open(bytes, "UTC", backend).unwrap()
}

#[test]
fn unmerging_a_spill_obstruction_recalculates_and_restores_with_history() {
    let mut book = book();
    let at = |cell: &str| CellRef::parse(cell).unwrap();
    let obstruction = span(at("J2"), at("K2"));
    assert!(book.merge_and_center(obstruction).unwrap());
    book.set_cell(at("J1"), "=SEQUENCE(3)").unwrap();
    book.set_cell(at("L1"), "=SUM(J1#)").unwrap();
    assert_eq!(
        shown(&book, &["J1", "J2", "J3", "L1"]),
        ["#SPILL!", "", "", "#SPILL!"]
    );

    assert!(book.merge_and_center(obstruction).unwrap());
    assert_eq!(
        shown(&book, &["J1", "J2", "J3", "L1"]),
        ["1", "2", "3", "6"]
    );
    assert!(book.undo().unwrap());
    assert_eq!(
        shown(&book, &["J1", "J2", "J3", "L1"]),
        ["#SPILL!", "", "", "#SPILL!"]
    );
    assert!(book.redo().unwrap());
    assert_eq!(
        shown(&book, &["J1", "J2", "J3", "L1"]),
        ["1", "2", "3", "6"]
    );

    let bytes = book.save_bytes().unwrap();
    let saved = jade_xlsx_reader::read_xlsx(&bytes).unwrap();
    let array = saved.sheets[0]
        .array_formulas
        .iter()
        .find(|array| array.anchor == at("J1"))
        .unwrap();
    assert_eq!(array.kind, jade_xlsx_reader::ArrayFormulaKind::Dynamic);
    assert_eq!(array.range, span(at("J1"), at("J3")));
    let mut reopened = open(&bytes);
    assert_eq!(
        shown(&reopened, &["J1", "J2", "J3", "L1"]),
        ["1", "2", "3", "6"]
    );
    reopened.set_cell(at("J1"), "=SEQUENCE(4)").unwrap();
    assert_eq!(
        shown(&reopened, &["J1", "J2", "J3", "J4", "L1"]),
        ["1", "2", "3", "4", "10"]
    );
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

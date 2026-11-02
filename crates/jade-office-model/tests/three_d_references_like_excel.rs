//! 3-D references typed into a workbook, as Excel takes them:
//! `=SUM(Jan:Mar!A1)` adds A1 on every sheet from Jan to Mar in tab order,
//! a sheet moved in between counts, renaming an end renames it in the
//! formula, deleting an end narrows the reference to the sheets left, and
//! the formula is saved as written.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::sheet::{CellRef, Workbook};
use jade_office_model::tiny::TinySkiaBackend;

fn backend() -> TinySkiaBackend {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let directory = core.join("fonts/Noto_Sans_JP/static");
    TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        None,
    ))
    .unwrap()
}

fn fixture() -> Vec<u8> {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    std::fs::read(core.join("public/fixtures/grid-operations-workbook.xlsx")).unwrap()
}

fn index_of(book: &Workbook<TinySkiaBackend>, name: &str) -> usize {
    book.sheet_names()
        .iter()
        .position(|sheet| sheet == name)
        .unwrap()
}

/// The fixture with Jan, Feb and Mar after its first sheet holding 1, 2
/// and 3 in A1, and the first sheet selected again.
fn quarter() -> Workbook<TinySkiaBackend> {
    let mut book = Workbook::open(&fixture(), "UTC", backend()).unwrap();
    for (month, value) in [("Jan", "1"), ("Feb", "2"), ("Mar", "3")] {
        let index = book.add_sheet().unwrap();
        book.rename_sheet(index, month).unwrap();
        book.set_cell(CellRef::new(0, 0), value).unwrap();
    }
    book.select_sheet(0).unwrap();
    book
}

const TOTAL: CellRef = CellRef { column: 7, row: 29 };

#[test]
fn sum_through_the_sheets() {
    let mut book = quarter();
    book.set_cell(TOTAL, "=SUM(Jan:Mar!A1)").unwrap();
    assert_eq!(book.display_text(TOTAL), "6");
    book.set_cell(CellRef::new(7, 30), "=AVERAGE(Jan:Mar!A1)")
        .unwrap();
    assert_eq!(book.display_text(CellRef::new(7, 30)), "2");
    book.set_cell(CellRef::new(7, 31), "=COUNT(Jan:Mar!A1:B2)")
        .unwrap();
    assert_eq!(book.display_text(CellRef::new(7, 31)), "3");
    book.set_cell(CellRef::new(7, 32), "=MAX(Jan:Mar!A1)")
        .unwrap();
    assert_eq!(book.display_text(CellRef::new(7, 32)), "3");
    book.set_cell(CellRef::new(7, 33), "=MIN(Jan:Mar!A1)")
        .unwrap();
    assert_eq!(book.display_text(CellRef::new(7, 33)), "1");

    // A value changed on a middle sheet.
    book.select_sheet(index_of(&book, "Feb")).unwrap();
    book.set_cell(CellRef::new(0, 0), "20").unwrap();
    book.select_sheet(0).unwrap();
    assert_eq!(book.display_text(TOTAL), "24");
}

#[test]
fn moving_renaming_and_deleting_sheets() {
    let mut book = quarter();
    book.set_cell(TOTAL, "=SUM(Jan:Mar!A1)").unwrap();
    // A sheet with 100 in A1 moved in between the ends is counted.
    book.select_sheet(index_of(&book, "Mar")).unwrap();
    let extra = book.add_sheet().unwrap();
    book.set_cell(CellRef::new(0, 0), "100").unwrap();
    book.move_sheet(extra, index_of(&book, "Feb")).unwrap();
    book.select_sheet(0).unwrap();
    assert_eq!(book.display_text(TOTAL), "106");

    book.rename_sheet(index_of(&book, "Mar"), "Q1 End").unwrap();
    book.select_sheet(0).unwrap();
    assert_eq!(book.edit_text(TOTAL), "=SUM('Jan:Q1 End'!A1)");
    assert_eq!(book.display_text(TOTAL), "106");

    // Jan deleted: Excel keeps the sheets that remain between the ends, so
    // the reference now starts at the sheet moved in after Jan.
    let moved_in = book.sheet_names()[index_of(&book, "Jan") + 1].clone();
    book.delete_sheet(index_of(&book, "Jan")).unwrap();
    book.select_sheet(0).unwrap();
    let after = book.edit_text(TOTAL);
    assert_eq!(after, format!("=SUM('{moved_in}:Q1 End'!A1)"));
    assert_eq!(book.display_text(TOTAL), "105");

    // Saved as written, and read back the same.
    let bytes = book.save_bytes().unwrap();
    let mut reopened = Workbook::open(&bytes, "UTC", backend()).unwrap();
    reopened.select_sheet(0).unwrap();
    assert_eq!(reopened.edit_text(TOTAL), after);
    assert_eq!(reopened.display_text(TOTAL), "105");
}

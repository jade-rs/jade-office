//! Move or Copy ▸ Create a copy, as Excel's: the copy right after the
//! sheet, named `Sheet (2)`, shown, one undo step.

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
    let bytes = std::fs::read(core.join("public/fixtures/sample-workbook.xlsx")).unwrap();
    Workbook::open(&bytes, "UTC", backend).unwrap()
}

#[test]
fn a_copy_lands_after_the_sheet_named_as_excel_names_it() {
    let mut book = book();
    let index = book.sheet_index();
    let name = book.sheet_names()[index].clone();
    book.set_cell(CellRef::new(7, 40), "copied").unwrap();
    let copy = book.copy_sheet(index).unwrap();
    assert_eq!(copy, index + 1);
    assert_eq!(book.sheet_index(), copy, "the copy shows");
    assert_eq!(book.sheet_names()[copy], format!("{name} (2)"));
    assert_eq!(book.display_text(CellRef::new(7, 40)), "copied");
    // A second copy takes the next number.
    let again = book.copy_sheet(index).unwrap();
    assert_eq!(book.sheet_names()[again], format!("{name} (3)"));
    assert!(book.undo().unwrap());
    assert!(book.undo().unwrap());
    assert!(!book
        .sheet_names()
        .iter()
        .any(|sheet| sheet.ends_with("(2)")));
}

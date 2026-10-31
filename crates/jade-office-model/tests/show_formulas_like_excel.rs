//! Formulas ▸ Show Formulas (⌃`), as Excel's: a sheet's formula cells show
//! their formulas, written to the sheet's view, one undo step.

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

#[test]
fn show_formulas_toggles_on_the_sheet_and_undoes() {
    let mut book = book();
    book.set_cell(CellRef::new(5, 5), "=1+2").unwrap();
    assert!(!book.show_formulas());
    assert!(book.set_show_formulas(true).unwrap());
    assert!(book.show_formulas());
    assert!(!book.set_show_formulas(true).unwrap());
    // Saved with the sheet.
    let bytes = book.save_bytes().unwrap();
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let backend = TinySkiaBackend::new(&aliased_faces(
        Arc::new(
            std::fs::read(core.join("fonts/Noto_Sans_JP/static/NotoSansJP-Regular.ttf")).unwrap(),
        ),
        None,
    ))
    .unwrap();
    let reopened = Workbook::open(&bytes, "UTC", backend).unwrap();
    assert!(reopened.show_formulas());
    assert!(book.undo().unwrap());
    assert!(!book.show_formulas());
}

//! Formulas ▸ Calculation Options and Calculate Now (F9): in manual
//! calculation a formula keeps its value until F9, and going back to
//! automatic recalculates at once.

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
fn manual_calculation_waits_for_f9() {
    let mut book = book();
    book.set_cell(CellRef::new(7, 29), "2").unwrap();
    book.set_cell(CellRef::new(8, 29), "=H30*10").unwrap();
    assert_eq!(book.display_text(CellRef::new(8, 29)), "20");

    assert!(!book.manual_calculation());
    assert!(book.set_manual_calculation(true).unwrap());
    assert!(book.manual_calculation());
    book.set_cell(CellRef::new(7, 29), "3").unwrap();
    // The formula keeps its value until F9.
    assert_eq!(book.display_text(CellRef::new(8, 29)), "20");
    assert!(book.calculate_now().unwrap());
    assert_eq!(book.display_text(CellRef::new(8, 29)), "30");

    book.set_cell(CellRef::new(7, 29), "4").unwrap();
    // Back to automatic: recalculated at once.
    assert!(book.set_manual_calculation(false).unwrap());
    assert_eq!(book.display_text(CellRef::new(8, 29)), "40");
    // Saved with the workbook.
    assert!(book.set_manual_calculation(true).unwrap());
    let saved = book.save_bytes().unwrap();
    assert_eq!(
        jade_xlsx_reader::read_xlsx(&saved)
            .unwrap()
            .calculation_mode,
        jade_xlsx_reader::CalculationMode::Manual
    );
}

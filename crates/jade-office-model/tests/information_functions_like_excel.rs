//! CELL, INFO and FORECAST.ETS typed into a workbook: CELL("filename") and
//! INFO("directory") name where the book is saved, INFO("origin") the
//! top-left cell the window shows, and FORECAST.ETS continues a series.

#![cfg(feature = "tiny-skia")]

use std::path::{Path, PathBuf};
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
fn cell_and_info_know_where_the_book_is() {
    let mut book = book();
    let (location, directory) = if cfg!(windows) {
        (
            r"C:\Users\me\Documents\Budget.xlsx",
            r"C:\Users\me\Documents\",
        )
    } else {
        ("/Users/me/Documents/Budget.xlsx", "/Users/me/Documents/")
    };
    book.set_location(Path::new(location));
    let sheet = book.sheet_name();
    book.set_cell(CellRef::new(7, 29), "=CELL(\"filename\",A1)")
        .unwrap();
    assert_eq!(
        book.display_text(CellRef::new(7, 29)),
        format!("{directory}[Budget.xlsx]{sheet}")
    );
    book.set_cell(CellRef::new(7, 30), "=INFO(\"directory\")")
        .unwrap();
    assert_eq!(book.display_text(CellRef::new(7, 30)), directory);
    // The window scrolled to show C5 at its top left.
    book.set_view_origin(4, 2);
    book.set_cell(CellRef::new(7, 31), "=INFO(\"origin\")")
        .unwrap();
    assert_eq!(book.display_text(CellRef::new(7, 31)), "$A:$C$5");
    assert!(book.set_manual_calculation(true).unwrap());
    book.set_cell(CellRef::new(7, 32), "=INFO(\"recalc\")")
        .unwrap();
    book.calculate_now().unwrap();
    assert_eq!(book.display_text(CellRef::new(7, 32)), "Manual");
}

#[test]
fn forecast_ets_continues_a_series() {
    let mut book = book();
    // A line, 3, 5, 7 … 21 over 1 to 10: Excel forecasts 23 at 11.
    for row in 0..10u32 {
        book.set_cell(CellRef::new(10, 40 + row), &(row + 1).to_string())
            .unwrap();
        book.set_cell(CellRef::new(11, 40 + row), &(2 * row + 3).to_string())
            .unwrap();
    }
    book.set_cell(CellRef::new(12, 40), "=FORECAST.ETS(11,L41:L50,K41:K50)")
        .unwrap();
    assert_eq!(book.display_text(CellRef::new(12, 40)), "23");
}

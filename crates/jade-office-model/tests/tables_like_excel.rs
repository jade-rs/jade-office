//! Formulas over an Excel table — a totals row's SUBTOTAL over a column,
//! `Table1[Qty]` typed beside it — recalculate when the table is edited, as
//! in Excel, rather than keeping the totals the file was saved with.

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
    // Table1 on A1:C5: Item, Qty, Amount; Total row SUBTOTAL(109,Table1[Amount]).
    let bytes = std::fs::read(core.join("public/fixtures/objects-workbook.xlsx")).unwrap();
    Workbook::open(&bytes, "UTC", backend).unwrap()
}

#[test]
fn editing_a_table_updates_its_totals_and_the_formulas_over_it() {
    let mut book = book();
    let total = CellRef::new(2, 4);
    assert_eq!(book.number(total), Some(749.0));
    let quantities = CellRef::new(4, 0);
    book.set_cell(quantities, "=SUM(Table1[Qty])").unwrap();
    assert_eq!(book.number(quantities), Some(6.0));

    // Gizmo: 1 for 99 becomes 3 for 199.
    book.set_cell(CellRef::new(1, 3), "3").unwrap();
    book.set_cell(CellRef::new(2, 3), "199").unwrap();
    assert_eq!(book.number(total), Some(849.0));
    assert_eq!(book.number(quantities), Some(8.0));
}

//! Data ▸ Subtotal as Excel's command makes it: a total row after each run
//! of one value, a grand total after the last, `SUBTOTAL` formulas that skip
//! the subtotals among their rows, a three-level outline, one undo step.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::sheet::{span, Axis, CellRef, Workbook};
use jade_office_model::subtotal::{SubtotalFunction, Subtotals};
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
fn each_run_gets_a_total_and_the_list_a_grand_total_in_an_outline() {
    let mut book = book();
    // H30:I35 — a header, then 東京 ×2, 大阪 ×3.
    let rows = [
        ("支店", "売上"),
        ("東京", "10"),
        ("東京", "20"),
        ("大阪", "1"),
        ("大阪", "2"),
        ("大阪", "3"),
    ];
    for (offset, (branch, sales)) in rows.iter().enumerate() {
        let row = 29 + offset as u32;
        book.set_cell(CellRef::new(7, row), branch).unwrap();
        book.set_cell(CellRef::new(8, row), sales).unwrap();
    }
    let subtotals = Subtotals {
        by: 7,
        function: SubtotalFunction::Sum,
        columns: vec![8],
        label: "{} 集計".into(),
        grand_label: "総計".into(),
    };
    assert!(book
        .subtotal(span(CellRef::new(7, 29), CellRef::new(8, 34)), &subtotals)
        .unwrap());

    let shown = |row: u32| {
        (
            book.display_text(CellRef::new(7, row)),
            book.display_text(CellRef::new(8, row)),
        )
    };
    assert_eq!(shown(32), ("東京 集計".into(), "30".into()));
    assert_eq!(shown(36), ("大阪 集計".into(), "6".into()));
    // The grand total reads past the subtotals.
    assert_eq!(shown(37), ("総計".into(), "36".into()));
    assert_eq!(book.edit_text(CellRef::new(8, 32)), "=SUBTOTAL(9,I31:I32)");
    assert!(book.has_outline(Axis::Rows));

    // One undo takes all of it back.
    assert!(book.undo().unwrap());
    assert_eq!(book.display_text(CellRef::new(7, 32)), "大阪");
    assert!(!book.has_outline(Axis::Rows));
}

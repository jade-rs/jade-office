//! Data ▸ Group and Outline as Excel's menu does it on selected rows or
//! columns: Group, Hide Detail, Show Detail and Ungroup, each one undo step,
//! the hidden detail stepped over like any hidden row.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::grid::Direction;
use jade_office_model::sheet::{Axis, CellRef, OutlineChange, Workbook};
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
fn grouped_rows_collapse_expand_and_undo_one_step_at_a_time() {
    let mut book = book();
    assert!(!book.has_outline(Axis::Rows));
    // Rows 3–5 grouped; row 6 is their summary.
    assert!(book
        .outline(Axis::Rows, 2, 4, OutlineChange::Group)
        .unwrap());
    assert!(book.has_outline(Axis::Rows));
    assert!(!book.has_outline(Axis::Columns));
    assert!(book.is_dirty());

    assert!(book
        .outline(Axis::Rows, 5, 5, OutlineChange::HideDetail)
        .unwrap());
    // Down from row 2 lands on the summary, past the hidden detail.
    assert_eq!(
        book.step(CellRef::new(0, 1), Direction::Down),
        CellRef::new(0, 5)
    );

    assert!(book
        .outline(Axis::Rows, 5, 5, OutlineChange::ShowDetail)
        .unwrap());
    assert_eq!(
        book.step(CellRef::new(0, 1), Direction::Down),
        CellRef::new(0, 2)
    );

    // Back through each step: shown, hidden, grouped, as it was.
    assert!(book.undo().unwrap());
    assert_eq!(
        book.step(CellRef::new(0, 1), Direction::Down),
        CellRef::new(0, 5)
    );
    assert!(book.undo().unwrap());
    assert!(book.has_outline(Axis::Rows));
    assert!(book.undo().unwrap());
    assert!(!book.has_outline(Axis::Rows));

    // Ungrouping nothing is no step.
    assert!(!book
        .outline(Axis::Rows, 2, 4, OutlineChange::Ungroup)
        .unwrap());
}

#[test]
fn grouped_columns_hide_their_detail_too() {
    let mut book = book();
    assert!(book
        .outline(Axis::Columns, 1, 2, OutlineChange::Group)
        .unwrap());
    assert!(book
        .outline(Axis::Columns, 3, 3, OutlineChange::HideDetail)
        .unwrap());
    assert_eq!(
        book.step(CellRef::new(0, 0), Direction::Right),
        CellRef::new(3, 0)
    );
    assert!(book
        .outline(Axis::Columns, 1, 2, OutlineChange::Ungroup)
        .unwrap());
    assert!(!book.has_outline(Axis::Columns));
}

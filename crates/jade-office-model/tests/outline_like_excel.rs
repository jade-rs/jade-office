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

#[test]
fn the_gutter_buttons_collapse_and_expand_on_a_click() {
    use jade_xlsx_layout::outline_gutter::lane_center;
    let mut book = book();
    let before = book.header_metrics();
    book.outline(Axis::Rows, 2, 4, OutlineChange::Group)
        .unwrap();
    let metrics = book.header_metrics();
    // The gutter widens the row band.
    assert!(metrics.row_width > before.row_width);
    let none = (0.0, 0.0);

    // Level 1 collapses the group: down from row 2 lands on row 6.
    let letters = (metrics.outline_height + metrics.column_height) / 2.0;
    assert_eq!(
        book.outline_click(lane_center(1), letters, none),
        Some(Ok(true))
    );
    assert_eq!(
        book.step(CellRef::new(0, 1), Direction::Down),
        CellRef::new(0, 5)
    );

    // Row 6's box is a + now; a click shows the rows again.
    let (_, top, _, height) = book.cell_rect(CellRef::new(0, 5), none).unwrap();
    assert_eq!(
        book.outline_click(lane_center(1), top + height / 2.0, none),
        Some(Ok(true))
    );
    assert_eq!(
        book.step(CellRef::new(0, 1), Direction::Down),
        CellRef::new(0, 2)
    );
    // A click on a row number is no gutter click.
    assert_eq!(
        book.outline_click(metrics.row_width - 2.0, metrics.column_height + 4.0, none),
        None
    );
}

#[test]
fn auto_outline_groups_under_summary_formulas_and_clears() {
    let mut book = book();
    for row in 20..=22 {
        book.set_cell(CellRef::new(7, row), "1").unwrap();
    }
    book.set_cell(CellRef::new(7, 23), "=SUM(H21:H23)").unwrap();
    assert!(book.auto_outline().unwrap());
    assert!(book.has_outline(Axis::Rows));
    assert!(book.clear_outline(Axis::Rows).unwrap());
    assert!(!book.has_outline(Axis::Rows));
    assert!(book.undo().unwrap());
    assert!(book.has_outline(Axis::Rows));
}

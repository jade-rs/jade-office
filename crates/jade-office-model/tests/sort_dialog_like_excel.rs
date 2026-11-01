//! Data ▸ Sort with the Sort dialog's levels, as Excel sorts: by the first
//! key, ties by the next, the header row left on top.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::sheet::{span, CellRef, Workbook};
use jade_office_model::sort_dialog::{SortDialog, SortLevel, SortOn};
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

fn fill(book: &mut Workbook<TinySkiaBackend>) {
    let rows = [
        ("Region", "Sales"),
        ("West", "10"),
        ("East", "5"),
        ("West", "30"),
        ("East", "20"),
    ];
    for (offset, (region, sales)) in rows.into_iter().enumerate() {
        let row = 29 + offset as u32;
        book.set_cell(CellRef::new(7, row), region).unwrap();
        book.set_cell(CellRef::new(8, row), sales).unwrap();
    }
}

#[test]
fn two_keys_sort_by_the_first_then_the_second() {
    let mut book = book();
    fill(&mut book);
    let range = span(CellRef::new(7, 29), CellRef::new(8, 33));
    // Region ascending, then Sales descending.
    assert!(book
        .sort_by(range, true, &[(7, false), (8, true)], false)
        .unwrap());
    let rows: Vec<(String, String)> = (29..=33)
        .map(|row| {
            (
                book.display_text(CellRef::new(7, row)),
                book.display_text(CellRef::new(8, row)),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("Region".to_string(), "Sales".to_string()),
            ("East".into(), "20".into()),
            ("East".into(), "5".into()),
            ("West".into(), "30".into()),
            ("West".into(), "10".into()),
        ]
    );
    assert!(book.undo().unwrap());
    assert_eq!(book.display_text(CellRef::new(7, 30)), "West");
}

#[test]
fn the_dialog_opens_on_the_list_with_one_level_and_adds_others() {
    let mut book = book();
    fill(&mut book);
    let at = CellRef::new(8, 31);
    let mut dialog = SortDialog::open(&book, span(at, at), at);
    assert_eq!(dialog.range, span(CellRef::new(7, 29), CellRef::new(8, 33)));
    assert!(dialog.header);
    assert_eq!(dialog.column_names(), ["Region", "Sales"]);
    assert_eq!(
        dialog.levels,
        [SortLevel {
            column: 1,
            descending: false,
            on: SortOn::Values,
        }]
    );
    dialog.add_level();
    assert_eq!(dialog.levels[1].column, 0);
    assert_eq!(dialog.keys(), [(8, false), (7, false)]);
    // Without a header the columns go by their letters.
    dialog.header = false;
    assert_eq!(dialog.column_names(), ["H", "I"]);
    dialog.remove_level(0);
    dialog.remove_level(0);
    assert_eq!(dialog.levels.len(), 1);
}

/// Sort On Cell Color: the dialog offers the colours the column has, the
/// first not none chosen; that colour's rows go on top, or on bottom.
#[test]
fn a_level_sorts_on_a_cell_colour_on_top_or_bottom() {
    use jade_office_model::sheet::{span, CellFormat};
    let mut book = book();
    for (row, name) in ["Name", "a", "b", "c", "d"].iter().enumerate() {
        book.set_cell(CellRef::new(20, 40 + row as u32), name)
            .unwrap();
    }
    let yellow = CellFormat {
        fill_color: Some("FFFF00".into()),
        ..CellFormat::default()
    };
    book.format(span(CellRef::new(20, 42), CellRef::new(20, 42)), &yellow)
        .unwrap();
    book.format(span(CellRef::new(20, 44), CellRef::new(20, 44)), &yellow)
        .unwrap();
    let at = CellRef::new(20, 41);
    let mut dialog = SortDialog::open(&book, span(CellRef::new(20, 40), CellRef::new(20, 44)), at);
    // Text over text reads as no header; this list has one.
    dialog.header = true;
    assert!(dialog.colors(0, false).contains(&"FFFF00".to_string()));
    dialog.sort_on(0, Some(false));
    assert_eq!(dialog.levels[0].on, SortOn::CellColor("FFFF00".into()));
    assert!(dialog.apply(&mut book).unwrap());
    let column = |book: &Workbook<TinySkiaBackend>| {
        (41..=44)
            .map(|row| book.display_text(CellRef::new(20, row)))
            .collect::<Vec<_>>()
    };
    assert_eq!(column(&book), ["b", "d", "a", "c"]);
    dialog.levels[0].descending = true;
    assert!(dialog.apply(&mut book).unwrap());
    assert_eq!(column(&book), ["a", "c", "b", "d"]);
    // Back to the values.
    dialog.sort_on(0, None);
    dialog.levels[0].descending = true;
    assert!(dialog.apply(&mut book).unwrap());
    assert_eq!(column(&book), ["d", "c", "b", "a"]);
}

/// Options ▸ Sort left to right: the levels pick rows, no header, and the
/// columns move by the row chosen; back to top to bottom resets the level.
#[test]
fn sorting_left_to_right_orders_the_columns_by_a_row() {
    use jade_office_model::sheet::span;
    let mut book = book();
    for (column, (key, name)) in [("3", "c"), ("1", "a"), ("2", "b")].iter().enumerate() {
        book.set_cell(CellRef::new(20 + column as u32, 60), key)
            .unwrap();
        book.set_cell(CellRef::new(20 + column as u32, 61), name)
            .unwrap();
    }
    let range = span(CellRef::new(20, 60), CellRef::new(22, 61));
    let mut dialog = SortDialog::open(&book, range, CellRef::new(21, 60));
    dialog.set_left_to_right(true);
    assert!(dialog.left_to_right && !dialog.header);
    assert_eq!(dialog.row_numbers(), [61, 62]);
    assert_eq!(dialog.levels[0].column, 0, "the active cell's row");
    assert!(dialog.apply(&mut book).unwrap());
    let row = |book: &Workbook<TinySkiaBackend>| {
        (20..=22)
            .map(|column| book.display_text(CellRef::new(column, 61)))
            .collect::<Vec<_>>()
    };
    assert_eq!(row(&book), ["a", "b", "c"]);
    // By the second row, descending.
    dialog.levels[0].column = 1;
    dialog.levels[0].descending = true;
    assert!(dialog.apply(&mut book).unwrap());
    assert_eq!(row(&book), ["c", "b", "a"]);
    assert!(book.undo().unwrap());
    assert_eq!(row(&book), ["a", "b", "c"]);
    dialog.set_left_to_right(false);
    assert_eq!(dialog.levels.len(), 1);
    assert_eq!(dialog.levels[0].column, 1, "the active cell's column again");
}

//! Data ▸ Remove Duplicates as Excel does it: repeated rows of a range taken
//! out (text compared without regard to case), the rest moving up within
//! the range the way Delete ▸ Shift cells up moves them, what lies below
//! it left where it was, one undo step.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::duplicates::{Deduplicated, DuplicatesDialog};
use jade_office_model::sheet::{span, CellRef, Workbook};
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
fn repeated_rows_go_and_the_rest_move_up_within_the_range() {
    let mut book = book();
    // H30:I36 — a header, then Tokyo twice (once in capitals), Osaka twice.
    let rows = [
        ("Branch", "Region"),
        ("Tokyo", "East"),
        ("Osaka", "West"),
        ("TOKYO", "east"),
        ("Nagoya", "Central"),
        ("Osaka", "West"),
        ("Sapporo", "North"),
    ];
    for (offset, (branch, region)) in rows.into_iter().enumerate() {
        let row = 29 + offset as u32;
        book.set_cell(CellRef::new(7, row), branch).unwrap();
        book.set_cell(CellRef::new(8, row), region).unwrap();
    }
    // Below the range: left alone.
    book.set_cell(CellRef::new(7, 37), "=COUNTA(H31:H36)")
        .unwrap();
    let range = span(CellRef::new(7, 29), CellRef::new(8, 35));

    let outcome = book.remove_duplicates(range, &[7, 8], true).unwrap();
    assert_eq!(
        outcome,
        Deduplicated {
            removed: 2,
            remaining: 4
        }
    );
    let column = |book: &Workbook<TinySkiaBackend>| {
        (29..=35)
            .map(|row| book.display_text(CellRef::new(7, row)))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        column(&book),
        ["Branch", "Tokyo", "Osaka", "Nagoya", "Sapporo", "", ""]
    );
    // The formula below the range is still in H38; its range shrank with
    // the cells taken out, as Delete ▸ Shift cells up shrinks it.
    assert_eq!(book.edit_text(CellRef::new(7, 37)), "=COUNTA(H31:H34)");
    assert_eq!(book.display_text(CellRef::new(7, 37)), "4");

    // Over the rows left, nothing more to remove: no step.
    let left = span(CellRef::new(7, 29), CellRef::new(8, 33));
    let again = book.remove_duplicates(left, &[7, 8], true).unwrap();
    assert_eq!(again.removed, 0);
    // One undo brings every row back.
    assert!(book.undo().unwrap());
    assert_eq!(
        column(&book),
        ["Branch", "Tokyo", "Osaka", "TOKYO", "Nagoya", "Osaka", "Sapporo"]
    );
}

#[test]
fn a_text_header_over_other_values_reads_as_a_header() {
    let mut book = book();
    book.set_cell(CellRef::new(7, 29), "Amount").unwrap();
    book.set_cell(CellRef::new(7, 30), "10").unwrap();
    assert!(book.looks_like_headers(span(CellRef::new(7, 29), CellRef::new(7, 30))));
    book.set_cell(CellRef::new(7, 30), "ten").unwrap();
    assert!(!book.looks_like_headers(span(CellRef::new(7, 29), CellRef::new(7, 30))));
}

/// The dialog opens on the list round the active cell, every column ticked
/// and named by its header; comparing one column takes out the rows that
/// repeat it alone.
#[test]
fn the_dialog_compares_only_the_columns_ticked() {
    let mut book = book();
    let rows = [
        ("Branch", "Sales"),
        ("Tokyo", "10"),
        ("Yokohama", "10"),
        ("Osaka", "20"),
    ];
    for (offset, (branch, region)) in rows.into_iter().enumerate() {
        let row = 29 + offset as u32;
        book.set_cell(CellRef::new(7, row), branch).unwrap();
        book.set_cell(CellRef::new(8, row), region).unwrap();
    }
    let at = CellRef::new(7, 30);
    let mut dialog = DuplicatesDialog::open(&book, span(at, at), at);
    assert_eq!(dialog.range, span(CellRef::new(7, 29), CellRef::new(8, 32)));
    assert!(dialog.header);
    assert_eq!(dialog.column_names("Column"), ["Branch", "Sales"]);
    assert_eq!(dialog.columns(), [7, 8]);
    dialog.header = false;
    assert_eq!(dialog.column_names("Column"), ["Column H", "Column I"]);
    dialog.header = true;
    dialog.select_all(false);
    assert!(dialog.columns().is_empty());
    dialog.compared[1] = true;
    assert_eq!(dialog.columns(), [8]);

    let outcome = book
        .remove_duplicates(dialog.range, &dialog.columns(), dialog.header)
        .unwrap();
    assert_eq!(
        outcome,
        Deduplicated {
            removed: 1,
            remaining: 2
        }
    );
    assert_eq!(book.display_text(CellRef::new(7, 31)), "Osaka");
}

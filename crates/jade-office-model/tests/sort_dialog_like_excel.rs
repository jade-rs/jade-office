//! Data ▸ Sort with the Sort dialog's levels, as Excel sorts: by the first
//! key, ties by the next, the header row left on top.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::sheet::{span, CellRef, Workbook};
use jade_office_model::sort_dialog::{SortDialog, SortLevel};
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
            descending: false
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

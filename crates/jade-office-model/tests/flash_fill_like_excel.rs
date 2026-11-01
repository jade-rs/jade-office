//! Data ▸ Flash Fill as Excel's: the column filled down its list by the
//! pattern the examples typed in it follow, one undo step.

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
fn flash_fill_follows_the_example_down_the_list() {
    let mut book = book();
    let at = |column, row| CellRef::new(column, row);
    let names = ["Taro Tanaka", "Hanako Suzuki", "Jiro Sato", ""];
    book.set_cell(at(20, 40), "Name").unwrap();
    book.set_cell(at(21, 40), "Sort as").unwrap();
    for (row, name) in names.iter().enumerate() {
        if !name.is_empty() {
            book.set_cell(at(20, 41 + row as u32), name).unwrap();
        }
    }
    book.set_cell(at(21, 41), "Tanaka, Taro").unwrap();
    let filled = book.flash_fill(at(21, 41)).unwrap().expect("filled");
    assert_eq!(filled.start, at(21, 42));
    assert_eq!(filled.end, at(21, 43));
    assert_eq!(book.display_text(at(21, 42)), "Suzuki, Hanako");
    assert_eq!(book.display_text(at(21, 43)), "Sato, Jiro");
    assert_eq!(book.display_text(at(21, 40)), "Sort as", "the header stays");
    assert!(book.undo().unwrap());
    assert_eq!(book.display_text(at(21, 42)), "");
    // Nothing typed to follow: nothing done.
    assert_eq!(book.flash_fill(at(22, 41)).unwrap(), None);
}

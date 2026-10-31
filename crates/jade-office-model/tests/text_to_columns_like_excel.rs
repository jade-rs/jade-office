//! Data ▸ Text to Columns, delimited, as Excel's wizard does it: pieces at
//! the delimiters chosen, quotes keeping delimiters as text, consecutive
//! delimiters as one when asked, each piece taken as typing takes it.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::sheet::{span, CellRef, Workbook};
use jade_office_model::text_columns::TextToColumns;
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
fn the_split_follows_delimiters_quotes_and_consecutive_ones() {
    let comma = TextToColumns {
        tab: false,
        comma: true,
        ..TextToColumns::default()
    };
    assert_eq!(comma.split("a,b,,c"), ["a", "b", "", "c"]);
    assert_eq!(
        comma.split(r#""Tokyo, Japan","""quoted"" here",3"#),
        ["Tokyo, Japan", "\"quoted\" here", "3"]
    );
    let spaces = TextToColumns {
        tab: false,
        space: true,
        consecutive: true,
        ..TextToColumns::default()
    };
    assert_eq!(spaces.split("one   two three"), ["one", "two", "three"]);
}

#[test]
fn a_column_splits_into_the_columns_beside_it_as_typed() {
    let mut book = book();
    book.set_cell(CellRef::new(7, 29), "東京,1234,2026/1/5")
        .unwrap();
    book.set_cell(CellRef::new(7, 30), "大阪,56").unwrap();
    book.set_cell(CellRef::new(7, 31), "no delimiter").unwrap();
    let split = TextToColumns {
        tab: false,
        comma: true,
        ..TextToColumns::default()
    };
    assert!(book
        .text_to_columns(span(CellRef::new(7, 29), CellRef::new(7, 31)), &split)
        .unwrap());
    assert_eq!(book.display_text(CellRef::new(7, 29)), "東京");
    assert_eq!(book.edit_text(CellRef::new(8, 29)), "1234");
    // Taken as typed: a date, not text.
    assert_ne!(book.edit_text(CellRef::new(9, 29)), "2026/1/5x");
    assert!(book.display_text(CellRef::new(9, 29)).contains("2026"));
    assert_eq!(book.display_text(CellRef::new(8, 30)), "56");
    assert_eq!(book.display_text(CellRef::new(7, 31)), "no delimiter");
    // One step back.
    assert!(book.undo().unwrap());
    assert_eq!(book.display_text(CellRef::new(7, 29)), "東京,1234,2026/1/5");
    assert_eq!(book.display_text(CellRef::new(8, 29)), "");
}

//! The Japanese text functions, typed into a cell, compute what a Japanese
//! Excel computes. The byte functions count a full-width character as two
//! bytes (Shift_JIS); ASC and JIS convert between half- and full-width.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::sheet::{CellRef, Workbook};
use jade_office_model::tiny::TinySkiaBackend;

fn core() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"))
}

fn book() -> Workbook<TinySkiaBackend> {
    let directory = core().join("fonts/Noto_Sans_JP/static");
    let backend = TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        None,
    ))
    .unwrap();
    let bytes =
        std::fs::read(core().join("public/fixtures/grid-operations-workbook.xlsx")).unwrap();
    Workbook::open(&bytes, "UTC", backend).unwrap()
}

/// A formula and what the cell shows.
const CASES: &[(&str, &str)] = &[
    ("=LEN(\"あいうabc\")", "6"),
    ("=LENB(\"あいうabc\")", "9"),
    ("=LEFTB(\"あいう\",4)", "あい"),
    ("=MIDB(\"あいうえ\",3,4)", "いう"),
    ("=RIGHTB(\"あいう\",2)", "う"),
    ("=FINDB(\"う\",\"あいう\")", "5"),
    ("=SEARCHB(\"U\",\"あいu\")", "5"),
    ("=REPLACEB(\"あいうえ\",3,2,\"X\")", "あXうえ"),
    ("=REPLACEB(\"abcdef\",2,3,\"あ\")", "aあef"),
    ("=ASC(\"ＡＢＣ１２３\")", "ABC123"),
    ("=JIS(\"ABC123\")", "ＡＢＣ１２３"),
];

#[test]
fn japanese_text_functions_compute_as_japanese_excel_does() {
    let mut book = book();
    let mut failures = Vec::new();
    for (index, (formula, expected)) in CASES.iter().enumerate() {
        let at = CellRef::new(8, 10 + index as u32);
        if let Err(error) = book.set_cell(at, formula) {
            failures.push(format!("{formula}: not accepted ({error})"));
            continue;
        }
        let shown = book.display_text(at);
        if shown != *expected {
            failures.push(format!("{formula}: expected {expected:?}, shown {shown:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

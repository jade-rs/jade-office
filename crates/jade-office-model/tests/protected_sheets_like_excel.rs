//! A protected sheet refuses what Excel refuses: typing into a locked cell,
//! and formatting, with words the shells show. Unlocked cells and other
//! sheets edit as ever.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::sheet::{protected_refusal, CellFormat, CellRange, CellRef, Workbook};
use jade_office_model::tiny::TinySkiaBackend;

fn book() -> Workbook<TinySkiaBackend> {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let directory = core.join("fonts/Noto_Sans_JP/static");
    let backend = TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        None,
    ))
    .unwrap();
    let bytes = std::fs::read(core.join("public/fixtures/protected-sheets-workbook.xlsx")).unwrap();
    Workbook::open(&bytes, "UTC", backend).unwrap()
}

#[test]
fn a_locked_cell_refuses_typing_and_says_why() {
    let mut book = book();
    assert_eq!(book.sheet_name(), "Locked");
    let a1 = CellRef::new(0, 0);
    let error = book.set_cell(a1, "5").unwrap_err();
    assert!(protected_refusal(&error), "{error}");
    assert_eq!(book.display_text(a1), "10");
    assert!(!book.is_dirty());
    // B1 is unlocked.
    let b1 = CellRef::new(1, 0);
    book.set_cell(b1, "25").unwrap();
    assert_eq!(book.display_text(b1), "25");
    // Formatting is refused even there: the protection forbids it.
    let bold = CellFormat {
        bold: Some(true),
        ..CellFormat::default()
    };
    let error = book
        .format(CellRange { start: b1, end: b1 }, &bold)
        .unwrap_err();
    assert!(protected_refusal(&error), "{error}");
    // The open sheet edits as ever.
    book.select_sheet(1).unwrap();
    book.set_cell(a1, "5").unwrap();
    assert_eq!(book.display_text(a1), "5");
    assert!(!protected_refusal("cannot write the workbook"));
}

//! Clear ▸ Clear Formats and Clear All as Excel's Home ▸ Clear does them:
//! formats back to the default with the values kept, or everything gone,
//! each one undo step.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::sheet::{span, CellFormat, CellRef, Workbook};
use jade_office_model::tiny::TinySkiaBackend;

fn book() -> Workbook<TinySkiaBackend> {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let directory = core.join("fonts/Noto_Sans_JP/static");
    let backend = TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        None,
    ))
    .unwrap();
    let bytes = std::fs::read(core.join("public/fixtures/sample-workbook.xlsx")).unwrap();
    Workbook::open(&bytes, "UTC", backend).unwrap()
}

fn percent(book: &mut Workbook<TinySkiaBackend>, from: CellRef, to: CellRef) {
    let change = CellFormat {
        number_format: Some("0.00%".to_string()),
        ..CellFormat::default()
    };
    book.format(span(from, to), &change).unwrap();
}

#[test]
fn clear_formats_keeps_the_values_and_undoes_in_one_step() {
    let mut book = book();
    let (a, b) = (CellRef::new(10, 40), CellRef::new(10, 41));
    book.set_cell(a, "0.5").unwrap();
    book.set_cell(b, "=K41*2").unwrap();
    percent(&mut book, a, b);
    assert_eq!(book.display_text(b), "100.00%");

    assert!(book.clear_formats(span(a, b)).unwrap());
    assert_eq!(book.display_text(a), "0.5");
    assert_eq!(book.display_text(b), "1");
    assert_eq!(book.edit_text(b), "=K41*2");
    // Nothing formatted left.
    assert!(!book.clear_formats(span(a, b)).unwrap());
    assert!(book.undo().unwrap());
    assert_eq!(book.display_text(a), "50.00%");
}

#[test]
fn clear_all_empties_and_unformats_in_one_step() {
    let mut book = book();
    let (a, b) = (CellRef::new(10, 40), CellRef::new(10, 41));
    book.set_cell(a, "0.5").unwrap();
    percent(&mut book, a, b);

    assert!(book.clear_all(span(a, b)).unwrap());
    assert_eq!(book.display_text(a), "");
    // Typed in again, the number wears no format.
    book.set_cell(b, "0.25").unwrap();
    assert_eq!(book.display_text(b), "0.25");
    assert!(book.undo().unwrap());
    assert!(book.undo().unwrap());
    assert_eq!(book.display_text(a), "50.00%");
}

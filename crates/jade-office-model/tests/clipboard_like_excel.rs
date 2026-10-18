//! Copy and paste speak Excel's clipboard text: tab between cells, a line
//! break between rows, and a cell holding a line break, a tab or a quote
//! wrapped in quotes with its quotes doubled. Windows Excel ends rows with
//! CR LF. A multi-line cell pasted from Excel stays one cell, and one copied
//! out arrives in Excel as one cell.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::sheet::{span, CellRef, Workbook};
use jade_office_model::tiny::TinySkiaBackend;

fn book() -> Workbook<TinySkiaBackend> {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let directory = core.join("fonts/Noto_Sans_JP/static");
    let backend = TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        Some(Arc::new(
            std::fs::read(directory.join("NotoSansJP-Bold.ttf")).unwrap(),
        )),
    ))
    .unwrap();
    let bytes = std::fs::read(core.join("public/fixtures/grid-operations-workbook.xlsx")).unwrap();
    Workbook::open(&bytes, "UTC", backend).unwrap()
}

#[test]
fn pasting_excel_text_keeps_quoted_cells_whole() {
    let mut book = book();
    let at = CellRef::new(8, 1);
    let clipboard = "\"line one\nline two\"\tplain\r\n\"say \"\"hi\"\"\"\t\"a\tb\"\r\n";
    let far = book.paste(at, clipboard).unwrap();
    assert_eq!(far, Some(CellRef::new(9, 2)));
    assert_eq!(book.edit_text(CellRef::new(8, 1)), "line one\nline two");
    assert_eq!(book.edit_text(CellRef::new(9, 1)), "plain");
    assert_eq!(book.edit_text(CellRef::new(8, 2)), "say \"hi\"");
    assert_eq!(book.edit_text(CellRef::new(9, 2)), "a\tb");
    assert_eq!(book.edit_text(CellRef::new(8, 3)), "");
}

#[test]
fn copying_quotes_what_excel_would_split() {
    let mut book = book();
    book.paste(CellRef::new(8, 1), "\"two\nlines\"\tsay \"hi\"\tplain\n")
        .unwrap();
    let text = book.range_text(span(CellRef::new(8, 1), CellRef::new(10, 1)));
    assert_eq!(text, "\"two\nlines\"\t\"say \"\"hi\"\"\"\tplain\n");
    // And it comes back in as it went out.
    book.paste(CellRef::new(8, 5), &text).unwrap();
    assert_eq!(book.edit_text(CellRef::new(8, 5)), "two\nlines");
    assert_eq!(book.edit_text(CellRef::new(9, 5)), "say \"hi\"");
}

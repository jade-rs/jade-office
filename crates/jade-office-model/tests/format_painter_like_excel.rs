//! Home ▸ Format Painter on a sheet: the formats of one selection given to
//! the next, tiled over a larger one, values left as they are, one undo
//! step.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::sheet::{span, CellRef, FontToggle, Workbook};
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
fn the_painter_gives_the_next_selection_the_formats_and_keeps_its_values() {
    let mut book = book();
    let source = CellRef::new(6, 1);
    book.set_cell(source, "1").unwrap();
    book.toggle(span(source, source), source, FontToggle::Bold)
        .unwrap();
    assert_eq!(book.format_at(source).bold, Some(true));
    for row in 1..=3 {
        book.set_cell(CellRef::new(7, row), &row.to_string())
            .unwrap();
    }

    let target = span(CellRef::new(7, 1), CellRef::new(7, 3));
    assert!(book.copy_format(span(source, source), target).unwrap());
    for row in 1..=3 {
        let at = CellRef::new(7, row);
        assert_eq!(book.format_at(at).bold, Some(true));
        assert_eq!(book.edit_text(at), row.to_string());
    }
    // One step takes it all back.
    assert!(book.undo().unwrap());
    assert_ne!(book.format_at(CellRef::new(7, 2)).bold, Some(true));
}

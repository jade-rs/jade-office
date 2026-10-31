//! A filter button's Top 10 and Reapply, as Excel's do: only the rows
//! with the largest (or smallest) numbers showing, and the filter run
//! again over values changed since.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::grid::Direction;
use jade_office_model::sheet::{span, CellRef, FilterButton, FilterButtonTarget, Workbook};
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

/// The rows showing below the header, walking down with the arrow.
fn showing(book: &Workbook<TinySkiaBackend>, header: CellRef, last: u32) -> Vec<String> {
    let mut rows = Vec::new();
    let mut at = header;
    loop {
        let next = book.step(at, Direction::Down);
        if next == at || next.row > last {
            return rows;
        }
        rows.push(book.display_text(next));
        at = next;
    }
}

#[test]
fn top_and_bottom_show_the_largest_and_smallest_and_reapply_follows_edits() {
    let mut book = book();
    let header = CellRef::new(20, 99);
    book.set_cell(header, "Score").unwrap();
    let scores = ["40", "10", "70", "30", "90", "20"];
    for (offset, score) in scores.iter().enumerate() {
        book.set_cell(CellRef::new(20, 100 + offset as u32), score)
            .unwrap();
    }
    let range = span(header, CellRef::new(20, 105));
    book.toggle_filter(range).unwrap();
    let button = FilterButton {
        target: FilterButtonTarget::Worksheet,
        column: 0,
        reference: header,
        range,
        active: false,
        rect: jade_xlsx_layout::Rect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        },
    };

    assert!(book.filter_top(&button, 2, true).unwrap());
    assert_eq!(showing(&book, header, 105), ["70", "90"]);
    assert!(book.filter_top(&button, 2, false).unwrap());
    assert_eq!(showing(&book, header, 105), ["10", "20"]);

    // 10 becomes 95: the bottom two are 20 and 30 once reapplied.
    book.set_cell(CellRef::new(20, 101), "95").unwrap();
    book.reapply_filter(&button).unwrap();
    assert_eq!(showing(&book, header, 105), ["30", "20"]);
}

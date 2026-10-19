//! HYPERLINK and ENCODEURL, typed into a cell, show what Excel shows.

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
fn a_link_shows_its_friendly_name_and_a_url_encodes_as_in_excel() {
    let mut book = book();
    let cases = [
        ("=HYPERLINK(\"https://example.com\",\"サイト\")", "サイト"),
        ("=HYPERLINK(\"https://example.com\")", "https://example.com"),
        ("=ENCODEURL(\"東京 駅\")", "%E6%9D%B1%E4%BA%AC%20%E9%A7%85"),
    ];
    for (row, (formula, shown)) in cases.iter().enumerate() {
        let at = CellRef::new(8, 10 + row as u32);
        book.set_cell(at, formula).unwrap();
        assert_eq!(book.display_text(at), *shown, "{formula}");
    }
}

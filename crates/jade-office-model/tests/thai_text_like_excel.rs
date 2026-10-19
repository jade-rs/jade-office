//! BAHTTEXT, typed into a cell, writes the amount in Thai words as Excel
//! does — over a cell's number too, as on an invoice.

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
fn an_amount_reads_in_thai_words_and_follows_its_cell() {
    let mut book = book();
    // Microsoft's own example.
    let typed = CellRef::new(8, 10);
    book.set_cell(typed, "=BAHTTEXT(1234)").unwrap();
    assert_eq!(book.display_text(typed), "หนึ่งพันสองร้อยสามสิบสี่บาทถ้วน");

    let amount = CellRef::new(8, 11);
    let words = CellRef::new(9, 11);
    book.set_cell(amount, "1250.75").unwrap();
    book.set_cell(words, "=BAHTTEXT(I12)").unwrap();
    assert_eq!(book.display_text(words), "หนึ่งพันสองร้อยห้าสิบบาทเจ็ดสิบห้าสตางค์");
    book.set_cell(amount, "21").unwrap();
    assert_eq!(book.display_text(words), "ยี่สิบเอ็ดบาทถ้วน");
}

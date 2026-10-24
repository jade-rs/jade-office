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

#[test]
fn hyperlink_dialog_writes_named_destinations_and_removal_keeps_text_in_one_undo() {
    use jade_office_model::{
        links::{LinkDialog, LinkField},
        sheet::{CellRange, Link},
    };
    let mut book = book();
    let at = CellRef::parse("H2").unwrap();
    let range = CellRange::parse("H2").unwrap();
    book.define_range_name("Destination", CellRange::parse("B3:C4").unwrap())
        .unwrap();
    let mut dialog = LinkDialog::open(&book, range, at);
    assert!(dialog
        .places
        .iter()
        .any(|(_, target)| target == "#Destination"));
    dialog.field_mut().replace(None, "#Destination", false);
    dialog.focus(LinkField::Label);
    dialog.field_mut().replace(None, "移動", false);
    dialog.focus(LinkField::Tooltip);
    dialog.field_mut().replace(None, "案内 & 確認", false);
    let link = dialog.rule().unwrap();
    book.insert_hyperlink(&link, dialog.label()).unwrap();
    assert_eq!(book.display_text(at), "移動");
    assert_eq!(book.hover_text(at), Some("案内 & 確認"));
    assert_eq!(
        book.hyperlink(at).unwrap().tooltip.as_deref(),
        Some("案内 & 確認")
    );
    assert_eq!(
        book.link(at),
        Some(Link::Place {
            sheet: book.sheet_index(),
            range: CellRange::parse("B3:C4").unwrap()
        })
    );
    book.undo().unwrap();
    assert!(book.hyperlink(at).is_none());
    assert_eq!(book.display_text(at), "");
    book.redo().unwrap();
    book.clear_hyperlinks(&[range]).unwrap();
    assert_eq!(book.display_text(at), "移動");
    book.undo().unwrap();
    assert!(book.hyperlink(at).is_some());
}
#[test]
fn hyperlink_dialog_keeps_numeric_types_formulas_and_existing_external_subaddresses() {
    use jade_office_model::{
        links::{LinkDialog, LinkField},
        sheet::{CellRange, Link},
    };
    let mut book = book();
    let at = CellRef::parse("H2").unwrap();
    let range = CellRange::parse("H2").unwrap();
    book.set_cell(at, "42").unwrap();
    let mut dialog = LinkDialog::open(&book, range, at);
    dialog
        .field_mut()
        .replace(None, "https://example.com", false);
    assert_eq!(dialog.label(), None);
    book.insert_hyperlink(&dialog.rule().unwrap(), dialog.label())
        .unwrap();
    assert_eq!(book.number(at), Some(42.));
    book.set_cell(at, "=1+2").unwrap();
    let mut dialog = LinkDialog::open(&book, range, at);
    dialog.focus(LinkField::Label);
    dialog.field_mut().replace(None, "保持", false);
    book.insert_hyperlink(&dialog.rule().unwrap(), dialog.label())
        .unwrap();
    assert_eq!(book.edit_text(at), "=1+2");
    assert_eq!(book.display_text(at), "3");
    let mut link = dialog.rule().unwrap();
    link.target = Some("../external.xlsx".into());
    link.location = Some("'Sheet 2'!B3".into());
    book.insert_hyperlink(&link, None).unwrap();
    let dialog = LinkDialog::open(&book, range, at);
    let saved = dialog.rule().unwrap();
    assert_eq!(saved.target, link.target);
    assert_eq!(saved.location, link.location);
    assert_eq!(
        book.link(at),
        Some(Link::Url("../external.xlsx#'Sheet 2'!B3".into()))
    );
}

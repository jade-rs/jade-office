#![cfg(feature = "tiny-skia")]
use jade_office_model::{
    aliased_faces,
    sheet::{CellRange, CellRef, Workbook},
    tiny::TinySkiaBackend,
};
use std::sync::Arc;
fn at(value: &str) -> CellRef {
    CellRef::parse(value).unwrap()
}
fn range(value: &str) -> CellRange {
    CellRange::parse(value).unwrap()
}
fn fixture() -> Vec<u8> {
    let mut p = jade_opc::OpcPackage::open(include_bytes!(
        "../../../jade-core/public/fixtures/sample-workbook.xlsx"
    ))
    .unwrap();
    p.replace("xl/worksheets/sheet1.xml", br#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1"><v>1</v></c><c r="B1"><v>2</v></c><c r="D1"><v>9</v></c></row><row r="2"><c r="A2"><v>3</v></c></row></sheetData><sheetProtection sheet="1"/><protectedRanges><protectedRange name="Input" sqref="A1"/></protectedRanges></worksheet>"#.to_vec());
    p.write().unwrap()
}
fn open(bytes: &[u8]) -> Workbook<TinySkiaBackend> {
    let font = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../jade-core/fonts/Noto_Sans_JP/static/NotoSansJP-Regular.ttf"
    ))
    .unwrap();
    Workbook::open(
        bytes,
        "UTC",
        TinySkiaBackend::new(&aliased_faces(Arc::new(font), None)).unwrap(),
    )
    .unwrap()
}
#[test]
fn refused_tiled_text_paste_keeps_the_origin_and_creates_no_history() {
    let mut b = open(&fixture());
    let before = b.save_bytes().unwrap();
    let revision = b.layout_revision();
    assert!(b.paste_over(range("A1:A2"), "42").is_err());
    assert_eq!(b.number(at("A1")), Some(1.));
    assert!(
        b.save_bytes().unwrap() == before,
        "refused paste changed package bytes"
    );
    assert_eq!(b.layout_revision(), revision);
    assert!(!b.is_dirty());
    assert!(!b.undo().unwrap());
}
#[test]
fn refused_fill_with_keeps_the_active_cell_and_creates_no_history() {
    let mut b = open(&fixture());
    let before = b.save_bytes().unwrap();
    let revision = b.layout_revision();
    assert!(b.fill_with(range("A1:A2"), at("A1"), "42").is_err());
    assert_eq!(b.number(at("A1")), Some(1.));
    assert!(
        b.save_bytes().unwrap() == before,
        "refused fill changed package bytes"
    );
    assert_eq!(b.layout_revision(), revision);
    assert!(!b.is_dirty());
    assert!(!b.undo().unwrap());
}

fn with_ranges(sqref: &str) -> Vec<u8> {
    let mut p = jade_opc::OpcPackage::open(&fixture()).unwrap();
    let xml = p
        .required_part_str("xl/worksheets/sheet1.xml")
        .unwrap()
        .replace("sqref=\"A1\"", &format!("sqref=\"{sqref}\""));
    p.replace("xl/worksheets/sheet1.xml", xml.into_bytes());
    p.write().unwrap()
}
fn unprotected() -> Vec<u8> {
    let mut p = jade_opc::OpcPackage::open(&fixture()).unwrap();
    let xml = p
        .required_part_str("xl/worksheets/sheet1.xml")
        .unwrap()
        .replace("<sheetProtection sheet=\"1\"/>", "");
    p.replace("xl/worksheets/sheet1.xml", xml.into_bytes());
    p.write().unwrap()
}
#[test]
fn refused_tiled_internal_copy_preserves_the_frozen_clipboard() {
    let mut b = open(&fixture());
    let copied = b.copy(range("B1"), false);
    let marked = b.copied();
    let before = b.save_bytes().unwrap();
    let revision = b.layout_revision();
    assert!(b.paste_over(range("A1:A2"), &copied).is_err());
    assert_eq!(b.number(at("A1")), Some(1.));
    assert_eq!(b.copied(), marked);
    assert_eq!(b.copied_text(), Some(copied));
    assert!(b.save_bytes().unwrap() == before);
    assert_eq!(b.layout_revision(), revision);
    assert!(!b.is_dirty());
    assert!(!b.undo().unwrap());
}
#[test]
fn refused_cut_retains_a_retryable_clipboard_and_does_not_clear_its_source() {
    let mut b = open(&with_ranges("A1 D1"));
    let copied = b.copy(range("A1"), true);
    let before = b.save_bytes().unwrap();
    assert!(b.paste_over(range("A2"), &copied).is_err());
    assert_eq!(b.copied_text(), Some(copied.clone()));
    assert!(b.save_bytes().unwrap() == before);
    assert_eq!(
        b.paste_over(range("D1"), &copied).unwrap(),
        Some(range("D1"))
    );
    assert_eq!(b.number(at("D1")), Some(1.));
    assert_eq!(b.number(at("A1")), None);
    assert!(b.copied().is_none());
    assert!(b.undo().unwrap());
    assert!(b.save_bytes().unwrap() == before);
    assert!(b.redo().unwrap());
    assert_eq!(b.number(at("D1")), Some(1.));
}
#[test]
fn cutting_a_locked_source_to_an_editable_target_is_refused_atomically() {
    let mut b = open(&fixture());
    let copied = b.copy(range("B1"), true);
    let before = b.save_bytes().unwrap();
    let revision = b.layout_revision();
    assert!(b.paste_over(range("A1"), &copied).is_err());
    assert!(b.save_bytes().unwrap() == before);
    assert_eq!(b.number(at("B1")), Some(2.));
    assert_eq!(b.number(at("A1")), Some(1.));
    assert_eq!(b.copied_text(), Some(copied));
    assert_eq!(b.layout_revision(), revision);
    assert!(!b.is_dirty());
}
#[test]
fn failed_compound_edits_keep_existing_redo_and_grouped_undo() {
    for paste in [false, true] {
        let mut b = open(&fixture());
        b.set_cell(at("A1"), "7").unwrap();
        assert!(b.undo().unwrap());
        let before = b.save_bytes().unwrap();
        let revision = b.layout_revision();
        let result = if paste {
            b.paste_over(range("A1:A2"), "42").map(|_| true)
        } else {
            b.fill_with(range("A1:A2"), at("A1"), "42")
        };
        assert!(result.is_err());
        assert!(b.save_bytes().unwrap() == before);
        assert_eq!(b.layout_revision(), revision);
        assert!(b.redo().unwrap());
        assert_eq!(b.number(at("A1")), Some(7.));
        assert!(b.undo().unwrap());
        assert_eq!(b.number(at("A1")), Some(1.));
        assert!(!b.undo().unwrap());
    }
}
#[test]
fn successful_tiled_paste_remains_one_undo_and_reopens_with_all_targets() {
    let mut b = open(&unprotected());
    let before = b.save_bytes().unwrap();
    assert_eq!(
        b.paste_over(range("A1:B2"), "42").unwrap(),
        Some(range("A1:B2"))
    );
    for cell in ["A1", "B1", "A2", "B2"] {
        assert_eq!(b.number(at(cell)), Some(42.));
    }
    let saved = b.save_bytes().unwrap();
    let reopened = open(&saved);
    assert_eq!(reopened.number(at("B2")), Some(42.));
    assert!(b.undo().unwrap());
    assert!(b.save_bytes().unwrap() == before);
    assert!(!b.undo().unwrap());
    assert!(b.redo().unwrap());
    assert_eq!(b.number(at("B2")), Some(42.));
}
#[test]
fn successful_fill_keeps_percentage_formats_in_the_same_undo_group() {
    let mut b = open(&unprotected());
    let before = b.save_bytes().unwrap();
    assert!(b.fill_with(range("A1:B2"), at("A1"), "15%").unwrap());
    for cell in ["A1", "B1", "A2", "B2"] {
        assert_eq!(b.number(at(cell)), Some(0.15));
        assert_eq!(b.display_text(at(cell)), "15%");
    }
    assert!(b.undo().unwrap());
    assert!(b.save_bytes().unwrap() == before);
    assert!(!b.undo().unwrap());
    assert!(b.redo().unwrap());
    assert_eq!(b.display_text(at("B2")), "15%");
}
#[test]
fn late_size_refusal_and_invalid_selections_preserve_the_original_document() {
    let mut b = open(&unprotected());
    let before = b.save_bytes().unwrap();
    let revision = b.layout_revision();
    assert!(b.fill_with(range("A1:A200001"), at("A1"), "42").is_err());
    assert!(b.paste_over(range("A1:A200001"), "42").is_err());
    let reversed = CellRange {
        start: at("B2"),
        end: at("A1"),
    };
    assert!(b.fill_with(reversed, at("B2"), "42").is_err());
    assert!(b.paste_over(reversed, "42").is_err());
    assert!(b.fill_with(range("A1"), at("B1"), "42").is_err());
    assert!(b.save_bytes().unwrap() == before);
    assert_eq!(b.layout_revision(), revision);
    assert!(!b.is_dirty());
    assert!(!b.undo().unwrap());
}
#[test]
fn failed_operations_keep_unsaved_password_grants_and_paint_change_notifications() {
    let mut p = jade_opc::OpcPackage::open(&fixture()).unwrap();
    let xml = p
        .required_part_str("xl/worksheets/sheet1.xml")
        .unwrap()
        .replace(
            "name=\"Input\" sqref=\"A1\"",
            "name=\"Input\" sqref=\"A1\" password=\"83AF\"",
        );
    p.replace("xl/worksheets/sheet1.xml", xml.into_bytes());
    let mut b = open(&p.write().unwrap());
    b.unlock_editable_range("Input", "password").unwrap();
    let before = b.save_bytes().unwrap();
    b.take_changes();
    let size = b.sheet_size();
    let revision = b.layout_revision();
    assert!(b.fill_with(range("A1:A2"), at("A1"), "42").is_err());
    assert!(b.editable_range_unlocked("Input").unwrap());
    assert_eq!(b.take_changes(), Some(Vec::new()));
    assert_eq!(b.sheet_size(), size);
    assert_eq!(b.layout_revision(), revision);
    assert!(b.save_bytes().unwrap() == before);
    assert!(b.set_cell(at("A1"), "7").unwrap());
    assert_eq!(b.number(at("A1")), Some(7.));
}

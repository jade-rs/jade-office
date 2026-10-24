#![cfg(feature = "tiny-skia")]
use jade_office_model::{
    aliased_faces,
    protection::ProtectionDialog,
    sheet::{CellRef, Workbook},
    tiny::TinySkiaBackend,
};
use std::sync::Arc;
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
fn book() -> Workbook<TinySkiaBackend> {
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../jade-core/public/fixtures/objects-workbook.xlsx"
    ))
    .unwrap();
    let mut p = jade_opc::OpcPackage::open(&bytes).unwrap();
    let part = "xl/worksheets/sheet1.xml";
    let xml = p.required_part_str(part).unwrap().replacen("</sheetData>", "</sheetData><sheetProtection sheet=\"0\" objects=\"1\" scenarios=\"1\" password=\"83AF\"/>", 1);
    p.replace(part, xml.into_bytes());
    open(&p.write().unwrap())
}
#[test]
fn shared_object_and_note_commands_refuse_without_dirtying_but_cells_remain_editable() {
    let mut b = book();
    let before = b.save_bytes().unwrap();
    let revision = b.layout_revision();
    assert!(b
        .set_note(CellRef::new(3, 19), "forbidden", "author")
        .is_err());
    assert!(b.place_object(2, (10.0, 10.0, 100.0, 100.0)).is_err());
    assert!(b.delete_object(4).is_err());
    assert_eq!(b.save_bytes().unwrap(), before);
    assert!(!b.is_dirty());
    assert_eq!(b.layout_revision(), revision);
    assert!(!b.undo().unwrap());
    b.set_cell(CellRef::new(3, 19), "72").unwrap();
    assert!(b.is_dirty());
    let mut reopened = open(&b.save_bytes().unwrap());
    assert!(reopened.delete_object(2).is_err());
    assert!(!reopened.sheet_protection().unwrap().unwrap().protected);
}
#[test]
fn existing_sheet_dialog_authenticates_independent_locks_and_undo_restores_guards() {
    let mut b = book();
    let before = b.save_bytes().unwrap();
    let mut d = ProtectionDialog::open_sheet(&b).unwrap();
    assert!(d.unprotect);
    d.field_mut().insert("wrong");
    assert_eq!(d.apply(&mut b), Err("protection.incorrect"));
    assert_eq!(b.save_bytes().unwrap(), before);
    assert!(!b.is_dirty());
    assert!(!b.undo().unwrap());
    d.focus(0);
    d.field_mut().insert("password");
    assert!(d.apply(&mut b).unwrap());
    assert!(b.is_dirty());
    b.delete_object(2).unwrap();
    assert!(b.undo().unwrap());
    assert!(b.undo().unwrap());
    let before = b.save_bytes().unwrap();
    assert!(b.redo().unwrap());
    assert!(b.undo().unwrap());
    assert_eq!(b.save_bytes().unwrap(), before);
    assert!(b.delete_object(2).is_err());
    assert_eq!(b.save_bytes().unwrap(), before);
    assert!(b.redo().unwrap());
    assert!(b.sheet_protection().unwrap().unwrap().allowed.objects);
}

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
    let sheet = "xl/worksheets/sheet1.xml";
    let mut headless =
        jade_xlsx_headless_editor::HeadlessWorkbook::open_for_editing(&p.write().unwrap()).unwrap();
    headless
        .set_note("Data", "D20", "original", "original")
        .unwrap();
    p = jade_opc::OpcPackage::open(&headless.export_bytes().unwrap()).unwrap();
    let rels = jade_opc::parse_relationships(
        &p.required_part_str("xl/worksheets/_rels/sheet1.xml.rels")
            .unwrap(),
    )
    .unwrap();
    let part = jade_opc::resolve_target(sheet, rels.target_by_type_suffix("/vmlDrawing").unwrap());
    let xml = p.required_part_str(&part).unwrap().replace(
        "</x:ClientData>",
        "<x:Locked>False</x:Locked><x:LockText>False</x:LockText></x:ClientData>",
    );
    p.replace(&part, xml.into_bytes());
    let xml = p
        .required_part_str("xl/drawings/drawing1.xml")
        .unwrap()
        .replace("<xdr:clientData", "<xdr:clientData fLocksWithSheet=\"0\"");
    p.replace("xl/drawings/drawing1.xml", xml.into_bytes());
    let xml = p.required_part_str(sheet).unwrap().replacen(
        "</sheetData>",
        "</sheetData><sheetProtection sheet=\"1\" objects=\"1\"/>",
        1,
    );
    p.replace(sheet, xml.into_bytes());
    open(&p.write().unwrap())
}
#[test]
fn individually_unlocked_model_objects_move_delete_and_restore_through_history() {
    let mut b = book();
    assert!(!b.sheet_protection().unwrap().unwrap().allowed.objects);
    assert!(b.object_bounds(2).is_some());
    b.place_object(2, (10.0, 20.0, 100.0, 80.0)).unwrap();
    let bytes = b.save_bytes().unwrap();
    let p = jade_opc::OpcPackage::open(&bytes).unwrap();
    let source = jade_xlsx_reader::object_protection::drawing_source(
        &p.required_part_str("xl/drawings/drawing1.xml").unwrap(),
        2,
    )
    .unwrap()
    .unwrap();
    assert!(!source.locks.locked);
    let mut b = open(&bytes);
    b.delete_object(2).unwrap();
    assert!(b.is_dirty());
    assert!(b.object_bounds(2).is_none());
    assert!(b.undo().unwrap());
    assert!(b.object_bounds(2).is_some());
    assert!(b.redo().unwrap());
    assert!(b.object_bounds(2).is_none());
}
#[test]
fn existing_unlocked_note_text_is_editable_and_its_vml_remains_unchanged() {
    let mut b = book();
    let before = b.save_bytes().unwrap();
    let p = jade_opc::OpcPackage::open(&before).unwrap();
    let sheet = "xl/worksheets/sheet1.xml";
    let rels = jade_opc::parse_relationships(
        &p.required_part_str("xl/worksheets/_rels/sheet1.xml.rels")
            .unwrap(),
    )
    .unwrap();
    let part = jade_opc::resolve_target(sheet, rels.target_by_type_suffix("/vmlDrawing").unwrap());
    let vml = p.required_part_str(&part).unwrap();
    b.set_note(CellRef::new(3, 19), "new text", "author")
        .unwrap();
    assert_eq!(b.note(CellRef::new(3, 19)), Some("new text"));
    assert!(b.is_dirty());
    let saved = b.save_bytes().unwrap();
    let out = jade_opc::OpcPackage::open(&saved).unwrap();
    assert_eq!(out.required_part_str(&part).unwrap(), vml);
    let mut b = open(&saved);
    let before = b.save_bytes().unwrap();
    let revision = b.layout_revision();
    assert!(b
        .set_note(CellRef::new(3, 20), "new forbidden object", "author")
        .is_err());
    assert_eq!(b.save_bytes().unwrap(), before);
    assert_eq!(b.layout_revision(), revision);
    assert!(!b.is_dirty());
    assert!(!b.undo().unwrap());
    b.set_note(CellRef::new(3, 19), "", "author").unwrap();
    assert_eq!(b.note(CellRef::new(3, 19)), None);
}
#[test]
fn global_protection_dialog_and_undo_keep_individual_drawing_settings() {
    let mut b = book();
    let before = b.save_bytes().unwrap();
    let mut d = ProtectionDialog::open_sheet(&b).unwrap();
    assert!(d.unprotect);
    assert!(d.apply(&mut b).unwrap());
    assert!(b.undo().unwrap());
    assert_eq!(b.save_bytes().unwrap(), before);
    b.place_object(2, (20.0, 30.0, 100.0, 80.0)).unwrap();
    let saved = b.save_bytes().unwrap();
    let mut b = open(&saved);
    assert!(!b.sheet_protection().unwrap().unwrap().allowed.objects);
    b.delete_object(2).unwrap();
}

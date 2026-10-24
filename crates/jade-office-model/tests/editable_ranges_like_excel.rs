#![cfg(feature = "tiny-skia")]
use jade_office_model::{
    aliased_faces,
    editable_ranges::{parse_references, RangeAction},
    protection::{ProtectionDialog, ProtectionTarget, SheetProtection},
    sheet::{CellRange, CellRef, Workbook},
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
        "/../../jade-core/public/fixtures/sample-workbook.xlsx"
    ))
    .unwrap();
    let mut b = open(&bytes);
    b.select_sheet(1).unwrap();
    b
}
fn range(s: &str) -> CellRange {
    CellRange::parse(s).unwrap()
}
fn dialog(b: &Workbook<TinySkiaBackend>) -> ProtectionDialog {
    ProtectionDialog::open_ranges(b, range("A1:A2"), "Range").unwrap()
}
#[test]
fn range_dialog_confirms_masks_and_keeps_existing_password_until_explicit_change() {
    let mut b = book();
    let before = b.save_bytes().unwrap();
    let mut d = dialog(&b);
    assert_eq!(d.target, ProtectionTarget::Ranges);
    assert_eq!(d.visible_fields(), [0, 1, 2, 3]);
    d.field_mut().insert("Inputs");
    d.focus(1);
    d.field_mut().insert("A1:A2");
    d.focus(2);
    d.field_mut().insert("秘密🔒");
    assert!(!d.masked(2).contains("秘密"));
    assert_eq!(d.apply(&mut b), Err("protection.mismatch"));
    assert_eq!(b.save_bytes().unwrap(), before);
    d.focus(3);
    d.field_mut().insert("秘密🔒");
    assert!(d.apply(&mut b).unwrap());
    assert_eq!(d.visible_fields(), [0, 1]);
    let password = b.editable_ranges().unwrap()[0].passwords.clone();
    d.focus(1);
    d.field_mut().insert("A1:A3");
    assert!(d.apply(&mut b).unwrap());
    assert_eq!(b.editable_ranges().unwrap()[0].passwords, password);
    d.range_action(RangeAction::Password, &mut b).unwrap();
    assert_eq!(d.visible_fields(), [0, 1, 2, 3]);
    assert!(d.apply(&mut b).unwrap());
    assert!(!b.editable_ranges().unwrap()[0].requires_password());
}
#[test]
fn successful_session_authentication_and_relocking_do_not_dirty_or_save_grants() {
    let mut b = book();
    b.set_editable_range(None, "Inputs", &[range("A1:A2")], Some("secret"))
        .unwrap();
    b.protect_sheet("", SheetProtection::default()).unwrap();
    let bytes = b.save_bytes().unwrap();
    let mut b = open(&bytes);
    b.select_sheet(1).unwrap();
    let before = b.save_bytes().unwrap();
    assert!(!b.is_dirty());
    let mut d = dialog(&b);
    assert!(d.unprotect);
    assert_eq!(d.visible_fields(), [2]);
    d.field_mut().insert("wrong");
    assert_eq!(d.apply(&mut b), Err("protection.incorrect"));
    assert_eq!(b.save_bytes().unwrap(), before);
    assert!(!b.is_dirty());
    d.focus(2);
    d.field_mut().insert("secret");
    assert!(d.apply(&mut b).unwrap());
    assert!(b.editable_range_unlocked("Inputs").unwrap());
    assert!(!b.is_dirty());
    assert_eq!(b.save_bytes().unwrap(), before);
    assert!(!d.range_action(RangeAction::LockAll, &mut b).unwrap());
    assert!(!b.editable_range_unlocked("Inputs").unwrap());
    assert!(!b.is_dirty());
    assert_eq!(b.save_bytes().unwrap(), before);
    assert!(b.set_cell(CellRef::parse("A1").unwrap(), "42").is_err());
    d.focus(2);
    d.field_mut().insert("secret");
    d.apply(&mut b).unwrap();
    b.set_cell(CellRef::parse("A1").unwrap(), "42").unwrap();
    let saved = b.save_bytes().unwrap();
    let mut reopened = open(&saved);
    reopened.select_sheet(1).unwrap();
    assert!(!reopened.editable_range_unlocked("Inputs").unwrap());
}
#[test]
fn stale_sheet_or_document_changes_cannot_retarget_pending_range_actions() {
    let mut b = book();
    let mut d = dialog(&b);
    b.select_sheet(0).unwrap();
    let before = b.save_bytes().unwrap();
    assert_eq!(d.apply(&mut b), Err("protection.range_stale"));
    assert_eq!(b.save_bytes().unwrap(), before);
    b.select_sheet(1).unwrap();
    let mut d = dialog(&b);
    b.set_cell(CellRef::parse("C3").unwrap(), "42").unwrap();
    let before = b.save_bytes().unwrap();
    assert_eq!(d.apply(&mut b), Err("protection.range_stale"));
    assert_eq!(b.save_bytes().unwrap(), before);
}
#[test]
fn new_names_avoid_collisions_and_editor_focus_supports_whole_axes_and_multiple_areas() {
    let mut b = book();
    for name in ["Range1", "Range3"] {
        b.set_editable_range(None, name, &[range("A1")], None)
            .unwrap();
    }
    let mut d = dialog(&b);
    assert_eq!(d.field().text, "Range2");
    d.key("tab", true, false);
    assert_eq!(d.focused, 3);
    d.key("tab", false, false);
    assert_eq!(d.focused, 0);
    let parsed = parse_references("$A:$B, 2:3, $D$5:E6").unwrap();
    assert_eq!(parsed.len(), 3);
    assert_eq!(
        parsed[0].end.row,
        jade_xlsx_reader::model::MAX_ROW_COUNT - 1
    );
    assert_eq!(
        parsed[1].end.column,
        jade_xlsx_reader::model::MAX_COLUMN_COUNT - 1
    );
    for bad in ["", "B2:A1", "Other!A1", "A0", "A1 XFE1", "B:A"] {
        assert!(parse_references(bad).is_none(), "{bad}");
    }
}
#[test]
fn range_creation_delete_and_history_are_undoable_but_cancel_writes_nothing() {
    let mut b = book();
    let before = b.save_bytes().unwrap();
    let mut cancelled = dialog(&b);
    cancelled.field_mut().insert("Cancelled");
    drop(cancelled);
    assert_eq!(b.save_bytes().unwrap(), before);
    assert!(!b.is_dirty());
    let mut d = dialog(&b);
    d.field_mut().insert("Inputs");
    assert!(d.apply(&mut b).unwrap());
    let after = b.save_bytes().unwrap();
    assert_eq!(b.editable_ranges().unwrap().len(), 1);
    assert!(d.range_action(RangeAction::Delete, &mut b).unwrap());
    assert!(b.editable_ranges().unwrap().is_empty());
    b.undo().unwrap();
    assert_eq!(b.editable_ranges().unwrap().len(), 1);
    assert_eq!(b.save_bytes().unwrap(), after);
    b.undo().unwrap();
    assert!(b.editable_ranges().unwrap().is_empty());
    b.redo().unwrap();
    assert_eq!(b.editable_ranges().unwrap().len(), 1);
}
#[test]
fn invalid_titles_references_and_confirmation_leave_bytes_and_history_unchanged() {
    let mut b = book();
    let before = b.save_bytes().unwrap();
    let mut d = dialog(&b);
    d.field_mut().insert("1Invalid");
    assert_eq!(d.apply(&mut b), Err("protection.range_name"));
    assert_eq!(b.save_bytes().unwrap(), before);
    assert!(!b.is_dirty());
    d.focus(0);
    d.field_mut().insert("Inputs");
    d.focus(1);
    d.field_mut().insert("Other!A1");
    assert_eq!(d.apply(&mut b), Err("protection.range_reference"));
    assert_eq!(b.save_bytes().unwrap(), before);
    assert!(!b.is_dirty());
}
#[test]
fn authored_range_passwords_get_fresh_platform_salts_and_sheet_guards_remain() {
    let mut a = book();
    let mut b = book();
    a.set_editable_range(None, "Inputs", &[range("A1")], Some("secret"))
        .unwrap();
    b.set_editable_range(None, "Inputs", &[range("A1")], Some("secret"))
        .unwrap();
    let x = a.editable_ranges().unwrap();
    let y = b.editable_ranges().unwrap();
    assert_ne!(x[0].passwords[0].salt, y[0].passwords[0].salt);
    assert_ne!(x[0].passwords[0].hash, y[0].passwords[0].hash);
    a.protect_sheet("", SheetProtection::default()).unwrap();
    assert!(a
        .set_editable_range(None, "Other", &[range("B1")], None)
        .is_err());
    let mut d = dialog(&a);
    let before = a.save_bytes().unwrap();
    assert!(!d.range_action(RangeAction::Delete, &mut a).unwrap());
    assert_eq!(a.save_bytes().unwrap(), before);
}

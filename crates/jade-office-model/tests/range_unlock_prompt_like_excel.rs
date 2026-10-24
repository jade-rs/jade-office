#![cfg(feature = "tiny-skia")]
use jade_office_model::{
    aliased_faces,
    editable_ranges::RangeAction,
    protection::ProtectionDialog,
    sheet::{CellRef, Workbook},
    tiny::TinySkiaBackend,
};
use std::sync::Arc;
fn at(s: &str) -> CellRef {
    CellRef::parse(s).unwrap()
}
fn fixture(extra: &str) -> Vec<u8> {
    let bytes = include_bytes!("../../../jade-core/public/fixtures/sample-workbook.xlsx");
    let mut p = jade_opc::OpcPackage::open(bytes).unwrap();
    let part = "xl/worksheets/sheet1.xml";
    let xml=p.required_part_str(part).unwrap().replace("</worksheet>",&format!(r#"<sheetProtection sheet="1"/><protectedRanges><protectedRange name="Inputs" sqref="A1:A2" password="83AF"/></protectedRanges>{extra}</worksheet>"#));
    p.replace(part, xml.into_bytes());
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
fn dialog(b: &Workbook<TinySkiaBackend>, value: &str) -> ProtectionDialog {
    ProtectionDialog::for_entry(b, at("A1"), value)
        .unwrap()
        .unwrap()
}
fn password(d: &mut ProtectionDialog, value: &str) {
    d.focus(2);
    d.field_mut().insert(value);
}
#[test]
fn wrong_cancel_and_authentication_preserve_the_entry_and_only_the_commit_adds_history() {
    let mut b = open(&fixture(""));
    let before = b.save_bytes().unwrap();
    let revision = b.layout_revision();
    let old = b.edit_text(at("A1"));
    assert!(b.set_cell(at("A1"), "42").is_err());
    let mut d = dialog(&b, "42");
    assert_eq!(d.entry_reference().as_deref(), Some("A1"));
    assert!(d.matches_entry(&b, at("A1"), "42"));
    assert_eq!(d.visible_fields(), [2]);
    password(&mut d, "wrong");
    assert!(!d.masked(2).contains("wrong"));
    assert_eq!(
        d.apply_for_entry(&mut b, at("A1"), "42"),
        Err("protection.incorrect")
    );
    assert_eq!(b.save_bytes().unwrap(), before);
    assert_eq!(b.layout_revision(), revision);
    assert!(!b.is_dirty());
    drop(d);
    assert!(b.set_cell(at("A1"), "42").is_err());
    let mut d = dialog(&b, "42");
    password(&mut d, "password");
    assert!(d.apply_for_entry(&mut b, at("A1"), "42").unwrap());
    assert!(!b.is_dirty());
    assert_eq!(b.layout_revision(), revision);
    assert_eq!(b.save_bytes().unwrap(), before);
    assert!(b.set_cell(at("A1"), "42").unwrap());
    assert!(b.is_dirty());
    assert_eq!(b.number(at("A1")), Some(42.));
    assert!(b.undo().unwrap());
    assert_eq!(b.edit_text(at("A1")), old);
    assert!(b.redo().unwrap());
    assert_eq!(b.number(at("A1")), Some(42.));
    let saved = b.save_bytes().unwrap();
    let reopened = open(&saved);
    assert!(ProtectionDialog::for_entry(&reopened, at("A1"), "43")
        .unwrap()
        .is_some());
}
#[test]
fn alternatives_are_limited_to_the_target_and_allow_any_one_covering_range() {
    let mut p = jade_opc::OpcPackage::open(&fixture("")).unwrap();
    let part = "xl/worksheets/sheet1.xml";
    let xml=p.required_part_str(part).unwrap().replace("</protectedRanges>",r#"<protectedRange name="Other" sqref="C1" password="83AF"/><protectedRange name="Second" sqref="A1" password="83AF"/><protectedRange name="Broken" sqref="A1" password="ZZZZ"/></protectedRanges>"#);
    p.replace(part, xml.into_bytes());
    let mut b = open(&p.write().unwrap());
    let before = b.save_bytes().unwrap();
    let mut d = dialog(&b, "42");
    assert_eq!(
        d.range_editor
            .as_ref()
            .unwrap()
            .entries
            .iter()
            .map(|e| e.name.as_str())
            .collect::<Vec<_>>(),
        ["Inputs", "Second"]
    );
    d.range_editor.as_mut().unwrap().selected = Some("Other".into());
    password(&mut d, "password");
    assert_eq!(
        d.apply_for_entry(&mut b, at("A1"), "42"),
        Err("protection.range_stale")
    );
    assert!(!b.editable_range_unlocked("Other").unwrap());
    d.range_action(RangeAction::Select(1), &mut b).unwrap();
    password(&mut d, "password");
    d.apply_for_entry(&mut b, at("A1"), "42").unwrap();
    assert!(b.editable_range_unlocked("Second").unwrap());
    assert!(!b.editable_range_unlocked("Inputs").unwrap());
    assert!(!b.editable_range_unlocked("Other").unwrap());
    assert!(!d.range_action(RangeAction::LockAll, &mut b).unwrap());
    assert!(b.editable_range_unlocked("Second").unwrap());
    assert_eq!(b.save_bytes().unwrap(), before);
    assert!(b.set_cell(at("A1"), "42").unwrap());
    assert!(b.set_cell(at("A2"), "42").is_err());
}
#[test]
fn a_different_cell_or_edited_text_cannot_authenticate_the_pending_request() {
    let mut b = open(&fixture(""));
    let before = b.save_bytes().unwrap();
    let mut d = dialog(&b, "42");
    password(&mut d, "password");
    for (cell, value) in [(at("A2"), "42"), (at("A1"), "43")] {
        assert_eq!(
            d.apply_for_entry(&mut b, cell, value),
            Err("protection.range_stale")
        );
        assert!(!b.editable_range_unlocked("Inputs").unwrap());
        assert_eq!(b.save_bytes().unwrap(), before);
    }
    assert!(d.apply_for_entry(&mut b, at("A1"), "42").unwrap());
}
#[test]
fn replacement_books_sheet_roundtrips_and_protection_history_make_the_prompt_stale() {
    let bytes = fixture("");
    let mut original = open(&bytes);
    let mut d = dialog(&original, "42");
    password(&mut d, "password");
    let mut replacement = open(&bytes);
    assert_eq!(original.layout_revision(), replacement.layout_revision());
    assert_eq!(
        d.apply_for_entry(&mut replacement, at("A1"), "42"),
        Err("protection.range_stale")
    );
    assert!(!replacement.editable_range_unlocked("Inputs").unwrap());
    original.select_sheet(1).unwrap();
    original.select_sheet(0).unwrap();
    assert_eq!(
        d.apply_for_entry(&mut original, at("A1"), "42"),
        Err("protection.range_stale")
    );
    let mut d = dialog(&original, "42");
    password(&mut d, "password");
    original.unprotect_sheet("").unwrap();
    original.undo().unwrap();
    let before = original.save_bytes().unwrap();
    assert_eq!(
        d.apply_for_entry(&mut original, at("A1"), "42"),
        Err("protection.range_stale")
    );
    assert_eq!(original.save_bytes().unwrap(), before);
    assert!(!original.editable_range_unlocked("Inputs").unwrap());
}
#[test]
fn successful_authentication_does_not_bypass_normal_validation() {
    let rules = r#"<dataValidations count="1"><dataValidation type="whole" operator="between" showErrorMessage="1" sqref="A1"><formula1>1</formula1><formula2>10</formula2></dataValidation></dataValidations>"#;
    let mut b = open(&fixture(rules));
    let mut d = dialog(&b, "17");
    password(&mut d, "password");
    d.apply_for_entry(&mut b, at("A1"), "17").unwrap();
    let before = b.save_bytes().unwrap();
    let revision = b.layout_revision();
    let error = b.set_cell(at("A1"), "17").unwrap_err();
    assert!(jade_office_model::sheet::validation_refusal(&error).is_some());
    assert_eq!(b.save_bytes().unwrap(), before);
    assert_eq!(b.layout_revision(), revision);
    assert!(!b.is_dirty());
    assert!(b.set_cell(at("A1"), "7").unwrap());
}
#[test]
fn successful_authentication_does_not_bypass_formula_syntax_checks() {
    let mut b = open(&fixture(""));
    let mut d = dialog(&b, "=SUM(");
    password(&mut d, "password");
    d.apply_for_entry(&mut b, at("A1"), "=SUM(").unwrap();
    let before = b.save_bytes().unwrap();
    let revision = b.layout_revision();
    assert!(b.set_cell(at("A1"), "=SUM(").is_err());
    assert_eq!(b.save_bytes().unwrap(), before);
    assert_eq!(b.layout_revision(), revision);
    assert!(!b.is_dirty());
    assert!(b.set_cell(at("A1"), "=1+2").unwrap());
    assert_eq!(b.number(at("A1")), Some(3.));
}
#[test]
fn unprotected_uncovered_and_already_authenticated_cells_have_no_password_prompt() {
    let mut b = open(&fixture(""));
    assert!(ProtectionDialog::for_entry(&b, at("B1"), "42")
        .unwrap()
        .is_none());
    b.unlock_editable_range("Inputs", "password").unwrap();
    assert!(ProtectionDialog::for_entry(&b, at("A1"), "42")
        .unwrap()
        .is_none());
    b.lock_editable_ranges().unwrap();
    b.unprotect_sheet("").unwrap();
    assert!(ProtectionDialog::for_entry(&b, at("A1"), "42")
        .unwrap()
        .is_none());
}

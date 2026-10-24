#![cfg(feature = "tiny-skia")]

use jade_office_model::{
    aliased_faces,
    sheet::{CellRef, SheetVisibility, Workbook},
    tiny::TinySkiaBackend,
};
use jade_opc::OpcPackage;
use std::sync::Arc;

fn book(attributes: &str) -> Workbook<TinySkiaBackend> {
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../jade-core/public/fixtures/sample-workbook.xlsx"
    ))
    .unwrap();
    let mut p = OpcPackage::open(&bytes).unwrap();
    let xml = p.required_part_str("xl/workbook.xml").unwrap();
    p.replace(
        "xl/workbook.xml",
        xml.replace(
            "<sheets>",
            &format!("<workbookProtection {attributes}/><sheets>"),
        )
        .into_bytes(),
    );
    let font = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../jade-core/fonts/Noto_Sans_JP/static/NotoSansJP-Regular.ttf"
    ))
    .unwrap();
    let backend = TinySkiaBackend::new(&aliased_faces(Arc::new(font), None)).unwrap();
    Workbook::open(&p.write().unwrap(), "UTC", backend).unwrap()
}

#[test]
fn protected_structure_refusals_preserve_the_selected_sheet_package_and_history() {
    let mut book = book(r#"lockStructure="1" workbookPassword="ABCD""#);
    assert!(book.structure_protected());
    book.select_sheet(1).unwrap();
    let names = book.sheet_names();
    let before = book.save_bytes().unwrap();
    for error in [
        book.add_sheet().unwrap_err(),
        book.delete_sheet(1).unwrap_err(),
        book.rename_sheet(1, "Other").unwrap_err(),
        book.move_sheet(1, 0).unwrap_err(),
        book.set_sheet_visibility(1, SheetVisibility::Hidden)
            .unwrap_err(),
    ] {
        assert!(error.contains("workbook structure is protected"), "{error}");
    }
    assert_eq!(book.sheet_index(), 1);
    assert_eq!(book.sheet_names(), names);
    assert!(!book.is_dirty());
    assert!(!book.undo().unwrap());
    assert!(!book.redo().unwrap());
    assert_eq!(book.save_bytes().unwrap(), before);
}

#[test]
fn structure_protection_does_not_prevent_cell_edits_or_consume_redo() {
    let mut book = book(r#"lockStructure="true""#);
    book.select_sheet(1).unwrap();
    book.set_cell(CellRef::parse("C3").unwrap(), "42").unwrap();
    assert!(book.undo().unwrap());
    assert!(book.delete_sheet(1).is_err());
    assert!(book.redo().unwrap());
    assert!(book.structure_protected());
    let bytes = book.save_bytes().unwrap();
    let reopened = jade_xlsx_reader::read_xlsx(&bytes).unwrap();
    assert!(reopened.structure_protected);
    assert_eq!(
        reopened.sheets[1]
            .rows
            .iter()
            .flat_map(|row| &row.cells)
            .find(|cell| cell.reference == CellRef::parse("C3").unwrap())
            .unwrap()
            .value,
        jade_xlsx_reader::CellValue::Number(42.0)
    );
}

#[test]
fn structure_controls_stay_available_when_only_windows_or_a_password_are_set() {
    for attributes in [
        r#"lockStructure="false" lockWindows="1""#,
        r#"workbookPassword="ABCD""#,
    ] {
        let mut book = book(attributes);
        assert!(!book.structure_protected());
        assert!(book.move_sheet(1, 0).unwrap());
        assert!(book.rename_sheet(0, "Other").unwrap());
        assert!(book
            .set_sheet_visibility(0, SheetVisibility::Hidden)
            .unwrap());
        assert!(book
            .set_sheet_visibility(0, SheetVisibility::Visible)
            .unwrap());
        book.add_sheet().unwrap();
        assert!(book.delete_sheet(1).unwrap());
    }
}

#[test]
fn shared_password_dialog_confirms_masks_and_authenticates_without_losing_state() {
    use jade_office_model::protection::ProtectionDialog;
    let mut book = book("");
    book.select_sheet(1).unwrap();
    let before = book.save_bytes().unwrap();
    let mut dialog = ProtectionDialog::open(&book).unwrap();
    assert!(!dialog.unprotect);
    dialog.field_mut().insert("秘密🔒");
    assert!(!dialog.masked(0).contains("秘密"));
    assert_eq!(dialog.apply(&mut book), Err("protection.mismatch"));
    assert_eq!(book.save_bytes().unwrap(), before);
    assert!(!book.is_dirty());
    dialog.key("tab", false, false);
    assert_eq!(dialog.focused, 1);
    dialog.field_mut().insert("秘密🔒");
    assert!(dialog.apply(&mut book).unwrap());
    assert!(book.structure_protected());
    let saved = book.save_bytes().unwrap();
    assert!(!OpcPackage::open(&saved)
        .unwrap()
        .required_part_str("xl/workbook.xml")
        .unwrap()
        .contains("秘密"));
    let mut dialog = ProtectionDialog::open(&book).unwrap();
    assert!(dialog.unprotect);
    dialog.focus(1);
    assert_eq!(dialog.focused, 0);
    dialog.field_mut().insert("wrong");
    assert_eq!(dialog.apply(&mut book), Err("protection.incorrect"));
    assert_eq!(book.save_bytes().unwrap(), saved);
    assert_eq!(book.sheet_index(), 1);
    dialog.focus(0);
    dialog.field_mut().insert("秘密🔒");
    assert!(dialog.apply(&mut book).unwrap());
    assert!(!book.structure_protected());
    assert!(book.rename_sheet(1, "Other").unwrap());
}

#[test]
fn authoring_history_restores_passwords_and_guarding_and_rejected_attempts_keep_redo() {
    let mut book = book("");
    assert!(book.protect_workbook("secret").unwrap());
    assert!(book.unprotect_workbook("secret").unwrap());
    assert!(book.undo().unwrap());
    assert!(book.structure_protected());
    let before = book.save_bytes().unwrap();
    assert!(book.unprotect_workbook("wrong").is_err());
    assert_eq!(book.save_bytes().unwrap(), before);
    assert!(book.redo().unwrap());
    assert!(!book.structure_protected());
    assert!(!book.unprotect_workbook("").unwrap());
    assert!(book.undo().unwrap());
    assert!(book.structure_protected());
    assert!(book.unprotect_workbook("secret").unwrap());
}

#[test]
fn new_protections_use_distinct_platform_salts_and_oversized_passwords_are_atomic() {
    let mut a = book("");
    let mut b = book("");
    let before = a.save_bytes().unwrap();
    assert!(a.protect_workbook(&"x".repeat(256)).is_err());
    assert_eq!(a.save_bytes().unwrap(), before);
    a.protect_workbook("same").unwrap();
    b.protect_workbook("same").unwrap();
    let ax = OpcPackage::open(&a.save_bytes().unwrap())
        .unwrap()
        .required_part_str("xl/workbook.xml")
        .unwrap();
    let bx = OpcPackage::open(&b.save_bytes().unwrap())
        .unwrap()
        .required_part_str("xl/workbook.xml")
        .unwrap();
    let salt = |s: &str| {
        s.split("workbookSaltValue=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .to_string()
    };
    assert_ne!(salt(&ax), salt(&bx));
    assert!(a.unprotect_workbook("same").unwrap());
    assert!(b.unprotect_workbook("same").unwrap());
}

#[test]
fn passwordless_dialog_cancellation_and_unsupported_imports_leave_the_workbook_intact() {
    use jade_office_model::protection::ProtectionDialog;
    let mut book = book("");
    let before = book.save_bytes().unwrap();
    let mut dialog = ProtectionDialog::open(&book).unwrap();
    dialog.field_mut().insert("cancelled");
    drop(dialog);
    assert_eq!(book.save_bytes().unwrap(), before);
    assert!(!book.undo().unwrap());
    let mut dialog = ProtectionDialog::open(&book).unwrap();
    assert!(dialog.apply(&mut book).unwrap());
    let mut dialog = ProtectionDialog::open(&book).unwrap();
    assert!(dialog.unprotect);
    assert!(dialog.apply(&mut book).unwrap());
    let mut unsupported = crate::book(r#"lockStructure="1" workbookHashValue="AA==""#);
    let before = unsupported.save_bytes().unwrap();
    let mut dialog = ProtectionDialog::open(&unsupported).unwrap();
    assert_eq!(
        dialog.apply(&mut unsupported),
        Err("protection.unsupported")
    );
    assert_eq!(unsupported.save_bytes().unwrap(), before);
}

#[test]
fn historical_iso_attributes_require_authentication_in_the_shared_dialog() {
    use jade_office_model::protection::ProtectionDialog;
    let mut b = book(
        r#"lockStructure="1" algorithmName="SHA-256" hashValue="06re8Tqktrf4w+0PNGtDcBzfZLISFoXXF6Ci2Yxxf30=" saltValue="AAECAwQFBgcICQoLDA0ODw==" spinCount="3""#,
    );
    let before = b.save_bytes().unwrap();
    assert!(
        b.unprotect_workbook("wrong").is_err(),
        "generic ISO verifier was treated as passwordless"
    );
    assert_eq!(b.save_bytes().unwrap(), before);
    let mut d = ProtectionDialog::open(&b).unwrap();
    assert!(d.unprotect);
    d.field_mut().insert("密碼🔒");
    assert!(d.apply(&mut b).unwrap());
    assert!(!b.structure_protected());
}

#[test]
fn partial_historical_iso_metadata_never_uses_a_matching_legacy_password() {
    let mut b = book(r#"lockStructure="1" workbookPassword="83AF" hashValue="AA==""#);
    let before = b.save_bytes().unwrap();
    assert!(
        b.unprotect_workbook("password").is_err(),
        "partial generic ISO metadata fell back to legacy"
    );
    assert_eq!(b.save_bytes().unwrap(), before);
    assert!(b.structure_protected());
}

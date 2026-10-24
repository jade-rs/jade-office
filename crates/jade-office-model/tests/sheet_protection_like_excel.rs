#![cfg(feature = "tiny-skia")]
use jade_office_model::{
    aliased_faces,
    protection::{ProtectionDialog, ProtectionTarget, SheetProtection},
    sheet::{CellFormat, CellRef, Workbook},
    tiny::TinySkiaBackend,
};
use std::sync::Arc;
fn book() -> Workbook<TinySkiaBackend> {
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../jade-core/public/fixtures/sample-workbook.xlsx"
    ))
    .unwrap();
    let font = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../jade-core/fonts/Noto_Sans_JP/static/NotoSansJP-Regular.ttf"
    ))
    .unwrap();
    Workbook::open(
        &bytes,
        "UTC",
        TinySkiaBackend::new(&aliased_faces(Arc::new(font), None)).unwrap(),
    )
    .unwrap()
}
fn at(reference: &str) -> CellRef {
    CellRef::parse(reference).unwrap()
}
#[test]
fn sheet_dialog_confirms_permissions_authenticates_and_preserves_failed_state() {
    let mut b = book();
    b.select_sheet(1).unwrap();
    let before = b.save_bytes().unwrap();
    let mut d = ProtectionDialog::open_sheet(&b).unwrap();
    assert_eq!(d.target, ProtectionTarget::Sheet);
    assert_eq!(d.permissions().iter().filter(|x| **x).count(), 2);
    d.toggle_permission(2);
    assert!(d.allowed.format_cells);
    d.toggle_permission(99);
    d.field_mut().insert("秘密🔒");
    assert!(!d.masked(0).contains("秘密"));
    assert_eq!(d.apply(&mut b), Err("protection.mismatch"));
    assert_eq!(b.save_bytes().unwrap(), before);
    d.focus(1);
    d.field_mut().insert("秘密🔒");
    assert!(d.apply(&mut b).unwrap());
    assert!(b.sheet_protection().unwrap().unwrap().allowed.format_cells);
    assert!(!b.structure_protected());
    let saved = b.save_bytes().unwrap();
    let mut d = ProtectionDialog::open_sheet(&b).unwrap();
    assert!(d.unprotect);
    d.field_mut().insert("wrong");
    assert_eq!(d.apply(&mut b), Err("protection.incorrect"));
    assert_eq!(b.save_bytes().unwrap(), saved);
    d.focus(0);
    d.field_mut().insert("秘密🔒");
    assert!(d.apply(&mut b).unwrap());
    assert!(!b.unprotect_sheet("").unwrap());
}
#[test]
fn hidden_contents_stay_in_the_grid_and_package_and_reappear_when_unprotected() {
    let mut b = book();
    let cell = at("A1");
    b.set_cell(cell, "=1+2").unwrap();
    b.format(
        jade_xlsx_reader::CellRange {
            start: cell,
            end: cell,
        },
        &CellFormat {
            locked: Some(false),
            formula_hidden: Some(true),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(b.edit_text(cell), "=1+2");
    b.protect_sheet("secret", SheetProtection::default())
        .unwrap();
    assert!(b.contents_hidden(cell));
    assert_eq!(b.edit_text(cell), "");
    let bytes = b.save_bytes().unwrap();
    let read = jade_xlsx_reader::read_xlsx(&bytes).unwrap();
    let value = read.sheets[0]
        .rows
        .iter()
        .flat_map(|r| &r.cells)
        .find(|c| c.reference == cell)
        .unwrap();
    assert_eq!(value.formula.as_deref(), Some("1+2"));
    assert_eq!(value.value, jade_xlsx_reader::CellValue::Number(3.));
    b.unprotect_sheet("secret").unwrap();
    assert_eq!(b.edit_text(cell), "=1+2");
    assert!(!b.contents_hidden(cell));
    assert!(b.undo().unwrap());
    assert!(b.contents_hidden(cell));
    assert!(b.redo().unwrap());
    assert_eq!(b.edit_text(cell), "=1+2");
}
#[test]
fn selecting_a_different_sheet_does_not_retarget_an_open_password_dialog() {
    let mut b = book();
    let mut d = ProtectionDialog::open_sheet(&b).unwrap();
    b.select_sheet(1).unwrap();
    let before = b.save_bytes().unwrap();
    assert_eq!(d.apply(&mut b), Err("protection.unsupported"));
    assert_eq!(b.save_bytes().unwrap(), before);
    assert!(!b.is_dirty());
    let mut cancelled = ProtectionDialog::open_sheet(&b).unwrap();
    cancelled.field_mut().insert("cancelled");
    drop(cancelled);
    assert_eq!(b.save_bytes().unwrap(), before);
}
#[test]
fn sheet_authentication_history_preserves_redo_and_other_sheet_guards() {
    let mut b = book();
    b.protect_sheet("secret", SheetProtection::default())
        .unwrap();
    let saved = b.save_bytes().unwrap();
    assert!(b.set_cell(at("A1"), "changed").is_err());
    assert!(b.unprotect_sheet("wrong").is_err());
    assert_eq!(b.save_bytes().unwrap(), saved);
    assert!(b.undo().unwrap());
    assert!(b.sheet_protection().unwrap().is_none());
    assert!(b.redo().unwrap());
    assert!(b.sheet_protection().unwrap().unwrap().protected);
    b.select_sheet(1).unwrap();
    b.set_cell(at("C3"), "42").unwrap();
    assert!(!b.contents_hidden(at("C3")));
    b.select_sheet(0).unwrap();
    assert!(b.unprotect_sheet("secret").unwrap());
}
#[test]
fn protected_hidden_text_and_selection_use_effective_column_and_row_formats() {
    let mut b = book();
    let a = at("A1");
    b.set_cell(a, "private").unwrap();
    b.format(
        jade_xlsx_reader::CellRange { start: a, end: a },
        &CellFormat {
            locked: Some(false),
            formula_hidden: Some(true),
            ..Default::default()
        },
    )
    .unwrap();
    let column = jade_xlsx_reader::CellRange {
        start: at("D1"),
        end: CellRef::new(3, jade_xlsx_reader::model::MAX_ROW_COUNT - 1),
    };
    b.format(
        column,
        &CellFormat {
            locked: Some(false),
            formula_hidden: Some(true),
            ..Default::default()
        },
    )
    .unwrap();
    let row = jade_xlsx_reader::CellRange {
        start: at("A1001"),
        end: CellRef::new(jade_xlsx_reader::model::MAX_COLUMN_COUNT - 1, 1000),
    };
    b.format(
        row,
        &CellFormat {
            locked: Some(true),
            formula_hidden: Some(false),
            ..Default::default()
        },
    )
    .unwrap();
    b.protect_sheet(
        "",
        SheetProtection {
            select_locked_cells: false,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(b.can_select_cell(a));
    assert!(!b.can_select_cell(at("B1")));
    assert!(b.can_select_range(jade_xlsx_reader::CellRange { start: a, end: a }));
    assert!(!b.can_select_range(jade_xlsx_reader::CellRange {
        start: a,
        end: at("A2")
    }));
    assert!(!b.can_select_range(jade_xlsx_reader::CellRange {
        start: at("D1000"),
        end: at("D1002")
    }));
    assert!(b.can_select_range(jade_xlsx_reader::CellRange {
        start: at("D2000"),
        end: at("D1048576")
    }));

    assert!(b.can_select_cell(at("D1000")));
    assert!(b.contents_hidden(at("D1000")));
    assert!(!b.can_select_cell(at("D1001")));
    assert!(!b.contents_hidden(at("D1001")));
    assert_eq!(
        b.step(at("D1000"), jade_office_model::grid::Direction::Down),
        at("D1002")
    );

    assert!(b.contents_hidden(a));
    assert_eq!(b.edit_text(a), "");
    b.unprotect_sheet("").unwrap();
    assert_eq!(b.edit_text(a), "private");
    assert!(b.can_select_cell(at("B1")));
}

#[test]
fn restricted_arrows_jump_sparse_rows_and_both_disabled_flags_stay_put() {
    let mut b = book();
    let cell = at("A100000");
    b.format(
        jade_xlsx_reader::CellRange {
            start: cell,
            end: cell,
        },
        &CellFormat {
            locked: Some(false),
            ..Default::default()
        },
    )
    .unwrap();
    b.protect_sheet(
        "",
        SheetProtection {
            select_locked_cells: false,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        b.step(at("A1"), jade_office_model::grid::Direction::Down),
        cell
    );
    assert_eq!(b.step(cell, jade_office_model::grid::Direction::Down), cell);
    b.unprotect_sheet("").unwrap();
    b.protect_sheet(
        "",
        SheetProtection {
            select_locked_cells: false,
            select_unlocked_cells: false,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!b.can_select_cell(cell));
    assert_eq!(
        b.step(at("A1"), jade_office_model::grid::Direction::Down),
        at("A1")
    );
}

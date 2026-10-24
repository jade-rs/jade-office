#![cfg(feature = "tiny-skia")]
use jade_office_model::{
    aliased_faces,
    editable_ranges::RangeAction,
    protected_edits::{attempt, ProtectedEdit},
    protection::ProtectionDialog,
    sheet::{CellRange, CellRef, Workbook},
    tiny::TinySkiaBackend,
};
use std::sync::Arc;
fn at(s: &str) -> CellRef {
    CellRef::parse(s).unwrap()
}
fn range(s: &str) -> CellRange {
    CellRange::parse(s).unwrap()
}
fn fixture(ranges: &str, extra: &str) -> Vec<u8> {
    let mut p = jade_opc::OpcPackage::open(include_bytes!(
        "../../../jade-core/public/fixtures/sample-workbook.xlsx"
    ))
    .unwrap();
    p.replace("xl/worksheets/sheet1.xml",format!(r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1"><v>1</v></c><c r="B1"><v>2</v></c><c r="D1"><v>9</v></c></row><row r="2"><c r="A2"><v>3</v></c></row></sheetData><sheetProtection sheet="1"/><protectedRanges>{ranges}</protectedRanges>{extra}</worksheet>"#).into_bytes());
    p.write().unwrap()
}
const TWO: &str = r#"<protectedRange name="First" sqref="A1" password="83AF"/><protectedRange name="Second" sqref="A2" password="83AF"/>"#;
fn open(bytes: &[u8]) -> Workbook<TinySkiaBackend> {
    static FONT: std::sync::OnceLock<Arc<Vec<u8>>> = std::sync::OnceLock::new();
    let font = FONT.get_or_init(|| {
        Arc::new(
            std::fs::read(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../jade-core/fonts/Noto_Sans_JP/static/NotoSansJP-Regular.ttf"
            ))
            .unwrap(),
        )
    });
    Workbook::open(
        bytes,
        "UTC",
        TinySkiaBackend::new(&aliased_faces(font.clone(), None)).unwrap(),
    )
    .unwrap()
}
fn prompt(b: &mut Workbook<TinySkiaBackend>, edit: ProtectedEdit) -> ProtectionDialog {
    let mut dialog = None;
    assert!(attempt(b, &mut dialog, edit).is_err());
    let d = dialog.expect("actual shell path should open a password prompt");
    assert!(d.is_operation_prompt());
    assert!(d.is_authentication_prompt());
    assert_eq!(d.visible_fields(), [2]);
    d
}
fn password(
    d: &mut ProtectionDialog,
    b: &mut Workbook<TinySkiaBackend>,
    name: &str,
    value: &str,
    entry: Option<(CellRef, &str)>,
) -> Result<bool, &'static str> {
    let i = d
        .range_editor
        .as_ref()
        .unwrap()
        .entries
        .iter()
        .position(|e| e.name.ends_with(&format!(" — {name}")))
        .unwrap();
    d.range_action(RangeAction::Select(i), b).unwrap();
    d.focus(2);
    d.field_mut().insert(value);
    d.apply_for_operation(b, entry)
}
fn paste(target: &str, text: &str) -> ProtectedEdit {
    ProtectedEdit::Paste {
        target: range(target),
        text: text.into(),
    }
}
#[test]
fn tiled_paste_authenticates_all_required_ranges_before_mutating_any_state() {
    let mut b = open(&fixture(TWO, ""));
    let before = b.save_bytes().unwrap();
    let rev = b.layout_revision();
    let copied = b.copy(range("B1"), false);
    let mut d = prompt(&mut b, paste("A1:A2", "42"));
    assert_eq!(
        password(&mut d, &mut b, "First", "wrong", None),
        Err("protection.incorrect")
    );
    assert!(!password(&mut d, &mut b, "First", "password", None).unwrap());
    assert_eq!(d.range_editor.as_ref().unwrap().entries.len(), 1);
    assert_eq!(b.save_bytes().unwrap(), before);
    assert_eq!(b.layout_revision(), rev);
    assert!(!b.is_dirty());
    assert!(!b.editable_range_unlocked("First").unwrap());
    assert_eq!(b.copied_text(), Some(copied));
    assert!(password(&mut d, &mut b, "Second", "password", None).unwrap());
    let out = d.take_completed_operation().unwrap();
    assert_eq!(out.selection, Some(range("A1:A2")));
    assert_eq!(b.number(at("A1")), Some(42.));
    assert_eq!(b.number(at("A2")), Some(42.));
    assert!(b.editable_range_unlocked("First").unwrap());
    assert!(b.editable_range_unlocked("Second").unwrap());
    assert!(b.undo().unwrap());
    assert_eq!(b.number(at("A1")), Some(1.));
    assert_eq!(b.number(at("A2")), Some(3.));
    assert!(!b.undo().unwrap());
    assert!(b.redo().unwrap());
    assert_eq!(b.number(at("A2")), Some(42.));
    let reopened = open(&b.save_bytes().unwrap());
    assert!(!reopened.editable_range_unlocked("First").unwrap());
}
#[test]
fn cancel_drops_all_staged_passwords_without_changing_grants_or_redo() {
    let mut b = open(&fixture(TWO, ""));
    let before = b.save_bytes().unwrap();
    let mut d = prompt(&mut b, paste("A1:A2", "42"));
    password(&mut d, &mut b, "First", "password", None).unwrap();
    drop(d);
    assert_eq!(b.save_bytes().unwrap(), before);
    assert!(!b.is_dirty());
    assert!(!b.undo().unwrap());
    assert!(!b.editable_range_unlocked("First").unwrap());
    let d = prompt(&mut b, paste("A1:A2", "42"));
    assert_eq!(d.range_editor.as_ref().unwrap().entries.len(), 2);
}
#[test]
fn clear_checks_only_stored_values_and_formulas_even_on_complete_axes() {
    for r in ["A1:A9", "A1:A1048576"] {
        let mut b = open(&fixture(TWO, ""));
        let mut d = prompt(&mut b, ProtectedEdit::Clear { range: range(r) });
        assert_eq!(d.range_editor.as_ref().unwrap().entries.len(), 2);
        password(&mut d, &mut b, "First", "password", None).unwrap();
        password(&mut d, &mut b, "Second", "password", None).unwrap();
        assert_eq!(b.number(at("A1")), None);
        assert_eq!(b.number(at("A2")), None);
        assert_eq!(b.number(at("B1")), Some(2.));
        assert!(b.undo().unwrap());
        assert_eq!(b.number(at("A2")), Some(3.));
    }
}
#[test]
fn ragged_tsv_does_not_require_permission_for_unwritten_holes() {
    let ranges = r#"<protectedRange name="First" sqref="A1:B1" password="83AF"/><protectedRange name="Second" sqref="A2" password="83AF"/>"#;
    let mut b = open(&fixture(ranges, ""));
    let mut d = prompt(&mut b, paste("A1", "7\t8\n9"));
    password(&mut d, &mut b, "First", "password", None).unwrap();
    password(&mut d, &mut b, "Second", "password", None).unwrap();
    assert_eq!(b.number(at("A1")), Some(7.));
    assert_eq!(b.number(at("B1")), Some(8.));
    assert_eq!(b.number(at("A2")), Some(9.));
    assert_eq!(b.number(at("B2")), None);
}
#[test]
fn down_right_and_series_require_only_the_written_destination() {
    for (edit, dest, expected) in [
        (
            ProtectedEdit::FillDown {
                range: range("A1:A2"),
            },
            "A2",
            1.,
        ),
        (
            ProtectedEdit::FillRight {
                range: range("A1:B1"),
            },
            "B1",
            1.,
        ),
        (
            ProtectedEdit::FillSeries {
                source: range("A1:A2"),
                target: range("A1:A3"),
            },
            "A3",
            5.,
        ),
    ] {
        let ranges =
            format!(r#"<protectedRange name="Destination" sqref="{dest}" password="83AF"/>"#);
        let mut b = open(&fixture(&ranges, ""));
        let mut d = prompt(&mut b, edit);
        assert_eq!(d.range_editor.as_ref().unwrap().entries.len(), 1);
        password(&mut d, &mut b, "Destination", "password", None).unwrap();
        assert_eq!(b.number(at(dest)), Some(expected));
        assert_eq!(b.number(at("A1")), Some(1.));
    }
}
#[test]
fn internal_tiled_copy_can_read_a_locked_source_without_unlocking_it() {
    let mut b = open(&fixture(TWO, ""));
    let text = b.copy(range("B1"), false);
    let mut d = prompt(&mut b, paste("A1:A2", &text));
    password(&mut d, &mut b, "First", "password", None).unwrap();
    password(&mut d, &mut b, "Second", "password", None).unwrap();
    assert_eq!(b.number(at("A1")), Some(2.));
    assert_eq!(b.number(at("A2")), Some(2.));
    assert_eq!(b.number(at("B1")), Some(2.));
    assert_eq!(b.copied_text(), Some(text));
}
#[test]
fn fill_with_binds_the_original_entry_and_keeps_excel_bulk_validation_semantics() {
    let validation = r#"<dataValidations count="1"><dataValidation type="whole" operator="between" showErrorMessage="1" sqref="A2"><formula1>1</formula1><formula2>10</formula2></dataValidation></dataValidations>"#;
    let mut b = open(&fixture(TWO, validation));
    let before = b.save_bytes().unwrap();
    let rev = b.layout_revision();
    let mut d = prompt(
        &mut b,
        ProtectedEdit::FillWith {
            range: range("A1:A2"),
            active: at("A1"),
            text: "42".into(),
        },
    );
    assert_eq!(
        password(&mut d, &mut b, "First", "password", Some((at("A1"), "43"))),
        Err("protection.range_stale")
    );
    password(&mut d, &mut b, "First", "password", Some((at("A1"), "42"))).unwrap();
    // Microsoft: copied/filled cells do not display validation messages.
    // https://support.microsoft.com/en-us/excel/more-on-data-validation
    assert!(password(&mut d, &mut b, "Second", "password", Some((at("A1"), "42"))).unwrap());
    assert!(d.take_completed_operation().is_some());
    assert_eq!(b.number(at("A1")), Some(42.));
    assert_eq!(b.number(at("A2")), Some(42.));
    assert_ne!(b.save_bytes().unwrap(), before);
    assert_ne!(b.layout_revision(), rev);
    assert!(b.undo().unwrap());
    assert_eq!(b.number(at("A1")), Some(1.));
    assert_eq!(b.number(at("A2")), Some(3.));
}
#[test]
fn failed_final_writer_rolls_back_all_grants_source_clipboard_and_history() {
    let mut b = open(&fixture(TWO, ""));
    let copied = b.copy(range("B1"), false);
    let before = b.save_bytes().unwrap();
    let revision = b.layout_revision();
    // Formula XML cannot encode NUL; permission checks run before the writer.
    let mut d = prompt(&mut b, paste("A1:A2", "=\0"));
    password(&mut d, &mut b, "First", "password", None).unwrap();
    assert!(password(&mut d, &mut b, "Second", "password", None).is_err());
    assert!(d.take_completed_operation().is_none());
    assert_eq!(b.save_bytes().unwrap(), before);
    assert_eq!(b.layout_revision(), revision);
    assert!(!b.is_dirty());
    assert_eq!(b.copied_text(), Some(copied));
    assert!(!b.editable_range_unlocked("First").unwrap());
    assert!(!b.editable_range_unlocked("Second").unwrap());
    assert!(!b.undo().unwrap());
}
#[test]
fn fill_with_can_reuse_an_unchanged_locked_active_cell_and_forgets_copy_on_success() {
    let ranges = r#"<protectedRange name="Destination" sqref="A2" password="83AF"/>"#;
    let mut b = open(&fixture(ranges, ""));
    b.copy(range("B1"), false);
    let mut d = prompt(
        &mut b,
        ProtectedEdit::FillWith {
            range: range("A1:A2"),
            active: at("A1"),
            text: "1".into(),
        },
    );
    password(
        &mut d,
        &mut b,
        "Destination",
        "password",
        Some((at("A1"), "1")),
    )
    .unwrap();
    assert_eq!(b.number(at("A2")), Some(1.));
    assert!(b.copied().is_none());
    assert!(b.undo().unwrap());
    assert_eq!(b.number(at("A2")), Some(3.));
}
#[test]
fn move_authenticates_both_ends_but_copy_does_not_require_source_permission() {
    let ranges = r#"<protectedRange name="First" sqref="A1" password="83AF"/><protectedRange name="Destination" sqref="D1" password="83AF"/>"#;
    let mut b = open(&fixture(ranges, ""));
    let mut d = prompt(
        &mut b,
        ProtectedEdit::MoveRange {
            source: range("A1"),
            to: at("D1"),
            copy: false,
        },
    );
    password(&mut d, &mut b, "First", "password", None).unwrap();
    assert_eq!(b.number(at("A1")), Some(1.));
    password(&mut d, &mut b, "Destination", "password", None).unwrap();
    let outcome = d.take_completed_operation().unwrap();
    assert_eq!(outcome.active_cell(at("A1")), at("D1"));
    assert_eq!(b.number(at("A1")), None);
    assert_eq!(b.number(at("D1")), Some(1.));
    assert!(b.undo().unwrap());
    assert_eq!(b.number(at("A1")), Some(1.));
    let mut b = open(&fixture(
        r#"<protectedRange name="Destination" sqref="D1" password="83AF"/>"#,
        "",
    ));
    let mut d = prompt(
        &mut b,
        ProtectedEdit::MoveRange {
            source: range("A1"),
            to: at("D1"),
            copy: true,
        },
    );
    password(&mut d, &mut b, "Destination", "password", None).unwrap();
    assert_eq!(b.number(at("A1")), Some(1.));
    assert_eq!(b.number(at("D1")), Some(1.));
}
#[test]
fn cut_across_sheets_disambiguates_equal_range_names_and_commits_once() {
    let mut p = jade_opc::OpcPackage::open(&fixture(
        r#"<protectedRange name="Same" sqref="A1" password="83AF"/>"#,
        "",
    ))
    .unwrap();
    p.replace("xl/worksheets/sheet2.xml",br#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/><sheetProtection sheet="1"/><protectedRanges><protectedRange name="Same" sqref="B1" password="83AF"/></protectedRanges></worksheet>"#.to_vec());
    let mut b = open(&p.write().unwrap());
    let text = b.copy(range("A1"), true);
    b.select_sheet(1).unwrap();
    let before = b.save_bytes().unwrap();
    let mut d = prompt(&mut b, paste("B1", &text));
    let entries = &d.range_editor.as_ref().unwrap().entries;
    assert_eq!(entries.len(), 2);
    assert_ne!(entries[0].key, entries[1].key);
    assert_ne!(entries[0].name, entries[1].name);
    d.focus(2);
    d.field_mut().insert("password");
    assert!(!d.apply_for_operation(&mut b, None).unwrap());
    assert_eq!(b.save_bytes().unwrap(), before);
    assert_eq!(b.copied_text(), Some(text));
    d.focus(2);
    d.field_mut().insert("password");
    assert!(d.apply_for_operation(&mut b, None).unwrap());
    assert_eq!(b.number(at("B1")), Some(1.));
    assert!(b.copied().is_none());
    assert!(b.undo().unwrap());
    assert_eq!(b.number(at("B1")), None);
    b.select_sheet(0).unwrap();
    assert_eq!(b.number(at("A1")), Some(1.));
}
#[test]
fn clipboard_replacement_navigation_and_replacement_workbooks_invalidate_the_request() {
    let bytes = fixture(TWO, "");
    let mut b = open(&bytes);
    let text = b.copy(range("B1"), false);
    let mut d = prompt(&mut b, paste("A1:A2", &text));
    password(&mut d, &mut b, "First", "password", None).unwrap();
    b.copy(range("D1"), false);
    let before = b.save_bytes().unwrap();
    assert_eq!(
        d.apply_for_operation(&mut b, None),
        Err("protection.range_stale")
    );
    assert_eq!(b.save_bytes().unwrap(), before);
    assert!(!b.editable_range_unlocked("First").unwrap());
    let mut d = prompt(&mut b, paste("A1:A2", "42"));
    let mut replacement = open(&bytes);
    assert_eq!(
        d.apply_for_operation(&mut replacement, None),
        Err("protection.range_stale")
    );
    b.select_sheet(1).unwrap();
    b.select_sheet(0).unwrap();
    assert_eq!(
        d.apply_for_operation(&mut b, None),
        Err("protection.range_stale")
    );
}
#[test]
fn partially_coverable_or_invalid_operations_do_not_offer_password_prompts() {
    let mut b = open(&fixture(TWO, ""));
    let before = b.save_bytes().unwrap();
    for edit in [
        paste("A1:A3", "42"),
        ProtectedEdit::MoveRange {
            source: range("A1"),
            to: at("XFD1048576"),
            copy: false,
        },
        ProtectedEdit::FillWith {
            range: range("A1"),
            active: at("A2"),
            text: "9".into(),
        },
    ] {
        let mut d = None;
        assert!(attempt(&mut b, &mut d, edit).is_err());
        assert!(d.is_none());
        assert_eq!(b.save_bytes().unwrap(), before);
    }
    let mut d = None;
    assert!(
        !attempt(
            &mut b,
            &mut d,
            ProtectedEdit::Clear {
                range: range("Z1:Z1048576")
            }
        )
        .unwrap()
        .changed
    );
    assert!(d.is_none());
}
#[test]
fn cancelling_a_partial_authentication_preserves_an_existing_redo_branch() {
    let ranges = format!(r#"{TWO}<protectedRange name="Free" sqref="D1"/>"#);
    let mut b = open(&fixture(&ranges, ""));
    assert!(b.set_cell(at("D1"), "17").unwrap());
    assert!(b.undo().unwrap());
    let before = b.save_bytes().unwrap();
    let dirty = b.is_dirty();
    let revision = b.layout_revision();
    let mut d = prompt(&mut b, paste("A1:A2", "42"));
    password(&mut d, &mut b, "First", "password", None).unwrap();
    drop(d);
    assert_eq!(b.save_bytes().unwrap(), before);
    assert_eq!(b.is_dirty(), dirty);
    assert_eq!(b.layout_revision(), revision);
    assert!(b.redo().unwrap());
    assert_eq!(b.number(at("D1")), Some(17.));
    assert!(!b.editable_range_unlocked("First").unwrap());
}
#[test]
fn overlapping_alternatives_disappear_after_one_covering_range_is_verified() {
    let ranges = format!(
        r#"{TWO}<protectedRange name="Alternative" sqref="A1" password="83AF"/><protectedRange name="Irrelevant" sqref="D1" password="83AF"/>"#
    );
    let mut b = open(&fixture(&ranges, ""));
    let mut d = prompt(&mut b, paste("A1:A2", "42"));
    assert_eq!(d.range_editor.as_ref().unwrap().entries.len(), 3);
    password(&mut d, &mut b, "Alternative", "password", None).unwrap();
    assert_eq!(d.range_editor.as_ref().unwrap().entries.len(), 1);
    password(&mut d, &mut b, "Second", "password", None).unwrap();
    assert!(b.editable_range_unlocked("Alternative").unwrap());
    assert!(!b.editable_range_unlocked("First").unwrap());
    assert!(!b.editable_range_unlocked("Irrelevant").unwrap());
}
#[test]
fn live_grant_changes_invalidate_a_pending_bulk_request_without_a_layout_change() {
    let mut b = open(&fixture(TWO, ""));
    let mut d = prompt(&mut b, paste("A1:A2", "42"));
    password(&mut d, &mut b, "First", "password", None).unwrap();
    let revision = b.layout_revision();
    b.unlock_editable_range("Second", "password").unwrap();
    assert_eq!(b.layout_revision(), revision);
    assert_eq!(
        d.apply_for_operation(&mut b, None),
        Err("protection.range_stale")
    );
    assert!(!b.editable_range_unlocked("First").unwrap());
    b.lock_editable_ranges().unwrap();
    assert_eq!(
        d.apply_for_operation(&mut b, None),
        Err("protection.range_stale")
    );
    assert_eq!(b.number(at("A1")), Some(1.));
}

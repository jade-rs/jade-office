#![cfg(feature = "tiny-skia")]
use jade_office_model::{
    aliased_faces,
    sheet::{CellRef, Workbook},
    tiny::TinySkiaBackend,
};
use std::sync::Arc;
const FIXTURE: &[u8] = include_bytes!("../../../jade-core/public/fixtures/pivot-cache-compat.xlsx");
fn at(s: &str) -> CellRef {
    CellRef::parse(s).unwrap()
}
fn open() -> Workbook<TinySkiaBackend> {
    open_bytes(FIXTURE)
}
fn open_bytes(bytes: &[u8]) -> Workbook<TinySkiaBackend> {
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
fn authored_aggregation_recalculates_queries_relayouts_and_undoes_as_one_model_edit() {
    use jade_office_model::pivots::{PivotMeasure, PivotUpdate};
    let mut b = open();
    b.select_sheet(1).unwrap();
    b.set_cell(at("H10"), r#"=GETPIVOTDATA("Amount",E3)"#)
        .unwrap();
    let before = b.save_bytes().unwrap();
    let revision = b.layout_revision();
    b.update_pivot_named(
        "RegionalSales",
        &PivotUpdate {
            measures: Some(vec![PivotMeasure {
                existing: Some(0),
                field: 1,
                aggregation: "average".into(),
                caption: "Average".into(),
                number_format_id: Some(164),
            }]),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(b.number(at("F4")), Some(7.5));
    assert_eq!(b.number(at("F5")), Some(13.5));
    assert_eq!(b.number(at("H10")), Some(10.5));
    assert!(b.layout_revision() > revision);
    let after = b.save_bytes().unwrap();
    assert!(b.undo().unwrap());
    assert_eq!(b.save_bytes().unwrap(), before);
    assert!(b.redo().unwrap());
    assert_eq!(b.save_bytes().unwrap(), after);
}

#[test]
fn shared_pivot_dialog_builds_only_changed_settings_and_applies_them_once() {
    use jade_office_model::pivots::{PivotArea, PivotDialog, PivotDialogField, PivotUpdate};
    let mut b = open();
    b.select_sheet(1).unwrap();
    let before = b.save_bytes().unwrap();
    let mut d = PivotDialog::open(&b, at("E3")).unwrap();
    assert_eq!(d.command().unwrap(), PivotUpdate::default());
    assert!(!d.apply(&mut b).unwrap());
    assert_eq!(b.save_bytes().unwrap(), before);
    d.select_field(2);
    d.set_area(PivotArea::Rows);
    d.select_field(0);
    d.focus(PivotDialogField::FieldCaption);
    d.field_mut().insert("Area");
    let change = d.command().unwrap();
    assert!(change.layout.is_some());
    assert!(change.measures.is_none());
    assert_eq!(change.field_options.len(), 1);
    assert_eq!(change.field_options[0].caption.as_deref(), Some("Area"));
    assert!(change.field_options[0].subtotals.is_none());
    assert!(d.apply(&mut b).unwrap());
    let after = b.save_bytes().unwrap();
    let parsed = jade_xlsx_reader::read_xlsx(&after).unwrap();
    assert_eq!(parsed.pivots.tables[0].row_fields, [0, 2]);
    assert!(parsed.pivots.tables[0].page_fields.is_empty());
    assert_eq!(
        parsed.pivots.tables[0].fields[0].caption.as_deref(),
        Some("Area")
    );
    assert!(b.undo().unwrap());
    assert_eq!(b.save_bytes().unwrap(), before);
    assert!(b.redo().unwrap());
    assert_eq!(b.save_bytes().unwrap(), after);
}

#[test]
fn pivot_dialog_rejects_malformed_source_quotes_and_stale_measure_identities() {
    use jade_office_model::pivots::{PivotDialog, PivotDialogField, PivotMeasure, PivotUpdate};
    let mut b = open();
    b.select_sheet(1).unwrap();
    let before = b.save_bytes().unwrap();
    let mut d = PivotDialog::open(&b, at("E3")).unwrap();
    d.focus(PivotDialogField::Source);
    d.field_mut().insert("'!A1:C5");
    assert_eq!(d.apply(&mut b).unwrap_err(), "pivot.invalid_source");
    assert_eq!(b.save_bytes().unwrap(), before);
    let mut d = PivotDialog::open(&b, at("E3")).unwrap();
    b.update_pivot_named(
        "RegionalSales",
        &PivotUpdate {
            measures: Some(vec![PivotMeasure {
                existing: Some(0),
                field: 1,
                aggregation: "average".into(),
                caption: "Average".into(),
                number_format_id: Some(164),
            }]),
            ..Default::default()
        },
    )
    .unwrap();
    let after = b.save_bytes().unwrap();
    let revision = b.layout_revision();
    assert_eq!(d.apply(&mut b).unwrap_err(), "pivot.stale");
    assert_eq!(b.save_bytes().unwrap(), after);
    assert_eq!(b.layout_revision(), revision);
}
#[test]
fn pivot_dialog_refuses_reused_member_indices_without_losing_the_redo_branch() {
    use jade_office_model::pivots::{PivotDialog, PivotDialogField};
    let mut original = open();
    original.select_sheet(1).unwrap();
    let mut dialog = PivotDialog::open(&original, at("E3")).unwrap();
    dialog.focus(PivotDialogField::MeasureCaption);
    dialog.field_mut().insert("Pending caption");

    // A remote replacement/Undo can reuse cache index zero with a different
    // value while retaining the same field names and report configuration.
    let mut package = jade_xlsx_reader::OpcPackage::open(FIXTURE).unwrap();
    let parsed = jade_xlsx_reader::read_xlsx(FIXTURE).unwrap();
    let part = &parsed.pivots.caches[0].part_name;
    let xml = package.required_part_str(part).unwrap();
    assert!(xml.contains("East"));
    package.replace(part, xml.replace("East", "North").into_bytes());
    let mut changed = open_bytes(&package.write().unwrap());
    changed.select_sheet(1).unwrap();
    changed.set_cell(at("H10"), "Keep redo").unwrap();
    assert!(changed.undo().unwrap());
    let before = changed.save_bytes().unwrap();
    let revision = changed.layout_revision();
    assert_eq!(dialog.apply(&mut changed).unwrap_err(), "pivot.stale");
    assert_eq!(
        dialog.refresh_items(&mut changed).unwrap_err(),
        "pivot.stale"
    );
    assert_eq!(changed.save_bytes().unwrap(), before);
    assert_eq!(changed.layout_revision(), revision);
    assert!(changed.redo().unwrap());
}

#[test]
fn refreshing_member_choices_uses_real_new_indices_and_keeps_unapplied_field_settings() {
    use jade_office_model::pivots::{PivotArea, PivotDialog, PivotDialogField};
    let mut b = open();
    b.select_sheet(1).unwrap();
    let mut d = PivotDialog::open(&b, at("E3")).unwrap();
    d.select_field(1);
    assert!(d.members[1].is_empty());
    d.set_area(PivotArea::Rows);
    d.focus(PivotDialogField::FieldCaption);
    d.field_mut().insert("Unit amount");
    b.select_sheet(0).unwrap();
    b.set_cell(at("B2"), "100").unwrap();
    b.select_sheet(1).unwrap();
    let before_refresh = b.save_bytes().unwrap();
    assert!(d.refresh_items(&mut b).unwrap());
    assert_eq!(d.area(1), PivotArea::Rows);
    assert_eq!(d.value(PivotDialogField::FieldCaption), "Unit amount");
    assert!(d.members[1].iter().any(|m| m.label == "100"));
    let member = d.members[1].iter().find(|m| m.label == "20").unwrap().index;
    d.all_members(false);
    d.toggle_member(member);
    let after_refresh = b.save_bytes().unwrap();
    assert!(d.apply(&mut b).unwrap());
    let after = b.save_bytes().unwrap();
    let parsed = jade_xlsx_reader::read_xlsx(&after).unwrap();
    let t = &parsed.pivots.tables[0];
    assert_eq!(t.fields[1].caption.as_deref(), Some("Unit amount"));
    assert_eq!(b.number(t.location.reference.end), Some(20.));
    assert!(b.undo().unwrap());
    assert_eq!(b.save_bytes().unwrap(), after_refresh);
    assert!(b.undo().unwrap());
    assert_eq!(b.save_bytes().unwrap(), before_refresh);
}
#[test]
fn shared_refresh_command_updates_saved_queries_and_round_trips_through_history() {
    let mut b = open();
    b.select_sheet(0).unwrap();
    b.set_cell(at("B2"), "100").unwrap();
    b.select_sheet(1).unwrap();
    assert!(b.pivot_at(at("F4")).is_some());
    assert!(b.pivot_at(at("H10")).is_none());
    b.set_cell(at("H10"), r#"=GETPIVOTDATA("Amount",E3)"#)
        .unwrap();
    assert_eq!(b.number(at("H10")), Some(42.));
    assert!(b.refresh_pivot_at(at("F4")).unwrap());
    assert_eq!(b.number(at("H10")), Some(132.));
    assert_eq!(b.number(at("F4")), Some(105.));
    let saved = b.save_bytes().unwrap();
    let parsed = jade_xlsx_reader::read_xlsx(&saved).unwrap();
    assert!(parsed.pivots.issues.is_empty());
    assert_eq!(
        parsed.pivots.caches[0].record_value(0, 1),
        Some(&jade_xlsx_reader::pivot::PivotScalar::Number(100.))
    );
    assert!(b.undo().unwrap());
    assert_eq!(b.number(at("H10")), Some(42.));
    assert!(b.redo().unwrap());
    assert_eq!(b.number(at("H10")), Some(132.));
    assert_eq!(b.save_bytes().unwrap(), saved);
}
#[test]
fn refused_refresh_keeps_saved_state_layout_revision_and_the_existing_undo_branch() {
    let mut b = open();
    b.select_sheet(0).unwrap();
    b.set_cell(at("A3"), "North").unwrap();
    b.select_sheet(1).unwrap();
    let before_keep = b.save_bytes().unwrap();
    b.set_cell(at("F7"), "Keep").unwrap();
    let before = b.save_bytes().unwrap();
    let revision = b.layout_revision();
    assert!(b.refresh_pivot_at(at("F4")).is_err());
    assert_eq!(b.layout_revision(), revision);
    assert_eq!(b.save_bytes().unwrap(), before);
    assert_eq!(b.number(at("F6")), Some(42.));
    assert!(b.undo().unwrap());
    assert_eq!(b.save_bytes().unwrap(), before_keep);
}

#[test]
fn refresh_all_is_available_away_from_reports_and_updates_one_shared_history_step() {
    let mut b = open();
    assert!(b.has_pivots());
    b.select_sheet(0).unwrap();
    b.set_cell(at("B2"), "100").unwrap();
    let before = b.save_bytes().unwrap();
    let revision = b.layout_revision();
    assert!(b.refresh_all_pivots().unwrap());
    assert!(b.layout_revision() > revision);
    assert_eq!(b.sheet_index(), 0);
    let after = b.save_bytes().unwrap();
    let parsed = jade_xlsx_reader::read_xlsx(&after).unwrap();
    assert_eq!(
        parsed.pivots.caches[0].record_value(0, 1),
        Some(&jade_xlsx_reader::pivot::PivotScalar::Number(100.))
    );
    b.select_sheet(1).unwrap();
    assert_eq!(b.number(at("F6")), Some(132.));
    assert!(b.undo().unwrap());
    assert_eq!(b.save_bytes().unwrap(), before);
    assert!(b.redo().unwrap());
    assert_eq!(b.save_bytes().unwrap(), after);
}

#[test]
fn refused_refresh_all_keeps_model_revision_and_saved_bytes() {
    let mut b = open();
    b.select_sheet(0).unwrap();
    b.set_cell(at("A3"), "North").unwrap();
    b.select_sheet(1).unwrap();
    b.set_cell(at("F7"), "Keep").unwrap();
    let before = b.save_bytes().unwrap();
    let revision = b.layout_revision();
    assert!(b.refresh_all_pivots().is_err());
    assert_eq!(b.layout_revision(), revision);
    assert_eq!(b.save_bytes().unwrap(), before);
}

#[test]
fn a_pivot_source_cycle_refuses_without_dirtying_the_shared_model() {
    let mut b = open();
    b.select_sheet(0).unwrap();
    b.set_cell(at("B2"), "=Report!F4").unwrap();
    let before = b.save_bytes().unwrap();
    let revision = b.layout_revision();
    assert!(b.refresh_all_pivots().is_err());
    assert_eq!(b.layout_revision(), revision);
    assert_eq!(b.save_bytes().unwrap(), before);
}

#[test]
fn refresh_fits_with_real_backend_fonts_and_keeps_all_widths_in_one_undo_step() {
    let mut b = open_bytes(include_bytes!(
        "../../../jade-core/public/fixtures/pivot-style-compat.xlsx"
    ));
    b.select_sheet(0).unwrap();
    b.set_cell(at("A2"), &"i".repeat(60)).unwrap();
    b.select_sheet(1).unwrap();
    b.set_cell(at("E40"), &"W".repeat(100)).unwrap();
    let before = b.save_bytes().unwrap();
    assert!(b.refresh_pivot_at(at("F4")).unwrap());
    let (_, _, width, _) = b.cell_rect(at("E4"), (0., 0.)).unwrap();
    // A real Noto Latin 'i' is much narrower than the generic half-em fallback.
    // The unrelated W text below the report must not drive this width either.
    assert!(width > 160. && width < 350., "actual fitted width {width}");
    let after = b.save_bytes().unwrap();
    let mut reopened = open_bytes(&after);
    reopened.select_sheet(1).unwrap();
    assert_eq!(reopened.cell_rect(at("E4"), (0., 0.)).unwrap().2, width);
    assert!(b.undo().unwrap());
    assert_eq!(b.save_bytes().unwrap(), before);
    assert!(b.redo().unwrap());
    assert_eq!(b.save_bytes().unwrap(), after);
    assert_eq!(b.cell_rect(at("E4"), (0., 0.)).unwrap().2, width);
}

#[test]
fn creation_dialog_creates_a_sheet_and_report_in_one_shared_history_step() {
    use jade_office_model::pivots::{PivotCreation, PivotCreationField, PivotDialog};
    let mut b = open();
    let before = b.save_bytes().unwrap();
    let mut d = PivotCreation::open(
        &b,
        jade_xlsx_reader::CellRange::parse("A1:C5").unwrap(),
        at("A1"),
    );
    d.focus(PivotCreationField::Name);
    d.field_mut().insert("収支_x0041_");
    d.focus(PivotCreationField::SheetName);
    d.field_mut().insert("分析_x0041_");
    let created = d.apply(&mut b).unwrap();
    assert_eq!(created.sheet, "分析_x0041_");
    assert_eq!(b.sheet_name(), "分析_x0041_");
    assert_eq!(b.number(at("B6")), Some(42.));
    assert!(PivotDialog::open(&b, at("A3")).is_ok());
    let saved = b.save_bytes().unwrap();
    assert!(b.undo().unwrap());
    assert_eq!(b.save_bytes().unwrap(), before);
    assert!(b.redo().unwrap());
    assert_eq!(b.save_bytes().unwrap(), saved);
}

#[test]
fn creation_panel_preserves_text_focus_and_a_refused_existing_destination() {
    use jade_office_model::pivots::{PivotCreation, PivotCreationField, PivotPanel};
    let mut b = open();
    let before = b.save_bytes().unwrap();
    let mut d = PivotCreation::open(
        &b,
        jade_xlsx_reader::CellRange::parse("A1:C5").unwrap(),
        at("A1"),
    );
    d.new_sheet = false;
    d.focus(PivotCreationField::Destination);
    d.field_mut().insert("'Report'!E3");
    let mut panel = PivotPanel::create(d);
    panel.key("tab", false, false);
    assert_eq!(panel.creator().unwrap().focus, PivotCreationField::Source);
    panel.key("tab", true, false);
    assert_eq!(
        panel.creator().unwrap().focus,
        PivotCreationField::Destination
    );
    panel.field_mut().replace(None, "'Report'!E3", true);
    assert!(panel.field().marked.is_some());
    assert!(panel.creator().unwrap().apply(&mut b).is_err());
    assert_eq!(b.save_bytes().unwrap(), before);
}

#[test]
fn the_creation_dialog_cannot_open_a_hidden_destination_as_the_active_sheet() {
    use jade_office_model::pivots::{PivotCreation, PivotCreationField};
    let mut p = jade_xlsx_reader::OpcPackage::open(FIXTURE).unwrap();
    let body = p
        .required_part_str("xl/workbook.xml")
        .unwrap()
        .replace("name=\"Report\"", "name=\"Report\" state=\"hidden\"");
    p.replace("xl/workbook.xml", body.into_bytes());
    let mut b = open_bytes(&p.write().unwrap());
    let before = b.save_bytes().unwrap();
    let revision = b.layout_revision();
    let mut d = PivotCreation::open(
        &b,
        jade_xlsx_reader::CellRange::parse("A1:C5").unwrap(),
        at("A1"),
    );
    d.new_sheet = false;
    d.focus(PivotCreationField::Destination);
    d.field_mut().insert("'Report'!K24");
    assert_eq!(d.apply(&mut b).unwrap_err(), "pivot.hidden_destination");
    assert_eq!(b.sheet_index(), 0);
    assert_eq!(b.save_bytes().unwrap(), before);
    assert_eq!(b.layout_revision(), revision);
}

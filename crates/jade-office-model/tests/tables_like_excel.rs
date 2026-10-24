//! Formulas over an Excel table — a totals row's SUBTOTAL over a column,
//! `Table1[Qty]` typed beside it — recalculate when the table is edited, as
//! in Excel, rather than keeping the totals the file was saved with.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::sheet::{CellRef, Workbook};
use jade_office_model::tiny::TinySkiaBackend;

fn book() -> Workbook<TinySkiaBackend> {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    // Table1 on A1:C5: Item, Qty, Amount; Total row SUBTOTAL(109,Table1[Amount]).
    let bytes = std::fs::read(core.join("public/fixtures/objects-workbook.xlsx")).unwrap();
    open_bytes(&bytes)
}
fn open_bytes(bytes: &[u8]) -> Workbook<TinySkiaBackend> {
    let directory = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../jade-core/fonts/Noto_Sans_JP/static"
    ));
    let backend = TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        None,
    ))
    .unwrap();
    Workbook::open(bytes, "UTC", backend).unwrap()
}

#[test]
fn editing_a_table_updates_its_totals_and_the_formulas_over_it() {
    let mut book = book();
    let total = CellRef::new(2, 4);
    assert_eq!(book.number(total), Some(749.0));
    let quantities = CellRef::new(4, 0);
    book.set_cell(quantities, "=SUM(Table1[Qty])").unwrap();
    assert_eq!(book.number(quantities), Some(6.0));

    // Gizmo: 1 for 99 becomes 3 for 199.
    book.set_cell(CellRef::new(1, 3), "3").unwrap();
    book.set_cell(CellRef::new(2, 3), "199").unwrap();
    assert_eq!(book.number(total), Some(849.0));
    assert_eq!(book.number(quantities), Some(8.0));
}

#[test]
fn table_dialog_creates_designs_resizes_calculates_and_converts_in_history() {
    use jade_office_model::{
        sheet::CellRange,
        tables::{TableDialog, TableField},
    };
    let mut book = book();
    for (at, text) in [
        ("H2", "Item"),
        ("I2", "Qty"),
        ("J2", "Amount"),
        ("H3", "A"),
        ("I3", "2"),
        ("J3", "20"),
        ("H4", "B"),
        ("I4", "3"),
        ("J4", "45"),
    ] {
        book.set_cell(CellRef::parse(at).unwrap(), text).unwrap();
    }
    let range = CellRange::parse("H2:J4").unwrap();
    let at = range.start;
    let mut d = TableDialog::open(&book, range, at);
    assert!(!d.editing());
    d.focus(TableField::Name);
    d.field_mut().replace(None, "Sales", false);
    d.style.name = Some("TableStyleDark8".into());
    d.style.first_column = true;
    book.apply_table_command(d.command().unwrap()).unwrap();
    assert_eq!(book.table(at).unwrap().name, "Sales");
    assert_eq!(
        book.table(at).unwrap().style_name.as_deref(),
        Some("TableStyleDark8")
    );
    book.undo().unwrap();
    assert!(book.table(at).is_none());
    book.redo().unwrap();
    let mut d = TableDialog::open(&book, range, at);
    d.field_mut().replace(None, "$H$2:$K$4", false);
    book.apply_table_command(d.command().unwrap()).unwrap();
    assert_eq!(book.table(at).unwrap().columns.len(), 4);
    let at = CellRef::parse("K3").unwrap();
    let mut d = TableDialog::open(&book, CellRange::parse("K3").unwrap(), at);
    assert_eq!(d.column, 3);
    d.focus(TableField::Formula);
    d.field_mut().replace(None, "I3*J3", false);
    d.has_totals = true;
    d.total_kind = 1;
    book.apply_table_command(d.command().unwrap()).unwrap();
    assert_eq!(book.number(CellRef::parse("K4").unwrap()), Some(135.));
    assert_eq!(book.number(CellRef::parse("K5").unwrap()), Some(175.));
    let mut d = TableDialog::open(&book, range, at);
    d.focus(TableField::Name);
    d.field_mut().replace(None, "Revenue", false);
    book.convert_table_command(d.command().unwrap()).unwrap();
    assert!(book.table(at).is_none());
    assert_eq!(book.number(CellRef::parse("K5").unwrap()), Some(175.));
    book.undo().unwrap();
    assert_eq!(book.table(at).unwrap().name, "Sales");
    assert!(book.table(at).unwrap().totals_row_count > 0);
}

#[test]
fn table_dialog_retains_column_drafts_when_switching_columns() {
    use jade_office_model::{
        sheet::CellRange,
        tables::{TableCommand, TableDialog, TableField, TableOptions, TableTotal},
    };
    let mut book = book();
    let range = CellRange::parse("H2:J4").unwrap();
    book.apply_table_command(TableCommand::Create(
        range,
        TableOptions {
            name: Some("Sales".into()),
            ..Default::default()
        },
    ))
    .unwrap();
    let mut d = TableDialog::open(&book, range, range.start);
    d.has_totals = true;
    d.total_kind = 9;
    d.focus(TableField::TotalValue);
    d.field_mut().replace(None, "合計", false);
    d.next_column();
    d.focus(TableField::Formula);
    d.field_mut().replace(None, "ROW()", false);
    d.total_kind = 1;
    let TableCommand::Edit(_, update) = d.command().unwrap() else {
        panic!()
    };
    assert_eq!(
        update.totals,
        vec![
            (0, TableTotal::Label("合計".into())),
            (
                1,
                TableTotal::Function(jade_office_model::tables::TotalFunction::Sum)
            )
        ]
    );
    assert_eq!(update.calculated_columns, vec![(1, Some("ROW()".into()))]);
}

#[test]
fn table_dialog_keeps_numeric_first_rows_by_guessing_no_headers() {
    use jade_office_model::{
        sheet::CellRange,
        tables::{TableCommand, TableDialog},
    };
    let mut book = book();
    for (at, text) in [("H2", "1"), ("I2", "2"), ("H3", "3"), ("I3", "4")] {
        book.set_cell(CellRef::parse(at).unwrap(), text).unwrap();
    }
    let selected = CellRange::parse("H2:I3").unwrap();
    let mut dialog = TableDialog::open(&book, selected, selected.start);
    assert!(!dialog.has_headers);
    let TableCommand::Create(_, options) = dialog.command().unwrap() else {
        panic!()
    };
    assert!(!options.has_headers);
}

#[test]
fn cell_shifts_rebuild_the_grid_follow_formulas_and_have_one_history_step() {
    use jade_office_model::sheet::{Axis, CellRange};
    let mut book = book();
    for (at, text) in [("H2", "2"), ("H3", "3"), ("I2", "20"), ("J2", "=H2+H3")] {
        book.set_cell(CellRef::parse(at).unwrap(), text).unwrap();
    }
    book.shift_cells(CellRange::parse("H2").unwrap(), Axis::Rows, true)
        .unwrap();
    assert_eq!(book.number(CellRef::parse("H3").unwrap()), Some(2.));
    assert_eq!(book.number(CellRef::parse("H4").unwrap()), Some(3.));
    assert_eq!(book.number(CellRef::parse("I2").unwrap()), Some(20.));
    assert_eq!(book.number(CellRef::parse("J2").unwrap()), Some(5.));
    book.undo().unwrap();
    assert_eq!(book.number(CellRef::parse("H2").unwrap()), Some(2.));
    assert_eq!(book.number(CellRef::parse("H3").unwrap()), Some(3.));
    book.redo().unwrap();
    assert_eq!(book.number(CellRef::parse("H4").unwrap()), Some(3.));
}

#[test]
fn inserted_table_rows_fill_formulas_and_refresh_values_through_model_history() {
    use jade_office_model::{
        sheet::{Axis, CellRange},
        tables::{TableCommand, TableOptions, TableUpdate},
    };
    let mut book = book();
    for (at, text) in [
        ("H2", "Item"),
        ("I2", "Qty"),
        ("J2", "Price"),
        ("K2", "Amount"),
        ("H3", "A"),
        ("I3", "2"),
        ("J3", "10"),
        ("H4", "B"),
        ("I4", "3"),
        ("J4", "20"),
    ] {
        book.set_cell(CellRef::parse(at).unwrap(), text).unwrap();
    }
    book.apply_table_command(TableCommand::Create(
        CellRange::parse("H2:K4").unwrap(),
        TableOptions {
            name: Some("Sales".into()),
            ..Default::default()
        },
    ))
    .unwrap();
    book.apply_table_command(TableCommand::Edit(
        "Sales".into(),
        TableUpdate {
            calculated_columns: vec![(3, Some("I3*J3".into()))],
            ..Default::default()
        },
    ))
    .unwrap();
    book.shift_cells(CellRange::parse("H3:K3").unwrap(), Axis::Rows, true)
        .unwrap();
    assert_eq!(book.number(CellRef::parse("K3").unwrap()), Some(0.));
    assert_eq!(book.number(CellRef::parse("K4").unwrap()), Some(20.));
    book.undo().unwrap();
    assert_eq!(book.number(CellRef::parse("K3").unwrap()), Some(20.));
    book.redo().unwrap();
    assert_eq!(book.number(CellRef::parse("K3").unwrap()), Some(0.));
    book.set_cell(CellRef::parse("I3").unwrap(), "5").unwrap();
    book.set_cell(CellRef::parse("J3").unwrap(), "6").unwrap();
    assert_eq!(book.number(CellRef::parse("K3").unwrap()), Some(30.));
}

#[test]
fn stale_table_relationships_do_not_turn_ordinary_numeric_edits_into_headings() {
    let mut package = jade_opc::OpcPackage::open(include_bytes!(
        "../../../jade-core/public/fixtures/objects-workbook.xlsx"
    ))
    .unwrap();
    let part = "xl/worksheets/sheet1.xml";
    let xml = package.required_part_str(part).unwrap();
    let source = xml.replace(
        "<tableParts count=\"1\"><tablePart r:id=\"rId1\"/></tableParts>",
        "",
    );
    assert_ne!(source, xml);
    package.replace(part, source.into_bytes());
    let mut book = open_bytes(&package.write().unwrap());
    let at = CellRef::new(1, 0);
    assert!(book.table(at).is_none());
    book.set_cell(at, "10").unwrap();
    assert_eq!(book.number(at), Some(10.));
    book.undo().unwrap();
    assert_eq!(book.edit_text(at), "Qty");
    book.redo().unwrap();
    assert_eq!(book.number(at), Some(10.));
    let saved = book.save_bytes().unwrap();
    let reopened = open_bytes(&saved);
    assert!(reopened.table(at).is_none());
    assert_eq!(reopened.number(at), Some(10.));
}

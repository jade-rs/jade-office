#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::sheet::{CellRef, Workbook};
use jade_office_model::tiny::TinySkiaBackend;

fn book() -> Workbook<TinySkiaBackend> {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let bytes = std::fs::read(core.join("public/fixtures/grid-operations-workbook.xlsx")).unwrap();
    open_bytes(&bytes)
}

#[test]
fn whole_axis_deletion_keeps_standard_and_x14_input_checks_at_surviving_cells() {
    for source in [
        r#"<dataValidations count="1"><dataValidation type="whole" operator="lessThan" showErrorMessage="1" sqref="B1:B3"><formula1>$A1</formula1></dataValidation></dataValidations>"#,
        r#"<extLst><ext uri="dv"><x:dataValidations count="1"><x:dataValidation type="whole" operator="lessThan" showErrorMessage="1"><x:formula1><m:f>$A1</m:f></x:formula1><m:sqref>B1:B3</m:sqref></x:dataValidation></x:dataValidations></ext></extLst>"#,
    ] {
        let mut package = jade_opc::OpcPackage::open(include_bytes!(
            "../../../jade-core/public/fixtures/grid-operations-workbook.xlsx"
        ))
        .unwrap();
        package.remove("xl/worksheets/_rels/sheet1.xml.rels");
        package.replace("xl/worksheets/sheet1.xml", format!(r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:x="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main" xmlns:m="http://schemas.microsoft.com/office/excel/2006/main"><sheetData><row r="1"><c r="A1"><v>10</v></c></row><row r="2"><c r="A2"><v>20</v></c></row><row r="3"><c r="A3"><v>30</v></c></row></sheetData>{source}</worksheet>"#).into_bytes());
        let mut book = open_bytes(&package.write().unwrap());
        let at = CellRef::new(1, 0);
        assert!(book.validation_alert(at, "15").unwrap().is_some());
        assert!(book
            .validation_alert(CellRef::new(1, 1), "15")
            .unwrap()
            .is_none());
        assert!(book.delete_rows(0, 1).unwrap());
        assert_eq!(book.number(CellRef::new(0, 0)), Some(20.0));
        assert_eq!(
            book.data_validation_at(at).unwrap().formula1.as_deref(),
            Some("$A1")
        );
        assert!(book.validation_alert(at, "15").unwrap().is_none());
        assert!(book.validation_alert(at, "25").unwrap().is_some());
        let saved = book.save_bytes().unwrap();
        let reopened = open_bytes(&saved);
        assert_eq!(reopened.data_validations(), book.data_validations());
        assert!(reopened.validation_alert(at, "15").unwrap().is_none());
        book.undo().unwrap();
        assert_eq!(book.number(CellRef::new(0, 0)), Some(10.0));
        assert!(book.validation_alert(at, "15").unwrap().is_some());
        book.redo().unwrap();
        assert!(book.validation_alert(at, "15").unwrap().is_none());
    }
}

#[test]
fn table_header_rename_keeps_x14_relative_input_bounds_valid_in_model_history() {
    use jade_xlsx_headless_editor::{HeadlessWorkbook, TableOptions};
    let mut package = jade_opc::OpcPackage::open(include_bytes!(
        "../../../jade-core/public/fixtures/grid-operations-workbook.xlsx"
    ))
    .unwrap();
    package.remove("xl/worksheets/_rels/sheet1.xml.rels");
    let part = "xl/worksheets/sheet1.xml";
    package.replace(part, br#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:x="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main" xmlns:m="http://schemas.microsoft.com/office/excel/2006/main"><sheetData><row r="1"><c r="H1" t="inlineStr"><is><t>Item</t></is></c><c r="I1" t="inlineStr"><is><t>Qty</t></is></c></row><row r="2"><c r="H2" t="inlineStr"><is><t>One</t></is></c><c r="I2"><v>2</v></c></row><row r="3"><c r="H3" t="inlineStr"><is><t>Two</t></is></c><c r="I3"><v>3</v></c></row></sheetData></worksheet>"#.to_vec());
    let mut core = HeadlessWorkbook::open(&package.write().unwrap()).unwrap();
    let name = core.workbook().sheets[0].name.clone();
    core.create_table(
        &name,
        CellRange::parse("H1:I3").unwrap(),
        &TableOptions {
            name: Some("Sales".into()),
            ..Default::default()
        },
    )
    .unwrap();
    let mut package = jade_opc::OpcPackage::open(core.bytes()).unwrap();
    let source = package.required_part_str(part).unwrap();
    let ext = r#"<extLst><ext uri="dv"><x:dataValidations count="1"><x:dataValidation type="whole" operator="lessThan" showErrorMessage="1"><x:formula1><m:f>[@Qty]</m:f></x:formula1><m:sqref>H2:H3</m:sqref></x:dataValidation></x:dataValidations></ext></extLst>"#;
    package.replace(
        part,
        source
            .replace("</worksheet>", &format!("{ext}</worksheet>"))
            .into_bytes(),
    );
    let mut book = open_bytes(&package.write().unwrap());
    let at = CellRef::new(7, 1);
    assert!(book.validation_alert(at, "1").unwrap().is_none());
    assert!(book.validation_alert(at, "2").unwrap().is_some());
    book.set_cell(CellRef::new(8, 0), "Units").unwrap();
    assert_eq!(
        book.data_validation_at(at).unwrap().formula1.as_deref(),
        Some("[@Units]")
    );
    assert!(book.validation_alert(at, "1").unwrap().is_none());
    assert!(book.validation_alert(at, "2").unwrap().is_some());
    let saved = book.save_bytes().unwrap();
    let reopened = open_bytes(&saved);
    assert_eq!(reopened.data_validations(), book.data_validations());
    assert!(reopened.validation_alert(at, "2").unwrap().is_some());
    book.undo().unwrap();
    assert_eq!(
        book.data_validation_at(at).unwrap().formula1.as_deref(),
        Some("[@Qty]")
    );
    book.redo().unwrap();
    assert_eq!(
        book.data_validation_at(at).unwrap().formula1.as_deref(),
        Some("[@Units]")
    );
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

use jade_office_model::validation::{AlertStyle, ValidationDialog, ValidationField, ValidationTab};
use jade_xlsx_reader::{CellRange, DataValidation};
fn rule(kind: &str, range: &str, first: &str) -> DataValidation {
    DataValidation {
        ranges: vec![CellRange::parse(range).unwrap()],
        validation_type: Some(kind.into()),
        operator: Some("lessThan".into()),
        show_error_message: true,
        formula1: Some(first.into()),
        ..Default::default()
    }
}

#[test]
fn validation_rejects_references_custom_predicates_dates_and_formula_results() {
    let mut book = book();
    let at = CellRef::new(7, 1);
    book.apply_data_validation(&rule("whole", "H2:H4", "$B2"))
        .unwrap();
    assert!(book.set_cell(at, "15").is_err());
    assert!(book.set_cell(CellRef::new(7, 2), "15").unwrap());
    book.apply_data_validation(&rule("decimal", "H2", "10"))
        .unwrap();
    assert!(book.set_cell(at, "=6+6").is_err());
    assert!(book.set_cell(at, "=3+3").unwrap());
    book.apply_data_validation(&rule("custom", "H2", "H2<>$B$2"))
        .unwrap();
    assert!(book.set_cell(at, "10").is_err());
    assert!(book.set_cell(at, "11").unwrap());
    book.apply_data_validation(&rule("date", "H2", "DATE(2024,2,1)"))
        .unwrap();
    assert!(book.set_cell(at, "2024/1/15").unwrap());
    assert!(book.set_cell(at, "2024/2/2").is_err());
}
#[test]
fn warning_and_information_require_explicit_acceptance_and_stop_cannot_be_bypassed() {
    let mut book = book();
    let at = CellRef::new(7, 1);
    for (style, expected) in [
        ("warning", AlertStyle::Warning),
        ("information", AlertStyle::Information),
    ] {
        let mut rule = rule("whole", "H2", "10");
        rule.error_style = Some(style.into());
        rule.error_title = Some("確認".into());
        rule.error = Some("10未満で入力".into());
        book.apply_data_validation(&rule).unwrap();
        assert!(book.set_cell(at, "20").is_err());
        let alert = book.validation_alert(at, "20").unwrap().unwrap();
        assert_eq!(alert.style, expected);
        assert_eq!(alert.title.as_deref(), Some("確認"));
        assert!(book.set_cell_accepting_warning(at, "20").unwrap());
        assert_eq!(book.number(at), Some(20.0));
        book.undo().unwrap();
        assert_ne!(book.number(at), Some(20.0));
    }
    book.apply_data_validation(&rule("whole", "H2", "10"))
        .unwrap();
    assert!(book.set_cell_accepting_warning(at, "20").is_err());
}
#[test]
fn dialog_all_tabs_apply_and_input_message_dropdown_flags_survive_save() {
    let mut book = book();
    let at = CellRef::new(7, 1);
    let range = CellRange::parse("H2:H4").unwrap();
    let mut dialog = ValidationDialog::open(&book, range, at);
    dialog.kind = 3;
    dialog.fields[1].text = "=Choices".into();
    dialog.select_tab(ValidationTab::Input);
    dialog.field_mut().text = "案内".into();
    dialog.focus(ValidationField::Prompt);
    dialog.field_mut().text = "候補から選択してください".into();
    dialog.select_tab(ValidationTab::Error);
    dialog.style = 1;
    dialog.fields[5].text = "確認".into();
    dialog.fields[6].text = "一覧にない値です".into();
    dialog.dropdown = false;
    dialog.ime = 4;
    book.define_range_name("Choices", CellRange::parse("A2:A4").unwrap())
        .unwrap();
    let rule = dialog.rule().unwrap();
    book.apply_data_validation(&rule).unwrap();
    assert_eq!(
        book.validation_prompt(at),
        Some(("案内".into(), "候補から選択してください".into()))
    );
    assert!(book.validation_list(at).is_some());
    assert!(book.validation_dropdown(at).is_none());
    let bytes = book.save_bytes().unwrap();
    let core = jade_xlsx_headless_editor::HeadlessWorkbook::open(&bytes).unwrap();
    let saved = core.workbook().sheets[0]
        .data_validations
        .iter()
        .find(|r| r.ranges == vec![range])
        .unwrap();
    assert_eq!(saved, &rule);
}
#[test]
fn clearing_multiple_selections_is_one_undo_and_does_not_require_a_source() {
    let mut book = book();
    let range = CellRange::parse("H2:H10").unwrap();
    book.apply_data_validation(&rule("whole", "H2:H10", "10"))
        .unwrap();
    let mut dialog = ValidationDialog::open(&book, range, range.start);
    dialog.fields[0].text = "H2:H3 H8:H10".into();
    dialog.fields[1].text.clear();
    assert!(dialog.rule().is_err());
    book.clear_data_validation_ranges(&dialog.ranges().unwrap())
        .unwrap();
    assert!(book.data_validation_at(CellRef::new(7, 1)).is_none());
    assert!(book.data_validation_at(CellRef::new(7, 4)).is_some());
    assert!(book.data_validation_at(CellRef::new(7, 8)).is_none());
    book.undo().unwrap();
    assert_eq!(
        book.data_validation_at(CellRef::new(7, 1)).unwrap().ranges,
        vec![range]
    );
}

#[test]
fn dialog_translates_literal_lists_and_typed_dates_to_file_values() {
    let book = book();
    let range = CellRange::parse("H2:H4").unwrap();
    let mut dialog = ValidationDialog::open(&book, range, range.start);
    dialog.kind = 3;
    dialog.fields[1].text = "東京,大阪".into();
    assert_eq!(
        dialog.rule().unwrap().formula1.as_deref(),
        Some("\"東京,大阪\"")
    );
    dialog.kind = 4;
    dialog.fields[1].text = "2024/1/1".into();
    dialog.fields[2].text = "2024/1/31".into();
    let rule = dialog.rule().unwrap();
    assert_eq!(rule.formula1.as_deref(), Some("45292"));
    assert_eq!(rule.formula2.as_deref(), Some("45322"));
}

#[test]
fn editing_part_of_a_rule_moves_relative_references_to_the_selected_cell() {
    let mut book = book();
    let at = CellRef::new(7, 4);
    book.apply_data_validation(&rule("custom", "H2:H10", "H2>0"))
        .unwrap();
    let dialog = ValidationDialog::open(&book, CellRange { start: at, end: at }, at);
    assert_eq!(dialog.value(ValidationField::First), "H5>0");
    book.apply_data_validation(&dialog.rule().unwrap()).unwrap();
    assert!(book.set_cell(at, "-1").is_err());
    assert!(book.set_cell(at, "3").unwrap());
}
#[test]
fn changes_can_apply_to_other_cells_with_matching_settings_in_one_undo() {
    let mut book = book();
    book.apply_data_validation(&rule("whole", "H2:H4", "10"))
        .unwrap();
    book.apply_data_validation(&rule("whole", "J2:J4", "10"))
        .unwrap();
    let at = CellRef::new(7, 2);
    let mut dialog = ValidationDialog::open(&book, CellRange { start: at, end: at }, at);
    dialog.apply_to_same = true;
    dialog.fields[1] = jade_office_model::text::TextEdit::new("20".into());
    book.apply_data_validation(&dialog.rule().unwrap()).unwrap();
    assert_eq!(
        book.data_validation_at(CellRef::new(9, 1))
            .unwrap()
            .formula1
            .as_deref(),
        Some("20")
    );
    book.undo().unwrap();
    assert_eq!(
        book.data_validation_at(CellRef::new(9, 1))
            .unwrap()
            .formula1
            .as_deref(),
        Some("10")
    );
}

#[test]
fn imported_x14_rules_drive_prompts_lists_stop_and_warning_choices_with_history() {
    let mut package = jade_opc::OpcPackage::open(include_bytes!(
        "../../../jade-core/public/fixtures/grid-operations-workbook.xlsx"
    ))
    .unwrap();
    let part = "xl/worksheets/sheet1.xml";
    // This scenario replaces the entire worksheet. Its original table
    // relationship would otherwise still make C1 a table heading (text).
    package.remove("xl/worksheets/_rels/sheet1.xml.rels");
    package.replace(part,br#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:x="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main" xmlns:m="http://schemas.microsoft.com/office/excel/2006/main"><sheetData/><extLst><ext uri="dv"><x:dataValidations count="2"><x:dataValidation type="list" showDropDown="1" showInputMessage="1" showErrorMessage="1" promptTitle="Choose" prompt="Pick a city" imeMode="hiragana"><x:formula1><m:f>"Tokyo,Osaka"</m:f></x:formula1><m:sqref>B1:B3</m:sqref></x:dataValidation><x:dataValidation type="whole" operator="lessThan" showErrorMessage="1" errorStyle="warning" errorTitle="Check" error="Below five"><x:formula1><m:f>5</m:f></x:formula1><m:sqref>C1</m:sqref></x:dataValidation></x:dataValidations></ext></extLst></worksheet>"#.to_vec());
    let mut book = open_bytes(&package.write().unwrap());
    let at = CellRef::new(1, 0);
    let warning = CellRef::new(2, 0);
    assert_eq!(book.data_validations().len(), 2);
    assert_eq!(
        book.validation_prompt(at),
        Some(("Choose".into(), "Pick a city".into()))
    );
    assert_eq!(
        book.validation_dropdown(at),
        Some(vec!["Tokyo".into(), "Osaka".into()])
    );
    assert_eq!(
        book.data_validation_at(at).unwrap().ime_mode.as_deref(),
        Some("hiragana")
    );
    assert!(book.set_cell(at, "Unknown").is_err());
    assert!(book.set_cell(at, "Tokyo").unwrap());
    assert!(book.set_cell(warning, "10").is_err());
    let alert = book.validation_alert(warning, "10").unwrap().unwrap();
    assert_eq!(alert.style, AlertStyle::Warning);
    assert_eq!(alert.title.as_deref(), Some("Check"));
    assert!(book.set_cell_accepting_warning(warning, "10").unwrap());
    assert_eq!(book.number(warning), Some(10.));
    book.undo().unwrap();
    assert_ne!(book.number(warning), Some(10.));
    let dialog = ValidationDialog::open(&book, CellRange::parse("B1:B3").unwrap(), at);
    assert_eq!(dialog.kind, 3);
    assert_eq!(dialog.fields[1].text, "Tokyo,Osaka");
    book.clear_data_validations(Some(CellRange::parse("B1:B3").unwrap()))
        .unwrap();
    assert!(book.validation_dropdown(at).is_none());
    book.undo().unwrap();
    assert!(book.validation_dropdown(at).is_some());
    let saved = book.save_bytes().unwrap();
    let reopened = open_bytes(&saved);
    assert_eq!(
        reopened.validation_dropdown(at),
        book.validation_dropdown(at)
    );
    assert_eq!(reopened.data_validations(), book.data_validations());
}

#[test]
fn quoted_list_dialog_and_imported_alternatives_preserve_candidates_and_history() {
    let mut package = jade_opc::OpcPackage::open(include_bytes!(
        "../../../jade-core/public/fixtures/grid-operations-workbook.xlsx"
    ))
    .unwrap();
    package.remove("xl/worksheets/_rels/sheet1.xml.rels");
    let part = "xl/worksheets/sheet1.xml";
    package.replace(part,br#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:c="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:l="http://schemas.microsoft.com/office/spreadsheetml/2011/1/ac"><sheetData/><dataValidations count="1"><dataValidation type="list" sqref="B1:B3" showErrorMessage="1"><c:AlternateContent><c:Choice Requires="l"><l:list>"North,East","a""b","=SUM(A1)"</l:list></c:Choice><c:Fallback><formula1>"wrong"</formula1></c:Fallback></c:AlternateContent></dataValidation></dataValidations></worksheet>"#.to_vec());
    let mut book = open_bytes(&package.write().unwrap());
    let at = CellRef::new(1, 0);
    assert_eq!(
        book.validation_dropdown(at),
        Some(vec!["North,East".into(), "a\"b".into(), "=SUM(A1)".into()])
    );
    assert!(book.set_cell(at, "North").is_err());
    assert!(book.set_cell(at, "North,East").unwrap());
    assert!(book.set_validation_choice(at, "=SUM(A1)").unwrap());
    let saved_choice = book.save_bytes().unwrap();
    let core = jade_xlsx_headless_editor::HeadlessWorkbook::open(&saved_choice).unwrap();
    let choice_cell = core.cell(&core.workbook().sheets[0].name, "B1").unwrap();
    assert!(choice_cell.formula.is_none());
    assert_eq!(choice_cell.text.as_deref(), Some("=SUM(A1)"));
    book.undo().unwrap();
    assert_eq!(book.edit_text(at), "North,East");

    let mut dialog = ValidationDialog::open(&book, CellRange::parse("B1:B3").unwrap(), at);
    assert_eq!(dialog.fields[1].text, "\"North,East\",\"a\"\"b\",=SUM(A1)");
    dialog.fields[1].text = "\"West,Coast\",other".into();
    let r = dialog.rule().unwrap();
    assert_eq!(r.literal_list.as_deref(), Some("\"West,Coast\",other"));
    assert!(r.formula1.is_none());
    book.apply_data_validation(&dialog.rule().unwrap()).unwrap();
    assert_eq!(
        book.validation_dropdown(at),
        Some(vec!["West,Coast".into(), "other".into()])
    );
    book.undo().unwrap();
    assert_eq!(book.validation_dropdown(at).unwrap()[0], "North,East");
    let saved = book.save_bytes().unwrap();
    assert_eq!(
        open_bytes(&saved).validation_dropdown(at),
        book.validation_dropdown(at)
    );
}
#[test]
fn quoted_single_items_formula_like_text_and_invalid_csv_are_distinct_dialog_sources() {
    let book = book();
    let at = CellRef::new(7, 1);
    let range = CellRange { start: at, end: at };
    let mut d = ValidationDialog::open(&book, range, at);
    d.kind = 3;
    d.fields[1].text = "\"a,b\"".into();
    assert_eq!(d.rule().unwrap().literal_list.as_deref(), Some("\"a,b\""));
    d.fields[1].text = "\"=A1\"".into();
    assert_eq!(d.rule().unwrap().formula1.as_deref(), Some("\"=A1\""));
    assert!(d.rule().unwrap().literal_list.is_none());
    d.fields[1].text = "=Choices".into();
    assert_eq!(d.rule().unwrap().formula1.as_deref(), Some("Choices"));
    d.fields[1].text = "\"bad".into();
    assert_eq!(d.rule().unwrap_err(), "validation.invalid_list");
}

#[test]
fn choosing_a_textual_boolean_or_date_does_not_fail_due_to_typed_input_coercion() {
    let mut book = book();
    let at = CellRef::new(7, 1);
    book.apply_data_validation(&rule("list", "H2", "\"TRUE,2024/1/1,=A1\""))
        .unwrap();
    for item in ["TRUE", "2024/1/1", "=A1"] {
        assert!(book.set_validation_choice(at, item).unwrap());
        let saved = book.save_bytes().unwrap();
        let core = jade_xlsx_headless_editor::HeadlessWorkbook::open(&saved).unwrap();
        let cell = core.cell(&core.workbook().sheets[0].name, "H2").unwrap();
        assert!(cell.formula.is_none());
        assert_eq!(cell.text.as_deref(), Some(item));
    }
    assert!(book.set_validation_choice(at, "not a choice").is_err());
}

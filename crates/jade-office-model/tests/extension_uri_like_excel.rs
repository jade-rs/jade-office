//! An unrecognized extension is retained without becoming an active input rule.
#![cfg(feature = "tiny-skia")]
use jade_office_model::{
    aliased_faces,
    sheet::{CellRef, Workbook},
    tiny::TinySkiaBackend,
};
use std::{path::PathBuf, sync::Arc};

fn open(bytes: &[u8]) -> Workbook<TinySkiaBackend> {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let font =
        std::fs::read(core.join("fonts/Noto_Sans_JP/static/NotoSansJP-Regular.ttf")).unwrap();
    let backend = TinySkiaBackend::new(&aliased_faces(Arc::new(font), None)).unwrap();
    Workbook::open(bytes, "UTC", backend).unwrap()
}

#[test]
fn unknown_uri_rules_do_not_reject_input_and_keep_original_xml_after_edits() {
    let opaque = r#"<ext uri="urn:unprocessed"><x:dataValidations count="1"><x:dataValidation type="whole" operator="lessThan" showErrorMessage="1"><x:formula1><m:f>Operations!A1</m:f></x:formula1><m:sqref>B1</m:sqref></x:dataValidation></x:dataValidations></ext>"#;
    let mut package = jade_opc::OpcPackage::open(include_bytes!(
        "../../../jade-core/public/fixtures/grid-operations-workbook.xlsx"
    ))
    .unwrap();
    let part = "xl/worksheets/sheet1.xml";
    package.remove("xl/worksheets/_rels/sheet1.xml.rels");
    package.replace(part,format!(r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:x="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main" xmlns:m="http://schemas.microsoft.com/office/excel/2006/main"><sheetData><row r="1"><c r="A1"><v>10</v></c></row></sheetData><dataValidations count="1"><dataValidation type="whole" operator="lessThan" showErrorMessage="1" sqref="C1"><formula1>Operations!$A1</formula1></dataValidation></dataValidations><extLst>{opaque}</extLst></worksheet>"#).into_bytes());
    let mut book = open(&package.write().unwrap());
    let at = |s: &str| CellRef::parse(s).unwrap();
    assert_eq!(book.data_validations().len(), 1);
    assert!(book.validation_alert(at("B1"), "999").unwrap().is_none());
    assert!(book.validation_alert(at("C1"), "10").unwrap().is_some());
    assert!(book.insert_columns(0, 1).unwrap());
    assert!(book.insert_rows(0, 1).unwrap());
    assert!(book.rename_sheet(0, "Renamed").unwrap());
    let saved = book.save_bytes().unwrap();
    let sheet = jade_opc::OpcPackage::open(&saved)
        .unwrap()
        .required_part_str(part)
        .unwrap();
    assert!(sheet.contains(opaque));
    assert!(sheet.contains("Renamed!$B2"));
    let mut reopened = open(&saved);
    assert_eq!(reopened.data_validations().len(), 1);
    assert!(reopened
        .validation_alert(at("B1"), "999")
        .unwrap()
        .is_none());
    assert!(reopened.validation_alert(at("D2"), "10").unwrap().is_some());
    assert!(reopened.validation_alert(at("D2"), "9").unwrap().is_none());
    reopened.set_cell(at("B2"), "5").unwrap();
    assert!(reopened.validation_alert(at("D2"), "9").unwrap().is_some());
}

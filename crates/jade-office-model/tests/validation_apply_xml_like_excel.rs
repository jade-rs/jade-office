//! Applying an input rule preserves its effective checks and source XML.
#![cfg(feature = "tiny-skia")]
use jade_office_model::{aliased_faces, sheet::Workbook, tiny::TinySkiaBackend};
use std::{path::PathBuf, sync::Arc};

fn open(bytes: &[u8]) -> Workbook<TinySkiaBackend> {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let font =
        std::fs::read(core.join("fonts/Noto_Sans_JP/static/NotoSansJP-Regular.ttf")).unwrap();
    Workbook::open(
        bytes,
        "UTC",
        TinySkiaBackend::new(&aliased_faces(Arc::new(font), None)).unwrap(),
    )
    .unwrap()
}
fn xml(book: &mut Workbook<TinySkiaBackend>) -> String {
    jade_opc::OpcPackage::open(&book.save_bytes().unwrap())
        .unwrap()
        .required_part_str("xl/worksheets/sheet1.xml")
        .unwrap()
}
#[test]
fn applying_an_input_rule_keeps_its_source_xml_and_effective_checks() {
    let uid = "r:uid=\"{11111111-2222-4333-8444-555555555555}\"";
    let shadow = "<u:pass>A99</u:pass>";
    let mut package = jade_opc::OpcPackage::open(include_bytes!(
        "../../../jade-core/public/fixtures/grid-operations-workbook.xlsx"
    ))
    .unwrap();
    package.remove("xl/worksheets/_rels/sheet1.xml.rels");
    package.replace("xl/worksheets/sheet1.xml", format!(r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.microsoft.com/office/spreadsheetml/2014/revision" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:u="urn:one" xmlns:v="urn:one" mc:Ignorable="u" mc:ProcessContent="u:pass"><sheetData><row r="1"><c r="A1"><v>10</v></c></row></sheetData><dataValidations count="1"><dataValidation {uid} sqref="B2" type="whole" operator="lessThan" showErrorMessage="1"><formula1 xmlns:u="urn:two" mc:Ignorable="u" u:note="keep"><!--source--><v:pass>Operations!$A$1</v:pass>{shadow}</formula1></dataValidation></dataValidations></worksheet>"#).into_bytes());
    let mut book = open(&package.write().unwrap());
    let at = jade_office_model::sheet::CellRef::parse("B2").unwrap();
    assert!(book.validation_alert(at, "9").unwrap().is_none());
    assert!(book.validation_alert(at, "10").unwrap().is_some());
    let before = xml(&mut book);
    let mut rule = book.data_validations()[0].clone();
    rule.formula1 = Some("Operations!$A$1+1".into());
    assert!(book.apply_data_validation(&rule).unwrap());
    assert!(book.validation_alert(at, "10").unwrap().is_none());
    assert!(book.validation_alert(at, "11").unwrap().is_some());
    let after = xml(&mut book);
    for marker in [
        uid,
        shadow,
        "u:note=\"keep\"",
        "<!--source-->",
        "<v:pass>Operations!$A$1+1</v:pass>",
    ] {
        assert!(after.contains(marker), "{marker}: {after}");
    }
    assert!(book.apply_data_validation(&rule).unwrap());
    assert_eq!(xml(&mut book), after);
    assert!(book.undo().unwrap());
    assert_eq!(xml(&mut book), before);
    assert!(book.validation_alert(at, "10").unwrap().is_some());
    assert!(book.redo().unwrap());
    assert_eq!(xml(&mut book), after);
    let mut reopened = open(&book.save_bytes().unwrap());
    assert_eq!(xml(&mut reopened), after);
    assert!(reopened.validation_alert(at, "10").unwrap().is_none());
    assert!(reopened.validation_alert(at, "11").unwrap().is_some());
}

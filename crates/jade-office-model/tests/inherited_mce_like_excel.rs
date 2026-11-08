//! Inherited MCE feature sources survive the app's editing and history paths.
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
fn inherited_mce_sources_follow_rows_columns_rename_and_history() {
    let opaque = r#"<ext uri="urn:opaque"><m:f>Operations!Z1</m:f></ext>"#;
    let shadow = r#"<u:pass><m:f>Operations!Z1</m:f></u:pass>"#;
    let mut package = jade_opc::OpcPackage::open(include_bytes!(
        "../../../jade-core/public/fixtures/grid-operations-workbook.xlsx"
    ))
    .unwrap();
    package.remove("xl/worksheets/_rels/sheet1.xml.rels");
    package.replace("xl/worksheets/sheet1.xml", format!(r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:x="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main" xmlns:m="http://schemas.microsoft.com/office/excel/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:u="urn:one" xmlns:v="urn:one" mc:Ignorable="u" mc:ProcessContent="u:pass"><sheetData><row r="2"><c r="A2"><v>2</v></c></row><row r="3"><c r="A3"><v>3</v></c></row></sheetData><conditionalFormatting sqref="D2:D3"><cfRule type="expression" priority="1" xmlns:u="urn:two" mc:Ignorable="u"><v:pass><formula>Operations!A2&gt;0</formula></v:pass>{shadow}</cfRule></conditionalFormatting><extLst>{opaque}<ext uri="{{78C0D931-6437-407D-A8EE-F0AAD7539E65}}"><x:conditionalFormattings><x:conditionalFormatting><x:cfRule type="expression" priority="2" xmlns:u="urn:two" mc:Ignorable="u"><m:f><v:pass>Operations!A2&gt;0</v:pass></m:f>{shadow}</x:cfRule><m:sqref>D2:D3</m:sqref></x:conditionalFormatting></x:conditionalFormattings></ext><ext uri="{{05C60535-1F16-4FD2-B633-F4F36F0B64E0}}"><x:sparklineGroups><x:sparklineGroup dateAxis="1" xmlns:u="urn:two" mc:Ignorable="u"><v:pass><m:f>Operations!$A$2:$A$3</m:f></v:pass><x:sparklines><x:sparkline><m:f><v:pass>Operations!A2:A3</v:pass></m:f><m:sqref>F3</m:sqref>{shadow}</x:sparkline></x:sparklines></x:sparklineGroup></x:sparklineGroups></ext></extLst></worksheet>"#).into_bytes());
    let mut book = open(&package.write().unwrap());
    assert!(book.insert_rows(1, 1).unwrap());
    assert!(xml(&mut book).contains("<formula>Operations!A3&gt;0</formula>"));
    assert!(book.undo().unwrap());
    assert!(xml(&mut book).contains("<formula>Operations!A2&gt;0</formula>"));
    assert!(book.redo().unwrap());
    assert!(book.insert_columns(0, 1).unwrap());
    assert!(book.rename_sheet(0, "Renamed").unwrap());
    let saved = book.save_bytes().unwrap();
    let result = jade_opc::OpcPackage::open(&saved)
        .unwrap()
        .required_part_str("xl/worksheets/sheet1.xml")
        .unwrap();
    assert!(result.contains("sqref=\"E3:E4\""), "{result}");
    assert!(result.contains("<m:sqref>E3:E4</m:sqref>"), "{result}");
    assert!(result.contains("<m:sqref>G4</m:sqref>"), "{result}");
    assert!(
        result.contains("<formula>Renamed!B3&gt;0</formula>"),
        "{result}"
    );
    assert!(
        result.contains("<m:f><v:pass>Renamed!B3&gt;0</v:pass></m:f>"),
        "{result}"
    );
    assert!(
        result.contains("<m:f><v:pass>Renamed!B3:B4</v:pass></m:f>"),
        "{result}"
    );
    assert!(result.contains("<m:f>Renamed!$B$3:$B$4</m:f>"), "{result}");
    assert!(result.contains(opaque), "{result}");
    assert_eq!(result.matches(shadow).count(), 3, "{result}");
    assert!(!result.contains("_jade_ctx"), "{result}");
    let mut reopened = open(&saved);
    assert_eq!(xml(&mut reopened), result);
}

#[test]
fn removing_last_input_rule_preserves_opaque_container_data() {
    for ns in [
        "http://schemas.openxmlformats.org/spreadsheetml/2006/main",
        "http://purl.oclc.org/ooxml/spreadsheetml/main",
    ] {
        for extended in [false, true] {
            for operation in 0..3 {
                let (prefix, rule) = if extended {
                    ("x:", "<x:dataValidation type=\"whole\"><x:formula1><m:f>1</m:f></x:formula1><m:sqref>B2</m:sqref></x:dataValidation>")
                } else {
                    ("", "<dataValidation type=\"whole\" sqref=\"B2\"><formula1>1</formula1></dataValidation>")
                };
                let opaque = "<!--keep--><u:payload>A2</u:payload>";
                let list = format!("<{prefix}dataValidations count=\"1\" u:flag=\"keep\">{rule}{opaque}</{prefix}dataValidations>");
                let tail = if extended {
                    format!("<extLst><ext uri=\"{{CCE6A557-97BC-4B89-ADB6-D9C93CAAB3DF}}\">{list}</ext></extLst>")
                } else {
                    list
                };
                let mut package = jade_opc::OpcPackage::open(include_bytes!(
                    "../../../jade-core/public/fixtures/grid-operations-workbook.xlsx"
                ))
                .unwrap();
                package.remove("xl/worksheets/_rels/sheet1.xml.rels");
                package.replace("xl/worksheets/sheet1.xml", format!(r#"<worksheet xmlns="{ns}" xmlns:x="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main" xmlns:m="http://schemas.microsoft.com/office/excel/2006/main" xmlns:u="urn:foreign"><sheetData/>{tail}</worksheet>"#).into_bytes());
                let mut book = open(&package.write().unwrap());
                let before = xml(&mut book);
                assert_eq!(book.data_validations().len(), 1);
                assert!(match operation {
                    0 => book
                        .clear_data_validations(Some(
                            jade_xlsx_reader::CellRange::parse("B2").unwrap()
                        ))
                        .unwrap(),
                    1 => book.delete_rows(1, 1).unwrap(),
                    _ => book.delete_columns(1, 1).unwrap(),
                });
                assert!(book.data_validations().is_empty());
                let after = xml(&mut book);
                assert!(after.contains(&format!("<{prefix}dataValidations count=\"0\" u:flag=\"keep\">{opaque}</{prefix}dataValidations>")), "{after}");
                assert!(book.undo().unwrap());
                assert_eq!(xml(&mut book), before);
                assert!(book.redo().unwrap());
                assert_eq!(xml(&mut book), after);
                let mut reopened = open(&book.save_bytes().unwrap());
                assert_eq!(xml(&mut reopened), after);
            }
        }
    }
}

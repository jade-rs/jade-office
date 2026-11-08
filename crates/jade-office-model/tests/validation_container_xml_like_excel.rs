//! Compatible input-rule containers retain their checks through app edits.
#![cfg(feature = "tiny-skia")]

use jade_office_model::{
    aliased_faces,
    sheet::{CellRef, Workbook},
    tiny::TinySkiaBackend,
};
use jade_xlsx_reader::{Axis, CellRange};
use std::{path::PathBuf, sync::Arc};

fn open(bytes: &[u8]) -> Workbook<TinySkiaBackend> {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let font = core.join("fonts/Noto_Sans_JP/static/NotoSansJP-Regular.ttf");
    let backend =
        TinySkiaBackend::new(&aliased_faces(Arc::new(std::fs::read(font).unwrap()), None)).unwrap();
    Workbook::open(bytes, "UTC", backend).unwrap()
}

#[test]
fn conditional_and_processed_input_rules_keep_checks_history_and_saved_alternatives() {
    for inside_extension in [false, true] {
        for (extended, wrapper) in [
            (false, None),
            (false, Some("wrap")),
            (false, Some("dataValidations")),
            (false, Some("dataValidation")),
            (false, Some("protectedRanges")),
            (false, Some("mergeCells")),
            (true, None),
            (true, Some("wrap")),
            (true, Some("dataValidations")),
            (true, Some("dataValidation")),
            (true, Some("protectedRanges")),
            (true, Some("mergeCells")),
        ] {
            if inside_extension && !extended {
                continue;
            }
            let container = |suffix: &str| {
                if extended {
                    format!(
                        r#"<x:dataValidations count="1"><x:dataValidation type="whole" operator="lessThan" showErrorMessage="1"><x:formula1><m:f>Operations!$A1{suffix}</m:f></x:formula1><m:sqref>B1:B3</m:sqref></x:dataValidation></x:dataValidations>"#
                    )
                } else {
                    format!(
                        r#"<dataValidations count="1"><dataValidation type="whole" operator="lessThan" showErrorMessage="1" sqref="B1:B3"><formula1>Operations!$A1{suffix}</formula1></dataValidation></dataValidations>"#
                    )
                }
            };
            let opaque = r#"<mc:Choice Requires="u"><dataValidations count="1"><dataValidation sqref="Z9"><formula1>A99</formula1></dataValidation></dataValidations></mc:Choice>"#;
            let branches = format!(
                r#"<mc:AlternateContent>{opaque}<mc:Choice Requires="s">{}</mc:Choice><mc:Fallback>{}</mc:Fallback></mc:AlternateContent>"#,
                container(""),
                container("+1")
            );
            let wrap = |tail: String| {
                if let Some(wrapper) = wrapper {
                    format!(
                        r#"<u:{wrapper} keep="rules" mc:Ignorable="u" mc:ProcessContent="u:{wrapper}">{tail}</u:{wrapper}>"#
                    )
                } else {
                    tail
                }
            };
            let branches = if inside_extension && extended {
                wrap(branches)
            } else {
                branches
            };
            let tail = if extended {
                format!(
                    r#"<extLst><ext uri="{{CCE6A557-97BC-4B89-ADB6-D9C93CAAB3DF}}">{branches}</ext></extLst>"#
                )
            } else {
                branches
            };
            let tail = if !inside_extension || !extended {
                wrap(tail)
            } else {
                tail
            };
            let mut package = jade_opc::OpcPackage::open(include_bytes!(
                "../../../jade-core/public/fixtures/grid-operations-workbook.xlsx"
            ))
            .unwrap();
            let part = "xl/worksheets/sheet1.xml";
            package.remove("xl/worksheets/_rels/sheet1.xml.rels");
            package.replace(part, format!(r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:s="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:x="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main" xmlns:m="http://schemas.microsoft.com/office/excel/2006/main" xmlns:u="urn:future"><sheetData><row r="1"><c r="A1"><v>10</v></c></row><row r="2"><c r="A2"><v>20</v></c></row><row r="3"><c r="A3"><v>30</v></c></row></sheetData>{tail}</worksheet>"#).into_bytes());
            let mut book = open(&package.write().unwrap());
            let at = |text: &str| CellRef::parse(text).unwrap();
            assert_eq!(book.data_validations().len(), 1);
            assert!(book.validation_alert(at("B1"), "10").unwrap().is_some());
            assert!(book.validation_alert(at("B1"), "9").unwrap().is_none());
            assert!(book.validation_alert(at("B2"), "15").unwrap().is_none());
            assert!(book.delete_rows(0, 1).unwrap());
            assert!(book.validation_alert(at("B1"), "15").unwrap().is_none());
            assert!(book.undo().unwrap());
            assert!(book.validation_alert(at("B1"), "15").unwrap().is_some());
            assert!(book.redo().unwrap());
            assert!(book
                .shift_cells(CellRange::parse("B2").unwrap(), Axis::Rows, true)
                .unwrap());
            assert_eq!(book.data_validations().len(), 3);
            assert!(book.validation_alert(at("B1"), "25").unwrap().is_some());
            for cell in ["B2", "B3"] {
                assert!(book.validation_alert(at(cell), "25").unwrap().is_none());
            }
            assert!(book.rename_sheet(0, "Renamed").unwrap());
            assert!(book.insert_columns(0, 1).unwrap());
            let saved = book.save_bytes().unwrap();
            let package = jade_opc::OpcPackage::open(&saved).unwrap();
            let sheet = package.required_part_str(part).unwrap();
            assert!(sheet.contains(opaque));
            assert!(sheet.contains("<mc:Fallback>"));
            assert!(sheet.contains("Renamed!$B2+1"), "{sheet}");
            if wrapper.is_some() {
                assert!(sheet.contains("keep=\"rules\""));
            }
            let mut reopened = open(&saved);
            assert_eq!(reopened.data_validations(), book.data_validations());
            assert!(reopened.validation_alert(at("C1"), "9").unwrap().is_none());
            reopened.set_cell(at("B1"), "5").unwrap();
            assert!(reopened.validation_alert(at("C1"), "9").unwrap().is_some());
            assert!(reopened.validation_alert(at("C2"), "25").unwrap().is_none());
        }
    }
}

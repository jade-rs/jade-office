//! Partial edits preserve source metadata and independent feature identities.
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

const NAMESPACES: [&str; 2] = [
    "http://schemas.openxmlformats.org/spreadsheetml/2006/main",
    "http://purl.oclc.org/ooxml/spreadsheetml/main",
];
fn source(ns: &str, body: &str, wrapper: u8) -> Vec<u8> {
    let body = match wrapper {
        1 => format!("<u:pass>{body}</u:pass>"),
        2 => format!("<mc:AlternateContent><mc:Choice Requires=\"u\"><u:ignored/></mc:Choice><mc:Fallback>{body}</mc:Fallback></mc:AlternateContent>"),
        _ => body.into(),
    };
    let mut package = jade_opc::OpcPackage::open(include_bytes!(
        "../../../jade-core/public/fixtures/grid-operations-workbook.xlsx"
    ))
    .unwrap();
    package.remove("xl/worksheets/_rels/sheet1.xml.rels");
    package.replace("xl/worksheets/sheet1.xml", format!(r#"<worksheet xmlns="{ns}" xmlns:x="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main" xmlns:m="http://schemas.microsoft.com/office/excel/2006/main" xmlns:rev="http://schemas.microsoft.com/office/spreadsheetml/2014/revision" xmlns:u="urn:foreign" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" mc:Ignorable="u" mc:ProcessContent="u:pass"><sheetData/>{body}</worksheet>"#).into_bytes());
    package.write().unwrap()
}
fn history(book: &mut Workbook<TinySkiaBackend>, before: &str, after: &str) {
    assert!(book.undo().unwrap());
    assert_eq!(xml(book), before);
    assert!(book.redo().unwrap());
    assert_eq!(xml(book), after);
    let mut reopened = open(&book.save_bytes().unwrap());
    assert_eq!(xml(&mut reopened), after);
}
#[test]
fn partial_deletion_keeps_the_last_input_rule_containers_opaque_data() {
    use jade_xlsx_reader::{Axis, CellRange};
    let opaque = "<!--keep--><u:payload>B2</u:payload>";
    for ns in NAMESPACES {
        for extended in [false, true] {
            let (prefix, rule) = if extended {
                ("x:", "<x:dataValidation type=\"custom\"><x:formula1><m:f>1</m:f></x:formula1><m:sqref>B2</m:sqref></x:dataValidation>")
            } else {
                ("", "<dataValidation type=\"custom\" sqref=\"B2\"><formula1>1</formula1></dataValidation>")
            };
            let list = format!("<{prefix}dataValidations count=\"1\" u:flag=\"keep\">{rule}{opaque}</{prefix}dataValidations>");
            let body = if extended {
                format!("<extLst><ext uri=\"{{CCE6A557-97BC-4B89-ADB6-D9C93CAAB3DF}}\">{list}</ext></extLst>")
            } else {
                list
            };
            for wrapper in 0..3 {
                for axis in [Axis::Rows, Axis::Columns] {
                    let mut book = open(&source(ns, &body, wrapper));
                    assert_eq!(book.data_validations().len(), 1);
                    let before = xml(&mut book);
                    assert!(book
                        .shift_cells(CellRange::parse("B2").unwrap(), axis, false)
                        .unwrap());
                    assert!(book.data_validations().is_empty());
                    let after = xml(&mut book);
                    assert!(after.contains(&format!("<{prefix}dataValidations count=\"0\" u:flag=\"keep\">{opaque}</{prefix}dataValidations>")), "{after}");
                    history(&mut book, &before, &after);
                }
            }
        }
    }
}
#[test]
fn partial_splits_keep_opaque_identities_and_remove_temporary_context() {
    use jade_xlsx_reader::{Axis, CellRange};
    let foreign = "<u:data><dataValidation rev:uid=\"{11111111-2222-4333-8444-555555555555}\" sqref=\"Z1\"/><cfRule priority=\"1\"/></u:data>";
    let body = format!("<dataValidations count=\"1\"><dataValidation rev:uid=\"{{11111111-2222-4333-8444-555555555555}}\" type=\"custom\" sqref=\"B2:D4\"><formula1>1</formula1>{foreign}</dataValidation></dataValidations><extLst><ext uri=\"{{CCE6A557-97BC-4B89-ADB6-D9C93CAAB3DF}}\"><x:dataValidations count=\"1\"><x:dataValidation rev:uid=\"{{22222222-3333-4444-8555-666666666666}}\" type=\"custom\"><x:formula1><m:f>1</m:f></x:formula1><m:sqref>B2:D4</m:sqref>{foreign}</x:dataValidation></x:dataValidations></ext></extLst>");
    for ns in NAMESPACES {
        for wrapper in 0..3 {
            for axis in [Axis::Rows, Axis::Columns] {
                for inserted in [false, true] {
                    let mut book = open(&source(ns, &body, wrapper));
                    let before = xml(&mut book);
                    assert!(book
                        .shift_cells(CellRange::parse("C3").unwrap(), axis, inserted)
                        .unwrap());
                    let count = if inserted { 10 } else { 8 };
                    assert_eq!(book.data_validations().len(), count);
                    let after = xml(&mut book);
                    assert_eq!(after.matches(foreign).count(), count, "{after}");
                    assert!(!after.contains("xmlns:_jade_ctx"), "{after}");
                    history(&mut book, &before, &after);
                }
            }
        }
    }
}

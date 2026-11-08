//! Worksheet XML representations keep their formulas through app structure edits.
#![cfg(feature = "tiny-skia")]

use std::{path::PathBuf, sync::Arc};

use jade_office_model::{
    aliased_faces,
    sheet::{CellRef, Workbook},
    tiny::TinySkiaBackend,
};

fn open(bytes: &[u8]) -> Workbook<TinySkiaBackend> {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let font = core.join("fonts/Noto_Sans_JP/static/NotoSansJP-Regular.ttf");
    let backend =
        TinySkiaBackend::new(&aliased_faces(Arc::new(std::fs::read(font).unwrap()), None)).unwrap();
    Workbook::open(bytes, "UTC", backend).unwrap()
}

fn shown(book: &Workbook<TinySkiaBackend>, cells: &[&str]) -> Vec<String> {
    cells
        .iter()
        .map(|cell| book.display_text(CellRef::parse(cell).unwrap()))
        .collect()
}

#[test]
fn structure_edits_keep_defined_names_formula_branches_and_foreign_payloads() {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let source = std::fs::read(core.join("public/fixtures/grid-operations-workbook.xlsx")).unwrap();
    let part = jade_xlsx_reader::read_xlsx(&source).unwrap().sheets[0]
        .part_name
        .clone();
    let mut package = jade_opc::OpcPackage::open(&source).unwrap();
    let payload = "<u:c r=\"Z9\"><u:f>A1+99</u:f></u:c>";
    let sheet = format!("<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:m=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\" xmlns:u=\"urn:foreign\"><sheetData><row r=\"1\"><c r=\"A1\"><v>1</v></c>{payload}<c r=\"B1\"><f><![CDATA[Price+1]]></f><v>2</v></c><c r=\"C1\"><f><mc:AlternateContent><mc:Choice Requires=\"m\">A1+1</mc:Choice><mc:Fallback>A1+2</mc:Fallback></mc:AlternateContent></f><v>2</v></c></row></sheetData></worksheet>");
    package.replace(&part, sheet.into_bytes());
    let workbook = package.part_str("xl/workbook.xml").unwrap().unwrap();
    assert!(workbook.contains("</sheets>"));
    let workbook = workbook.replace("</sheets>", "</sheets><definedNames><definedName name=\"Price\"><![CDATA[Operations!$A$1]]></definedName></definedNames>");
    package.replace("xl/workbook.xml", workbook.into_bytes());
    let mut book = open(&package.write().unwrap());
    assert_eq!(shown(&book, &["A1", "B1", "C1"]), ["1", "2", "2"]);
    assert!(book.insert_rows(0, 1).unwrap());
    assert_eq!(shown(&book, &["A2", "B2", "C2"]), ["1", "2", "2"]);
    assert!(book.insert_columns(0, 1).unwrap());
    assert_eq!(shown(&book, &["B2", "C2", "D2"]), ["1", "2", "2"]);
    let saved = book.save_bytes().unwrap();
    let package = jade_opc::OpcPackage::open(&saved).unwrap();
    let sheet = package.part_str(&part).unwrap().unwrap();
    assert!(sheet.contains(payload));
    assert!(sheet.contains("<![CDATA[Price+1]]>"));
    assert!(sheet.contains(">B2+1</mc:Choice>"));
    assert!(sheet.contains(">B2+2</mc:Fallback>"));
    let saved_book = jade_xlsx_reader::read_xlsx(&saved).unwrap();
    assert_eq!(
        saved_book
            .defined_names
            .iter()
            .find(|name| name.name == "Price")
            .unwrap()
            .formula,
        "Operations!$B$2"
    );
    assert!(book.undo().unwrap());
    assert_eq!(shown(&book, &["A2", "B2", "C2"]), ["1", "2", "2"]);
    assert!(book.undo().unwrap());
    assert_eq!(shown(&book, &["A1", "B1", "C1"]), ["1", "2", "2"]);
    assert!(book.redo().unwrap());
    assert!(book.redo().unwrap());
    let mut reopened = open(&book.save_bytes().unwrap());
    assert_eq!(shown(&reopened, &["B2", "C2", "D2"]), ["1", "2", "2"]);
    reopened
        .set_cell(CellRef::parse("B2").unwrap(), "5")
        .unwrap();
    assert_eq!(shown(&reopened, &["B2", "C2", "D2"]), ["5", "6", "6"]);
}

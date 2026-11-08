//! Dynamic arrays as in Excel 365: a formula that returns several values
//! spills them into the cells beside it, the spill can be read whole with
//! `#`, and a value typed in its way blocks it with `#SPILL!`.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::sheet::{span, CellRef, Workbook};
use jade_office_model::tiny::TinySkiaBackend;

fn book() -> Workbook<TinySkiaBackend> {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let bytes = std::fs::read(core.join("public/fixtures/grid-operations-workbook.xlsx")).unwrap();
    open(&bytes)
}

fn open(bytes: &[u8]) -> Workbook<TinySkiaBackend> {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let directory = core.join("fonts/Noto_Sans_JP/static");
    let backend = TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        None,
    ))
    .unwrap();
    Workbook::open(bytes, "UTC", backend).unwrap()
}

#[test]
fn processed_cells_and_metadata_keep_calculation_history_and_reopen() {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let source = std::fs::read(core.join("public/fixtures/grid-operations-workbook.xlsx")).unwrap();
    let part = jade_xlsx_reader::read_xlsx(&source).unwrap().sheets[0]
        .part_name
        .clone();
    let mut package = jade_opc::OpcPackage::open(&source).unwrap();
    let ns = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
    let mc = "http://schemas.openxmlformats.org/markup-compatibility/2006";
    let sheet = format!("<worksheet xmlns=\"{ns}\" xmlns:mc=\"{mc}\" xmlns:u=\"urn:future\" mc:Ignorable=\"u\" mc:ProcessContent=\"u:wrap\"><sheetData><row r=\"1\"><c r=\"A1\" cm=\"1\"><v>2</v></c></row><row r=\"2\"><u:wrap keep=\"cell\"><c r=\"A2\"><u:wrap><f>A1+1</f><v>3</v></u:wrap></c></u:wrap></row></sheetData></worksheet>");
    package.replace(&part, sheet.into_bytes());
    let metadata = format!("<metadata xmlns=\"{ns}\" xmlns:mc=\"{mc}\" xmlns:u=\"urn:future\" mc:Ignorable=\"u\" mc:ProcessContent=\"u:wrap\"><u:wrap keep=\"metadata\"><metadataTypes count=\"1\"><metadataType name=\"OTHER\"/></metadataTypes><futureMetadata name=\"OTHER\" count=\"1\"><bk/></futureMetadata><cellMetadata count=\"1\"><bk><rc t=\"1\" v=\"0\"/></bk></cellMetadata></u:wrap></metadata>");
    package.replace("xl/metadata.xml", metadata.into_bytes());
    let rels = package.part_str("xl/_rels/workbook.xml.rels").unwrap().unwrap().replace("</Relationships>", "<Relationship Id=\"rId900\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/sheetMetadata\" Target=\"metadata.xml\"/></Relationships>");
    package.replace("xl/_rels/workbook.xml.rels", rels.into_bytes());
    let mut book = open(&package.write().unwrap());
    let at = |cell: &str| CellRef::parse(cell).unwrap();
    assert_eq!(shown(&book, &["A1", "A2"]), ["2", "3"]);
    book.set_cell(at("A1"), "5").unwrap();
    assert_eq!(shown(&book, &["A1", "A2"]), ["5", "6"]);
    book.set_cell(at("C1"), "=SEQUENCE(3)").unwrap();
    assert_eq!(shown(&book, &["C1", "C2", "C3"]), ["1", "2", "3"]);
    assert!(book.undo().unwrap());
    assert_eq!(shown(&book, &["C1", "C2", "C3"]), ["", "", ""]);
    assert!(book.redo().unwrap());
    let saved = book.save_bytes().unwrap();
    let package = jade_opc::OpcPackage::open(&saved).unwrap();
    let sheet = package.part_str(&part).unwrap().unwrap();
    assert!(sheet.contains("<u:wrap keep=\"cell\">"));
    assert!(sheet.contains("<f>A1+1</f><v>6</v>"));
    let metadata = package.part_str("xl/metadata.xml").unwrap().unwrap();
    assert!(metadata.contains("<u:wrap keep=\"metadata\">"));
    assert_eq!(metadata.matches("<metadataTypes ").count(), 1);
    assert_eq!(
        jade_xlsx_reader::array_metadata::cell_dynamic_array_flags(&metadata).unwrap(),
        [None, Some(true)]
    );
    let mut reopened = open(&saved);
    assert_eq!(
        shown(&reopened, &["A2", "C1", "C2", "C3"]),
        ["6", "1", "2", "3"]
    );
    reopened.set_cell(at("C1"), "=SEQUENCE(4)").unwrap();
    assert_eq!(
        shown(&reopened, &["C1", "C2", "C3", "C4"]),
        ["1", "2", "3", "4"]
    );
}

#[test]
fn supported_dynamic_choices_keep_spills_through_app_history_and_reopen() {
    let at = |cell: &str| CellRef::parse(cell).unwrap();
    let mut seed = book();
    seed.set_cell(at("J1"), "=SEQUENCE(3)").unwrap();
    let source = seed.save_bytes().unwrap();
    let part = jade_xlsx_reader::read_xlsx(&source).unwrap().sheets[0]
        .part_name
        .clone();
    let mut package = jade_opc::OpcPackage::open(&source).unwrap();
    let sheet = package.part_str(&part).unwrap().unwrap();
    let cells = jade_xlsx_reader::cell_content::compatible_cells(
        &sheet,
        &std::collections::HashSet::from([jade_xlsx_reader::CellRef::parse("J1").unwrap()]),
    )
    .unwrap();
    let cell = cells.into_iter().find(|cell| cell.selected).unwrap();
    let wrapped = format!("<mc:AlternateContent xmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\" xmlns:d=\"http://schemas.microsoft.com/office/spreadsheetml/2017/dynamicarray\"><mc:Choice Requires=\"d\">{}</mc:Choice><mc:Fallback><c r=\"J1\"><v>0</v></c></mc:Fallback></mc:AlternateContent>", &sheet[cell.range.clone()]);
    let mut sheet = sheet.to_string();
    sheet.replace_range(cell.range, &wrapped);
    package.replace(&part, sheet.into_bytes());
    let mut book = open(&package.write().unwrap());
    assert_eq!(shown(&book, &["J1", "J2", "J3"]), ["1", "2", "3"]);
    book.set_cell(at("J1"), "=SEQUENCE(4)").unwrap();
    assert_eq!(shown(&book, &["J1", "J4"]), ["1", "4"]);
    assert!(book.undo().unwrap());
    assert_eq!(shown(&book, &["J1", "J4"]), ["1", ""]);
    assert!(book.redo().unwrap());
    assert!(book.insert_rows(0, 1).unwrap());
    assert_eq!(shown(&book, &["J2", "J5"]), ["1", "4"]);
    let saved = book.save_bytes().unwrap();
    let package = jade_opc::OpcPackage::open(&saved).unwrap();
    let sheet = package.part_str(&part).unwrap().unwrap();
    assert!(sheet.contains("<mc:Choice Requires=\"d\">"));
    assert!(sheet.contains("<mc:Fallback>"));
    let parsed = jade_xlsx_reader::read_xlsx(&saved).unwrap();
    let array = &parsed.sheets[0].array_formulas[0];
    assert_eq!(array.kind, jade_xlsx_reader::ArrayFormulaKind::Dynamic);
    assert_eq!(
        array.range,
        jade_xlsx_reader::CellRange::parse("J2:J5").unwrap()
    );
    let mut reopened = open(&saved);
    reopened.set_cell(at("J2"), "=SEQUENCE(5)").unwrap();
    assert_eq!(shown(&reopened, &["J2", "J6"]), ["1", "5"]);
}

#[test]
fn unmerging_a_spill_obstruction_recalculates_and_restores_with_history() {
    let mut book = book();
    let at = |cell: &str| CellRef::parse(cell).unwrap();
    let obstruction = span(at("J2"), at("K2"));
    assert!(book.merge_and_center(obstruction).unwrap());
    book.set_cell(at("J1"), "=SEQUENCE(3)").unwrap();
    book.set_cell(at("L1"), "=SUM(J1#)").unwrap();
    assert_eq!(
        shown(&book, &["J1", "J2", "J3", "L1"]),
        ["#SPILL!", "", "", "#SPILL!"]
    );

    assert!(book.merge_and_center(obstruction).unwrap());
    assert_eq!(
        shown(&book, &["J1", "J2", "J3", "L1"]),
        ["1", "2", "3", "6"]
    );
    assert!(book.undo().unwrap());
    assert_eq!(
        shown(&book, &["J1", "J2", "J3", "L1"]),
        ["#SPILL!", "", "", "#SPILL!"]
    );
    assert!(book.redo().unwrap());
    assert_eq!(
        shown(&book, &["J1", "J2", "J3", "L1"]),
        ["1", "2", "3", "6"]
    );

    let bytes = book.save_bytes().unwrap();
    let saved = jade_xlsx_reader::read_xlsx(&bytes).unwrap();
    let array = saved.sheets[0]
        .array_formulas
        .iter()
        .find(|array| array.anchor == at("J1"))
        .unwrap();
    assert_eq!(array.kind, jade_xlsx_reader::ArrayFormulaKind::Dynamic);
    assert_eq!(array.range, span(at("J1"), at("J3")));
    let mut reopened = open(&bytes);
    assert_eq!(
        shown(&reopened, &["J1", "J2", "J3", "L1"]),
        ["1", "2", "3", "6"]
    );
    reopened.set_cell(at("J1"), "=SEQUENCE(4)").unwrap();
    assert_eq!(
        shown(&reopened, &["J1", "J2", "J3", "J4", "L1"]),
        ["1", "2", "3", "4", "10"]
    );
}

fn shown(book: &Workbook<TinySkiaBackend>, cells: &[&str]) -> Vec<String> {
    cells
        .iter()
        .map(|cell| book.display_text(CellRef::parse(cell).unwrap()))
        .collect()
}

#[test]
fn a_spilling_formula_fills_its_cells_and_a_value_in_its_way_blocks_it() {
    let mut book = book();
    let at = |cell: &str| CellRef::parse(cell).unwrap();
    book.set_cell(at("J1"), "=SEQUENCE(3)").unwrap();
    assert_eq!(shown(&book, &["J1", "J2", "J3", "J4"]), ["1", "2", "3", ""]);
    book.set_cell(at("L1"), "=SUM(J1#)").unwrap();
    assert_eq!(shown(&book, &["L1"]), ["6"]);

    book.set_cell(at("J2"), "x").unwrap();
    assert_eq!(
        shown(&book, &["J1", "J2", "J3", "L1"]),
        ["#SPILL!", "x", "", "#SPILL!"]
    );
    assert!(book.undo().unwrap());
    assert_eq!(
        shown(&book, &["J1", "J2", "J3", "L1"]),
        ["1", "2", "3", "6"]
    );
}

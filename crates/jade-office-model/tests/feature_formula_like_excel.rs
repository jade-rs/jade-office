//! Feature formulas remain aligned through the app's editing and history paths.
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

#[test]
fn sparkline_and_conditional_sources_survive_app_history_and_first_cell_deletion() {
    let opaque = r#"<ext uri="urn:opaque"><m:f>Operations!Z1</m:f></ext>"#;
    let mut package = jade_opc::OpcPackage::open(include_bytes!(
        "../../../jade-core/public/fixtures/grid-operations-workbook.xlsx"
    ))
    .unwrap();
    let part = "xl/worksheets/sheet1.xml";
    package.remove("xl/worksheets/_rels/sheet1.xml.rels");
    package.replace(part,format!(r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:x="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main" xmlns:m="http://schemas.microsoft.com/office/excel/2006/main"><sheetData><row r="1"><c r="A1"><v>1</v></c></row><row r="2"><c r="A2"><v>2</v></c></row><row r="3"><c r="A3"><v>3</v></c></row></sheetData><conditionalFormatting sqref="D1:D3"><cfRule type="expression" priority="1"><formula>Operations!A1&gt;0</formula></cfRule></conditionalFormatting><extLst><ext uri="{{05C60535-1F16-4FD2-B633-F4F36F0B64E0}}"><x:sparklineGroups><x:sparklineGroup dateAxis="1"><m:f>Operations!$A$1:$A$3</m:f><x:sparklines><x:sparkline><m:f>Operations!A1:A3</m:f><m:sqref>F3</m:sqref></x:sparkline></x:sparklines></x:sparklineGroup></x:sparklineGroups></ext>{opaque}</extLst></worksheet>"#).into_bytes());
    let mut book = open(&package.write().unwrap());
    assert!(book.insert_rows(0, 1).unwrap());
    let saved = book.save_bytes().unwrap();
    assert_eq!(
        jade_xlsx_reader::read_xlsx(&saved).unwrap().sheets[0].sparkline_groups[0].sparklines[0]
            .formula,
        "Operations!A2:A4"
    );
    assert!(book.undo().unwrap());
    assert_eq!(
        jade_xlsx_reader::read_xlsx(&book.save_bytes().unwrap())
            .unwrap()
            .sheets[0]
            .sparkline_groups[0]
            .sparklines[0]
            .formula,
        "Operations!A1:A3"
    );
    assert!(book.redo().unwrap());
    assert!(book.delete_rows(1, 1).unwrap());
    assert!(book.insert_columns(0, 1).unwrap());
    assert!(book.rename_sheet(0, "Renamed").unwrap());
    let saved = book.save_bytes().unwrap();
    let parsed = jade_xlsx_reader::read_xlsx(&saved).unwrap();
    let group = &parsed.sheets[0].sparkline_groups[0];
    assert_eq!(group.date_range.as_deref(), Some("Renamed!$B$2:$B$3"));
    assert_eq!(group.sparklines[0].formula, "Renamed!B2:B3");
    assert_eq!(
        group.sparklines[0].cell,
        jade_xlsx_reader::CellRef::parse("G3").unwrap()
    );
    let sheet = jade_opc::OpcPackage::open(&saved)
        .unwrap()
        .required_part_str(part)
        .unwrap();
    assert!(
        sheet.contains("<formula>Renamed!B2&gt;0</formula>"),
        "{sheet}"
    );
    assert!(sheet.contains("sqref=\"E2:E3\""), "{sheet}");
    assert!(sheet.contains(opaque));
    let mut reopened = open(&saved);
    assert_eq!(
        jade_xlsx_reader::read_xlsx(&reopened.save_bytes().unwrap())
            .unwrap()
            .sheets[0]
            .sparkline_groups,
        parsed.sheets[0].sparkline_groups
    );
}

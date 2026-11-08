//! Last-link deletion keeps opaque metadata through app history and later edits.
#![cfg(feature = "tiny-skia")]
use jade_office_model::{
    aliased_faces,
    sheet::{CellRef, Workbook},
    tiny::TinySkiaBackend,
};
use jade_xlsx_reader::{Axis, CellRange, Hyperlink};
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
fn last_link_removal_preserves_source_and_later_links_stay_live() {
    let at = CellRef::parse("B2").unwrap();
    let metadata = "<!--keep--><u:hyperlink ref=\"B2\"/><u:payload>B2</u:payload>";
    for ns in [
        "http://schemas.openxmlformats.org/spreadsheetml/2006/main",
        "http://purl.oclc.org/ooxml/spreadsheetml/main",
    ] {
        for opaque in [false, true] {
            for mode in ["clear", "partial-row", "partial-column", "row", "column"] {
                let mut source = jade_opc::OpcPackage::open(include_bytes!(
                    "../../../jade-core/public/fixtures/grid-operations-workbook.xlsx"
                ))
                .unwrap();
                source.remove("xl/worksheets/_rels/sheet1.xml.rels");
                let attrs = if opaque { " u:flag=\"keep\"" } else { "" };
                let payload = if opaque { metadata } else { "" };
                source.replace("xl/worksheets/sheet1.xml", format!(r#"<worksheet xmlns="{ns}" xmlns:u="urn:foreign" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" mc:Ignorable="u"><sheetData/><hyperlinks{attrs}><hyperlink ref="B2" location="Destination"/>{payload}</hyperlinks></worksheet>"#).into_bytes());
                let mut book = open(&source.write().unwrap());
                assert!(book.hyperlink(at).is_some());
                let before = xml(&mut book);
                assert!(match mode {
                    "clear" => book.clear_hyperlinks(&[CellRange::parse("B2").unwrap()]),
                    "partial-row" =>
                        book.shift_cells(CellRange::parse("B2").unwrap(), Axis::Rows, false),
                    "partial-column" =>
                        book.shift_cells(CellRange::parse("B2").unwrap(), Axis::Columns, false),
                    "row" => book.delete_rows(1, 1),
                    _ => book.delete_columns(1, 1),
                }
                .unwrap());
                assert!(book.hyperlink(at).is_none());
                let after = xml(&mut book);
                if opaque {
                    assert!(after.contains(metadata), "{mode}: {after}");
                    assert!(after.contains("u:flag=\"keep\""));
                    assert!(after.contains("urn:jade-office:preserved-ooxml"));
                } else {
                    assert!(!after.contains("<hyperlinks"), "{mode}: {after}");
                }
                assert!(book.undo().unwrap());
                assert_eq!(xml(&mut book), before);
                assert!(book.hyperlink(at).is_some());
                assert!(book.redo().unwrap());
                assert_eq!(xml(&mut book), after);
                let mut reopened = open(&book.save_bytes().unwrap());
                assert_eq!(xml(&mut reopened), after);
                assert!(reopened.hyperlink(at).is_none());
                assert!(reopened
                    .insert_hyperlink(
                        &Hyperlink {
                            reference: CellRange::parse("B2").unwrap(),
                            target: None,
                            location: Some("Destination".into()),
                            display: None,
                            tooltip: None
                        },
                        None
                    )
                    .unwrap());
                assert!(reopened.insert_rows(0, 1).unwrap());
                assert!(reopened.hyperlink(CellRef::parse("B3").unwrap()).is_some());
                if opaque {
                    assert!(xml(&mut reopened).contains(metadata));
                }
            }
        }
    }
}

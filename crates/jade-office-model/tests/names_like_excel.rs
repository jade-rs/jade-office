#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::sheet::Workbook;
use jade_office_model::tiny::TinySkiaBackend;

fn book() -> Workbook<TinySkiaBackend> {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let directory = core.join("fonts/Noto_Sans_JP/static");
    let backend = TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        None,
    ))
    .unwrap();
    // Table1 on A1:C5: Item, Qty, Amount; Total row SUBTOTAL(109,Table1[Amount]).
    let bytes = std::fs::read(core.join("public/fixtures/sample-workbook.xlsx")).unwrap();
    Workbook::open(&bytes, "UTC", backend).unwrap()
}

#[test]
fn the_name_box_defines_and_navigates_ranges_including_other_sheets() {
    let mut book = book();
    let range = jade_office_model::sheet::CellRange::parse("B4:C6").unwrap();
    book.define_range_name("Sales", range).unwrap();
    assert_eq!(book.resolve_reference("sales"), Some((0, range)));
    book.select_sheet(1).unwrap();
    assert_eq!(book.resolve_reference("Sales"), Some((0, range)));
    assert_eq!(
        book.resolve_reference("'売上サマリ'!$B$4:$C$6"),
        Some((0, range))
    );
    book.set_defined_name(&jade_office_model::sheet::DefinedName {
        name: "Sales".into(),
        formula: "Notes!$A$1:$A$2".into(),
        local_sheet_index: Some(1),
        hidden: false,
    })
    .unwrap();
    assert_eq!(
        book.resolve_reference("Sales"),
        Some((
            1,
            jade_office_model::sheet::CellRange::parse("A1:A2").unwrap()
        ))
    );
    book.undo().unwrap();
    assert_eq!(book.resolve_reference("Sales"), Some((0, range)));
    let bytes = book.save_bytes().unwrap();
    let backend = TinySkiaBackend::new(&aliased_faces(
        Arc::new(
            std::fs::read(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../jade-core/fonts/Noto_Sans_JP/static/NotoSansJP-Regular.ttf"
            ))
            .unwrap(),
        ),
        None,
    ))
    .unwrap();
    let reopened = Workbook::open(&bytes, "UTC", backend).unwrap();
    assert_eq!(reopened.resolve_reference("Sales"), Some((0, range)));
}

#[test]
fn hide_unhide_and_history_keep_the_active_tab_visible() {
    use jade_office_model::sheet::SheetVisibility;
    let mut book = book();
    book.set_sheet_visibility(0, SheetVisibility::Hidden)
        .unwrap();
    assert_eq!(book.sheet_index(), 1);
    assert_eq!(book.sheet_tabs(), [1]);
    assert_eq!(book.hidden_sheets(), [0]);
    book.set_sheet_visibility(0, SheetVisibility::Visible)
        .unwrap();
    book.select_sheet(0).unwrap();
    book.undo().unwrap();
    assert_eq!(book.sheet_index(), 1);
    assert_eq!(book.sheet_tabs(), [1]);
    book.redo().unwrap();
    assert_eq!(book.sheet_tabs(), [0, 1]);
    book.set_sheet_visibility(0, SheetVisibility::VeryHidden)
        .unwrap();
    assert!(
        book.hidden_sheets().is_empty(),
        "VeryHidden is absent from Excel's Unhide menu"
    );
    assert!(book
        .set_sheet_visibility(1, SheetVisibility::Hidden)
        .is_err());
}

#[test]
fn a_name_box_entry_cannot_overwrite_a_named_constant() {
    let mut book = book();
    book.set_defined_name(&jade_office_model::sheet::DefinedName {
        name: "Tax".into(),
        formula: "0.1".into(),
        local_sheet_index: None,
        hidden: false,
    })
    .unwrap();
    assert_eq!(book.resolve_reference("Tax"), None);
    assert!(book
        .define_range_name(
            "Tax",
            jade_office_model::sheet::CellRange::parse("A1:B2").unwrap()
        )
        .is_err());
    assert_eq!(
        book.defined_names()
            .iter()
            .find(|n| n.name == "Tax")
            .unwrap()
            .formula,
        "0.1"
    );
}

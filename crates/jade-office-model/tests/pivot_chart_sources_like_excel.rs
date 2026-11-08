//! Renamed pivot-report tabs keep chart references through app edits and save.
#![cfg(feature = "tiny-skia")]
use jade_office_model::{
    aliased_faces,
    sheet::{CellRef, Workbook},
    tiny::TinySkiaBackend,
};
use jade_xlsx_reader::{chart::Chart, read_xlsx};
use std::sync::Arc;

fn open(bytes: &[u8]) -> Workbook<TinySkiaBackend> {
    let font = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../jade-core/fonts/Noto_Sans_JP/static/NotoSansJP-Regular.ttf"
    ))
    .unwrap();
    Workbook::open(
        bytes,
        "UTC",
        TinySkiaBackend::new(&aliased_faces(Arc::new(font), None)).unwrap(),
    )
    .unwrap()
}
fn chart(bytes: &[u8]) -> Chart {
    read_xlsx(bytes)
        .unwrap()
        .sheets
        .into_iter()
        .flat_map(|sheet| sheet.shapes)
        .find_map(|shape| shape.chart)
        .unwrap()
}

#[test]
fn renamed_report_refreshes_chart_formulas_and_caches_after_reopen() {
    let mut book = open(include_bytes!(
        "../../../jade-core/public/fixtures/pivot-chart-compat.xlsx"
    ));
    let before = book.save_bytes().unwrap();
    let target = "O'Brien & _x0041_";
    assert!(book.rename_sheet(1, target).unwrap());
    let renamed = book.save_bytes().unwrap();
    let binding = chart(&renamed)
        .pivot_source
        .unwrap()
        .qualified_name()
        .unwrap();
    assert_eq!(binding.sheet, target);
    assert_eq!(
        chart(&renamed).series[0].values.formula.as_deref(),
        Some("'O''Brien & _x0041_'!$F$4:$F$5")
    );
    assert!(book.undo().unwrap());
    assert_eq!(book.save_bytes().unwrap(), before);
    assert!(book.redo().unwrap());
    assert_eq!(book.save_bytes().unwrap(), renamed);
    let mut reopened = open(&renamed);
    reopened.select_sheet(0).unwrap();
    reopened
        .set_cell(CellRef::parse("A3").unwrap(), "North")
        .unwrap();
    reopened.select_sheet(1).unwrap();
    assert!(reopened.refresh_pivot_named("RegionalSales").unwrap());
    let saved = reopened.save_bytes().unwrap();
    let result = chart(&saved);
    assert_eq!(result.series[0].values.points.len(), 3);
    assert_eq!(
        result.series[0].values.formula.as_deref(),
        Some("'O''Brien & _x0041_'!$F$4:$F$6")
    );
    let mut again = open(&saved);
    assert_eq!(
        chart(&again.save_bytes().unwrap()).series[0].values,
        result.series[0].values
    );
}

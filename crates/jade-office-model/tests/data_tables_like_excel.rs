//! Data ▸ What-If Analysis ▸ Data Table as Excel's: a formula worked out
//! for each value of an input cell (one-way) or each pair of values of two
//! (two-way), `{=TABLE(row,column)}` in the results, worked out again as
//! the values change, saved as Excel saves it.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::data_table::DATA_TABLE_REFUSED;
use jade_office_model::sheet::{span, CellRef, Workbook};
use jade_office_model::tiny::TinySkiaBackend;
use jade_office_model::what_if::{WhatIfDialog, WhatIfTool};

fn backend() -> TinySkiaBackend {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let directory = core.join("fonts/Noto_Sans_JP/static");
    TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        None,
    ))
    .unwrap()
}

fn book() -> Workbook<TinySkiaBackend> {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let bytes = std::fs::read(core.join("public/fixtures/grid-operations-workbook.xlsx")).unwrap();
    Workbook::open(&bytes, "UTC", backend()).unwrap()
}

fn at(column: u32, row: u32) -> CellRef {
    CellRef::new(column, row)
}

/// H30 a price, H31 a quantity, H32 their product; J30:K33 a one-way table
/// of prices down J31:J33 with the product heading K30.
fn one_way(book: &mut Workbook<TinySkiaBackend>) {
    book.set_cell(at(7, 29), "100").unwrap();
    book.set_cell(at(7, 30), "3").unwrap();
    book.set_cell(at(7, 31), "=H30*H31").unwrap();
    for (row, price) in [(30, "10"), (31, "20"), (32, "30")] {
        book.set_cell(at(9, row), price).unwrap();
    }
    book.set_cell(at(10, 29), "=H32").unwrap();
}

#[test]
fn a_one_way_table_works_the_formula_out_for_each_value_down_the_side() {
    let mut book = book();
    one_way(&mut book);
    assert!(book
        .data_table(span(at(9, 29), at(10, 32)), None, Some(at(7, 29)))
        .unwrap());
    let results: Vec<String> = (30..=32).map(|row| book.display_text(at(10, row))).collect();
    assert_eq!(results, ["30", "60", "90"]);
    assert_eq!(book.edit_text(at(10, 31)), "{=TABLE(,H30)}");
    // The input cell keeps its own value.
    assert_eq!(book.display_text(at(7, 31)), "300");
    // Part of a table cannot be typed over.
    assert_eq!(
        book.set_cell(at(10, 31), "5").unwrap_err(),
        DATA_TABLE_REFUSED
    );
    // A value changed: the table follows.
    book.set_cell(at(7, 30), "4").unwrap();
    assert_eq!(book.display_text(at(10, 30)), "40");
    book.set_cell(at(9, 32), "50").unwrap();
    assert_eq!(book.display_text(at(10, 32)), "200");
}

#[test]
fn a_one_way_table_along_the_top_takes_a_row_input_cell() {
    let mut book = book();
    book.set_cell(at(7, 29), "2").unwrap();
    book.set_cell(at(7, 30), "=H30^2").unwrap();
    // J30 empty, K30:M30 the values, J31 the formula heading its row.
    for (column, value) in [(10, "1"), (11, "2"), (12, "3")] {
        book.set_cell(at(column, 29), value).unwrap();
    }
    book.set_cell(at(9, 30), "=H31").unwrap();
    book.data_table(span(at(9, 29), at(12, 30)), Some(at(7, 29)), None)
        .unwrap();
    let results: Vec<String> = (10..=12).map(|column| book.display_text(at(column, 30))).collect();
    assert_eq!(results, ["1", "4", "9"]);
    assert_eq!(book.edit_text(at(11, 30)), "{=TABLE(H30,)}");
}

#[test]
fn a_two_way_table_takes_both_input_cells_round_its_corner_formula() {
    let mut book = book();
    book.set_cell(at(7, 29), "1").unwrap();
    book.set_cell(at(7, 30), "1").unwrap();
    // J30 the corner formula, K30:L30 widths, J31:J32 heights.
    book.set_cell(at(9, 29), "=H30*H31").unwrap();
    book.set_cell(at(10, 29), "2").unwrap();
    book.set_cell(at(11, 29), "3").unwrap();
    book.set_cell(at(9, 30), "10").unwrap();
    book.set_cell(at(9, 31), "20").unwrap();
    let mut dialog = WhatIfDialog::open(
        WhatIfTool::DataTable,
        &book,
        span(at(9, 29), at(11, 31)),
        at(9, 29),
    );
    dialog.form_mut().set_text("row", "$H$30");
    dialog.form_mut().set_text("column", "H31");
    let outcome = dialog.press("ok", &mut book).unwrap();
    assert!(outcome.close && outcome.wrote);
    let grid: Vec<String> = [(10, 30), (11, 30), (10, 31), (11, 31)]
        .iter()
        .map(|&(column, row)| book.display_text(at(column, row)))
        .collect();
    assert_eq!(grid, ["20", "30", "40", "60"]);
    assert_eq!(book.edit_text(at(11, 31)), "{=TABLE(H30,H31)}");
    // One undo takes the table away.
    assert!(book.undo().unwrap());
    assert_eq!(book.display_text(at(10, 30)), "");
    assert!(book.data_table_at(at(10, 30)).is_none());
}

#[test]
fn the_dialog_wants_an_input_cell() {
    let mut book = book();
    one_way(&mut book);
    let mut dialog = WhatIfDialog::open(
        WhatIfTool::DataTable,
        &book,
        span(at(9, 29), at(10, 32)),
        at(9, 29),
    );
    let outcome = dialog.press("ok", &mut book).unwrap();
    assert!(!outcome.close);
    assert!(dialog.form().alert.is_some());
    dialog.form_mut().set_text("column", "Z");
    assert!(!dialog.press("ok", &mut book).unwrap().close);
}

#[test]
fn a_table_is_saved_as_excel_saves_it_and_read_back() {
    let mut book = book();
    one_way(&mut book);
    book.data_table(span(at(9, 29), at(10, 32)), None, Some(at(7, 29)))
        .unwrap();
    let bytes = book.save_bytes().unwrap();
    let package = jade_opc::OpcPackage::open(&bytes).unwrap();
    let sheet = (1..=8)
        .filter_map(|n| {
            package
                .part_str(&format!("xl/worksheets/sheet{n}.xml"))
                .ok()
                .flatten()
        })
        .find(|xml| xml.contains("dataTable"))
        .expect("a sheet with the table");
    assert!(
        sheet.contains(r#"<f t="dataTable" ref="K31:K33" dt2D="0" dtr="0" r1="H30"/>"#),
        "{}",
        &sheet[sheet.find("dataTable").unwrap() - 40..][..120]
    );
    let mut again = Workbook::open(&bytes, "UTC", backend()).unwrap();
    again.select_sheet(book.sheet_index()).unwrap();
    assert_eq!(again.edit_text(at(10, 30)), "{=TABLE(,H30)}");
    again.set_cell(at(9, 30), "1").unwrap();
    assert_eq!(again.display_text(at(10, 30)), "3");
}

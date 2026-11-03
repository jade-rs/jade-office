//! Excel's data tools: Data ▸ Consolidate (by position or by labels, with
//! links to the sources in an outline), Data ▸ Advanced (in place or copied
//! elsewhere, computed criteria, unique records), and custom lists for the
//! Sort dialog's Order; and Data ▸ Forecast Sheet — each one undo step,
//! saved as Excel saves it.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use jade_office_model::advanced_filter::AdvancedFilter;
use jade_office_model::aliased_faces;
use jade_office_model::consolidate::{ConsolidateFunction, Consolidation};
use jade_office_model::custom_lists;
use jade_office_model::forecast_sheet::{Forecast, ForecastWords};
use jade_office_model::sheet::{CellRef, Workbook};
use jade_office_model::sort_dialog::SortDialog;
use jade_office_model::tiny::TinySkiaBackend;
use jade_office_model::what_if::{WhatIfDialog, WhatIfTool};
use jade_xlsx_reader::CellRange;

fn book() -> Workbook<TinySkiaBackend> {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let directory = core.join("fonts/Noto_Sans_JP/static");
    let backend = TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        None,
    ))
    .unwrap();
    let bytes = std::fs::read(core.join("public/fixtures/grid-operations-workbook.xlsx")).unwrap();
    Workbook::open(&bytes, "UTC", backend).unwrap()
}

fn at(text: &str) -> CellRef {
    jade_xlsx_reader::CellRef::parse(text).unwrap()
}

fn range(text: &str) -> CellRange {
    CellRange::parse(text).unwrap()
}

fn fill(book: &mut Workbook<TinySkiaBackend>, cells: &[(&str, &str)]) {
    for (cell, value) in cells {
        book.set_cell(at(cell), value).unwrap();
    }
}

/// Two regions' sales on two sheets, labelled down the side and along the
/// top, their products not quite the same.
fn regions(book: &mut Workbook<TinySkiaBackend>) -> (usize, usize) {
    let first = book.sheet_index();
    fill(
        book,
        &[
            ("H40", ""),
            ("I40", "Q1"),
            ("J40", "Q2"),
            ("H41", "Apples"),
            ("I41", "10"),
            ("J41", "20"),
            ("H42", "Pears"),
            ("I42", "5"),
            ("J42", "6"),
        ],
    );
    let second = book.add_sheet().unwrap();
    book.select_sheet(second).unwrap();
    fill(
        book,
        &[
            ("A1", ""),
            ("B1", "Q2"),
            ("C1", "Q3"),
            ("A2", "pears"),
            ("B2", "4"),
            ("C2", "7"),
            ("A3", "Plums"),
            ("B3", "1"),
            ("C3", "2"),
        ],
    );
    (first, second)
}

#[test]
fn consolidate_sums_by_labels_down_the_side_and_along_the_top_in_one_step() {
    let mut b = book();
    let (first, second) = regions(&mut b);
    let summary = b.add_sheet().unwrap();
    b.select_sheet(summary).unwrap();
    let job = Consolidation {
        function: ConsolidateFunction::Sum,
        sources: vec![(first, range("H40:J42")), (second, range("A1:C3"))],
        top_row: true,
        left_column: true,
        links: false,
    };
    let written = b.consolidate(at("B2"), &job).unwrap();
    assert_eq!(written, range("B2:E5"));
    // Q1, Q2, Q3 along the top; Apples, Pears (matched without case),
    // Plums down the side, as first met.
    assert_eq!(b.display_text(at("C2")), "Q1");
    assert_eq!(b.display_text(at("E2")), "Q3");
    assert_eq!(b.display_text(at("B3")), "Apples");
    assert_eq!(b.display_text(at("B4")), "Pears");
    assert_eq!(b.display_text(at("B5")), "Plums");
    assert_eq!(b.number(at("D4")), Some(10.0), "Pears Q2: 6 + 4");
    assert_eq!(b.number(at("C3")), Some(10.0));
    assert_eq!(b.number(at("E5")), Some(2.0));
    assert_eq!(b.display_text(at("C5")), "", "no Plums in Q1");
    assert_eq!(b.edit_text(at("D4")), "10", "values, not formulas");
    assert!(b.undo().unwrap());
    assert_eq!(b.display_text(at("C2")), "");
}

#[test]
fn consolidate_by_position_averages_what_sits_in_the_same_place() {
    let mut b = book();
    fill(
        &mut b,
        &[
            ("H40", "1"),
            ("I40", "2"),
            ("H41", "3"),
            ("H50", "5"),
            ("I50", "text"),
            ("H51", "7"),
            ("I51", "8"),
        ],
    );
    let here = b.sheet_index();
    let job = Consolidation {
        function: ConsolidateFunction::Average,
        sources: vec![(here, range("H40:I41")), (here, range("H50:I51"))],
        top_row: false,
        left_column: false,
        links: false,
    };
    b.consolidate(at("K40"), &job).unwrap();
    assert_eq!(b.number(at("K40")), Some(3.0));
    assert_eq!(b.number(at("L40")), Some(2.0), "text is left out");
    assert_eq!(b.number(at("K41")), Some(5.0));
    assert_eq!(b.number(at("L41")), Some(8.0));
    // The destination inside a source is refused, as Excel refuses it.
    assert_eq!(
        b.consolidate(at("H41"), &job).unwrap_err(),
        "consolidate.alert.overlap"
    );
}

#[test]
fn consolidate_with_links_writes_detail_rows_under_an_outline() {
    let mut b = book();
    let (first, second) = regions(&mut b);
    let summary = b.add_sheet().unwrap();
    b.select_sheet(summary).unwrap();
    let job = Consolidation {
        function: ConsolidateFunction::Sum,
        sources: vec![(first, range("H40:J42")), (second, range("A1:C3"))],
        top_row: true,
        left_column: true,
        links: true,
    };
    b.consolidate(at("A1"), &job).unwrap();
    // Apples: one detail row, then its sum; Pears: two, then the sum.
    let names = b.sheet_names();
    assert_eq!(b.display_text(at("B2")), names[first]);
    assert!(
        b.edit_text(at("C2")).ends_with("!$I$41"),
        "{}",
        b.edit_text(at("C2"))
    );
    assert_eq!(b.display_text(at("A3")), "Apples");
    assert_eq!(b.edit_text(at("C3")), "=SUM(C2:C2)");
    assert_eq!(b.number(at("C3")), Some(10.0));
    assert_eq!(b.display_text(at("A6")), "Pears");
    assert_eq!(b.edit_text(at("D6")), "=SUM(D4:D5)");
    assert_eq!(b.number(at("D6")), Some(10.0));
    assert!(b.has_outline(jade_office_model::sheet::Axis::Rows));
    // Links to the destination's own sheet are refused.
    let here = b.sheet_index();
    let mut same = job.clone();
    same.sources = vec![(here, range("A1:B2"))];
    assert_eq!(
        b.consolidate(at("H1"), &same).unwrap_err(),
        "consolidate.alert.links_same_sheet"
    );
}

#[test]
fn the_consolidate_dialog_adds_references_and_runs() {
    let mut b = book();
    let (first, second) = regions(&mut b);
    b.select_sheet(first).unwrap();
    let mut dialog = WhatIfDialog::open(WhatIfTool::Consolidate, &b, range("L40:L40"), at("L40"));
    let names = b.sheet_names();
    dialog
        .form_mut()
        .set_text("reference", &format!("{}!A1:C3", names[second]));
    dialog.press("add", &mut b).unwrap();
    dialog.form_mut().set_text("reference", "H40:J42");
    dialog.press("add", &mut b).unwrap();
    dialog.form_mut().set_checked("top", true);
    dialog.form_mut().set_checked("left", true);
    let outcome = dialog.press("ok", &mut b).unwrap();
    assert!(outcome.close && outcome.wrote);
    assert_eq!(b.number(at("N42")), Some(10.0), "Pears Q2");
}

/// A list of salespeople in H40:J45, and criteria beside it.
fn sales(book: &mut Workbook<TinySkiaBackend>) {
    fill(
        book,
        &[
            ("H40", "Name"),
            ("I40", "Region"),
            ("J40", "Sales"),
            ("H41", "Davolio"),
            ("I41", "East"),
            ("J41", "100"),
            ("H42", "Buchanan"),
            ("I42", "West"),
            ("J42", "400"),
            ("H43", "Davis"),
            ("I43", "East"),
            ("J43", "250"),
            ("H44", "Suyama"),
            ("I44", "West"),
            ("J44", "50"),
            ("H45", "Davolio"),
            ("I45", "East"),
            ("J45", "100"),
        ],
    );
}

/// The rows of the active sheet the saved file hides, 0-based.
fn hidden_rows(book: &mut Workbook<TinySkiaBackend>) -> Vec<u32> {
    let here = book.sheet_index();
    let saved = jade_xlsx_reader::read_xlsx(&book.save_bytes().unwrap()).unwrap();
    saved.sheets[here]
        .rows
        .iter()
        .filter(|row| row.hidden)
        .map(|row| row.index)
        .collect()
}

#[test]
fn advanced_filter_in_place_ands_a_row_and_ors_the_rows() {
    let mut b = book();
    sales(&mut b);
    // East with more than 150, or any name starting Su.
    fill(
        &mut b,
        &[
            ("L40", "Region"),
            ("M40", "Sales"),
            ("N40", "Name"),
            ("L41", "East"),
            ("M41", ">150"),
            ("N42", "Su"),
        ],
    );
    let here = b.sheet_index();
    let job = AdvancedFilter {
        list: range("H40:J45"),
        criteria: Some((here, range("L40:N42"))),
        copy_to: None,
        unique: false,
    };
    assert_eq!(b.advanced_filter_rows(&job).unwrap(), vec![42, 43]);
    assert_eq!(b.advanced_filter(&job).unwrap(), 2);
    let hidden = hidden_rows(&mut b);
    assert!(
        [40, 41, 44].iter().all(|row| hidden.contains(row)),
        "{hidden:?}"
    );
    assert!(
        ![39, 42, 43].iter().any(|row| hidden.contains(row)),
        "{hidden:?}"
    );
    assert!(b.advanced_filtered());
    assert!(b.show_all_records().unwrap());
    assert!(hidden_rows(&mut b)
        .iter()
        .all(|row| !(40..=44).contains(row)));
}

#[test]
fn advanced_filter_reads_conditions_as_excel_does() {
    let mut b = book();
    sales(&mut b);
    let here = b.sheet_index();
    let rows = |b: &mut Workbook<TinySkiaBackend>, field: &str, condition: &str| {
        fill(b, &[("L40", field), ("L41", condition)]);
        b.advanced_filter_rows(&AdvancedFilter {
            list: range("H40:J45"),
            criteria: Some((here, range("L40:L41"))),
            copy_to: None,
            unique: false,
        })
        .unwrap()
    };
    assert_eq!(rows(&mut b, "Sales", "<=100"), vec![40, 43, 44]);
    assert_eq!(rows(&mut b, "Sales", "100"), vec![40, 44]);
    // Exactly Davis, typed as Excel has it typed: ="=Davis".
    assert_eq!(rows(&mut b, "Name", "=\"=Davis\""), vec![42]);
    assert_eq!(rows(&mut b, "Name", "Dav"), vec![40, 42, 44], "begins with");
    assert_eq!(rows(&mut b, "Name", "*an*"), vec![41], "wildcards");
    assert_eq!(rows(&mut b, "Name", "<>Davolio"), vec![41, 42, 43]);
    assert_eq!(rows(&mut b, "region", "west"), vec![41, 43], "without case");
}

#[test]
fn a_computed_criterion_is_worked_out_for_each_record() {
    let mut b = book();
    sales(&mut b);
    // Under a name that is no field's: above the average of all sales.
    fill(
        &mut b,
        &[("L40", "Above"), ("L41", "=J41>AVERAGE($J$41:$J$45)")],
    );
    let here = b.sheet_index();
    let job = AdvancedFilter {
        list: range("H40:J45"),
        criteria: Some((here, range("L40:L41"))),
        copy_to: None,
        unique: false,
    };
    assert_eq!(b.advanced_filter_rows(&job).unwrap(), vec![41, 42]);
}

#[test]
fn advanced_filter_copies_unique_records_of_the_fields_named() {
    let mut b = book();
    sales(&mut b);
    fill(&mut b, &[("L40", "Region"), ("L41", "East")]);
    let here = b.sheet_index();
    // Copy to names two fields, in its own order.
    fill(&mut b, &[("N40", "Sales"), ("O40", "Name")]);
    let job = AdvancedFilter {
        list: range("H40:J45"),
        criteria: Some((here, range("L40:L41"))),
        copy_to: Some(range("N40:O40")),
        unique: true,
    };
    assert_eq!(b.advanced_filter(&job).unwrap(), 2);
    assert_eq!(b.number(at("N41")), Some(100.0));
    assert_eq!(b.display_text(at("O41")), "Davolio");
    assert_eq!(b.display_text(at("O42")), "Davis");
    assert_eq!(b.display_text(at("O43")), "", "the repeat left out");
    assert!(
        !hidden_rows(&mut b).contains(&41),
        "nothing hidden when copying"
    );
    let extract = b
        .defined_names()
        .iter()
        .find(|name| name.name == "_xlnm.Extract")
        .unwrap()
        .formula
        .clone();
    assert!(extract.ends_with("!$N$40:$O$40"), "{extract}");
    assert!(b.undo().unwrap());
    assert_eq!(b.display_text(at("O41")), "");
}

/// The custom lists are the app's, shared by every test here.
static LISTS: Mutex<()> = Mutex::new(());

#[test]
fn a_custom_list_orders_the_sort_dialog_s_level() {
    let _lists = LISTS.lock().unwrap_or_else(|poison| poison.into_inner());
    let mut b = book();
    fill(
        &mut b,
        &[
            ("H40", "Size"),
            ("H41", "Large"),
            ("H42", "Small"),
            ("H43", "Medium"),
        ],
    );
    custom_lists::set_user_lists(vec![vec![
        "Small".to_string(),
        "Medium".to_string(),
        "Large".to_string(),
    ]]);
    let mut dialog = SortDialog::open(&b, range("H40:H43"), at("H40"));
    // My data has headers, ticked: the column is all text.
    dialog.header = true;
    let place = custom_lists::all().len() - 1;
    dialog.set_custom_list(0, Some(place));
    assert!(dialog.apply(&mut b).unwrap());
    let sizes: Vec<String> = (41..=43)
        .map(|row| b.display_text(CellRef::new(7, row - 1)))
        .collect();
    assert_eq!(sizes, ["Small", "Medium", "Large"]);
    // The built-in lists come first: days, then months.
    assert_eq!(custom_lists::all()[0][0], "Sun");
}

#[test]
fn the_custom_lists_dialog_adds_imports_and_deletes_lists() {
    let _lists = LISTS.lock().unwrap_or_else(|poison| poison.into_inner());
    let mut b = book();
    custom_lists::set_user_lists(Vec::new());
    fill(&mut b, &[("H40", "North"), ("H41", "South")]);
    let mut dialog = WhatIfDialog::open(WhatIfTool::CustomLists, &b, range("H40:H41"), at("H40"));
    dialog
        .form_mut()
        .set_text("entries", "Gold, Silver, Bronze");
    dialog.press("add", &mut b).unwrap();
    dialog.press("import_cells", &mut b).unwrap();
    assert_eq!(
        custom_lists::user_lists(),
        vec![
            vec!["Gold".to_string(), "Silver".into(), "Bronze".into()],
            vec!["North".to_string(), "South".into()],
        ]
    );
    // A built-in list cannot be deleted.
    dialog.form_mut().choose("lists", 1);
    dialog.press("delete", &mut b).unwrap();
    assert!(dialog.form().alert.is_some());
    let last = custom_lists::all().len();
    dialog.form_mut().choose("lists", last);
    dialog.press("delete", &mut b).unwrap();
    assert_eq!(custom_lists::user_lists().len(), 1);
}

#[test]
fn a_forecast_sheet_carries_the_timeline_on_with_its_bounds_and_a_chart() {
    let mut b = book();
    let mut cells = vec![
        ("H40".to_string(), "Month".to_string()),
        ("I40".into(), "Sales".into()),
    ];
    for month in 1..=12 {
        cells.push((format!("H{}", 40 + month), month.to_string()));
        cells.push((format!("I{}", 40 + month), (10 + 2 * month).to_string()));
    }
    for (cell, value) in &cells {
        b.set_cell(at(cell), value).unwrap();
    }
    let source = b.sheet_index();
    let (last, end) = b
        .forecast_defaults(range("H40:H52"), range("I40:I52"))
        .unwrap();
    assert_eq!((last, end), (12.0, 16.0), "a third of the data on");
    let job = Forecast {
        timeline: range("H40:H52"),
        values: range("I40:I52"),
        end: 15.0,
        start: None,
        confidence: Some(0.95),
        seasonality: None,
        statistics: true,
        zeros: false,
        aggregation: 1,
        column_chart: false,
    };
    let sheet = b.forecast_sheet(&job, &ForecastWords::default()).unwrap();
    assert_eq!(b.sheet_index(), sheet);
    assert_eq!(sheet, source + 1, "after the data's sheet");
    assert_eq!(b.display_text(at("A1")), "Month");
    assert_eq!(b.display_text(at("C1")), "Forecast(Sales)");
    assert_eq!(b.display_text(at("E1")), "Upper Confidence Bound(Sales)");
    // The last point starts the forecast's lines, then the formulas.
    assert_eq!(b.number(at("C13")), Some(34.0));
    assert_eq!(b.number(at("D13")), Some(34.0));
    assert_eq!(b.number(at("A16")), Some(15.0));
    assert_eq!(
        b.edit_text(at("C14")),
        "=FORECAST.ETS(A14,$B$2:$B$13,$A$2:$A$13,1,1,1)"
    );
    let forecast = b.number(at("C14")).unwrap();
    assert!((forecast - 36.0).abs() < 1.0, "{forecast}");
    let lower = b.number(at("D14")).unwrap();
    let upper = b.number(at("E14")).unwrap();
    assert!(lower <= forecast && forecast <= upper, "{lower} {upper}");
    assert_eq!(b.display_text(at("A17")), "", "no more than asked for");
    assert_eq!(b.display_text(at("G2")), "Alpha");
    assert!(b.edit_text(at("H2")).starts_with("=FORECAST.ETS.STAT("));
    let saved = jade_xlsx_reader::read_xlsx(&b.save_bytes().unwrap()).unwrap();
    let chart = saved.sheets[sheet]
        .shapes
        .iter()
        .find_map(|shape| shape.chart.as_ref())
        .expect("a chart");
    assert_eq!(chart.series.len(), 4);
    // The whole sheet, chart and all, is one undo step.
    assert!(b.undo().unwrap());
    assert_eq!(b.sheet_names().len(), saved.sheets.len() - 1);
}

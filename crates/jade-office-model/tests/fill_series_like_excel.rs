//! Home ▸ Fill ▸ Series… as Excel's dialog: linear, growth and date series
//! down or across the selection, a stop value, a trend fitted to the values
//! already there, AutoFill, and one undo step for the lot.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::fill_series::{
    DateUnit, SeriesDialog, SeriesIn, SeriesKey, SeriesOptions, SeriesType,
};
use jade_office_model::sheet::{CellRef, Workbook};
use jade_office_model::tiny::TinySkiaBackend;
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

/// Column J from row 40 down, out of the fixture's way.
fn j(row: u32) -> CellRef {
    CellRef::new(9, row - 1)
}

fn down(from: u32, to: u32) -> CellRange {
    CellRange {
        start: j(from),
        end: j(to),
    }
}

fn options(kind: SeriesType, step: f64, stop: Option<f64>) -> SeriesOptions {
    SeriesOptions {
        series_in: SeriesIn::Columns,
        kind,
        unit: DateUnit::Day,
        step,
        stop,
        trend: false,
    }
}

fn numbers(book: &Workbook<TinySkiaBackend>, from: u32, to: u32) -> Vec<Option<f64>> {
    (from..=to).map(|row| book.number(j(row))).collect()
}

#[test]
fn a_linear_series_steps_down_the_selection_and_stops_at_the_stop_value() {
    let mut book = book();
    book.set_cell(j(40), "1").unwrap();
    let series = options(SeriesType::Linear, 2.0, Some(11.0));
    assert!(book.fill_series_dialog(down(40, 49), &series).unwrap());
    assert_eq!(
        numbers(&book, 40, 47),
        [1.0, 3.0, 5.0, 7.0, 9.0, 11.0]
            .map(Some)
            .into_iter()
            .chain([None, None])
            .collect::<Vec<_>>()
    );
    // One undo step takes the whole series back.
    assert!(book.undo().unwrap());
    assert_eq!(numbers(&book, 41, 45), vec![None; 5]);
    assert_eq!(book.number(j(40)), Some(1.0));
}

#[test]
fn one_cell_selected_runs_on_until_the_stop_value() {
    let mut book = book();
    book.set_cell(j(40), "10").unwrap();
    let mut across = options(SeriesType::Linear, 10.0, Some(50.0));
    across.series_in = SeriesIn::Rows;
    let one = CellRange {
        start: j(40),
        end: j(40),
    };
    assert!(book.fill_series_dialog(one, &across).unwrap());
    let row: Vec<_> = (9..=14)
        .map(|column| book.number(CellRef::new(column, 39)))
        .collect();
    assert_eq!(
        row,
        vec![
            Some(10.0),
            Some(20.0),
            Some(30.0),
            Some(40.0),
            Some(50.0),
            None
        ]
    );
    // With no stop value there is nowhere to run to, as in Excel.
    book.set_cell(j(50), "1").unwrap();
    let alone = CellRange {
        start: j(50),
        end: j(50),
    };
    let unbounded = options(SeriesType::Linear, 1.0, None);
    assert!(!book.fill_series_dialog(alone, &unbounded).unwrap());
}

#[test]
fn growth_multiplies_and_keeps_fifteen_digits() {
    let mut book = book();
    book.set_cell(j(40), "2").unwrap();
    let growth = options(SeriesType::Growth, 3.0, None);
    assert!(book.fill_series_dialog(down(40, 43), &growth).unwrap());
    assert_eq!(
        numbers(&book, 40, 43),
        vec![Some(2.0), Some(6.0), Some(18.0), Some(54.0)]
    );
    book.set_cell(j(50), "0.1").unwrap();
    let tenths = options(SeriesType::Linear, 0.1, None);
    assert!(book.fill_series_dialog(down(50, 52), &tenths).unwrap());
    assert_eq!(book.number(j(52)), Some(0.3));
}

#[test]
fn dates_step_by_months_to_the_last_day_and_keep_their_format() {
    let mut book = book();
    // 2024-01-31.
    book.set_cell(j(40), "2024/1/31").unwrap();
    let start = book.number(j(40)).unwrap();
    assert_eq!(start, 45322.0);
    // The dialog starts on a date series for a date.
    assert_eq!(
        SeriesOptions::for_range(&book, down(40, 43)).kind,
        SeriesType::Date
    );
    let mut months = options(SeriesType::Date, 1.0, None);
    months.unit = DateUnit::Month;
    assert!(book.fill_series_dialog(down(40, 43), &months).unwrap());
    // 2024-02-29, 2024-03-31, 2024-04-30.
    assert_eq!(
        numbers(&book, 41, 43),
        vec![Some(45351.0), Some(45382.0), Some(45412.0)]
    );
    assert_eq!(
        book.format_at(j(43)).number_format,
        book.format_at(j(40)).number_format
    );

    // Weekdays skip the weekend: Friday 2024-01-05, then Monday the 8th.
    book.set_cell(j(50), "2024/1/5").unwrap();
    let mut weekdays = options(SeriesType::Date, 1.0, None);
    weekdays.unit = DateUnit::Weekday;
    assert!(book.fill_series_dialog(down(50, 51), &weekdays).unwrap());
    assert_eq!(book.number(j(51)), Some(45299.0));
}

#[test]
fn a_trend_replaces_the_values_with_their_best_fit_line() {
    let mut book = book();
    for (row, value) in [(40, "1"), (41, "3"), (42, "4")] {
        book.set_cell(j(row), value).unwrap();
    }
    let mut trend = options(SeriesType::Linear, 1.0, None);
    trend.trend = true;
    assert!(book.fill_series_dialog(down(40, 44), &trend).unwrap());
    // Through (0, 1), (1, 3), (2, 4): slope 1.5 from 7/6.
    let fitted: Vec<f64> = numbers(&book, 40, 44)
        .into_iter()
        .map(Option::unwrap)
        .collect();
    for (value, expected) in
        fitted
            .iter()
            .zip([7.0 / 6.0, 8.0 / 3.0, 25.0 / 6.0, 17.0 / 3.0, 43.0 / 6.0])
    {
        assert!((value - expected).abs() < 1e-9, "{fitted:?}");
    }
}

#[test]
fn autofill_extends_the_cells_at_the_start_as_the_fill_handle_does() {
    let mut book = book();
    book.set_cell(j(40), "1").unwrap();
    book.set_cell(j(41), "2").unwrap();
    let autofill = options(SeriesType::AutoFill, 1.0, None);
    assert!(book.fill_series_dialog(down(40, 44), &autofill).unwrap());
    assert_eq!(
        numbers(&book, 42, 44),
        vec![Some(3.0), Some(4.0), Some(5.0)]
    );
}

#[test]
fn the_dialog_refuses_a_step_that_is_not_a_number() {
    let mut book = book();
    book.set_cell(j(40), "5").unwrap();
    let mut dialog = SeriesDialog::open(&book, down(40, 42));
    // Taller than wide: down the columns, linear by 1.
    assert_eq!(dialog.series_in, SeriesIn::Columns);
    assert_eq!(dialog.kind, SeriesType::Linear);
    // Typed text reaches the box with the keys through the input handler.
    dialog.field_mut().insert("x");
    assert!(dialog.apply(&mut book).is_none());
    assert!(dialog.problem);
    assert_eq!(book.number(j(41)), None);
    // Editing again clears the complaint; Tab moves to the stop value.
    assert_eq!(dialog.key("backspace", false), SeriesKey::Handled);
    assert!(!dialog.problem);
    dialog.field_mut().insert("2");
    assert_eq!(dialog.key("tab", false), SeriesKey::Handled);
    dialog.field_mut().insert("8");
    assert_eq!(dialog.key("enter", false), SeriesKey::Take);
    assert!(dialog.apply(&mut book).unwrap().unwrap());
    assert_eq!(numbers(&book, 40, 42), vec![Some(5.0), Some(7.0), None]);
}

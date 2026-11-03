//! Data ▸ Data Analysis as Excel's Analysis ToolPak: a tool picked from the
//! list, its input range read by columns or rows with or without labels,
//! and its table written at an output range or on a sheet of its own, in
//! one undo step.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::analysis::Tool;
use jade_office_model::sheet::{span, CellRef, Workbook};
use jade_office_model::tiny::TinySkiaBackend;
use jade_office_model::what_if::{WhatIfDialog, WhatIfTool};

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

fn at(column: u32, row: u32) -> CellRef {
    CellRef::new(column, row)
}

/// The English words, from the shell's table.
fn english(key: &'static str) -> String {
    let table = include_str!("../../jade-gpui-app/i18n/en.tsv");
    table
        .lines()
        .find_map(|line| {
            line.strip_prefix(key)?
                .strip_prefix('\t')
                .map(str::to_string)
        })
        .unwrap_or_else(|| key.to_string())
}

/// Three groups with a label row in H30:J33.
fn groups(book: &mut Workbook<TinySkiaBackend>) {
    for (column, (label, values)) in [("A", [4, 5, 6]), ("B", [6, 7, 8]), ("C", [9, 10, 11])]
        .iter()
        .enumerate()
    {
        let column = 7 + column as u32;
        book.set_cell(at(column, 29), label).unwrap();
        for (row, value) in values.iter().enumerate() {
            book.set_cell(at(column, 30 + row as u32), &value.to_string())
                .unwrap();
        }
    }
}

fn dialog_for(
    book: &mut Workbook<TinySkiaBackend>,
    tool: Tool,
    range: (CellRef, CellRef),
) -> WhatIfDialog {
    let mut dialog = WhatIfDialog::open(
        WhatIfTool::DataAnalysis,
        book,
        span(range.0, range.1),
        range.0,
    );
    if let WhatIfDialog::DataAnalysis(analysis) = &mut dialog {
        analysis.set_word_source(Box::new(english));
    }
    let index = Tool::ALL.iter().position(|t| *t == tool).unwrap();
    dialog.click(0, index, book).unwrap();
    dialog
}

#[test]
fn anova_single_factor_goes_on_a_sheet_of_its_own() {
    let mut book = book();
    groups(&mut book);
    let mut dialog = dialog_for(&mut book, Tool::AnovaSingle, (at(7, 29), at(9, 32)));
    dialog.press("ok", &mut book).unwrap();
    assert_eq!(dialog.form().text("input"), "$H$30:$J$33");
    let labels = dialog
        .form()
        .items
        .iter()
        .position(|i| i.id == "labels")
        .unwrap();
    dialog.click(labels, 0, &mut book).unwrap();
    dialog.form_mut().set_text("sheet_name", "Anova");
    let sheets = book.sheet_names().len();
    let outcome = dialog.press("ok", &mut book).unwrap();
    assert!(outcome.close && outcome.wrote, "{:?}", dialog.form().alert);
    assert_eq!(book.sheet_names().len(), sheets + 1);
    assert_eq!(book.sheet_name(), "Anova");
    assert_eq!(book.display_text(at(0, 0)), "Anova: Single Factor");
    assert_eq!(book.display_text(at(0, 4)), "A");
    assert_eq!(book.display_text(at(0, 11)), "Between Groups");
    assert_eq!(book.display_text(at(1, 11)), "38");
    assert_eq!(book.display_text(at(4, 11)), "19");
    // One undo takes the sheet away.
    assert!(book.undo().unwrap());
    assert_eq!(book.sheet_names().len(), sheets);
}

#[test]
fn descriptive_statistics_go_at_an_output_range() {
    let mut book = book();
    groups(&mut book);
    let mut dialog = dialog_for(&mut book, Tool::Descriptive, (at(7, 29), at(7, 32)));
    dialog.press("ok", &mut book).unwrap();
    let labels = dialog
        .form()
        .items
        .iter()
        .position(|i| i.id == "labels")
        .unwrap();
    dialog.click(labels, 0, &mut book).unwrap();
    let output = dialog
        .form()
        .items
        .iter()
        .position(|i| i.id == "output")
        .unwrap();
    dialog.click(output, 0, &mut book).unwrap();
    dialog.form_mut().set_text("output_range", "L30");
    assert!(
        dialog.press("ok", &mut book).unwrap().close,
        "{:?}",
        dialog.form().alert
    );
    assert_eq!(book.display_text(at(11, 29)), "A");
    assert_eq!(book.display_text(at(11, 31)), "Mean");
    assert_eq!(book.display_text(at(12, 31)), "5");
    assert_eq!(book.display_text(at(11, 43)), "Count");
    assert_eq!(book.display_text(at(12, 43)), "3");
}

#[test]
fn a_moving_average_writes_formulas_and_text_in_the_input_is_refused() {
    let mut book = book();
    groups(&mut book);
    let mut dialog = dialog_for(&mut book, Tool::MovingAverage, (at(9, 30), at(9, 32)));
    dialog.press("ok", &mut book).unwrap();
    dialog.form_mut().set_text("interval", "2");
    dialog.form_mut().set_text("output_range", "L30");
    assert!(
        dialog.press("ok", &mut book).unwrap().close,
        "{:?}",
        dialog.form().alert
    );
    assert_eq!(book.display_text(at(11, 29)), "#N/A");
    assert_eq!(book.edit_text(at(11, 30)), "=AVERAGE(J31:J32)");
    assert_eq!(book.display_text(at(11, 31)), "10.5");
    // A label in the numbers is refused.
    let mut dialog = dialog_for(&mut book, Tool::Descriptive, (at(7, 29), at(7, 32)));
    dialog.press("ok", &mut book).unwrap();
    assert!(!dialog.press("ok", &mut book).unwrap().close);
    assert!(dialog.form().alert.is_some());
}

#[test]
fn a_t_test_and_a_regression_report_excel_s_numbers() {
    let mut book = book();
    for (row, (x, y)) in [(1.0, 3.0), (2.0, 5.0), (3.0, 7.1), (4.0, 8.9), (5.0, 11.0)]
        .iter()
        .enumerate()
    {
        book.set_cell(at(7, 29 + row as u32), &x.to_string())
            .unwrap();
        book.set_cell(at(8, 29 + row as u32), &y.to_string())
            .unwrap();
    }
    let mut dialog = dialog_for(&mut book, Tool::Regression, (at(8, 29), at(8, 33)));
    dialog.press("ok", &mut book).unwrap();
    dialog.form_mut().set_text("x_range", "H30:H34");
    assert!(
        dialog.press("ok", &mut book).unwrap().close,
        "{:?}",
        dialog.form().alert
    );
    assert_eq!(book.display_text(at(0, 0)), "SUMMARY OUTPUT");
    assert_eq!(book.display_text(at(0, 17)), "X Variable");
    assert_eq!(book.display_text(at(1, 17)), "1.99");
    book.undo().unwrap();
    let mut dialog = dialog_for(&mut book, Tool::TPaired, (at(7, 29), at(7, 33)));
    dialog.press("ok", &mut book).unwrap();
    dialog.form_mut().set_text("variable2", "I30:I34");
    assert!(
        dialog.press("ok", &mut book).unwrap().close,
        "{:?}",
        dialog.form().alert
    );
    assert_eq!(
        book.display_text(at(0, 0)),
        "t-Test: Paired Two Sample for Means"
    );
    assert_eq!(book.display_text(at(0, 8)), "df");
    assert_eq!(book.display_text(at(1, 8)), "4");
}

#[test]
fn random_numbers_repeat_for_a_seed() {
    let mut book = book();
    let run = |book: &mut Workbook<TinySkiaBackend>| {
        let mut dialog = dialog_for(book, Tool::Random, (at(0, 0), at(0, 0)));
        dialog.press("ok", book).unwrap();
        dialog.form_mut().set_text("variables", "2");
        dialog.form_mut().set_text("count", "5");
        dialog.form_mut().set_text("seed", "42");
        assert!(
            dialog.press("ok", book).unwrap().close,
            "{:?}",
            dialog.form().alert
        );
        (0..5)
            .map(|row| book.display_text(at(1, row)))
            .collect::<Vec<_>>()
    };
    let first = run(&mut book);
    let second = run(&mut book);
    assert_eq!(first, second);
    assert!(first.iter().all(|text| !text.is_empty()));
}

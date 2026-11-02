//! Data ▸ What-If Analysis ▸ Scenario Manager as Excel's: named sets of
//! values for changing cells, added, edited, shown, deleted and merged; a
//! summary and a PivotTable report of what each makes of the result cells;
//! all saved in the sheet's `<scenarios>` and read back.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::scenarios::{cells_text, parse_cells, SummaryWords};
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

/// H30 a price, H31 a quantity, H32 their product.
fn sales(book: &mut Workbook<TinySkiaBackend>) {
    book.set_cell(at(7, 29), "10").unwrap();
    book.set_cell(at(7, 30), "100").unwrap();
    book.set_cell(at(7, 31), "=H30*H31").unwrap();
}

/// Adds a scenario through the dialog: Add, name and cells, OK, values, OK.
fn add(
    dialog: &mut WhatIfDialog,
    book: &mut Workbook<TinySkiaBackend>,
    name: &str,
    values: &[&str],
) {
    dialog.press("add", book).unwrap();
    dialog.form_mut().set_text("name", name);
    dialog.form_mut().set_text("cells", "H30:H31");
    dialog.press("ok", book).unwrap();
    assert!(dialog.form().alert.is_none(), "{:?}", dialog.form().alert);
    for (line, value) in values.iter().enumerate() {
        dialog.form_mut().set_text(["v1", "v2"][line], value);
    }
    let outcome = dialog.press("ok", book).unwrap();
    assert!(outcome.wrote && !outcome.close);
}

#[test]
fn scenarios_are_added_shown_and_each_undone_in_a_step() {
    let mut book = book();
    sales(&mut book);
    let mut dialog = WhatIfDialog::open(
        WhatIfTool::ScenarioManager,
        &book,
        span(at(7, 29), at(7, 30)),
        at(7, 29),
    );
    add(&mut dialog, &mut book, "Best", &["12", "150"]);
    add(&mut dialog, &mut book, "Worst", &["8", "60"]);
    let names: Vec<String> = book
        .scenarios()
        .scenarios
        .iter()
        .map(|s| s.name.clone())
        .collect();
    assert_eq!(names, ["Best", "Worst"]);
    // Adding leaves the cells as they were.
    assert_eq!(book.display_text(at(7, 31)), "1000");
    // Show puts the picked scenario's values in.
    dialog.click(0, 0, &mut book).unwrap();
    assert!(dialog.press("show", &mut book).unwrap().wrote);
    assert_eq!(book.display_text(at(7, 31)), "1800");
    assert_eq!(book.scenarios().show, Some(0));
    assert!(dialog.press("close", &mut book).unwrap().close);
    // One undo takes the shown values back; one more the second scenario.
    assert!(book.undo().unwrap());
    assert_eq!(book.display_text(at(7, 31)), "1000");
    assert!(book.undo().unwrap());
    assert_eq!(book.scenarios().scenarios.len(), 1);
}

#[test]
fn a_name_is_wanted_once_and_changing_cells_must_be_on_the_sheet() {
    let mut book = book();
    sales(&mut book);
    let mut dialog = WhatIfDialog::open(
        WhatIfTool::ScenarioManager,
        &book,
        span(at(7, 29), at(7, 30)),
        at(7, 29),
    );
    add(&mut dialog, &mut book, "Best", &["12", "150"]);
    dialog.press("add", &mut book).unwrap();
    dialog.form_mut().set_text("name", "best");
    dialog.press("ok", &mut book).unwrap();
    assert!(dialog.form().alert.is_some(), "a name taken");
    dialog.form_mut().set_text("name", "Other");
    dialog.form_mut().set_text("cells", "nowhere");
    dialog.press("ok", &mut book).unwrap();
    assert!(dialog.form().alert.is_some(), "no such cells");
    assert_eq!(parse_cells(&book, "H30:H31,J5").unwrap().len(), 3);
    assert_eq!(
        cells_text(&parse_cells(&book, "H30:H31,J5").unwrap()),
        "$H$30:$H$31,$J$5"
    );
}

#[test]
fn editing_keeps_the_values_and_delete_takes_a_scenario_out() {
    let mut book = book();
    sales(&mut book);
    let mut dialog = WhatIfDialog::open(
        WhatIfTool::ScenarioManager,
        &book,
        span(at(7, 29), at(7, 30)),
        at(7, 29),
    );
    add(&mut dialog, &mut book, "Best", &["12", "150"]);
    dialog.click(0, 0, &mut book).unwrap();
    dialog.press("edit", &mut book).unwrap();
    assert_eq!(dialog.form().text("name"), "Best");
    dialog.form_mut().set_text("name", "Top");
    dialog.form_mut().set_text("comment", "Busy month");
    dialog.press("ok", &mut book).unwrap();
    assert_eq!(dialog.form().text("v1"), "12");
    dialog.press("ok", &mut book).unwrap();
    let scenario = &book.scenarios().scenarios[0];
    assert_eq!(scenario.name, "Top");
    assert_eq!(scenario.comment.as_deref(), Some("Busy month"));
    dialog.click(0, 0, &mut book).unwrap();
    dialog.press("delete", &mut book).unwrap();
    assert!(book.scenarios().scenarios.is_empty());
}

#[test]
fn a_summary_compares_each_scenario_on_a_sheet_of_its_own() {
    let mut book = book();
    sales(&mut book);
    let mut dialog = WhatIfDialog::open(
        WhatIfTool::ScenarioManager,
        &book,
        span(at(7, 29), at(7, 30)),
        at(7, 29),
    );
    add(&mut dialog, &mut book, "Best", &["12", "150"]);
    add(&mut dialog, &mut book, "Worst", &["8", "60"]);
    let sheet = book.sheet_index();
    let sheets = book.sheet_names().len();
    dialog.press("summary", &mut book).unwrap();
    dialog.form_mut().set_text("results", "H32");
    let outcome = dialog.press("ok", &mut book).unwrap();
    assert!(outcome.close && outcome.wrote);
    assert_eq!(book.sheet_names().len(), sheets + 1);
    assert_eq!(book.sheet_name(), "Scenario Summary");
    assert_eq!(book.display_text(at(1, 1)), "Scenario Summary");
    assert_eq!(book.display_text(at(4, 2)), "Best");
    assert_eq!(book.display_text(at(5, 2)), "Worst");
    assert_eq!(book.display_text(at(2, 5)), "$H$30");
    assert_eq!(book.display_text(at(4, 5)), "12");
    // The result row: current, Best, Worst.
    let results: Vec<String> = (3..=5)
        .map(|column| book.display_text(at(column, 8)))
        .collect();
    assert_eq!(results, ["1000", "1800", "480"]);
    // The sheet the scenarios are on is left as it was.
    book.select_sheet(sheet).unwrap();
    assert_eq!(book.display_text(at(7, 31)), "1000");
    assert_eq!(book.scenarios().result_cells.len(), 1);
    // The PivotTable report: a row per scenario.
    book.scenario_pivot(&[at(7, 31)], &SummaryWords::english())
        .unwrap();
    assert_eq!(book.display_text(at(0, 3)), "Best");
    assert_eq!(book.display_text(at(1, 4)), "480");
}

#[test]
fn scenarios_merge_from_another_sheet_and_are_saved_as_excel_saves_them() {
    let mut book = book();
    sales(&mut book);
    let mut dialog = WhatIfDialog::open(
        WhatIfTool::ScenarioManager,
        &book,
        span(at(7, 29), at(7, 30)),
        at(7, 29),
    );
    add(&mut dialog, &mut book, "Best & <top>", &["12", "150"]);
    let first = book.sheet_index();
    // A new sheet, selected, takes the first one's scenarios.
    book.add_sheet().unwrap();
    assert_ne!(book.sheet_index(), first);
    assert!(book.merge_scenarios(first).unwrap());
    assert!(book.merge_scenarios(first).unwrap());
    let names: Vec<String> = book
        .scenarios()
        .scenarios
        .iter()
        .map(|s| s.name.clone())
        .collect();
    assert_eq!(names, ["Best & <top>", "Best & <top> 2"]);
    let bytes = book.save_bytes().unwrap();
    let package = jade_opc::OpcPackage::open(&bytes).unwrap();
    let saved = (1..=8)
        .filter_map(|n| {
            package
                .part_str(&format!("xl/worksheets/sheet{n}.xml"))
                .ok()
                .flatten()
        })
        .find(|xml| xml.contains("<scenarios"))
        .expect("a sheet with scenarios");
    assert!(
        saved.contains(r#"<scenario name="Best &amp; &lt;top&gt;" locked="1" count="2">"#),
        "{saved}"
    );
    assert!(saved.contains(r#"<inputCells r="H30" val="12"/>"#));
    let mut again = Workbook::open(&bytes, "UTC", backend()).unwrap();
    again.select_sheet(first).unwrap();
    assert_eq!(again.scenarios(), {
        book.select_sheet(first).unwrap();
        book.scenarios()
    });
}

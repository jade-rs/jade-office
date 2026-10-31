//! Find and Replace as Excel's dialog does them: Match case, Match entire
//! cell contents, Within Sheet or Workbook; Replace on the active cell,
//! Replace All in one undo, formulas replaced in their source and values
//! read again as typed.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::find::{replaced, FindBar, FindOptions, Found};
use jade_office_model::sheet::{protected_refusal, span, CellRef, Workbook};
use jade_office_model::tiny::TinySkiaBackend;

fn open(name: &str) -> Workbook<TinySkiaBackend> {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let directory = core.join("fonts/Noto_Sans_JP/static");
    let backend = TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        None,
    ))
    .unwrap();
    let bytes = std::fs::read(core.join("public/fixtures").join(name)).unwrap();
    Workbook::open(&bytes, "UTC", backend).unwrap()
}

fn at(column: u32, row: u32) -> CellRef {
    CellRef::new(column, row)
}

/// A new sheet with Apple, apple pie, APPLE, 100 and a formula doubling
/// it.
fn fruit() -> Workbook<TinySkiaBackend> {
    let mut book = open("grid-operations-workbook.xlsx");
    let sheet = book.add_sheet().unwrap();
    book.select_sheet(sheet).unwrap();
    book.set_cell(at(10, 0), "Apple").unwrap();
    book.set_cell(at(10, 1), "apple pie").unwrap();
    book.set_cell(at(10, 2), "APPLE").unwrap();
    book.set_cell(at(10, 3), "100").unwrap();
    book.set_cell(at(10, 4), "=K4*2").unwrap();
    book
}

#[test]
fn match_case_and_entire_cell_narrow_what_find_stops_at() {
    let book = fruit();
    let sheet = book.sheet_index();
    let cells = |options| {
        book.find_all("apple", options)
            .into_iter()
            .map(|(_, cell)| cell)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        cells(FindOptions::default()),
        vec![at(10, 0), at(10, 1), at(10, 2)]
    );
    let case = FindOptions {
        match_case: true,
        ..FindOptions::default()
    };
    assert_eq!(cells(case), vec![at(10, 1)]);
    let entire = FindOptions {
        entire_cell: true,
        ..FindOptions::default()
    };
    assert_eq!(cells(entire), vec![at(10, 0), at(10, 2)]);
    let both = FindOptions {
        match_case: true,
        entire_cell: true,
        ..FindOptions::default()
    };
    assert!(cells(both).is_empty());
    // 10 is in 100 but is not the whole of it.
    assert!(book
        .find_all("10", entire)
        .iter()
        .all(|(_, cell)| *cell != at(10, 3)));
    // Find Next goes round, Find Previous back.
    let first = book
        .find_next("apple", FindOptions::default(), (sheet, at(0, 0)), false)
        .unwrap();
    assert_eq!(first, (sheet, at(10, 0)));
    assert_eq!(
        book.find_next("apple", FindOptions::default(), first, true),
        Some((sheet, at(10, 2)))
    );
}

#[test]
fn within_sheet_stays_on_the_sheet_and_workbook_goes_on() {
    let mut book = fruit();
    let first = book.sheet_index();
    let other = book.add_sheet().unwrap();
    book.select_sheet(other).unwrap();
    book.set_cell(at(0, 0), "apple tart").unwrap();
    book.select_sheet(first).unwrap();
    assert_eq!(book.find_all("tart", FindOptions::default()), vec![]);
    let workbook = FindOptions {
        workbook: true,
        ..FindOptions::default()
    };
    assert_eq!(book.find_all("tart", workbook), vec![(other, at(0, 0))]);
    // Replace All within the workbook writes the other sheet too, and
    // leaves this one showing.
    assert_eq!(book.replace_all("apple", "pear", workbook).unwrap(), 4);
    assert_eq!(book.sheet_index(), first);
    assert_eq!(book.display_text(at(10, 1)), "pear pie");
    book.select_sheet(other).unwrap();
    assert_eq!(book.display_text(at(0, 0)), "pear tart");
}

#[test]
fn replace_changes_the_active_cell_only_when_it_matches() {
    let mut book = fruit();
    let options = FindOptions::default();
    assert!(!book
        .replace_at(at(10, 3), "apple", "pear", options)
        .unwrap());
    assert!(book
        .replace_at(at(10, 1), "apple", "pear", options)
        .unwrap());
    assert_eq!(book.display_text(at(10, 1)), "pear pie");
    assert_eq!(book.display_text(at(10, 0)), "Apple", "the others wait");
    book.undo().unwrap();
    assert_eq!(book.display_text(at(10, 1)), "apple pie");
}

#[test]
fn replace_all_is_one_undo_and_values_read_again_as_typed() {
    let mut book = fruit();
    let options = FindOptions::default();
    assert_eq!(book.replace_all("apple", "pear", options).unwrap(), 3);
    assert_eq!(book.display_text(at(10, 0)), "pear");
    assert_eq!(book.display_text(at(10, 2)), "pear");
    // A number replaced in is a number: 100 → 200, and its dependant
    // follows.
    assert_eq!(book.replace_all("1", "2", options).unwrap(), 1);
    assert_eq!(book.number(at(10, 3)), Some(200.0));
    assert_eq!(book.display_text(at(10, 4)), "400");
    book.undo().unwrap();
    assert_eq!(book.number(at(10, 3)), Some(100.0));
    book.undo().unwrap();
    assert_eq!(
        book.display_text(at(10, 0)),
        "Apple",
        "all three in one undo"
    );
    assert_eq!(book.display_text(at(10, 2)), "APPLE");
    assert_eq!(book.replace_all("nowhere", "x", options).unwrap(), 0);
}

#[test]
fn a_formula_is_replaced_in_its_source() {
    let mut book = fruit();
    book.set_cell(at(10, 5), "=SUM(K4:K5)").unwrap();
    let options = FindOptions::default();
    assert_eq!(book.replace_all("SUM", "MAX", options).unwrap(), 1);
    assert_eq!(book.edit_text(at(10, 5)), "=MAX(K4:K5)");
    assert_eq!(book.display_text(at(10, 5)), "200");
    // A replacement that would break the formula leaves it be.
    assert_eq!(book.replace_all(")", "))", options).unwrap(), 0);
    assert_eq!(book.edit_text(at(10, 5)), "=MAX(K4:K5)");
}

#[test]
fn replace_looks_in_formulas_where_find_also_reads_what_cells_show() {
    let mut book = fruit();
    // K5 holds =K4*2 and shows 200.
    let shown = book.find_all("200", FindOptions::default());
    assert_eq!(
        shown.iter().map(|(_, cell)| *cell).collect::<Vec<_>>(),
        vec![at(10, 4)]
    );
    let formulas = FindOptions {
        formulas: true,
        ..FindOptions::default()
    };
    assert!(book.find_all("200", formulas).is_empty());
    assert!(!book
        .replace_at(at(10, 4), "200", "300", FindOptions::default())
        .unwrap());
    assert_eq!(
        book.replace_all("200", "300", FindOptions::default())
            .unwrap(),
        0
    );
    // The bar looks in formulas while its Replace field shows.
    let mut bar = FindBar::new(false);
    assert!(!bar.options().formulas);
    bar.toggle_replacing();
    assert!(bar.options().formulas);
}

#[test]
fn a_protected_sheet_refuses_replace_all() {
    let mut book = open("protected-sheets-workbook.xlsx");
    let error = book
        .replace_all("10", "20", FindOptions::default())
        .unwrap_err();
    assert!(protected_refusal(&error), "{error}");
}

#[test]
fn the_text_replace_follows_the_options() {
    let options = FindOptions::default();
    assert_eq!(
        replaced("Apple apple", "apple", "pear", options).as_deref(),
        Some("pear pear")
    );
    let case = FindOptions {
        match_case: true,
        ..options
    };
    assert_eq!(
        replaced("Apple apple", "apple", "pear", case).as_deref(),
        Some("Apple pear")
    );
    let entire = FindOptions {
        entire_cell: true,
        ..options
    };
    assert_eq!(replaced("Apple apple", "apple", "pear", entire), None);
    assert_eq!(
        replaced("APPLE", "apple", "pear", entire).as_deref(),
        Some("pear")
    );
    assert_eq!(
        replaced("りんごとりんご", "りんご", "梨", options).as_deref(),
        Some("梨と梨")
    );
    assert_eq!(replaced("abc", "", "x", options), None);
}

#[test]
fn the_bar_types_into_find_or_replace() {
    let mut bar = FindBar::new(false);
    bar.field_mut().insert("apple");
    bar.next_field();
    assert_eq!(bar.field().text, "apple", "no Replace field to go to");
    bar.toggle_replacing();
    bar.field_mut().insert("pear");
    assert_eq!(bar.find.text, "apple");
    assert_eq!(bar.replace.text, "pear");
    bar.next_field();
    assert_eq!(bar.field().text, "apple");
    let replacing = FindBar::new(true);
    assert!(replacing.replacing && !replacing.in_replace);
}

/// Find All lists every match with its sheet, cell, what it shows and its
/// formula, as Excel's list does.
#[test]
fn find_all_lists_the_sheet_cell_value_and_formula() {
    let book = fruit();
    let sheet = book.sheet_index();
    let name = book.sheet_names()[sheet].clone();
    let found = book.found("200", FindOptions::default());
    assert_eq!(
        found,
        vec![Found {
            sheet,
            sheet_name: name.clone(),
            cell: at(10, 4),
            value: "200".into(),
            formula: "=K4*2".into(),
        }]
    );
    let found = book.found("apple", FindOptions::default());
    assert_eq!(found.len(), 3);
    assert_eq!(found[1].value, "apple pie");
    assert_eq!(found[1].formula, "");
    assert_eq!(found[1].sheet_name, name);
}

/// With more than one cell selected Find and Replace All look only there;
/// one cell selected, the whole sheet.
#[test]
fn a_selection_of_several_cells_is_where_find_and_replace_look() {
    let mut book = fruit();
    let mut bar = FindBar::new(false);
    bar.find.text = "apple".into();
    let options = bar.options_in(span(at(10, 1), at(10, 3)));
    let cells: Vec<CellRef> = book
        .find_all("apple", options)
        .into_iter()
        .map(|(_, cell)| cell)
        .collect();
    assert_eq!(cells, vec![at(10, 1), at(10, 2)]);
    let whole = bar.options_in(span(at(0, 0), at(0, 0)));
    assert_eq!(book.find_all("apple", whole).len(), 3);
    // Within: Workbook ignores the selection.
    bar.options.workbook = true;
    assert_eq!(bar.options_in(span(at(10, 1), at(10, 3))).within, None);
    bar.options.workbook = false;
    assert_eq!(book.replace_all("apple", "pear", options).unwrap(), 2);
    assert_eq!(book.display_text(at(10, 0)), "Apple");
    assert_eq!(book.display_text(at(10, 1)), "pear pie");
}

/// Look in: Values finds by what a cell shows: 200, not the formula's K4.
#[test]
fn look_in_values_finds_by_what_cells_show() {
    let book = fruit();
    let mut bar = FindBar::new(false);
    bar.options.values = true;
    let options = bar.options();
    let cells = |needle| {
        book.find_all(needle, options)
            .into_iter()
            .map(|(_, cell)| cell)
            .collect::<Vec<_>>()
    };
    assert_eq!(cells("200"), vec![at(10, 4)]);
    assert!(cells("K4").is_empty());
    let formulas = FindBar::new(false).options();
    assert_eq!(book.find_all("K4", formulas).len(), 1);
    // The Replace tab looks in formulas whatever Look in says.
    bar.replacing = true;
    assert!(!bar.options().values);
}

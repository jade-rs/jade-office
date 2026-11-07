//! Entering formulas as Excel takes them: an entry that starts with `+` or
//! `-` and is not a number becomes a formula (`-A1` is `=-A1`), and Formula
//! AutoComplete lists the workbook's defined names and tables with the
//! functions.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::formula_complete;
use jade_office_model::sheet::{CellRange, CellRef, Workbook};
use jade_office_model::text::TextEdit;
use jade_office_model::tiny::TinySkiaBackend;

fn core() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"))
}

fn open(bytes: &[u8]) -> Workbook<TinySkiaBackend> {
    let directory = core().join("fonts/Noto_Sans_JP/static");
    let backend = TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        None,
    ))
    .unwrap();
    Workbook::open(bytes, "UTC", backend).unwrap()
}

fn book() -> Workbook<TinySkiaBackend> {
    // Table1 on A1:C5.
    open(&std::fs::read(core().join("public/fixtures/objects-workbook.xlsx")).unwrap())
}

#[test]
fn special_table_headers_can_be_entered_in_formulas_without_extra_parentheses() {
    for (header, selector) in [
        ("Count(", "[[Count(]]"),
        ("Count)", "[[Count)]]"),
        ("Bob's", "[[Bob''s]]"),
    ] {
        let mut book = book();
        let expected: f64 = (1..=4)
            .filter_map(|row| book.number(CellRef::new(1, row)))
            .sum();
        book.set_cell(CellRef::new(1, 0), header).unwrap();
        let formula = format!("=SUM(Table1{selector})");
        let at = CellRef::new(25, 0);
        book.set_cell(at, &formula).unwrap();
        assert_eq!(book.number(at), Some(expected), "{header}");
        assert_eq!(book.edit_text(at), formula);
    }
}

#[test]
fn an_entry_starting_with_a_sign_becomes_a_formula_and_is_saved_as_one() {
    let mut book = book();
    let value = CellRef::parse("Z1").unwrap();
    book.set_cell(value, "5").unwrap();

    let negated = CellRef::parse("Z2").unwrap();
    book.set_cell(negated, "-Z1").unwrap();
    assert_eq!(book.number(negated), Some(-5.0));
    assert_eq!(book.edit_text(negated), "=-Z1");

    let doubled = CellRef::parse("Z3").unwrap();
    book.set_cell(doubled, "+Z1*2").unwrap();
    assert_eq!(book.number(doubled), Some(10.0));
    assert_eq!(book.edit_text(doubled), "=+Z1*2");

    // A signed number stays a number, and a sign alone stays text.
    let number = CellRef::parse("Z4").unwrap();
    book.set_cell(number, "-5").unwrap();
    assert_eq!(book.number(number), Some(-5.0));
    assert_eq!(book.edit_text(number), "-5");
    let sign = CellRef::parse("Z5").unwrap();
    book.set_cell(sign, "-").unwrap();
    assert_eq!(book.display_text(sign), "-");

    // A signed entry Excel would refuse as a formula is refused.
    assert!(book
        .set_cell(CellRef::parse("Z6").unwrap(), "-Z1+")
        .is_err());

    // The formula follows its cell.
    book.set_cell(value, "7").unwrap();
    assert_eq!(book.number(negated), Some(-7.0));

    let reopened = open(&book.save_bytes().unwrap());
    assert_eq!(reopened.edit_text(negated), "=-Z1");
    assert_eq!(reopened.edit_text(doubled), "=+Z1*2");
    assert_eq!(reopened.number(doubled), Some(14.0));
}

#[test]
fn autocomplete_lists_defined_names_and_tables_with_the_functions() {
    let mut book = book();
    book.define_range_name("SalesTax", CellRange::parse("C2:C4").unwrap())
        .unwrap();
    let names = book.formula_names();
    assert!(names.iter().any(|name| name == "SalesTax"));
    assert!(names.iter().any(|name| name == "Table1"));

    // Typing "=sa" lists the name among SUM and its kind, alphabetically.
    let list = formula_complete::completion_with("=sa", 3, &names).unwrap();
    assert!(list.names.iter().any(|name| name == "SalesTax"));
    let mut sorted = list.names.clone();
    sorted.sort_by_key(|name| name.to_uppercase());
    assert_eq!(sorted, list.names);

    // A table goes in without a parenthesis, a function with one.
    let mut edit = TextEdit::new("=SUM(tab".into());
    edit.caret = edit.text.len();
    let list = formula_complete::completion_with(&edit.text, edit.caret, &names).unwrap();
    assert!(list.names.iter().any(|name| name == "Table1"));
    formula_complete::accept(&mut edit, &list, "Table1");
    assert_eq!(edit.text, "=SUM(Table1");
    let mut edit = TextEdit::new("-su".into());
    edit.caret = edit.text.len();
    let list = formula_complete::completion_with(&edit.text, edit.caret, &names).unwrap();
    formula_complete::accept(&mut edit, &list, "SUM");
    assert_eq!(edit.text, "-SUM(");

    // The finished entry calculates.
    let at = CellRef::parse("AA1").unwrap();
    book.set_cell(at, "=SUM(SalesTax)").unwrap();
    assert!(book.number(at).is_some());
}

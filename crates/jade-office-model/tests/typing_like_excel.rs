//! What a cell becomes when a person types into it, as Excel reads it.
//!
//! Excel reads `10%`, `1,234`, `$1,234.50`, `2024/1/15` and `12:30` as
//! numbers — dates and times included — and gives the cell the format the
//! text was written in, so `=SUM` counts them and they show as typed. A
//! leading apostrophe keeps text as text. Anything that is not quite one
//! of these stays the text it was.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::sheet::{CellRef, Workbook};
use jade_office_model::tiny::TinySkiaBackend;

fn core() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"))
}

fn book() -> Workbook<TinySkiaBackend> {
    let directory = core().join("fonts/Noto_Sans_JP/static");
    let backend = TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        Some(Arc::new(
            std::fs::read(directory.join("NotoSansJP-Bold.ttf")).unwrap(),
        )),
    ))
    .unwrap();
    let bytes =
        std::fs::read(core().join("public/fixtures/grid-operations-workbook.xlsx")).unwrap();
    Workbook::open(&bytes, "UTC", backend).unwrap()
}

/// Typed text → what the cell shows, what the formula bar shows, and the
/// number it holds (`None` for text).
const CASES: &[(&str, &str, &str, Option<f64>)] = &[
    ("42", "42", "42", Some(42.0)),
    (" 42 ", "42", "42", Some(42.0)),
    ("1.5E3", "1500", "1500", Some(1500.0)),
    ("10%", "10%", "10%", Some(0.1)),
    ("12.5%", "12.50%", "12.5%", Some(0.125)),
    ("1,234", "1,234", "1234", Some(1234.0)),
    ("1,234.5", "1,234.50", "1234.5", Some(1234.5)),
    ("-1,234", "-1,234", "-1234", Some(-1234.0)),
    ("$1,234.50", "$1,234.50", "1234.5", Some(1234.5)),
    ("¥1,000", "¥1,000", "1000", Some(1000.0)),
    ("(100)", "-100", "-100", Some(-100.0)),
    ("2024/1/15", "2024/1/15", "2024/1/15", Some(45306.0)),
    ("2024-01-15", "2024/1/15", "2024/1/15", Some(45306.0)),
    ("12:30", "12:30", "12:30:00", Some(0.5208333333333334)),
    (
        "2024/1/15 12:30",
        "2024/1/15 12:30",
        "2024/1/15 12:30:00",
        Some(45306.520833333336),
    ),
    ("TRUE", "TRUE", "TRUE", None),
    ("'0123", "0123", "'0123", None),
    ("0123", "123", "123", Some(123.0)),
    ("1,2,3", "1,2,3", "1,2,3", None),
    ("2024/13/40", "2024/13/40", "2024/13/40", None),
    ("12:75", "12:75", "12:75", None),
    ("abc", "abc", "abc", None),
];

#[test]
fn typed_text_becomes_what_excel_makes_of_it() {
    let mut failures = Vec::new();
    for (index, (typed, shown, edited, number)) in CASES.iter().enumerate() {
        let mut book = book();
        let at = CellRef::new(8, 10 + index as u32);
        if let Err(error) = book.set_cell(at, typed) {
            failures.push(format!("{typed:?}: not accepted ({error})"));
            continue;
        }
        let display = book.display_text(at);
        let edit = book.edit_text(at);
        let value = book.number(at);
        if display != *shown
            || edit != *edited
            || value.map(|v| (v * 1e9).round()) != number.map(|v| (v * 1e9).round())
        {
            failures.push(format!(
                "{typed:?}: expected shown {shown:?}, bar {edited:?}, value {number:?}; got {display:?}, {edit:?}, {value:?}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn typed_numbers_count_and_one_typing_undoes_in_one_step() {
    let mut book = book();
    let (a, b, total) = (CellRef::new(8, 1), CellRef::new(8, 2), CellRef::new(8, 3));
    book.set_cell(a, "1,234").unwrap();
    book.set_cell(b, "10%").unwrap();
    book.set_cell(total, "=I2+I3").unwrap();
    assert_eq!(book.number(total), Some(1234.1));
    // Undoing the total, then the percentage, leaves the first entry.
    assert!(book.undo().unwrap());
    assert!(book.undo().unwrap());
    assert_eq!(book.edit_text(b), "");
    assert_eq!(book.display_text(a), "1,234");
    assert!(book.redo().unwrap());
    assert_eq!(book.display_text(b), "10%");
}

#[test]
fn a_misspelt_function_shows_name_error_and_so_do_its_dependents() {
    let mut book = book();
    let (a, b, total) = (CellRef::new(8, 1), CellRef::new(8, 2), CellRef::new(8, 3));
    book.set_cell(a, "5").unwrap();
    book.set_cell(b, "=I2+1").unwrap();
    book.set_cell(total, "=SUM(I2:I3)").unwrap();
    assert_eq!(book.number(total), Some(11.0));
    // A typo where a number was: Excel shows #NAME? there and in every cell
    // that reads it, not an empty cell and stale totals.
    book.set_cell(a, "=SUMM(1,2)").unwrap();
    assert_eq!(book.display_text(a), "#NAME?");
    assert_eq!(book.display_text(b), "#NAME?");
    assert_eq!(book.display_text(total), "#NAME?");
    assert_eq!(book.display_text(CellRef::new(8, 4)), "");
    // Fixing it brings the numbers back.
    book.set_cell(a, "=SUM(1,2)").unwrap();
    assert_eq!(book.number(b), Some(4.0));
    assert_eq!(book.number(total), Some(7.0));
}

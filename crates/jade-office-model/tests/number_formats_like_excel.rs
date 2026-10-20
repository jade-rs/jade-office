//! The Number Format dropdown: each entry gives the selection Excel's
//! format for it — in English and in Japanese, whose currency and dates
//! differ — shows what the active cell would read as in it, and the
//! button names the format the active cell has.

#![cfg(feature = "tiny-skia")]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::number_formats::{CUSTOM_NAME, NUMBER_FORMATS};
use jade_office_model::sheet::{CellFormat, CellRange, CellRef, Workbook};
use jade_office_model::tiny::TinySkiaBackend;

fn book() -> Workbook<TinySkiaBackend> {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let directory = core.join("fonts/Noto_Sans_JP/static");
    let backend = TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        None,
    ))
    .unwrap();
    let bytes = std::fs::read(core.join("public/fixtures/sample-workbook.xlsx")).unwrap();
    Workbook::open(&bytes, "UTC", backend).unwrap()
}

/// The shells' table for a language.
fn words(table: &'static str) -> HashMap<&'static str, &'static str> {
    table
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once('\t'))
        .collect()
}

fn english() -> HashMap<&'static str, &'static str> {
    words(include_str!("../../jade-gpui-app/i18n/en.tsv"))
}

fn japanese() -> HashMap<&'static str, &'static str> {
    words(include_str!("../../jade-gpui-app/i18n/ja.tsv"))
}

fn apply(book: &mut Workbook<TinySkiaBackend>, at: CellRef, code: &str) {
    let change = CellFormat {
        number_format: Some(code.to_string()),
        ..CellFormat::default()
    };
    book.format(CellRange { start: at, end: at }, &change)
        .unwrap();
}

#[test]
fn every_entry_formats_a_number_as_excel_shows_it() {
    let words = english();
    let mut book = book();
    let at = CellRef::new(5, 10);
    book.set_cell(at, "1234.5").unwrap();
    let expected = [
        ("nf-general", "1234.5"),
        ("nf-number", "1,234.50"),
        ("nf-currency", "$1,234.50 "),
        ("nf-date", "18-May-03"),
        ("nf-long-date", "Monday, May 18, 1903"),
        ("nf-time", "12:00 PM"),
        ("nf-percent", "123450.00%"),
        ("nf-fraction", "1234 1/2"),
        ("nf-scientific", "1.23E+03"),
        ("nf-text", "1234.5"),
    ];
    assert_eq!(NUMBER_FORMATS.len(), expected.len());
    for (choice, (id, shown)) in NUMBER_FORMATS.iter().zip(expected) {
        assert_eq!(choice.id, id);
        let code = words[choice.code];
        // The dropdown's sample is what the cell will show.
        assert_eq!(
            book.number_format_sample(at, code).as_deref(),
            Some(shown),
            "{id}"
        );
        apply(&mut book, at, code);
        assert_eq!(book.display_text(at), shown, "{id}");
        assert_eq!(
            book.number_format_name(at, |key| words[key]),
            choice.name,
            "the button names {id}"
        );
    }
}

#[test]
fn japanese_currency_and_dates_are_japanese_excels() {
    let words = japanese();
    let mut book = book();
    let at = CellRef::new(5, 10);
    book.set_cell(at, "1234.5").unwrap();
    let currency = NUMBER_FORMATS
        .iter()
        .find(|choice| choice.id == "nf-currency")
        .unwrap();
    apply(&mut book, at, words[currency.code]);
    assert_eq!(book.display_text(at), "¥1,235");
    assert_eq!(
        book.number_format_name(at, |key| words[key]),
        "sheet.format.currency_name"
    );
    let long_date = NUMBER_FORMATS
        .iter()
        .find(|choice| choice.id == "nf-long-date")
        .unwrap();
    apply(&mut book, at, words[long_date.code]);
    assert_eq!(book.display_text(at), "1903年5月18日");
}

#[test]
fn a_format_off_the_list_is_custom_and_an_empty_cell_has_no_sample() {
    let words = english();
    let mut book = book();
    let at = CellRef::new(5, 10);
    assert_eq!(
        book.number_format_name(at, |key| words[key]),
        "sheet.format.general_name",
        "a new cell is General"
    );
    assert_eq!(book.number_format_sample(at, "0.00"), None);
    book.set_cell(at, "7").unwrap();
    apply(&mut book, at, "0.000");
    assert_eq!(book.number_format_name(at, |key| words[key]), CUSTOM_NAME);
    assert!(words.contains_key(CUSTOM_NAME));
    assert!(japanese().contains_key(CUSTOM_NAME));
    // Text reads as itself, whatever the code.
    book.set_cell(at, "abc").unwrap();
    assert_eq!(
        book.number_format_sample(at, "0.00%").as_deref(),
        Some("abc")
    );
}

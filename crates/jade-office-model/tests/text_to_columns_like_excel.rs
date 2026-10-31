//! Data ▸ Text to Columns, delimited, as Excel's wizard does it: pieces at
//! the delimiters chosen, quotes keeping delimiters as text, consecutive
//! delimiters as one when asked, each piece taken as typing takes it.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::sheet::{span, CellRef, Workbook};
use jade_office_model::text_columns::{guess_breaks, TextColumnsDialog, TextToColumns};
use jade_office_model::tiny::TinySkiaBackend;

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

#[test]
fn the_split_follows_delimiters_quotes_and_consecutive_ones() {
    let comma = TextToColumns {
        tab: false,
        comma: true,
        ..TextToColumns::default()
    };
    assert_eq!(comma.split("a,b,,c"), ["a", "b", "", "c"]);
    assert_eq!(
        comma.split(r#""Tokyo, Japan","""quoted"" here",3"#),
        ["Tokyo, Japan", "\"quoted\" here", "3"]
    );
    let spaces = TextToColumns {
        tab: false,
        space: true,
        consecutive: true,
        ..TextToColumns::default()
    };
    assert_eq!(spaces.split("one   two three"), ["one", "two", "three"]);
}

#[test]
fn a_column_splits_into_the_columns_beside_it_as_typed() {
    let mut book = book();
    book.set_cell(CellRef::new(7, 29), "東京,1234,2026/1/5")
        .unwrap();
    book.set_cell(CellRef::new(7, 30), "大阪,56").unwrap();
    book.set_cell(CellRef::new(7, 31), "no delimiter").unwrap();
    let split = TextToColumns {
        tab: false,
        comma: true,
        ..TextToColumns::default()
    };
    assert!(book
        .text_to_columns(span(CellRef::new(7, 29), CellRef::new(7, 31)), &split)
        .unwrap());
    assert_eq!(book.display_text(CellRef::new(7, 29)), "東京");
    assert_eq!(book.edit_text(CellRef::new(8, 29)), "1234");
    // Taken as typed: a date, not text.
    assert_ne!(book.edit_text(CellRef::new(9, 29)), "2026/1/5x");
    assert!(book.display_text(CellRef::new(9, 29)).contains("2026"));
    assert_eq!(book.display_text(CellRef::new(8, 30)), "56");
    assert_eq!(book.display_text(CellRef::new(7, 31)), "no delimiter");
    // One step back.
    assert!(book.undo().unwrap());
    assert_eq!(book.display_text(CellRef::new(7, 29)), "東京,1234,2026/1/5");
    assert_eq!(book.display_text(CellRef::new(8, 29)), "");
}

/// Fixed width: fields cut at the breaks, without the spaces round them;
/// the wizard guesses the breaks from where every line has a space.
#[test]
fn fixed_width_cuts_at_the_breaks_the_wizard_guesses() {
    let lines = vec![
        "Tokyo   1200  East".to_string(),
        "Osaka    800  West".to_string(),
    ];
    assert_eq!(guess_breaks(&lines), [8, 14]);
    let fixed = TextToColumns {
        fixed_width: Some(vec![8, 14]),
        ..TextToColumns::default()
    };
    assert_eq!(fixed.split(&lines[0]), ["Tokyo", "1200", "East"]);
    // A short line has no pieces past its end; tabs do not split.
    assert_eq!(fixed.split("Nagoya\t9"), ["Nagoya\t9"]);
    assert_eq!(fixed.split("Kobe     70"), ["Kobe", "70"]);

    let mut book = book();
    book.set_cell(CellRef::new(7, 29), &lines[0]).unwrap();
    book.set_cell(CellRef::new(7, 30), &lines[1]).unwrap();
    let at = CellRef::new(7, 29);
    let mut dialog = TextColumnsDialog::open(&book, span(at, at), at);
    dialog.set_fixed_width(true);
    assert_eq!(dialog.split.fixed_width.as_deref(), Some(&[8, 14][..]));
    assert_eq!(dialog.sample_width(), 18);
    dialog.toggle_break(14);
    dialog.toggle_break(3);
    assert_eq!(dialog.split.fixed_width.as_deref(), Some(&[3, 8][..]));
    dialog.toggle_break(3);
    dialog.toggle_break(14);
    assert_eq!(dialog.preview()[1], ["Osaka", "800", "West"]);
    assert!(book.text_to_columns(dialog.range, &dialog.split).unwrap());
    assert_eq!(book.display_text(CellRef::new(7, 30)), "Osaka");
    assert_eq!(book.edit_text(CellRef::new(8, 30)), "800");
    assert_eq!(book.display_text(CellRef::new(9, 29)), "East");
    dialog.set_fixed_width(false);
    assert!(dialog.split.fixed_width.is_none());
}

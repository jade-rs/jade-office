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

/// The third step: a column taken as Text keeps `007` as written, in the
/// Text format; a skipped one is left out and the rest close up.
#[test]
fn column_formats_keep_text_and_skip_columns() {
    use jade_office_model::text_columns::ColumnFormat;
    let mut book = book();
    let at = CellRef::new(20, 40);
    book.set_cell(at, "007,drop,2026/1/5").unwrap();
    let mut dialog = TextColumnsDialog::open(&book, span(at, at), at);
    dialog.split = TextToColumns {
        tab: false,
        comma: true,
        ..TextToColumns::default()
    };
    dialog.select_column(0);
    dialog.set_format(ColumnFormat::Text);
    dialog.select_column(1);
    dialog.set_format(ColumnFormat::Skip);
    assert_eq!(dialog.format(), ColumnFormat::Skip);
    assert!(dialog.apply(&mut book).unwrap());
    assert_eq!(book.display_text(at), "007");
    assert_eq!(book.format_at(at).number_format.as_deref(), Some("@"));
    // The date closes up next to it, shown as a date.
    let date = CellRef::new(21, 40);
    assert_eq!(book.display_text(date), "2026/1/5");
    assert_eq!(book.display_text(CellRef::new(22, 40)), "");
    // One undo takes the lot back.
    assert!(book.undo().unwrap());
    assert_eq!(book.edit_text(at), "007,drop,2026/1/5");
}

/// Data ▸ From Text: the file's lines from the start row, split at what
/// it is split at, a quoted line break kept in its field, put at the active
/// cell as one undo step.
#[test]
fn from_text_imports_a_file_at_the_active_cell() {
    use jade_office_model::text_columns::ColumnFormat;
    let mut book = book();
    let file =
        "Exported 2026-11-01\r\nCode,Name,Amount\r\n007,\"Two\nlines\",1200\r\n010,Plain,30\r\n";
    let at = CellRef::new(20, 50);
    let mut dialog = TextColumnsDialog::open_import("export.txt", file.as_bytes(), at);
    assert!(dialog.split.comma && !dialog.split.tab);
    assert_eq!(dialog.sample.len(), 4);
    dialog.step_start(1);
    assert_eq!(dialog.sample[0], "Code,Name,Amount");
    assert_eq!(dialog.preview()[1], ["007", "Two\nlines", "1200"]);
    dialog.select_column(0);
    dialog.set_format(ColumnFormat::Text);
    assert!(dialog.apply(&mut book).unwrap());
    assert_eq!(book.display_text(at), "Code");
    assert_eq!(book.display_text(CellRef::new(20, 51)), "007");
    assert_eq!(book.edit_text(CellRef::new(21, 51)), "Two\nlines");
    assert_eq!(book.number(CellRef::new(22, 51)), Some(1200.0));
    assert_eq!(book.display_text(CellRef::new(20, 52)), "010");
    assert!(book.undo().unwrap());
    assert_eq!(book.display_text(at), "");
}

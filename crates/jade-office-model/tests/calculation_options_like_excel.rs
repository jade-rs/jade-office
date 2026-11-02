//! The Calculation Options dialog, as Excel's: iterative calculation lets a
//! circular reference settle, precision as displayed calculates with what
//! cells show, "automatic except for data tables" is a mode of its own,
//! and all of it is saved with the workbook.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::calculation_options::{
    CalculationField, CalculationKey, CalculationOptionsDialog,
};
use jade_office_model::sheet::{CellFormat, CellRange, CellRef, Workbook};
use jade_office_model::tiny::TinySkiaBackend;
use jade_xlsx_reader::CalculationMode;

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

fn number_format(book: &mut Workbook<TinySkiaBackend>, at: CellRef, code: &str) {
    let change = CellFormat {
        number_format: Some(code.to_string()),
        ..CellFormat::default()
    };
    book.format(CellRange { start: at, end: at }, &change)
        .unwrap();
}

/// The keys typed into the dialog, one by one.
fn type_keys(dialog: &mut CalculationOptionsDialog, keys: &[&str]) {
    for key in keys {
        dialog.key(key);
    }
}

#[test]
fn iterative_calculation_from_the_dialog() {
    let mut book = book();
    // A balance including the interest on itself: circular.
    book.set_cell(CellRef::new(7, 29), "1000").unwrap();
    book.set_cell(CellRef::new(7, 30), "=H30+H32").unwrap();
    book.set_cell(CellRef::new(7, 31), "=ROUND(H31*0.1,2)")
        .unwrap();

    let mut dialog = CalculationOptionsDialog::new(book.calculation_options());
    assert!(!dialog.options.iteration.enabled);
    dialog.toggle_iteration();
    dialog.focus(CalculationField::MaximumIterations);
    type_keys(&mut dialog, &["backspace", "backspace", "backspace", "5", "0"]);
    assert_eq!(dialog.key("enter"), CalculationKey::Take);
    assert!(dialog.apply(&mut book).unwrap());

    let options = book.calculation_options();
    assert!(options.iteration.enabled);
    assert_eq!(options.iteration.count, 50);
    assert_eq!(book.display_text(CellRef::new(7, 30)), "1111.11");
    assert_eq!(book.display_text(CellRef::new(7, 31)), "111.11");

    // Saved, and read back the same.
    let bytes = book.save_bytes().unwrap();
    let reopened = Workbook::open(&bytes, "UTC", backend()).unwrap();
    assert_eq!(reopened.calculation_options(), options);
    assert_eq!(reopened.display_text(CellRef::new(7, 30)), "1111.11");
}

#[test]
fn precision_as_displayed_and_the_modes() {
    let mut book = book();
    book.set_cell(CellRef::new(7, 29), "=1/3").unwrap();
    book.set_cell(CellRef::new(7, 30), "=H30*3").unwrap();
    book.set_cell(CellRef::new(7, 31), "=H30*3").unwrap();
    number_format(&mut book, CellRef::new(7, 29), "0.00");
    number_format(&mut book, CellRef::new(7, 31), "0.0000");
    assert_eq!(book.display_text(CellRef::new(7, 31)), "1.0000");

    let mut dialog = CalculationOptionsDialog::new(book.calculation_options());
    dialog.toggle_precision_as_displayed();
    dialog.set_mode(CalculationMode::AutoNoTable);
    assert!(dialog.apply(&mut book).unwrap());
    // H30 holds the 0.33 it shows.
    assert_eq!(book.display_text(CellRef::new(7, 31)), "0.9900");
    let options = book.calculation_options();
    assert!(options.precision_as_displayed);
    assert_eq!(options.mode, CalculationMode::AutoNoTable);
    // Not manual: edits still recalculate.
    assert!(!book.manual_calculation());

    let bytes = book.save_bytes().unwrap();
    let reopened = Workbook::open(&bytes, "UTC", backend()).unwrap();
    assert_eq!(reopened.calculation_options(), options);

    // Manual, without recalculating before saving.
    let mut dialog = CalculationOptionsDialog::new(book.calculation_options());
    dialog.set_mode(CalculationMode::Manual);
    dialog.toggle_recalculate_before_save();
    assert!(dialog.apply(&mut book).unwrap());
    assert!(book.manual_calculation());
    assert!(!book.calculation_options().recalculate_before_save);
}

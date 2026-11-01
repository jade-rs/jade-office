//! The Page Setup dialog as Excel's: orientation, paper, a scale or fitting
//! to pages, the margin presets, and centring and gridlines, written to the
//! sheet as one undo step.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::page_setup_dialog::{MarginPreset, PageSetupDialog, PrintOption};
use jade_office_model::sheet::Workbook;
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
fn the_dialog_writes_orientation_paper_scaling_and_margins() {
    let mut book = book();
    let mut dialog = PageSetupDialog::open(&book);
    assert!(!dialog.landscape());
    dialog.set_landscape(true);
    dialog.set_paper(8);
    dialog.step_scale(-30);
    assert_eq!(dialog.scale(), 70);
    dialog.set_margins(MarginPreset::Narrow);
    assert_eq!(dialog.margin_preset(), Some(MarginPreset::Narrow));
    assert!(book.set_page_setup(&dialog.setup).unwrap());

    let again = PageSetupDialog::open(&book);
    assert!(again.landscape());
    assert_eq!(again.setup.paper_size, Some(8));
    assert_eq!(again.scale(), 70);
    assert!(!again.setup.fit_to_page);
    assert_eq!(again.margin_preset(), Some(MarginPreset::Narrow));

    // Fitting all columns on one page; the scale stays for later.
    let mut fit = again.clone();
    fit.set_fit((1, 0));
    assert!(book.set_page_setup(&fit.setup).unwrap());
    assert_eq!(book.page_setup().fit_to(), Some((1, 0)));
    // The scale is held between 10 and 400.
    fit.step_scale(1000);
    assert_eq!(fit.scale(), 400);
    assert!(!fit.setup.fit_to_page);

    assert!(book.undo().unwrap());
    assert_eq!(book.page_setup().fit_to(), None);
    assert!(book.undo().unwrap());
    assert!(!PageSetupDialog::open(&book).landscape());
}

#[test]
fn the_dialog_toggles_centring_and_gridlines() {
    let mut book = book();
    let mut dialog = PageSetupDialog::open(&book);
    for option in PrintOption::ALL {
        assert!(!dialog.print_option(option));
    }
    dialog.toggle_print_option(PrintOption::CenterHorizontally);
    dialog.toggle_print_option(PrintOption::GridLines);
    assert!(book.set_page_setup(&dialog.setup).unwrap());
    let again = PageSetupDialog::open(&book);
    assert!(again.print_option(PrintOption::CenterHorizontally));
    assert!(!again.print_option(PrintOption::CenterVertically));
    assert!(again.print_option(PrintOption::GridLines));
    assert!(book.undo().unwrap());
    assert!(!PageSetupDialog::open(&book).print_option(PrintOption::GridLines));
}

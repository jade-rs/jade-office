//! Data ▸ What-If Analysis ▸ Goal Seek as Excel's: the changing cell's
//! value that brings a formula to its target, a status to keep it with OK
//! or take it back with Cancel, and one undo step.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::form::Label;
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

/// A loan's payment: rate in H30, years in H31, amount in H32, and the
/// monthly payment in H33.
fn loan(book: &mut Workbook<TinySkiaBackend>) {
    book.set_cell(CellRef::new(7, 29), "0.05").unwrap();
    book.set_cell(CellRef::new(7, 30), "30").unwrap();
    book.set_cell(CellRef::new(7, 31), "100000").unwrap();
    book.set_cell(CellRef::new(7, 32), "=-PMT(H30/12,H31*12,H32)")
        .unwrap();
}

#[test]
fn goal_seek_finds_the_amount_a_payment_affords_in_one_undo_step() {
    let mut book = book();
    loan(&mut book);
    let payment = CellRef::new(7, 32);
    let amount = CellRef::new(7, 31);
    let sought = book.goal_seek(payment, 1000.0, amount).unwrap();
    assert!(sought.found);
    assert!((sought.result - 1000.0).abs() < 0.001, "{sought:?}");
    // 1000 a month at 5% for 30 years borrows about 186,281.62.
    assert!((sought.value - 186_281.62).abs() < 0.01, "{sought:?}");
    assert!((book.number(amount).unwrap() - sought.value).abs() < 1e-9);
    assert!(book.undo().unwrap());
    assert_eq!(book.number(amount), Some(100000.0));
}

#[test]
fn goal_seek_refuses_a_set_cell_without_a_formula_and_a_changing_formula() {
    let mut book = book();
    loan(&mut book);
    assert_eq!(
        book.goal_seek(CellRef::new(7, 31), 10.0, CellRef::new(7, 30))
            .unwrap_err(),
        "goal_seek.alert.formula"
    );
    assert_eq!(
        book.goal_seek(CellRef::new(7, 32), 10.0, CellRef::new(7, 32))
            .unwrap_err(),
        "goal_seek.alert.value"
    );
}

#[test]
fn a_target_out_of_reach_is_not_found_and_the_nearest_try_stays() {
    let mut book = book();
    book.set_cell(CellRef::new(7, 29), "3").unwrap();
    book.set_cell(CellRef::new(7, 30), "=H30*H30+1").unwrap();
    let sought = book
        .goal_seek(CellRef::new(7, 30), 0.0, CellRef::new(7, 29))
        .unwrap();
    assert!(!sought.found);
    // x² + 1 is never below 1: the try nearest is close to x = 0.
    assert!(sought.result < 1.01, "{sought:?}");
}

#[test]
fn the_dialog_asks_then_shows_its_status_and_cancel_puts_the_value_back() {
    let mut book = book();
    loan(&mut book);
    let payment = CellRef::new(7, 32);
    let mut dialog =
        WhatIfDialog::open(WhatIfTool::GoalSeek, &book, span(payment, payment), payment);
    // Set cell starts as the active cell.
    assert_eq!(dialog.form().text("set"), "$H$33");
    dialog.form_mut().set_text("to", "abc");
    dialog.form_mut().set_text("changing", "H32");
    let outcome = dialog.press("ok", &mut book).unwrap();
    assert!(!outcome.close);
    assert_eq!(
        dialog.form().alert,
        Some(Label::Key("goal_seek.alert.number"))
    );
    dialog.form_mut().set_text("to", "1000");
    let outcome = dialog.press("ok", &mut book).unwrap();
    assert!(outcome.wrote && !outcome.close);
    assert_eq!(
        dialog.form().items[0].id,
        "outcome",
        "the status replaces what was asked"
    );
    assert!((book.number(CellRef::new(7, 31)).unwrap() - 186_281.62).abs() < 0.01);
    let outcome = dialog.press("cancel", &mut book).unwrap();
    assert!(outcome.close && outcome.wrote);
    assert_eq!(book.number(CellRef::new(7, 31)), Some(100000.0));
    // Nothing left to undo from it: the loan's own formula is the last edit.
    assert!(book.undo().unwrap());
    assert_eq!(book.edit_text(payment), "");
}

#[test]
fn the_status_ok_keeps_the_value() {
    let mut book = book();
    loan(&mut book);
    let payment = CellRef::new(7, 32);
    let mut dialog =
        WhatIfDialog::open(WhatIfTool::GoalSeek, &book, span(payment, payment), payment);
    dialog.form_mut().set_text("to", "1000");
    dialog.form_mut().set_text("changing", "$H$32");
    dialog.press("ok", &mut book).unwrap();
    let outcome = dialog.press("ok", &mut book).unwrap();
    assert!(outcome.close);
    assert!((book.number(CellRef::new(7, 31)).unwrap() - 186_281.62).abs() < 0.01);
}

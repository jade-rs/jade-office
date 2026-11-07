//! Copy and paste speak Excel's clipboard text: tab between cells, a line
//! break between rows, and a cell holding a line break, a tab or a quote
//! wrapped in quotes with its quotes doubled. Windows Excel ends rows with
//! CR LF. A multi-line cell pasted from Excel stays one cell, and one copied
//! out arrives in Excel as one cell.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::sheet::{span, CellRef, Workbook};
use jade_office_model::tiny::TinySkiaBackend;

fn book() -> Workbook<TinySkiaBackend> {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let directory = core.join("fonts/Noto_Sans_JP/static");
    let backend = TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        Some(Arc::new(
            std::fs::read(directory.join("NotoSansJP-Bold.ttf")).unwrap(),
        )),
    ))
    .unwrap();
    let bytes = std::fs::read(core.join("public/fixtures/grid-operations-workbook.xlsx")).unwrap();
    Workbook::open(&bytes, "UTC", backend).unwrap()
}

#[test]
fn pasting_excel_text_keeps_quoted_cells_whole() {
    let mut book = book();
    let at = CellRef::new(8, 1);
    let clipboard = "\"line one\nline two\"\tplain\r\n\"say \"\"hi\"\"\"\t\"a\tb\"\r\n";
    let far = book.paste(at, clipboard).unwrap();
    assert_eq!(far, Some(CellRef::new(9, 2)));
    assert_eq!(book.edit_text(CellRef::new(8, 1)), "line one\nline two");
    assert_eq!(book.edit_text(CellRef::new(9, 1)), "plain");
    assert_eq!(book.edit_text(CellRef::new(8, 2)), "say \"hi\"");
    assert_eq!(book.edit_text(CellRef::new(9, 2)), "a\tb");
    assert_eq!(book.edit_text(CellRef::new(8, 3)), "");
}

#[test]
fn copying_quotes_what_excel_would_split() {
    let mut book = book();
    book.paste(CellRef::new(8, 1), "\"two\nlines\"\tsay \"hi\"\tplain\n")
        .unwrap();
    let text = book.range_text(span(CellRef::new(8, 1), CellRef::new(10, 1)));
    assert_eq!(text, "\"two\nlines\"\t\"say \"\"hi\"\"\"\tplain\n");
    // And it comes back in as it went out.
    book.paste(CellRef::new(8, 5), &text).unwrap();
    assert_eq!(book.edit_text(CellRef::new(8, 5)), "two\nlines");
    assert_eq!(book.edit_text(CellRef::new(9, 5)), "say \"hi\"");
}

fn at(reference: &str) -> CellRef {
    CellRef::parse(reference).unwrap()
}

#[test]
fn copying_a_formula_pastes_it_adjusted_as_in_excel() {
    let mut book = book();
    book.set_cell(at("I2"), "3").unwrap();
    book.set_cell(at("I3"), "4").unwrap();
    book.set_cell(at("J2"), "=I2*2").unwrap();
    let text = book.copy(span(at("J2"), at("J2")), false);
    assert_eq!(text, "6\n");
    // The clipboard comes back as the system gives it, CR LF and all.
    let far = book.paste(at("J3"), "6\r\n").unwrap();
    assert_eq!(far, Some(at("J3")));
    assert_eq!(book.edit_text(at("J3")), "=I3*2");
    assert_eq!(book.display_text(at("J3")), "8");
}

#[test]
fn clipboard_keeps_scalar_array_evaluation_through_model_history_and_save() {
    let mut book = book();
    for (reference, text) in [("I1", "a"), ("I2", "bb"), ("I3", "ccc")] {
        book.set_cell(at(reference), text).unwrap();
    }
    let formula = "=SUM(LEN($I$1:$I$3))";
    book.set_cell(at("J5"), formula).unwrap();
    assert_eq!(book.display_text(at("J5")), "6");
    assert_eq!(book.copy(span(at("J5"), at("J5")), false), "6\n");
    book.paste(at("L5"), "6\r\n").unwrap();
    assert_eq!(book.display_text(at("L5")), "6");
    assert_eq!(book.edit_text(at("L5")), formula);
    assert!(book.undo().unwrap());
    assert_eq!(book.display_text(at("L5")), "");
    assert!(book.redo().unwrap());
    let text = book.copy(span(at("L5"), at("L5")), true);
    book.paste(at("N5"), &text).unwrap();
    assert_eq!(book.display_text(at("L5")), "");
    assert_eq!(book.display_text(at("N5")), "6");
    assert_eq!(book.edit_text(at("N5")), formula);
    let saved = book.save_bytes().unwrap();
    let reopened = jade_xlsx_reader::read_xlsx(&saved).unwrap();
    assert!(reopened.sheets[0].array_formulas.iter().any(|array| {
        array.anchor == at("N5") && array.kind == jade_xlsx_reader::ArrayFormulaKind::Dynamic
    }));
}

#[test]
fn a_cut_moves_the_cells_when_pasted_and_references_follow() {
    let mut book = book();
    book.set_cell(at("I2"), "5").unwrap();
    book.set_cell(at("J2"), "=I2+1").unwrap();
    let text = book.copy(span(at("I2"), at("I2")), true);
    // Cutting alone changes nothing, as in Excel.
    assert_eq!(book.display_text(at("I2")), "5");
    book.paste(at("K5"), &text).unwrap();
    assert_eq!(book.display_text(at("I2")), "");
    assert_eq!(book.display_text(at("K5")), "5");
    assert_eq!(book.edit_text(at("J2")), "=K5+1");
    assert_eq!(book.display_text(at("J2")), "6");
    // One undo puts the cells back.
    assert!(book.undo().unwrap());
    assert_eq!(book.display_text(at("I2")), "5");
    assert_eq!(book.edit_text(at("J2")), "=I2+1");
}

#[test]
fn text_from_elsewhere_still_pastes_as_text() {
    let mut book = book();
    book.set_cell(at("J2"), "=1+1").unwrap();
    book.copy(span(at("J2"), at("J2")), false);
    // The clipboard now holds something else: it pastes as typed text.
    book.paste(at("J3"), "=1+2").unwrap();
    assert_eq!(book.display_text(at("J3")), "3");
}

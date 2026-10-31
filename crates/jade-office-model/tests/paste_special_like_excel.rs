//! Paste Special as Excel's dialog does it: Values, Formulas, Formats and
//! All, Skip blanks and Transpose, from the cells as they are now; one
//! undo; nothing after a cut; text from elsewhere pasting its values.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::paste_special::{PasteSpecial, PasteWhat};
use jade_office_model::sheet::{span, CellFormat, CellRange, CellRef, Workbook};
use jade_office_model::tiny::TinySkiaBackend;

/// Name/Amount/Double over Alpha 10 =B2*2, Beta 20 =B3*2, Gamma 30 =B4*2.
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

fn at(column: u32, row: u32) -> CellRef {
    CellRef::new(column, row)
}

fn one(cell: CellRef) -> CellRange {
    CellRange {
        start: cell,
        end: cell,
    }
}

fn special(what: PasteWhat) -> PasteSpecial {
    PasteSpecial {
        what,
        ..PasteSpecial::default()
    }
}

fn bold(book: &mut Workbook<TinySkiaBackend>, range: CellRange) {
    let change = CellFormat {
        bold: Some(true),
        ..CellFormat::default()
    };
    book.format(range, &change).unwrap();
}

#[test]
fn values_paste_what_formulas_show_keeping_the_formats_pasted_on() {
    let mut book = book();
    bold(&mut book, one(at(5, 1)));
    book.copy(span(at(2, 1), at(2, 3)), false);
    let landed = book
        .paste_special(at(5, 1), None, special(PasteWhat::Values))
        .unwrap();
    assert_eq!(landed, Some(span(at(5, 1), at(5, 3))));
    assert_eq!(book.edit_text(at(5, 1)), "20", "the result, not =B2*2");
    assert_eq!(book.edit_text(at(5, 3)), "60");
    assert_eq!(book.format_at(at(5, 1)).bold, Some(true), "F2 kept bold");
    // One undo takes the whole paste back.
    book.undo().unwrap();
    assert_eq!(book.edit_text(at(5, 1)), "");
    assert_eq!(book.edit_text(at(5, 3)), "");
}

#[test]
fn formulas_move_as_a_paste_moves_them_and_formats_stay() {
    let mut book = book();
    bold(&mut book, one(at(2, 1)));
    book.copy(span(at(2, 1), at(2, 3)), false);
    book.paste_special(at(5, 1), None, special(PasteWhat::Formulas))
        .unwrap();
    assert_eq!(book.edit_text(at(5, 1)), "=E2*2");
    assert_ne!(
        book.format_at(at(5, 1)).bold,
        Some(true),
        "C2's bold stays put"
    );
    // All brings the format too.
    book.paste_special(at(7, 1), None, special(PasteWhat::All))
        .unwrap();
    assert_eq!(book.edit_text(at(7, 1)), "=G2*2");
    assert_eq!(book.format_at(at(7, 1)).bold, Some(true));
}

#[test]
fn formats_paste_the_look_and_leave_the_values() {
    let mut book = book();
    bold(&mut book, one(at(1, 1)));
    book.set_cell(at(7, 4), "keep").unwrap();
    book.copy(one(at(1, 1)), false);
    book.paste_special(at(7, 4), None, special(PasteWhat::Formats))
        .unwrap();
    assert_eq!(book.display_text(at(7, 4)), "keep");
    assert_eq!(book.format_at(at(7, 4)).bold, Some(true));
}

#[test]
fn transpose_turns_the_block_and_its_formulas() {
    let mut book = book();
    book.copy(span(at(0, 0), at(2, 1)), false);
    let transpose = PasteSpecial {
        transpose: true,
        ..PasteSpecial::default()
    };
    let landed = book.paste_special(at(4, 9), None, transpose).unwrap();
    // Two rows of three become three rows of two.
    assert_eq!(landed, Some(span(at(4, 9), at(5, 11))));
    let shown = |book: &Workbook<TinySkiaBackend>, cell| book.display_text(cell);
    assert_eq!(shown(&book, at(4, 9)), "Name");
    assert_eq!(shown(&book, at(4, 10)), "Amount");
    assert_eq!(shown(&book, at(4, 11)), "Double");
    assert_eq!(shown(&book, at(5, 9)), "Alpha");
    assert_eq!(shown(&book, at(5, 10)), "10");
    // C2's =B2*2 lands on F12 and names the cell above it, where B2 went.
    assert_eq!(book.edit_text(at(5, 11)), "=F11*2");
    assert_eq!(shown(&book, at(5, 11)), "20");
}

#[test]
fn skip_blanks_leaves_the_cells_under_an_empty_copied_one() {
    let mut book = book();
    // A1:E1 has nothing in D1.
    book.set_cell(at(3, 19), "here").unwrap();
    book.copy(span(at(0, 0), at(4, 0)), false);
    let skipping = PasteSpecial {
        skip_blanks: true,
        ..PasteSpecial::default()
    };
    book.paste_special(at(0, 19), None, skipping).unwrap();
    assert_eq!(book.display_text(at(3, 19)), "here");
    assert_eq!(book.display_text(at(4, 19)), "Merged header");
    book.paste_special(at(0, 19), None, PasteSpecial::default())
        .unwrap();
    assert_eq!(book.display_text(at(3, 19)), "", "pasted over without it");
}

#[test]
fn it_pastes_the_cells_as_they_are_now_and_not_after_a_cut() {
    let mut book = book();
    book.copy(one(at(1, 1)), false);
    book.set_cell(at(1, 1), "99").unwrap();
    assert!(book.can_paste_special(None));
    book.paste_special(at(6, 6), None, special(PasteWhat::Values))
        .unwrap();
    assert_eq!(book.display_text(at(6, 6)), "99");
    book.copy(one(at(1, 2)), true);
    assert!(!book.can_paste_special(None), "a cut has no Paste Special");
    assert_eq!(
        book.paste_special(at(6, 7), None, PasteSpecial::default())
            .unwrap(),
        None
    );
    assert_eq!(book.display_text(at(6, 7)), "");
}

#[test]
fn text_from_elsewhere_pastes_its_values_and_transposes() {
    let mut book = book();
    book.forget_copy();
    assert!(!book.can_paste_special(None));
    assert!(book.can_paste_special(Some("1\t2\t3")));
    let transpose = PasteSpecial {
        what: PasteWhat::Values,
        transpose: true,
        ..PasteSpecial::default()
    };
    let landed = book
        .paste_special(at(8, 0), Some("1\t2\t3"), transpose)
        .unwrap();
    assert_eq!(landed, Some(span(at(8, 0), at(8, 2))));
    assert_eq!(book.number(at(8, 2)), Some(3.0));
}

#[test]
fn the_dialog_moves_between_its_choices_with_the_arrows() {
    let mut dialog = PasteSpecial::default();
    assert_eq!(dialog.what, PasteWhat::All);
    assert!(dialog.key("down"));
    assert!(dialog.key("down"));
    assert_eq!(dialog.what, PasteWhat::Values);
    dialog.key("down");
    dialog.key("down");
    dialog.key("down");
    assert_eq!(dialog.what, PasteWhat::ColumnWidths, "stops at the last");
    dialog.key("up");
    dialog.key("up");
    assert_eq!(dialog.what, PasteWhat::Values);
    assert!(!dialog.key("x"));
    let labels: Vec<_> = PasteWhat::ALL.iter().map(|what| what.label()).collect();
    for words in [
        include_str!("../../jade-gpui-app/i18n/en.tsv"),
        include_str!("../../jade-gpui-app/i18n/ja.tsv"),
    ] {
        for label in &labels {
            assert!(
                words
                    .lines()
                    .any(|line| line.starts_with(&format!("{label}\t"))),
                "{label}"
            );
        }
    }
}

/// Paste Special ▸ Values with Add, Multiply and Divide combines each
/// copied number with the one pasted on: a blank counts as 0, a formula
/// pasted on becomes one with the number, and text pasted on stays.
#[test]
fn operations_combine_the_copied_numbers_with_those_pasted_on() {
    use jade_office_model::paste_special::PasteOperation;
    let mut book = book();
    book.set_cell(at(20, 50), "5").unwrap();
    book.set_cell(at(20, 51), "5").unwrap();
    book.set_cell(at(20, 52), "5").unwrap();
    book.set_cell(at(20, 53), "5").unwrap();
    book.set_cell(at(22, 50), "10").unwrap();
    book.set_cell(at(22, 51), "=W51*2").unwrap();
    book.set_cell(at(22, 52), "text").unwrap();
    // W54 is blank.
    let with = |operation| PasteSpecial {
        what: PasteWhat::Values,
        operation,
        ..PasteSpecial::default()
    };
    book.copy(span(at(20, 50), at(20, 53)), false);
    book.paste_special(at(22, 50), None, with(PasteOperation::Add))
        .unwrap();
    assert_eq!(book.display_text(at(22, 50)), "15");
    assert_eq!(book.edit_text(at(22, 51)), "=(W51*2)+5");
    assert_eq!(book.display_text(at(22, 52)), "text");
    assert_eq!(book.display_text(at(22, 53)), "5");
    // One undo takes it all back.
    assert!(book.undo().unwrap());
    assert_eq!(book.display_text(at(22, 50)), "10");
    assert_eq!(book.edit_text(at(22, 51)), "=W51*2");

    book.copy(span(at(20, 50), at(20, 50)), false);
    book.paste_special(at(22, 50), None, with(PasteOperation::Multiply))
        .unwrap();
    assert_eq!(book.display_text(at(22, 50)), "50");
    book.paste_special(at(22, 50), None, with(PasteOperation::Divide))
        .unwrap();
    assert_eq!(book.display_text(at(22, 50)), "10");
    book.set_cell(at(20, 50), "0").unwrap();
    book.copy(span(at(20, 50), at(20, 50)), false);
    book.paste_special(at(22, 50), None, with(PasteOperation::Divide))
        .unwrap();
    assert_eq!(book.display_text(at(22, 50)), "#DIV/0!");
}

/// Paste Link writes formulas reading the copied cells: absolute for one
/// cell, relative for several, with the sheet's name from another sheet.
#[test]
fn paste_link_writes_formulas_reading_the_copied_cells() {
    let mut book = book();
    book.set_cell(at(20, 60), "7").unwrap();
    book.set_cell(at(21, 60), "8").unwrap();
    book.copy(span(at(20, 60), at(20, 60)), false);
    assert_eq!(
        book.paste_link(at(23, 60)).unwrap(),
        Some(span(at(23, 60), at(23, 60)))
    );
    assert_eq!(book.edit_text(at(23, 60)), "=$U$61");
    assert_eq!(book.display_text(at(23, 60)), "7");
    book.copy(span(at(20, 60), at(21, 60)), false);
    book.paste_link(at(23, 62)).unwrap();
    assert_eq!(book.edit_text(at(24, 62)), "=V61");
    // The link follows the source.
    book.set_cell(at(21, 60), "80").unwrap();
    assert_eq!(book.display_text(at(24, 62)), "80");
    // From another sheet, the sheet's name goes in front.
    let here = book.sheet_index();
    let name = book.sheet_names()[here].clone();
    book.copy(span(at(20, 60), at(20, 60)), false);
    let other = book.add_sheet().unwrap();
    book.select_sheet(other).unwrap();
    book.paste_link(at(0, 0)).unwrap();
    assert!(book.edit_text(at(0, 0)).ends_with("!$U$61"));
    assert!(book.edit_text(at(0, 0)).contains(&name));
    assert_eq!(book.display_text(at(0, 0)), "7");
    // Not after a cut.
    book.select_sheet(here).unwrap();
    book.copy(span(at(20, 60), at(20, 60)), true);
    assert_eq!(book.paste_link(at(25, 60)).unwrap(), None);
}

/// Paste Special ▸ Column widths: the copied columns' widths, nothing else.
#[test]
fn column_widths_paste_only_the_widths() {
    use jade_xlsx_reader::Axis;
    let mut book = book();
    book.set_cell(at(20, 60), "wide").unwrap();
    book.resize(Axis::Columns, 20, 20, 200.0).unwrap();
    book.copy(span(at(20, 60), at(20, 60)), false);
    let special = PasteSpecial {
        what: PasteWhat::ColumnWidths,
        ..PasteSpecial::default()
    };
    book.paste_special(at(23, 60), None, special).unwrap();
    assert_eq!(book.display_text(at(23, 60)), "", "no value pasted");
    let width = |book: &mut Workbook<TinySkiaBackend>, column: u32| {
        let saved = jade_xlsx_reader::read_xlsx(&book.save_bytes().unwrap()).unwrap();
        let sheet = &saved.sheets[book.sheet_index()];
        sheet
            .columns
            .iter()
            .find(|span| span.first <= column && column <= span.last)
            .and_then(|span| span.width_chars)
    };
    let source = width(&mut book, 20);
    assert!(source.is_some());
    assert_eq!(width(&mut book, 23), source);
    assert!(book.undo().unwrap());
    assert_ne!(width(&mut book, 23), source);
}

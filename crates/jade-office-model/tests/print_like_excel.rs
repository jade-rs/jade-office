//! Printing and PDF export as Excel's defaults print: the print area or
//! the cells in use, A4 portrait at 100% with Normal margins, pages cut
//! where columns and rows stop fitting and at the sheet's page breaks,
//! numbered down then over; the header and footer with their fields; a PDF
//! of those pages from either backend.

#![cfg(all(feature = "tiny-skia", feature = "skia"))]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::print::{sections, Page, PrintFields, Section};
use jade_office_model::sheet::{span, CellRef, Workbook};
use jade_office_model::skia::SkiaBackend;
use jade_office_model::tiny::TinySkiaBackend;

fn faces() -> Vec<jade_office_model::Face> {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let directory = core.join("fonts/Noto_Sans_JP/static");
    aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        None,
    )
}

fn fixture(name: &str) -> Vec<u8> {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    std::fs::read(core.join("public/fixtures").join(name)).unwrap()
}

fn tiny(name: &str) -> Workbook<TinySkiaBackend> {
    Workbook::open(
        &fixture(name),
        "UTC",
        TinySkiaBackend::new(&faces()).unwrap(),
    )
    .unwrap()
}

fn at(column: u32, row: u32) -> CellRef {
    CellRef::new(column, row)
}

/// How many pages a PDF says it has, written either way Skia and the
/// picture writer write it.
fn page_count(pdf: &[u8]) -> usize {
    let text = String::from_utf8_lossy(pdf).replace("/Type /", "/Type/");
    text.matches("/Type/Page").count() - text.matches("/Type/Pages").count()
}

#[test]
fn a_small_sheet_prints_on_one_page_from_its_first_cell_to_its_last() {
    let book = tiny("grid-operations-workbook.xlsx");
    assert_eq!(book.print_area(), Some(span(at(0, 0), at(4, 3))));
    let pages = book.print_pages().unwrap();
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0].range, span(at(0, 0), at(4, 3)));
}

#[test]
fn a_wide_and_long_sheet_breaks_into_pages_down_then_over() {
    let mut book = tiny("grid-operations-workbook.xlsx");
    // Past what an A4 page holds either way: 64px columns, 20px rows.
    book.set_cell(at(14, 79), "far").unwrap();
    let pages = book.print_pages().unwrap();
    assert!(pages.len() >= 4, "{pages:?}");
    let first = pages[0].range;
    assert_eq!(first.start, at(0, 0));
    assert!(first.end.column < 14 && first.end.row < 79);
    // Down first: the second page carries on below the first.
    assert_eq!(pages[1].range.start, at(0, first.end.row + 1));
    // Every cell prints once.
    let cells: u32 = pages
        .iter()
        .map(|page| {
            (page.range.end.column - page.range.start.column + 1)
                * (page.range.end.row - page.range.start.row + 1)
        })
        .sum();
    assert_eq!(cells, 15 * 80);
}

#[test]
fn the_print_area_and_a_manual_page_break_decide_the_pages() {
    // objects-workbook.xlsx: Print_Area Data!$A$1:$C$5, a break above row 4,
    // "Page &P of &N" on the right of the footer.
    let book = tiny("objects-workbook.xlsx");
    assert_eq!(book.print_area(), Some(span(at(0, 0), at(2, 4))));
    let pages: Vec<_> = book
        .print_pages()
        .unwrap()
        .into_iter()
        .map(|page| page.range)
        .collect();
    assert_eq!(
        pages,
        vec![span(at(0, 0), at(2, 2)), span(at(0, 3), at(2, 4))]
    );
    assert_eq!(page_count(&book.pdf(&PrintFields::default()).unwrap()), 2);
}

#[test]
fn the_pdf_has_a_page_for_each_printed_page_in_either_backend() {
    let mut book = tiny("grid-operations-workbook.xlsx");
    book.set_cell(at(0, 79), "below").unwrap();
    let pages = book.print_pages().unwrap().len();
    assert_eq!(pages, 2);
    let fields = PrintFields::default();
    let pdf = book.pdf(&fields).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    assert_eq!(page_count(&pdf), 2);
    assert!(String::from_utf8_lossy(&pdf).contains("/MediaBox [0 0 595.28 841.89]"));
    let native = Workbook::open(
        &fixture("grid-operations-workbook.xlsx"),
        "UTC",
        SkiaBackend::new(&faces()).unwrap(),
    )
    .unwrap();
    let pdf = native.pdf(&fields).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    assert_eq!(page_count(&pdf), 1);
}

#[test]
fn an_empty_sheet_has_nothing_to_print() {
    let mut book = tiny("grid-operations-workbook.xlsx");
    let sheet = book.add_sheet().unwrap();
    book.select_sheet(sheet).unwrap();
    assert_eq!(book.print_area(), None);
    assert!(book.print_pages().unwrap().is_empty());
    assert!(book.pdf(&PrintFields::default()).is_err());
}

#[test]
fn header_and_footer_codes_read_as_excel_prints_them() {
    let fields = PrintFields {
        date: "2026/10/20".into(),
        time: "9:05".into(),
        file: "book.xlsx".into(),
    };
    let page = Page {
        number: 2,
        total: 5,
        sheet: "売上",
        fields: &fields,
    };
    assert_eq!(
        sections("&L&\"Arial,Bold\"&14&A&C&P / &N ページ&R&D &T", &page),
        vec![
            (Section::Left, "売上".to_string()),
            (Section::Center, "2 / 5 ページ".to_string()),
            (Section::Right, "2026/10/20 9:05".to_string()),
        ]
    );
    assert_eq!(
        sections("Page &P of &N", &page),
        vec![(Section::Center, "Page 2 of 5".to_string())]
    );
    assert_eq!(
        sections("&F && &KFF0000red&B", &page),
        vec![(Section::Center, "book.xlsx & red".to_string())]
    );
}

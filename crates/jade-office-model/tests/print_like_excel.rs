//! Printing and PDF export as Excel prints: the print area or the cells in
//! use, on the sheet's paper and orientation with its margins and scale or
//! Fit to pages (A4 portrait at 100% with Normal margins by default), pages
//! cut where columns and rows stop fitting and at the sheet's page breaks,
//! numbered down then over unless the sheet says over then down, the print
//! titles on every page; the header and footer with their fields; a PDF of
//! those pages from either backend.

#![cfg(all(feature = "tiny-skia", feature = "skia"))]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::print::{paper_size, sections, Page, PrintFields, Section, A4};
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

/// page-setup-workbook.xlsx with sheet `index` active. Its cells are the
/// default 64px wide and 20px tall; Normal margins are 0.7" (67.2px) at the
/// sides and 0.75" (72px) at the top and bottom.
fn page_setup_sheet(index: usize) -> Workbook<TinySkiaBackend> {
    let mut book = tiny("page-setup-workbook.xlsx");
    book.select_sheet(index).unwrap();
    book
}

fn ranges(book: &Workbook<TinySkiaBackend>) -> Vec<jade_office_model::sheet::CellRange> {
    book.print_pages()
        .unwrap()
        .into_iter()
        .map(|page| page.range)
        .collect()
}

#[test]
fn letter_landscape_with_narrow_margins_lays_out_a_wider_page() {
    // Letter turned is 1056 × 816px; 0.25" sides and 0.75" top and bottom
    // leave 1008 × 672: 15 columns by 33 rows of the 30 × 60 used.
    let book = page_setup_sheet(0);
    let format = book.page_format().unwrap();
    assert_eq!(format.paper, (1056.0, 816.0));
    assert_eq!((format.margins.left, format.margins.top), (24.0, 72.0));
    assert_eq!(format.scale, 1.0);
    assert_eq!(
        ranges(&book),
        vec![
            span(at(0, 0), at(14, 32)),
            span(at(0, 33), at(14, 59)),
            span(at(15, 0), at(29, 32)),
            span(at(15, 33), at(29, 59)),
        ]
    );
    let pdf = book.pdf(&PrintFields::default()).unwrap();
    assert_eq!(page_count(&pdf), 4);
    assert!(String::from_utf8_lossy(&pdf).contains("/MediaBox [0 0 792.00 612.00]"));
}

#[test]
fn paper_codes_are_excels() {
    assert_eq!(paper_size(1), (816.0, 1056.0));
    assert_eq!(paper_size(9), A4);
    assert_eq!(paper_size(5), (816.0, 1344.0));
    let a3 = paper_size(8);
    assert!((a3.0 - A4.1).abs() < 0.01 && (a3.1 - 420.0 / 25.4 * 96.0).abs() < 0.01);
    assert_eq!(paper_size(9999), A4);
}

#[test]
fn half_scale_fits_twice_as_much_and_numbers_over_then_down() {
    // A4 at 50% with Normal margins: 1318.6 × 1957px of cells, 20 columns
    // by 97 rows of the 40 × 200 used, numbered across first.
    let book = page_setup_sheet(1);
    let format = book.page_format().unwrap();
    assert_eq!(format.paper, A4);
    assert_eq!(format.scale, 0.5);
    assert_eq!(
        ranges(&book),
        vec![
            span(at(0, 0), at(19, 96)),
            span(at(20, 0), at(39, 96)),
            span(at(0, 97), at(19, 193)),
            span(at(20, 97), at(39, 193)),
            span(at(0, 194), at(19, 199)),
            span(at(20, 194), at(39, 199)),
        ]
    );
    assert_eq!(page_count(&book.pdf(&PrintFields::default()).unwrap()), 6);
}

#[test]
fn fit_to_one_page_wide_shrinks_until_the_columns_fit() {
    // 30 columns are 1920px; 659.3px of A4 holds them at 34%, where a page
    // is 2878px tall: 143 rows.
    let book = page_setup_sheet(2);
    assert_eq!(book.page_format().unwrap().scale, 0.34);
    assert_eq!(
        ranges(&book),
        vec![
            span(at(0, 0), at(29, 142)),
            span(at(0, 143), at(29, 285)),
            span(at(0, 286), at(29, 299)),
        ]
    );
}

#[test]
fn fit_to_one_page_shrinks_until_everything_fits() {
    // 300 rows are 6000px; 978.5px of A4 holds them at 16%.
    let book = page_setup_sheet(3);
    assert_eq!(book.page_format().unwrap().scale, 0.16);
    assert_eq!(ranges(&book), vec![span(at(0, 0), at(29, 299))]);
    assert_eq!(page_count(&book.pdf(&PrintFields::default()).unwrap()), 1);
}

#[test]
fn print_titles_repeat_on_every_page_after_the_first() {
    // Rows 1–2 and column A repeat. The first page holds them as its own
    // cells: 10 columns and 48 rows. The rest give them their room: 9
    // columns beside column A, 46 rows below rows 1–2.
    let book = page_setup_sheet(4);
    let pages = book.print_pages().unwrap();
    let laid: Vec<_> = pages
        .iter()
        .map(|page| (page.range, page.title_columns, page.title_rows))
        .collect();
    let rows = Some((0, 1));
    let column = Some((0, 0));
    assert_eq!(
        laid,
        vec![
            (span(at(0, 0), at(9, 47)), None, None),
            (span(at(0, 48), at(9, 93)), None, rows),
            (span(at(0, 94), at(9, 99)), None, rows),
            (span(at(10, 0), at(14, 47)), column, None),
            (span(at(10, 48), at(14, 93)), column, rows),
            (span(at(10, 94), at(14, 99)), column, rows),
        ]
    );
    assert_eq!(page_count(&book.pdf(&PrintFields::default()).unwrap()), 6);
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

/// Where page 1's cells start and how many lines it strokes, as painted.
fn first_page(book: &Workbook<TinySkiaBackend>) -> ((f64, f64), usize) {
    use jade_painter::recording::{Op, RecordingPainter};
    let pages = book
        .paint_pages(
            &PrintFields::default(),
            |_, _| Some(RecordingPainter::new()),
        )
        .unwrap();
    let ops = pages[0].ops();
    let origin = ops
        .iter()
        .find_map(|op| match op {
            Op::Translate(x, y) => Some((*x, *y)),
            _ => None,
        })
        .expect("the page moves to its margins");
    let lines = ops.iter().filter(|op| matches!(op, Op::LineTo(..))).count();
    (origin, lines)
}

#[test]
fn centring_on_the_page_and_gridlines_follow_page_setup() {
    let mut book = tiny("grid-operations-workbook.xlsx");
    let ((left, top), lines) = first_page(&book);
    let format = book.page_format().unwrap();
    assert_eq!((left, top), (format.margins.left, format.margins.top));
    let mut setup = book.page_setup().clone();
    setup.center_horizontally = true;
    setup.center_vertically = true;
    setup.print_grid_lines = true;
    assert!(book.set_page_setup(&setup).unwrap());
    let ((centred_left, centred_top), gridded) = first_page(&book);
    // The cells are narrower and shorter than the room inside the margins.
    assert!(centred_left > left && centred_top > top);
    assert!(gridded > lines);
}

#[test]
fn headings_print_round_the_cells_and_take_their_room() {
    use jade_painter::recording::{Op, RecordingPainter};
    let mut book = tiny("grid-operations-workbook.xlsx");
    let labels = |book: &Workbook<TinySkiaBackend>| {
        let pages = book
            .paint_pages(
                &PrintFields::default(),
                |_, _| Some(RecordingPainter::new()),
            )
            .unwrap();
        pages[0].count(|op| matches!(op, Op::FillText(text, _, _) if text == "A" || text == "1"))
    };
    let plain = book.page_format().unwrap().room();
    assert_eq!(labels(&book), 0);
    let mut setup = book.page_setup().clone();
    setup.print_headings = true;
    assert!(book.set_page_setup(&setup).unwrap());
    let format = book.page_format().unwrap();
    assert!(format.headings.0 > 0.0 && format.headings.1 > 0.0);
    assert_eq!(format.room().0, plain.0 - format.headings.0);
    assert_eq!(labels(&book), 2);
}

/// Page Break Preview: the print area, its pages in order and the manual
/// break inside it, for drawing over the grid; none for an empty sheet.
#[test]
fn page_break_preview_shows_the_print_area_its_pages_and_manual_breaks() {
    let book = tiny("objects-workbook.xlsx");
    let view = book.page_break_view().expect("something prints");
    assert_eq!(view.area, span(at(0, 0), at(2, 4)));
    assert_eq!(
        view.pages,
        vec![span(at(0, 0), at(2, 2)), span(at(0, 3), at(2, 4))]
    );
    assert_eq!(view.manual_rows, vec![3]);
    assert!(view.manual_columns.is_empty());

    let mut empty = tiny("grid-operations-workbook.xlsx");
    let sheets = empty.sheet_names().len();
    empty.add_sheet().unwrap();
    assert_eq!(empty.sheet_names().len(), sheets + 1);
    assert!(empty.page_break_view().is_none());
}

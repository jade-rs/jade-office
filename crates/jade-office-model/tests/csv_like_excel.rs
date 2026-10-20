//! CSV as Excel opens and saves it: fields read as typed (numbers, dates,
//! percentages, formulas), quoted fields kept whole, UTF-8 or Shift_JIS in;
//! the active sheet out as it shows, quoted where needed, CR LF, UTF-8
//! with a BOM; Save As by the name's extension.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::csv::{decode, is_csv, sheet_name};
use jade_office_model::sheet::{CellRef, Workbook};
use jade_office_model::tiny::TinySkiaBackend;

fn backend() -> TinySkiaBackend {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let directory = core.join("fonts/Noto_Sans_JP/static");
    TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        None,
    ))
    .unwrap()
}

fn at(column: u32, row: u32) -> CellRef {
    CellRef::new(column, row)
}

const SALES: &str = "\u{feff}品名,数量,単価,日付,率,合計\r\n\"りんご, 青森\",007,\"1,200\",2024/1/15,10%,=B2*C2\r\n\"二行の\n説明\",3,450,2024/2/1,5%,=B3*C3\r\n";

#[test]
fn a_csv_opens_as_a_sheet_named_for_the_file_read_as_typed() {
    let book = Workbook::open_csv("売上 2024.csv", SALES.as_bytes(), "UTC", backend()).unwrap();
    assert_eq!(book.sheet_names(), vec!["売上 2024"]);
    assert_eq!(
        book.display_text(at(0, 0)),
        "品名",
        "the BOM is not part of it"
    );
    assert_eq!(book.display_text(at(0, 1)), "りんご, 青森");
    assert_eq!(book.display_text(at(0, 2)), "二行の\n説明");
    assert_eq!(
        book.number(at(1, 1)),
        Some(7.0),
        "007 reads as 7, as in Excel"
    );
    assert_eq!(book.number(at(2, 1)), Some(1200.0));
    assert_eq!(
        book.display_text(at(3, 1)),
        "2024/1/15",
        "a date shows as one"
    );
    assert_eq!(book.number(at(4, 1)), Some(0.1));
    assert_eq!(book.display_text(at(4, 1)), "10%");
    assert_eq!(book.edit_text(at(5, 1)), "=B2*C2");
    assert_eq!(book.number(at(5, 1)), Some(8400.0));
    assert!(!book.is_dirty());
}

#[test]
fn nothing_to_undo_after_opening_a_csv() {
    let mut book = Workbook::open_csv("a.csv", b"1,2\r\n", "UTC", backend()).unwrap();
    assert!(!book.undo().unwrap());
    assert_eq!(book.number(at(1, 0)), Some(2.0));
}

#[test]
fn shift_jis_from_a_japanese_excel_reads_as_japanese() {
    let (bytes, _, _) = encoding_rs::SHIFT_JIS.encode("氏名,年齢\r\n山田太郎,42\r\n");
    assert!(std::str::from_utf8(&bytes).is_err());
    assert_eq!(decode(&bytes), "氏名,年齢\r\n山田太郎,42\r\n");
    let book = Workbook::open_csv("名簿.csv", &bytes, "UTC", backend()).unwrap();
    assert_eq!(book.display_text(at(0, 1)), "山田太郎");
    assert_eq!(book.number(at(1, 1)), Some(42.0));
}

#[test]
fn the_active_sheet_saves_as_it_shows_quoted_and_rectangular() {
    let mut book = Workbook::open_csv("a.csv", SALES.as_bytes(), "UTC", backend()).unwrap();
    book.set_cell(at(7, 3), "end").unwrap();
    let text = book.csv_text();
    let lines: Vec<&str> = text.split("\r\n").collect();
    assert_eq!(lines[0], "品名,数量,単価,日付,率,合計,,");
    // 1,200 was read with its thousands separator and shows with it, as
    // Excel writes it back.
    assert_eq!(
        lines[1],
        "\"りんご, 青森\",7,\"1,200\",2024/1/15,10%,8400,,"
    );
    assert_eq!(lines[2], "\"二行の\n説明\",3,450,2024/2/1,5%,1350,,");
    assert_eq!(lines[3], ",,,,,,,end");
    assert_eq!(lines[4], "", "every row ends in CR LF");
    let bytes = book.save_bytes_as("out.CSV").unwrap();
    assert!(
        bytes.starts_with("\u{feff}品名".as_bytes()),
        "UTF-8 with a BOM"
    );
    assert!(!book.is_dirty());
    // Opened again, it reads the same.
    let again = Workbook::open_csv("out.csv", &bytes, "UTC", backend()).unwrap();
    assert_eq!(again.display_text(at(0, 2)), "二行の\n説明");
    assert_eq!(
        again.number(at(5, 1)),
        Some(8400.0),
        "the result, not the formula"
    );
}

#[test]
fn save_as_writes_by_the_name_and_csv_holds_only_the_active_sheet() {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let bytes = std::fs::read(core.join("public/fixtures/grid-operations-workbook.xlsx")).unwrap();
    let mut book = Workbook::open(&bytes, "UTC", backend()).unwrap();
    let other = book.add_sheet().unwrap();
    book.set_cell(at(0, 0), "elsewhere").unwrap();
    book.select_sheet(0).unwrap();
    let csv = String::from_utf8(book.save_bytes_as("book.csv").unwrap()).unwrap();
    assert!(csv.starts_with("\u{feff}Name,Amount,Double"));
    assert!(csv.contains("Alpha,10,20"));
    assert!(!csv.contains("elsewhere"));
    let xlsx = book.save_bytes_as("book.xlsx").unwrap();
    assert!(xlsx.starts_with(b"PK"), "a zip package");
    let reopened = Workbook::open(&xlsx, "UTC", backend()).unwrap();
    assert_eq!(reopened.sheet_names().len(), other + 1);
    assert_eq!(
        Workbook::open_csv("e.csv", b"", "UTC", backend())
            .unwrap()
            .csv_text(),
        ""
    );
}

#[test]
fn names_and_extensions() {
    assert!(is_csv("a.csv") && is_csv("B.CSV") && !is_csv("a.xlsx") && !is_csv("csv"));
    assert_eq!(sheet_name("/tmp/report.csv"), "report");
    assert_eq!(sheet_name("a[1]:b?.csv"), "a1b");
    assert_eq!(sheet_name(".csv"), "Sheet1");
    assert_eq!(sheet_name(&format!("{}.csv", "x".repeat(40))).len(), 31);
}

//! CSV as Excel opens and saves it: fields read as typed (numbers, dates,
//! percentages, formulas), quoted fields kept whole, UTF-8 or Shift_JIS in;
//! the active sheet out as it shows, quoted where needed, CR LF, UTF-8
//! with a BOM; Save As by the name's extension.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::csv::{decode, is_csv, separator, sheet_name};
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

#[test]
fn explicit_separator_metadata_is_not_imported_as_a_data_row() {
    for ending in ["\r\n", "\n", "\r"] {
        let text = format!("\u{feff}sep=|{ending}Name|Amount{ending}\"a|b\"|12{ending}");
        assert_eq!(separator("data.csv", &text), '|');
        let book = Workbook::open_csv("data.csv", text.as_bytes(), "UTC", backend()).unwrap();
        assert_eq!(book.display_text(at(0, 0)), "Name");
        assert_eq!(book.display_text(at(0, 1)), "a|b");
        assert_eq!(book.number(at(1, 1)), Some(12.0));
        assert_eq!(book.csv_text(), "Name,Amount\r\na|b,12\r\n");
    }
    assert_eq!(separator("data.csv", "SEP=;\r\nx;y"), ';');
    assert_eq!(separator("data.tsv", "sep=|\r\nx\ty"), '\t');
    let quoted = Workbook::open_csv("data.csv", b"\"sep=|\",data\r\n", "UTC", backend()).unwrap();
    assert_eq!(quoted.display_text(at(0, 0)), "sep=|");
    assert_eq!(quoted.display_text(at(1, 0)), "data");
}

#[test]
fn signed_grouped_numbers_and_fractional_percentages_import_as_numbers() {
    let book = Workbook::open_csv(
        "numbers.csv",
        b"\"+1,234\",.5%,+$1.50,$.50\r\n",
        "UTC",
        backend(),
    )
    .unwrap();
    for (column, value) in [(0, 1234.0), (1, 0.005), (2, 1.5), (3, 0.5)] {
        assert_eq!(book.number(at(column, 0)), Some(value));
    }
    assert_eq!(book.display_text(at(1, 0)), "0.50%");
    assert_eq!(book.display_text(at(2, 0)), "$1.50");
    assert_eq!(book.display_text(at(3, 0)), "$0.50");
}

#[test]
fn am_pm_times_import_as_numeric_values_with_their_display_format() {
    let mut book = Workbook::open_csv(
        "times.csv",
        b"1:30 PM,12:00 AM,12:00 PM,2024/1/15 1:30 PM\r\n",
        "UTC",
        backend(),
    )
    .unwrap();
    for (column, value) in [(0, 0.5625), (1, 0.0), (2, 0.5), (3, 45306.5625)] {
        assert_eq!(book.number(at(column, 0)), Some(value));
    }
    assert_eq!(book.display_text(at(0, 0)), "1:30 PM");
    assert_eq!(book.display_text(at(1, 0)), "12:00 AM");
    assert_eq!(book.display_text(at(2, 0)), "12:00 PM");
    let saved = book.save_bytes_as("times.csv").unwrap();
    let reopened = Workbook::open_csv("times.csv", &saved, "UTC", backend()).unwrap();
    assert_eq!(reopened.number(at(3, 0)), Some(45306.5625));
}

#[test]
fn abbreviated_meridiem_times_support_entry_and_saved_xlsx_values() {
    let mut book = Workbook::open_csv(
        "times.csv",
        b"9:00 p,12:00 a,12:00 P,2024/1/15 1:30 p\r\n",
        "UTC",
        backend(),
    )
    .unwrap();
    for (column, value) in [(0, 0.875), (1, 0.0), (2, 0.5), (3, 45306.5625)] {
        assert_eq!(book.number(at(column, 0)), Some(value));
    }
    book.set_cell(at(0, 1), "9:00 a").unwrap();
    book.set_cell(at(1, 1), "=A1-A2").unwrap();
    assert_eq!(book.number(at(1, 1)), Some(0.5));
    assert_eq!(book.display_text(at(0, 0)), "9:00 PM");
    let saved = book.save_bytes_as("times.xlsx").unwrap();
    let reopened = Workbook::open(&saved, "UTC", backend()).unwrap();
    assert_eq!(reopened.number(at(0, 0)), Some(0.875));
    assert_eq!(reopened.number(at(1, 1)), Some(0.5));
}

#[test]
fn csv_error_literals_are_errors_while_apostrophe_prefixed_values_stay_text() {
    let mut book = Workbook::open_csv("errors.csv", b"#N/A,'#N/A\r\n", "UTC", backend()).unwrap();
    book.set_cell(at(0, 1), "=ISNA(A1)").unwrap();
    book.set_cell(at(1, 1), "=ISNA(B1)").unwrap();
    book.set_cell(at(2, 1), "=ISFORMULA(A1)").unwrap();
    assert_eq!(book.display_text(at(0, 1)), "TRUE");
    assert_eq!(book.display_text(at(1, 1)), "FALSE");
    assert_eq!(book.display_text(at(2, 1)), "FALSE");
    assert_eq!(book.edit_text(at(0, 0)), "#N/A");
    let saved = book.save_bytes_as("errors.xlsx").unwrap();
    let reopened = Workbook::open(&saved, "UTC", backend()).unwrap();
    assert_eq!(reopened.display_text(at(0, 1)), "TRUE");
    assert_eq!(reopened.display_text(at(1, 1)), "FALSE");
    assert_eq!(reopened.display_text(at(2, 1)), "FALSE");
}

/// A `.tsv` splits at tabs, a European CSV at its semicolons, and UTF-16
/// *Unicode Text* reads by its BOM; a `.tsv` saves tab-separated.
#[test]
fn tabs_semicolons_and_utf16_read_as_excel_reads_them() {
    assert_eq!(decode(b"\xef\xbb\xbfName,\xff\r\n"), "Name,\u{fffd}\r\n");
    assert_eq!(separator("a.tsv", "x,y\tz"), '\t');
    assert_eq!(separator("a.csv", "x;y;\"1,5\"\n1,2"), ';');
    assert_eq!(separator("a.csv", "x;y,z"), ',');
    assert_eq!(separator("a.csv", "x\ty"), '\t');
    assert_eq!(separator("a.csv", "\"a,b\";c"), ';');
    assert!(is_csv("list.TSV"));

    let book = Workbook::open_csv(
        "eu.csv",
        "Name;Price\r\nTee;12\r\n".as_bytes(),
        "UTC",
        backend(),
    )
    .unwrap();
    assert_eq!(book.display_text(at(1, 1)), "12");

    let mut utf16 = vec![0xFF, 0xFE];
    for unit in "品名\t数量\r\nりんご\t3\r\n".encode_utf16() {
        utf16.extend(unit.to_le_bytes());
    }
    assert_eq!(decode(&utf16), "品名\t数量\r\nりんご\t3\r\n");
    let mut book = Workbook::open_csv("fruit.tsv", &utf16, "UTC", backend()).unwrap();
    assert_eq!(book.display_text(at(0, 1)), "りんご");
    assert_eq!(book.display_text(at(1, 1)), "3");
    let saved = book.save_bytes_as("fruit.tsv").unwrap();
    assert_eq!(
        String::from_utf8(saved).unwrap(),
        "\u{feff}品名\t数量\r\nりんご\t3\r\n"
    );
}

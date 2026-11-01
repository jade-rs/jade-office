//! A filter button's Top 10 and Reapply, as Excel's do: only the rows
//! with the largest (or smallest) numbers showing, and the filter run
//! again over values changed since.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::grid::Direction;
use jade_office_model::sheet::{span, CellRef, FilterButton, FilterButtonTarget, Workbook};
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

/// The rows showing below the header, walking down with the arrow.
fn showing(book: &Workbook<TinySkiaBackend>, header: CellRef, last: u32) -> Vec<String> {
    let mut rows = Vec::new();
    let mut at = header;
    loop {
        let next = book.step(at, Direction::Down);
        if next == at || next.row > last {
            return rows;
        }
        rows.push(book.display_text(next));
        at = next;
    }
}

#[test]
fn top_and_bottom_show_the_largest_and_smallest_and_reapply_follows_edits() {
    let mut book = book();
    let header = CellRef::new(20, 99);
    book.set_cell(header, "Score").unwrap();
    let scores = ["40", "10", "70", "30", "90", "20"];
    for (offset, score) in scores.iter().enumerate() {
        book.set_cell(CellRef::new(20, 100 + offset as u32), score)
            .unwrap();
    }
    let range = span(header, CellRef::new(20, 105));
    book.toggle_filter(range).unwrap();
    let button = FilterButton {
        target: FilterButtonTarget::Worksheet,
        column: 0,
        reference: header,
        range,
        active: false,
        rect: jade_xlsx_layout::Rect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        },
    };

    assert!(book.filter_top(&button, 2, true).unwrap());
    assert_eq!(showing(&book, header, 105), ["70", "90"]);
    assert!(book.filter_top(&button, 2, false).unwrap());
    assert_eq!(showing(&book, header, 105), ["10", "20"]);

    // 10 becomes 95: the bottom two are 20 and 30 once reapplied.
    book.set_cell(CellRef::new(20, 101), "95").unwrap();
    book.reapply_filter(&button).unwrap();
    assert_eq!(showing(&book, header, 105), ["30", "20"]);
}

/// Custom Filter: greater than a number, or text that begins with or
/// contains what is typed; nothing typed filters nothing.
#[test]
fn custom_filters_compare_with_the_value_typed() {
    use jade_office_model::custom_filter::{CustomFilterDialog, CustomFilterKey, FilterTest};
    let mut book = book();
    let header = CellRef::new(20, 99);
    book.set_cell(header, "Item").unwrap();
    let items = ["apple", "banana", "pineapple", "cherry", "apricot"];
    for (offset, item) in items.iter().enumerate() {
        book.set_cell(CellRef::new(20, 100 + offset as u32), item)
            .unwrap();
        book.set_cell(
            CellRef::new(21, 100 + offset as u32),
            &((offset + 1) * 10).to_string(),
        )
        .unwrap();
    }
    book.set_cell(CellRef::new(21, 99), "Qty").unwrap();
    let range = span(header, CellRef::new(21, 104));
    book.toggle_filter(range).unwrap();
    let rect = jade_xlsx_layout::Rect {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };
    let button = |column| FilterButton {
        target: FilterButtonTarget::Worksheet,
        column,
        reference: CellRef::new(20 + column, 99),
        range,
        active: false,
        rect,
    };

    assert!(book
        .filter_custom(&button(0), FilterTest::BeginsWith, "ap")
        .unwrap());
    assert_eq!(showing(&book, header, 104), ["apple", "apricot"]);
    assert!(book
        .filter_custom(&button(0), FilterTest::Contains, "apple")
        .unwrap());
    assert_eq!(showing(&book, header, 104), ["apple", "pineapple"]);
    book.clear_filter(&button(0)).unwrap();
    assert!(book
        .filter_custom(&button(1), FilterTest::Greater, "25")
        .unwrap());
    assert_eq!(
        showing(&book, header, 104),
        ["pineapple", "cherry", "apricot"]
    );
    assert!(!book
        .filter_custom(&button(1), FilterTest::Less, "  ")
        .unwrap());

    // The dialog types its value; Enter takes it.
    let mut dialog = CustomFilterDialog::new(button(0));
    dialog.value.insert("ch");
    assert_eq!(dialog.key("backspace", false), CustomFilterKey::Handled);
    assert_eq!(dialog.value.text, "c");
    assert_eq!(dialog.key("enter", false), CustomFilterKey::Take);
}

/// Two conditions: And keeps the rows that pass both, Or those that pass
/// either; Tab types into the second value.
#[test]
fn two_conditions_join_by_and_or_or() {
    use jade_office_model::custom_filter::{CustomFilterDialog, FilterTest};
    let mut book = book();
    let header = CellRef::new(20, 99);
    book.set_cell(header, "Qty").unwrap();
    for (offset, qty) in ["5", "15", "25", "35"].iter().enumerate() {
        book.set_cell(CellRef::new(20, 100 + offset as u32), qty)
            .unwrap();
    }
    let range = span(header, CellRef::new(20, 103));
    book.toggle_filter(range).unwrap();
    let button = FilterButton {
        target: FilterButtonTarget::Worksheet,
        column: 0,
        reference: header,
        range,
        active: false,
        rect: jade_xlsx_layout::Rect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        },
    };
    let mut dialog = CustomFilterDialog::new(button);
    dialog.test = FilterTest::Greater;
    dialog.value.insert("10");
    dialog.key("tab", false);
    dialog.second_test = FilterTest::Less;
    dialog.field_mut().insert("30");
    assert_eq!(dialog.second.text, "30");
    assert!(dialog.apply(&mut book).unwrap());
    assert_eq!(showing(&book, header, 103), ["15", "25"]);
    dialog.and = false;
    dialog.test = FilterTest::Less;
    dialog.second_test = FilterTest::Greater;
    assert!(dialog.apply(&mut book).unwrap());
    assert_eq!(showing(&book, header, 103), ["5", "35"]);
    // A second condition left empty is left out.
    dialog.second.text.clear();
    assert!(dialog.apply(&mut book).unwrap());
    assert_eq!(showing(&book, header, 103), ["5"]);
}

/// Top 10's dialog: Bottom 2 items, or the top half by percent.
#[test]
fn the_top_ten_dialog_picks_how_many_and_items_or_percent() {
    use jade_office_model::top_ten::{TopTenDialog, TopTenKey};
    let mut book = book();
    let header = CellRef::new(20, 99);
    book.set_cell(header, "Score").unwrap();
    for (offset, score) in ["40", "10", "70", "30"].iter().enumerate() {
        book.set_cell(CellRef::new(20, 100 + offset as u32), score)
            .unwrap();
    }
    let range = span(header, CellRef::new(20, 103));
    book.toggle_filter(range).unwrap();
    let button = FilterButton {
        target: FilterButtonTarget::Worksheet,
        column: 0,
        reference: header,
        range,
        active: false,
        rect: jade_xlsx_layout::Rect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        },
    };
    let mut dialog = TopTenDialog::new(button);
    assert_eq!(
        (dialog.top, dialog.count, dialog.percent),
        (true, 10, false)
    );
    dialog.top = false;
    dialog.step(-8);
    assert_eq!(dialog.key("enter"), TopTenKey::Take);
    assert!(dialog.apply(&mut book).unwrap());
    assert_eq!(showing(&book, header, 103), ["10", "30"]);
    dialog.top = true;
    dialog.set_percent(true);
    dialog.step(48);
    assert_eq!(dialog.count, 50);
    assert!(dialog.apply(&mut book).unwrap());
    assert_eq!(showing(&book, header, 103), ["40", "70"]);
    // Held to Excel's bounds.
    dialog.step(1000);
    assert_eq!(dialog.count, 100);
    dialog.step(-1000);
    assert_eq!(dialog.count, 1);
}

/// The date filter lists the years and months a date column falls in, and
/// shows only the rows of the one picked.
#[test]
fn the_date_filter_groups_by_year_and_month() {
    let mut book = book();
    let header = CellRef::new(20, 99);
    book.set_cell(header, "When").unwrap();
    for (offset, date) in ["2024/1/15", "2024/3/2", "2023/12/31", "2024/3/20"]
        .iter()
        .enumerate()
    {
        book.set_cell(CellRef::new(20, 100 + offset as u32), date)
            .unwrap();
    }
    let range = span(header, CellRef::new(20, 103));
    book.toggle_filter(range).unwrap();
    let button = FilterButton {
        target: FilterButtonTarget::Worksheet,
        column: 0,
        reference: header,
        range,
        active: false,
        rect: jade_xlsx_layout::Rect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        },
    };
    assert_eq!(
        book.filter_date_groups(&button),
        vec![(2023, vec![12]), (2024, vec![1, 3])]
    );
    assert!(book.filter_date(&button, 2024, Some(3)).unwrap());
    assert_eq!(showing(&book, header, 103).len(), 2);
    assert!(book.filter_date(&button, 2023, None).unwrap());
    assert_eq!(showing(&book, header, 103).len(), 1);
}

/// Above/Below Average and Date Filters ▸ This Month, worked out when
/// applied.
#[test]
fn dynamic_filters_show_rows_against_the_average_or_today() {
    let mut book = book();
    let header = CellRef::new(20, 99);
    book.set_cell(header, "Score").unwrap();
    for (offset, score) in ["40", "10", "70"].iter().enumerate() {
        book.set_cell(CellRef::new(20, 100 + offset as u32), score)
            .unwrap();
    }
    let range = span(header, CellRef::new(20, 102));
    book.toggle_filter(range).unwrap();
    let button = FilterButton {
        target: FilterButtonTarget::Worksheet,
        column: 0,
        reference: header,
        range,
        active: false,
        rect: jade_xlsx_layout::Rect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        },
    };
    // The average is 40.
    assert!(book.filter_dynamic(&button, "aboveAverage").unwrap());
    assert_eq!(showing(&book, header, 102), ["70"]);
    assert!(book.filter_dynamic(&button, "belowAverage").unwrap());
    assert_eq!(showing(&book, header, 102), ["10"]);
    // Numbers that are no dates of this month: none show.
    assert!(book.filter_dynamic(&button, "thisMonth").unwrap());
    assert!(showing(&book, header, 102).is_empty());
    assert!(book.filter_dynamic(&button, "someday").is_err());
}

/// A day of a month ticked shows only that day's rows.
#[test]
fn the_date_filter_ticks_single_days() {
    let mut book = book();
    let header = CellRef::new(20, 99);
    book.set_cell(header, "When").unwrap();
    for (offset, date) in ["2024/1/15", "2024/1/20", "2024/2/1"].iter().enumerate() {
        book.set_cell(CellRef::new(20, 100 + offset as u32), date)
            .unwrap();
    }
    let range = span(header, CellRef::new(20, 102));
    book.toggle_filter(range).unwrap();
    let button = FilterButton {
        target: FilterButtonTarget::Worksheet,
        column: 0,
        reference: header,
        range,
        active: false,
        rect: jade_xlsx_layout::Rect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        },
    };
    assert_eq!(book.filter_date_days(&button, 2024, 1), vec![15, 20]);
    assert!(book
        .filter_dates(&button, &[(2024, Some(1), Some(20)), (2024, Some(2), None)])
        .unwrap());
    assert_eq!(showing(&book, header, 102).len(), 2);
    assert_eq!(
        book.step(header, jade_office_model::grid::Direction::Down),
        CellRef::new(20, 101)
    );
}

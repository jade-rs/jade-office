//! Home ▸ Conditional Formatting, as Excel has it: the highlight and
//! top/bottom rules ask for their value in a small dialog filled in from the
//! selection, data bars and colour scales apply at once, the Rules Manager
//! lists and deletes, and Clear Rules clears the selection or the sheet.
//! Each draws on the grid and is one undo step.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::{aliased_faces, Backend};
use jade_office_model::conditional_formatting::{
    describe, AskKey, ConditionalStyle, NewConditionalRule, Preset, RuleAsk, RuleDialog,
    COLOR_SCALES, DATA_BARS,
};
use jade_office_model::sheet::{protected_refusal, CellRange, CellRef, Overlay, Workbook};
use jade_office_model::tiny::TinySkiaBackend;

fn open(fixture: &str) -> Workbook<TinySkiaBackend> {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let directory = core.join("fonts/Noto_Sans_JP/static");
    let backend = TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        None,
    ))
    .unwrap();
    let bytes = std::fs::read(core.join("public/fixtures").join(fixture)).unwrap();
    Workbook::open(&bytes, "UTC", backend).unwrap()
}

/// Operations!B2:B4 hold 10, 20, 30.
fn book() -> Workbook<TinySkiaBackend> {
    open("grid-operations-workbook.xlsx")
}

fn range(start: (u32, u32), end: (u32, u32)) -> CellRange {
    CellRange {
        start: CellRef::new(start.0, start.1),
        end: CellRef::new(end.0, end.1),
    }
}

const AMOUNTS: ((u32, u32), (u32, u32)) = ((1, 1), (1, 3));

/// The colour the grid paints near the cell's top-left corner, past its
/// gridline, as `RRGGBB`.
fn painted(book: &Workbook<TinySkiaBackend>, at: CellRef) -> u32 {
    let painter = book
        .render_viewport((0.0, 0.0), 480, 200, 1.0, &Overlay::default())
        .unwrap();
    let png = TinySkiaBackend::png(&painter).unwrap();
    let decoder = png::Decoder::new(std::io::Cursor::new(png));
    let mut reader = decoder.read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut pixels).unwrap();
    let stride = info.color_type.samples();
    let (x, y, _, _) = book.cell_rect(at, (0.0, 0.0)).unwrap();
    let offset = ((y as usize + 3) * info.width as usize + x as usize + 3) * stride;
    u32::from_be_bytes([0, pixels[offset], pixels[offset + 1], pixels[offset + 2]])
}

const WHITE: u32 = 0xFFFFFF;
const LIGHT_RED: u32 = 0xFFC7CE;

#[test]
fn greater_than_asks_for_a_value_and_highlights_what_passes() {
    let mut book = book();
    let amounts = range(AMOUNTS.0, AMOUNTS.1);
    let (b2, b3) = (CellRef::new(1, 1), CellRef::new(1, 2));
    // Filled in from the selection, as Excel does: here its average.
    let mut dialog = RuleDialog::open(Preset::GreaterThan, &book, amounts, b2);
    assert_eq!(dialog.values, ["20"]);
    assert_eq!(dialog.style, ConditionalStyle::LightRedFillDarkRedText);
    dialog.values[0] = "15".into();
    let rule = dialog.rule().unwrap();
    assert!(book
        .add_conditional_format(amounts, &rule, dialog.style)
        .unwrap());
    assert_eq!(painted(&book, b2), WHITE);
    assert_eq!(painted(&book, b3), LIGHT_RED);
    assert!(book.is_dirty());
    // The Rules Manager lists it, in words the shells translate.
    let rules = book.conditional_formats();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].range, "B2:B4");
    let words = describe(&rules[0]);
    assert_eq!(words.key, "cf.describe.greater_than");
    assert_eq!(words.values, [("value", "15".to_string())]);
    // One undo step, and redo brings it back.
    assert!(book.undo().unwrap());
    assert_eq!(painted(&book, b3), WHITE);
    assert!(book.conditional_formats().is_empty());
    assert!(book.redo().unwrap());
    assert_eq!(painted(&book, b3), LIGHT_RED);
}

#[test]
fn every_dialog_fills_in_and_checks_its_values() {
    let book = book();
    let amounts = range(AMOUNTS.0, AMOUNTS.1);
    let b3 = CellRef::new(1, 2);
    let between = RuleDialog::open(Preset::Between, &book, amounts, b3);
    assert_eq!(between.values, ["10", "30"]);
    let equal = RuleDialog::open(Preset::EqualTo, &book, amounts, b3);
    assert_eq!(equal.values, ["20"]);
    let text = RuleDialog::open(
        Preset::TextContains,
        &book,
        range((0, 1), (0, 3)),
        CellRef::new(0, 3),
    );
    assert_eq!(text.values, ["Gamma"]);
    let mut top = RuleDialog::open(Preset::TopItems, &book, amounts, b3);
    assert_eq!(top.values, ["10"]);
    assert!(RuleDialog::open(Preset::AboveAverage, &book, amounts, b3)
        .values
        .is_empty());
    // A rank has to be a whole number from 1; a percent at most 100.
    top.values[0] = "0".into();
    assert_eq!(top.rule().unwrap_err(), "cf.invalid_rank");
    let mut percent = RuleDialog::open(Preset::TopPercent, &book, amounts, b3);
    percent.values[0] = "101".into();
    assert_eq!(percent.rule().unwrap_err(), "cf.invalid_rank");
    let mut empty = RuleDialog::open(Preset::GreaterThan, &book, amounts, b3);
    empty.values[0] = " ".into();
    assert_eq!(empty.rule().unwrap_err(), "cf.invalid_value");
}

#[test]
fn bars_scales_and_the_rules_manager() {
    let mut book = book();
    let amounts = range(AMOUNTS.0, AMOUNTS.1);
    let (b2, b4) = (CellRef::new(1, 1), CellRef::new(1, 3));
    // Green - Yellow - Red: the highest green, the lowest red.
    let scale = COLOR_SCALES[0];
    assert_eq!(scale.label_key, "cf.scale.green_yellow_red");
    book.add_conditional_format(amounts, &scale.rule(), ConditionalStyle::None)
        .unwrap();
    assert_eq!(painted(&book, b2), 0xF8696B);
    assert_eq!(painted(&book, b4), 0x63BE7B);
    book.add_conditional_format(
        range((2, 1), (2, 3)),
        &DATA_BARS[0].rule(),
        ConditionalStyle::None,
    )
    .unwrap();
    let rules = book.conditional_formats();
    assert_eq!(
        rules
            .iter()
            .map(|rule| describe(rule).key)
            .collect::<Vec<_>>(),
        ["cf.describe.data_bar", "cf.describe.color_scale"]
    );
    assert!(book.delete_conditional_format(0).unwrap());
    assert_eq!(book.conditional_formats().len(), 1);
    // Clear Rules from Selected Cells: nothing in A1, the scale in B3 —
    // which keeps B2 and B4.
    let a1 = range((0, 0), (0, 0));
    assert!(!book.clear_conditional_formats(Some(a1)).unwrap());
    assert!(book
        .clear_conditional_formats(Some(range((1, 2), (1, 2))))
        .unwrap());
    assert_eq!(book.conditional_formats()[0].range, "B2 B4");
    // Clear Rules from Entire Sheet.
    assert!(book.clear_conditional_formats(None).unwrap());
    assert!(book.conditional_formats().is_empty());
    assert_eq!(painted(&book, b2), WHITE);
    assert!(book.undo().unwrap());
    assert_eq!(book.conditional_formats().len(), 1);
}

#[test]
fn a_protected_sheet_refuses_conditional_formats() {
    let mut book = open("protected-sheets-workbook.xlsx");
    let error = book
        .add_conditional_format(
            range((0, 0), (0, 0)),
            &DATA_BARS[0].rule(),
            ConditionalStyle::None,
        )
        .unwrap_err();
    assert!(protected_refusal(&error), "{error}");
    assert!(!book.is_dirty());
}

/// The dialog's keys: typing replaces the selected value, Tab moves to the
/// next field, Enter takes it — or says what is wrong — and Escape leaves.
#[test]
fn the_dialog_takes_typing_tab_and_enter() {
    let book = book();
    let amounts = range(AMOUNTS.0, AMOUNTS.1);
    let mut ask = RuleAsk::new(RuleDialog::open(
        Preset::Between,
        &book,
        amounts,
        CellRef::new(1, 1),
    ));
    assert_eq!(ask.field, 0);
    ask.typing().unwrap().replace(None, "12", false);
    assert_eq!(ask.key("tab", false, false), AskKey::Handled);
    assert_eq!(ask.field, 1);
    ask.typing().unwrap().replace(None, "25", false);
    // A character is text input, not a key the dialog takes.
    assert_eq!(ask.key("x", false, false), AskKey::Unhandled);
    assert_eq!(ask.key("enter", false, false), AskKey::Take);
    assert_eq!(
        ask.rule(),
        Some(NewConditionalRule::Between("12".into(), "25".into()))
    );
    ask.typing().unwrap().replace(None, "", false);
    ask.fields[1].text.clear();
    assert_eq!(ask.rule(), None);
    assert_eq!(ask.error, Some("cf.invalid_value"));
    assert_eq!(ask.key("escape", false, false), AskKey::Close);
}

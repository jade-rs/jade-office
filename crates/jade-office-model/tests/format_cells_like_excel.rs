//! The Format Cells dialog (⌘1) as Excel's: its Number tab reads the cell's
//! format code into a category and options, writes them back as the code
//! Excel would, and previews a sample; the dialog applies only what was
//! changed, as one undoable step.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::format_cells::{FormatCellsDialog, Negative, NumberCategory, NumberFormat};
use jade_office_model::sheet::{CellRef, Workbook};
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

/// The codes Excel's categories write, read back into the same category
/// and written again unchanged.
#[test]
fn excel_codes_read_into_their_category_and_back() {
    for (code, category) in [
        ("General", NumberCategory::General),
        ("0", NumberCategory::Number),
        ("0.00", NumberCategory::Number),
        ("#,##0", NumberCategory::Number),
        ("#,##0.00;[Red]-#,##0.00", NumberCategory::Number),
        ("#,##0;[Red]#,##0", NumberCategory::Number),
        ("\"¥\"#,##0;\"¥\"\\-#,##0", NumberCategory::Currency),
        (
            "\"¥\"#,##0.00;[Red]\"¥\"\\-#,##0.00",
            NumberCategory::Currency,
        ),
        ("\"$\"#,##0.00;\"$\"\\-#,##0.00", NumberCategory::Currency),
        ("0%", NumberCategory::Percentage),
        ("0.00%", NumberCategory::Percentage),
        ("0.00E+00", NumberCategory::Scientific),
        ("yyyy/m/d", NumberCategory::Date),
        ("yyyy\"年\"m\"月\"d\"日\"", NumberCategory::Date),
        ("[$-411]ggge\"年\"m\"月\"d\"日\"", NumberCategory::Date),
        ("h:mm", NumberCategory::Time),
        ("h:mm:ss", NumberCategory::Time),
        ("# ?/?", NumberCategory::Fraction),
        ("@", NumberCategory::Text),
        ("0.0\"kg\"", NumberCategory::Custom),
    ] {
        let format = NumberFormat::from_code(code);
        assert_eq!(format.category, category, "{code}");
        assert_eq!(format.code(), code, "{code}");
    }
}

#[test]
fn the_number_options_write_the_code_excel_writes() {
    let mut format = NumberFormat::from_code("0.00");
    assert_eq!((format.decimals, format.thousands), (2, false));
    format.decimals = 3;
    format.thousands = true;
    assert_eq!(format.code(), "#,##0.000");
    format.negative = Negative::RedMinus;
    assert_eq!(format.code(), "#,##0.000;[Red]-#,##0.000");
    format.category = NumberCategory::Currency;
    format.decimals = 0;
    assert_eq!(format.code(), "\"¥\"#,##0;[Red]\"¥\"\\-#,##0");
    format.category = NumberCategory::Percentage;
    format.decimals = 1;
    assert_eq!(format.code(), "0.0%");
    // Every category has a sample, as the dialog shows one.
    let mut format = NumberFormat::from_code("#,##0.00");
    assert_eq!(format.sample(1234.5), "1,234.50");
    format.category = NumberCategory::Date;
    format.pattern = "yyyy/m/d".into();
    assert_eq!(format.sample(45000.0), "2023/3/15");
    format.category = NumberCategory::General;
    assert_eq!(format.sample(1234.5), "1234.5");
}

/// What the dialog changes, and only that, lands on the selection as one
/// undoable step.
#[test]
fn the_dialog_applies_only_what_was_changed() {
    let mut book = book();
    let at = CellRef::new(5, 20);
    book.set_cell(at, "1234.5").unwrap();
    let mut dialog = FormatCellsDialog::open(book.format_at(at));
    assert!(dialog.change().is_empty());
    assert_eq!(dialog.number.category, NumberCategory::General);
    dialog.number.category = NumberCategory::Currency;
    dialog.bold = true;
    let change = dialog.change();
    assert_eq!(
        change.number_format.as_deref(),
        Some("\"¥\"#,##0;\"¥\"\\-#,##0")
    );
    assert_eq!(change.bold, Some(true));
    assert!(change.italic.is_none() && change.fill_color.is_none() && change.horizontal.is_none());
    let range = jade_office_model::sheet::CellRange { start: at, end: at };
    assert!(book.format(range, &change).unwrap());
    assert_eq!(book.display_text(at), "¥1,235");
    assert!(book.format_at(at).bold.unwrap_or(false));
    // Opened again, it reads what was applied.
    let again = FormatCellsDialog::open(book.format_at(at));
    assert_eq!(again.number.category, NumberCategory::Currency);
    assert!(again.bold);
    assert!(book.undo().unwrap());
    assert_eq!(book.display_text(at), "1234.5");
}

/// The Alignment tab's indent, orientation and Shrink to fit, as Excel's:
/// an indent left-aligns General text, degrees below the horizon are
/// written 91–180, vertical text is 255, and wrapped text cannot shrink.
#[test]
fn the_alignment_tab_indents_turns_and_shrinks() {
    use jade_office_model::format_cells::{rotation_degrees, text_rotation, VERTICAL_TEXT};
    use jade_xlsx_reader::HorizontalAlignment;

    for (degrees, rotation) in [(0, 0), (45, 45), (90, 90), (-45, 135), (-90, 180)] {
        assert_eq!(text_rotation(degrees), rotation, "{degrees}");
        assert_eq!(rotation_degrees(rotation), degrees, "{rotation}");
    }
    assert_eq!(text_rotation(VERTICAL_TEXT), 255);

    let mut book = book();
    let at = CellRef::new(5, 21);
    book.set_cell(at, "見出し").unwrap();
    let mut dialog = FormatCellsDialog::open(book.format_at(at));
    assert_eq!(
        (dialog.indent, dialog.rotation, dialog.shrink),
        (0, 0, false)
    );
    dialog.indent = 2;
    dialog.rotation = -30;
    dialog.shrink = true;
    let change = dialog.change();
    assert_eq!(change.indent, Some(2));
    assert_eq!(change.text_rotation, Some(120));
    assert_eq!(change.shrink_to_fit, Some(true));
    let range = jade_office_model::sheet::CellRange { start: at, end: at };
    assert!(book.format(range, &change).unwrap());
    let look = book.format_at(at);
    assert_eq!(look.horizontal, Some(HorizontalAlignment::Left));
    assert_eq!((look.indent, look.text_rotation), (Some(2), Some(120)));
    let mut again = FormatCellsDialog::open(look);
    assert_eq!((again.indent, again.rotation, again.shrink), (2, -30, true));

    // Wrapping drops Shrink to fit; centring drops the indent.
    again.wrap = true;
    again.horizontal = HorizontalAlignment::Center;
    let change = again.change();
    assert_eq!(change.shrink_to_fit, Some(false));
    assert_eq!(change.indent, Some(0));
    assert!(book.format(range, &change).unwrap());
    let look = book.format_at(at);
    assert_eq!((look.indent, look.shrink_to_fit), (Some(0), Some(false)));
    assert_eq!(look.wrap_text, Some(true));
}

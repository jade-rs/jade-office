//! Home ▸ Cell Styles: Excel's built-in styles for good, bad and neutral
//! values, titles and headings, totals, input, output and notes. Each sets
//! what the style decides — fill, font colour, weight and size, borders —
//! and leaves the number format as it is, as Excel's own do.

use jade_xlsx_reader::CellRange;

use crate::sheet::{BorderLine, BorderStyle, CellFormat as FormatChange, Workbook};
use crate::Backend;

/// One of Excel's built-in cell styles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellStyle {
    Normal,
    Good,
    Bad,
    Neutral,
    Title,
    Heading1,
    Heading2,
    Heading3,
    Total,
    Input,
    Output,
    Note,
}

impl CellStyle {
    pub const ALL: [CellStyle; 12] = [
        CellStyle::Normal,
        CellStyle::Good,
        CellStyle::Bad,
        CellStyle::Neutral,
        CellStyle::Title,
        CellStyle::Heading1,
        CellStyle::Heading2,
        CellStyle::Heading3,
        CellStyle::Total,
        CellStyle::Input,
        CellStyle::Output,
        CellStyle::Note,
    ];

    /// The i18n key of its name.
    pub fn label(self) -> &'static str {
        match self {
            CellStyle::Normal => "cell_style.normal",
            CellStyle::Good => "cell_style.good",
            CellStyle::Bad => "cell_style.bad",
            CellStyle::Neutral => "cell_style.neutral",
            CellStyle::Title => "cell_style.title",
            CellStyle::Heading1 => "cell_style.heading1",
            CellStyle::Heading2 => "cell_style.heading2",
            CellStyle::Heading3 => "cell_style.heading3",
            CellStyle::Total => "cell_style.total",
            CellStyle::Input => "cell_style.input",
            CellStyle::Output => "cell_style.output",
            CellStyle::Note => "cell_style.note",
        }
    }

    /// The format the style gives a cell, as the Office theme draws it.
    pub fn change(self) -> FormatChange {
        let line = |style: BorderStyle, color: &str| BorderLine {
            style,
            color: Some(color.to_string()),
        };
        let none = BorderLine {
            style: BorderStyle::None,
            color: None,
        };
        // Normal: what every style starts from.
        let mut change = FormatChange {
            fill_color: Some(String::new()),
            font_color: Some("000000".into()),
            bold: Some(false),
            font_size: Some(11.0),
            border_left: Some(none.clone()),
            border_right: Some(none.clone()),
            border_top: Some(none.clone()),
            border_bottom: Some(none),
            ..Default::default()
        };
        let boxed = |change: &mut FormatChange, color: &str| {
            for edge in [
                &mut change.border_left,
                &mut change.border_right,
                &mut change.border_top,
                &mut change.border_bottom,
            ] {
                *edge = Some(line(BorderStyle::Thin, color));
            }
        };
        match self {
            CellStyle::Normal => {}
            CellStyle::Good => {
                change.fill_color = Some("C6EFCE".into());
                change.font_color = Some("006100".into());
            }
            CellStyle::Bad => {
                change.fill_color = Some("FFC7CE".into());
                change.font_color = Some("9C0006".into());
            }
            CellStyle::Neutral => {
                change.fill_color = Some("FFEB9C".into());
                change.font_color = Some("9C5700".into());
            }
            CellStyle::Title => {
                change.font_color = Some("44546A".into());
                change.font_size = Some(18.0);
            }
            CellStyle::Heading1 => {
                change.font_color = Some("44546A".into());
                change.font_size = Some(15.0);
                change.bold = Some(true);
                change.border_bottom = Some(line(BorderStyle::Thick, "4472C4"));
            }
            CellStyle::Heading2 => {
                change.font_color = Some("44546A".into());
                change.font_size = Some(13.0);
                change.bold = Some(true);
                change.border_bottom = Some(line(BorderStyle::Thick, "A2B8E1"));
            }
            CellStyle::Heading3 => {
                change.font_color = Some("44546A".into());
                change.bold = Some(true);
                change.border_bottom = Some(line(BorderStyle::Medium, "8EA9DB"));
            }
            CellStyle::Total => {
                change.bold = Some(true);
                change.border_top = Some(line(BorderStyle::Thin, "4472C4"));
                change.border_bottom = Some(line(BorderStyle::Double, "4472C4"));
            }
            CellStyle::Input => {
                change.fill_color = Some("FFCC99".into());
                change.font_color = Some("3F3F76".into());
                boxed(&mut change, "7F7F7F");
            }
            CellStyle::Output => {
                change.fill_color = Some("F2F2F2".into());
                change.font_color = Some("3F3F3F".into());
                change.bold = Some(true);
                boxed(&mut change, "3F3F3F");
            }
            CellStyle::Note => {
                change.fill_color = Some("FFFFCC".into());
                boxed(&mut change, "B2B2B2");
            }
        }
        change
    }
}

impl<B: Backend> Workbook<B> {
    /// `style` given to the cells in `range`. One undo step.
    pub fn apply_cell_style(&mut self, range: CellRange, style: CellStyle) -> Result<bool, String> {
        self.format(range, &style.change())
    }
}

//! The Number Format dropdown on Excel's Home tab: General, Number,
//! Currency, the dates, Time, Percentage, Fraction, Scientific and Text,
//! each shown with what the active cell would read as in it, and the
//! button naming the format the active cell has.
//!
//! The names and codes are the shells' i18n keys, since a Japanese Excel's
//! currency and dates are not an English one's: a shell looks them up
//! through its own table, passed in as `text`.

use crate::sheet::{CellRef, Workbook};
use crate::Backend;
use jade_xlsx_reader::CellValue;

/// One entry of the dropdown: its element id, and the i18n keys of its
/// name and its format code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NumberFormatChoice {
    pub id: &'static str,
    pub name: &'static str,
    pub code: &'static str,
}

const fn choice(id: &'static str, name: &'static str, code: &'static str) -> NumberFormatChoice {
    NumberFormatChoice { id, name, code }
}

/// The dropdown's entries, in Excel's order.
pub const NUMBER_FORMATS: &[NumberFormatChoice] = &[
    choice(
        "nf-general",
        "sheet.format.general_name",
        "sheet.format.general",
    ),
    choice(
        "nf-number",
        "sheet.format.number_name",
        "sheet.format.number",
    ),
    choice(
        "nf-currency",
        "sheet.format.currency_name",
        "sheet.format.currency",
    ),
    choice("nf-date", "sheet.format.date_name", "sheet.format.date"),
    choice(
        "nf-long-date",
        "sheet.format.long_date_name",
        "sheet.format.long_date",
    ),
    choice("nf-time", "sheet.format.time_name", "sheet.format.time"),
    choice(
        "nf-percent",
        "sheet.format.percent_name",
        "sheet.format.percent",
    ),
    choice(
        "nf-fraction",
        "sheet.format.fraction_name",
        "sheet.format.fraction",
    ),
    choice(
        "nf-scientific",
        "sheet.format.scientific_name",
        "sheet.format.scientific",
    ),
    choice("nf-text", "sheet.format.text_name", "sheet.format.text"),
];

/// What the button says for a cell whose format is none of the entries.
pub const CUSTOM_NAME: &str = "sheet.format.custom_name";

impl<B: Backend> Workbook<B> {
    /// The i18n key of the name the dropdown's button shows for the cell
    /// at `at`: the entry whose code (looked up through `text`) is the
    /// cell's, or Custom.
    pub fn number_format_name<S: AsRef<str>>(
        &self,
        at: CellRef,
        text: impl Fn(&'static str) -> S,
    ) -> &'static str {
        let code = self.format_at(at).number_format.unwrap_or_default();
        let code = if code.is_empty() { "General" } else { &code };
        NUMBER_FORMATS
            .iter()
            .find(|choice| text(choice.code).as_ref().eq_ignore_ascii_case(code))
            .map_or(CUSTOM_NAME, |choice| choice.name)
    }

    /// What the cell at `at` would read as in `code`, as the dropdown shows
    /// beside each entry; nothing for an empty cell. Text shows as it is,
    /// whatever the code.
    pub fn number_format_sample(&self, at: CellRef, code: &str) -> Option<String> {
        let value = match self.number(at) {
            Some(number) => CellValue::Number(number),
            None => {
                let shown = self.display_text(at);
                return (!shown.is_empty()).then_some(shown);
            }
        };
        let text =
            jade_xlsx_layout::number_format::format_value(&value, Some(code), self.date_system())
                .text;
        Some(text)
    }
}

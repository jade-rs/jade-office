//! CSV as Excel opens and saves it.
//!
//! Opening: a sheet named after the file, each field read as if typed —
//! `007` the number 7, `2024/1/15` a date shown as one, `=SUM(A1:A2)` a
//! formula — quoted fields kept whole, line breaks and all. The bytes are
//! UTF-8 (with or without the BOM Excel's *CSV UTF-8* writes) or, failing
//! that, Shift_JIS, which a Japanese Excel's *CSV (Comma delimited)* writes.
//!
//! Saving: the active sheet only, as its cells show — `1,234.50` as it
//! reads, quoted — comma-separated with CR LF line ends, every row as wide
//! as the part of the sheet in use; UTF-8 with a BOM, as *CSV UTF-8*.

use std::collections::BTreeMap;

use jade_xlsx_headless_api::CellInput;
use jade_xlsx_headless_editor::{FormatChange, HeadlessWorkbook};

use crate::sheet::{range_text, reference_text, CellRange, CellRef, Workbook};
use crate::{clipboard, typing, Backend};

/// A workbook with one empty sheet, for a CSV to fill.
const BLANK: &[u8] = include_bytes!("../assets/blank.xlsx");

const BOM: &str = "\u{feff}";

/// Whether `name` is a CSV file's, by its extension.
pub fn is_csv(name: &str) -> bool {
    name.rsplit_once('.')
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("csv"))
}

/// CSV bytes as text: UTF-8 (its BOM dropped), else Shift_JIS.
pub fn decode(bytes: &[u8]) -> String {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.strip_prefix(BOM).unwrap_or(text).to_string();
    }
    let (text, _, _) = encoding_rs::SHIFT_JIS.decode(bytes);
    text.into_owned()
}

/// The rows and fields of CSV text.
pub fn parse(text: &str) -> Vec<Vec<String>> {
    clipboard::parse_separated(text, ',')
}

/// What Excel names a sheet opened from `file`: the name without its
/// extension, without the characters a sheet name cannot have, at most 31
/// characters.
pub fn sheet_name(file: &str) -> String {
    let stem = file
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(file)
        .rsplit_once('.')
        .map_or(file, |(stem, _)| stem);
    let name: String = stem
        .chars()
        .filter(|c| !matches!(c, '[' | ']' | ':' | '*' | '?' | '/' | '\\'))
        .take(31)
        .collect();
    let name = name.trim_matches('\'').to_string();
    if name.is_empty() {
        "Sheet1".to_string()
    } else {
        name
    }
}

impl<B: Backend> Workbook<B> {
    /// Opens CSV `bytes` from the file `name` as a workbook of one sheet,
    /// nothing to undo and nothing unsaved.
    pub fn open_csv(name: &str, bytes: &[u8], time_zone: &str, backend: B) -> Result<Self, String> {
        let xlsx = csv_to_xlsx(name, &decode(bytes))?;
        Self::open(&xlsx, time_zone, backend)
    }

    /// The active sheet as CSV text, as Excel saves it.
    pub fn csv_text(&self) -> String {
        let mut out = String::new();
        let sheet = &self.engine().workbook().sheets[self.sheet_index()];
        if sheet.rows.iter().all(|row| row.cells.is_empty()) {
            return out;
        }
        let end = self.used_end();
        for row in 0..=end.row {
            let fields: Vec<String> = (0..=end.column)
                .map(|column| csv_field(&self.display_text(CellRef::new(column, row))))
                .collect();
            out.push_str(&fields.join(","));
            out.push_str("\r\n");
        }
        out
    }

    /// The bytes saving under `name` writes: the active sheet as CSV (UTF-8
    /// with a BOM) for a `.csv` name, the workbook as `.xlsx` otherwise.
    /// Either way the workbook counts as saved.
    pub fn save_bytes_as(&mut self, name: &str) -> Result<Vec<u8>, String> {
        if !is_csv(name) {
            return self.save_bytes();
        }
        let bytes = format!("{BOM}{}", self.csv_text()).into_bytes();
        self.mark_saved();
        Ok(bytes)
    }
}

/// One field as CSV writes it, quoted when it holds a comma, a quote or a
/// line break.
fn csv_field(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}

/// The CSV's fields typed into a blank workbook's one sheet, named for the
/// file; the formats the typing implies (a date's, a percentage's) given
/// column by column.
fn csv_to_xlsx(name: &str, text: &str) -> Result<Vec<u8>, String> {
    let mut book = HeadlessWorkbook::open_for_editing(BLANK).map_err(|error| error.to_string())?;
    let sheet = sheet_name(name);
    book.rename_sheet("Sheet1", &sheet)
        .map_err(|error| error.to_string())?;
    let date1904 = false;
    let mut values: Vec<(String, CellInput)> = Vec::new();
    // Per column, the rows each format code goes on.
    let mut formats: BTreeMap<(u32, &'static str), Vec<u32>> = BTreeMap::new();
    for (row, fields) in parse(text).iter().enumerate() {
        for (column, field) in fields.iter().enumerate() {
            if field.is_empty() {
                continue;
            }
            let at = CellRef::new(column as u32, row as u32);
            let typed = typing::read(field, date1904);
            if let Some(code) = typed.format {
                formats.entry((at.column, code)).or_default().push(at.row);
            }
            values.push((reference_text(at), typed.input));
        }
    }
    if !values.is_empty() {
        book.set_cells(&sheet, &values)
            .map_err(|error| error.to_string())?;
    }
    for ((column, code), rows) in formats {
        let change = FormatChange {
            number_format: Some(code.to_string()),
            ..FormatChange::default()
        };
        for run in runs(&rows) {
            let range = CellRange {
                start: CellRef::new(column, run.0),
                end: CellRef::new(column, run.1),
            };
            book.set_format(&sheet, &range_text(range), &change)
                .map_err(|error| error.to_string())?;
        }
    }
    book.save_bytes().map_err(|error| error.to_string())
}

/// Sorted rows as runs of consecutive ones, first and last.
fn runs(rows: &[u32]) -> Vec<(u32, u32)> {
    let mut runs: Vec<(u32, u32)> = Vec::new();
    for &row in rows {
        match runs.last_mut() {
            Some(run) if run.1 + 1 == row => run.1 = row,
            _ => runs.push((row, row)),
        }
    }
    runs
}

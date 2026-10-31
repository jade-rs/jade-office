//! Data ▸ Text to Columns, delimited: the text of one column split at the
//! delimiters chosen into the columns from it rightward, each piece taken as
//! typing would take it — `1,234` a number, `2026/1/5` a date.

use jade_xlsx_headless_api::{CellInput, CellKind};
use jade_xlsx_reader::{CellRange, CellRef};

use crate::sheet::{cell_input, reference_text, Workbook};
use crate::Backend;

/// The delimiters the wizard's second step offers, and how it reads them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextToColumns {
    pub tab: bool,
    pub semicolon: bool,
    pub comma: bool,
    pub space: bool,
    /// Treat consecutive delimiters as one.
    pub consecutive: bool,
    /// The text qualifier: delimiters inside a pair of these are text, and
    /// a doubled one stands for itself. `"` by default, as in Excel.
    pub qualifier: Option<char>,
}

impl Default for TextToColumns {
    /// Excel's defaults: split at tabs, `"` around text.
    fn default() -> Self {
        Self {
            tab: true,
            semicolon: false,
            comma: false,
            space: false,
            consecutive: false,
            qualifier: Some('"'),
        }
    }
}

impl TextToColumns {
    fn splits_at(&self, character: char) -> bool {
        (self.tab && character == '\t')
            || (self.semicolon && character == ';')
            || (self.comma && character == ',')
            || (self.space && character == ' ')
    }

    /// `text` in pieces at the delimiters, as the wizard's preview shows it.
    pub fn split(&self, text: &str) -> Vec<String> {
        let mut pieces = vec![String::new()];
        let mut quoted = false;
        let mut chars = text.chars().peekable();
        let mut last_was_delimiter = false;
        while let Some(character) = chars.next() {
            if Some(character) == self.qualifier {
                if quoted && chars.peek() == Some(&character) {
                    // A doubled qualifier inside the quotes stands for one.
                    chars.next();
                    pieces.last_mut().expect("a piece").push(character);
                } else {
                    quoted = !quoted;
                }
                last_was_delimiter = false;
                continue;
            }
            if !quoted && self.splits_at(character) {
                if !(self.consecutive && last_was_delimiter) {
                    pieces.push(String::new());
                }
                last_was_delimiter = true;
                continue;
            }
            last_was_delimiter = false;
            pieces.last_mut().expect("a piece").push(character);
        }
        pieces
    }
}

impl<B: Backend> Workbook<B> {
    /// Splits the text cells of `range`'s first column into the columns
    /// from it rightward, overwriting what is there as Excel does. One undo
    /// step; `false` when no cell had text to split.
    pub fn text_to_columns(
        &mut self,
        range: CellRange,
        split: &TextToColumns,
    ) -> Result<bool, String> {
        let name = self.sheet_name();
        let column = range.start.column;
        let mut values: Vec<(String, CellInput)> = Vec::new();
        for row in range.start.row..=range.end.row {
            let at = CellRef::new(column, row);
            let Ok(cell) = self.engine().cell(&name, &reference_text(at)) else {
                continue;
            };
            let Some(text) = cell
                .text
                .filter(|_| cell.formula.is_none() && cell.kind == CellKind::Text)
            else {
                continue;
            };
            let pieces = split.split(&text);
            if pieces.len() < 2 && pieces.first().is_some_and(|piece| *piece == text) {
                continue;
            }
            for (offset, piece) in pieces.iter().enumerate() {
                let target = CellRef::new(column + offset as u32, row);
                let input = if piece.is_empty() {
                    CellInput::Blank
                } else {
                    cell_input(piece)
                };
                values.push((reference_text(target), input));
            }
        }
        if values.is_empty() {
            return Ok(false);
        }
        self.engine_mut()
            .set_cells(&name, &values)
            .map_err(|error| error.to_string())?;
        self.wrote(1)?;
        Ok(true)
    }
}

/// The Text to Columns wizard, delimited: the column it splits and the
/// delimiters chosen, with the first rows for its preview.
#[derive(Debug, Clone, PartialEq)]
pub struct TextColumnsDialog {
    pub range: CellRange,
    pub split: TextToColumns,
    /// The first few texts of the column, as the preview splits them.
    pub sample: Vec<String>,
}

impl TextColumnsDialog {
    /// Opened on the selection's first column — or, with one cell selected,
    /// the column of the list round it.
    pub fn open<B: Backend>(book: &Workbook<B>, selected: CellRange, at: CellRef) -> Self {
        let region = if selected.start == selected.end {
            book.current_region(at).unwrap_or(selected)
        } else {
            selected
        };
        let column = region.start.column;
        let range = CellRange {
            start: CellRef::new(column, region.start.row),
            end: CellRef::new(column, region.end.row),
        };
        let sample = (range.start.row..=range.end.row)
            .map(|row| book.display_text(CellRef::new(column, row)))
            .filter(|text| !text.is_empty())
            .take(5)
            .collect();
        Self {
            range,
            split: TextToColumns::default(),
            sample,
        }
    }

    /// The sample as the delimiters chosen would split it.
    pub fn preview(&self) -> Vec<Vec<String>> {
        self.sample
            .iter()
            .map(|text| self.split.split(text))
            .collect()
    }
}

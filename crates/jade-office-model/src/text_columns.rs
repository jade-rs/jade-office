//! Data ▸ Text to Columns: the text of one column split at the delimiters
//! chosen — or, fixed width, at break positions — into the columns from it
//! rightward, each piece taken as typing would take it — `1,234` a number,
//! `2026/1/5` a date.

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
    /// Fixed width: the character positions a field starts at, past the
    /// first, in order. The delimiters do not count then.
    pub fixed_width: Option<Vec<usize>>,
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
            fixed_width: None,
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
        if let Some(breaks) = &self.fixed_width {
            return split_fixed(text, breaks);
        }
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

/// `text` cut at the character positions `breaks`, each piece without
/// the spaces round it, as Excel takes fixed-width fields. A line ending
/// before a break has no piece past it.
fn split_fixed(text: &str, breaks: &[usize]) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut pieces = Vec::new();
    let mut start = 0;
    for &end in breaks.iter().chain(std::iter::once(&usize::MAX)) {
        if start >= chars.len() && !pieces.is_empty() {
            break;
        }
        let end = end.min(chars.len());
        let piece: String = chars[start.min(end)..end].iter().collect();
        pieces.push(piece.trim_matches(' ').to_string());
        start = end;
    }
    pieces
}

/// Where the wizard first puts fixed-width breaks: before each column of
/// characters that starts a field on some line, after a column that is a
/// space (or past the end) on every line.
pub fn guess_breaks(lines: &[String]) -> Vec<usize> {
    let lines: Vec<Vec<char>> = lines.iter().map(|line| line.chars().collect()).collect();
    let width = lines.iter().map(Vec::len).max().unwrap_or(0);
    let blank = |line: &Vec<char>, at: usize| line.get(at).is_none_or(|c| *c == ' ');
    (1..width)
        .filter(|&at| {
            lines.iter().all(|line| blank(line, at - 1))
                && lines.iter().any(|line| !blank(line, at))
        })
        .collect()
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

    /// Fixed width, with the breaks guessed from the sample as the wizard's
    /// second step shows them; or, `false`, delimited again.
    pub fn set_fixed_width(&mut self, on: bool) {
        self.split.fixed_width = on.then(|| guess_breaks(&self.sample));
    }

    /// A click on the ruler: a break before character `at` put in, or taken
    /// away when there is one.
    pub fn toggle_break(&mut self, at: usize) {
        let Some(breaks) = self.split.fixed_width.as_mut() else {
            return;
        };
        if at == 0 {
            return;
        }
        match breaks.binary_search(&at) {
            Ok(index) => {
                breaks.remove(index);
            }
            Err(index) => breaks.insert(index, at),
        }
    }

    /// The sample's widest line, in characters: how long the ruler is.
    pub fn sample_width(&self) -> usize {
        self.sample
            .iter()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0)
    }

    /// The sample as the delimiters chosen would split it.
    pub fn preview(&self) -> Vec<Vec<String>> {
        self.sample
            .iter()
            .map(|text| self.split.split(text))
            .collect()
    }
}

//! Data ▸ Text to Columns: the text of one column split at the delimiters
//! chosen — or, fixed width, at break positions — into the columns from it
//! rightward, each piece taken as typing would take it — `1,234` a number,
//! `2026/1/5` a date — unless its column is set to Text, or not imported.
//!
//! Data ▸ From Text is the same wizard over a text file's lines, from the
//! row chosen, put at the active cell: Excel's Text Import Wizard.

use std::collections::BTreeMap;

use jade_xlsx_headless_api::{CellInput, CellKind};
use jade_xlsx_headless_editor::FormatChange;
use jade_xlsx_reader::{CellRange, CellRef};

use crate::sheet::{range_text, reference_text, Workbook};
use crate::{typing, Backend};

/// The wizard's third step: how one column's pieces are taken.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ColumnFormat {
    /// As typing takes them: numbers, dates and the rest.
    #[default]
    General,
    /// As text, `007` staying `007`, in the Text number format.
    Text,
    /// Not put anywhere; the columns after it close up.
    Skip,
}

impl ColumnFormat {
    /// The step's choices, in Excel's order.
    pub const ALL: [ColumnFormat; 3] = [
        ColumnFormat::General,
        ColumnFormat::Text,
        ColumnFormat::Skip,
    ];

    /// The i18n key of its name.
    pub fn label(self) -> &'static str {
        match self {
            ColumnFormat::General => "text_columns.general",
            ColumnFormat::Text => "text_columns.text",
            ColumnFormat::Skip => "text_columns.skip",
        }
    }
}

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
        self.text_to_columns_formatted(range, split, &[])
    }

    /// [`Workbook::text_to_columns`] with each column taken as `formats`
    /// says, by piece; pieces past its end are General.
    pub fn text_to_columns_formatted(
        &mut self,
        range: CellRange,
        split: &TextToColumns,
        formats: &[ColumnFormat],
    ) -> Result<bool, String> {
        let name = self.sheet_name();
        let column = range.start.column;
        let mut rows: Vec<(CellRef, Vec<String>)> = Vec::new();
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
            if pieces.len() < 2
                && pieces.first().is_some_and(|piece| *piece == text)
                && formats
                    .first()
                    .is_none_or(|format| *format == ColumnFormat::General)
            {
                continue;
            }
            rows.push((at, pieces));
        }
        self.write_pieces(&rows, formats)
    }

    /// Data ▸ From Text: the lines of `import` from its start row, split as
    /// `split` says and taken as `formats` says, put at `import.at` down and
    /// rightward. One undo step. Returns the range filled, `None` when there
    /// was nothing to put.
    pub fn import_text(
        &mut self,
        import: &TextImport,
        split: &TextToColumns,
        formats: &[ColumnFormat],
    ) -> Result<Option<CellRange>, String> {
        let rows: Vec<(CellRef, Vec<String>)> = import
            .lines
            .iter()
            .skip(import.start)
            .enumerate()
            .filter(|(_, line)| !line.is_empty())
            .map(|(row, line)| {
                (
                    CellRef::new(import.at.column, import.at.row + row as u32),
                    split.split(line),
                )
            })
            .collect();
        let Some(last) = rows.last().map(|(at, _)| at.row) else {
            return Ok(None);
        };
        let width = rows
            .iter()
            .map(|(_, pieces)| {
                pieces
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| formats.get(*index) != Some(&ColumnFormat::Skip))
                    .count()
            })
            .max()
            .unwrap_or(0) as u32;
        if width == 0 || !self.write_pieces(&rows, formats)? {
            return Ok(None);
        }
        Ok(Some(CellRange {
            start: import.at,
            end: CellRef::new(import.at.column + width - 1, last),
        }))
    }

    /// Each row's pieces put from its cell rightward, a skipped column's
    /// left out; General ones as typed (with the number format a date or a
    /// percentage implies), Text ones as text in the Text format. One undo
    /// step; `false` when there was nothing to put.
    fn write_pieces(
        &mut self,
        rows: &[(CellRef, Vec<String>)],
        formats: &[ColumnFormat],
    ) -> Result<bool, String> {
        let date1904 = matches!(self.date_system(), jade_xlsx_reader::DateSystem::Nineteen04);
        let mut values: Vec<(String, CellInput)> = Vec::new();
        // Per column, the rows each number format goes on.
        let mut implied: BTreeMap<(u32, &'static str), Vec<u32>> = BTreeMap::new();
        for (from, pieces) in rows {
            let mut column = from.column;
            for (index, piece) in pieces.iter().enumerate() {
                let format = formats.get(index).copied().unwrap_or_default();
                if format == ColumnFormat::Skip {
                    continue;
                }
                let target = CellRef::new(column, from.row);
                column += 1;
                let input = match format {
                    _ if piece.is_empty() => CellInput::Blank,
                    ColumnFormat::Text => CellInput::Text(piece.clone()),
                    _ => {
                        let typed = typing::read(piece, date1904);
                        if let Some(code) = typed.format {
                            implied
                                .entry((target.column, code))
                                .or_default()
                                .push(target.row);
                        }
                        typed.input
                    }
                };
                if format == ColumnFormat::Text {
                    implied
                        .entry((target.column, "@"))
                        .or_default()
                        .push(target.row);
                }
                values.push((reference_text(target), input));
            }
        }
        if values.is_empty() {
            return Ok(false);
        }
        let name = self.sheet_name();
        self.atomic_edit(|book| {
            let mut steps = 0;
            // The Text format first, so what goes in stays text.
            for ((column, code), rows) in implied.iter().filter(|((_, code), _)| *code == "@") {
                steps += book.set_number_format_runs(&name, *column, code, rows)?;
            }
            book.engine_mut()
                .set_cells(&name, &values)
                .map_err(|error| error.to_string())?;
            steps += 1;
            for ((column, code), rows) in implied.iter().filter(|((_, code), _)| *code != "@") {
                steps += book.set_number_format_runs(&name, *column, code, rows)?;
            }
            book.wrote(steps)?;
            Ok(true)
        })
    }

    /// `code` given to the cells of `column` at `rows` (sorted), a run of
    /// consecutive rows at a time; the writes it took.
    fn set_number_format_runs(
        &mut self,
        name: &str,
        column: u32,
        code: &str,
        rows: &[u32],
    ) -> Result<usize, String> {
        let change = FormatChange {
            number_format: Some(code.to_string()),
            ..FormatChange::default()
        };
        let mut steps = 0;
        let mut start = 0;
        while start < rows.len() {
            let mut end = start;
            while end + 1 < rows.len() && rows[end + 1] == rows[end] + 1 {
                end += 1;
            }
            let range = CellRange {
                start: CellRef::new(column, rows[start]),
                end: CellRef::new(column, rows[end]),
            };
            self.engine_mut()
                .set_format(name, &range_text(range), &change)
                .map_err(|error| error.to_string())?;
            steps += 1;
            start = end + 1;
        }
        Ok(steps)
    }
}

/// From Text's file: its lines, the first to import and where they go.
#[derive(Debug, Clone, PartialEq)]
pub struct TextImport {
    /// The file's lines, a quoted field's line breaks kept inside its line.
    pub lines: Vec<String>,
    /// The first line imported (the wizard's "Start import at row", less 1).
    pub start: usize,
    /// The cell the first line's first piece goes in.
    pub at: CellRef,
}

/// `text` in lines at its line breaks, except those inside a pair of
/// `qualifier`s, which belong to a field; a last empty line dropped.
pub fn text_lines(text: &str, qualifier: Option<char>) -> Vec<String> {
    let mut lines = vec![String::new()];
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(character) = chars.next() {
        if Some(character) == qualifier {
            quoted = !quoted;
        }
        match character {
            '\r' if !quoted && chars.peek() == Some(&'\n') => {}
            '\n' | '\r' if !quoted => lines.push(String::new()),
            _ => lines.last_mut().expect("a line").push(character),
        }
    }
    if lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines
}

/// The Text to Columns wizard, delimited: the column it splits and the
/// delimiters chosen, with the first rows for its preview, and each
/// column's format. With `import`, it is From Text's wizard over a file.
#[derive(Debug, Clone, PartialEq)]
pub struct TextColumnsDialog {
    pub range: CellRange,
    pub split: TextToColumns,
    /// The first few texts of the column, as the preview splits them.
    pub sample: Vec<String>,
    /// Each column's format, by piece; past its end, General.
    pub formats: Vec<ColumnFormat>,
    /// The column of the preview whose format the step's choices set.
    pub column: usize,
    pub import: Option<TextImport>,
}

/// How many lines From Text's preview shows.
const IMPORT_SAMPLE: usize = 8;

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
            formats: Vec::new(),
            column: 0,
            import: None,
        }
    }

    /// From Text over the file `name` holding `bytes`, to go at `at`: its
    /// text decoded as a CSV's is, split at what the file seems to be split
    /// at — tabs, commas or semicolons.
    pub fn open_import(name: &str, bytes: &[u8], at: CellRef) -> Self {
        let text = crate::csv::decode(bytes);
        let mut split = TextToColumns::default();
        if !name.to_ascii_lowercase().ends_with(".txt") || !text.contains('\t') {
            let separator = crate::csv::separator(name, &text);
            split.tab = separator == '\t';
            split.comma = separator == ',';
            split.semicolon = separator == ';';
        }
        let lines = text_lines(&text, split.qualifier);
        let mut dialog = Self {
            range: CellRange { start: at, end: at },
            split,
            sample: Vec::new(),
            formats: Vec::new(),
            column: 0,
            import: Some(TextImport {
                lines,
                start: 0,
                at,
            }),
        };
        dialog.resample();
        dialog
    }

    /// "Start import at row": the first line imported moved by `step`,
    /// staying within the file.
    pub fn step_start(&mut self, step: isize) {
        if let Some(import) = self.import.as_mut() {
            let last = import.lines.len().saturating_sub(1);
            import.start = import.start.saturating_add_signed(step).min(last);
        }
        self.resample();
    }

    fn resample(&mut self) {
        if let Some(import) = &self.import {
            self.sample = import
                .lines
                .iter()
                .skip(import.start)
                .take(IMPORT_SAMPLE)
                .cloned()
                .collect();
        }
    }

    /// The preview's column `column` chosen, for the format choices.
    pub fn select_column(&mut self, column: usize) {
        self.column = column;
    }

    /// The chosen column's format.
    pub fn format(&self) -> ColumnFormat {
        self.formats.get(self.column).copied().unwrap_or_default()
    }

    /// The chosen column taken as `format`.
    pub fn set_format(&mut self, format: ColumnFormat) {
        if self.formats.len() <= self.column {
            self.formats.resize(self.column + 1, ColumnFormat::General);
        }
        self.formats[self.column] = format;
    }

    /// OK: the column split, or the file put in, as chosen. `true` when
    /// something was written.
    pub fn apply<B: Backend>(&self, book: &mut Workbook<B>) -> Result<bool, String> {
        match &self.import {
            Some(import) => Ok(book
                .import_text(import, &self.split, &self.formats)?
                .is_some()),
            None => book.text_to_columns_formatted(self.range, &self.split, &self.formats),
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

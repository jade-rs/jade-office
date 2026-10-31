//! Data ▸ Remove Duplicates: rows of a range whose values in the columns
//! compared repeat an earlier row's are taken out, the rows below moving up
//! within the range as Delete ▸ Shift cells up moves them (formulas reading
//! them follow), and what lies below the range staying where it is.

use jade_xlsx_headless_api::CellKind;
use jade_xlsx_reader::{Axis, CellRange, CellRef};

use crate::sheet::{column_label, reference_text, span, Workbook};
use crate::Backend;

/// What Remove Duplicates did: the rows taken out and those that remain,
/// for Excel's message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deduplicated {
    pub removed: usize,
    pub remaining: usize,
}

impl<B: Backend> Workbook<B> {
    /// Remove Duplicates over `range`, comparing the values in `columns`
    /// (text without regard to case, as Excel does), its first row a header
    /// left out of the comparison when `headers`. One undo step.
    pub fn remove_duplicates(
        &mut self,
        range: CellRange,
        columns: &[u32],
        headers: bool,
    ) -> Result<Deduplicated, String> {
        let first = range.start.row + u32::from(headers);
        if first > range.end.row || columns.is_empty() {
            return Ok(Deduplicated {
                removed: 0,
                remaining: 0,
            });
        }
        let mut seen = std::collections::HashSet::new();
        let mut duplicates = Vec::new();
        for row in first..=range.end.row {
            let key: Vec<String> = columns
                .iter()
                .map(|&column| self.comparable(CellRef::new(column, row)))
                .collect();
            if !seen.insert(key) {
                duplicates.push(row);
            }
        }
        let remaining = (range.end.row - first + 1) as usize - duplicates.len();
        if duplicates.is_empty() {
            return Ok(Deduplicated {
                removed: 0,
                remaining,
            });
        }
        // Runs of neighbouring duplicates, bottom up so the rows above keep
        // their places.
        let mut runs: Vec<(u32, u32)> = Vec::new();
        for &row in &duplicates {
            match runs.last_mut() {
                Some(run) if run.1 + 1 == row => run.1 = row,
                _ => runs.push((row, row)),
            }
        }
        let name = self.sheet_name();
        let (left, right) = (range.start.column, range.end.column);
        self.atomic_edit(|book| {
            let mut steps = 0;
            for &(top, bottom) in runs.iter().rev() {
                let count = bottom - top + 1;
                // Out with the run, the range's cells below it moving up …
                book.engine_mut()
                    .shift_cells(
                        &name,
                        span(CellRef::new(left, top), CellRef::new(right, bottom)),
                        Axis::Rows,
                        false,
                    )
                    .map_err(|error| error.to_string())?;
                // … and as many cells back at the range's foot, so what lies
                // below the range moves back down to where it was.
                let foot = range.end.row + 1 - count;
                book.engine_mut()
                    .shift_cells(
                        &name,
                        span(CellRef::new(left, foot), CellRef::new(right, range.end.row)),
                        Axis::Rows,
                        true,
                    )
                    .map_err(|error| error.to_string())?;
                steps += 2;
            }
            book.wrote(steps)?;
            Ok(Deduplicated {
                removed: duplicates.len(),
                remaining,
            })
        })
    }

    /// Whether the range's first row reads as a header — all text, over a
    /// row that is not — Excel's guess for My data has headers.
    pub fn looks_like_headers(&self, range: CellRange) -> bool {
        self.looks_like_header(range)
    }

    /// A cell's value as Remove Duplicates compares it.
    fn comparable(&self, at: CellRef) -> String {
        let name = self.sheet_name();
        let Ok(cell) = self.engine().cell(&name, &reference_text(at)) else {
            return String::new();
        };
        match cell.kind {
            CellKind::Empty => String::new(),
            CellKind::Number | CellKind::Bool => {
                // -0 and 0 are one value.
                let number = cell.number.unwrap_or_default() + 0.0;
                format!("{:?}:{}", cell.kind, number.to_bits())
            }
            _ => format!(
                "{:?}:{}",
                cell.kind,
                cell.text.unwrap_or_default().to_lowercase()
            ),
        }
    }
}

/// The Remove Duplicates dialog: the list, whether its first row is a
/// header, and the columns compared — every one to start with, as Excel
/// opens it.
#[derive(Debug, Clone, PartialEq)]
pub struct DuplicatesDialog {
    pub range: CellRange,
    /// My data has headers.
    pub header: bool,
    /// Whether each column of the list, from its first, is compared.
    pub compared: Vec<bool>,
    /// The first row's texts, the names of the columns when it is a header.
    headers: Vec<String>,
}

impl DuplicatesDialog {
    /// Opened on the selection, or on the list round the active cell when
    /// only one cell is selected.
    pub fn open<B: Backend>(book: &Workbook<B>, selected: CellRange, at: CellRef) -> Self {
        let range = if selected.start == selected.end {
            book.current_region(at).unwrap_or(selected)
        } else {
            selected
        };
        let headers: Vec<String> = (range.start.column..=range.end.column)
            .map(|column| book.display_text(CellRef::new(column, range.start.row)))
            .collect();
        Self {
            range,
            header: book.looks_like_headers(range),
            compared: vec![true; headers.len()],
            headers,
        }
    }

    /// The columns' names as the dialog lists them: the header row's when
    /// the list has one, else `Column A` and so on.
    pub fn column_names(&self, column_word: &str) -> Vec<String> {
        (0..self.headers.len())
            .map(
                |index| match self.headers.get(index).filter(|_| self.header) {
                    Some(name) if !name.is_empty() => name.clone(),
                    _ => format!(
                        "{column_word} {}",
                        column_label(self.range.start.column + index as u32)
                    ),
                },
            )
            .collect()
    }

    /// Select All, or Unselect All.
    pub fn select_all(&mut self, on: bool) {
        self.compared.iter_mut().for_each(|compared| *compared = on);
    }

    /// The sheet columns compared.
    pub fn columns(&self) -> Vec<u32> {
        self.compared
            .iter()
            .enumerate()
            .filter(|(_, compared)| **compared)
            .map(|(index, _)| self.range.start.column + index as u32)
            .collect()
    }
}

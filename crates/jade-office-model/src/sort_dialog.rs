//! The Data ▸ Sort dialog: levels of keys, each a column of the list and an
//! order, the first sorted by first, and whether the list has a header row
//! and whether case matters.

use jade_xlsx_reader::{CellRange, CellRef};

use crate::sheet::{column_label, Workbook};
use crate::Backend;

/// One level of the Sort dialog: a column of the list, counted from its
/// first, and its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortLevel {
    pub column: usize,
    pub descending: bool,
}

/// What the Sort dialog asks for.
#[derive(Debug, Clone, PartialEq)]
pub struct SortDialog {
    pub range: CellRange,
    /// My data has headers.
    pub header: bool,
    pub case_sensitive: bool,
    pub levels: Vec<SortLevel>,
    /// The header row's names, for when the list has one.
    headers: Vec<String>,
}

/// Excel's limit on sort levels.
pub const MAX_LEVELS: usize = 64;

impl SortDialog {
    /// Opened on the selection, or on the list round the active cell when
    /// only one cell is selected: one level, the active cell's column,
    /// ascending; a header row when the first row reads as one.
    pub fn open<B: Backend>(book: &Workbook<B>, selected: CellRange, at: CellRef) -> Self {
        let range = if selected.start == selected.end {
            book.current_region(at).unwrap_or(selected)
        } else {
            selected
        };
        let headers = (range.start.column..=range.end.column)
            .map(|column| book.display_text(CellRef::new(column, range.start.row)))
            .collect();
        let column = at.column.clamp(range.start.column, range.end.column) - range.start.column;
        Self {
            range,
            header: book.looks_like_headers(range),
            case_sensitive: false,
            levels: vec![SortLevel {
                column: column as usize,
                descending: false,
            }],
            headers,
        }
    }

    /// The columns' names as the dialog lists them: the header row's when
    /// the list has one, else the column letters.
    pub fn column_names(&self) -> Vec<String> {
        (0..self.headers.len())
            .map(|index| {
                let letter = column_label(self.range.start.column + index as u32);
                match self.headers.get(index).filter(|_| self.header) {
                    Some(name) if !name.is_empty() => name.clone(),
                    _ => letter,
                }
            })
            .collect()
    }

    /// Add Level: the first column not sorted by yet, ascending.
    pub fn add_level(&mut self) {
        if self.levels.len() >= MAX_LEVELS {
            return;
        }
        let column = (0..self.headers.len())
            .find(|column| self.levels.iter().all(|level| level.column != *column))
            .unwrap_or(0);
        self.levels.push(SortLevel {
            column,
            descending: false,
        });
    }

    /// Delete Level, keeping one.
    pub fn remove_level(&mut self, index: usize) {
        if self.levels.len() > 1 && index < self.levels.len() {
            self.levels.remove(index);
        }
    }

    /// The keys to sort by, as sheet columns and orders.
    pub fn keys(&self) -> Vec<(u32, bool)> {
        self.levels
            .iter()
            .map(|level| {
                (
                    self.range.start.column + level.column as u32,
                    level.descending,
                )
            })
            .collect()
    }
}

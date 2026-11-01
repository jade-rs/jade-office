//! The Data ▸ Sort dialog: levels of keys, each a column of the list and an
//! order, the first sorted by first, and whether the list has a header row
//! and whether case matters. A level sorts on the values, or on a cell or
//! font colour the column has, put on top or on bottom.

use jade_xlsx_headless_api::SortColor;
use jade_xlsx_reader::{CellRange, CellRef};

use crate::sheet::{column_label, Workbook};
use crate::Backend;

/// What a level sorts on.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SortOn {
    #[default]
    Values,
    /// The cells of this fill colour (`RRGGBB`, empty for No Fill).
    CellColor(String),
    /// The cells whose text is this colour (empty for Automatic).
    FontColor(String),
}

/// One level of the Sort dialog: a column of the list, counted from its
/// first, what it sorts on and its order — for a colour, on bottom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortLevel {
    pub column: usize,
    pub descending: bool,
    pub on: SortOn,
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
    /// Per column, the fill colours and the font colours its cells have, in
    /// the order they first come, for Sort On's choices.
    colors: Vec<(Vec<String>, Vec<String>)>,
}

/// How far down the list the colours offered are looked for.
const COLOR_ROWS: u32 = 2000;

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
        let colors = (range.start.column..=range.end.column)
            .map(|column| {
                let (mut fills, mut fonts) = (Vec::new(), Vec::new());
                let last = range.end.row.min(range.start.row + COLOR_ROWS);
                for row in range.start.row..=last {
                    let format = book.format_at(CellRef::new(column, row));
                    let note = |list: &mut Vec<String>, color: Option<String>| {
                        let color = color.unwrap_or_default();
                        if !list.contains(&color) {
                            list.push(color);
                        }
                    };
                    if let Some(fill) = format.fill_color {
                        note(&mut fills, Some(fill));
                    }
                    note(&mut fonts, format.font_color);
                }
                (fills, fonts)
            })
            .collect();
        Self {
            range,
            header: book.looks_like_headers(range),
            case_sensitive: false,
            levels: vec![SortLevel {
                column: column as usize,
                descending: false,
                on: SortOn::Values,
            }],
            headers,
            colors,
        }
    }

    /// The colours level `level`'s column has for Sort On Cell Color
    /// (`font` false) or Font Color: `RRGGBB`, empty for none.
    pub fn colors(&self, level: usize, font: bool) -> &[String] {
        self.levels
            .get(level)
            .and_then(|level| self.colors.get(level.column))
            .map_or(&[], |(fills, fonts)| if font { fonts } else { fills })
    }

    /// Sort On for level `level`: the values, or a colour — the first its
    /// column has that is not none, as Excel offers it first.
    pub fn sort_on(&mut self, level: usize, colour: Option<bool>) {
        let on = match colour {
            None => SortOn::Values,
            Some(font) => {
                let colors = self.colors(level, font);
                let first = colors
                    .iter()
                    .find(|color| !color.is_empty())
                    .or(colors.first())
                    .cloned()
                    .unwrap_or_default();
                if font {
                    SortOn::FontColor(first)
                } else {
                    SortOn::CellColor(first)
                }
            }
        };
        if let Some(level) = self.levels.get_mut(level) {
            level.on = on;
        }
    }

    /// The colour level `level` puts on top or bottom.
    pub fn set_color(&mut self, level: usize, color: String) {
        if let Some(level) = self.levels.get_mut(level) {
            match &mut level.on {
                SortOn::CellColor(chosen) | SortOn::FontColor(chosen) => *chosen = color,
                SortOn::Values => {}
            }
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
            on: SortOn::Values,
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

    /// The keys with what each sorts on: a colour, or the values.
    pub fn color_keys(&self) -> Vec<(u32, bool, Option<SortColor>)> {
        self.levels
            .iter()
            .map(|level| {
                let color = match &level.on {
                    SortOn::Values => None,
                    SortOn::CellColor(rgb) => Some(SortColor {
                        font: false,
                        rgb: rgb.clone(),
                    }),
                    SortOn::FontColor(rgb) => Some(SortColor {
                        font: true,
                        rgb: rgb.clone(),
                    }),
                };
                (
                    self.range.start.column + level.column as u32,
                    level.descending,
                    color,
                )
            })
            .collect()
    }

    /// OK: the list sorted as the levels say. `false` when no row moved.
    pub fn apply<B: Backend>(&self, book: &mut Workbook<B>) -> Result<bool, String> {
        book.sort_by_colors(
            self.range,
            self.header,
            &self.color_keys(),
            self.case_sensitive,
        )
    }
}

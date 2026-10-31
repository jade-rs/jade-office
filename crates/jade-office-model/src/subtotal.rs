//! Data ▸ Subtotal: a list with a header row, sorted by one of its columns,
//! gets a total row after each run of the same value in that column and a
//! grand total after the last, each a `SUBTOTAL` over the rows above it, and
//! an outline to collapse the detail — as Excel's command makes them.

use jade_xlsx_headless_api::CellInput;
use jade_xlsx_headless_editor::OutlineChange;
use jade_xlsx_reader::{Axis, CellRange, CellRef};

use crate::sheet::{column_label, Workbook};
use crate::Backend;

/// The function a subtotal row applies, as `SUBTOTAL` numbers them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubtotalFunction {
    Sum,
    /// Excel's Count: the cells that are not empty (`COUNTA`).
    Count,
    Average,
    Max,
    Min,
    Product,
    /// Count Numbers (`COUNT`).
    CountNumbers,
    StdDev,
    StdDevP,
    Var,
    VarP,
}

impl SubtotalFunction {
    /// The function's number in `SUBTOTAL`.
    pub fn code(self) -> u8 {
        match self {
            SubtotalFunction::Average => 1,
            SubtotalFunction::CountNumbers => 2,
            SubtotalFunction::Count => 3,
            SubtotalFunction::Max => 4,
            SubtotalFunction::Min => 5,
            SubtotalFunction::Product => 6,
            SubtotalFunction::StdDev => 7,
            SubtotalFunction::StdDevP => 8,
            SubtotalFunction::Sum => 9,
            SubtotalFunction::Var => 10,
            SubtotalFunction::VarP => 11,
        }
    }
}

/// What the Subtotal dialog asks for.
#[derive(Debug, Clone, PartialEq)]
pub struct Subtotals {
    /// At each change in: the column whose runs are totalled.
    pub by: u32,
    pub function: SubtotalFunction,
    /// Add subtotal to: the columns that get a formula.
    pub columns: Vec<u32>,
    /// A group's label, `{}` standing for its value: `{} Total`, `{} 集計`.
    pub label: String,
    /// The grand total's label: `Grand Total`, `総計`.
    pub grand_label: String,
}

impl<B: Backend> Workbook<B> {
    /// Data ▸ Subtotal over `range`, whose first row is the header. One undo
    /// step; `false` when the range has no data row or nothing to total.
    pub fn subtotal(&mut self, range: CellRange, subtotals: &Subtotals) -> Result<bool, String> {
        let (first, last) = (range.start.row + 1, range.end.row);
        if last < first || subtotals.columns.is_empty() {
            return Ok(false);
        }
        // The runs of one value in the column totalled by, as shown.
        let mut groups: Vec<(u32, u32, String)> = Vec::new();
        for row in first..=last {
            let value = self.display_text(CellRef::new(subtotals.by, row));
            match groups.last_mut() {
                Some(group) if group.2 == value => group.1 = row,
                _ => groups.push((row, row, value)),
            }
        }
        let name = self.sheet_name();
        let code = subtotals.function.code();
        self.atomic_edit(|book| {
            let engine = book.engine_mut();
            let mut steps = 0;
            // Bottom up, so the rows above keep their places: the grand
            // total's row, then each group's (rows are 1-based here).
            engine
                .insert_rows(&name, last + 2, 1)
                .map_err(|error| error.to_string())?;
            steps += 1;
            for group in groups.iter().rev() {
                engine
                    .insert_rows(&name, group.1 + 2, 1)
                    .map_err(|error| error.to_string())?;
                steps += 1;
            }
            let at = |column: u32, row: u32| format!("{}{}", column_label(column), row + 1);
            let formula = |column: u32, top: u32, bottom: u32| {
                CellInput::Formula(format!(
                    "=SUBTOTAL({code},{}:{})",
                    at(column, top),
                    at(column, bottom)
                ))
            };
            let mut values = Vec::new();
            let mut details = Vec::new();
            for (shift, (start, end, value)) in groups.iter().enumerate() {
                let (start, end) = (start + shift as u32, end + shift as u32);
                let total = end + 1;
                details.push((start, end));
                values.push((
                    at(subtotals.by, total),
                    CellInput::Text(subtotals.label.replace("{}", value)),
                ));
                for &column in &subtotals.columns {
                    values.push((at(column, total), formula(column, start, end)));
                }
            }
            // SUBTOTAL leaves out the subtotals among the rows it reads.
            let grand = last + groups.len() as u32 + 1;
            values.push((
                at(subtotals.by, grand),
                CellInput::Text(subtotals.grand_label.clone()),
            ));
            for &column in &subtotals.columns {
                values.push((at(column, grand), formula(column, first, grand - 1)));
            }
            engine
                .set_cells(&name, &values)
                .map_err(|error| error.to_string())?;
            steps += 1;
            // Everything above the grand total, then each group above its
            // total: three levels, the detail deepest.
            for (top, bottom) in std::iter::once((first, grand - 1)).chain(details) {
                if engine
                    .outline(&name, Axis::Rows, top, bottom, OutlineChange::Group)
                    .map_err(|error| error.to_string())?
                {
                    steps += 1;
                }
            }
            book.wrote(steps)?;
            Ok(true)
        })
    }
}

impl SubtotalFunction {
    /// Every function, in the dialog's order.
    pub const ALL: [SubtotalFunction; 11] = [
        SubtotalFunction::Sum,
        SubtotalFunction::Count,
        SubtotalFunction::Average,
        SubtotalFunction::Max,
        SubtotalFunction::Min,
        SubtotalFunction::Product,
        SubtotalFunction::CountNumbers,
        SubtotalFunction::StdDev,
        SubtotalFunction::StdDevP,
        SubtotalFunction::Var,
        SubtotalFunction::VarP,
    ];

    /// The function's name in i18n keys: `subtotal.fn.sum`, the dialog's
    /// label; `subtotal.row.sum`, a group's (`{} Total`); and
    /// `subtotal.grand.sum`, the grand total's.
    pub fn key(self) -> &'static str {
        match self {
            SubtotalFunction::Sum => "sum",
            SubtotalFunction::Count => "count",
            SubtotalFunction::Average => "average",
            SubtotalFunction::Max => "max",
            SubtotalFunction::Min => "min",
            SubtotalFunction::Product => "product",
            SubtotalFunction::CountNumbers => "count_numbers",
            SubtotalFunction::StdDev => "stdev",
            SubtotalFunction::StdDevP => "stdevp",
            SubtotalFunction::Var => "var",
            SubtotalFunction::VarP => "varp",
        }
    }
}

/// The Data ▸ Subtotal dialog: the list it works on, named by its header
/// row, and what it asks.
#[derive(Debug, Clone, PartialEq)]
pub struct SubtotalDialog {
    pub range: CellRange,
    /// The header row's names, one per column of the list.
    pub headers: Vec<String>,
    /// At each change in: a column of the list, counted from its first.
    pub by: usize,
    pub function: SubtotalFunction,
    /// Add subtotal to: whether each column gets a formula.
    pub columns: Vec<bool>,
}

impl SubtotalDialog {
    /// Opened on the selection, or on the list round the active cell when
    /// only one cell is selected, with Excel's defaults: the first column's
    /// runs, summed, in the last column.
    pub fn open<B: Backend>(book: &Workbook<B>, selected: CellRange, at: CellRef) -> Self {
        let range = if selected.start == selected.end {
            book.current_region(at).unwrap_or(selected)
        } else {
            selected
        };
        let headers: Vec<String> = (range.start.column..=range.end.column)
            .map(|column| {
                let name = book.display_text(CellRef::new(column, range.start.row));
                if name.is_empty() {
                    column_label(column)
                } else {
                    name
                }
            })
            .collect();
        let mut columns = vec![false; headers.len()];
        if let Some(last) = columns.last_mut() {
            *last = true;
        }
        Self {
            range,
            headers,
            by: 0,
            function: SubtotalFunction::Sum,
            columns,
        }
    }

    /// What OK applies, with the row labels in the user's language; `None`
    /// when no column is ticked.
    pub fn subtotals(&self, label: String, grand_label: String) -> Option<Subtotals> {
        let columns: Vec<u32> = self
            .columns
            .iter()
            .enumerate()
            .filter(|(_, ticked)| **ticked)
            .map(|(index, _)| self.range.start.column + index as u32)
            .collect();
        (!columns.is_empty()).then(|| Subtotals {
            by: self.range.start.column + self.by as u32,
            function: self.function,
            columns,
            label,
            grand_label,
        })
    }
}

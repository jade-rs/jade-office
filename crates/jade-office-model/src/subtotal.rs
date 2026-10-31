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

//! Data ▸ What-If Analysis ▸ Data Table: a formula worked out for each of a
//! list of values in an input cell — or two lists, in two input cells —
//! the results filling the range between, `{=TABLE(row,column)}` in each,
//! as Excel's dialog makes the table.

use jade_xlsx_reader::{CellRange, CellRef, DataTable};

use crate::form::Form;
use crate::sheet::{reference_text, Workbook};
use crate::what_if::{single_cell, Outcome, ToolDialog};
use crate::Backend;

/// What typing into a data table's result cell is refused with, as Excel
/// refuses it.
pub const DATA_TABLE_REFUSED: &str = "\u{1}data-table";

impl<B: Backend> Workbook<B> {
    /// A data table over `range`: input values along its top row and down
    /// its left column, the formulas (or, two-way, the formula in the
    /// corner) round them; `row` takes the values along the top, `column`
    /// those down the side. The results fill the range less its top row and
    /// left column. One undo step.
    pub fn data_table(
        &mut self,
        range: CellRange,
        row: Option<CellRef>,
        column: Option<CellRef>,
    ) -> Result<bool, String> {
        let name = self.sheet_name();
        self.engine_mut()
            .set_data_table(&name, range, row, column)
            .map_err(|error| error.to_string())?;
        self.wrote(1)?;
        Ok(true)
    }

    /// The data table whose results include `at`, on the active sheet.
    pub fn data_table_at(&self, at: CellRef) -> Option<DataTable> {
        self.data_table_on(self.sheet_index(), at)
    }

    pub(crate) fn data_table_on(&self, index: usize, at: CellRef) -> Option<DataTable> {
        self.engine().workbook().sheets[index]
            .data_tables
            .iter()
            .find(|table| {
                let range = table.range;
                (range.start.column..=range.end.column).contains(&at.column)
                    && (range.start.row..=range.end.row).contains(&at.row)
            })
            .copied()
    }
}

/// What the formula bar shows in a data table's cells: `{=TABLE(B1,B2)}`,
/// an input cell left out where the table has none.
pub fn table_formula(table: &DataTable) -> String {
    let (row, column) = table.inputs();
    format!(
        "{{=TABLE({},{})}}",
        row.map(reference_text).unwrap_or_default(),
        column.map(reference_text).unwrap_or_default()
    )
}

/// The Data Table dialog: the row and column input cells, over the range
/// selected.
pub struct DataTableDialog {
    pub form: Form,
    range: CellRange,
}

impl DataTableDialog {
    pub fn open(range: CellRange) -> Self {
        Self {
            form: Form::new("data_table.dialog")
                .field("row", "data_table.row", "")
                .field("column", "data_table.column", "")
                .button("ok", "validation.ok")
                .button("cancel", "validation.cancel"),
            range,
        }
    }
}

impl ToolDialog for DataTableDialog {
    fn form(&self) -> &Form {
        &self.form
    }

    fn form_mut(&mut self) -> &mut Form {
        &mut self.form
    }

    fn press<B: Backend>(
        &mut self,
        button: &str,
        book: &mut Workbook<B>,
    ) -> Result<Outcome, String> {
        if button != "ok" {
            return Ok(Outcome::close());
        }
        let form = &mut self.form;
        form.alert = None;
        let input = |id: &str| -> Result<Option<CellRef>, ()> {
            let text = form.text(id);
            if text.is_empty() {
                return Ok(None);
            }
            single_cell(book, text).map(Some).ok_or(())
        };
        let (Ok(row), Ok(column)) = (input("row"), input("column")) else {
            form.alert = Some("data_table.alert.input".into());
            return Ok(Outcome::stay());
        };
        let range = self.range;
        if row.is_none() && column.is_none() {
            form.alert = Some("data_table.alert.input".into());
            return Ok(Outcome::stay());
        }
        if range.end.row == range.start.row || range.end.column == range.start.column {
            form.alert = Some("data_table.alert.selection".into());
            return Ok(Outcome::stay());
        }
        book.data_table(range, row, column)?;
        Ok(Outcome::closed_writing())
    }
}

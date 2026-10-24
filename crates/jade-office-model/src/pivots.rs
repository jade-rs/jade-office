//! Explicit saved-pivot commands shared by the native and browser editors.
mod creation;
mod dialog;
use crate::{
    sheet::{CellRef, Workbook},
    Backend,
};
pub use creation::{PivotCreation, PivotCreationField, PivotPanel};
pub use dialog::{PivotArea, PivotDialog, PivotDialogField, PivotMember, PIVOT_AGGREGATIONS};
pub use jade_xlsx_headless_editor::{
    PivotCreate, PivotDestination, PivotFieldOptions, PivotFilterSelection, PivotLayout,
    PivotMeasure, PivotMemberFilter, PivotPage, PivotReportLayout, PivotSource, PivotStyle,
    PivotUpdate,
};
use jade_xlsx_reader::pivot::PivotTable;
impl<B: Backend> Workbook<B> {
    pub fn create_pivot_command(
        &mut self,
        options: &PivotCreate,
    ) -> Result<jade_xlsx_headless_editor::PivotCreationSummary, String> {
        self.atomic_edit(|book| {
            if let PivotDestination::Existing { sheet, .. } = &options.destination {
                if book.engine().workbook().sheets.iter().any(|s| {
                    jade_xlsx_reader::formula::same_sheet_name(&s.name, sheet)
                        && s.visibility != jade_xlsx_reader::SheetVisibility::Visible
                }) {
                    return Err("pivot.hidden_destination".into());
                }
            }
            let layout = book.layout_options();
            let summary = book
                .engine_mut()
                .create_pivot_with_layout(options, &layout)
                .map_err(|e| e.to_string())?;
            book.wrote(1)?;
            let index = book
                .engine()
                .workbook()
                .sheets
                .iter()
                .position(|s| s.name == summary.sheet)
                .ok_or("missing created pivot sheet")?;
            book.select_sheet(index)?;
            Ok(summary)
        })
    }
    pub fn delete_pivot_at(&mut self, at: CellRef) -> Result<bool, String> {
        let name = self
            .pivot_at(at)
            .map(|t| t.name.clone())
            .ok_or("select a cell in a pivot table")?;
        self.atomic_edit(|book| {
            let sheet = book.sheet_name();
            book.engine_mut()
                .delete_pivot(&sheet, &name)
                .map_err(|e| e.to_string())?;
            book.wrote(1)?;
            Ok(true)
        })
    }
    pub fn update_pivot_named(&mut self, name: &str, update: &PivotUpdate) -> Result<bool, String> {
        self.atomic_edit(|book| {
            let sheet = book.sheet_name();
            let options = book.layout_options();
            book.engine_mut()
                .update_pivot_with_layout(&sheet, name, update, &options)
                .map_err(|e| e.to_string())?;
            book.wrote(1)?;
            Ok(true)
        })
    }
    pub fn pivot_at(&self, at: CellRef) -> Option<&PivotTable> {
        let part = &self.engine().workbook().sheets[self.sheet_index()].part_name;
        self.engine()
            .workbook()
            .pivots
            .tables
            .iter()
            .find(|t| &t.sheet_part == part && t.location.reference.contains(at))
            .map(|t| t.as_ref())
    }
    pub fn has_pivots(&self) -> bool {
        !self.engine().workbook().pivots.tables.is_empty()
    }
    pub fn refresh_all_pivots(&mut self) -> Result<bool, String> {
        self.atomic_edit(|book| {
            let options = book.layout_options();
            let summaries = book
                .engine_mut()
                .refresh_all_pivots_with_layout(&options)
                .map_err(|e| e.to_string())?;
            if summaries.is_empty() {
                return Ok(false);
            }
            book.wrote(1)?;
            Ok(true)
        })
    }
    pub fn refresh_pivot_at(&mut self, at: CellRef) -> Result<bool, String> {
        let name = self
            .pivot_at(at)
            .map(|t| t.name.clone())
            .ok_or("select a cell in a pivot table")?;
        self.refresh_pivot_named(&name)
    }
    pub fn refresh_pivot_named(&mut self, name: &str) -> Result<bool, String> {
        self.atomic_edit(|book| {
            let sheet = book.sheet_name();
            let options = book.layout_options();
            book.engine_mut()
                .refresh_pivot_with_layout(&sheet, name, &options)
                .map_err(|e| e.to_string())?;
            book.wrote(1)?;
            Ok(true)
        })
    }
}

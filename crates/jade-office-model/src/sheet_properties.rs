use crate::{sheet::Workbook, Backend};
use std::collections::BTreeSet;

use jade_xlsx_reader::{Axis, CellRange, CellRef, PageSetup, SheetVisibility};

impl<B: Backend> Workbook<B> {
    pub fn hidden_sheets(&self) -> Vec<usize> {
        self.engine()
            .workbook()
            .sheets
            .iter()
            .enumerate()
            .filter_map(|(i, s)| (s.visibility == SheetVisibility::Hidden).then_some(i))
            .collect()
    }

    pub fn set_sheet_visibility(
        &mut self,
        index: usize,
        visibility: SheetVisibility,
    ) -> Result<bool, String> {
        let name = self
            .sheet_names()
            .get(index)
            .cloned()
            .ok_or_else(|| format!("no sheet at index {index}"))?;
        if !self
            .engine_mut()
            .set_sheet_visibility(&name, visibility)
            .map_err(|e| e.to_string())?
        {
            return Ok(false);
        }
        // Hidden sheets cannot remain the active UI tab.
        if self.sheet_index() == index && visibility != SheetVisibility::Visible {
            let next = self
                .engine()
                .workbook()
                .active_sheet_index
                .filter(|i| self.sheet_tabs().contains(i))
                .or_else(|| self.sheet_tabs().first().copied())
                .unwrap();
            self.select_sheet(next)?;
        }
        self.wrote(1)?;
        Ok(true)
    }

    pub fn set_tab_color(&mut self, index: usize, color: Option<&str>) -> Result<bool, String> {
        let name = self
            .sheet_names()
            .get(index)
            .cloned()
            .ok_or_else(|| format!("no sheet at index {index}"))?;
        if !self
            .engine_mut()
            .set_tab_color(&name, color)
            .map_err(|e| e.to_string())?
        {
            return Ok(false);
        }
        self.wrote(1)?;
        Ok(true)
    }

    /// Formulas ▸ Show Formulas: whether the active sheet's cells show
    /// their formulas rather than what they work out to.
    pub fn show_formulas(&self) -> bool {
        self.engine().workbook().sheets[self.sheet_index()].show_formulas
    }

    /// Show Formulas on the active sheet turned on or off (⌃`). One undo
    /// step; `false` when it already was.
    pub fn set_show_formulas(&mut self, on: bool) -> Result<bool, String> {
        let name = self.sheet_name();
        if !self
            .engine_mut()
            .set_show_formulas(&name, on)
            .map_err(|e| e.to_string())?
        {
            return Ok(false);
        }
        self.wrote(1)?;
        Ok(true)
    }

    /// The zoom the active sheet was saved at, as a factor (0.85 for 85%):
    /// the one Excel opens it at.
    pub fn sheet_zoom(&self) -> f32 {
        let percent = self.engine().workbook().sheets[self.sheet_index()]
            .zoom_percent
            .unwrap_or(100);
        percent as f32 / 100.0
    }

    /// Whether the active sheet shows a zero in cells worth zero, as Excel
    /// does unless told not to.
    pub fn show_zeros(&self) -> bool {
        self.engine().workbook().sheets[self.sheet_index()].show_zeros
    }

    /// Zeros on the active sheet shown or (`on` false) left blank. One undo
    /// step; `false` when it already was.
    pub fn set_show_zeros(&mut self, on: bool) -> Result<bool, String> {
        let name = self.sheet_name();
        if !self
            .engine_mut()
            .set_show_zeros(&name, on)
            .map_err(|e| e.to_string())?
        {
            return Ok(false);
        }
        self.wrote(1)?;
        Ok(true)
    }

    // ---- page breaks ------------------------------------------------------

    /// The manual page breaks on the active sheet: the rows they sit above
    /// and the columns they sit left of.
    pub(crate) fn manual_breaks(&self) -> (BTreeSet<u32>, BTreeSet<u32>) {
        let sheet = &self.engine().workbook().sheets[self.sheet_index()];
        let manual = |breaks: &[jade_xlsx_reader::PageBreak]| {
            breaks
                .iter()
                .filter(|brk| brk.manual)
                .map(|brk| brk.before)
                .collect::<BTreeSet<u32>>()
        };
        (manual(&sheet.row_breaks), manual(&sheet.column_breaks))
    }

    /// Whether a manual page break sits above `at`'s row or left of its
    /// column: Remove Page Break has one to take away.
    pub fn page_break_at(&self, at: CellRef) -> bool {
        let (rows, columns) = self.manual_breaks();
        rows.contains(&at.row) || columns.contains(&at.column)
    }

    /// Whether the active sheet has any manual page break.
    pub fn has_page_breaks(&self) -> bool {
        let (rows, columns) = self.manual_breaks();
        !rows.is_empty() || !columns.is_empty()
    }

    /// Page Layout ▸ Breaks ▸ Insert Page Break: a break above `at`'s row and
    /// left of its column, as Excel puts them — none above the first row or
    /// left of the first column. One undo step.
    pub fn insert_page_break(&mut self, at: CellRef) -> Result<bool, String> {
        let (mut rows, mut columns) = self.manual_breaks();
        if at.row > 0 {
            rows.insert(at.row);
        }
        if at.column > 0 {
            columns.insert(at.column);
        }
        self.set_breaks(rows, columns)
    }

    /// Remove Page Break: the breaks above `at`'s row and left of its column
    /// taken away. One undo step.
    pub fn remove_page_break(&mut self, at: CellRef) -> Result<bool, String> {
        let (mut rows, mut columns) = self.manual_breaks();
        rows.remove(&at.row);
        columns.remove(&at.column);
        self.set_breaks(rows, columns)
    }

    /// Reset All Page Breaks: every manual break on the sheet taken away.
    pub fn reset_page_breaks(&mut self) -> Result<bool, String> {
        self.set_breaks(BTreeSet::new(), BTreeSet::new())
    }

    pub(crate) fn set_breaks(
        &mut self,
        rows: BTreeSet<u32>,
        columns: BTreeSet<u32>,
    ) -> Result<bool, String> {
        let name = self.sheet_name();
        self.atomic_edit(|book| {
            let mut steps = 0;
            for (axis, breaks) in [(Axis::Rows, &rows), (Axis::Columns, &columns)] {
                if book
                    .engine_mut()
                    .set_manual_breaks(&name, axis, breaks)
                    .map_err(|error| error.to_string())?
                {
                    steps += 1;
                }
            }
            if steps == 0 {
                return Ok(false);
            }
            book.wrote(steps)?;
            Ok(true)
        })
    }

    pub fn page_setup(&self) -> &PageSetup {
        &self.engine().workbook().sheets[self.sheet_index()].page_setup
    }

    pub fn set_page_setup(&mut self, setup: &PageSetup) -> Result<bool, String> {
        let name = self.sheet_name();
        if !self
            .engine_mut()
            .set_page_setup(&name, setup)
            .map_err(|e| e.to_string())?
        {
            return Ok(false);
        }
        self.wrote(1)?;
        Ok(true)
    }

    pub fn set_print_area(&mut self, range: Option<CellRange>) -> Result<bool, String> {
        let name = self.sheet_name();
        if !self
            .engine_mut()
            .set_print_area(&name, range)
            .map_err(|e| e.to_string())?
        {
            return Ok(false);
        }
        self.wrote(1)?;
        Ok(true)
    }
}

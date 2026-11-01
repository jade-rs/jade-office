use crate::{sheet::Workbook, Backend};
use jade_xlsx_reader::{CellRange, PageSetup, SheetVisibility};

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

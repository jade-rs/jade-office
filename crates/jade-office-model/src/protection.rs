//! Workbook protection and its shared native/browser password dialog.
use crate::{sheet::Workbook, text::TextEdit, Backend};
pub use jade_xlsx_headless_editor::{SheetProtection, SheetProtectionInfo, WorkbookProtectionInfo};

impl<B: Backend> Workbook<B> {
    pub fn contents_hidden(&self, at: crate::sheet::CellRef) -> bool {
        self.contents_hidden_on(self.sheet_index(), at)
    }
    pub(crate) fn contents_hidden_on(&self, index: usize, at: crate::sheet::CellRef) -> bool {
        let sheet = &self.engine().workbook().sheets[index];
        sheet.protection.is_some()
            && self
                .engine()
                .workbook()
                .styles
                .cell_formats
                .get(jade_xlsx_headless_editor::effective_style(sheet, at) as usize)
                .is_some_and(|f| f.formula_hidden)
    }
    pub fn can_select_cell(&self, at: crate::sheet::CellRef) -> bool {
        let sheet = &self.engine().workbook().sheets[self.sheet_index()];
        let Some(allowed) = sheet.protection else {
            return true;
        };
        let unlocked = self
            .engine()
            .workbook()
            .styles
            .cell_formats
            .get(jade_xlsx_headless_editor::effective_style(sheet, at) as usize)
            .is_some_and(|f| f.unlocked);
        if unlocked {
            allowed.select_unlocked_cells
        } else {
            allowed.select_locked_cells
        }
    }
    /// Test a whole selection using sparse style bands, including blank cells.
    pub fn can_select_range(&self, range: crate::sheet::CellRange) -> bool {
        if range.start.row > range.end.row
            || range.start.column > range.end.column
            || range.end.row >= jade_xlsx_reader::model::MAX_ROW_COUNT
            || range.end.column >= jade_xlsx_reader::model::MAX_COLUMN_COUNT
        {
            return false;
        }
        let sheet = &self.engine().workbook().sheets[self.sheet_index()];
        let Some(p) = sheet.protection else {
            return true;
        };
        if p.select_locked_cells && p.select_unlocked_cells {
            return true;
        }
        if !p.select_locked_cells && !p.select_unlocked_cells {
            return false;
        }
        let allowed = |style: u32| {
            let unlocked = self
                .engine()
                .workbook()
                .styles
                .cell_formats
                .get(style as usize)
                .is_some_and(|f| f.unlocked);
            if unlocked {
                p.select_unlocked_cells
            } else {
                p.select_locked_cells
            }
        };
        let mut rows = vec![range.start.row, range.end.row + 1];
        for row in &sheet.rows {
            if row.custom_format
                && row.style_index.is_some()
                && range.start.row <= row.index
                && row.index <= range.end.row
            {
                rows.extend([row.index, row.index + 1]);
            }
        }
        let mut columns = vec![range.start.column, range.end.column + 1];
        for column in &sheet.columns {
            if column.style_index.is_some() {
                columns.extend(
                    [column.first, column.last + 1]
                        .into_iter()
                        .filter(|c| range.start.column < *c && *c <= range.end.column),
                );
            }
        }
        rows.sort_unstable();
        rows.dedup();
        columns.sort_unstable();
        columns.dedup();
        for r in rows.windows(2) {
            let first = sheet.rows.partition_point(|row| row.index < r[0]);
            let last = sheet.rows.partition_point(|row| row.index < r[1]);
            let records = &sheet.rows[first..last];
            let row = records.first().filter(|row| row.index == r[0]);
            for c in columns.windows(2) {
                let base = sheet
                    .effective_style_index_in_row(crate::sheet::CellRef::new(c[0], r[0]), None, row)
                    .unwrap_or(0);
                let base_allowed = allowed(base);
                let area = u64::from(r[1] - r[0]) * u64::from(c[1] - c[0]);
                let mut exceptions = std::collections::HashSet::new();
                for row in records {
                    for cell in &row.cells {
                        if c[0] <= cell.reference.column && cell.reference.column < c[1] {
                            if let Some(style) = cell.style_index {
                                if !allowed(style) {
                                    return false;
                                }
                                exceptions.insert(cell.reference);
                            }
                        }
                    }
                }
                if !base_allowed && (exceptions.len() as u64) < area {
                    return false;
                }
            }
        }
        true
    }
    pub fn sheet_protection(&self) -> Result<Option<SheetProtectionInfo>, String> {
        self.engine()
            .sheet_protection(&self.sheet_name())
            .map_err(|e| e.to_string())
    }
    pub fn protect_sheet(
        &mut self,
        password: &str,
        allowed: SheetProtection,
    ) -> Result<bool, String> {
        let mut salt = [0; 16];
        if !password.is_empty() {
            getrandom::getrandom(&mut salt).map_err(|_| "protection.random_failed".to_string())?;
        }
        let name = self.sheet_name();
        if !self
            .engine_mut()
            .protect_sheet(&name, allowed, password, &salt)
            .map_err(|e| e.to_string())?
        {
            return Ok(false);
        }
        self.wrote(1)?;
        Ok(true)
    }
    pub fn unprotect_sheet(&mut self, password: &str) -> Result<bool, String> {
        let name = self.sheet_name();
        if !self
            .engine_mut()
            .unprotect_sheet(&name, password)
            .map_err(|e| e.to_string())?
        {
            return Ok(false);
        }
        self.wrote(1)?;
        Ok(true)
    }
    pub fn workbook_protection(&self) -> Result<Option<WorkbookProtectionInfo>, String> {
        self.engine()
            .workbook_protection()
            .map_err(|e| e.to_string())
    }
    pub fn protect_workbook(&mut self, password: &str) -> Result<bool, String> {
        let mut salt = [0; 16];
        if !password.is_empty() {
            getrandom::getrandom(&mut salt).map_err(|_| "protection.random_failed".to_string())?;
        }
        if !self
            .engine_mut()
            .protect_workbook(password, &salt)
            .map_err(|e| e.to_string())?
        {
            return Ok(false);
        }
        self.wrote(1)?;
        Ok(true)
    }
    pub fn unprotect_workbook(&mut self, password: &str) -> Result<bool, String> {
        if !self
            .engine_mut()
            .unprotect_workbook(password)
            .map_err(|e| e.to_string())?
        {
            return Ok(false);
        }
        self.wrote(1)?;
        Ok(true)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtectionTarget {
    Workbook,
    Sheet,
    Ranges,
}
pub struct ProtectionDialog {
    pub target: ProtectionTarget,
    pub range_editor: Option<crate::editable_ranges::RangeEditor>,
    sheet_name: Option<String>,
    pub allowed: SheetProtection,
    pub unprotect: bool,
    pub focused: usize,
    fields: [TextEdit; 2],
    pub problem: Option<&'static str>,
    pub input_bounds: std::rc::Rc<std::cell::Cell<Option<[f32; 4]>>>,
}
impl ProtectionDialog {
    pub fn open<B: Backend>(book: &Workbook<B>) -> Result<Self, String> {
        Ok(Self {
            target: ProtectionTarget::Workbook,
            range_editor: None,
            sheet_name: None,
            allowed: SheetProtection::default(),
            unprotect: book.workbook_protection()?.is_some_and(|p| p.configured()),
            focused: 0,
            fields: [TextEdit::new(String::new()), TextEdit::new(String::new())],
            problem: None,
            input_bounds: std::rc::Rc::new(std::cell::Cell::new(None)),
        })
    }
    pub fn open_sheet<B: Backend>(book: &Workbook<B>) -> Result<Self, String> {
        let mut dialog = Self {
            sheet_name: Some(book.sheet_name()),
            target: ProtectionTarget::Sheet,
            range_editor: None,
            allowed: SheetProtection::default(),
            unprotect: false,
            focused: 0,
            fields: [TextEdit::new(String::new()), TextEdit::new(String::new())],
            problem: None,
            input_bounds: std::rc::Rc::new(std::cell::Cell::new(None)),
        };
        dialog.target = ProtectionTarget::Sheet;
        dialog.unprotect = book.sheet_protection()?.is_some_and(|p| p.configured());
        Ok(dialog)
    }
    pub fn open_ranges<B: Backend>(
        book: &Workbook<B>,
        range: crate::sheet::CellRange,
        prefix: &str,
    ) -> Result<Self, String> {
        let editor = crate::editable_ranges::RangeEditor::open(book, range, prefix)?;
        Ok(Self {
            target: ProtectionTarget::Ranges,
            unprotect: editor.protected,
            focused: editor.focused,
            range_editor: Some(editor),
            sheet_name: None,
            allowed: SheetProtection::default(),
            fields: [TextEdit::new(String::new()), TextEdit::new(String::new())],
            problem: None,
            input_bounds: std::rc::Rc::new(std::cell::Cell::new(None)),
        })
    }
    /// Password dialog for a refused typed entry. The editor keeps the entry;
    /// successful authentication is session-only and does not itself retry it.
    pub fn for_entry<B: Backend>(
        book: &Workbook<B>,
        at: crate::sheet::CellRef,
        value: &str,
    ) -> Result<Option<Self>, String> {
        let Some(editor) = crate::editable_ranges::RangeEditor::for_entry(book, at, value)? else {
            return Ok(None);
        };
        let mut dialog =
            Self::open_ranges(book, crate::sheet::CellRange { start: at, end: at }, "")?;
        dialog.range_editor = Some(editor);
        Ok(Some(dialog))
    }
    pub fn for_operation<B: Backend>(
        book: &Workbook<B>,
        edit: crate::protected_edits::ProtectedEdit,
    ) -> Result<Option<Self>, String> {
        let Some(editor) = crate::editable_ranges::RangeEditor::for_operation(book, edit)? else {
            return Ok(None);
        };
        let at = crate::sheet::CellRef::new(0, 0);
        let mut dialog =
            Self::open_ranges(book, crate::sheet::CellRange { start: at, end: at }, "")?;
        dialog.unprotect = true;
        dialog.range_editor = Some(editor);
        Ok(Some(dialog))
    }
    pub fn is_operation_prompt(&self) -> bool {
        self.range_editor
            .as_ref()
            .is_some_and(|e| e.is_operation_prompt())
    }
    pub fn is_authentication_prompt(&self) -> bool {
        self.entry_cell().is_some() || self.is_operation_prompt()
    }
    pub fn take_completed_operation(&mut self) -> Option<crate::protected_edits::EditOutcome> {
        self.range_editor
            .as_mut()
            .and_then(|e| e.take_completed_operation())
    }
    pub fn apply_for_operation<B: Backend>(
        &mut self,
        book: &mut Workbook<B>,
        entry: Option<(crate::sheet::CellRef, &str)>,
    ) -> Result<bool, &'static str> {
        if !self
            .range_editor
            .as_ref()
            .is_some_and(|e| e.operation_entry_matches(entry))
        {
            self.problem = Some("protection.range_stale");
            return Err("protection.range_stale");
        }
        self.apply(book)
    }
    pub fn entry_cell(&self) -> Option<crate::sheet::CellRef> {
        self.range_editor.as_ref().and_then(|e| e.entry_cell())
    }
    pub fn entry_reference(&self) -> Option<String> {
        self.entry_cell().map(crate::sheet::reference_text)
    }
    pub fn matches_entry<B: Backend>(
        &self,
        book: &Workbook<B>,
        at: crate::sheet::CellRef,
        value: &str,
    ) -> bool {
        self.range_editor
            .as_ref()
            .is_some_and(|e| e.matches_entry(book, at, value))
    }
    pub fn apply_for_entry<B: Backend>(
        &mut self,
        book: &mut Workbook<B>,
        at: crate::sheet::CellRef,
        value: &str,
    ) -> Result<bool, &'static str> {
        if !self.matches_entry(book, at, value) {
            self.problem = Some("protection.range_stale");
            return Err("protection.range_stale");
        }
        self.apply(book)
    }
    pub fn visible_fields(&self) -> Vec<usize> {
        self.range_editor.as_ref().map_or_else(
            || if self.unprotect { vec![0] } else { vec![0, 1] },
            |e| e.visible_fields(),
        )
    }
    pub fn range_action<B: Backend>(
        &mut self,
        action: crate::editable_ranges::RangeAction,
        book: &mut Workbook<B>,
    ) -> Result<bool, &'static str> {
        self.problem = None;
        let result = self
            .range_editor
            .as_mut()
            .ok_or("protection.unsupported")?
            .action(action, book);
        if let Err(e) = result {
            self.problem = Some(e);
        }
        if let Some(editor) = &self.range_editor {
            self.focused = editor.focused;
        }
        self.input_bounds.set(None);
        result
    }
    pub fn permissions(&self) -> [bool; 15] {
        [
            self.allowed.select_locked_cells,
            self.allowed.select_unlocked_cells,
            self.allowed.format_cells,
            self.allowed.format_columns,
            self.allowed.format_rows,
            self.allowed.insert_columns,
            self.allowed.insert_rows,
            self.allowed.insert_hyperlinks,
            self.allowed.delete_columns,
            self.allowed.delete_rows,
            self.allowed.sort,
            self.allowed.auto_filter,
            self.allowed.pivot_tables,
            self.allowed.objects,
            self.allowed.scenarios,
        ]
    }
    pub fn toggle_permission(&mut self, index: usize) {
        if self.target != ProtectionTarget::Sheet || self.unprotect {
            return;
        }
        match index {
            0 => self.allowed.select_locked_cells = !self.allowed.select_locked_cells,
            1 => self.allowed.select_unlocked_cells = !self.allowed.select_unlocked_cells,
            2 => self.allowed.format_cells = !self.allowed.format_cells,
            3 => self.allowed.format_columns = !self.allowed.format_columns,
            4 => self.allowed.format_rows = !self.allowed.format_rows,
            5 => self.allowed.insert_columns = !self.allowed.insert_columns,
            6 => self.allowed.insert_rows = !self.allowed.insert_rows,
            7 => self.allowed.insert_hyperlinks = !self.allowed.insert_hyperlinks,
            8 => self.allowed.delete_columns = !self.allowed.delete_columns,
            9 => self.allowed.delete_rows = !self.allowed.delete_rows,
            10 => self.allowed.sort = !self.allowed.sort,
            11 => self.allowed.auto_filter = !self.allowed.auto_filter,
            12 => self.allowed.pivot_tables = !self.allowed.pivot_tables,
            13 => self.allowed.objects = !self.allowed.objects,
            14 => self.allowed.scenarios = !self.allowed.scenarios,
            _ => {}
        }
    }
    pub fn field(&self) -> &TextEdit {
        if let Some(editor) = &self.range_editor {
            editor.field()
        } else {
            &self.fields[self.focused]
        }
    }
    pub fn field_mut(&mut self) -> &mut TextEdit {
        if let Some(editor) = &mut self.range_editor {
            editor.field_mut()
        } else {
            &mut self.fields[self.focused]
        }
    }
    pub fn focus(&mut self, index: usize) {
        if let Some(editor) = &mut self.range_editor {
            editor.focus(index);
            self.focused = editor.focused;
        } else {
            self.focused = if self.unprotect { 0 } else { index.min(1) };
        }
        self.field_mut().whole = true;
        self.input_bounds.set(None);
    }
    pub fn masked(&self, index: usize) -> String {
        if let Some(editor) = &self.range_editor {
            return editor.display(index);
        }
        let text = &self.fields[index.min(1)];
        let before = text.text[..text.caret()].chars().count();
        let total = text.text.chars().count();
        if self.focused == index {
            format!("{}│{}", "●".repeat(before), "●".repeat(total - before))
        } else {
            "●".repeat(total)
        }
    }
    pub fn key(&mut self, key: &str, shift: bool, command: bool) {
        if let Some(editor) = &mut self.range_editor {
            editor.key(key, shift, command);
            self.focused = editor.focused;
            self.input_bounds.set(None);
            return;
        }
        if key == "tab" {
            self.focus((self.focused + 1) % 2);
        } else if command && key == "a" {
            self.field_mut().whole = true;
        } else {
            let _ = shift;
            self.field_mut().edit_key(key);
        }
    }
    pub fn apply<B: Backend>(&mut self, book: &mut Workbook<B>) -> Result<bool, &'static str> {
        self.problem = None;
        if let Some(editor) = &mut self.range_editor {
            let result = editor.apply(book);
            if let Err(error) = result {
                self.problem = Some(error);
            }
            self.focused = editor.focused;
            self.input_bounds.set(None);
            return result;
        }
        if self.target == ProtectionTarget::Sheet
            && self.sheet_name.as_deref() != Some(book.sheet_name().as_str())
        {
            self.problem = Some("protection.unsupported");
            return Err("protection.unsupported");
        }
        if !self.unprotect && self.fields[0].text != self.fields[1].text {
            self.problem = Some("protection.mismatch");
            return Err("protection.mismatch");
        }
        if self.fields[0].text.encode_utf16().count() > 255 {
            self.problem = Some("protection.long");
            return Err("protection.long");
        }
        let result = match (self.target, self.unprotect) {
            (ProtectionTarget::Workbook, true) => book.unprotect_workbook(&self.fields[0].text),
            (ProtectionTarget::Workbook, false) => book.protect_workbook(&self.fields[0].text),
            (ProtectionTarget::Sheet, true) => book.unprotect_sheet(&self.fields[0].text),
            (ProtectionTarget::Sheet, false) => {
                book.protect_sheet(&self.fields[0].text, self.allowed)
            }
            (ProtectionTarget::Ranges, _) => return Err("protection.range_unsupported"),
        };
        result.map_err(|error| {
            let key = if error.contains("protection password is incorrect") {
                "protection.incorrect"
            } else if error == "protection.random_failed" {
                "protection.random_failed"
            } else {
                "protection.unsupported"
            };
            self.problem = Some(key);
            key
        })
    }
}

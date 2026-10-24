//! Shared editable-range management and unsaved range authentication.
use crate::{
    sheet::{CellRange, Workbook},
    text::TextEdit,
    Backend,
};
pub use jade_xlsx_reader::protected_ranges::ProtectedRange;
impl<B: Backend> Workbook<B> {
    pub fn editable_ranges(&self) -> Result<Vec<ProtectedRange>, String> {
        self.engine()
            .editable_ranges(&self.sheet_name())
            .map(|r| r.to_vec())
            .map_err(|e| e.to_string())
    }
    pub fn editable_range_unlocked(&self, name: &str) -> Result<bool, String> {
        self.engine()
            .editable_range_unlocked(&self.sheet_name(), name)
            .map_err(|e| e.to_string())
    }
    pub fn unlock_editable_range(&mut self, name: &str, password: &str) -> Result<bool, String> {
        let sheet = self.sheet_name();
        self.engine_mut()
            .unlock_editable_range(&sheet, name, password)
            .map_err(|e| e.to_string())
    }
    pub fn lock_editable_ranges(&mut self) -> Result<bool, String> {
        let sheet = self.sheet_name();
        self.engine_mut()
            .lock_editable_ranges(&sheet)
            .map_err(|e| e.to_string())
    }
    pub fn set_editable_range(
        &mut self,
        old_name: Option<&str>,
        name: &str,
        ranges: &[CellRange],
        password: Option<&str>,
    ) -> Result<bool, String> {
        let mut salt = [0; 16];
        if password.is_some_and(|p| !p.is_empty()) {
            getrandom::getrandom(&mut salt).map_err(|_| "protection.random_failed".to_string())?;
        }
        let sheet = self.sheet_name();
        let changed = self
            .engine_mut()
            .set_editable_range(&sheet, old_name, name, ranges, password.map(|p| (p, &salt)))
            .map_err(|e| e.to_string())?;
        if changed {
            self.wrote(1)?;
        }
        Ok(changed)
    }
    pub fn delete_editable_range(&mut self, name: &str) -> Result<bool, String> {
        let sheet = self.sheet_name();
        let changed = self
            .engine_mut()
            .delete_editable_range(&sheet, name)
            .map_err(|e| e.to_string())?;
        if changed {
            self.wrote(1)?;
        }
        Ok(changed)
    }
}
#[derive(Clone, Copy)]
pub enum RangeAction {
    Select(usize),
    New,
    Delete,
    Password,
    LockAll,
}
pub struct RangeEntry {
    pub key: String,
    pub name: String,
    pub reference: String,
    pub has_password: bool,
    pub unlocked: bool,
    pub ambiguous: bool,
}
struct PendingEntry {
    at: crate::sheet::CellRef,
    text: String,
    definitions: Vec<ProtectedRange>,
    names: Vec<String>,
}
pub struct RangeEditor {
    owner: std::sync::Weak<()>,
    pending: Option<PendingEntry>,
    operation: Option<crate::protected_edits::PendingOperation>,
    pub entries: Vec<RangeEntry>,
    pub selected: Option<String>,
    pub protected: bool,
    pub replace_password: bool,
    pub focused: usize,
    fields: [TextEdit; 4],
    part: String,
    revision: u64,
    prefix: String,
    initial_reference: String,
}
fn text(value: String) -> TextEdit {
    TextEdit::new(value)
}
pub(crate) fn message(error: String) -> &'static str {
    if error.contains("protection password is incorrect") {
        "protection.incorrect"
    } else if error == "protection.random_failed" {
        "protection.random_failed"
    } else if error.contains("another editable range") {
        "protection.range_duplicate"
    } else if error.contains("invalid range name") || error.contains("a range name is required") {
        "protection.range_name"
    } else if error.contains("255 UTF-16") {
        "protection.long"
    } else {
        "protection.range_unsupported"
    }
}
impl RangeEditor {
    pub fn open<B: Backend>(
        book: &Workbook<B>,
        range: CellRange,
        prefix: &str,
    ) -> Result<Self, String> {
        let part = book.engine().workbook().sheets[book.sheet_index()]
            .part_name
            .clone();
        let mut editor = Self {
            owner: book.session_identity(),
            pending: None,
            operation: None,
            entries: Vec::new(),
            selected: None,
            protected: book.engine().workbook().sheets[book.sheet_index()]
                .protection
                .is_some(),
            replace_password: true,
            focused: 0,
            fields: std::array::from_fn(|_| text(String::new())),
            part,
            revision: book.layout_revision(),
            prefix: prefix.into(),
            initial_reference: crate::sheet::range_text(range),
        };
        editor.refresh(book)?;
        editor.new_range();
        if editor.protected && !editor.entries.is_empty() {
            editor.select(0);
        }
        Ok(editor)
    }
    pub fn for_entry<B: Backend>(
        book: &Workbook<B>,
        at: crate::sheet::CellRef,
        value: &str,
    ) -> Result<Option<Self>, String> {
        let candidates = book
            .engine()
            .range_unlock_candidates(&book.sheet_name(), at)
            .map_err(|e| e.to_string())?;
        if candidates.is_empty() {
            return Ok(None);
        }
        let mut editor = Self::open(book, CellRange { start: at, end: at }, "")?;
        editor.pending = Some(PendingEntry {
            at,
            text: value.to_string(),
            definitions: book.editable_ranges()?,
            names: candidates.into_iter().map(|r| r.name).collect(),
        });
        editor.refresh(book)?;
        editor.select(0);
        Ok(Some(editor))
    }
    pub fn for_operation<B: Backend>(
        book: &Workbook<B>,
        edit: crate::protected_edits::ProtectedEdit,
    ) -> Result<Option<Self>, String> {
        let Some(operation) = crate::protected_edits::PendingOperation::new(book, edit)? else {
            return Ok(None);
        };
        let at = crate::sheet::CellRef::new(0, 0);
        let mut editor = Self::open(book, CellRange { start: at, end: at }, "")?;
        editor.protected = true;
        editor.operation = Some(operation);
        editor.refresh_operation();
        Ok(Some(editor))
    }
    fn refresh_operation(&mut self) {
        let Some(operation) = &self.operation else {
            return;
        };
        self.entries = operation
            .candidates
            .iter()
            .map(|c| RangeEntry {
                key: c.key(),
                name: format!("{} — {}", c.sheet, c.definition.name),
                reference: c
                    .definition
                    .ranges
                    .iter()
                    .map(|r| crate::sheet::range_text(*r))
                    .collect::<Vec<_>>()
                    .join(" "),
                has_password: true,
                unlocked: false,
                ambiguous: false,
            })
            .collect();
        self.selected = None;
        if !self.entries.is_empty() {
            self.select(0);
        }
    }
    pub fn is_operation_prompt(&self) -> bool {
        self.operation.is_some()
    }
    pub fn operation_entry_matches(&self, entry: Option<(crate::sheet::CellRef, &str)>) -> bool {
        self.operation
            .as_ref()
            .is_some_and(|op| op.entry_matches(entry))
    }
    pub fn take_completed_operation(&mut self) -> Option<crate::protected_edits::EditOutcome> {
        self.operation.as_mut().and_then(|op| op.completed.take())
    }
    pub fn entry_cell(&self) -> Option<crate::sheet::CellRef> {
        self.pending.as_ref().map(|p| p.at)
    }
    pub fn matches_entry<B: Backend>(
        &self,
        book: &Workbook<B>,
        at: crate::sheet::CellRef,
        value: &str,
    ) -> bool {
        self.check(book).is_ok()
            && self
                .pending
                .as_ref()
                .is_some_and(|p| p.at == at && p.text == value)
    }
    fn refresh<B: Backend>(&mut self, book: &Workbook<B>) -> Result<(), String> {
        self.entries = book
            .editable_ranges()?
            .into_iter()
            .map(|r| {
                let unlocked = book.editable_range_unlocked(&r.name).unwrap_or(false);
                RangeEntry {
                    key: r.name.clone(),
                    name: r.name.clone(),
                    reference: r
                        .ranges
                        .iter()
                        .map(|r| crate::sheet::range_text(*r))
                        .collect::<Vec<_>>()
                        .join(" "),
                    has_password: r.requires_password(),
                    unlocked,
                    ambiguous: r.ambiguous,
                }
            })
            .collect();
        if let Some(pending) = &self.pending {
            self.entries.retain(|e| pending.names.contains(&e.name));
        }
        self.revision = book.layout_revision();
        Ok(())
    }
    fn check<B: Backend>(&self, book: &Workbook<B>) -> Result<(), &'static str> {
        let sheet = &book.engine().workbook().sheets[book.sheet_index()];
        if !book.same_session(&self.owner)
            || self
                .pending
                .as_ref()
                .is_some_and(|pending| book.editable_ranges().as_ref() != Ok(&pending.definitions))
            || self.part != sheet.part_name
            || self.revision != book.layout_revision()
            || (self.operation.is_none() && self.protected != sheet.protection.is_some())
            || self.operation.as_ref().is_some_and(|op| !op.matches(book))
        {
            Err("protection.range_stale")
        } else {
            Ok(())
        }
    }
    fn new_range(&mut self) {
        self.selected = None;
        self.replace_password = true;
        let mut n = 1;
        loop {
            let name = format!("{}{n}", self.prefix);
            if !self
                .entries
                .iter()
                .any(|e| e.name.to_lowercase() == name.to_lowercase())
            {
                self.fields[0] = text(name);
                break;
            }
            n += 1;
        }
        self.fields[1] = text(self.initial_reference.clone());
        self.fields[2] = text(String::new());
        self.fields[3] = text(String::new());
        self.focused = if self.protected { 2 } else { 0 };
        self.focus(self.focused);
    }
    fn select(&mut self, index: usize) {
        if let Some(entry) = self.entries.get(index) {
            self.selected = Some(entry.key.clone());
            self.fields[0] = text(entry.name.clone());
            self.fields[1] = text(entry.reference.clone());
            self.fields[2] = text(String::new());
            self.fields[3] = text(String::new());
            self.replace_password = false;
            self.focused = if self.protected { 2 } else { 0 };
            self.focus(self.focused);
        }
    }
    pub fn visible_fields(&self) -> Vec<usize> {
        if self.protected {
            vec![2]
        } else if self.replace_password {
            vec![0, 1, 2, 3]
        } else {
            vec![0, 1]
        }
    }
    pub fn field(&self) -> &TextEdit {
        &self.fields[self.focused]
    }
    pub fn field_mut(&mut self) -> &mut TextEdit {
        &mut self.fields[self.focused]
    }
    pub fn focus(&mut self, index: usize) {
        let visible = self.visible_fields();
        self.focused = if visible.contains(&index) {
            index
        } else {
            visible[0]
        };
        self.field_mut().whole = true;
    }
    pub fn display(&self, index: usize) -> String {
        let field = &self.fields[index.min(3)];
        let before = field.caret();
        if index < 2 {
            if self.focused == index {
                format!("{}│{}", &field.text[..before], &field.text[before..])
            } else {
                field.text.clone()
            }
        } else {
            let before = field.text[..before].chars().count();
            let total = field.text.chars().count();
            if self.focused == index {
                format!("{}│{}", "●".repeat(before), "●".repeat(total - before))
            } else {
                "●".repeat(total)
            }
        }
    }
    pub fn key(&mut self, key: &str, shift: bool, command: bool) {
        if key == "tab" {
            let visible = self.visible_fields();
            let at = visible.iter().position(|i| *i == self.focused).unwrap_or(0);
            let index = if shift {
                (at + visible.len() - 1) % visible.len()
            } else {
                (at + 1) % visible.len()
            };
            self.focus(visible[index]);
        } else if command && key == "a" {
            self.field_mut().whole = true;
        } else {
            self.field_mut().edit_key(key);
        }
    }
    /// Returns document mutation only. Session grants never mark the file dirty.
    pub fn action<B: Backend>(
        &mut self,
        action: RangeAction,
        book: &mut Workbook<B>,
    ) -> Result<bool, &'static str> {
        self.check(book)?;
        match action {
            RangeAction::Select(index) => self.select(index),
            RangeAction::New if !self.protected => self.new_range(),
            RangeAction::Password if !self.protected && self.selected.is_some() => {
                self.replace_password = !self.replace_password;
                self.fields[2] = text(String::new());
                self.fields[3] = text(String::new());
                if !self.visible_fields().contains(&self.focused) {
                    self.focus(0);
                }
            }
            RangeAction::Delete if !self.protected => {
                let name = self
                    .selected
                    .as_ref()
                    .ok_or("protection.range_select")?
                    .clone();
                let changed = book.delete_editable_range(&name).map_err(message)?;
                self.refresh(book).map_err(message)?;
                self.new_range();
                return Ok(changed);
            }
            RangeAction::LockAll
                if self.protected && self.pending.is_none() && self.operation.is_none() =>
            {
                book.lock_editable_ranges().map_err(message)?;
                self.refresh(book).map_err(message)?;
                self.fields[2] = text(String::new());
            }
            _ => {}
        }
        Ok(false)
    }
    pub fn apply<B: Backend>(&mut self, book: &mut Workbook<B>) -> Result<bool, &'static str> {
        self.check(book)?;
        if let Some(operation) = self.operation.as_mut() {
            if self.fields[2].text.encode_utf16().count() > 255 {
                return Err("protection.long");
            }
            let changed = operation.apply(
                book,
                self.selected.as_deref().ok_or("protection.range_select")?,
                &self.fields[2].text,
            )?;
            self.fields[2] = text(String::new());
            self.refresh_operation();
            return Ok(changed);
        }
        if self.protected {
            let name = self
                .selected
                .as_ref()
                .ok_or("protection.range_select")?
                .clone();
            if self
                .pending
                .as_ref()
                .is_some_and(|pending| !pending.names.contains(&name))
            {
                return Err("protection.range_stale");
            }
            if self.fields[2].text.encode_utf16().count() > 255 {
                return Err("protection.long");
            }
            let changed = book
                .unlock_editable_range(&name, &self.fields[2].text)
                .map_err(message)?;
            self.refresh(book).map_err(message)?;
            self.fields[2] = text(String::new());
            return Ok(changed);
        }
        if self.replace_password && self.fields[2].text != self.fields[3].text {
            return Err("protection.mismatch");
        }
        if self.replace_password && self.fields[2].text.encode_utf16().count() > 255 {
            return Err("protection.long");
        }
        let name = self.fields[0].text.trim().to_string();
        let ranges = parse_references(&self.fields[1].text).ok_or("protection.range_reference")?;
        let password = self
            .replace_password
            .then_some(self.fields[2].text.as_str());
        let changed = book
            .set_editable_range(self.selected.as_deref(), &name, &ranges, password)
            .map_err(message)?;
        self.refresh(book).map_err(message)?;
        if let Some(index) = self.entries.iter().position(|e| e.name == name) {
            self.select(index);
        }
        Ok(changed)
    }
}
/// The editor accepts comma-separated areas and complete axes; XML uses explicit local rectangles.
pub fn parse_references(value: &str) -> Option<Vec<CellRange>> {
    let normalized = value.replace(',', " ");
    let mut result = Vec::new();
    for area in normalized.split_whitespace() {
        if let Some(mut ranges) = jade_xlsx_reader::protected_ranges::parse_sqref(area) {
            result.append(&mut ranges);
            continue;
        }
        let (a, b) = area.split_once(':')?;
        let a = a.replace('$', "");
        let b = b.replace('$', "");
        let range = if a.chars().all(|c| c.is_ascii_alphabetic())
            && b.chars().all(|c| c.is_ascii_alphabetic())
        {
            let start = crate::sheet::CellRef::parse(&format!("{a}1")).ok()?;
            let mut end = crate::sheet::CellRef::parse(&format!("{b}1")).ok()?;
            end.row = jade_xlsx_reader::model::MAX_ROW_COUNT - 1;
            CellRange { start, end }
        } else if a.chars().all(|c| c.is_ascii_digit()) && b.chars().all(|c| c.is_ascii_digit()) {
            let start = crate::sheet::CellRef::parse(&format!("A{a}")).ok()?;
            let mut end = crate::sheet::CellRef::parse(&format!("A{b}")).ok()?;
            end.column = jade_xlsx_reader::model::MAX_COLUMN_COUNT - 1;
            CellRange { start, end }
        } else {
            return None;
        };
        if range.start.row > range.end.row || range.start.column > range.end.column {
            return None;
        }
        result.push(range);
    }
    (!result.is_empty()).then_some(result)
}

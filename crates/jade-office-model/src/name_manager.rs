//! Formulas ▸ Name Manager: the workbook's names listed with what they
//! refer to and their scope, the selected one's reference edited in place
//! (Enter keeps it) or the name deleted, as Excel's dialog does.

use jade_xlsx_reader::DefinedName;

use crate::sheet::Workbook;
use crate::text::TextEdit;
use crate::Backend;

/// One listed name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NameRow {
    pub name: String,
    /// What it refers to, as Excel shows it: with its `=`.
    pub refers_to: String,
    /// The sheet it belongs to, by index; `None` for the workbook.
    pub scope: Option<usize>,
}

/// What a key did in the dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameManagerKey {
    Take,
    Close,
    Handled,
    Unhandled,
}

/// The dialog: which name is selected and its reference being edited.
pub struct NameManager {
    pub selected: Option<usize>,
    pub refers_to: TextEdit,
}

impl NameManager {
    /// Opened with the first name selected, when there is one.
    pub fn open<B: Backend>(book: &Workbook<B>) -> Self {
        let mut dialog = Self {
            selected: None,
            refers_to: TextEdit::new(String::new()),
        };
        dialog.select(book, 0);
        dialog
    }

    /// The names the dialog lists: those the user made, not Excel's own
    /// (`_xlnm.Print_Area` and the like) or hidden ones, by name.
    pub fn rows<B: Backend>(book: &Workbook<B>) -> Vec<NameRow> {
        let mut rows: Vec<NameRow> = book
            .defined_names()
            .iter()
            .filter(|name| !name.hidden && !name.name.starts_with("_xlnm."))
            .map(|name| NameRow {
                name: name.name.clone(),
                refers_to: format!("={}", name.formula),
                scope: name.local_sheet_index,
            })
            .collect();
        rows.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then(a.scope.cmp(&b.scope))
        });
        rows
    }

    /// Selects row `index`, its reference ready to edit.
    pub fn select<B: Backend>(&mut self, book: &Workbook<B>, index: usize) {
        let rows = Self::rows(book);
        self.selected = (index < rows.len()).then_some(index);
        let text = rows
            .get(index)
            .map(|row| row.refers_to.clone())
            .unwrap_or_default();
        self.refers_to = TextEdit::new(text);
    }

    /// Keys: Enter keeps the edited reference, Escape closes, ↑ and ↓ move
    /// through the names, the rest edit the reference.
    pub fn key<B: Backend>(
        &mut self,
        book: &Workbook<B>,
        key: &str,
        command: bool,
    ) -> NameManagerKey {
        if self.refers_to.marked.is_some() {
            return NameManagerKey::Unhandled;
        }
        let count = Self::rows(book).len();
        match key {
            "enter" => NameManagerKey::Take,
            "escape" => NameManagerKey::Close,
            "up" | "down" if count > 0 => {
                let at = self.selected.unwrap_or(0);
                let next = if key == "up" {
                    at.saturating_sub(1)
                } else {
                    (at + 1).min(count - 1)
                };
                self.select(book, next);
                NameManagerKey::Handled
            }
            "a" if command => {
                self.refers_to.whole = true;
                NameManagerKey::Handled
            }
            key if self.refers_to.edit_key(key) => NameManagerKey::Handled,
            _ => NameManagerKey::Unhandled,
        }
    }

    /// The selected name made to refer to what is typed. `false` when
    /// nothing is selected or it already does.
    pub fn apply<B: Backend>(&self, book: &mut Workbook<B>) -> Result<bool, String> {
        let Some(row) = self
            .selected
            .and_then(|at| Self::rows(book).into_iter().nth(at))
        else {
            return Ok(false);
        };
        let formula = self.refers_to.text.trim().trim_start_matches('=').trim();
        if formula.is_empty() {
            return Err("a name must refer to something".into());
        }
        if format!("={formula}") == row.refers_to {
            return Ok(false);
        }
        book.set_defined_name(&DefinedName {
            name: row.name,
            formula: formula.to_string(),
            local_sheet_index: row.scope,
            hidden: false,
        })
    }

    /// The selected name deleted; the one after it (or before, at the end)
    /// selected next.
    pub fn delete<B: Backend>(&mut self, book: &mut Workbook<B>) -> Result<bool, String> {
        let Some(at) = self.selected else {
            return Ok(false);
        };
        let Some(row) = Self::rows(book).into_iter().nth(at) else {
            return Ok(false);
        };
        let deleted = book.delete_defined_name(&row.name, row.scope)?;
        let left = Self::rows(book).len();
        self.select(book, at.min(left.saturating_sub(1)));
        Ok(deleted)
    }
}

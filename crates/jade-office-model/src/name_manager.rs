//! Formulas ▸ Name Manager: the workbook's names listed with what they
//! refer to and their scope, the selected one's reference edited in place
//! (Enter keeps it) or the name deleted, as Excel's dialog does; New… makes
//! a name for the workbook or one sheet, and Filter narrows the list.

use jade_xlsx_reader::{quote_sheet_name, CellRange, CellRef, DefinedName};

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

/// Name Manager's Filter: which of the names the list shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NameFilter {
    #[default]
    All,
    /// Names scoped to a worksheet.
    Worksheet,
    /// Names scoped to the workbook.
    Workbook,
    /// Names whose reference is an error, `#REF!` and the like.
    Errors,
    WithoutErrors,
}

impl NameFilter {
    /// The menu's choices, in Excel's order.
    pub const ALL: [NameFilter; 5] = [
        NameFilter::All,
        NameFilter::Worksheet,
        NameFilter::Workbook,
        NameFilter::Errors,
        NameFilter::WithoutErrors,
    ];

    /// The i18n key of its name.
    pub fn label(self) -> &'static str {
        match self {
            NameFilter::All => "name_manager.filter_all",
            NameFilter::Worksheet => "name_manager.filter_worksheet",
            NameFilter::Workbook => "name_manager.filter_workbook",
            NameFilter::Errors => "name_manager.filter_errors",
            NameFilter::WithoutErrors => "name_manager.filter_no_errors",
        }
    }

    fn shows(self, row: &NameRow) -> bool {
        match self {
            NameFilter::All => true,
            NameFilter::Worksheet => row.scope.is_some(),
            NameFilter::Workbook => row.scope.is_none(),
            NameFilter::Errors => is_error(&row.refers_to),
            NameFilter::WithoutErrors => !is_error(&row.refers_to),
        }
    }
}

/// Whether a reference holds one of Excel's error values outside quotes,
/// as a name left pointing at a deleted sheet or cells does.
fn is_error(refers_to: &str) -> bool {
    const ERRORS: [&str; 7] = [
        "#REF!", "#NAME?", "#VALUE!", "#DIV/0!", "#N/A", "#NUM!", "#NULL!",
    ];
    let mut quoted = false;
    refers_to.char_indices().any(|(at, character)| {
        if character == '"' || character == '\'' {
            quoted = !quoted;
        }
        !quoted
            && character == '#'
            && ERRORS
                .iter()
                .any(|error| refers_to[at..].to_ascii_uppercase().starts_with(error))
    })
}

/// New Name: the name being made, its scope and which field takes typing
/// (the reference is the dialog's `refers_to`).
pub struct NewName {
    pub name: TextEdit,
    /// The sheet it is for, by index; `None` for the workbook.
    pub scope: Option<usize>,
    /// Typing goes to the name rather than the reference.
    pub on_name: bool,
}

/// What a key did in the dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameManagerKey {
    Take,
    Close,
    Handled,
    Unhandled,
}

/// The dialog: which name is selected and its reference being edited,
/// the filter, and New Name while one is being made.
pub struct NameManager {
    pub selected: Option<usize>,
    pub refers_to: TextEdit,
    pub filter: NameFilter,
    pub new: Option<NewName>,
}

impl NameManager {
    /// Opened with the first name selected, when there is one.
    pub fn open<B: Backend>(book: &Workbook<B>) -> Self {
        let mut dialog = Self {
            selected: None,
            refers_to: TextEdit::new(String::new()),
            filter: NameFilter::All,
            new: None,
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

    /// The names the filter lets the list show, in the list's order; the
    /// selection counts rows of this.
    pub fn listed<B: Backend>(&self, book: &Workbook<B>) -> Vec<NameRow> {
        let mut rows = Self::rows(book);
        rows.retain(|row| self.filter.shows(row));
        rows
    }

    /// Shows only the names `filter` lets through, the first selected.
    pub fn set_filter<B: Backend>(&mut self, book: &Workbook<B>, filter: NameFilter) {
        self.filter = filter;
        self.select(book, 0);
    }

    /// The field typing goes to.
    pub fn field_mut(&mut self) -> &mut TextEdit {
        match self.new.as_mut() {
            Some(new) if new.on_name => &mut new.name,
            _ => &mut self.refers_to,
        }
    }

    pub fn field(&self) -> &TextEdit {
        match self.new.as_ref() {
            Some(new) if new.on_name => &new.name,
            _ => &self.refers_to,
        }
    }

    /// New…: an empty name for the workbook, referring to `selection` on
    /// the active sheet, absolute, as Excel's New Name starts.
    pub fn start_new<B: Backend>(&mut self, book: &Workbook<B>, selection: CellRange) {
        let absolute = |cell: CellRef| {
            format!(
                "${}${}",
                jade_xlsx_layout::column_label(cell.column),
                cell.row + 1
            )
        };
        let mut reference = format!(
            "={}!{}",
            quote_sheet_name(&book.sheet_name()),
            absolute(selection.start)
        );
        if selection.end != selection.start {
            reference.push(':');
            reference.push_str(&absolute(selection.end));
        }
        self.refers_to = TextEdit::new(reference);
        self.new = Some(NewName {
            name: TextEdit::new(String::new()),
            scope: None,
            on_name: true,
        });
    }

    /// New Name's Cancel: back to the list as it was.
    pub fn cancel_new<B: Backend>(&mut self, book: &Workbook<B>) {
        self.new = None;
        self.select(book, self.selected.unwrap_or(0));
    }

    /// New Name's Scope.
    pub fn set_scope(&mut self, scope: Option<usize>) {
        if let Some(new) = self.new.as_mut() {
            new.scope = scope;
        }
    }

    /// Which New Name field typing goes to: the name, or the reference.
    pub fn focus_name(&mut self, on_name: bool) {
        if let Some(new) = self.new.as_mut() {
            new.on_name = on_name;
        }
    }

    /// Selects row `index`, its reference ready to edit.
    pub fn select<B: Backend>(&mut self, book: &Workbook<B>, index: usize) {
        let rows = self.listed(book);
        self.selected = (index < rows.len()).then_some(index);
        let text = rows
            .get(index)
            .map(|row| row.refers_to.clone())
            .unwrap_or_default();
        self.refers_to = TextEdit::new(text);
    }

    /// Keys: Enter keeps the edited reference, Escape closes, ↑ and ↓ move
    /// through the names, the rest edit the reference. Making a name, Tab
    /// moves between the name and its reference and Escape goes back to the
    /// list.
    pub fn key<B: Backend>(
        &mut self,
        book: &Workbook<B>,
        key: &str,
        command: bool,
    ) -> NameManagerKey {
        if self.field().marked.is_some() {
            return NameManagerKey::Unhandled;
        }
        if let Some(on_name) = self.new.as_ref().map(|new| new.on_name) {
            return match key {
                "enter" => NameManagerKey::Take,
                "escape" => {
                    self.cancel_new(book);
                    NameManagerKey::Handled
                }
                "tab" => {
                    self.focus_name(!on_name);
                    NameManagerKey::Handled
                }
                "a" if command => {
                    self.field_mut().whole = true;
                    NameManagerKey::Handled
                }
                key if self.field_mut().edit_key(key) => NameManagerKey::Handled,
                _ => NameManagerKey::Unhandled,
            };
        }
        let count = self.listed(book).len();
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
    /// nothing is selected or it already does. Making a name, the name
    /// made — refused when its scope has one by that name — and selected.
    pub fn apply<B: Backend>(&mut self, book: &mut Workbook<B>) -> Result<bool, String> {
        let formula = self.refers_to.text.trim().trim_start_matches('=').trim();
        if formula.is_empty() {
            return Err("a name must refer to something".into());
        }
        if let Some(new) = self.new.as_ref() {
            let name = new.name.text.trim().to_string();
            if name.is_empty() {
                return Err("type a name".into());
            }
            if book.defined_names().iter().any(|taken| {
                taken.local_sheet_index == new.scope
                    && taken.name.to_lowercase() == name.to_lowercase()
            }) {
                return Err("this name already exists".into());
            }
            let scope = new.scope;
            let made = book.set_defined_name(&DefinedName {
                name: name.clone(),
                formula: formula.to_string(),
                local_sheet_index: scope,
                hidden: false,
            })?;
            self.new = None;
            self.filter = NameFilter::All;
            let at = self
                .listed(book)
                .iter()
                .position(|row| row.name == name && row.scope == scope)
                .unwrap_or(0);
            self.select(book, at);
            return Ok(made);
        }
        let Some(row) = self
            .selected
            .and_then(|at| self.listed(book).into_iter().nth(at))
        else {
            return Ok(false);
        };
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
        let Some(row) = self.listed(book).into_iter().nth(at) else {
            return Ok(false);
        };
        let deleted = book.delete_defined_name(&row.name, row.scope)?;
        let left = self.listed(book).len();
        self.select(book, at.min(left.saturating_sub(1)));
        Ok(deleted)
    }
}

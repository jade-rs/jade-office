//! Custom lists, as Excel's Edit Custom Lists keeps them: its built-in
//! lists of days and months, then the user's own — typed in, or imported
//! from cells — for the Sort dialog's Order ▸ Custom List, which sorts a
//! column in a list's order.

use std::sync::Mutex;

use jade_xlsx_reader::{CellRange, CellRef};

use crate::form::{Control, Form, Label};
use crate::what_if::{Outcome, ToolDialog};
use crate::{sheet::Workbook, Backend};

/// The user's own lists, for every workbook open: Excel keeps them for the
/// user rather than in a workbook.
static USER_LISTS: Mutex<Vec<Vec<String>>> = Mutex::new(Vec::new());

/// Excel's built-in lists.
pub fn built_in() -> Vec<Vec<String>> {
    jade_xlsx_headless_editor::fill::BUILT_IN_LISTS
        .iter()
        .map(|list| list.iter().map(|entry| entry.to_string()).collect())
        .collect()
}

/// The user's lists.
pub fn user_lists() -> Vec<Vec<String>> {
    USER_LISTS
        .lock()
        .map(|lists| lists.clone())
        .unwrap_or_default()
}

/// The user's lists, replaced.
pub fn set_user_lists(lists: Vec<Vec<String>>) {
    if let Ok(mut kept) = USER_LISTS.lock() {
        *kept = lists;
    }
}

/// Every list the Sort dialog offers: the built-in ones, then the user's.
pub fn all() -> Vec<Vec<String>> {
    let mut lists = built_in();
    lists.extend(user_lists());
    lists
}

/// A list as the dialogs show it: `Sun, Mon, Tue, …`.
pub fn line(list: &[String]) -> String {
    list.join(", ")
}

/// List entries as typed: one an entry, split at commas and line ends,
/// blanks left out.
pub fn entries(text: &str) -> Vec<String> {
    text.split([',', '\n', '、'])
        .map(|entry| entry.trim().to_string())
        .filter(|entry| !entry.is_empty())
        .collect()
}

/// The Custom Lists dialog: the lists, List entries for the one picked (or
/// a new one), Add, Delete and Import list from cells.
pub struct CustomListsDialog {
    pub form: Form,
    /// NEW LIST, in the user's language.
    new_list: String,
}

/// The line that starts a new list, first in Custom lists.
const NEW_LIST: usize = 0;

impl CustomListsDialog {
    pub fn open(selected: CellRange) -> Self {
        let mut dialog = Self {
            form: Form::new("custom_lists.dialog")
                .list("lists", "custom_lists.lists", Vec::new())
                .field("entries", "custom_lists.entries", "")
                .field(
                    "import",
                    "custom_lists.import",
                    &crate::what_if::absolute_range(selected),
                )
                .button("ok", "validation.ok")
                .button("add", "custom_lists.add")
                .button("delete", "custom_lists.delete")
                .button("import_cells", "custom_lists.import_button")
                .button("cancel", "validation.cancel"),
            new_list: "NEW LIST".to_string(),
        };
        dialog.refresh(None);
        dialog
    }

    /// Custom lists anew: NEW LIST, the built-in ones, the user's; `pick`
    /// the line chosen.
    fn refresh(&mut self, pick: Option<usize>) {
        let lines = std::iter::once(self.new_list.clone())
            .chain(all().iter().map(|list| line(list)))
            .collect();
        self.form.set_list("lists", lines);
        if let Some(Control::List { chosen, .. }) = self
            .form
            .items
            .iter_mut()
            .find(|item| item.id == "lists")
            .map(|item| &mut item.control)
        {
            *chosen = Some(pick.unwrap_or(NEW_LIST));
        }
        self.show_chosen();
    }

    /// List entries shows the list picked, a list of its entries.
    fn show_chosen(&mut self) {
        let text = match self.form.chosen("lists") {
            Some(index) if index > NEW_LIST => all()
                .get(index - 1)
                .map(|list| line(list))
                .unwrap_or_default(),
            _ => String::new(),
        };
        self.form.set_text("entries", &text);
    }

    /// The user's list a line of Custom lists is, by its place among them.
    fn user_index(&self) -> Option<usize> {
        let index = self.form.chosen("lists")?.checked_sub(1)?;
        index.checked_sub(built_in().len())
    }
}

impl ToolDialog for CustomListsDialog {
    fn form(&self) -> &Form {
        &self.form
    }

    fn form_mut(&mut self) -> &mut Form {
        &mut self.form
    }

    fn set_words(&mut self, words: fn(&'static str) -> &'static str) {
        self.new_list = words("custom_lists.new").to_string();
        let pick = self.form.chosen("lists");
        self.refresh(pick);
    }

    fn changed<B: Backend>(
        &mut self,
        id: &'static str,
        _book: &mut Workbook<B>,
    ) -> Result<Outcome, String> {
        if id == "lists" {
            self.show_chosen();
        }
        Ok(Outcome::stay())
    }

    fn press<B: Backend>(
        &mut self,
        button: &str,
        book: &mut Workbook<B>,
    ) -> Result<Outcome, String> {
        self.form.alert = None;
        let built = built_in().len();
        match button {
            "add" | "ok" => {
                let typed = entries(self.form.text("entries"));
                let mut lists = user_lists();
                let place = match self.user_index() {
                    // The user's list picked: changed to what is typed.
                    Some(index) if index < lists.len() => {
                        if typed.is_empty() {
                            lists.remove(index);
                            None
                        } else {
                            lists[index] = typed;
                            Some(index)
                        }
                    }
                    _ if typed.is_empty() || self.form.chosen("lists") != Some(NEW_LIST) => None,
                    _ => {
                        lists.push(typed);
                        Some(lists.len() - 1)
                    }
                };
                set_user_lists(lists);
                if button == "ok" {
                    return Ok(Outcome::close());
                }
                self.refresh(place.map(|index| index + built + 1));
                Ok(Outcome::stay())
            }
            "delete" => {
                let Some(index) = self.user_index() else {
                    // A built-in list cannot be deleted, as in Excel.
                    self.form.alert = Some(Label::Key("custom_lists.alert.built_in"));
                    return Ok(Outcome::stay());
                };
                let mut lists = user_lists();
                if index < lists.len() {
                    lists.remove(index);
                }
                set_user_lists(lists);
                self.refresh(None);
                Ok(Outcome::stay())
            }
            "import_cells" => {
                let Some((sheet, range)) = book.resolve_reference(self.form.text("import")) else {
                    self.form.alert = Some(Label::Key("custom_lists.alert.import"));
                    return Ok(Outcome::stay());
                };
                // A column or a row of cells, each one an entry, blanks left
                // out.
                let names = book.sheet_names();
                let name = names.get(sheet).cloned().unwrap_or_default();
                let mut list = Vec::new();
                for row in range.start.row..=range.end.row {
                    for column in range.start.column..=range.end.column {
                        let at = crate::sheet::reference_text(CellRef::new(column, row));
                        let text = book
                            .engine()
                            .cell(&name, &at)
                            .map(|cell| cell.display)
                            .unwrap_or_default();
                        if !text.trim().is_empty() {
                            list.push(text.trim().to_string());
                        }
                    }
                }
                if list.is_empty() {
                    self.form.alert = Some(Label::Key("custom_lists.alert.import"));
                    return Ok(Outcome::stay());
                }
                let mut lists = user_lists();
                lists.push(list);
                let place = lists.len() - 1;
                set_user_lists(lists);
                self.refresh(Some(place + built + 1));
                Ok(Outcome::stay())
            }
            _ => Ok(Outcome::close()),
        }
    }
}

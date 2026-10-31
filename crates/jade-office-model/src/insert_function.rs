//! Excel's Insert Function (Shift+F3): the functions searched by name, the
//! one picked shown with its arguments, and OK starting the formula with it
//! in the active cell.

use crate::formula_complete::function_names;
use crate::text::TextEdit;

/// What the list shows before anything is typed: Excel's Most Recently
/// Used to start with.
const COMMON: &[&str] = &[
    "SUM",
    "AVERAGE",
    "IF",
    "HYPERLINK",
    "COUNT",
    "MAX",
    "SIN",
    "SUMIF",
    "PMT",
    "STDEV",
];

/// How many functions the list shows at most.
pub const MAX_SHOWN: usize = 12;

/// What a key did in the dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsertFunctionKey {
    Take,
    Close,
    Handled,
    Unhandled,
}

/// The dialog: what is typed to search for, and which match is picked.
pub struct InsertFunctionDialog {
    pub search: TextEdit,
    pub picked: usize,
}

impl Default for InsertFunctionDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl InsertFunctionDialog {
    pub fn new() -> Self {
        Self {
            search: TextEdit::new(String::new()),
            picked: 0,
        }
    }

    /// The functions the search finds: those whose names start with it,
    /// then those that only contain it; the common ones before anything is
    /// typed.
    pub fn matches(&self) -> Vec<&'static str> {
        let typed = self.search.text.trim().to_ascii_uppercase();
        let names = function_names();
        if typed.is_empty() {
            return COMMON
                .iter()
                .filter_map(|common| names.iter().find(|name| name == common))
                .map(String::as_str)
                .collect();
        }
        let starting = names.iter().filter(|name| name.starts_with(&typed));
        let containing = names
            .iter()
            .filter(|name| !name.starts_with(&typed) && name.contains(&typed));
        starting
            .chain(containing)
            .take(MAX_SHOWN)
            .map(String::as_str)
            .collect()
    }

    /// The function picked, if the search finds any.
    pub fn chosen(&self) -> Option<&'static str> {
        let matches = self.matches();
        matches
            .get(self.picked.min(matches.len().saturating_sub(1)))
            .copied()
    }

    /// The dialog's keys: Enter takes the function picked, Escape closes, Up
    /// and Down pick, the rest edit the search (which picks the first again).
    pub fn key(&mut self, key: &str, command: bool) -> InsertFunctionKey {
        if self.search.marked.is_some() {
            return InsertFunctionKey::Unhandled;
        }
        match key {
            "enter" => InsertFunctionKey::Take,
            "escape" => InsertFunctionKey::Close,
            "down" => {
                self.picked = (self.picked + 1).min(self.matches().len().saturating_sub(1));
                InsertFunctionKey::Handled
            }
            "up" => {
                self.picked = self.picked.saturating_sub(1);
                InsertFunctionKey::Handled
            }
            "a" if command => {
                self.search.whole = true;
                InsertFunctionKey::Handled
            }
            key if self.search.edit_key(key) => {
                self.picked = 0;
                InsertFunctionKey::Handled
            }
            _ => InsertFunctionKey::Unhandled,
        }
    }
}

/// What the cell being typed into becomes with `name` put in: at the caret
/// of a formula already being typed, or a new formula with it.
pub fn insert_into(edit: Option<&mut TextEdit>, name: &str) -> Option<String> {
    match edit {
        Some(edit) if edit.text.starts_with('=') => {
            edit.insert(&format!("{name}("));
            None
        }
        _ => Some(format!("={name}(")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_search_finds_names_starting_then_containing_it() {
        let mut dialog = InsertFunctionDialog::new();
        assert_eq!(dialog.matches()[..3], ["SUM", "AVERAGE", "IF"]);
        dialog.search.insert("look");
        let found = dialog.matches();
        assert_eq!(found[0], "LOOKUP");
        assert!(found.contains(&"VLOOKUP") && found.contains(&"XLOOKUP"));
        assert_eq!(dialog.key("down", false), InsertFunctionKey::Handled);
        assert_eq!(dialog.chosen(), Some(found[1]));
        dialog.key("backspace", false);
        assert_eq!(dialog.picked, 0, "typing picks the first again");
        assert_eq!(dialog.key("enter", false), InsertFunctionKey::Take);
    }

    #[test]
    fn the_function_starts_a_formula_or_goes_in_at_the_caret() {
        assert_eq!(insert_into(None, "SUM").as_deref(), Some("=SUM("));
        let mut text = TextEdit::new("abc".into());
        assert_eq!(
            insert_into(Some(&mut text), "SUM").as_deref(),
            Some("=SUM(")
        );
        let mut formula = TextEdit::new("=1+".into());
        assert_eq!(insert_into(Some(&mut formula), "ROUND"), None);
        assert_eq!(formula.text, "=1+ROUND(");
    }
}

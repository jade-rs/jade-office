//! Excel's Find and Replace (⌘F, ⌃H): what to find and what to put in its
//! place, Match case, Match entire cell contents, and Within: Sheet or
//! Workbook; Find Next and Find Previous, Replace and Replace All.
//!
//! Find looks where Excel's default *Look in: Formulas* looks: a cell's
//! formula or its value as typed, and also what it shows, so a date is
//! found by the date it reads as. Replace works on what the formula bar
//! shows — the formula with its `=`, or the value as typed — and writes
//! the result back as if typed, so replacing `1` with `2` in 100 gives the
//! number 200. Replace All is one write and one undo.
//!
//! [`FindBar`] is the bar both shells show: its two fields and options.

use jade_xlsx_headless_api::CellInput;
use jade_xlsx_reader::{CellRef, SheetVisibility};

use crate::sheet::{reference_text, Workbook};
use crate::text::TextEdit;
use crate::{typing, Backend};

/// Find and Replace's options, as Excel's dialog has them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FindOptions {
    /// Match case: `abc` does not find `ABC`.
    pub match_case: bool,
    /// Match entire cell contents: `10` does not find `100`.
    pub entire_cell: bool,
    /// Within: Workbook, rather than Excel's default Sheet.
    pub workbook: bool,
    /// Look in: Formulas only — a cell's formula or value as typed, not
    /// what it shows — which is where Replace looks, and so Find Next too
    /// while the Replace field shows.
    pub formulas: bool,
}

/// The find bar: what is typed to find and to replace with, which field
/// typing goes into, and the options.
pub struct FindBar {
    pub find: TextEdit,
    pub replace: TextEdit,
    /// The Replace field shows: opened with ⌃H, or turned on in the bar.
    pub replacing: bool,
    /// Typing goes into the Replace field.
    pub in_replace: bool,
    pub options: FindOptions,
}

impl FindBar {
    /// A bar for Find (⌘F), or Find and Replace (⌃H).
    pub fn new(replacing: bool) -> Self {
        Self {
            find: TextEdit::new(String::new()),
            replace: TextEdit::new(String::new()),
            replacing,
            in_replace: false,
            options: FindOptions::default(),
        }
    }

    /// The field typing goes into.
    pub fn field(&self) -> &TextEdit {
        if self.in_replace && self.replacing {
            &self.replace
        } else {
            &self.find
        }
    }

    pub fn field_mut(&mut self) -> &mut TextEdit {
        if self.in_replace && self.replacing {
            &mut self.replace
        } else {
            &mut self.find
        }
    }

    /// The options Find and Replace go by: the bar's, looking in formulas
    /// only while the Replace field shows, as Excel's Replace tab does.
    pub fn options(&self) -> FindOptions {
        FindOptions {
            formulas: self.replacing,
            ..self.options
        }
    }

    /// Tab: from Find to Replace and back, when Replace shows.
    pub fn next_field(&mut self) {
        self.in_replace = self.replacing && !self.in_replace;
    }

    /// The Replace field shown or hidden; hidden, typing goes to Find.
    pub fn toggle_replacing(&mut self) {
        self.replacing = !self.replacing;
        self.in_replace = self.replacing;
    }
}

/// `haystack` with every `needle` in it replaced, or `None` when there is
/// none; Match case decides whether case counts, Match entire cell whether
/// only the whole of it does.
pub fn replaced(
    haystack: &str,
    needle: &str,
    replacement: &str,
    options: FindOptions,
) -> Option<String> {
    if needle.is_empty() {
        return None;
    }
    if options.entire_cell {
        return same(haystack, needle, options.match_case).then(|| replacement.to_string());
    }
    let mut out = String::with_capacity(haystack.len());
    let mut rest = haystack;
    let mut found = false;
    while !rest.is_empty() {
        match match_at(rest, needle, options.match_case) {
            Some(length) => {
                out.push_str(replacement);
                rest = &rest[length..];
                found = true;
            }
            None => {
                let ch = rest.chars().next().unwrap_or_default();
                out.push(ch);
                rest = &rest[ch.len_utf8()..];
            }
        }
    }
    found.then_some(out)
}

/// The byte length of `needle` at the start of `text`, if it is there.
fn match_at(text: &str, needle: &str, match_case: bool) -> Option<usize> {
    let mut chars = text.char_indices();
    for wanted in needle.chars() {
        let (_, got) = chars.next()?;
        if !same_char(got, wanted, match_case) {
            return None;
        }
    }
    Some(chars.next().map_or(text.len(), |(at, _)| at))
}

fn same_char(a: char, b: char, match_case: bool) -> bool {
    a == b || (!match_case && a.to_lowercase().eq(b.to_lowercase()))
}

fn same(a: &str, b: &str, match_case: bool) -> bool {
    a.chars().count() == b.chars().count()
        && a.chars()
            .zip(b.chars())
            .all(|(x, y)| same_char(x, y, match_case))
}

impl<B: Backend> Workbook<B> {
    /// The cells Find would stop at, as (sheet index, cell), by rows: on
    /// this sheet, or on every sheet a reader can see when Within is
    /// Workbook.
    pub fn find_all(&self, needle: &str, options: FindOptions) -> Vec<(usize, CellRef)> {
        if needle.is_empty() {
            return Vec::new();
        }
        let here = self.sheet_index();
        let engine = self.engine();
        let sheets = &engine.workbook().sheets;
        let mut hits: Vec<(usize, CellRef)> = engine
            .find(needle, options.match_case)
            .into_iter()
            .filter(|hit| options.workbook || hit.sheet_index == here)
            .filter(|hit| sheets[hit.sheet_index].visibility == SheetVisibility::Visible)
            .filter_map(|hit| {
                let cell = CellRef::parse(&hit.reference).ok()?;
                // What the formula bar shows: the formula, or the value as
                // typed (`1000` for a 1,000.00).
                let typed = self.edit_text_on(hit.sheet_index, cell);
                let in_typed = replaced(&typed, needle, needle, options).is_some();
                let in_shown = if options.entire_cell {
                    same(&hit.display, needle, options.match_case)
                } else {
                    true
                };
                (in_typed || (!options.formulas && in_shown)).then_some((hit.sheet_index, cell))
            })
            .collect();
        // Excel searches by rows.
        hits.sort_by_key(|(sheet, cell)| (*sheet, cell.row, cell.column));
        hits
    }

    /// Find Next (or, `back`, Find Previous): the match after `from`, round
    /// to the first again, as (sheet index, cell).
    pub fn find_next(
        &self,
        needle: &str,
        options: FindOptions,
        from: (usize, CellRef),
        back: bool,
    ) -> Option<(usize, CellRef)> {
        let hits = self.find_all(needle, options);
        let key = |(sheet, cell): &(usize, CellRef)| (*sheet, cell.row, cell.column);
        let here = key(&from);
        if back {
            hits.iter()
                .rev()
                .find(|hit| key(hit) < here)
                .or_else(|| hits.last())
                .copied()
        } else {
            hits.iter()
                .find(|hit| key(hit) > here)
                .or_else(|| hits.first())
                .copied()
        }
    }

    /// Replace: the cell at `at` on this sheet with `needle` replaced in
    /// what its formula bar shows, if it has it. Returns whether it did;
    /// the shell then finds the next, as Excel's button does.
    pub fn replace_at(
        &mut self,
        at: CellRef,
        needle: &str,
        replacement: &str,
        options: FindOptions,
    ) -> Result<bool, String> {
        let here = self.sheet_index();
        let options = FindOptions {
            formulas: true,
            ..options
        };
        if !self.find_all(needle, options).contains(&(here, at)) {
            return Ok(false);
        }
        let Some(text) = replaced(&self.edit_text(at), needle, replacement, options) else {
            return Ok(false);
        };
        self.set_cell(at, &text)
    }

    /// Replace All: every match on the sheet (or in the workbook) replaced
    /// in one write, one undo. Returns how many cells changed. A formula
    /// the replacement breaks is left as it was, as Excel leaves it.
    pub fn replace_all(
        &mut self,
        needle: &str,
        replacement: &str,
        options: FindOptions,
    ) -> Result<usize, String> {
        let options = FindOptions {
            formulas: true,
            ..options
        };
        let hits = self.find_all(needle, options);
        let mut writes: Vec<(String, String, CellInput)> = Vec::new();
        let names = self.sheet_names();
        let date1904 = matches!(self.date_system(), jade_xlsx_reader::DateSystem::Nineteen04);
        for (sheet, cell) in hits {
            let typed = self.edit_text_on(sheet, cell);
            let Some(text) = replaced(&typed, needle, replacement, options) else {
                continue;
            };
            if let Some(formula) = text.strip_prefix('=').filter(|formula| !formula.is_empty()) {
                if typing::formula_problem(formula) {
                    continue;
                }
            }
            let input = typing::read(&text, date1904).input;
            if let CellInput::Formula(formula) = &input {
                if jade_xlsx_headless_editor::formula_text::wrong_argument_count(formula) {
                    continue;
                }
            }
            writes.push((names[sheet].clone(), reference_text(cell), input));
        }
        if writes.is_empty() {
            return Ok(0);
        }
        self.engine_mut()
            .set_workbook_cells(&writes)
            .map_err(|error| error.to_string())?;
        self.wrote(1)?;
        Ok(writes.len())
    }
}

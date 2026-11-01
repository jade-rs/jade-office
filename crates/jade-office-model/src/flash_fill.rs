//! Data ▸ Flash Fill (⌘E, Ctrl+E): the column the active cell is in filled
//! down its list by the pattern its typed examples follow — each example
//! put together from words of the cells beside it, as they are or in upper,
//! lower or proper case, with fixed text between: `Tanaka, Taro` from
//! `Taro` and `Tanaka`, `T. Tanaka` from `Taro Tanaka`.
//!
//! The pattern has to give every example as typed; the cells it fills are
//! text, and a row it cannot make anything of is left empty.

use jade_xlsx_headless_api::CellInput;
use jade_xlsx_reader::{CellRange, CellRef};

use crate::sheet::{reference_text, Workbook};
use crate::Backend;

/// How a word is cased in the result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Case {
    AsIs,
    Upper,
    Lower,
    Proper,
}

impl Case {
    const ALL: [Case; 4] = [Case::AsIs, Case::Upper, Case::Lower, Case::Proper];

    fn apply(self, text: &str) -> String {
        match self {
            Case::AsIs => text.to_string(),
            Case::Upper => text.to_uppercase(),
            Case::Lower => text.to_lowercase(),
            Case::Proper => {
                let mut chars = text.chars();
                match chars.next() {
                    Some(first) => first
                        .to_uppercase()
                        .chain(chars.flat_map(char::to_lowercase))
                        .collect(),
                    None => String::new(),
                }
            }
        }
    }
}

/// Which part of a neighbouring cell a piece takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    /// All of it.
    Whole,
    /// Its `n`th word, counted from the first (0) or, negative, the last.
    Word(isize),
    /// The first letter of that word.
    Initial(isize),
}

/// One piece of the pattern.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Piece {
    Text(String),
    From {
        column: usize,
        part: Part,
        case: Case,
    },
}

/// The words of `text`: runs of letters and digits.
fn words(text: &str) -> Vec<&str> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect()
}

fn word_at<'a>(words: &[&'a str], n: isize) -> Option<&'a str> {
    let index = if n < 0 {
        words.len().checked_sub(n.unsigned_abs())?
    } else {
        n as usize
    };
    words.get(index).copied()
}

impl Piece {
    /// What the piece gives for a row whose neighbouring cells are `row`.
    fn give(&self, row: &[String]) -> Option<String> {
        match self {
            Piece::Text(text) => Some(text.clone()),
            Piece::From { column, part, case } => {
                let text = row.get(*column)?;
                let taken = match part {
                    Part::Whole => text.as_str(),
                    Part::Word(n) => word_at(&words(text), *n)?,
                    Part::Initial(n) => {
                        let word = word_at(&words(text), *n)?;
                        &word[..word.chars().next()?.len_utf8()]
                    }
                };
                (!taken.is_empty()).then(|| case.apply(taken))
            }
        }
    }
}

fn run(pattern: &[Piece], row: &[String]) -> Option<String> {
    pattern.iter().map(|piece| piece.give(row)).collect()
}

/// The pieces that can stand at the start of `rest` in a row whose
/// neighbouring cells are `row`, longest first, fixed text last.
fn candidates(row: &[String], rest: &str) -> Vec<(Piece, usize)> {
    let mut found = Vec::new();
    for (column, text) in row.iter().enumerate() {
        let list = words(text);
        let count = list.len() as isize;
        let mut parts = vec![Part::Whole];
        for n in 0..count {
            for part in [
                Part::Word(n),
                Part::Word(n - count),
                Part::Initial(n),
                Part::Initial(n - count),
            ] {
                parts.push(part);
            }
        }
        for part in parts {
            for case in Case::ALL {
                let piece = Piece::From { column, part, case };
                if let Some(given) = piece.give(row) {
                    if !given.is_empty() && rest.starts_with(&given) {
                        found.push((piece, given.len()));
                    }
                }
            }
        }
    }
    found.sort_by_key(|(_, length)| std::cmp::Reverse(*length));
    let first = rest.chars().next().map_or(0, char::len_utf8);
    if first > 0 {
        found.push((Piece::Text(rest[..first].to_string()), first));
    }
    found
}

/// How many patterns are tried before giving up.
const SEARCH: usize = 20_000;

/// A pattern giving `outputs[i]` from `rows[i]` for every example, built
/// on the first and checked on the rest.
fn learn(rows: &[Vec<String>], outputs: &[String]) -> Option<Vec<Piece>> {
    fn search(
        rows: &[Vec<String>],
        outputs: &[String],
        at: usize,
        pattern: &mut Vec<Piece>,
        budget: &mut usize,
    ) -> bool {
        if *budget == 0 {
            return false;
        }
        *budget -= 1;
        let target = &outputs[0];
        if at == target.len() {
            return rows
                .iter()
                .zip(outputs)
                .all(|(row, output)| run(pattern, row).as_deref() == Some(output.as_str()));
        }
        for (piece, length) in candidates(&rows[0], &target[at..]) {
            // Fixed text runs on as one piece.
            let merged = match (&piece, pattern.last_mut()) {
                (Piece::Text(next), Some(Piece::Text(text))) => {
                    text.push_str(next);
                    true
                }
                _ => {
                    pattern.push(piece.clone());
                    false
                }
            };
            if search(rows, outputs, at + length, pattern, budget) {
                return true;
            }
            match (merged, &piece) {
                (true, Piece::Text(next)) => {
                    if let Some(Piece::Text(text)) = pattern.last_mut() {
                        text.truncate(text.len() - next.len());
                    }
                }
                _ => {
                    pattern.pop();
                }
            }
        }
        false
    }
    let mut pattern = Vec::new();
    let mut budget = SEARCH;
    search(rows, outputs, 0, &mut pattern, &mut budget).then_some(pattern)
}

impl<B: Backend> Workbook<B> {
    /// Flash Fill down the list round `at` in `at`'s column, from the
    /// examples typed there: the rest of its rows that have something
    /// beside them filled by the pattern, one undo step. Returns the cells
    /// filled; `None` when there is no example, nothing to fill or no
    /// pattern gives every example.
    pub fn flash_fill(&mut self, at: CellRef) -> Result<Option<CellRange>, String> {
        let Some(region) = self.current_region(at) else {
            return Ok(None);
        };
        let sources: Vec<u32> = (region.start.column..=region.end.column)
            .filter(|column| *column != at.column)
            .collect();
        if sources.is_empty() {
            return Ok(None);
        }
        // Each row: what is beside it, and what is typed in the column.
        let rows: Vec<(u32, Vec<String>, String)> = (region.start.row..=region.end.row)
            .map(|row| {
                let beside = sources
                    .iter()
                    .map(|column| self.display_text(CellRef::new(*column, row)))
                    .collect();
                (row, beside, self.display_text(CellRef::new(at.column, row)))
            })
            .collect();
        // The first row is a header when it reads as one, or when no pattern
        // gives it along with the rest of the examples.
        let header = self.looks_like_header(region);
        let learned = [header, true]
            .into_iter()
            .filter(|skip| *skip || !header)
            .find_map(|skip| {
                let body = &rows[usize::from(skip).min(rows.len())..];
                let (examples, typed): (Vec<Vec<String>>, Vec<String>) = body
                    .iter()
                    .filter(|(_, _, typed)| !typed.is_empty())
                    .map(|(_, beside, typed)| (beside.clone(), typed.clone()))
                    .unzip();
                if typed.is_empty() {
                    return None;
                }
                learn(&examples, &typed).map(|pattern| (pattern, body))
            });
        let Some((pattern, body)) = learned else {
            return Ok(None);
        };
        let first = body.first().map_or(region.start.row, |(row, _, _)| *row);
        let empty: Vec<(u32, Vec<String>)> = body
            .iter()
            .filter(|(_, beside, typed)| {
                typed.is_empty() && beside.iter().any(|text| !text.is_empty())
            })
            .map(|(row, beside, _)| (*row, beside.clone()))
            .collect();
        let values: Vec<(String, CellInput)> = empty
            .iter()
            .filter_map(|(row, beside)| {
                let text = run(&pattern, beside)?;
                Some((
                    reference_text(CellRef::new(at.column, *row)),
                    CellInput::Text(text),
                ))
            })
            .collect();
        if values.is_empty() {
            return Ok(None);
        }
        let name = self.sheet_name();
        self.engine_mut()
            .set_cells(&name, &values)
            .map_err(|error| error.to_string())?;
        self.wrote(1)?;
        let rows = empty.iter().map(|(row, _)| *row);
        let (top, bottom) = (
            rows.clone().min().unwrap_or(first),
            rows.max().unwrap_or(first),
        );
        Ok(Some(CellRange {
            start: CellRef::new(at.column, top),
            end: CellRef::new(at.column, bottom),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(cells: &[&[&str]]) -> Vec<Vec<String>> {
        cells
            .iter()
            .map(|row| row.iter().map(|text| text.to_string()).collect())
            .collect()
    }

    #[test]
    fn patterns_come_from_words_case_and_fixed_text() {
        // Last name, first name.
        let pattern = learn(&rows(&[&["Taro Tanaka"]]), &["Tanaka, Taro".to_string()]).unwrap();
        assert_eq!(
            run(&pattern, &["Hanako Suzuki".to_string()]).as_deref(),
            Some("Suzuki, Hanako")
        );
        // An initial and the last word in capitals, from two columns.
        let pattern = learn(&rows(&[&["taro", "tanaka"]]), &["T. TANAKA".to_string()]).unwrap();
        assert_eq!(
            run(&pattern, &["jiro".to_string(), "sato".to_string()]).as_deref(),
            Some("J. SATO")
        );
        // The last word, whatever comes before it: two examples tell.
        let pattern = learn(
            &rows(&[&["Mary Ann Smith"], &["Bob Lee"]]),
            &["Smith".to_string(), "Lee".to_string()],
        )
        .unwrap();
        assert_eq!(
            run(&pattern, &["Jean Paul Marie Dupont".to_string()]).as_deref(),
            Some("Dupont")
        );
        // A domain out of an address.
        let pattern = learn(&rows(&[&["ann@example.com"]]), &["example.com".to_string()]).unwrap();
        assert_eq!(
            run(&pattern, &["bob@jade.dev".to_string()]).as_deref(),
            Some("jade.dev")
        );
    }

    #[test]
    fn examples_no_pattern_gives_are_left_alone() {
        assert!(learn(
            &rows(&[&["a"], &["b"]]),
            &["x".to_string(), "y".to_string()]
        )
        .is_none());
    }
}

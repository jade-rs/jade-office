//! Formulas ▸ Trace Precedents and Trace Dependents: the cells and ranges
//! on the same sheet a formula reads, and the formulas that read a cell,
//! which Excel draws blue arrows between.

use std::rc::Rc;

use jade_xlsx_reader::{CellRange, CellRef};

use crate::sheet::Workbook;
use crate::Backend;

/// A sheet's formulas, each with the cells and ranges it reads there.
pub(crate) type References = Rc<Vec<(CellRef, Vec<CellRange>)>>;

/// The references in `formula` (with or without its `=`) to cells and
/// ranges of its own sheet: `A1`, `$B$2`, `C3:D9`. A reference to another
/// sheet, a name, a function or text in quotes is not one.
pub fn formula_precedents(formula: &str) -> Vec<CellRange> {
    let text = formula.strip_prefix('=').unwrap_or(formula);
    let bytes = text.as_bytes();
    let is_part = |b: u8| b.is_ascii_alphanumeric() || b == b'$';
    let mut found = Vec::new();
    let mut quoted = false;
    let mut at = 0;
    while at < bytes.len() {
        let b = bytes[at];
        if b == b'"' {
            quoted = !quoted;
            at += 1;
            continue;
        }
        if quoted || !is_part(b) {
            at += 1;
            continue;
        }
        // A token of letters, digits and `$`, and a second one after `:`.
        let start = at;
        while at < bytes.len() && is_part(bytes[at]) {
            at += 1;
        }
        let mut end = at;
        if at < bytes.len() && bytes[at] == b':' {
            let mut second = at + 1;
            while second < bytes.len() && is_part(bytes[second]) {
                second += 1;
            }
            if second > at + 1 {
                end = second;
            }
        }
        let before = text[..start].chars().last();
        // Another sheet's (`Sheet2!A1`), a name's tail, or a function.
        if matches!(before, Some('!' | '.' | '_')) || bytes.get(end) == Some(&b'(') {
            at = end;
            continue;
        }
        if let Some(range) = token_range(&text[start..end]) {
            if !found.contains(&range) {
                found.push(range);
            }
        }
        at = end;
    }
    found
}

/// Whether `formula` reads another sheet (`Sheet2!A1`, `'Q1 data'!B2:C9`):
/// a `!` outside text in quotes.
pub fn formula_reads_other_sheets(formula: &str) -> bool {
    let mut quoted = false;
    formula.chars().any(|c| {
        if c == '"' {
            quoted = !quoted;
        }
        c == '!' && !quoted
    })
}

/// `A1`, `$B$2` or `C3:D9` as the range it names, corners in order.
fn token_range(token: &str) -> Option<CellRange> {
    let token: String = token.chars().filter(|c| *c != '$').collect();
    match token.split_once(':') {
        Some((first, last)) => CellRef::parse(first)
            .ok()
            .zip(CellRef::parse(last).ok())
            .map(|(a, b)| CellRange {
                start: CellRef::new(a.column.min(b.column), a.row.min(b.row)),
                end: CellRef::new(a.column.max(b.column), a.row.max(b.row)),
            }),
        None => CellRef::parse(&token).ok().map(|cell| CellRange {
            start: cell,
            end: cell,
        }),
    }
}

/// The references in `formula` to cells of named sheets: each sheet's name
/// (`'Q1 data'` unquoted, `''` read as `'`) with the range read there.
/// A reference into another workbook (`[1]Sheet1!A1`) is not one.
pub fn formula_sheet_references(formula: &str) -> Vec<(String, CellRange)> {
    let text = formula.strip_prefix('=').unwrap_or(formula);
    let chars: Vec<char> = text.chars().collect();
    let is_part = |c: char| c.is_ascii_alphanumeric() || c == '$';
    let mut found = Vec::new();
    let mut quoted = false;
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        if c == '"' {
            quoted = !quoted;
        }
        if quoted || c != '!' {
            at += 1;
            continue;
        }
        // The sheet's name, back from the `!`.
        let name = if at > 0 && chars[at - 1] == '\'' {
            let mut name = Vec::new();
            let mut back = at - 1;
            let mut closed = false;
            while back > 0 {
                back -= 1;
                if chars[back] == '\'' {
                    if back > 0 && chars[back - 1] == '\'' {
                        name.push('\'');
                        back -= 1;
                        continue;
                    }
                    closed = true;
                    break;
                }
                name.push(chars[back]);
            }
            name.reverse();
            closed.then(|| name.into_iter().collect::<String>())
        } else {
            let mut back = at;
            while back > 0
                && (chars[back - 1].is_alphanumeric() || matches!(chars[back - 1], '_' | '.'))
            {
                back -= 1;
            }
            let external = back > 0 && chars[back - 1] == ']';
            (back < at && !external).then(|| chars[back..at].iter().collect::<String>())
        };
        // The range after it.
        let mut end = at + 1;
        while end < chars.len() && is_part(chars[end]) {
            end += 1;
        }
        if end < chars.len() && chars[end] == ':' {
            let mut second = end + 1;
            while second < chars.len() && is_part(chars[second]) {
                second += 1;
            }
            if second > end + 1 {
                end = second;
            }
        }
        let token: String = chars[at + 1..end].iter().collect();
        if let (Some(name), Some(range)) = (name, token_range(&token)) {
            found.push((name, range));
        }
        at = end.max(at + 1);
    }
    found
}

impl<B: Backend> Workbook<B> {
    /// Trace Precedents from `at`: what its formula reads on this sheet;
    /// none for a value.
    pub fn precedents(&self, at: CellRef) -> Vec<CellRange> {
        let typed = self.edit_text(at);
        if !typed.starts_with('=') {
            return Vec::new();
        }
        formula_precedents(&typed)
    }

    /// Whether `at`'s formula reads another sheet, which Trace Precedents
    /// shows as an arrow from a worksheet icon.
    pub fn reads_other_sheets(&self, at: CellRef) -> bool {
        let typed = self.edit_text(at);
        typed.starts_with('=') && formula_reads_other_sheets(&typed)
    }

    /// Trace Dependents from `at`: the cells on this sheet whose formulas
    /// read it, by itself or in a range, in row order.
    pub fn dependents(&self, at: CellRef) -> Vec<CellRef> {
        self.references()
            .iter()
            .filter(|(cell, reads)| *cell != at && reads.iter().any(|range| range.contains(at)))
            .map(|(cell, _)| *cell)
            .collect()
    }

    /// Whether a formula on another sheet reads `at`, which Trace
    /// Dependents shows as an arrow to a worksheet icon.
    pub fn read_by_other_sheets(&self, at: CellRef) -> bool {
        self.read_elsewhere().iter().any(|range| range.contains(at))
    }

    /// What formulas on the other sheets read of the active one, gathered
    /// once per layout revision.
    fn read_elsewhere(&self) -> Rc<Vec<CellRange>> {
        let revision = self.layout_revision();
        if let Some((at, ranges)) = &*self.read_elsewhere.borrow() {
            if *at == revision {
                return ranges.clone();
            }
        }
        let workbook = self.engine().workbook();
        let name = self.sheet_name();
        let ranges: Rc<Vec<CellRange>> = Rc::new(
            workbook
                .sheets
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != self.sheet_index())
                .flat_map(|(_, sheet)| sheet.rows.iter().flat_map(|row| row.cells.iter()))
                .filter_map(|cell| cell.formula.as_deref())
                .flat_map(formula_sheet_references)
                .filter(|(sheet, _)| sheet.eq_ignore_ascii_case(&name))
                .map(|(_, range)| range)
                .collect(),
        );
        *self.read_elsewhere.borrow_mut() = Some((revision, ranges.clone()));
        ranges
    }

    /// The active sheet's formulas and what each reads, gathered once per
    /// layout revision.
    fn references(&self) -> References {
        let revision = self.layout_revision();
        if let Some((at, references)) = &*self.references.borrow() {
            if *at == revision {
                return references.clone();
            }
        }
        let sheet = &self.engine().workbook().sheets[self.sheet_index()];
        let references: References = Rc::new(
            sheet
                .rows
                .iter()
                .flat_map(|row| row.cells.iter())
                .filter(|cell| cell.formula.is_some())
                .map(|cell| (cell.reference, self.precedents(cell.reference)))
                .filter(|(_, reads)| !reads.is_empty())
                .collect(),
        );
        *self.references.borrow_mut() = Some((revision, references.clone()));
        references
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(a: &str, b: &str) -> CellRange {
        CellRange {
            start: CellRef::parse(a).unwrap(),
            end: CellRef::parse(b).unwrap(),
        }
    }

    #[test]
    fn references_to_other_sheets_name_the_sheet_and_the_range() {
        assert_eq!(
            formula_sheet_references("=SUM('Q1 ''data'''!B2:C9)+Totals!$A$1-\"x!A1\"+[1]Ext!A2"),
            vec![
                ("Q1 'data'".to_string(), range("B2", "C9")),
                ("Totals".to_string(), range("A1", "A1")),
            ]
        );
    }

    #[test]
    fn a_bang_outside_text_reads_another_sheet() {
        assert!(formula_reads_other_sheets("=Sheet2!A1+1"));
        assert!(formula_reads_other_sheets("=SUM('Q1 data'!B2:C9)"));
        assert!(!formula_reads_other_sheets("=A1&\"!\""));
        assert!(!formula_reads_other_sheets("=SUM(A1:B2)"));
    }

    #[test]
    fn a_formula_reads_its_cells_and_ranges() {
        assert_eq!(
            formula_precedents("=SUM(B2:C3)+$D$4*2"),
            [range("B2", "C3"), range("D4", "D4")]
        );
        // Another sheet, quoted text, functions and repeats are left out.
        assert_eq!(
            formula_precedents("=Sheet2!A1+\"A1\"+LOG10(E5)+E5"),
            [range("E5", "E5")]
        );
        assert!(formula_precedents("=TODAY()").is_empty());
        assert_eq!(formula_precedents("=C9:A1"), [range("A1", "C9")]);
    }
}

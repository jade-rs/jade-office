//! Selecting on a sheet as Excel does: a range with an active cell in it,
//! what a click on a cell or a header selects, what a drag from there
//! reaches, and where Enter and Tab go inside a range.
//!
//! The shells keep a [`Selection`] and turn their own mouse and key events
//! into these moves; what the data decides — a ⌘-arrow's stop, the used
//! range — is asked of [`crate::sheet::Workbook`].

use crate::sheet::{span, CellRange, CellRef, MAX_COLUMN, MAX_ROW};

/// What is under the pointer on a sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    Cell(CellRef),
    /// A row header: the whole row.
    Row(u32),
    /// A column header: the whole column.
    Column(u32),
    /// The corner where the headers meet: the whole sheet.
    All,
}

impl Hit {
    /// The cell this hit puts the selection's corner on, as a drag
    /// reaches it: a header's row or column at the far edge of the sheet.
    fn corner(self, toward: CellRef) -> CellRef {
        match self {
            Hit::Cell(cell) => cell,
            Hit::Row(row) => CellRef::new(toward.column, row),
            Hit::Column(column) => CellRef::new(column, toward.row),
            Hit::All => toward,
        }
    }
}

/// What a press started dragging over, which decides what the drag
/// extends: cells, whole rows or whole columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragKind {
    Cells,
    Rows,
    Columns,
}

/// A selected range and the active cell inside it — the one typing goes
/// into, which Enter and Tab move without losing the range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    /// Where typing goes.
    pub active: CellRef,
    /// The corner the range was started from.
    pub anchor: CellRef,
    /// The corner it was extended to.
    pub focus: CellRef,
}

impl Default for Selection {
    fn default() -> Self {
        Self::cell(CellRef::new(0, 0))
    }
}

impl Selection {
    pub fn cell(at: CellRef) -> Self {
        Self {
            active: at,
            anchor: at,
            focus: at,
        }
    }

    /// From `anchor`, which is active, to `focus`.
    pub fn range_from(anchor: CellRef, focus: CellRef) -> Self {
        Self {
            active: anchor,
            anchor,
            focus,
        }
    }

    pub fn range(&self) -> CellRange {
        span(self.anchor, self.focus)
    }

    /// One cell, not a range.
    pub fn is_cell(&self) -> bool {
        self.anchor == self.focus
    }

    /// Every column of the rows it covers.
    pub fn is_whole_rows(&self) -> bool {
        let range = self.range();
        range.start.column == 0 && range.end.column == MAX_COLUMN
    }

    /// Every row of the columns it covers.
    pub fn is_whole_columns(&self) -> bool {
        let range = self.range();
        range.start.row == 0 && range.end.row == MAX_ROW
    }

    /// Moves the range's far corner, keeping the anchor and the active cell.
    pub fn extend_to(&mut self, focus: CellRef) {
        self.focus = focus;
    }

    /// What a press selects: a cell, or a whole row, column or sheet. With
    /// Shift it extends from the anchor instead, the active cell staying.
    pub fn press(&mut self, hit: Hit, extend: bool) -> DragKind {
        let kind = match hit {
            Hit::Cell(_) | Hit::All => DragKind::Cells,
            Hit::Row(_) => DragKind::Rows,
            Hit::Column(_) => DragKind::Columns,
        };
        if extend {
            match hit {
                Hit::Cell(cell) => self.focus = cell,
                Hit::Row(row) => {
                    self.anchor = CellRef::new(0, self.anchor.row);
                    self.focus = CellRef::new(MAX_COLUMN, row);
                }
                Hit::Column(column) => {
                    self.anchor = CellRef::new(self.anchor.column, 0);
                    self.focus = CellRef::new(column, MAX_ROW);
                }
                Hit::All => *self = Self::all(),
            }
            return kind;
        }
        *self = match hit {
            Hit::Cell(cell) => Self::cell(cell),
            Hit::Row(row) => Self {
                active: CellRef::new(0, row),
                anchor: CellRef::new(0, row),
                focus: CellRef::new(MAX_COLUMN, row),
            },
            Hit::Column(column) => Self {
                active: CellRef::new(column, 0),
                anchor: CellRef::new(column, 0),
                focus: CellRef::new(column, MAX_ROW),
            },
            Hit::All => Self::all(),
        };
        kind
    }

    /// Where a drag that started as `kind` has reached: a cell for cells, a
    /// row or column for headers, wherever over the sheet the pointer is.
    pub fn drag(&mut self, kind: DragKind, hit: Hit) {
        let reached = hit.corner(self.focus);
        self.focus = match kind {
            DragKind::Cells => reached,
            DragKind::Rows => CellRef::new(MAX_COLUMN, reached.row),
            DragKind::Columns => CellRef::new(reached.column, MAX_ROW),
        };
    }

    /// The whole sheet, A1 active.
    pub fn all() -> Self {
        Self {
            active: CellRef::new(0, 0),
            anchor: CellRef::new(0, 0),
            focus: CellRef::new(MAX_COLUMN, MAX_ROW),
        }
    }

    /// Shift+Space: the rows the selection covers, whole.
    pub fn whole_rows(&mut self) {
        let range = self.range();
        self.anchor = CellRef::new(0, range.start.row);
        self.focus = CellRef::new(MAX_COLUMN, range.end.row);
    }

    /// Ctrl+Space: the columns the selection covers, whole.
    pub fn whole_columns(&mut self) {
        let range = self.range();
        self.anchor = CellRef::new(range.start.column, 0);
        self.focus = CellRef::new(range.end.column, MAX_ROW);
    }

    /// Enter (`by_row` false) or Tab (`by_row` true), Shift going back: the
    /// next cell. Inside a range the active cell walks it and wraps, the
    /// range staying selected, as in Excel; a single cell just moves.
    pub fn advance(&mut self, by_row: bool, back: bool) {
        let range = self.range();
        if self.is_cell() {
            let (columns, rows) = match (by_row, back) {
                (true, false) => (1, 0),
                (true, true) => (-1, 0),
                (false, false) => (0, 1),
                (false, true) => (0, -1),
            };
            *self = Self::cell(offset(self.active, columns, rows));
            return;
        }
        let CellRef {
            mut column,
            mut row,
        } = self.active;
        // Walk along the minor axis, wrapping onto the next line of the
        // major one, and round to the start after the last cell.
        let (minor, minor_first, minor_last, major, major_first, major_last) = if by_row {
            (
                &mut column,
                range.start.column,
                range.end.column,
                &mut row,
                range.start.row,
                range.end.row,
            )
        } else {
            (
                &mut row,
                range.start.row,
                range.end.row,
                &mut column,
                range.start.column,
                range.end.column,
            )
        };
        if back {
            if *minor > minor_first {
                *minor -= 1;
            } else {
                *minor = minor_last;
                *major = if *major > major_first {
                    *major - 1
                } else {
                    major_last
                };
            }
        } else if *minor < minor_last {
            *minor += 1;
        } else {
            *minor = minor_first;
            *major = if *major < major_last {
                *major + 1
            } else {
                major_first
            };
        }
        self.active = CellRef::new(column, row);
    }
}

/// `from` moved by whole cells, kept on the sheet.
pub fn offset(from: CellRef, columns: i64, rows: i64) -> CellRef {
    CellRef::new(
        (i64::from(from.column) + columns).clamp(0, i64::from(MAX_COLUMN)) as u32,
        (i64::from(from.row) + rows).clamp(0, i64::from(MAX_ROW)) as u32,
    )
}

/// The direction an arrow key points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    /// The arrow key gpui names `key`, if it is one.
    pub fn of_key(key: &str) -> Option<Self> {
        match key {
            "up" => Some(Self::Up),
            "down" => Some(Self::Down),
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            _ => None,
        }
    }

    /// (columns, rows) one step goes.
    pub fn delta(self) -> (i64, i64) {
        match self {
            Self::Up => (0, -1),
            Self::Down => (0, 1),
            Self::Left => (-1, 0),
            Self::Right => (1, 0),
        }
    }

    /// Along a row (left or right) rather than a column.
    pub fn is_horizontal(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
}

/// The name box's text while a drag is selecting: `3R x 2C`, as Excel
/// shows it.
pub fn size_label(range: CellRange) -> String {
    format!(
        "{}R x {}C",
        range.end.row - range.start.row + 1,
        range.end.column - range.start.column + 1
    )
}

/// Whether a reference can go in at byte `caret` of a formula being typed —
/// after `=`, an operator, `(`, `,` or `:` — so that an arrow key or a click
/// points at a cell rather than leaving the cell, as Excel's Point mode.
pub fn can_point(text: &str, caret: usize) -> bool {
    text.starts_with('=')
        && text
            .get(..caret)
            .and_then(|before| before.trim_end().chars().last())
            .is_some_and(|ch| "=+-*/^&(,:<>;".contains(ch))
}

/// F4 on a reference in a formula: `A1` → `$A$1` → `A$1` → `$A1` → `A1`.
/// Looks at the reference that ends at (or contains) byte `caret`; returns
/// the new text and where the caret goes.
pub fn cycle_reference(text: &str, caret: usize) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    let is_part = |b: u8| b.is_ascii_alphanumeric() || b == b'$';
    let mut start = caret.min(bytes.len());
    while start > 0 && is_part(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = caret.min(bytes.len());
    while end < bytes.len() && is_part(bytes[end]) {
        end += 1;
    }
    let token = &text[start..end];
    let plain: String = token.chars().filter(|ch| *ch != '$').collect();
    let letters = plain
        .chars()
        .take_while(|ch| ch.is_ascii_alphabetic())
        .count();
    let digits = &plain[letters..];
    if letters == 0
        || letters > 3
        || digits.is_empty()
        || !digits.chars().all(|ch| ch.is_ascii_digit())
    {
        return None;
    }
    let (column, row) = plain.split_at(letters);
    let column_fixed = token.starts_with('$');
    let row_fixed = token[1..].contains('$');
    let next = match (column_fixed, row_fixed) {
        (false, false) => format!("${column}${row}"),
        (true, true) => format!("{column}${row}"),
        (false, true) => format!("${column}{row}"),
        (true, false) => format!("{column}{row}"),
    };
    let mut out = String::with_capacity(text.len() + 2);
    out.push_str(&text[..start]);
    out.push_str(&next);
    out.push_str(&text[end..]);
    Some((out, start + next.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(column: u32, row: u32) -> CellRef {
        CellRef::new(column, row)
    }

    /// Enter walks down a range's columns and Tab along its rows, wrapping,
    /// and the range stays selected.
    #[test]
    fn enter_and_tab_walk_the_range() {
        let mut selection = Selection::range_from(at(1, 1), at(2, 2));
        selection.advance(false, false);
        assert_eq!(selection.active, at(1, 2));
        selection.advance(false, false);
        assert_eq!(selection.active, at(2, 1));
        selection.advance(false, false);
        selection.advance(false, false);
        assert_eq!(selection.active, at(1, 1), "wraps round to the start");
        assert_eq!(selection.range(), span(at(1, 1), at(2, 2)));
        selection.advance(true, false);
        assert_eq!(selection.active, at(2, 1));
        selection.advance(true, false);
        assert_eq!(selection.active, at(1, 2));
        selection.advance(true, true);
        assert_eq!(selection.active, at(2, 1));
        selection.advance(false, true);
        assert_eq!(selection.active, at(1, 2));
        // A single cell just moves.
        let mut single = Selection::cell(at(0, 0));
        single.advance(false, false);
        assert_eq!(single, Selection::cell(at(0, 1)));
        single.advance(true, true);
        assert_eq!(single, Selection::cell(at(0, 1)), "stays on the sheet");
    }

    /// A header press selects the whole row or column and a drag from it
    /// keeps selecting whole rows or columns, wherever the pointer goes.
    #[test]
    fn header_drags_select_whole_rows_and_columns() {
        let mut selection = Selection::default();
        let kind = selection.press(Hit::Row(3), false);
        assert!(selection.is_whole_rows());
        selection.drag(kind, Hit::Cell(at(4, 6)));
        assert_eq!(selection.range(), span(at(0, 3), at(MAX_COLUMN, 6)));
        assert_eq!(selection.active, at(0, 3));

        let kind = selection.press(Hit::Column(2), false);
        selection.drag(kind, Hit::Column(0));
        assert_eq!(selection.range(), span(at(0, 0), at(2, MAX_ROW)));
        assert!(selection.is_whole_columns());

        let kind = selection.press(Hit::Cell(at(1, 1)), false);
        selection.drag(kind, Hit::Cell(at(3, 4)));
        assert_eq!(selection.range(), span(at(1, 1), at(3, 4)));
        assert_eq!(size_label(selection.range()), "4R x 3C");
        // Shift-click moves the far corner; the active cell stays.
        selection.press(Hit::Cell(at(0, 0)), true);
        assert_eq!(selection.range(), span(at(0, 0), at(1, 1)));
        assert_eq!(selection.active, at(1, 1));
    }

    #[test]
    fn shift_space_and_ctrl_space_widen_to_rows_and_columns() {
        let mut selection = Selection::range_from(at(2, 4), at(3, 6));
        selection.whole_rows();
        assert_eq!(selection.range(), span(at(0, 4), at(MAX_COLUMN, 6)));
        let mut selection = Selection::range_from(at(2, 4), at(3, 6));
        selection.whole_columns();
        assert_eq!(selection.range(), span(at(2, 0), at(3, MAX_ROW)));
    }

    #[test]
    fn formulas_point_after_operators_and_f4_cycles_dollars() {
        assert!(can_point("=", 1));
        assert!(can_point("=SUM(", 5));
        assert!(can_point("=A1+", 4));
        assert!(!can_point("=A1", 3));
        assert!(!can_point("abc+", 4));
        let (text, caret) = cycle_reference("=A1+B2", 3).unwrap();
        assert_eq!((text.as_str(), caret), ("=$A$1+B2", 5));
        let (text, _) = cycle_reference(&text, 5).unwrap();
        assert_eq!(text, "=A$1+B2");
        let (text, _) = cycle_reference(&text, 4).unwrap();
        assert_eq!(text, "=$A1+B2");
        let (text, _) = cycle_reference(&text, 4).unwrap();
        assert_eq!(text, "=A1+B2");
        assert_eq!(cycle_reference("=SUM(", 5), None);
    }
}

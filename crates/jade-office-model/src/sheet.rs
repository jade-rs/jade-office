//! An editable workbook: jade-core's `HeadlessWorkbook` with the selected
//! sheet laid out and painted through a [`Backend`].
//!
//! The editing core the browser spreadsheet and the headless CLI also use: a
//! write patches the workbook's bytes, recalculates, and keeps undo history.
//! A shell hit-tests through [`Workbook::hit`], types through
//! [`Workbook::set_cell`], and paints with [`Workbook::render_viewport`].

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use jade_painter::Painter;
use jade_xlsx_headless_api::{AutoFilterChange, AutoFilterTarget, CellInput};
use jade_xlsx_headless_editor::{
    chart_kind, formula_text, ClipboardSnapshot, ConditionalFormatView, ConditionalStyle,
    FormatChange, HeadlessWorkbook, NewConditionalRule, ShapeAnchor,
};

use crate::charts::{ChartChange, ChartInfo, ChartKind, LegendPosition};

use crate::grid::{self, Direction, Hit, Selection};
use crate::{clipboard, typing};
pub use jade_xlsx_headless_api::{AutoFilterOption, AutoFilterOptions, BorderLine, BorderStyle};
pub use jade_xlsx_headless_editor::FormatChange as CellFormat;
pub use jade_xlsx_layout::filter_button::{FilterButton, FilterButtonTarget};
pub use jade_xlsx_layout::resize::{ResizeGesture, ResizeHit};
pub use jade_xlsx_layout::{column_label, HeaderMetrics};
use jade_xlsx_layout::{layout_workbook_sheet, LayoutOptions, Rect, SheetLayout};
pub use jade_xlsx_reader::Axis;
pub use jade_xlsx_reader::HorizontalAlignment;
use jade_xlsx_reader::{translate_formula, CellValue, SheetVisibility};
pub use jade_xlsx_reader::{CellRange, CellRef};
use jade_xlsx_render::{render_grid, GridSelection, PainterMeasure, RenderOptions};

use crate::Backend;

/// The grid drawn past the last used cell; it grows as the selection moves.
const MIN_COLUMNS: u32 = 12;
const MIN_ROWS: u32 = 30;
/// What an entry a data validation refuses is reported as: this, then the
/// rule's own message (empty for Excel's standard one). See
/// [`validation_refusal`].
pub const VALIDATION_REFUSED: &str = "\u{1}validation:";

/// The message of an entry a data validation refused — the rule's own, or
/// an empty one for the shell's standard words — when `error` is one.
pub fn validation_refusal(error: &str) -> Option<&str> {
    error.strip_prefix(VALIDATION_REFUSED)
}

/// Whether `error` is a protected sheet refusing the change — a locked
/// cell typed into, or formatting it does not allow — which the shells say
/// in their own words.
pub fn protected_refusal(error: &str) -> bool {
    error.starts_with("sheet `") && error.contains("` is protected: ")
}

/// Excel's limits.
pub const MAX_COLUMN: u32 = 16_383;
pub const MAX_ROW: u32 = 1_048_575;
/// The most cells a copy puts on the clipboard.
const CLIPBOARD_CELL_LIMIT: u64 = 200_000;

/// `B3`.
pub fn reference_text(reference: CellRef) -> String {
    format!("{}{}", column_label(reference.column), reference.row + 1)
}

/// Clipboard text compared line by line, whatever line ends the system gave
/// it and however many it left at the end.
fn clipboard_lines(text: &str) -> Vec<&str> {
    text.trim_end_matches(['\r', '\n'])
        .split('\n')
        .map(|line| line.trim_end_matches('\r'))
        .collect()
}

/// `B3`, or `B3:D7` for a range.
pub fn range_text(range: CellRange) -> String {
    if range.start == range.end {
        reference_text(range.start)
    } else {
        format!(
            "{}:{}",
            reference_text(range.start),
            reference_text(range.end)
        )
    }
}

/// The rectangle two corners span, top-left first.
pub fn span(a: CellRef, b: CellRef) -> CellRange {
    CellRange {
        start: CellRef::new(a.column.min(b.column), a.row.min(b.row)),
        end: CellRef::new(a.column.max(b.column), a.row.max(b.row)),
    }
}

/// What typed text becomes: `=` starts a formula, otherwise it is read the
/// way a spreadsheet reads what you type.
pub fn cell_input(text: &str) -> CellInput {
    match text.strip_prefix('=') {
        Some(formula) if !formula.is_empty() => CellInput::Formula(formula.to_string()),
        _ => CellInput::parse(text),
    }
}

/// A number as the formula bar shows it: up to fifteen significant
/// digits, Excel's precision, without trailing zeros.
pub(crate) fn full_precision(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        return format!("{}", value as i64);
    }
    let digits = 15 - (value.abs().log10().floor() as i32 + 1);
    let text = format!("{value:.*}", digits.clamp(0, 15) as usize);
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A number as a status bar shows it: integers bare, others to a few places.
pub fn format_number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        let text = format!("{value:.4}");
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Excel's status-bar summary of a selection.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    /// Cells holding anything.
    pub count: usize,
    /// Sum and average of the numbers, when there are any.
    pub sum: Option<f64>,
    pub average: Option<f64>,
}

pub struct Workbook<B: Backend> {
    book: HeadlessWorkbook,
    backend: B,
    index: usize,
    layout: SheetLayout,
    extent: (u32, u32),
    dirty: bool,
    /// How many of the book's undo steps each edit took, latest last: a
    /// typed `10%` is a value and a format, and undoes as one.
    steps: Vec<usize>,
    /// The same for the edits undone, for redo.
    undone: Vec<usize>,
    /// The last copy or cut: the engine's snapshot of the cells, the text it
    /// put on the clipboard, and whether it was a cut. A paste of that same
    /// text pastes the cells themselves, as Excel does within a workbook.
    clipboard: Option<Clip>,
    /// The last surface painted on, reused while the size stays the same:
    /// a sheet repaints on every scroll frame.
    surface: RefCell<Option<(B::Painter, u32, u32)>>,
    /// Bumped whenever the sheet is laid out again, so a shell keeping
    /// painted pieces of it knows they are stale.
    revision: u64,
    /// What the relayouts since a shell last asked changed, in sheet px;
    /// `None` for anything. See [`Self::take_changes`].
    changes: Option<Vec<Rect>>,
    /// The same for pieces of the sheet painted on their own — tiles of
    /// cells, the header bands — by their size.
    pieces: RefCell<Vec<(B::Painter, u32, u32)>>,
}

impl<B: Backend> Workbook<B> {
    /// Opens `.xlsx` bytes for editing, recalculating in `time_zone` (an IANA
    /// name such as `Asia/Tokyo`; UTC when the engine does not know it).
    pub fn open(bytes: &[u8], time_zone: &str, backend: B) -> Result<Self, String> {
        let book = HeadlessWorkbook::open_for_editing_in_time_zone(bytes, time_zone)
            .or_else(|_| HeadlessWorkbook::open_for_editing(bytes))
            .map_err(|error| format!("cannot read the workbook: {error}"))?;
        let workbook = book.workbook();
        if workbook.sheets.is_empty() {
            return Err("the workbook has no sheets".to_string());
        }
        // The sheet the file says was open, if a reader can see it.
        let visible = |index: &usize| {
            workbook
                .sheets
                .get(*index)
                .is_some_and(|sheet| sheet.visibility == SheetVisibility::Visible)
        };
        let index = workbook
            .active_sheet_index
            .filter(visible)
            .or_else(|| (0..workbook.sheets.len()).find(visible))
            .unwrap_or(0);
        let layout = lay_out(&book, index, (MIN_COLUMNS, MIN_ROWS), &backend)?;
        Ok(Self {
            book,
            backend,
            index,
            layout,
            extent: (MIN_COLUMNS, MIN_ROWS),
            dirty: false,
            steps: Vec::new(),
            undone: Vec::new(),
            clipboard: None,
            surface: RefCell::new(None),
            pieces: RefCell::new(Vec::new()),
            revision: 0,
            changes: None,
        })
    }

    pub fn sheet_names(&self) -> Vec<String> {
        self.book
            .workbook()
            .sheets
            .iter()
            .map(|sheet| sheet.name.clone())
            .collect()
    }

    /// The sheets that have a tab, by index: every sheet but a hidden one,
    /// as Excel shows them.
    pub fn sheet_tabs(&self) -> Vec<usize> {
        self.book
            .workbook()
            .sheets
            .iter()
            .enumerate()
            .filter(|(_, sheet)| sheet.visibility == SheetVisibility::Visible)
            .map(|(index, _)| index)
            .collect()
    }

    pub fn sheet_index(&self) -> usize {
        self.index
    }

    /// The selected sheet's name.
    pub fn sheet_name(&self) -> String {
        self.book.workbook().sheets[self.index].name.clone()
    }

    pub fn select_sheet(&mut self, index: usize) -> Result<(), String> {
        if index >= self.book.workbook().sheets.len() {
            return Err(format!("no sheet at index {index}"));
        }
        if index != self.index {
            self.changes = None;
        }
        self.index = index;
        self.extent = (MIN_COLUMNS, MIN_ROWS);
        self.relayout()
    }

    fn relayout(&mut self) -> Result<(), String> {
        let layout = lay_out(&self.book, self.index, self.extent, &self.backend)?;
        let changed = layout_changes(&self.layout, &layout);
        if let (Some(changes), Some(changed)) = (&mut self.changes, changed) {
            changes.extend(changed);
        } else {
            self.changes = None;
        }
        self.layout = layout;
        self.revision += 1;
        Ok(())
    }

    /// Where the sheet may paint differently since the last call, in sheet px
    /// as `(x, y, width, height)` — the cells an edit touched, their text
    /// running into their neighbours, the grid grown — or `None` for
    /// anywhere: another sheet, columns or rows resized or moved, a chart
    /// changed. A shell keeping painted pieces of the sheet repaints those
    /// these touch.
    pub fn take_changes(&mut self) -> Option<Vec<SheetRect>> {
        self.changes.replace(Vec::new()).map(|rects| {
            rects
                .into_iter()
                .map(|r| (r.x, r.y, r.width, r.height))
                .collect()
        })
    }

    /// Changes whenever the sheet is laid out again — an edit, another
    /// sheet, the grid grown — and so whenever what it paints may have.
    pub fn layout_revision(&self) -> u64 {
        self.revision
    }

    /// The laid-out sheet's size in px, headers included.
    pub fn sheet_size(&self) -> (f64, f64) {
        let metrics = self.layout.header_metrics();
        (
            self.layout.content_width + metrics.row_width,
            self.layout.content_height + metrics.column_height,
        )
    }

    pub fn header_metrics(&self) -> HeaderMetrics {
        self.layout.header_metrics()
    }

    /// The far corner of the laid-out grid, for select-all.
    pub fn grid_end(&self) -> CellRef {
        CellRef::new(
            (self.layout.columns.len() as u32).saturating_sub(1),
            (self.layout.rows.len() as u32).saturating_sub(1),
        )
    }

    /// Grows the laid-out grid so `reference`, and a margin past it, exist.
    pub fn reach(&mut self, reference: CellRef) {
        let wanted = (
            self.extent.0.max(reference.column + 8),
            self.extent.1.max(reference.row + 20),
        );
        if wanted != self.extent {
            self.extent = wanted;
            self.relayout().ok();
        }
    }

    /// What is under a point of the viewport (sheet px from its top-left,
    /// headers included) at `scroll`: a cell, or a row or column header, or
    /// the corner where the headers meet.
    pub fn hit(&self, x: f64, y: f64, scroll: (f64, f64)) -> Option<Hit> {
        let metrics = self.layout.header_metrics();
        let cell = self.reach_point(x, y, scroll);
        Some(match (x < metrics.row_width, y < metrics.column_height) {
            (true, true) => Hit::All,
            (true, false) => Hit::Row(cell.row),
            (false, true) => Hit::Column(cell.column),
            (false, false) => Hit::Cell(self.layout_cell(x, y, scroll)?),
        })
    }

    fn layout_cell(&self, x: f64, y: f64, scroll: (f64, f64)) -> Option<CellRef> {
        let metrics = self.layout.header_metrics();
        let (sheet_x, sheet_y) = self.layout.sheet_point_in_viewport(
            x - metrics.row_width,
            y - metrics.column_height,
            scroll.0,
            scroll.1,
        );
        self.layout.cell_at(sheet_x, sheet_y)
    }

    /// The cell a drag has reached at a viewport point: under it, or the
    /// nearest one when the pointer is over a header or past the grid.
    pub fn reach_point(&self, x: f64, y: f64, scroll: (f64, f64)) -> CellRef {
        let layout = &self.layout;
        let metrics = layout.header_metrics();
        let (sheet_x, sheet_y) = layout.sheet_point_in_viewport(
            (x - metrics.row_width).max(0.0),
            (y - metrics.column_height).max(0.0),
            scroll.0,
            scroll.1,
        );
        let last = self.grid_end();
        let column = layout
            .column_at(sheet_x.max(0.0))
            .unwrap_or(if sheet_x <= 0.0 { 0 } else { last.column });
        let row = layout
            .row_at(sheet_y.max(0.0))
            .unwrap_or(if sheet_y <= 0.0 { 0 } else { last.row });
        CellRef::new(column, row)
    }

    /// Whether a viewport point is on the fill handle — the square at the
    /// bottom-right corner of `range` a drag fills from.
    pub fn on_fill_handle(&self, x: f64, y: f64, scroll: (f64, f64), range: CellRange) -> bool {
        let Some((cx, cy, w, h)) = self.cell_rect(range.end, scroll) else {
            return false;
        };
        let (corner_x, corner_y) = (cx + w, cy + h);
        (x - corner_x).abs() <= 5.0 && (y - corner_y).abs() <= 5.0
    }

    /// The column or row edge under a header point, which a drag resizes.
    pub fn resize_hit(
        &self,
        x: f64,
        y: f64,
        scroll: (f64, f64),
    ) -> Option<jade_xlsx_layout::resize::ResizeHit> {
        jade_xlsx_layout::resize::header_resize_hit(&self.layout, x, y, scroll.0, scroll.1)
    }

    /// The cell's rectangle in the viewport (sheet px, headers included) at
    /// `scroll`, as (x, y, width, height).
    pub fn cell_rect(
        &self,
        reference: CellRef,
        scroll: (f64, f64),
    ) -> Option<(f64, f64, f64, f64)> {
        let metrics = self.layout.header_metrics();
        let rect = self
            .layout
            .cell_rect_in_viewport(reference, scroll.0, scroll.1)?;
        Some((
            rect.x + metrics.row_width,
            rect.y + metrics.column_height,
            rect.width,
            rect.height,
        ))
    }

    /// The scroll (sheet px) that shows `reference` in a viewport of `size`
    /// (sheet px, headers included) now at `scroll`.
    pub fn reveal(&self, reference: CellRef, scroll: (f64, f64), size: (f64, f64)) -> (f64, f64) {
        let metrics = self.layout.header_metrics();
        let viewport = Rect {
            x: scroll.0,
            y: scroll.1,
            width: (size.0 - metrics.row_width).max(1.0),
            height: (size.1 - metrics.column_height).max(1.0),
        };
        self.layout.scroll_to_reveal(reference, viewport)
    }

    // ---- moving round the data -----------------------------------------

    /// Where a stored cell holds a value or a formula along one row
    /// (`horizontal`) or column, through `at`.
    fn filled_line(&self, at: CellRef, horizontal: bool) -> BTreeSet<u32> {
        let sheet = &self.book.workbook().sheets[self.index];
        let mut line = BTreeSet::new();
        for row in &sheet.rows {
            if horizontal && row.index != at.row {
                continue;
            }
            for cell in &row.cells {
                let holds = cell.formula.is_some() || !matches!(cell.value, CellValue::Empty);
                if !holds {
                    continue;
                }
                if horizontal {
                    line.insert(cell.reference.column);
                } else if cell.reference.column == at.column {
                    line.insert(cell.reference.row);
                }
            }
        }
        line
    }

    /// An arrow key's step from `from`, as Excel takes it: over the rest of
    /// a merged cell, past hidden rows and columns, onto a merge's top-left
    /// cell. Stays put at the sheet's edge.
    pub fn step(&self, from: CellRef, direction: Direction) -> CellRef {
        let sheet = &self.book.workbook().sheets[self.index];
        // From a merged cell, the step starts at its far edge that way.
        let edge = match sheet.merge_at(from) {
            Some(merge) => match direction {
                Direction::Up => CellRef::new(from.column, merge.start.row),
                Direction::Down => CellRef::new(from.column, merge.end.row),
                Direction::Left => CellRef::new(merge.start.column, from.row),
                Direction::Right => CellRef::new(merge.end.column, from.row),
            },
            None => from,
        };
        let (columns, rows) = direction.delta();
        let hidden = |at: CellRef| {
            if direction.is_horizontal() {
                self.layout
                    .column(at.column)
                    .is_some_and(|entry| entry.hidden || entry.size <= 0.0)
            } else {
                self.layout
                    .row(at.row)
                    .is_some_and(|entry| entry.hidden || entry.size <= 0.0)
            }
        };
        let mut to = edge;
        loop {
            let next = grid::offset(to, columns, rows);
            if next == to {
                return from;
            }
            to = next;
            if !hidden(to) {
                break;
            }
        }
        // A merged cell is entered at its top-left, the cell it is named by.
        sheet.merge_at(to).map_or(to, |merge| merge.start)
    }

    /// `range` grown to take in every merged cell it touches, as Excel's
    /// selection grows over them.
    pub fn with_merges(&self, range: CellRange) -> CellRange {
        let sheet = &self.book.workbook().sheets[self.index];
        let mut range = range;
        loop {
            let before = range;
            for merge in &sheet.merges {
                let touches = merge.start.column <= range.end.column
                    && range.start.column <= merge.end.column
                    && merge.start.row <= range.end.row
                    && range.start.row <= merge.end.row;
                if touches {
                    range = span(
                        CellRef::new(
                            range.start.column.min(merge.start.column),
                            range.start.row.min(merge.start.row),
                        ),
                        CellRef::new(
                            range.end.column.max(merge.end.column),
                            range.end.row.max(merge.end.row),
                        ),
                    );
                }
            }
            if range == before {
                return range;
            }
        }
    }

    /// ⌘-arrow (Ctrl-arrow elsewhere): from a filled cell with a filled
    /// neighbour, to the last filled cell of the run; otherwise to the next
    /// filled cell — or the edge of the sheet when there is none.
    pub fn jump(&self, from: CellRef, direction: Direction) -> CellRef {
        let horizontal = direction.is_horizontal();
        let line = self.filled_line(from, horizontal);
        let (position, limit) = if horizontal {
            (from.column, MAX_COLUMN)
        } else {
            (from.row, MAX_ROW)
        };
        let forward = matches!(direction, Direction::Right | Direction::Down);
        let next = |p: u32| {
            if forward {
                (p < limit).then(|| p + 1)
            } else {
                p.checked_sub(1)
            }
        };
        let Some(first) = next(position) else {
            return from;
        };
        let to = if line.contains(&position) && line.contains(&first) {
            let mut at = first;
            while let Some(further) = next(at).filter(|p| line.contains(p)) {
                at = further;
            }
            at
        } else if forward {
            line.range(first..).next().copied().unwrap_or(limit)
        } else {
            line.range(..=first).next_back().copied().unwrap_or(0)
        };
        if horizontal {
            CellRef::new(to, from.row)
        } else {
            CellRef::new(from.column, to)
        }
    }

    /// AutoSum's guess for a SUM in `at`: the run of numbers straight above
    /// it, or else to its left, as Excel proposes.
    pub fn autosum_range(&self, at: CellRef) -> Option<CellRange> {
        let run = |direction: Direction| {
            let mut first = None;
            let mut cursor = at;
            loop {
                let next = grid::offset(cursor, direction.delta().0, direction.delta().1);
                if next == cursor || self.number(next).is_none() {
                    break;
                }
                first = Some(next);
                cursor = next;
            }
            first.map(|first| {
                span(
                    first,
                    grid::offset(at, direction.delta().0, direction.delta().1),
                )
            })
        };
        run(Direction::Up).or_else(|| run(Direction::Left))
    }

    /// The note on `at` (or the merged cell it is in), as a hover shows it.
    pub fn note(&self, at: CellRef) -> Option<&str> {
        self.layout
            .notes
            .iter()
            .find(|note| note.reference == at)
            .map(|note| note.text.as_str())
    }

    /// The hyperlink on `at`, if it has one: a web address, or a place in
    /// the workbook.
    pub fn link(&self, at: CellRef) -> Option<Link> {
        let workbook = self.book.workbook();
        let link = workbook.sheets[self.index].hyperlink_at(at)?;
        if let Some(target) = link.target.as_ref().filter(|target| !target.is_empty()) {
            let mut url = target.clone();
            if let Some(location) = link
                .location
                .as_ref()
                .filter(|location| !location.is_empty())
            {
                url = format!("{url}#{location}");
            }
            return Some(Link::Url(url));
        }
        let location = link.location.as_ref()?;
        let (sheet, reference) = match location.rsplit_once('!') {
            Some((sheet, reference)) => (sheet.trim_matches('\'').replace("''", "'"), reference),
            None => (self.sheet_name(), location.as_str()),
        };
        let index = workbook
            .sheets
            .iter()
            .position(|candidate| candidate.name.eq_ignore_ascii_case(&sheet))?;
        let range = parse_range(&reference.replace('$', ""))?;
        Some(Link::Place {
            sheet: index,
            range,
        })
    }

    /// The data validation that applies to `at`.
    fn validation(&self, at: CellRef) -> Option<&jade_xlsx_reader::DataValidation> {
        self.book.workbook().sheets[self.index]
            .data_validations
            .iter()
            .find(|validation| validation.ranges.iter().any(|range| range.contains(at)))
    }

    /// The choices of a list validation on `at` — what Excel's in-cell
    /// dropdown offers — from the list typed into the rule or the cells it
    /// names.
    pub fn validation_list(&self, at: CellRef) -> Option<Vec<String>> {
        let validation = self.validation(at)?;
        if validation.validation_type.as_deref() != Some("list") {
            return None;
        }
        let source = validation.formula1.as_deref()?.trim();
        if let Some(list) = source
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'))
        {
            return Some(
                list.split(',')
                    .map(|item| item.trim().to_string())
                    .collect(),
            );
        }
        let source = source.trim_start_matches('=');
        let (sheet, reference) = match source.rsplit_once('!') {
            Some((sheet, reference)) => (sheet.trim_matches('\'').replace("''", "'"), reference),
            None => (self.sheet_name(), source),
        };
        let range = parse_range(&reference.replace('$', ""))?;
        let cells = u64::from(range.end.column - range.start.column + 1)
            * u64::from(range.end.row - range.start.row + 1);
        if cells > 10_000 {
            return None;
        }
        let mut items = Vec::new();
        for row in range.start.row..=range.end.row {
            for column in range.start.column..=range.end.column {
                let text = self
                    .book
                    .cell(&sheet, &reference_text(CellRef::new(column, row)))
                    .map(|cell| cell.display)
                    .unwrap_or_default();
                if !text.is_empty() {
                    items.push(text);
                }
            }
        }
        Some(items)
    }

    /// Whether `text` may go into `at` under its data validation: `Err`
    /// with the rule's own error message (or `None` for Excel's) when a
    /// Stop rule refuses it.
    fn check_validation(&self, at: CellRef, text: &str) -> Result<(), Option<String>> {
        let Some(validation) = self.validation(at) else {
            return Ok(());
        };
        if !validation.show_error_message || text.starts_with('=') {
            return Ok(());
        }
        if text.is_empty() {
            return if validation.allow_blank {
                Ok(())
            } else {
                Err(validation.error.clone())
            };
        }
        let refuse = || Err(validation.error.clone());
        let kind = validation.validation_type.as_deref().unwrap_or("none");
        if kind == "list" {
            return match self.validation_list(at) {
                Some(items)
                    if !items
                        .iter()
                        .any(|item| item.eq_ignore_ascii_case(text.trim())) =>
                {
                    refuse()
                }
                _ => Ok(()),
            };
        }
        let value = match kind {
            "whole" | "decimal" => {
                let date1904 = matches!(
                    self.book.workbook().date_system,
                    jade_xlsx_reader::DateSystem::Nineteen04
                );
                match typing::read(text, date1904).input {
                    CellInput::Number(number) => number,
                    _ => return refuse(),
                }
            }
            "textLength" => text.chars().count() as f64,
            _ => return Ok(()),
        };
        if kind == "whole" && value.fract() != 0.0 {
            return refuse();
        }
        let bound = |formula: Option<&String>| {
            formula.and_then(|formula| formula.trim_start_matches('=').trim().parse::<f64>().ok())
        };
        let (Some(first), second) = (
            bound(validation.formula1.as_ref()),
            bound(validation.formula2.as_ref()),
        ) else {
            return Ok(());
        };
        let ok = match validation.operator.as_deref().unwrap_or("between") {
            "between" => value >= first && second.is_none_or(|second| value <= second),
            "notBetween" => !(value >= first && second.is_none_or(|second| value <= second)),
            "equal" => value == first,
            "notEqual" => value != first,
            "greaterThan" => value > first,
            "lessThan" => value < first,
            "greaterThanOrEqual" => value >= first,
            "lessThanOrEqual" => value <= first,
            _ => true,
        };
        if ok {
            Ok(())
        } else {
            refuse()
        }
    }

    /// The bottom-right corner of what the sheet uses — where ⌘End goes.
    pub fn used_end(&self) -> CellRef {
        let mut end = CellRef::new(0, 0);
        for row in &self.book.workbook().sheets[self.index].rows {
            for cell in &row.cells {
                end.column = end.column.max(cell.reference.column);
                end.row = end.row.max(cell.reference.row);
            }
        }
        end
    }

    /// The block of data round `at`, bounded by empty rows and columns —
    /// what ⌘A selects first.
    pub fn current_region(&self, at: CellRef) -> Option<CellRange> {
        let text = self
            .book
            .current_region(&self.sheet_name(), &reference_text(at))
            .ok()?;
        let range = parse_range(&text)?;
        (range.start != range.end).then_some(range)
    }

    /// How many rows from `row` down fill a viewport `height` sheet px tall
    /// (column headers included): the step of Page Down.
    pub fn page_rows(&self, row: u32, height: f64) -> u32 {
        let body = (height - self.layout.header_metrics().column_height).max(1.0);
        let mut used = 0.0;
        let mut count = 0;
        for entry in self.layout.rows.iter().skip(row as usize) {
            used += entry.size;
            if used > body {
                break;
            }
            count += 1;
        }
        count.max(1)
    }

    fn cells_in(&self, range: CellRange) -> impl Iterator<Item = &jade_xlsx_reader::Cell> {
        self.book.workbook().sheets[self.index]
            .rows
            .iter()
            .filter(move |row| row.index >= range.start.row && row.index <= range.end.row)
            .flat_map(|row| row.cells.iter())
            .filter(move |cell| {
                cell.reference.column >= range.start.column
                    && cell.reference.column <= range.end.column
            })
    }

    /// The formula with its `=`, or the stored value: what the formula bar
    /// shows and what an edit starts from.
    pub fn edit_text(&self, reference: CellRef) -> String {
        self.edit_text_on(self.index, reference)
    }

    /// [`Self::edit_text`] for a cell on the sheet at `index`.
    pub(crate) fn edit_text_on(&self, index: usize, reference: CellRef) -> String {
        let Some(cell) = self.book.workbook().sheets[index]
            .rows
            .iter()
            .find(|row| row.index == reference.row)
            .and_then(|row| {
                row.cells
                    .iter()
                    .find(|cell| cell.reference.column == reference.column)
            })
        else {
            return String::new();
        };
        if let Some(formula) = &cell.formula {
            // As Excel shows it: the file's `_xlfn.` prefixes are not what
            // anybody typed.
            return format!("={}", formula_text::to_display_formula(formula));
        }
        match &cell.value {
            CellValue::Empty => String::new(),
            CellValue::Number(number) => self.edit_number(index, reference, *number),
            // Text that would read as a value if typed back keeps the
            // apostrophe that makes it text, as Excel's formula bar shows.
            CellValue::Text(text) if typing::reads_as_value(text) => format!("'{text}"),
            CellValue::Text(text) => text.clone(),
            CellValue::Bool(flag) => if *flag { "TRUE" } else { "FALSE" }.to_string(),
            CellValue::Error(error) => error.clone(),
        }
    }

    /// A number as Excel's formula bar shows it: a percentage as one, a
    /// date or time as one, anything else at full precision.
    fn edit_number(&self, index: usize, reference: CellRef, number: f64) -> String {
        let format = self.number_format_on(index, reference).unwrap_or_default();
        if format.contains('%') && !format.contains('"') {
            return format!("{}%", full_precision(number * 100.0));
        }
        if !format.is_empty() && jade_xlsx_layout::number_format::is_date_format(&format) {
            let date1904 = matches!(
                self.book.workbook().date_system,
                jade_xlsx_reader::DateSystem::Nineteen04
            );
            let code = match (number.trunc() != 0.0, number.fract() != 0.0) {
                (true, true) => "yyyy/m/d h:mm:ss",
                (true, false) => "yyyy/m/d",
                _ => "h:mm:ss",
            };
            return jade_xlsx_layout::number_format::format_value(
                &CellValue::Number(number),
                Some(code),
                if date1904 {
                    jade_xlsx_reader::DateSystem::Nineteen04
                } else {
                    jade_xlsx_reader::DateSystem::Nineteen00
                },
            )
            .text;
        }
        full_precision(number)
    }

    /// The cell's number format code, if it has one other than General.
    fn number_format(&self, reference: CellRef) -> Option<String> {
        self.number_format_on(self.index, reference)
    }

    fn number_format_on(&self, index: usize, reference: CellRef) -> Option<String> {
        let name = &self.book.workbook().sheets[index].name;
        self.book
            .cell(name, &reference_text(reference))
            .ok()?
            .number_format
            .filter(|code| !code.eq_ignore_ascii_case("general"))
    }

    /// The number a cell holds, if it holds one.
    pub fn number(&self, reference: CellRef) -> Option<f64> {
        let range = CellRange {
            start: reference,
            end: reference,
        };
        match self.cells_in(range).next()?.value {
            CellValue::Number(number) => Some(number),
            _ => None,
        }
    }

    /// The workbook's date system: 1900, or the Mac's old 1904.
    pub(crate) fn date_system(&self) -> jade_xlsx_reader::DateSystem {
        self.book.workbook().date_system
    }

    /// What a cell shows, through its number format.
    pub fn display_text(&self, reference: CellRef) -> String {
        self.book
            .cell(&self.sheet_name(), &reference_text(reference))
            .map(|cell| cell.display)
            .unwrap_or_default()
    }

    /// Count, sum and average over the cells in `range`.
    pub fn summary(&self, range: CellRange) -> Summary {
        let mut count = 0;
        let mut numbers = Vec::new();
        for cell in self.cells_in(range) {
            match cell.value {
                CellValue::Empty => {}
                CellValue::Number(number) => {
                    count += 1;
                    numbers.push(number);
                }
                _ => count += 1,
            }
        }
        let sum = (!numbers.is_empty()).then(|| numbers.iter().sum::<f64>());
        Summary {
            count,
            sum,
            average: sum.map(|sum| sum / numbers.len() as f64),
        }
    }

    /// The range's displayed text, tab-separated by column and
    /// newline-separated by row — what Excel copies as text. Empty past the
    /// last used cell, or when the range is too large to copy.
    pub fn range_text(&self, range: CellRange) -> String {
        let mut used = CellRef::new(0, 0);
        for row in &self.book.workbook().sheets[self.index].rows {
            for cell in &row.cells {
                used.column = used.column.max(cell.reference.column);
                used.row = used.row.max(cell.reference.row);
            }
        }
        let end = CellRef::new(
            range.end.column.min(used.column),
            range.end.row.min(used.row),
        );
        if end.column < range.start.column || end.row < range.start.row {
            return String::new();
        }
        let cells = u64::from(end.column - range.start.column + 1)
            * u64::from(end.row - range.start.row + 1);
        if cells > CLIPBOARD_CELL_LIMIT {
            return String::new();
        }
        let mut out = String::new();
        for row in range.start.row..=end.row {
            let line: Vec<String> = (range.start.column..=end.column)
                .map(|column| clipboard::field(&self.display_text(CellRef::new(column, row))))
                .collect();
            out.push_str(&line.join("\t"));
            out.push('\n');
        }
        out
    }

    /// Writes typed text into a cell, recalculating. Returns whether the
    /// cell changed.
    pub fn set_cell(&mut self, reference: CellRef, text: &str) -> Result<bool, String> {
        let Some(steps) = self.write_cell(reference, text)? else {
            return Ok(false);
        };
        self.record(steps);
        self.after_write()?;
        Ok(true)
    }

    /// The write of [`Self::set_cell`], returning how many of the book's
    /// undo steps it took, or `None` when the cell already reads `text`.
    fn write_cell(&mut self, reference: CellRef, text: &str) -> Result<Option<usize>, String> {
        if text == self.edit_text(reference) {
            return Ok(None);
        }
        if let Err(message) = self.check_validation(reference, text) {
            return Err(format!(
                "{VALIDATION_REFUSED}{}",
                message.unwrap_or_default()
            ));
        }
        let name = self.sheet_name();
        let at = reference_text(reference);
        let date1904 = matches!(
            self.book.workbook().date_system,
            jade_xlsx_reader::DateSystem::Nineteen04
        );
        if let Some(formula) = text.strip_prefix('=').filter(|formula| !formula.is_empty()) {
            if typing::formula_problem(formula) {
                return Err(typing::FORMULA_PROBLEM.to_string());
            }
        }
        let typed = typing::read(text, date1904);
        // Its parentheses closed, a function given the wrong number of
        // arguments is refused too, as Excel refuses it.
        if let CellInput::Formula(formula) = &typed.input {
            if formula_text::wrong_argument_count(formula) {
                return Err(typing::FORMULA_PROBLEM.to_string());
            }
        }
        // Excel gives a General cell the format the value was typed in; a
        // cell someone formatted keeps its format.
        let format = typed
            .format
            .filter(|_| self.number_format(reference).is_none());
        self.book
            .set(&name, &at, typed.input)
            .map_err(|error| error.to_string())?;
        let mut steps = 1;
        if let Some(code) = format {
            let change = FormatChange {
                number_format: Some(code.to_string()),
                ..Default::default()
            };
            if self.book.set_format(&name, &at, &change).is_ok() {
                steps += 1;
            }
        }
        // A line break typed into a cell wraps it, as Excel does.
        if text.contains('\n') && !text.starts_with('=') {
            let change = FormatChange {
                wrap_text: Some(true),
                ..Default::default()
            };
            if self.book.set_format(&name, &at, &change).is_ok() {
                steps += 1;
            }
        }
        Ok(Some(steps))
    }

    // ---- filling ----------------------------------------------------------

    /// What `target` holds once `source` is laid over it, tiled from its
    /// top-left, formulas moved as a paste moves them and formats carried:
    /// the writes for the cells of `target` outside `source`.
    fn tile_writes(
        &self,
        source: CellRange,
        target: CellRange,
    ) -> Vec<(String, CellInput, Option<u32>)> {
        let sheet = &self.book.workbook().sheets[self.index];
        let cells: BTreeMap<CellRef, &jade_xlsx_reader::Cell> = self
            .cells_in(source)
            .map(|cell| (cell.reference, cell))
            .collect();
        let width = source.end.column - source.start.column + 1;
        let height = source.end.row - source.start.row + 1;
        let mut writes = Vec::new();
        for row in target.start.row..=target.end.row {
            for column in target.start.column..=target.end.column {
                let at = CellRef::new(column, row);
                if source.contains(at) {
                    continue;
                }
                let from = CellRef::new(
                    source.start.column + (column - target.start.column) % width,
                    source.start.row + (row - target.start.row) % height,
                );
                let style = Some(jade_xlsx_headless_editor::effective_style(sheet, from));
                let value = match cells.get(&from) {
                    Some(cell) => match &cell.formula {
                        Some(formula) => CellInput::Formula(translate_formula(
                            &formula_text::to_display_formula(formula),
                            from,
                            at,
                        )),
                        None => match &cell.value {
                            CellValue::Empty => CellInput::Blank,
                            CellValue::Number(number) => CellInput::Number(*number),
                            CellValue::Text(text) => CellInput::Text(text.clone()),
                            CellValue::Bool(flag) => CellInput::Bool(*flag),
                            CellValue::Error(error) => CellInput::Text(error.clone()),
                        },
                    },
                    None => CellInput::Blank,
                };
                writes.push((reference_text(at), value, style));
            }
        }
        writes
    }

    /// Lays `source` over `target` as one write; returns whether anything
    /// was written. `extra` steps already taken join the same undo.
    fn tile(&mut self, source: CellRange, target: CellRange, extra: usize) -> Result<bool, String> {
        let cells = u64::from(target.end.column - target.start.column + 1)
            * u64::from(target.end.row - target.start.row + 1);
        if cells > CLIPBOARD_CELL_LIMIT {
            return Err(format!("{} is too large to fill", range_text(target)));
        }
        let writes = self.tile_writes(source, target);
        let mut steps = extra;
        if !writes.is_empty() {
            let name = self.sheet_name();
            self.book
                .set_cells_styled(&name, &writes)
                .map_err(|error| error.to_string())?;
            steps += 1;
        }
        if steps == 0 {
            return Ok(false);
        }
        self.record(steps);
        self.after_write()?;
        Ok(true)
    }

    /// The selection's rows and columns, kept to the part of the sheet in
    /// use when it runs to the sheet's edge — a whole column is filled as
    /// far as there is data, not a million rows down.
    fn in_use(&self, range: CellRange) -> CellRange {
        let used = self.used_end();
        let mut range = range;
        if range.end.row == MAX_ROW {
            range.end.row = used.row.max(range.start.row);
        }
        if range.end.column == MAX_COLUMN {
            range.end.column = used.column.max(range.start.column);
        }
        range
    }

    /// ⌘D: the top row of the range copied down through it; a single row
    /// takes the row above.
    pub fn fill_down(&mut self, range: CellRange) -> Result<bool, String> {
        let mut target = self.in_use(range);
        if target.start.row == target.end.row {
            if target.start.row == 0 {
                return Ok(false);
            }
            target.start.row -= 1;
        }
        let source = CellRange {
            start: target.start,
            end: CellRef::new(target.end.column, target.start.row),
        };
        self.tile(source, target, 0)
    }

    /// ⌘R: the left column copied right through the range; a single column
    /// takes the column to its left.
    pub fn fill_right(&mut self, range: CellRange) -> Result<bool, String> {
        let mut target = self.in_use(range);
        if target.start.column == target.end.column {
            if target.start.column == 0 {
                return Ok(false);
            }
            target.start.column -= 1;
        }
        let source = CellRange {
            start: target.start,
            end: CellRef::new(target.start.column, target.end.row),
        };
        self.tile(source, target, 0)
    }

    /// ⌘Enter: what was typed into the active cell, into every cell of the
    /// range, formulas moved for each — one undo.
    pub fn fill_with(
        &mut self,
        range: CellRange,
        active: CellRef,
        text: &str,
    ) -> Result<bool, String> {
        let steps = self.write_cell(active, text)?.unwrap_or(0);
        let range = self.in_use(range);
        let source = CellRange {
            start: active,
            end: active,
        };
        self.tile(source, range, steps)
    }

    /// The fill handle dragged from `source` over `target`: the run carried
    /// on — numbers as a series, dates by the day, `Item 1` counting —
    /// as Excel's fill does. One undo.
    pub fn fill_series(&mut self, source: CellRange, target: CellRange) -> Result<bool, String> {
        if source == target {
            return Ok(false);
        }
        let name = self.sheet_name();
        let summary = self
            .book
            .fill(&name, &range_text_full(source), &range_text_full(target))
            .map_err(|error| error.to_string())?;
        if summary.cells.is_empty() {
            return Ok(false);
        }
        self.record(1);
        self.after_write()?;
        Ok(true)
    }

    /// Where a fill-handle drag from `source` reaches with the pointer over
    /// `to`: the source grown down, up, right or left — whichever way the
    /// pointer has gone further — to take `to` in.
    pub fn fill_target(source: CellRange, to: CellRef) -> CellRange {
        let below = i64::from(to.row) - i64::from(source.end.row);
        let above = i64::from(source.start.row) - i64::from(to.row);
        let right = i64::from(to.column) - i64::from(source.end.column);
        let left = i64::from(source.start.column) - i64::from(to.column);
        let vertical = below.max(above);
        let horizontal = right.max(left);
        let mut target = source;
        if vertical <= 0 && horizontal <= 0 {
            return source;
        }
        if vertical >= horizontal {
            if below > 0 {
                target.end.row = to.row;
            } else {
                target.start.row = to.row;
            }
        } else if right > 0 {
            target.end.column = to.column;
        } else {
            target.start.column = to.column;
        }
        target
    }

    /// A double-click on the fill handle: the source carried down as far as
    /// the data in the column beside it goes, as Excel does.
    pub fn fill_handle_extent(&self, source: CellRange) -> Option<CellRange> {
        let below = source.end.row + 1;
        let mut end = source.end.row;
        let neighbours = [
            source.start.column.checked_sub(1),
            (source.end.column < MAX_COLUMN).then(|| source.end.column + 1),
        ];
        for column in neighbours.into_iter().flatten() {
            let line = self.filled_line(CellRef::new(column, below), false);
            let mut row = source.end.row;
            while line.contains(&(row + 1)) {
                row += 1;
            }
            end = end.max(row);
        }
        (end > source.end.row).then(|| CellRange {
            start: source.start,
            end: CellRef::new(source.end.column, end),
        })
    }

    // ---- the clipboard ----------------------------------------------------

    /// Copies (or, with `cut`, cuts) `range`: returns the text for the
    /// clipboard, and keeps the cells so that pasting that text pastes them —
    /// formulas adjusted to where they land, formats with them. A cut leaves
    /// the cells where they are until the paste moves them, as in Excel.
    pub fn copy(&mut self, range: CellRange, cut: bool) -> String {
        let range = self.in_use(range);
        let text = self.range_text(range);
        let name = self.sheet_name();
        self.clipboard = self
            .book
            .copy_snapshot(&name, &range_text_full(range), cut)
            .ok()
            .map(|snapshot| Clip {
                snapshot,
                text: text.clone(),
                cut,
                range,
                sheet: self.index,
            });
        text
    }

    /// What was last copied or cut, while a paste would still paste it: its
    /// sheet and range, which Excel outlines.
    pub fn copied(&self) -> Option<(usize, CellRange)> {
        self.clipboard.as_ref().map(|clip| (clip.sheet, clip.range))
    }

    /// The last copy or cut: its sheet, its range, and whether it was a cut.
    pub(crate) fn copied_cells(&self) -> Option<(usize, CellRange, bool)> {
        self.clipboard
            .as_ref()
            .map(|clip| (clip.sheet, clip.range, clip.cut))
    }

    /// The text the last copy put on the clipboard, for a paste from a menu.
    pub fn copied_text(&self) -> Option<String> {
        self.clipboard.as_ref().map(|clip| clip.text.clone())
    }

    /// Lets go of what was copied — Escape, or typing into a cell — as
    /// Excel does.
    pub fn forget_copy(&mut self) {
        self.clipboard = None;
    }

    /// Drag and drop: `source` moved (or with `copy`, copied) so its
    /// top-left lands on `to`, formulas following as a cut and paste moves
    /// them. Returns where it went. What is on the clipboard stays.
    pub fn move_range(
        &mut self,
        source: CellRange,
        to: CellRef,
        copy: bool,
    ) -> Result<CellRange, String> {
        if to == source.start {
            return Ok(source);
        }
        let name = self.sheet_name();
        let snapshot = self
            .book
            .copy_snapshot(&name, &range_text_full(source), !copy)
            .map_err(|error| error.to_string())?;
        let moved = self
            .book
            .paste_snapshot(&snapshot, &name, &reference_text(to), false)
            .map_err(|error| error.to_string())?;
        self.record(1);
        self.after_write()?;
        Ok(moved)
    }

    /// Whether a viewport point is on the selection's border — where Excel
    /// lets a drag pick the cells up — rather than inside it.
    pub fn on_selection_border(
        &self,
        x: f64,
        y: f64,
        scroll: (f64, f64),
        range: CellRange,
    ) -> bool {
        let last = self.grid_end();
        let end = CellRef::new(
            range.end.column.min(last.column),
            range.end.row.min(last.row),
        );
        let (Some((left, top, _, _)), Some((ex, ey, ew, eh))) = (
            self.cell_rect(range.start, scroll),
            self.cell_rect(end, scroll),
        ) else {
            return false;
        };
        let (right, bottom) = (ex + ew, ey + eh);
        let near = |a: f64, b: f64| (a - b).abs() <= 3.0;
        let within_x = x >= left - 3.0 && x <= right + 3.0;
        let within_y = y >= top - 3.0 && y <= bottom + 3.0;
        ((near(x, left) || near(x, right)) && within_y)
            || ((near(y, top) || near(y, bottom)) && within_x)
    }

    /// The engine, for the editing other modules of this crate add.
    pub(crate) fn engine(&self) -> &HeadlessWorkbook {
        &self.book
    }

    pub(crate) fn engine_mut(&mut self) -> &mut HeadlessWorkbook {
        &mut self.book
    }

    /// Notes a write through [`Self::engine_mut`] that took `steps` of the
    /// book's undo steps, and lays the sheet out again.
    pub(crate) fn wrote(&mut self, steps: usize) -> Result<(), String> {
        self.record(steps);
        self.after_write()
    }

    /// Notes an edit that took `steps` of the book's undo steps; a new
    /// edit ends what redo could bring back.
    fn record(&mut self, steps: usize) {
        self.steps.push(steps);
        self.undone.clear();
    }

    /// Pastes the clipboard at `origin`, returning the far corner it filled.
    /// Text this workbook copied pastes its cells; anything else pastes as
    /// tab-separated text typed in from `origin` down and right.
    pub fn paste(&mut self, origin: CellRef, text: &str) -> Result<Option<CellRef>, String> {
        Ok(self
            .paste_over(
                CellRange {
                    start: origin,
                    end: origin,
                },
                text,
            )?
            .map(|range| range.end))
    }

    /// Pastes the clipboard onto `target`: from its top-left, or tiled over
    /// it when it is a whole multiple of what is pasted, as Excel does.
    /// Text this workbook copied pastes its cells, and a cut moves them
    /// once; anything else pastes as text. Returns what was filled.
    pub fn paste_over(
        &mut self,
        target: CellRange,
        text: &str,
    ) -> Result<Option<CellRange>, String> {
        let ours = |copied: &str| clipboard_lines(copied) == clipboard_lines(text);
        if let Some(clip) = self.clipboard.take() {
            if !ours(&clip.text) {
                self.clipboard = Some(clip);
            } else {
                let name = self.sheet_name();
                let pasted = self.book.paste_snapshot(
                    &clip.snapshot,
                    &name,
                    &reference_text(target.start),
                    false,
                );
                let cut = clip.cut;
                match pasted {
                    Ok(pasted) => {
                        // A cut moves once; a copy pastes as often as asked.
                        if !cut {
                            self.clipboard = Some(clip);
                        }
                        let tiled = (!cut).then(|| tiled_over(pasted, target)).flatten();
                        return match tiled {
                            Some(whole) => {
                                self.tile(pasted, whole, 1)?;
                                Ok(Some(whole))
                            }
                            None => {
                                self.record(1);
                                self.after_write()?;
                                Ok(Some(pasted))
                            }
                        };
                    }
                    // An edit since the cut cancelled it, as in Excel.
                    Err(error) if cut => return Err(error.to_string()),
                    Err(_) => {}
                }
            }
        }
        self.paste_text(target, text)
    }

    /// Pastes tab-separated text from `target`'s top-left down and right as
    /// one write, tiled over `target` when it is a whole multiple of it.
    fn paste_text(&mut self, target: CellRange, text: &str) -> Result<Option<CellRange>, String> {
        let origin = target.start;
        let mut values = Vec::new();
        let mut far = origin;
        let date1904 = matches!(
            self.book.workbook().date_system,
            jade_xlsx_reader::DateSystem::Nineteen04
        );
        for (row_offset, row) in clipboard::parse(text).iter().enumerate() {
            for (column_offset, value) in row.iter().enumerate() {
                let reference = CellRef::new(
                    origin.column + column_offset as u32,
                    origin.row + row_offset as u32,
                );
                far.column = far.column.max(reference.column);
                far.row = far.row.max(reference.row);
                values.push((
                    reference_text(reference),
                    typing::read(value, date1904).input,
                ));
            }
        }
        if values.is_empty() {
            return Ok(None);
        }
        let name = self.sheet_name();
        self.book
            .set_cells(&name, &values)
            .map_err(|error| error.to_string())?;
        let pasted = CellRange {
            start: origin,
            end: far,
        };
        match tiled_over(pasted, target) {
            Some(whole) => {
                self.tile(pasted, whole, 1)?;
                Ok(Some(whole))
            }
            None => {
                self.record(1);
                self.after_write()?;
                Ok(Some(pasted))
            }
        }
    }

    /// Empties the cells in `range` that hold anything, keeping formats.
    pub fn clear(&mut self, range: CellRange) -> Result<bool, String> {
        let values: Vec<(String, CellInput)> = self
            .cells_in(range)
            .filter(|cell| cell.formula.is_some() || !matches!(cell.value, CellValue::Empty))
            .map(|cell| (reference_text(cell.reference), CellInput::Blank))
            .collect();
        if values.is_empty() {
            return Ok(false);
        }
        let name = self.sheet_name();
        self.book
            .set_cells(&name, &values)
            .map_err(|error| error.to_string())?;
        self.record(1);
        self.after_write()?;
        Ok(true)
    }

    // ---- formats -------------------------------------------------------

    /// Changes how `range` looks — one write, one undo.
    pub fn format(&mut self, range: CellRange, change: &FormatChange) -> Result<bool, String> {
        let name = self.sheet_name();
        let range = if range.end.row == MAX_ROW || range.end.column == MAX_COLUMN {
            range
        } else {
            self.in_use(range)
        };
        self.book
            .set_format(&name, &range_text_full(range), change)
            .map_err(|error| error.to_string())?;
        self.record(1);
        self.after_write()?;
        Ok(true)
    }

    /// How the cell at `at` looks, as a change that would give a cell its
    /// look: bold, italic and the rest.
    pub fn format_at(&self, at: CellRef) -> FormatChange {
        self.book
            .format_at(&self.sheet_name(), &reference_text(at))
            .unwrap_or_default()
    }

    /// ⌘B, ⌘I, ⌘U, ⌘⇧X: the style turned on through `range`, or off when the
    /// active cell has it, as Excel decides from the active cell.
    pub fn toggle(
        &mut self,
        range: CellRange,
        active: CellRef,
        style: FontToggle,
    ) -> Result<bool, String> {
        let now = self.format_at(active);
        let on = |flag: Option<bool>| Some(!flag.unwrap_or(false));
        let change = match style {
            FontToggle::Bold => FormatChange {
                bold: on(now.bold),
                ..Default::default()
            },
            FontToggle::Italic => FormatChange {
                italic: on(now.italic),
                ..Default::default()
            },
            FontToggle::Underline => FormatChange {
                underline: on(now.underline),
                ..Default::default()
            },
            FontToggle::Strike => FormatChange {
                strike: on(now.strike),
                ..Default::default()
            },
        };
        self.format(range, &change)
    }

    /// Merge & Center: `range` becomes one cell, its top-left's content
    /// centred; a range that is already one merge is split again.
    pub fn merge_and_center(&mut self, range: CellRange) -> Result<bool, String> {
        let name = self.sheet_name();
        let existing = self.book.workbook().sheets[self.index]
            .merges
            .iter()
            .any(|merge| {
                merge.start.column <= range.end.column
                    && range.start.column <= merge.end.column
                    && merge.start.row <= range.end.row
                    && range.start.row <= merge.end.row
            });
        if existing {
            self.book
                .unmerge_cells(&name, &range_text_full(range))
                .map_err(|error| error.to_string())?;
            return self.restructured();
        }
        if range.start == range.end {
            return Ok(false);
        }
        self.book
            .merge_cells(&name, &range_text_full(range))
            .map_err(|error| error.to_string())?;
        let center = FormatChange {
            horizontal: Some(HorizontalAlignment::Center),
            ..Default::default()
        };
        let steps = if self
            .book
            .set_format(&name, &range_text_full(range), &center)
            .is_ok()
        {
            2
        } else {
            1
        };
        self.record(steps);
        self.after_write()?;
        Ok(true)
    }

    /// The drawing object (chart, picture, shape) under a point of the
    /// viewport (sheet px from its top-left, headers included) at
    /// `scroll`: the one on top, as Excel picks it.
    pub fn object_at(&self, x: f64, y: f64, scroll: (f64, f64)) -> Option<u32> {
        let metrics = self.layout.header_metrics();
        if x < metrics.row_width || y < metrics.column_height {
            return None;
        }
        let (sheet_x, sheet_y) = self.layout.sheet_point_in_viewport(
            x - metrics.row_width,
            y - metrics.column_height,
            scroll.0,
            scroll.1,
        );
        self.layout.shape_at(sheet_x, sheet_y).map(|shape| shape.id)
    }

    /// Where the object `id` is on the sheet, in sheet px (x, y, width,
    /// height) from the top-left of the cells.
    pub fn object_bounds(&self, id: u32) -> Option<(f64, f64, f64, f64)> {
        let everything = Rect {
            x: 0.0,
            y: 0.0,
            width: f64::MAX / 4.0,
            height: f64::MAX / 4.0,
        };
        let shape = self
            .layout
            .shapes_in(everything)
            .find(|shape| shape.id == id)?;
        Some((
            shape.rect.x,
            shape.rect.y,
            shape.rect.width,
            shape.rect.height,
        ))
    }

    /// Where the object `id` is in the viewport at `scroll` (sheet px,
    /// headers included), for its frame and handles.
    pub fn object_rect(&self, id: u32, scroll: (f64, f64)) -> Option<(f64, f64, f64, f64)> {
        let (x, y, width, height) = self.object_bounds(id)?;
        let metrics = self.layout.header_metrics();
        // Frozen panes do not scroll.
        let x = if x < self.layout.frozen_width() {
            x
        } else {
            x - scroll.0
        };
        let y = if y < self.layout.frozen_height() {
            y
        } else {
            y - scroll.1
        };
        Some((
            x + metrics.row_width,
            y + metrics.column_height,
            width,
            height,
        ))
    }

    /// The cell a sheet point is in, and how far into it in EMUs, growing
    /// the laid-out grid when the point is past it.
    fn anchor_point(&mut self, x: f64, y: f64) -> (CellRef, (i64, i64)) {
        const EMU_PER_PX: f64 = 9_525.0;
        let (x, y) = (x.max(0.0), y.max(0.0));
        for _ in 0..64 {
            let column = self.layout.column_at(x);
            let row = self.layout.row_at(y);
            if let (Some(column), Some(row)) = (column, row) {
                let left = self.layout.column(column).map_or(0.0, |entry| entry.offset);
                let top = self.layout.row(row).map_or(0.0, |entry| entry.offset);
                return (
                    CellRef::new(column, row),
                    (
                        ((x - left) * EMU_PER_PX).round() as i64,
                        ((y - top) * EMU_PER_PX).round() as i64,
                    ),
                );
            }
            let (columns, rows) = self.extent;
            let wider = if column.is_none() { 16 } else { 0 };
            let taller = if row.is_none() { 64 } else { 0 };
            self.reach(CellRef::new(
                (columns + wider).min(MAX_COLUMN),
                (rows + taller).min(MAX_ROW),
            ));
        }
        (CellRef::new(0, 0), (0, 0))
    }

    /// Puts the object `id` at `bounds` (sheet px): moved by its border,
    /// or resized by a handle.
    pub fn place_object(&mut self, id: u32, bounds: (f64, f64, f64, f64)) -> Result<bool, String> {
        let (x, y, width, height) = bounds;
        let (from, from_offset) = self.anchor_point(x, y);
        let (to, to_offset) = self.anchor_point(x + width, y + height);
        let anchor = ShapeAnchor {
            from,
            from_offset,
            to,
            to_offset,
        };
        let name = self.sheet_name();
        self.book
            .place_shape(&name, id, &anchor)
            .map_err(|error| error.to_string())?;
        self.record(1);
        self.after_write()?;
        Ok(true)
    }

    /// Deletes the object `id`, as Delete does to a selected one.
    pub fn delete_object(&mut self, id: u32) -> Result<bool, String> {
        let name = self.sheet_name();
        self.book
            .delete_shape(&name, id)
            .map_err(|error| error.to_string())?;
        self.record(1);
        self.after_write()?;
        Ok(true)
    }

    /// Insert ▸ Chart over `range`: the new chart's id.
    pub fn insert_chart(&mut self, range: CellRange, kind: ChartKind) -> Result<u32, String> {
        let name = self.sheet_name();
        let id = self
            .book
            .insert_chart(&name, &range_text_full(range), kind)
            .map_err(|error| error.to_string())?;
        self.record(1);
        self.after_write()?;
        // The grid reaches far enough to lay the chart out.
        if let Some(to) = self.book.workbook().sheets[self.index]
            .shapes
            .iter()
            .find(|shape| shape.id == id)
            .and_then(|shape| shape.to)
        {
            self.reach(to);
        }
        Ok(id)
    }

    /// The chart `id` as Chart Design shows it.
    pub fn chart(&self, id: u32) -> Option<ChartInfo> {
        let chart = self.book.workbook().sheets[self.index]
            .shapes
            .iter()
            .find(|shape| shape.id == id)?
            .chart
            .as_ref()?;
        let legend = match chart.legend.as_deref() {
            Some("r") => Some(LegendPosition::Right),
            Some("l") => Some(LegendPosition::Left),
            Some("t") | Some("tr") => Some(LegendPosition::Top),
            Some("b") => Some(LegendPosition::Bottom),
            _ => None,
        };
        // The range its series' references cover, on this sheet.
        let sheet = self.sheet_name();
        let mut bounds: Option<CellRange> = None;
        for series in &chart.series {
            for data in [
                &series.name,
                &series.categories,
                &series.values,
                &series.x_values,
            ] {
                let Some(formula) = data.formula.as_deref() else {
                    continue;
                };
                let Some((on, reference)) = formula.rsplit_once('!') else {
                    continue;
                };
                let on = on.trim_matches('\'').replace("''", "'");
                if on != sheet {
                    continue;
                }
                let Ok(range) = CellRange::parse(&reference.replace('$', "")) else {
                    continue;
                };
                bounds = Some(match bounds {
                    None => range,
                    Some(around) => CellRange {
                        start: CellRef::new(
                            around.start.column.min(range.start.column),
                            around.start.row.min(range.start.row),
                        ),
                        end: CellRef::new(
                            around.end.column.max(range.end.column),
                            around.end.row.max(range.end.row),
                        ),
                    },
                });
            }
        }
        Some(ChartInfo {
            kind: chart_kind(chart),
            title: chart.title.clone(),
            legend,
            data: bounds.map(range_text_full),
        })
    }

    /// Chart Design: the chart `id`'s type, title, legend or data changed.
    pub fn set_chart(&mut self, id: u32, change: &ChartChange) -> Result<bool, String> {
        let name = self.sheet_name();
        self.book
            .set_chart(&name, id, change)
            .map_err(|error| error.to_string())?;
        self.record(1);
        self.after_write()?;
        Ok(true)
    }

    /// The sheet's conditional formats as Excel's Rules Manager lists them:
    /// first the one that wins.
    pub fn conditional_formats(&self) -> Vec<ConditionalFormatView> {
        self.book
            .conditional_formats(&self.sheet_name())
            .unwrap_or_default()
    }

    /// Adds a rule from Home ▸ Conditional Formatting over `range`, ahead
    /// of the sheet's others.
    pub fn add_conditional_format(
        &mut self,
        range: CellRange,
        rule: &NewConditionalRule,
        style: ConditionalStyle,
    ) -> Result<bool, String> {
        let name = self.sheet_name();
        self.book
            .add_conditional_format(&name, &range_text_full(range), rule, style)
            .map_err(|error| error.to_string())?;
        self.record(1);
        self.after_write()?;
        Ok(true)
    }

    /// Deletes the rule at `index` in [`Self::conditional_formats`].
    pub fn delete_conditional_format(&mut self, index: usize) -> Result<bool, String> {
        let name = self.sheet_name();
        self.book
            .delete_conditional_format(&name, index)
            .map_err(|error| error.to_string())?;
        self.record(1);
        self.after_write()?;
        Ok(true)
    }

    /// Clear Rules from Selected Cells (`range`) or from Entire Sheet.
    pub fn clear_conditional_formats(&mut self, range: Option<CellRange>) -> Result<bool, String> {
        let name = self.sheet_name();
        let range = range.map(range_text_full);
        if !self
            .book
            .clear_conditional_formats(&name, range.as_deref())
            .map_err(|error| error.to_string())?
        {
            return Ok(false);
        }
        self.record(1);
        self.after_write()?;
        Ok(true)
    }

    /// Sort A to Z (or Z to A) by the column of `at`: over `range`, or — a
    /// single cell — the block of data round it, keeping a header row on
    /// top when the first row reads as one, as Excel guesses.
    pub fn sort(
        &mut self,
        range: CellRange,
        at: CellRef,
        descending: bool,
    ) -> Result<bool, String> {
        let range = if range.start == range.end {
            match self.current_region(at) {
                Some(region) => region,
                None => return Ok(false),
            }
        } else {
            self.in_use(range)
        };
        if range.start.row == range.end.row {
            return Ok(false);
        }
        let header = self.looks_like_header(range);
        let request = jade_xlsx_headless_api::SortRequest {
            reference: range_text_full(range),
            header,
            case_sensitive: false,
            locale: None,
            phonetic: None,
            keys: vec![jade_xlsx_headless_api::SortKey {
                column: at.column.clamp(range.start.column, range.end.column) - range.start.column,
                descending,
            }],
        };
        let name = self.sheet_name();
        let summary = self
            .book
            .sort_range(&name, &request)
            .map_err(|error| error.to_string())?;
        if summary.moved_rows == 0 {
            return Ok(false);
        }
        self.restructured()
    }

    /// Whether the range's first row is a header: all text over a row that
    /// holds something other than text.
    fn looks_like_header(&self, range: CellRange) -> bool {
        let first = CellRange {
            start: range.start,
            end: CellRef::new(range.end.column, range.start.row),
        };
        let second = CellRange {
            start: CellRef::new(range.start.column, range.start.row + 1),
            end: CellRef::new(range.end.column, range.start.row + 1),
        };
        let mut first_cells = self
            .cells_in(first)
            .filter(|cell| !cell.value.is_empty())
            .peekable();
        let all_text = first_cells.peek().is_some()
            && first_cells
                .all(|cell| matches!(cell.value, CellValue::Text(_)) && cell.formula.is_none());
        all_text
            && self
                .cells_in(second)
                .any(|cell| !matches!(cell.value, CellValue::Text(_) | CellValue::Empty))
    }

    // ---- filters --------------------------------------------------------

    /// The filter button (▼) under a viewport point, if there is one.
    pub fn filter_button_at(&self, x: f64, y: f64, scroll: (f64, f64)) -> Option<FilterButton> {
        let metrics = self.layout.header_metrics();
        self.layout
            .filter_buttons
            .iter()
            .find(|button| {
                let left = button.rect.x + metrics.row_width
                    - if button.reference.column < self.layout.frozen_columns {
                        0.0
                    } else {
                        scroll.0
                    };
                let top = button.rect.y + metrics.column_height
                    - if button.reference.row < self.layout.frozen_rows {
                        0.0
                    } else {
                        scroll.1
                    };
                x >= left
                    && x <= left + button.rect.width
                    && y >= top
                    && y <= top + button.rect.height
            })
            .cloned()
    }

    fn filter_target(button: &FilterButton) -> AutoFilterTarget {
        match &button.target {
            FilterButtonTarget::Worksheet => AutoFilterTarget::Worksheet,
            FilterButtonTarget::Table { name } => AutoFilterTarget::Table { name: name.clone() },
        }
    }

    /// The values a filter button offers, with which are showing.
    pub fn filter_options(&self, button: &FilterButton) -> Result<AutoFilterOptions, String> {
        self.book
            .auto_filter_options(
                &self.sheet_name(),
                &Self::filter_target(button),
                button.column,
                None,
            )
            .map_err(|error| error.to_string())
    }

    /// Shows only the rows whose cell in the button's column reads one of
    /// `values` (and blank ones with `blank`); the rest are hidden.
    pub fn filter_values(
        &mut self,
        button: &FilterButton,
        values: Vec<String>,
        blank: bool,
    ) -> Result<bool, String> {
        let change = AutoFilterChange::Values {
            target: Self::filter_target(button),
            reference: None,
            column: button.column,
            values,
            include_blank: blank,
        };
        self.change_filter(&change)
    }

    /// Takes the button's column out of the filter.
    pub fn clear_filter(&mut self, button: &FilterButton) -> Result<bool, String> {
        let change = AutoFilterChange::Clear {
            target: Self::filter_target(button),
            column: Some(button.column),
        };
        self.change_filter(&change)
    }

    /// ⌘⇧F: filter buttons on the data round `at` (or on `range`), or off
    /// again.
    pub fn toggle_filter(&mut self, range: CellRange) -> Result<bool, String> {
        let name = self.sheet_name();
        let range = self.in_use(range);
        self.book
            .toggle_auto_filter(&name, &range_text_full(range))
            .map_err(|error| error.to_string())?;
        self.restructured()
    }

    fn change_filter(&mut self, change: &AutoFilterChange) -> Result<bool, String> {
        let name = self.sheet_name();
        self.book
            .change_auto_filter(&name, change)
            .map_err(|error| error.to_string())?;
        self.restructured()
    }

    /// Sort from a filter button: the filtered range by its column, the
    /// header row staying.
    pub fn sort_filter(&mut self, button: &FilterButton, descending: bool) -> Result<bool, String> {
        let range = button.range;
        let request = jade_xlsx_headless_api::SortRequest {
            reference: range_text_full(range),
            header: true,
            case_sensitive: false,
            locale: None,
            phonetic: None,
            keys: vec![jade_xlsx_headless_api::SortKey {
                column: button.column,
                descending,
            }],
        };
        let name = self.sheet_name();
        let summary = self
            .book
            .sort_range(&name, &request)
            .map_err(|error| error.to_string())?;
        if summary.moved_rows == 0 {
            return Ok(false);
        }
        self.restructured()
    }

    /// Several format changes as one edit, one undo.
    fn format_all(&mut self, changes: &[(CellRange, FormatChange)]) -> Result<bool, String> {
        let name = self.sheet_name();
        let mut steps = 0;
        for (range, change) in changes {
            let range = if range.end.row == MAX_ROW || range.end.column == MAX_COLUMN {
                *range
            } else {
                self.in_use(*range)
            };
            self.book
                .set_format(&name, &range_text_full(range), change)
                .map_err(|error| error.to_string())?;
            steps += 1;
        }
        if steps == 0 {
            return Ok(false);
        }
        self.record(steps);
        self.after_write()?;
        Ok(true)
    }

    /// The Home tab's alignment buttons: left, centre or right — or back to
    /// General when the active cell is already aligned so.
    pub fn align(
        &mut self,
        range: CellRange,
        active: CellRef,
        horizontal: HorizontalAlignment,
    ) -> Result<bool, String> {
        let now = self.format_at(active).horizontal;
        let horizontal = if now == Some(horizontal) {
            HorizontalAlignment::General
        } else {
            horizontal
        };
        self.format(
            range,
            &FormatChange {
                horizontal: Some(horizontal),
                ..Default::default()
            },
        )
    }

    /// Wrap Text, on or off as the active cell decides.
    pub fn toggle_wrap(&mut self, range: CellRange, active: CellRef) -> Result<bool, String> {
        let wrap = !self.format_at(active).wrap_text.unwrap_or(false);
        self.format(
            range,
            &FormatChange {
                wrap_text: Some(wrap),
                ..Default::default()
            },
        )
    }

    /// Increase (or Decrease) Decimal: the active cell's number format with
    /// one more (or fewer) place after the point, on every cell.
    pub fn decimals(
        &mut self,
        range: CellRange,
        active: CellRef,
        more: bool,
    ) -> Result<bool, String> {
        let code = self
            .format_at(active)
            .number_format
            .unwrap_or_else(|| "General".to_string());
        let shown = self.display_text(active);
        let next = adjust_decimals(&code, &shown, more);
        if next == code {
            return Ok(false);
        }
        self.format(
            range,
            &FormatChange {
                number_format: Some(next),
                ..Default::default()
            },
        )
    }

    /// Increase (or Decrease) Font Size, through Excel's list of sizes.
    pub fn grow_font(
        &mut self,
        range: CellRange,
        active: CellRef,
        larger: bool,
    ) -> Result<bool, String> {
        const SIZES: &[f32] = &[
            8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 22.0, 24.0, 26.0, 28.0, 36.0, 48.0,
            72.0,
        ];
        let now = self.format_at(active).font_size.unwrap_or(11.0);
        let next = if larger {
            SIZES
                .iter()
                .copied()
                .find(|size| *size > now + 0.01)
                .unwrap_or(now + 10.0)
        } else {
            SIZES
                .iter()
                .rev()
                .copied()
                .find(|size| *size < now - 0.01)
                .unwrap_or((now - 1.0).max(1.0))
        };
        self.format(
            range,
            &FormatChange {
                font_size: Some(next),
                ..Default::default()
            },
        )
    }

    /// The fill (`RRGGBB`, or `None` for No Fill).
    pub fn fill_color(&mut self, range: CellRange, color: Option<&str>) -> Result<bool, String> {
        self.format(
            range,
            &FormatChange {
                fill_color: Some(color.unwrap_or_default().to_string()),
                ..Default::default()
            },
        )
    }

    /// The font colour (`RRGGBB`).
    pub fn font_color(&mut self, range: CellRange, color: &str) -> Result<bool, String> {
        self.format(
            range,
            &FormatChange {
                font_color: Some(color.to_string()),
                ..Default::default()
            },
        )
    }

    /// The Borders button's choices.
    pub fn borders(&mut self, range: CellRange, borders: Borders) -> Result<bool, String> {
        let thin = |thick: bool| BorderLine {
            style: if thick {
                BorderStyle::Medium
            } else {
                BorderStyle::Thin
            },
            color: None,
        };
        let edges = |line: BorderLine| FormatChange {
            border_left: Some(line.clone()),
            border_right: Some(line.clone()),
            border_top: Some(line.clone()),
            border_bottom: Some(line),
            ..Default::default()
        };
        let range = if range.end.row == MAX_ROW || range.end.column == MAX_COLUMN {
            self.in_use(range)
        } else {
            range
        };
        let top = CellRange {
            start: range.start,
            end: CellRef::new(range.end.column, range.start.row),
        };
        let bottom = CellRange {
            start: CellRef::new(range.start.column, range.end.row),
            end: range.end,
        };
        let left = CellRange {
            start: range.start,
            end: CellRef::new(range.start.column, range.end.row),
        };
        let right = CellRange {
            start: CellRef::new(range.end.column, range.start.row),
            end: range.end,
        };
        let outline = |thick: bool| {
            vec![
                (
                    top,
                    FormatChange {
                        border_top: Some(thin(thick)),
                        ..Default::default()
                    },
                ),
                (
                    bottom,
                    FormatChange {
                        border_bottom: Some(thin(thick)),
                        ..Default::default()
                    },
                ),
                (
                    left,
                    FormatChange {
                        border_left: Some(thin(thick)),
                        ..Default::default()
                    },
                ),
                (
                    right,
                    FormatChange {
                        border_right: Some(thin(thick)),
                        ..Default::default()
                    },
                ),
            ]
        };
        let changes = match borders {
            Borders::All => vec![(range, edges(thin(false)))],
            Borders::None => vec![(range, edges(BorderLine::none()))],
            Borders::Outline => outline(false),
            Borders::ThickOutline => outline(true),
            Borders::Bottom => vec![(
                bottom,
                FormatChange {
                    border_bottom: Some(thin(false)),
                    ..Default::default()
                },
            )],
        };
        self.format_all(&changes)
    }

    // ---- notes ------------------------------------------------------------

    /// Puts a note on `at` (replacing its note), or with empty text takes
    /// it away.
    pub fn set_note(&mut self, at: CellRef, text: &str, author: &str) -> Result<bool, String> {
        let name = self.sheet_name();
        if text.trim().is_empty() {
            let deleted = self
                .book
                .delete_note(&name, &reference_text(at))
                .map_err(|error| error.to_string())?;
            return if deleted {
                self.restructured()
            } else {
                Ok(false)
            };
        }
        if self.note(at) == Some(text) {
            return Ok(false);
        }
        self.book
            .set_note(&name, &reference_text(at), text, author)
            .map_err(|error| error.to_string())?;
        self.restructured()
    }

    // ---- rows, columns and sheets -----------------------------------------

    /// Inserts `count` rows above `row` (0-based); what was below moves
    /// down and formulas follow.
    pub fn insert_rows(&mut self, row: u32, count: u32) -> Result<bool, String> {
        let name = self.sheet_name();
        self.book
            .insert_rows(&name, row + 1, count)
            .map_err(|error| error.to_string())?;
        self.restructured()
    }

    pub fn delete_rows(&mut self, row: u32, count: u32) -> Result<bool, String> {
        let name = self.sheet_name();
        self.book
            .delete_rows(&name, row + 1, count)
            .map_err(|error| error.to_string())?;
        self.restructured()
    }

    /// Inserts `count` columns left of `column` (0-based).
    pub fn insert_columns(&mut self, column: u32, count: u32) -> Result<bool, String> {
        let name = self.sheet_name();
        self.book
            .insert_columns(&name, &column_label(column), count)
            .map_err(|error| error.to_string())?;
        self.restructured()
    }

    pub fn delete_columns(&mut self, column: u32, count: u32) -> Result<bool, String> {
        let name = self.sheet_name();
        self.book
            .delete_columns(&name, &column_label(column), count)
            .map_err(|error| error.to_string())?;
        self.restructured()
    }

    fn restructured(&mut self) -> Result<bool, String> {
        self.record(1);
        self.after_write()?;
        Ok(true)
    }

    /// Sets the columns' (or rows') size from a header edge dragged to
    /// `size_px` sheet px.
    pub fn resize(
        &mut self,
        axis: Axis,
        first: u32,
        last: u32,
        size_px: f64,
    ) -> Result<bool, String> {
        let gesture = jade_xlsx_layout::resize::ResizeGesture {
            hit: jade_xlsx_layout::resize::ResizeHit {
                axis,
                index: first,
                size_px,
            },
            pointer_start: 0.0,
            size_px,
        };
        let stored = gesture.stored_size(self.layout.max_digit_width_px);
        let name = self.sheet_name();
        self.book
            .set_axis_size(&name, axis, first, last, stored)
            .map_err(|error| error.to_string())?;
        self.restructured()
    }

    /// Hide: the rows or columns down to nothing.
    pub fn hide(&mut self, axis: Axis, first: u32, last: u32) -> Result<bool, String> {
        let name = self.sheet_name();
        self.book
            .set_axis_size(&name, axis, first, last, 0.0)
            .map_err(|error| error.to_string())?;
        self.restructured()
    }

    /// Unhide: the hidden rows or columns among `first..=last` back at the
    /// sheet's standard size.
    pub fn unhide(&mut self, axis: Axis, first: u32, last: u32) -> Result<bool, String> {
        let entries = match axis {
            Axis::Rows => &self.layout.rows,
            Axis::Columns => &self.layout.columns,
        };
        let standard = match axis {
            Axis::Rows => 15.0,
            Axis::Columns => 8.43,
        };
        let sizes: BTreeMap<u32, f32> = entries
            .iter()
            .filter(|entry| entry.index >= first && entry.index <= last)
            .filter(|entry| entry.hidden || entry.size <= 0.0)
            .map(|entry| (entry.index, standard))
            .collect();
        if sizes.is_empty() {
            return Ok(false);
        }
        let name = self.sheet_name();
        self.book
            .set_axis_sizes(&name, axis, &sizes)
            .map_err(|error| error.to_string())?;
        self.restructured()
    }

    /// Moves the sheet at `index` to `to` in the tab order, keeping it
    /// selected.
    pub fn move_sheet(&mut self, index: usize, to: usize) -> Result<bool, String> {
        let names = self.sheet_names();
        let Some(name) = names.get(index) else {
            return Err(format!("no sheet at index {index}"));
        };
        if to >= names.len() || to == index {
            return Ok(false);
        }
        self.book
            .move_sheet(name, to)
            .map_err(|error| error.to_string())?;
        self.record(1);
        self.dirty = true;
        self.select_sheet(to)?;
        Ok(true)
    }

    /// A header edge double-clicked: the columns as wide as what they hold.
    pub fn auto_fit_columns(&mut self, first: u32, last: u32) -> Result<bool, String> {
        let sheet = &self.book.workbook().sheets[self.index];
        let sizes = jade_xlsx_layout::resize::auto_fit_columns(
            sheet,
            &self.book.workbook().styles,
            first,
            last,
            &self.layout_options(),
        );
        if sizes.is_empty() {
            return Ok(false);
        }
        let name = self.sheet_name();
        self.book
            .set_axis_sizes(&name, Axis::Columns, &sizes)
            .map_err(|error| error.to_string())?;
        self.restructured()
    }

    /// Adds a sheet after the current one and selects it, named as Excel
    /// names a new sheet (`Sheet4`).
    pub fn add_sheet(&mut self) -> Result<usize, String> {
        let names = self.sheet_names();
        let name = (1..)
            .map(|n| format!("Sheet{n}"))
            .find(|name| !names.iter().any(|taken| taken.eq_ignore_ascii_case(name)))
            .unwrap_or_default();
        let after = self.sheet_name();
        let index = self
            .book
            .add_sheet(&name, Some(&after))
            .map_err(|error| error.to_string())?;
        self.record(1);
        self.dirty = true;
        self.select_sheet(index)?;
        Ok(index)
    }

    pub fn rename_sheet(&mut self, index: usize, name: &str) -> Result<bool, String> {
        let old = self
            .sheet_names()
            .get(index)
            .cloned()
            .ok_or_else(|| format!("no sheet at index {index}"))?;
        if old == name {
            return Ok(false);
        }
        self.book
            .rename_sheet(&old, name)
            .map_err(|error| error.to_string())?;
        self.restructured()
    }

    /// Deletes a sheet; the last visible one cannot go.
    pub fn delete_sheet(&mut self, index: usize) -> Result<bool, String> {
        let name = self
            .sheet_names()
            .get(index)
            .cloned()
            .ok_or_else(|| format!("no sheet at index {index}"))?;
        self.book
            .delete_sheet(&name)
            .map_err(|error| error.to_string())?;
        self.record(1);
        self.dirty = true;
        let count = self.book.workbook().sheets.len();
        let next = (0..count)
            .map(|offset| (index + offset) % count)
            .chain(std::iter::once(0))
            .find(|candidate| {
                self.book.workbook().sheets[*candidate].visibility == SheetVisibility::Visible
            })
            .unwrap_or(0);
        self.select_sheet(next.min(count - 1))?;
        Ok(true)
    }

    fn layout_options(&self) -> LayoutOptions {
        let workbook = self.book.workbook();
        let measure: Rc<dyn jade_xlsx_layout::TextMeasure> = match self.backend.surface(1, 1) {
            Some(surface) => Rc::new(PainterMeasure::new(surface)),
            None => LayoutOptions::default().measure,
        };
        LayoutOptions {
            min_column_count: self.extent.0,
            min_row_count: self.extent.1,
            date_system: workbook.date_system,
            measure,
            ..LayoutOptions::default()
        }
        .with_normal_font_metrics(&workbook.styles)
    }

    fn after_write(&mut self) -> Result<(), String> {
        self.dirty = true;
        // An undone sheet added, or a redone one deleted, can leave the index
        // past the end.
        let count = self.book.workbook().sheets.len();
        if self.index >= count {
            self.index = count.saturating_sub(1);
            self.changes = None;
        }
        self.relayout()
    }

    /// Undoes the last edit, however many of the book's steps it took.
    pub fn undo(&mut self) -> Result<bool, String> {
        let steps = self.steps.pop().unwrap_or(1);
        let mut undone = 0;
        for _ in 0..steps {
            if !self.book.undo().map_err(|error| error.to_string())? {
                break;
            }
            undone += 1;
        }
        if undone > 0 {
            self.undone.push(undone);
            self.after_write()?;
        }
        Ok(undone > 0)
    }

    pub fn redo(&mut self) -> Result<bool, String> {
        let steps = self.undone.pop().unwrap_or(1);
        let mut redone = 0;
        for _ in 0..steps {
            if !self.book.redo().map_err(|error| error.to_string())? {
                break;
            }
            redone += 1;
        }
        if redone > 0 {
            self.steps.push(redone);
            self.after_write()?;
        }
        Ok(redone > 0)
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Counts the workbook as saved: written out some other way than
    /// [`Self::save_bytes`], as CSV say.
    pub(crate) fn mark_saved(&mut self) {
        self.dirty = false;
    }

    /// The workbook as `.xlsx` bytes, as saving writes it; marks it saved.
    pub fn save_bytes(&mut self) -> Result<Vec<u8>, String> {
        let bytes = self.book.save_bytes().map_err(|error| error.to_string())?;
        self.dirty = false;
        Ok(bytes)
    }

    /// Paints the part of the sheet under `scroll` (sheet px) onto a
    /// `width` × `height` surface at `scale` device px per sheet px, with
    /// the selection and what else the overlay asks for drawn over it.
    pub fn render_viewport(
        &self,
        scroll: (f64, f64),
        width: u32,
        height: u32,
        scale: f64,
        overlay: &Overlay,
    ) -> Option<B::Painter> {
        let (width, height) = (width.max(1), height.max(1));
        let mut surface = self.surface.borrow_mut();
        if !matches!(&*surface, Some((_, w, h)) if *w == width && *h == height) {
            *surface = Some((self.backend.surface(width, height)?, width, height));
        }
        let (painter, _, _) = surface.as_ref()?;
        let metrics = self.layout.header_metrics();
        let viewport = Rect {
            x: scroll.0.max(0.0),
            y: scroll.1.max(0.0),
            width: (f64::from(width) / scale - metrics.row_width).max(0.0),
            height: (f64::from(height) / scale - metrics.column_height).max(0.0),
        };
        let selection = overlay.selection.map(|selection| GridSelection {
            range: if selection.is_cell() {
                selection.range()
            } else {
                self.with_merges(selection.range())
            },
            active: selection.active,
            fill_preview: overlay.preview,
            fill_handle: overlay.preview.is_none(),
        });
        painter.save();
        let mut painted = painter.scale(scale, scale).is_ok()
            && render_grid(
                painter,
                &self.layout,
                viewport,
                selection,
                &RenderOptions::default(),
            )
            .is_ok();
        if let Some(marquee) = overlay.marquee {
            painted &= self.draw_marquee(painter, marquee, viewport).is_ok();
        }
        if let Some(gesture) = overlay.resize {
            jade_xlsx_render::render_resize_guide(painter, &self.layout, viewport, gesture);
        }
        painter.restore();
        painted.then(|| painter.clone())
    }

    // ---- the sheet in pieces ---------------------------------------------------
    //
    // A shell that keeps what it has painted asks for the sheet in pieces:
    // the cells as tiles fixed in sheet space, which scrolling only moves; the
    // header bands, which are small; and the selection as shapes it draws
    // itself, so moving it repaints nothing. [`Self::render_viewport`] paints
    // the same things in one picture.

    /// A surface `width`×`height` from the pool, reused while a piece of that
    /// size is asked for again.
    fn piece(&self, width: u32, height: u32) -> Option<B::Painter> {
        let (width, height) = (width.max(1), height.max(1));
        let mut pieces = self.pieces.borrow_mut();
        if let Some((painter, ..)) = pieces.iter().find(|(_, w, h)| (*w, *h) == (width, height)) {
            return Some(painter.clone());
        }
        let painter = self.backend.surface(width, height)?;
        // A tile and the two bands at most, with room for a resize.
        if pieces.len() >= 4 {
            pieces.remove(0);
        }
        pieces.push((painter.clone(), width, height));
        Some(painter)
    }

    /// The panes a viewport of `size` (sheet px, headers included) at
    /// `scroll` shows: where each sits in the viewport and the part of the
    /// sheet it shows, both in sheet px, as `((x, y), (x, y, width,
    /// height))`. One pane, or up to four with frozen rows and columns: the
    /// scrolling one, the frozen rows above it, the frozen columns beside it
    /// and the corner they share.
    pub fn panes(&self, scroll: (f64, f64), size: (f64, f64)) -> Vec<Pane> {
        let metrics = self.layout.header_metrics();
        let viewport = Rect {
            x: scroll.0.max(0.0),
            y: scroll.1.max(0.0),
            width: (size.0 - metrics.row_width).max(0.0),
            height: (size.1 - metrics.column_height).max(0.0),
        };
        let regions = self.layout.viewport_regions(viewport);
        [
            (regions.main, (regions.frozen_width, regions.frozen_height)),
            (regions.frozen_rows, (regions.frozen_width, 0.0)),
            (regions.frozen_columns, (0.0, regions.frozen_height)),
            (regions.corner, (0.0, 0.0)),
        ]
        .into_iter()
        .filter(|(region, _)| region.width > 0.0 && region.height > 0.0)
        .map(|(region, offset)| {
            (
                (
                    metrics.row_width + offset.0,
                    metrics.column_height + offset.1,
                ),
                (region.x, region.y, region.width, region.height),
            )
        })
        .collect()
    }

    /// Paints the cells in `region` (sheet px, as `(x, y, width, height)`) —
    /// no headers, no selection — onto a `width`×`height` device px surface,
    /// `scale` device px to a sheet px. What [`Self::render_viewport`] paints
    /// under its headers is these, side by side.
    pub fn render_cells(
        &self,
        region: SheetRect,
        width: u32,
        height: u32,
        scale: f64,
    ) -> Option<B::Painter> {
        let painter = self.piece(width, height)?;
        let region = Rect {
            x: region.0,
            y: region.1,
            width: region.2,
            height: region.3,
        };
        painter.save();
        let painted = painter.scale(scale, scale).is_ok()
            && jade_xlsx_render::render_viewport(
                &painter,
                &self.layout,
                region,
                &RenderOptions::default(),
            )
            .is_ok();
        painter.restore();
        painted.then_some(painter)
    }

    /// The column letters across the top of a viewport of `size` (sheet px,
    /// headers included) at `scroll`, with the corner, as a band `width`
    /// device px wide and as tall as the headers are and the line under
    /// them, which reaches a px past them; `selection`'s columns highlighted.
    /// It goes over the row band.
    pub fn render_column_headers(
        &self,
        scroll: (f64, f64),
        size: (f64, f64),
        width: u32,
        scale: f64,
        selection: Option<Selection>,
    ) -> Option<B::Painter> {
        let metrics = self.layout.header_metrics();
        let height = ((metrics.column_height + 1.0) * scale).ceil() as u32;
        self.render_header_band(scroll, size, (width, height), scale, selection, true)
    }

    /// The row numbers down the left of the same viewport, from the bottom
    /// of the headers, as a band as wide as they are and the line beside
    /// them, and `height` device px tall.
    pub fn render_row_headers(
        &self,
        scroll: (f64, f64),
        size: (f64, f64),
        height: u32,
        scale: f64,
        selection: Option<Selection>,
    ) -> Option<B::Painter> {
        let metrics = self.layout.header_metrics();
        let width = ((metrics.row_width + 1.0) * scale).ceil() as u32;
        self.render_header_band(scroll, size, (width, height), scale, selection, false)
    }

    /// One header band, painted as `jade_xlsx_render::render_grid` paints
    /// them both round a viewport.
    fn render_header_band(
        &self,
        scroll: (f64, f64),
        size: (f64, f64),
        device: (u32, u32),
        scale: f64,
        selection: Option<Selection>,
        columns: bool,
    ) -> Option<B::Painter> {
        let painter = self.piece(device.0, device.1)?;
        let metrics = self.layout.header_metrics();
        let viewport = Rect {
            x: scroll.0.max(0.0),
            y: scroll.1.max(0.0),
            width: (size.0 - metrics.row_width).max(0.0),
            height: (size.1 - metrics.column_height).max(0.0),
        };
        let selected = selection.map(|selection| self.selection_range(selection));
        painter.save();
        let painted = painter.scale(scale, scale).is_ok()
            // The row band starts below the column band.
            && (columns || painter.translate(0.0, -metrics.column_height).is_ok())
            && paint_header_band(&painter, &self.layout, viewport, selected, columns).is_ok();
        painter.restore();
        painted.then_some(painter)
    }

    /// The range `selection` reads as on the grid: grown over the merges it
    /// touches, or, for one cell anchoring a merge, the whole merge.
    fn selection_range(&self, selection: Selection) -> CellRange {
        if selection.is_cell() {
            if let Some(merge) = self
                .layout
                .cell(selection.active)
                .and_then(|cell| cell.merge)
            {
                return merge;
            }
            return selection.range();
        }
        self.with_merges(selection.range())
    }

    /// Where `selection` is drawn, in sheet px as `(x, y, width, height)`:
    /// the range's rectangle and the active cell's, which its wash leaves
    /// clear. As `jade_xlsx_render` draws it: the wash over the range, a
    /// 2 px border centred on its edge and the fill handle, 6 px square, on
    /// its bottom-right corner.
    pub fn selection_shape(&self, selection: Selection) -> Option<(SheetRect, SheetRect)> {
        let range = self.selection_range(selection);
        let last = self.grid_end();
        let first_column = self.layout.column(range.start.column)?;
        let first_row = self.layout.row(range.start.row)?;
        let last_column = self.layout.column(range.end.column.min(last.column))?;
        let last_row = self.layout.row(range.end.row.min(last.row))?;
        let rect = (
            first_column.offset,
            first_row.offset,
            last_column.offset + last_column.size - first_column.offset,
            last_row.offset + last_row.size - first_row.offset,
        );
        let active = match self.layout.cell(selection.active) {
            Some(placed) => (
                placed.rect.x,
                placed.rect.y,
                placed.rect.width,
                placed.rect.height,
            ),
            None => {
                let column = self.layout.column(selection.active.column)?;
                let row = self.layout.row(selection.active.row)?;
                (column.offset, row.offset, column.size, row.size)
            }
        };
        Some((rect, active))
    }

    /// The dashed outline Excel keeps round copied cells until they are
    /// pasted or let go.
    fn draw_marquee(
        &self,
        painter: &B::Painter,
        range: CellRange,
        viewport: Rect,
    ) -> Result<(), <B::Painter as Painter>::Error> {
        let last = self.grid_end();
        let end = CellRef::new(
            range.end.column.min(last.column),
            range.end.row.min(last.row),
        );
        let (Some(first), Some(far)) = (
            self.layout
                .cell_rect_in_viewport(range.start, viewport.x, viewport.y),
            self.layout
                .cell_rect_in_viewport(end, viewport.x, viewport.y),
        ) else {
            return Ok(());
        };
        let metrics = self.layout.header_metrics();
        painter.save();
        painter.begin_path();
        painter.rect(
            metrics.row_width,
            metrics.column_height,
            viewport.width,
            viewport.height,
        );
        painter.clip();
        painter.translate(metrics.row_width, metrics.column_height)?;
        painter.set_stroke_style_str("#217346");
        painter.set_line_width(2.0);
        painter.set_line_dash(&[5.0, 3.0]);
        painter.stroke_rect(
            first.x,
            first.y,
            far.x + far.width - first.x,
            far.y + far.height - first.y,
        );
        painter.set_line_dash(&[]);
        painter.restore();
        Ok(())
    }
}

/// Where `new` paints differently from `old`, or `None` when that could be
/// anywhere. Cells are compared one by one — their rectangles, the text
/// spilling out of them and a border's width round them — and the grid
/// grown past its old extent is new; anything that moves cells (an axis
/// changed), restyles blank ones or draws over them (shapes) is anywhere.
fn layout_changes(old: &SheetLayout, new: &SheetLayout) -> Option<Vec<Rect>> {
    let prefix = |a: &[jade_xlsx_layout::AxisEntry], b: &[jade_xlsx_layout::AxisEntry]| {
        let shared = a.len().min(b.len());
        a[..shared] == b[..shared]
    };
    if !prefix(&old.columns, &new.columns)
        || !prefix(&old.rows, &new.rows)
        || old.show_grid_lines != new.show_grid_lines
        || old.frozen_columns != new.frozen_columns
        || old.frozen_rows != new.frozen_rows
        || old.cell_padding_px != new.cell_padding_px
        || old.max_digit_width_px != new.max_digit_width_px
        || old.blank_conditional_rules != new.blank_conditional_rules
        || old.blank_row_styles != new.blank_row_styles
        || old.blank_column_styles != new.blank_column_styles
        || old.shapes != new.shapes
    {
        return None;
    }
    // Far enough to be everywhere past an edge.
    const BEYOND: f64 = 1e9;
    // A thick border straddles a cell's edge.
    const MARGIN: f64 = 3.0;
    let grown = |rect: Rect| Rect {
        x: rect.x - MARGIN,
        y: rect.y - MARGIN,
        width: rect.width + 2.0 * MARGIN,
        height: rect.height + 2.0 * MARGIN,
    };
    let mut changes = Vec::new();
    if old.content_width != new.content_width {
        changes.push(Rect {
            x: old.content_width.min(new.content_width) - MARGIN,
            y: 0.0,
            width: BEYOND,
            height: BEYOND,
        });
    }
    if old.content_height != new.content_height {
        changes.push(Rect {
            x: 0.0,
            y: old.content_height.min(new.content_height) - MARGIN,
            width: BEYOND,
            height: BEYOND,
        });
    }
    fn by_reference(layout: &SheetLayout) -> BTreeMap<CellRef, &jade_xlsx_layout::PlacedCell> {
        layout
            .cells
            .iter()
            .map(|cell| (cell.reference, cell))
            .collect()
    }
    let (before, after) = (by_reference(old), by_reference(new));
    let mut mark = |cell: &jade_xlsx_layout::PlacedCell| {
        changes.push(grown(cell.rect));
        if let Some(clip) = cell.text_clip {
            changes.push(grown(clip));
        }
    };
    for (reference, cell) in &before {
        if after.get(reference) != Some(cell) {
            mark(cell);
        }
    }
    for (reference, cell) in &after {
        if before.get(reference) != Some(cell) {
            mark(cell);
        }
    }
    if old.notes != new.notes {
        changes.extend(
            old.notes
                .iter()
                .chain(&new.notes)
                .map(|note| grown(note.rect)),
        );
    }
    if old.filter_buttons != new.filter_buttons {
        changes.extend(
            old.filter_buttons
                .iter()
                .chain(&new.filter_buttons)
                .map(|button| grown(button.rect)),
        );
    }
    Some(changes)
}

/// The column band (with the corner) or the row band of a viewport, in
/// viewport px: `jade_xlsx_render`'s `render_headers`, one band at a time, so
/// a shell can repaint the bands without the cells between them.
fn paint_header_band<P: Painter>(
    painter: &P,
    layout: &SheetLayout,
    viewport: Rect,
    selected: Option<CellRange>,
    columns: bool,
) -> Result<(), P::Error> {
    use jade_xlsx_layout::units::points_to_px;

    let style = jade_xlsx_render::HeaderStyle::default();
    let metrics = layout.header_metrics();
    let surface_width = metrics.row_width + viewport.width;
    let surface_height = metrics.column_height + viewport.height;
    painter.set_fill_style_str(&style.background);
    if columns {
        painter.fill_rect(0.0, 0.0, surface_width, metrics.column_height);
    } else {
        painter.fill_rect(0.0, 0.0, metrics.row_width, surface_height);
    }
    painter.set_font(&format!("{:.2}pt 'Calibri'", style.font_size_points));
    painter.set_text_baseline("alphabetic");
    let em = points_to_px(style.font_size_points);
    let regions = layout.viewport_regions(viewport);
    let in_columns = |index: u32| {
        selected.is_some_and(|range| (range.start.column..=range.end.column).contains(&index))
    };
    let in_rows = |index: u32| {
        selected.is_some_and(|range| (range.start.row..=range.end.row).contains(&index))
    };

    let column = |entry: &jade_xlsx_layout::AxisEntry, x: f64| -> Result<(), P::Error> {
        if in_columns(entry.index) {
            painter.set_fill_style_str(&style.highlight);
            painter.fill_rect(x, 0.0, entry.size, metrics.column_height);
        }
        let label = jade_xlsx_layout::column_label(entry.index);
        painter.set_fill_style_str(&style.text_color);
        let width = painter.measure_text_width(&label)?;
        painter.fill_text(
            &label,
            x + (entry.size - width) / 2.0,
            (metrics.column_height + em * 0.7) / 2.0,
        )
    };
    let row = |entry: &jade_xlsx_layout::AxisEntry, y: f64| -> Result<(), P::Error> {
        if in_rows(entry.index) {
            painter.set_fill_style_str(&style.highlight);
            painter.fill_rect(0.0, y, metrics.row_width, entry.size);
        }
        let label = (entry.index + 1).to_string();
        painter.set_fill_style_str(&style.text_color);
        let width = painter.measure_text_width(&label)?;
        painter.fill_text(
            &label,
            (metrics.row_width - width - 4.0).max(2.0),
            y + (entry.size + em * 0.7) / 2.0,
        )
    };

    // The scrolling pane's, then the frozen ones over them.
    if let Some(range) = layout.visible_range(regions.main) {
        if columns {
            for index in range.first_column..=range.last_column {
                if let Some(entry) = layout.column(index).filter(|entry| entry.size > 0.0) {
                    column(
                        entry,
                        metrics.row_width + regions.frozen_width + entry.offset - regions.main.x,
                    )?;
                }
            }
        } else {
            for index in range.first_row..=range.last_row {
                if let Some(entry) = layout.row(index).filter(|entry| entry.size > 0.0) {
                    row(
                        entry,
                        metrics.column_height + regions.frozen_height + entry.offset
                            - regions.main.y,
                    )?;
                }
            }
        }
    }
    if columns {
        for entry in layout
            .columns
            .iter()
            .take(layout.frozen_columns as usize)
            .take_while(|entry| entry.offset < regions.frozen_width)
            .filter(|entry| entry.size > 0.0)
        {
            column(entry, metrics.row_width + entry.offset)?;
        }
    } else {
        for entry in layout
            .rows
            .iter()
            .take(layout.frozen_rows as usize)
            .take_while(|entry| entry.offset < regions.frozen_height)
            .filter(|entry| entry.size > 0.0)
        {
            row(entry, metrics.column_height + entry.offset)?;
        }
    }

    // The line that closes the band.
    painter.set_stroke_style_str(&style.line_color);
    painter.set_line_width(1.0);
    painter.set_line_dash(&[]);
    painter.begin_path();
    if columns {
        let bottom = metrics.column_height.floor() + 0.5;
        painter.move_to(0.0, bottom);
        painter.line_to(surface_width, bottom);
        // The corner's own edge, which the row band continues below it.
        let right = metrics.row_width.floor() + 0.5;
        painter.move_to(right, 0.0);
        painter.line_to(right, metrics.column_height);
    } else {
        let right = metrics.row_width.floor() + 0.5;
        painter.move_to(right, 0.0);
        painter.line_to(right, surface_height);
    }
    painter.stroke();
    Ok(())
}

/// A rectangle of the sheet in sheet px, as `(x, y, width, height)`.
pub type SheetRect = (f64, f64, f64, f64);

/// A pane of a viewport: where it sits in the viewport, in sheet px, and the
/// part of the sheet it shows. See [`Workbook::panes`].
pub type Pane = ((f64, f64), SheetRect);

/// What a sheet is painted with over its cells.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Overlay {
    pub selection: Option<Selection>,
    /// Drawn dashed: where a fill-handle drag would fill, or the range a
    /// formula being typed points at.
    pub preview: Option<CellRange>,
    /// Copied cells, outlined until pasted or let go.
    pub marquee: Option<CellRange>,
    /// A column or row edge being dragged: the guide line where it is.
    pub resize: Option<jade_xlsx_layout::resize::ResizeGesture>,
}

fn lay_out<B: Backend>(
    book: &HeadlessWorkbook,
    index: usize,
    extent: (u32, u32),
    backend: &B,
) -> Result<SheetLayout, String> {
    let workbook = book.workbook();
    let measuring = backend
        .surface(1, 1)
        .ok_or_else(|| "cannot create a measuring surface".to_string())?;
    layout_workbook_sheet(
        workbook,
        index,
        &LayoutOptions {
            min_column_count: extent.0,
            min_row_count: extent.1,
            date_system: workbook.date_system,
            measure: Rc::new(PainterMeasure::new(measuring)),
            ..LayoutOptions::default()
        }
        .with_normal_font_metrics(&workbook.styles),
    )
    .ok_or_else(|| "the sheet does not exist".to_string())
}

/// `B3:B3` for a single cell too, as the engine's range arguments take.
fn range_text_full(range: CellRange) -> String {
    format!(
        "{}:{}",
        reference_text(range.start),
        reference_text(range.end)
    )
}

/// `B3:D7` (or `B3`) read back.
fn parse_range(text: &str) -> Option<CellRange> {
    let (start, end) = text.split_once(':').unwrap_or((text, text));
    Some(span(CellRef::parse(start).ok()?, CellRef::parse(end).ok()?))
}

/// The target a paste tiles over: `target` when it is larger than what was
/// pasted and a whole multiple of it in both directions.
fn tiled_over(pasted: CellRange, target: CellRange) -> Option<CellRange> {
    let width = pasted.end.column - pasted.start.column + 1;
    let height = pasted.end.row - pasted.start.row + 1;
    let target_width = target.end.column - target.start.column + 1;
    let target_height = target.end.row - target.start.row + 1;
    let larger = target_width > width || target_height > height;
    (larger && target_width.is_multiple_of(width) && target_height.is_multiple_of(height))
        .then_some(CellRange {
            start: pasted.start,
            end: target.end,
        })
}

/// A number format with one more (or fewer) decimal place, as Excel's
/// Increase and Decrease Decimal make it: each section's first number gets
/// a `0` after its point, or loses one. General starts from what the cell
/// shows.
fn adjust_decimals(code: &str, shown: &str, more: bool) -> String {
    if code.eq_ignore_ascii_case("general") || code.is_empty() {
        let places = shown.rsplit_once('.').map_or(0, |(_, fraction)| {
            fraction.chars().take_while(char::is_ascii_digit).count()
        });
        let places = if more {
            places + 1
        } else {
            places.saturating_sub(1)
        };
        return if places == 0 {
            "0".to_string()
        } else {
            format!("0.{}", "0".repeat(places))
        };
    }
    code.split(';')
        .map(|section| adjust_section(section, more))
        .collect::<Vec<_>>()
        .join(";")
}

fn adjust_section(section: &str, more: bool) -> String {
    // Outside quotes and brackets, the last run of digit placeholders.
    let bytes: Vec<char> = section.chars().collect();
    let mut quoted = false;
    let mut bracket = false;
    let mut last_digit = None;
    let mut point = None;
    for (index, ch) in bytes.iter().enumerate() {
        match ch {
            '"' => quoted = !quoted,
            '[' if !quoted => bracket = true,
            ']' if !quoted => bracket = false,
            _ if quoted || bracket => {}
            '0' | '#' | '?' => last_digit = Some(index),
            '.' if last_digit.is_some() && point.is_none() => point = Some(index),
            '%' | 'E' | 'e' if point.is_none() && last_digit.is_some() => break,
            _ => {}
        }
        if point.is_some() && !matches!(ch, '0' | '#' | '?' | '.') && last_digit.is_some() {
            break;
        }
    }
    let Some(last) = last_digit else {
        return section.to_string();
    };
    let mut out: Vec<char> = bytes.clone();
    match (more, point) {
        (true, Some(_)) => out.insert(last + 1, '0'),
        (true, None) => {
            out.insert(last + 1, '0');
            out.insert(last + 1, '.');
        }
        (false, Some(point)) => {
            if last > point {
                out.remove(last);
                if last == point + 1 {
                    out.remove(point);
                }
            }
        }
        (false, None) => {}
    }
    out.into_iter().collect()
}

/// The Borders button's choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Borders {
    All,
    Outline,
    ThickOutline,
    Bottom,
    None,
}

/// Where a hyperlink goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    Url(String),
    /// A sheet (by index) and the cells to select there.
    Place {
        sheet: usize,
        range: CellRange,
    },
}

/// The font styles a shortcut turns on and off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontToggle {
    Bold,
    Italic,
    Underline,
    Strike,
}

/// The last copy or cut: the engine's snapshot of the cells, the text it
/// put on the clipboard, whether it was a cut, and where it came from.
struct Clip {
    snapshot: ClipboardSnapshot,
    text: String,
    cut: bool,
    range: CellRange,
    sheet: usize,
}

#[cfg(all(test, feature = "tiny-skia"))]
mod tests {
    use super::*;
    use crate::grid::{Direction, Hit, Selection};
    use crate::test_support::{faces, fixture};
    use crate::tiny::TinySkiaBackend;

    fn open(name: &str) -> Workbook<TinySkiaBackend> {
        Workbook::open(
            &fixture(name),
            "Asia/Tokyo",
            TinySkiaBackend::new(&faces()).unwrap(),
        )
        .unwrap()
    }

    /// A hidden sheet has no tab, as in Excel: the workbook opens on the
    /// first visible sheet, and the tabs skip the hidden one.
    #[test]
    fn a_hidden_sheet_has_no_tab() {
        let book = open("hidden-sheets-workbook.xlsx");
        assert_eq!(book.sheet_names(), ["売上サマリ", "Notes"]);
        assert_eq!(book.sheet_tabs(), [1]);
        assert_eq!(book.sheet_index(), 1);
    }

    #[test]
    fn typed_text_becomes_the_right_kind_of_input() {
        assert!(matches!(cell_input("=SUM(A1:A3)"), CellInput::Formula(f) if f == "SUM(A1:A3)"));
        assert!(matches!(cell_input("42"), CellInput::Number(n) if n == 42.0));
        assert!(matches!(cell_input("="), CellInput::Text(_)));
        assert_eq!(reference_text(CellRef::new(27, 9)), "AB10");
        assert_eq!(
            range_text(span(CellRef::new(3, 4), CellRef::new(1, 1))),
            "B2:D5"
        );
        assert_eq!(format_number(1.0 / 3.0), "0.3333");
    }

    #[test]
    fn a_typed_formula_recalculates_undoes_and_saves() {
        let mut book = open("grid-operations-workbook.xlsx");
        assert!(book.set_cell(CellRef::new(4, 1), "=B2*10").unwrap());
        assert_eq!(book.display_text(CellRef::new(4, 1)), "100");
        assert!(book.is_dirty());
        let (x, y, w, h) = book.cell_rect(CellRef::new(4, 1), (0.0, 0.0)).unwrap();
        assert_eq!(
            book.hit(x + w / 2.0, y + h / 2.0, (0.0, 0.0)),
            Some(Hit::Cell(CellRef::new(4, 1)))
        );
        assert!(book.undo().unwrap());
        assert_eq!(book.edit_text(CellRef::new(4, 1)), "");
        assert!(book.redo().unwrap());
        let saved = book.save_bytes().unwrap();
        assert!(!book.is_dirty());
        let backend = TinySkiaBackend::new(&faces()).unwrap();
        let reopened = Workbook::open(&saved, "UTC", backend).unwrap();
        assert_eq!(reopened.edit_text(CellRef::new(4, 1)), "=B2*10");
    }

    /// The formula bar shows what was typed; the file stores what Excel
    /// reads (`_xlfn.` before the newer functions) — otherwise Excel opens
    /// the saved workbook as `#NAME?`.
    #[test]
    fn newer_functions_read_as_typed_and_save_as_excel_reads_them() {
        let mut book = open("grid-operations-workbook.xlsx");
        let at = CellRef::new(6, 1);
        let typed = "=XLOOKUP(20,B2:B4,C2:C4,\"none\")";
        assert!(book.set_cell(at, typed).unwrap());
        assert_eq!(book.edit_text(at), typed);
        assert_eq!(book.display_text(at), "40");
        let saved = book.save_bytes().unwrap();
        let package = jade_opc::OpcPackage::open(&saved).unwrap();
        let stored = package
            .part_names()
            .iter()
            .filter(|name| name.starts_with("xl/worksheets/"))
            .filter_map(|name| package.part_str(name).ok().flatten())
            .any(|xml| xml.contains("_xlfn.XLOOKUP("));
        assert!(stored, "XLOOKUP is saved without its _xlfn. prefix");
        let backend = TinySkiaBackend::new(&faces()).unwrap();
        let reopened = Workbook::open(&saved, "UTC", backend).unwrap();
        assert_eq!(reopened.edit_text(at), typed);
        // Typing the same text again is no change.
        let mut reopened = reopened;
        assert!(!reopened.set_cell(at, typed).unwrap());
    }

    #[test]
    fn copy_paste_and_the_summary_follow_excel() {
        let mut book = open("grid-operations-workbook.xlsx");
        let range = span(CellRef::new(1, 1), CellRef::new(2, 3));
        let summary = book.summary(range);
        assert_eq!(summary.count, 6);
        assert_eq!(summary.sum, Some(180.0));
        assert_eq!(summary.average, Some(30.0));
        let text = book.range_text(range);
        assert_eq!(text, "10\t20\n20\t40\n30\t60\n");
        let far = book.paste(CellRef::new(5, 10), &text).unwrap();
        assert_eq!(far, Some(CellRef::new(6, 12)));
        assert_eq!(book.edit_text(CellRef::new(6, 12)), "60");
        assert!(book
            .clear(span(CellRef::new(5, 10), CellRef::new(6, 12)))
            .unwrap());
        assert_eq!(book.edit_text(CellRef::new(6, 12)), "");
        let painter = book
            .render_viewport(
                (0.0, 0.0),
                320,
                200,
                1.0,
                &Overlay {
                    selection: Some(Selection::range_from(
                        CellRef::new(1, 1),
                        CellRef::new(2, 3),
                    )),
                    marquee: Some(span(CellRef::new(1, 1), CellRef::new(1, 2))),
                    ..Overlay::default()
                },
            )
            .unwrap();
        assert!(TinySkiaBackend::png(&painter).is_some());
    }

    fn at(column: u32, row: u32) -> CellRef {
        CellRef::new(column, row)
    }

    /// ⌘-arrows stop at the ends of runs of data and at the sheet's edge,
    /// ⌘End at the last used cell, and ⌘A takes the block of data first.
    #[test]
    fn command_arrows_jump_as_excel_does() {
        let book = open("grid-operations-workbook.xlsx");
        // B2:C4 hold numbers under a header row.
        assert_eq!(book.jump(at(1, 1), Direction::Down), at(1, 3));
        assert_eq!(book.jump(at(1, 3), Direction::Down), at(1, MAX_ROW));
        assert_eq!(book.jump(at(1, 3), Direction::Up), at(1, 0));
        assert_eq!(book.jump(at(5, 2), Direction::Left), at(2, 2));
        assert_eq!(book.jump(at(0, 2), Direction::Left), at(0, 2));
        let end = book.used_end();
        assert!(end.row >= 3 && end.column >= 2);
        let region = book.current_region(at(1, 2)).unwrap();
        assert!(region.contains(at(1, 1)) && region.contains(at(2, 3)));
        assert!(book.page_rows(0, 400.0) > 10);
    }

    /// ⌘D and ⌘R copy the first row or column through the range with the
    /// formulas moved, ⌘Enter fills a range with what was typed, and the
    /// fill handle continues a series — each one undo.
    #[test]
    fn fills_move_formulas_and_undo_as_one() {
        let mut book = open("grid-operations-workbook.xlsx");
        assert!(book.set_cell(at(3, 1), "=B2+C2").unwrap());
        assert!(book.fill_down(span(at(3, 1), at(3, 3))).unwrap());
        assert_eq!(book.edit_text(at(3, 3)), "=B4+C4");
        assert_eq!(book.display_text(at(3, 3)), "90");
        assert!(book.undo().unwrap());
        assert_eq!(book.edit_text(at(3, 3)), "");
        assert_eq!(book.edit_text(at(3, 1)), "=B2+C2", "the typed cell stays");

        // A single cell takes the one above.
        assert!(book.fill_down(span(at(3, 2), at(3, 2))).unwrap());
        assert_eq!(book.edit_text(at(3, 2)), "=B3+C3");

        assert!(book.fill_right(span(at(1, 1), at(4, 1))).unwrap());
        assert_eq!(book.display_text(at(4, 1)), "10");

        assert!(book
            .fill_with(span(at(6, 1), at(6, 3)), at(6, 1), "=B2*2")
            .unwrap());
        assert_eq!(book.edit_text(at(6, 3)), "=B4*2");
        assert_eq!(book.display_text(at(6, 3)), "60");
        assert!(book.undo().unwrap());
        assert_eq!(book.edit_text(at(6, 1)), "", "⌘Enter undoes as one");

        // The fill handle: 10, 20, 30 goes on 40, 50.
        let source = span(at(1, 1), at(1, 3));
        let target = Workbook::<TinySkiaBackend>::fill_target(source, at(1, 5));
        assert_eq!(target, span(at(1, 1), at(1, 5)));
        assert!(book.fill_series(source, target).unwrap());
        assert_eq!(book.display_text(at(1, 5)), "50");
    }

    /// A copy pasted in the same workbook moves its formulas and is tiled
    /// over a larger selection; a cut moves the cells.
    #[test]
    fn copies_paste_formulas_and_cuts_move() {
        let mut book = open("grid-operations-workbook.xlsx");
        assert!(book.set_cell(at(3, 1), "=B2*10").unwrap());
        let copied = book.copy(span(at(3, 1), at(3, 1)), false);
        assert_eq!(copied, "100\n");
        assert_eq!(
            book.copied(),
            Some((book.sheet_index(), span(at(3, 1), at(3, 1))))
        );
        let filled = book.paste_over(span(at(4, 1), at(4, 3)), &copied).unwrap();
        assert_eq!(filled, Some(span(at(4, 1), at(4, 3))));
        assert_eq!(book.edit_text(at(4, 3)), "=C4*10");
        assert!(book.undo().unwrap());
        assert_eq!(book.edit_text(at(4, 1)), "", "a tiled paste undoes as one");

        // Text tiles too: one value over three cells.
        let filled = book.paste_over(span(at(5, 1), at(5, 3)), "7").unwrap();
        assert_eq!(filled, Some(span(at(5, 1), at(5, 3))));
        assert_eq!(book.display_text(at(5, 3)), "7");

        let cut = book.copy(span(at(3, 1), at(3, 1)), true);
        book.paste_over(span(at(3, 6), at(3, 6)), &cut).unwrap();
        assert_eq!(
            book.edit_text(at(3, 6)),
            "=B2*10",
            "a moved formula keeps pointing"
        );
        assert_eq!(book.copied(), None, "a cut moves once");
        assert_eq!(book.edit_text(at(3, 1)), "");
    }

    #[test]
    fn styles_rows_columns_and_sheets_change_as_in_excel() {
        let mut book = open("grid-operations-workbook.xlsx");
        let range = span(at(1, 1), at(2, 2));
        assert!(book.toggle(range, at(1, 1), FontToggle::Bold).unwrap());
        assert_eq!(book.format_at(at(2, 2)).bold, Some(true));
        assert!(book.toggle(range, at(1, 1), FontToggle::Bold).unwrap());
        assert_ne!(book.format_at(at(2, 2)).bold, Some(true));

        assert!(book.set_cell(at(3, 4), "=SUM(B2:B4)").unwrap());
        assert!(book.insert_rows(2, 1).unwrap());
        assert_eq!(book.edit_text(at(3, 5)), "=SUM(B2:B5)");
        assert!(book.delete_rows(2, 1).unwrap());
        assert_eq!(book.edit_text(at(3, 4)), "=SUM(B2:B4)");
        assert!(book.insert_columns(0, 1).unwrap());
        assert_eq!(book.edit_text(at(4, 4)), "=SUM(C2:C4)");
        assert!(book.undo().unwrap());

        assert!(book.resize(Axis::Columns, 1, 1, 150.0).unwrap());
        let width = book.layout.column(1).unwrap().size;
        assert!((width - 150.0).abs() < 8.0, "{width}");

        let sheets = book.sheet_names().len();
        let index = book.add_sheet().unwrap();
        assert_eq!(book.sheet_names().len(), sheets + 1);
        assert_eq!(book.sheet_index(), index);
        assert!(book.rename_sheet(index, "Totals").unwrap());
        assert_eq!(book.sheet_name(), "Totals");
        assert!(book.delete_sheet(index).unwrap());
        assert_eq!(book.sheet_names().len(), sheets);
    }

    /// Arrows step over merged cells and past hidden rows; Merge & Center
    /// toggles; a sort keeps the header row on top.
    #[test]
    fn arrows_merges_and_sorts_behave_as_in_excel() {
        let mut book = open("grid-operations-workbook.xlsx");
        assert!(book.merge_and_center(span(at(4, 1), at(5, 2))).unwrap());
        assert_eq!(
            book.step(at(4, 0), Direction::Down),
            at(4, 1),
            "onto the merge"
        );
        assert_eq!(
            book.step(at(4, 1), Direction::Down),
            at(4, 3),
            "over the rest of it"
        );
        assert_eq!(book.step(at(3, 2), Direction::Right), at(4, 1));
        assert_eq!(book.step(at(4, 1), Direction::Right), at(6, 1));
        assert_eq!(
            book.with_merges(span(at(3, 2), at(4, 2))),
            span(at(3, 1), at(5, 2))
        );
        assert!(book.merge_and_center(span(at(4, 1), at(5, 2))).unwrap());
        assert_eq!(
            book.step(at(4, 1), Direction::Down),
            at(4, 2),
            "split again"
        );

        // A row hidden by a zero height is stepped past.
        assert!(book.resize(Axis::Rows, 2, 2, 0.0).unwrap());
        assert_eq!(book.step(at(1, 1), Direction::Down), at(1, 3));
        assert!(book.undo().unwrap());

        // B2:C4 under a header row, sorted largest first by column B.
        let header = book.edit_text(at(1, 0));
        assert!(book.sort(span(at(1, 2), at(1, 2)), at(1, 2), true).unwrap());
        assert_eq!(book.edit_text(at(1, 0)), header, "the header stays on top");
        assert_eq!(book.display_text(at(1, 1)), "30");
        assert_eq!(book.display_text(at(2, 1)), "60", "rows move whole");
        assert!(book.undo().unwrap());
        assert_eq!(book.display_text(at(1, 1)), "10");
    }

    #[test]
    fn a_fill_handle_double_click_fills_as_far_as_the_data_beside() {
        let mut book = open("grid-operations-workbook.xlsx");
        // D2 beside B2:C4: down to row 4.
        assert!(book.set_cell(at(3, 1), "=B2+C2").unwrap());
        let source = span(at(3, 1), at(3, 1));
        let target = book.fill_handle_extent(source).unwrap();
        assert_eq!(target, span(at(3, 1), at(3, 3)));
        assert!(book.fill_series(source, target).unwrap());
        assert_eq!(book.edit_text(at(3, 3)), "=B4+C4");
        assert_eq!(book.fill_handle_extent(span(at(9, 1), at(9, 1))), None);
    }

    #[test]
    fn autosum_sums_the_numbers_above_or_beside() {
        let book = open("grid-operations-workbook.xlsx");
        assert_eq!(book.autosum_range(at(1, 4)), Some(span(at(1, 1), at(1, 3))));
        assert_eq!(book.autosum_range(at(3, 2)), Some(span(at(1, 2), at(2, 2))));
        assert_eq!(book.autosum_range(at(8, 8)), None);
    }

    #[test]
    fn find_goes_round_the_matches_by_rows() {
        let mut book = open("grid-operations-workbook.xlsx");
        assert!(book.set_cell(at(5, 5), "needle one").unwrap());
        assert!(book.set_cell(at(2, 7), "Needle two").unwrap());
        let sheet = book.sheet_index();
        let options = crate::find::FindOptions::default();
        let first = book
            .find_next("needle", options, (sheet, at(0, 0)), false)
            .unwrap();
        assert_eq!(first, (sheet, at(5, 5)));
        let second = book.find_next("needle", options, first, false).unwrap();
        assert_eq!(second, (sheet, at(2, 7)));
        assert_eq!(
            book.find_next("needle", options, second, false),
            Some(first),
            "round again"
        );
        assert_eq!(book.find_next("needle", options, first, true), Some(second));
        assert_eq!(book.find_next("nowhere", options, first, false), None);
    }

    /// A filter button's menu: its values, a filter by some of them, a
    /// clear, a sort under the header.
    #[test]
    fn filter_buttons_filter_and_sort_their_columns() {
        let mut book = open("grid-operations-workbook.xlsx");
        let metrics = book.header_metrics();
        let button = book
            .layout
            .filter_buttons
            .first()
            .cloned()
            .expect("a filter button");
        let (x, y) = (
            button.rect.x + metrics.row_width + button.rect.width / 2.0,
            button.rect.y + metrics.column_height + button.rect.height / 2.0,
        );
        assert_eq!(
            book.filter_button_at(x, y, (0.0, 0.0)),
            Some(button.clone())
        );
        let options = book.filter_options(&button).unwrap();
        let labels: Vec<_> = options
            .options
            .iter()
            .map(|option| option.label.as_str())
            .collect();
        assert!(labels.contains(&"Alpha"), "{labels:?}");
        assert!(book
            .filter_values(&button, vec!["Beta".into()], false)
            .unwrap());
        assert!(
            book.layout.row(1).unwrap().hidden,
            "Alpha's row is filtered out"
        );
        assert!(!book.layout.row(2).unwrap().hidden);
        assert!(book.clear_filter(&button).unwrap());
        assert!(!book.layout.row(1).unwrap().hidden);
        assert!(book.sort_filter(&button, true).unwrap());
        assert_eq!(book.display_text(at(0, 1)), "Gamma");
        assert!(book.toggle_filter(span(at(0, 0), at(0, 0))).unwrap());
        assert!(book.layout.filter_buttons.is_empty(), "filters off");
    }

    #[test]
    fn decimals_step_as_excel_steps_them() {
        assert_eq!(adjust_decimals("General", "10", true), "0.0");
        assert_eq!(adjust_decimals("General", "3.14", false), "0.0");
        assert_eq!(adjust_decimals("#,##0", "1,000", true), "#,##0.0");
        assert_eq!(adjust_decimals("#,##0.00", "1,000.00", false), "#,##0.0");
        assert_eq!(adjust_decimals("0.0", "1.0", false), "0");
        assert_eq!(adjust_decimals("0%", "10%", true), "0.0%");
        assert_eq!(
            adjust_decimals("\"¥\"#,##0;\"¥\"\\-#,##0", "¥1", true),
            "\"¥\"#,##0.0;\"¥\"\\-#,##0.0"
        );
    }

    #[test]
    fn the_home_tab_formats_cells() {
        let mut book = open("grid-operations-workbook.xlsx");
        let range = span(at(1, 1), at(2, 3));
        assert!(book.decimals(range, at(1, 1), true).unwrap());
        assert_eq!(book.display_text(at(2, 3)), "60.0");
        assert!(book
            .align(range, at(1, 1), HorizontalAlignment::Center)
            .unwrap());
        assert_eq!(
            book.format_at(at(2, 2)).horizontal,
            Some(HorizontalAlignment::Center)
        );
        assert!(book
            .align(range, at(1, 1), HorizontalAlignment::Center)
            .unwrap());
        assert_eq!(
            book.format_at(at(2, 2)).horizontal,
            Some(HorizontalAlignment::General)
        );
        assert!(book.grow_font(range, at(1, 1), true).unwrap());
        assert_eq!(book.format_at(at(1, 1)).font_size, Some(12.0));
        assert!(book.fill_color(range, Some("FFFF00")).unwrap());
        assert_eq!(
            book.format_at(at(1, 1)).fill_color.as_deref(),
            Some("FFFF00")
        );
        assert!(book.font_color(range, "FF0000").unwrap());
        assert_eq!(
            book.format_at(at(1, 1)).font_color.as_deref(),
            Some("FF0000")
        );
        assert!(book.toggle_wrap(range, at(1, 1)).unwrap());
        assert_eq!(book.format_at(at(1, 1)).wrap_text, Some(true));

        assert!(book.borders(range, Borders::Outline).unwrap());
        let corner = book.format_at(at(1, 1));
        assert_eq!(corner.border_top.unwrap().style, BorderStyle::Thin);
        assert_eq!(corner.border_right.unwrap().style, BorderStyle::None);
        assert!(book.undo().unwrap());
        assert_eq!(
            book.format_at(at(1, 1)).border_top.unwrap().style,
            BorderStyle::None,
            "an outline undoes as one"
        );
        assert!(book.borders(range, Borders::All).unwrap());
        assert_eq!(
            book.format_at(at(2, 2)).border_left.unwrap().style,
            BorderStyle::Thin
        );
        assert!(book.borders(range, Borders::None).unwrap());
        assert_eq!(
            book.format_at(at(2, 2)).border_left.unwrap().style,
            BorderStyle::None
        );
    }

    #[test]
    fn notes_hidden_rows_and_moved_sheets() {
        let mut book = open("grid-operations-workbook.xlsx");
        assert!(book.set_note(at(1, 1), "check this", "jade").unwrap());
        assert_eq!(book.note(at(1, 1)), Some("check this"));
        assert!(book.set_note(at(1, 1), "", "jade").unwrap());
        assert_eq!(book.note(at(1, 1)), None);

        assert!(book.hide(Axis::Rows, 1, 2).unwrap());
        assert_eq!(book.step(at(0, 0), Direction::Down), at(0, 3));
        assert!(book.unhide(Axis::Rows, 0, 5).unwrap());
        assert_eq!(book.step(at(0, 0), Direction::Down), at(0, 1));
        assert!(book.hide(Axis::Columns, 1, 1).unwrap());
        assert!(
            book.layout.column(1).unwrap().size <= 0.0 || book.layout.column(1).unwrap().hidden
        );
        assert!(book.unhide(Axis::Columns, 0, 2).unwrap());
        assert!(book.layout.column(1).unwrap().size > 10.0);

        let first = book.sheet_names()[0].clone();
        book.add_sheet().unwrap();
        assert!(book.move_sheet(1, 0).unwrap());
        assert_eq!(book.sheet_index(), 0);
        assert_eq!(book.sheet_names()[1], first);
    }

    /// A link to the web opens it; one into the workbook goes there.
    #[test]
    fn hyperlinks_lead_to_the_web_or_a_place_in_the_workbook() {
        let mut package =
            jade_opc::OpcPackage::open(&fixture("grid-operations-workbook.xlsx")).unwrap();
        let sheet = package
            .part_str("xl/worksheets/sheet1.xml")
            .unwrap()
            .unwrap()
            .replace(
                "<tableParts",
                "<hyperlinks><hyperlink ref=\"A2\" r:id=\"link\"/>\
             <hyperlink ref=\"A3\" location=\"'Operations'!$C$4\"/></hyperlinks><tableParts",
            );
        package.replace("xl/worksheets/sheet1.xml", sheet.into_bytes());
        let rels = package
            .part_str("xl/worksheets/_rels/sheet1.xml.rels")
            .unwrap()
            .unwrap()
            .replace(
                "</Relationships>",
                "<Relationship Id=\"link\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink\" Target=\"https://example.com/\" TargetMode=\"External\"/></Relationships>",
            );
        package.replace("xl/worksheets/_rels/sheet1.xml.rels", rels.into_bytes());
        let bytes = package.write().unwrap();
        let book = Workbook::open(&bytes, "UTC", TinySkiaBackend::new(&faces()).unwrap()).unwrap();
        assert_eq!(
            book.link(at(0, 1)),
            Some(Link::Url("https://example.com/".into()))
        );
        assert_eq!(
            book.link(at(0, 2)),
            Some(Link::Place {
                sheet: 0,
                range: span(at(2, 3), at(2, 3))
            })
        );
        assert_eq!(book.link(at(0, 3)), None);
    }

    /// A list rule offers its choices and refuses anything else; a number
    /// rule refuses what is out of bounds, with the rule's message.
    #[test]
    fn data_validation_lists_and_refuses_as_excel_does() {
        let mut package =
            jade_opc::OpcPackage::open(&fixture("grid-operations-workbook.xlsx")).unwrap();
        let sheet = package
            .part_str("xl/worksheets/sheet1.xml")
            .unwrap()
            .unwrap()
            .replace(
                "<tableParts",
                "<dataValidations count=\"2\">\
                 <dataValidation type=\"list\" allowBlank=\"1\" showErrorMessage=\"1\" sqref=\"D2:D4\"><formula1>\"Yes,No\"</formula1></dataValidation>\
                 <dataValidation type=\"whole\" operator=\"between\" showErrorMessage=\"1\" error=\"1 to 10\" sqref=\"E2\"><formula1>1</formula1><formula2>10</formula2></dataValidation>\
                 </dataValidations><tableParts",
            );
        package.replace("xl/worksheets/sheet1.xml", sheet.into_bytes());
        let bytes = package.write().unwrap();
        let mut book =
            Workbook::open(&bytes, "UTC", TinySkiaBackend::new(&faces()).unwrap()).unwrap();
        assert_eq!(
            book.validation_list(at(3, 2)),
            Some(vec!["Yes".to_string(), "No".to_string()])
        );
        assert!(book.set_cell(at(3, 2), "yes").unwrap());
        let refused = book.set_cell(at(3, 2), "Maybe").unwrap_err();
        assert_eq!(validation_refusal(&refused), Some(""));
        assert!(book.set_cell(at(4, 1), "7").unwrap());
        let refused = book.set_cell(at(4, 1), "70").unwrap_err();
        assert_eq!(validation_refusal(&refused), Some("1 to 10"));
        assert!(
            book.set_cell(at(4, 1), "=B2").unwrap(),
            "formulas are not checked"
        );
        assert_eq!(book.validation_list(at(0, 0)), None);
    }

    #[test]
    fn a_dragged_selection_moves_or_copies() {
        let mut book = open("grid-operations-workbook.xlsx");
        let moved = book
            .move_range(span(at(2, 1), at(2, 3)), at(5, 1), false)
            .unwrap();
        assert_eq!(moved, span(at(5, 1), at(5, 3)));
        assert_eq!(
            book.edit_text(at(5, 3)),
            "=B4*2",
            "a moved formula keeps pointing"
        );
        assert_eq!(book.edit_text(at(2, 3)), "");
        assert!(book.undo().unwrap());
        assert_eq!(book.edit_text(at(2, 3)), "=B4*2");
        book.move_range(span(at(2, 1), at(2, 1)), at(6, 1), true)
            .unwrap();
        assert_eq!(
            book.edit_text(at(6, 1)),
            "=F2*2",
            "a copy moves its references"
        );
        assert_eq!(book.edit_text(at(2, 1)), "=B2*2");
        let (x, y, w, h) = book.cell_rect(at(1, 1), (0.0, 0.0)).unwrap();
        let range = span(at(1, 1), at(1, 2));
        assert!(book.on_selection_border(x, y + h / 2.0, (0.0, 0.0), range));
        assert!(!book.on_selection_border(x + w / 2.0, y + h / 2.0, (0.0, 0.0), range));
    }
}

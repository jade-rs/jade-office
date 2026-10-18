//! An editable workbook: jade-core's `HeadlessWorkbook` with the selected
//! sheet laid out and painted through a [`Backend`].
//!
//! The editing core the browser spreadsheet and the headless CLI also use: a
//! write patches the workbook's bytes, recalculates, and keeps undo history.
//! A shell hit-tests through [`Workbook::hit`], types through
//! [`Workbook::set_cell`], and paints with [`Workbook::render_viewport`].

use std::cell::RefCell;
use std::rc::Rc;

use jade_painter::Painter;
use jade_xlsx_headless_api::CellInput;
use jade_xlsx_headless_editor::{formula_text, HeadlessWorkbook};

use crate::{clipboard, typing};
pub use jade_xlsx_layout::HeaderMetrics;
use jade_xlsx_layout::{column_label, layout_workbook_sheet, LayoutOptions, Rect, SheetLayout};
pub use jade_xlsx_reader::{CellRange, CellRef};
use jade_xlsx_reader::{CellValue, SheetVisibility};
use jade_xlsx_render::{render_grid, GridSelection, PainterMeasure, RenderOptions};

use crate::Backend;

/// The grid drawn past the last used cell; it grows as the selection moves.
const MIN_COLUMNS: u32 = 12;
const MIN_ROWS: u32 = 30;
/// Excel's limits.
pub const MAX_COLUMN: u32 = 16_383;
pub const MAX_ROW: u32 = 1_048_575;
/// The most cells a copy puts on the clipboard.
const CLIPBOARD_CELL_LIMIT: u64 = 200_000;

/// `B3`.
pub fn reference_text(reference: CellRef) -> String {
    format!("{}{}", column_label(reference.column), reference.row + 1)
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
fn full_precision(value: f64) -> String {
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
    /// The last surface painted on, reused while the size stays the same:
    /// a sheet repaints on every scroll frame.
    surface: RefCell<Option<(B::Painter, u32, u32)>>,
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
            surface: RefCell::new(None),
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
        self.index = index;
        self.extent = (MIN_COLUMNS, MIN_ROWS);
        self.relayout()
    }

    fn relayout(&mut self) -> Result<(), String> {
        self.layout = lay_out(&self.book, self.index, self.extent, &self.backend)?;
        Ok(())
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
    /// headers included) at `scroll`: a cell, or a whole row or column when
    /// the point is on a header, as (anchor, far corner).
    pub fn hit(&self, x: f64, y: f64, scroll: (f64, f64)) -> Option<(CellRef, CellRef)> {
        let layout = &self.layout;
        let metrics = layout.header_metrics();
        let (sheet_x, sheet_y) = layout.sheet_point_in_viewport(
            (x - metrics.row_width).max(0.0),
            (y - metrics.column_height).max(0.0),
            scroll.0,
            scroll.1,
        );
        let end = self.grid_end();
        match (x < metrics.row_width, y < metrics.column_height) {
            (true, true) => Some((CellRef::new(0, 0), end)),
            (true, false) => {
                let row = layout.cell_at(0.0, sheet_y)?.row;
                Some((CellRef::new(0, row), CellRef::new(end.column, row)))
            }
            (false, true) => {
                let column = layout.cell_at(sheet_x, 0.0)?.column;
                Some((CellRef::new(column, 0), CellRef::new(column, end.row)))
            }
            (false, false) => {
                let cell = layout.cell_at(sheet_x, sheet_y)?;
                Some((cell, cell))
            }
        }
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
        let range = CellRange {
            start: reference,
            end: reference,
        };
        let Some(cell) = self.cells_in(range).next() else {
            return String::new();
        };
        if let Some(formula) = &cell.formula {
            // As Excel shows it: the file's `_xlfn.` prefixes are not what
            // anybody typed.
            return format!("={}", formula_text::to_display_formula(formula));
        }
        match &cell.value {
            CellValue::Empty => String::new(),
            CellValue::Number(number) => self.edit_number(reference, *number),
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
    fn edit_number(&self, reference: CellRef, number: f64) -> String {
        let format = self.number_format(reference).unwrap_or_default();
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
        self.book
            .cell(&self.sheet_name(), &reference_text(reference))
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
        if text == self.edit_text(reference) {
            return Ok(false);
        }
        let name = self.sheet_name();
        let at = reference_text(reference);
        let date1904 = matches!(
            self.book.workbook().date_system,
            jade_xlsx_reader::DateSystem::Nineteen04
        );
        let typed = typing::read(text, date1904);
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
            let change = jade_xlsx_headless_editor::FormatChange {
                number_format: Some(code.to_string()),
                ..Default::default()
            };
            if self.book.set_format(&name, &at, &change).is_ok() {
                steps += 1;
            }
        }
        self.record(steps);
        self.after_write()?;
        Ok(true)
    }

    /// Notes an edit that took `steps` of the book's undo steps; a new
    /// edit ends what redo could bring back.
    fn record(&mut self, steps: usize) {
        self.steps.push(steps);
        self.undone.clear();
    }

    /// Pastes tab-separated text from `origin` down and right as one write,
    /// returning the far corner it filled.
    pub fn paste(&mut self, origin: CellRef, text: &str) -> Result<Option<CellRef>, String> {
        let mut values = Vec::new();
        let mut far = origin;
        for (row_offset, row) in clipboard::parse(text).iter().enumerate() {
            for (column_offset, value) in row.iter().enumerate() {
                let reference = CellRef::new(
                    origin.column + column_offset as u32,
                    origin.row + row_offset as u32,
                );
                far.column = far.column.max(reference.column);
                far.row = far.row.max(reference.row);
                values.push((reference_text(reference), typing::read(value, false).input));
            }
        }
        if values.is_empty() {
            return Ok(None);
        }
        let name = self.sheet_name();
        self.book
            .set_cells(&name, &values)
            .map_err(|error| error.to_string())?;
        self.record(1);
        self.after_write()?;
        Ok(Some(far))
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

    fn after_write(&mut self) -> Result<(), String> {
        self.dirty = true;
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

    /// The workbook as `.xlsx` bytes, as saving writes it; marks it saved.
    pub fn save_bytes(&mut self) -> Result<Vec<u8>, String> {
        let bytes = self.book.save_bytes().map_err(|error| error.to_string())?;
        self.dirty = false;
        Ok(bytes)
    }

    /// Paints the part of the sheet under `scroll` (sheet px) onto a
    /// `width` × `height` surface at `scale` device px per sheet px, with the
    /// selection from `anchor` (the active cell) to its far corner.
    pub fn render_viewport(
        &self,
        scroll: (f64, f64),
        width: u32,
        height: u32,
        scale: f64,
        selection: Option<(CellRef, CellRef)>,
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
        let selection = selection.map(|(anchor, focus)| GridSelection {
            range: span(anchor, focus),
            active: anchor,
            fill_preview: None,
            fill_handle: false,
        });
        painter.save();
        let painted = painter.scale(scale, scale).is_ok()
            && render_grid(
                painter,
                &self.layout,
                viewport,
                selection,
                &RenderOptions::default(),
            )
            .is_ok();
        painter.restore();
        painted.then(|| painter.clone())
    }
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

#[cfg(all(test, feature = "tiny-skia"))]
mod tests {
    use super::*;
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
            Some((CellRef::new(4, 1), CellRef::new(4, 1)))
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
                Some((CellRef::new(1, 1), CellRef::new(2, 3))),
            )
            .unwrap();
        assert!(TinySkiaBackend::png(&painter).is_some());
    }
}

//! An editable workbook: jade-core's `HeadlessWorkbook` with the selected
//! sheet laid out and painted on the pure-Rust painter.
//!
//! The same editing core the native app and the browser spreadsheet use: a
//! write patches the workbook's bytes, recalculates, and keeps undo history.
//! Everything here is UI-free; a shell hit-tests through [`Workbook::hit`],
//! types through [`Workbook::set_cell`], and paints with
//! [`Workbook::render_viewport`].

use std::rc::Rc;

use jade_painter::Painter;
use jade_tiny_skia_painter::{FontBook, TinySkiaPainter};
use jade_xlsx_headless_api::CellInput;
use jade_xlsx_headless_editor::HeadlessWorkbook;
pub use jade_xlsx_layout::HeaderMetrics;
use jade_xlsx_layout::{column_label, layout_workbook_sheet, LayoutOptions, Rect, SheetLayout};
pub use jade_xlsx_reader::{CellRange, CellRef};
use jade_xlsx_reader::{CellValue, SheetVisibility};
use jade_xlsx_render::{render_grid, GridSelection, RenderOptions};

/// The grid drawn past the last used cell; it grows as the selection moves.
const MIN_COLUMNS: u32 = 12;
const MIN_ROWS: u32 = 30;
/// Excel's limits.
pub const MAX_COLUMN: u32 = 16_383;
pub const MAX_ROW: u32 = 1_048_575;

/// `B3`.
pub fn reference_text(reference: CellRef) -> String {
    format!("{}{}", column_label(reference.column), reference.row + 1)
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

fn format_number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        let text = format!("{value:.4}");
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

pub struct Workbook {
    book: HeadlessWorkbook,
    fonts: Rc<FontBook>,
    index: usize,
    layout: SheetLayout,
    extent: (u32, u32),
    dirty: bool,
}

impl Workbook {
    /// Opens `.xlsx` bytes for editing, recalculating in `time_zone` (an IANA
    /// name such as `Asia/Tokyo`; `UTC` when it is not one the engine knows).
    pub fn open(bytes: &[u8], time_zone: &str, fonts: Rc<FontBook>) -> Result<Self, String> {
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
        let layout = lay_out(&book, index, (MIN_COLUMNS, MIN_ROWS), &fonts)?;
        Ok(Self {
            book,
            fonts,
            index,
            layout,
            extent: (MIN_COLUMNS, MIN_ROWS),
            dirty: false,
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

    pub fn select_sheet(&mut self, index: usize) -> Result<(), String> {
        if index >= self.book.workbook().sheets.len() {
            return Err(format!("no sheet at index {index}"));
        }
        self.index = index;
        self.extent = (MIN_COLUMNS, MIN_ROWS);
        self.relayout()
    }

    fn relayout(&mut self) -> Result<(), String> {
        self.layout = lay_out(&self.book, self.index, self.extent, &self.fonts)?;
        Ok(())
    }

    fn name(&self) -> String {
        self.book.workbook().sheets[self.index].name.clone()
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
        let last_row = (layout.rows.len() as u32).saturating_sub(1);
        let last_column = (layout.columns.len() as u32).saturating_sub(1);
        match (x < metrics.row_width, y < metrics.column_height) {
            (true, true) => Some((CellRef::new(0, 0), CellRef::new(last_column, last_row))),
            (true, false) => {
                let row = layout.cell_at(0.0, sheet_y)?.row;
                Some((CellRef::new(0, row), CellRef::new(last_column, row)))
            }
            (false, true) => {
                let column = layout.cell_at(sheet_x, 0.0)?.column;
                Some((CellRef::new(column, 0), CellRef::new(column, last_row)))
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

    /// The formula with its `=`, or the stored value: what the formula bar
    /// shows and what an edit starts from.
    pub fn edit_text(&self, reference: CellRef) -> String {
        let sheet = &self.book.workbook().sheets[self.index];
        let Some(cell) = sheet
            .rows
            .iter()
            .find(|row| row.index == reference.row)
            .and_then(|row| row.cells.iter().find(|cell| cell.reference == reference))
        else {
            return String::new();
        };
        if let Some(formula) = &cell.formula {
            return format!("={formula}");
        }
        match &cell.value {
            CellValue::Empty => String::new(),
            CellValue::Number(number) => format_number(*number),
            CellValue::Text(text) => text.clone(),
            CellValue::Bool(flag) => if *flag { "TRUE" } else { "FALSE" }.to_string(),
            CellValue::Error(error) => error.clone(),
        }
    }

    /// What a cell shows, through its number format.
    pub fn display_text(&self, reference: CellRef) -> String {
        self.book
            .cell(&self.name(), &reference_text(reference))
            .map(|cell| cell.display)
            .unwrap_or_default()
    }

    /// Writes typed text into a cell, recalculating. Returns whether the
    /// cell changed.
    pub fn set_cell(&mut self, reference: CellRef, text: &str) -> Result<bool, String> {
        if text == self.edit_text(reference) {
            return Ok(false);
        }
        let name = self.name();
        self.book
            .set(&name, &reference_text(reference), cell_input(text))
            .map_err(|error| error.to_string())?;
        self.after_write()?;
        Ok(true)
    }

    /// Empties the cells in `range` that hold anything, keeping formats.
    pub fn clear(&mut self, range: CellRange) -> Result<bool, String> {
        let sheet = &self.book.workbook().sheets[self.index];
        let values: Vec<(String, CellInput)> = sheet
            .rows
            .iter()
            .filter(|row| row.index >= range.start.row && row.index <= range.end.row)
            .flat_map(|row| row.cells.iter())
            .filter(|cell| {
                cell.reference.column >= range.start.column
                    && cell.reference.column <= range.end.column
                    && (cell.formula.is_some() || !matches!(cell.value, CellValue::Empty))
            })
            .map(|cell| (reference_text(cell.reference), CellInput::Blank))
            .collect();
        if values.is_empty() {
            return Ok(false);
        }
        let name = self.name();
        self.book
            .set_cells(&name, &values)
            .map_err(|error| error.to_string())?;
        self.after_write()?;
        Ok(true)
    }

    fn after_write(&mut self) -> Result<(), String> {
        self.dirty = true;
        self.relayout()
    }

    pub fn undo(&mut self) -> Result<bool, String> {
        let undone = self.book.undo().map_err(|error| error.to_string())?;
        if undone {
            self.after_write()?;
        }
        Ok(undone)
    }

    pub fn redo(&mut self) -> Result<bool, String> {
        let redone = self.book.redo().map_err(|error| error.to_string())?;
        if redone {
            self.after_write()?;
        }
        Ok(redone)
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// The workbook as `.xlsx` bytes, as saving writes it.
    pub fn save_bytes(&mut self) -> Result<Vec<u8>, String> {
        let bytes = self.book.save_bytes().map_err(|error| error.to_string())?;
        self.dirty = false;
        Ok(bytes)
    }

    /// Paints the part of the sheet under `scroll` (sheet px) onto a
    /// `width` × `height` surface at `scale` device px per sheet px, with the
    /// selection from `anchor` (the active cell) to `focus`.
    pub fn render_viewport(
        &self,
        scroll: (f64, f64),
        width: u32,
        height: u32,
        scale: f64,
        selection: Option<(CellRef, CellRef)>,
    ) -> Option<TinySkiaPainter> {
        let painter = TinySkiaPainter::new(width.max(1), height.max(1), Rc::clone(&self.fonts))?;
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
        painter.scale(scale, scale).ok()?;
        render_grid(
            &painter,
            &self.layout,
            viewport,
            selection,
            &RenderOptions::default(),
        )
        .ok()?;
        painter.restore();
        Some(painter)
    }
}

fn lay_out(
    book: &HeadlessWorkbook,
    index: usize,
    extent: (u32, u32),
    fonts: &Rc<FontBook>,
) -> Result<SheetLayout, String> {
    let workbook = book.workbook();
    let measuring = TinySkiaPainter::new(1, 1, Rc::clone(fonts))
        .ok_or_else(|| "cannot create a measuring surface".to_string())?;
    layout_workbook_sheet(
        workbook,
        index,
        &LayoutOptions {
            min_column_count: extent.0,
            min_row_count: extent.1,
            date_system: workbook.date_system,
            measure: Rc::new(jade_xlsx_render::PainterMeasure::new(measuring)),
            ..LayoutOptions::default()
        }
        .with_normal_font_metrics(&workbook.styles),
    )
    .ok_or_else(|| "the sheet does not exist".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_text_becomes_the_right_kind_of_input() {
        assert!(matches!(cell_input("=SUM(A1:A3)"), CellInput::Formula(f) if f == "SUM(A1:A3)"));
        assert!(matches!(cell_input("42"), CellInput::Number(n) if n == 42.0));
        assert!(matches!(cell_input("="), CellInput::Text(_)));
        assert_eq!(reference_text(CellRef::new(27, 9)), "AB10");
    }

    #[test]
    fn a_typed_formula_recalculates_undoes_and_saves() {
        let core =
            std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
        let fonts = crate::tests::fonts();
        let bytes =
            std::fs::read(core.join("public/fixtures/grid-operations-workbook.xlsx")).unwrap();
        let mut book = Workbook::open(&bytes, "Asia/Tokyo", fonts).unwrap();
        assert!(book.set_cell(CellRef::new(4, 1), "=B2*10").unwrap());
        assert_eq!(book.display_text(CellRef::new(4, 1)), "100");
        assert!(book.is_dirty());
        // A click in the body finds the cell under it.
        let (x, y, w, h) = book.cell_rect(CellRef::new(4, 1), (0.0, 0.0)).unwrap();
        assert_eq!(
            book.hit(x + w / 2.0, y + h / 2.0, (0.0, 0.0)),
            Some((CellRef::new(4, 1), CellRef::new(4, 1)))
        );
        assert!(book.undo().unwrap());
        assert_eq!(book.edit_text(CellRef::new(4, 1)), "");
        assert!(book.redo().unwrap());
        let saved = book.save_bytes().unwrap();
        let reopened = Workbook::open(&saved, "UTC", crate::tests::fonts()).unwrap();
        assert_eq!(reopened.edit_text(CellRef::new(4, 1)), "=B2*10");
        assert!(book
            .render_viewport(
                (0.0, 0.0),
                320,
                200,
                1.0,
                Some((CellRef::new(4, 1), CellRef::new(4, 1)))
            )
            .is_some());
    }
}

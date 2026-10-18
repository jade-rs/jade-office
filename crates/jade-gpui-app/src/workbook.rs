//! The spreadsheet (XLSX) view: a sheet you can select in, type into, and
//! save.
//!
//! The model is jade-core's `HeadlessWorkbook` — the same editing core the
//! browser's spreadsheet and the headless CLI use: a write patches the
//! workbook's bytes, recalculates, and keeps an undo history. This view lays
//! the selected sheet out with `jade-xlsx-layout` and paints it with
//! `jade-xlsx-render` through Skia, and puts the editing affordances Excel
//! has round it: a name box and formula bar, a cell editor over the active
//! cell, the selection, and the sheet tabs.
//!
//! ## Why this paints a viewport rather than pages
//!
//! A sheet cannot be rasterised up front the way a document's pages are: a
//! real workbook is hundreds of thousands of px tall. So the surface stays
//! the size of the space the view has, and each repaint draws the part of the
//! sheet under the scroll position — at the display's pixel density and the
//! zoom — with the selection drawn by the same renderer.

use gpui::{
    canvas, div, img, point, prelude::*, px, size, App, Bounds, ClipboardItem, Context,
    ElementInputHandler, Entity, EntityInputHandler, FocusHandle, Focusable, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, RenderImage, ScrollHandle, SharedString,
    UTF16Selection, Window,
};
use jade_painter::Painter;
use jade_pptx_native_render::{FaceSpec, FontSet};
use jade_skia_painter::SkiaPainter;
use jade_xlsx_headless_api::CellInput;
use jade_xlsx_headless_editor::HeadlessWorkbook;
use jade_xlsx_layout::{column_label, layout_workbook_sheet, LayoutOptions, Rect, SheetLayout};
use jade_xlsx_native_render::{aliased_faces, bundled_font_paths};
use jade_xlsx_reader::{CellRange, CellRef, CellValue};
use jade_xlsx_render::{render_grid, GridSelection, PainterMeasure, RenderOptions};
use std::cell::Cell;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use crate::editor::{self, Editor};
use crate::{
    byte_to_utf16, i18n, theme, utf16_to_byte, workspace, Copy, Cut, Paste, Redo, Save, SaveAs,
    SelectAll, Undo, UI_FONT_FAMILY,
};

/// The grid drawn past the last used cell, so a small sheet still reads as a
/// grid; it grows as the selection moves past it.
const MIN_COLUMNS: u32 = 12;
const MIN_ROWS: u32 = 30;
/// Excel's limits.
const MAX_COLUMN: u32 = 16_383;
const MAX_ROW: u32 = 1_048_575;
/// The name box and formula bar above the grid.
const FORMULA_BAR_HEIGHT: f32 = 32.0;
/// The strip of sheet tabs under the grid.
const SHEET_TABS_HEIGHT: f32 = 30.0;
/// The most cells a copy writes to the clipboard.
const CLIPBOARD_CELL_LIMIT: u64 = 200_000;

/// The zone formulas like `NOW()` calculate in: the system's, as the browser
/// editor uses the browser's.
fn system_time_zone() -> String {
    std::fs::read_link("/etc/localtime")
        .ok()
        .and_then(|target| {
            let target = target.to_string_lossy().into_owned();
            target
                .split_once("zoneinfo/")
                .map(|(_, zone)| zone.to_string())
        })
        .unwrap_or_else(|| "UTC".to_string())
}

/// `B3`.
fn reference_text(reference: CellRef) -> String {
    format!("{}{}", column_label(reference.column), reference.row + 1)
}

/// The rectangle two corners span, top-left first.
fn span(a: CellRef, b: CellRef) -> CellRange {
    CellRange {
        start: CellRef::new(a.column.min(b.column), a.row.min(b.row)),
        end: CellRef::new(a.column.max(b.column), a.row.max(b.row)),
    }
}

/// What typed text becomes: `=` starts a formula, otherwise it is read the
/// way a spreadsheet reads what you type.
fn cell_input(text: &str) -> CellInput {
    match text.strip_prefix('=') {
        Some(formula) if !formula.is_empty() => CellInput::Formula(formula.to_string()),
        _ => CellInput::parse(text),
    }
}

/// A number as a status bar shows it: integers bare, others to a few places.
fn format_number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        let text = format!("{value:.4}");
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// The workbook being edited, the faces every surface gets, and the selected
/// sheet's layout.
struct Sheets {
    book: HeadlessWorkbook,
    fonts: FontSet,
    index: usize,
    layout: Rc<SheetLayout>,
    /// How far the laid-out grid reaches; it grows with the selection.
    extent: (u32, u32),
}

impl Sheets {
    fn open(bytes: &[u8]) -> Result<Self, String> {
        let book = HeadlessWorkbook::open_for_editing_in_time_zone(bytes, &system_time_zone())
            .or_else(|_| HeadlessWorkbook::open_for_editing(bytes))
            .map_err(|error| format!("cannot read the workbook: {error}"))?;
        if book.workbook().sheets.is_empty() {
            return Err("the workbook has no sheets".to_string());
        }
        // The fonts ship with the engine, in the `jade-core` submodule.
        let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
        let (regular, bold) = bundled_font_paths(&core);
        let faces: Vec<FaceSpec<'_>> = aliased_faces(&regular, &bold)
            .into_iter()
            .map(|face| FaceSpec {
                family: face.family,
                weight: face.weight,
                path: face.path,
            })
            .collect();
        let fonts = FontSet::load(&faces)?;
        // The sheet the file says was open, as long as it is one a reader can
        // see; otherwise the first visible one.
        let workbook = book.workbook();
        let visible = |index: &usize| {
            workbook
                .sheets
                .get(*index)
                .is_some_and(|sheet| sheet.visibility == jade_xlsx_reader::SheetVisibility::Visible)
        };
        let index = workbook
            .active_sheet_index
            .filter(visible)
            .or_else(|| (0..workbook.sheets.len()).find(visible))
            .unwrap_or(0);
        let mut sheets = Self {
            book,
            fonts,
            index,
            layout: Rc::new(empty_layout()),
            extent: (MIN_COLUMNS, MIN_ROWS),
        };
        sheets.lay_out()?;
        Ok(sheets)
    }

    fn painter(&self, width: i32, height: i32) -> Option<SkiaPainter> {
        self.fonts.painter(width, height)
    }

    /// Lays the selected sheet out, measuring through a surface with the
    /// same faces registered — wrapping has to happen against the font that
    /// will draw.
    fn lay_out(&mut self) -> Result<(), String> {
        let measuring = self
            .painter(1, 1)
            .ok_or_else(|| "cannot create a measuring surface".to_string())?;
        let workbook = self.book.workbook();
        let layout = layout_workbook_sheet(
            workbook,
            self.index,
            &LayoutOptions {
                min_column_count: self.extent.0,
                min_row_count: self.extent.1,
                date_system: workbook.date_system,
                measure: Rc::new(PainterMeasure::new(measuring)),
                ..LayoutOptions::default()
            }
            .with_normal_font_metrics(&workbook.styles),
        )
        .ok_or_else(|| "the sheet does not exist".to_string())?;
        self.layout = Rc::new(layout);
        Ok(())
    }

    /// Grows the laid-out grid so `reference` and a margin past it exist.
    fn reach(&mut self, reference: CellRef) {
        let wanted = (
            self.extent.0.max(reference.column + 8),
            self.extent.1.max(reference.row + 20),
        );
        if wanted != self.extent {
            self.extent = wanted;
            self.lay_out().ok();
        }
    }

    fn names(&self) -> Vec<SharedString> {
        self.book
            .workbook()
            .sheets
            .iter()
            .map(|sheet| SharedString::from(sheet.name.clone()))
            .collect()
    }

    fn name(&self) -> String {
        self.book.workbook().sheets[self.index].name.clone()
    }

    fn sheet(&self) -> &jade_xlsx_reader::Sheet {
        &self.book.workbook().sheets[self.index]
    }

    /// The stored cells inside `range`.
    fn cells_in(&self, range: CellRange) -> impl Iterator<Item = &jade_xlsx_reader::Cell> {
        self.sheet()
            .rows
            .iter()
            .filter(move |row| row.index >= range.start.row && row.index <= range.end.row)
            .flat_map(|row| row.cells.iter())
            .filter(move |cell| {
                cell.reference.column >= range.start.column
                    && cell.reference.column <= range.end.column
            })
    }

    /// The last row and column holding a cell, so a whole-column copy does
    /// not walk a million rows.
    fn used_end(&self) -> CellRef {
        let mut end = CellRef::new(0, 0);
        for row in &self.sheet().rows {
            for cell in &row.cells {
                end.column = end.column.max(cell.reference.column);
                end.row = end.row.max(cell.reference.row);
            }
        }
        end
    }

    /// What a cell shows in the formula bar, and what an edit of it starts
    /// from: the formula with its `=`, or the stored value.
    fn edit_text(&self, reference: CellRef) -> String {
        let Some(cell) = self
            .sheet()
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
}

fn empty_layout() -> SheetLayout {
    SheetLayout {
        columns: Vec::new(),
        rows: Vec::new(),
        cells: Vec::new(),
        filter_buttons: Vec::new(),
        blank_conditional_rules: Vec::new(),
        blank_row_styles: Default::default(),
        blank_column_styles: Vec::new(),
        shapes: Vec::new(),
        notes: Vec::new(),
        show_grid_lines: true,
        content_width: 0.0,
        max_digit_width_px: jade_xlsx_layout::units::DEFAULT_MAX_DIGIT_WIDTH_PX,
        content_height: 0.0,
        frozen_columns: 0,
        frozen_rows: 0,
        cell_padding_px: 0.0,
    }
}

/// What a painted image was painted for. A frame that matches it needs no new
/// pixels, which is what keeps scrolling from re-rendering the same view.
#[derive(Clone, Copy, PartialEq)]
struct PaintedFor {
    sheet: usize,
    scroll: (f32, f32),
    /// The image's size in device pixels.
    size: (i32, i32),
    /// Device pixels per sheet px: the window's scale factor times the zoom.
    scale: f32,
    selection: (CellRef, CellRef),
    /// Bumped by every write, so an edit repaints even with nothing else
    /// changed.
    generation: u64,
}

/// A cell being typed into.
struct CellEdit {
    text: String,
    /// The IME's composition, as a byte range of `text`.
    marked: Option<Range<usize>>,
    /// Started with F2, a double-click or the formula bar, where the arrow
    /// keys belong to the text rather than to the grid.
    in_place: bool,
}

pub struct WorkbookView {
    path: PathBuf,
    sheets: Sheets,
    sheet_names: Vec<SharedString>,
    /// Where the selection started; also the active cell.
    anchor: CellRef,
    /// The corner the selection was extended to.
    focus_cell: CellRef,
    edit: Option<CellEdit>,
    dragging: bool,
    dirty: bool,
    generation: u64,
    image: Option<Arc<RenderImage>>,
    /// Textures replaced since the last frame. gpui wants them released with a
    /// window to hand, which only `render` has.
    retired: Vec<Arc<RenderImage>>,
    painted: Option<PaintedFor>,
    /// Reused between frames while the device size stays the same.
    surface: Option<(SkiaPainter, i32, i32)>,
    zoom: f32,
    /// The grid's size in window px, from the last frame.
    viewport: (f32, f32),
    /// Where the grid sits in the window, recorded at paint: clicks arrive in
    /// window coordinates.
    grid_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    scroll: ScrollHandle,
    focus: FocusHandle,
    status: SharedString,
}

impl WorkbookView {
    fn active(&self) -> CellRef {
        self.anchor
    }

    fn range(&self) -> CellRange {
        span(self.anchor, self.focus_cell)
    }

    fn selection_label(&self) -> String {
        let range = self.range();
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

    fn select_sheet(&mut self, index: usize, cx: &mut Context<Self>) {
        if index == self.sheets.index || index >= self.sheet_names.len() {
            return;
        }
        self.commit_edit(cx);
        self.sheets.index = index;
        self.sheets.extent = (MIN_COLUMNS, MIN_ROWS);
        if let Err(error) = self.sheets.lay_out() {
            self.status = SharedString::from(error);
        }
        self.anchor = CellRef::new(0, 0);
        self.focus_cell = self.anchor;
        self.scroll.set_offset(point(px(0.0), px(0.0)));
        // The old image belongs to the old sheet.
        self.painted = None;
        cx.notify();
    }

    // ---- selection -------------------------------------------------------

    /// The scroll position in sheet px.
    fn sheet_scroll(&self) -> (f64, f64) {
        let offset = self.scroll.offset();
        (
            (-f64::from(offset.x) / f64::from(self.zoom)).max(0.0),
            (-f64::from(offset.y) / f64::from(self.zoom)).max(0.0),
        )
    }

    /// What is under a window point: a cell, or a whole row or column when
    /// the point is on a header.
    fn hit(&self, position: gpui::Point<Pixels>) -> Option<(CellRef, CellRef)> {
        let bounds = self.grid_bounds.get()?;
        let zoom = f64::from(self.zoom);
        let local_x = f64::from(position.x - bounds.origin.x) / zoom;
        let local_y = f64::from(position.y - bounds.origin.y) / zoom;
        let layout = &self.sheets.layout;
        let metrics = layout.header_metrics();
        let (scroll_x, scroll_y) = self.sheet_scroll();
        let (sheet_x, sheet_y) = layout.sheet_point_in_viewport(
            (local_x - metrics.row_width).max(0.0),
            (local_y - metrics.column_height).max(0.0),
            scroll_x,
            scroll_y,
        );
        let last_row = (layout.rows.len() as u32).saturating_sub(1);
        let last_column = (layout.columns.len() as u32).saturating_sub(1);
        match (local_x < metrics.row_width, local_y < metrics.column_height) {
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

    /// The cell's rectangle in the window, for the cell editor and the IME.
    fn cell_bounds(&self, reference: CellRef) -> Option<Bounds<Pixels>> {
        let grid = self.grid_bounds.get()?;
        let layout = &self.sheets.layout;
        let metrics = layout.header_metrics();
        let (scroll_x, scroll_y) = self.sheet_scroll();
        let rect = layout.cell_rect_in_viewport(reference, scroll_x, scroll_y)?;
        let zoom = self.zoom;
        Some(Bounds {
            origin: point(
                grid.origin.x + px((rect.x + metrics.row_width) as f32 * zoom),
                grid.origin.y + px((rect.y + metrics.column_height) as f32 * zoom),
            ),
            size: size(px(rect.width as f32 * zoom), px(rect.height as f32 * zoom)),
        })
    }

    /// Scrolls just enough to show `reference`.
    fn reveal(&mut self, reference: CellRef) {
        let layout = &self.sheets.layout;
        let metrics = layout.header_metrics();
        let zoom = f64::from(self.zoom);
        let (scroll_x, scroll_y) = self.sheet_scroll();
        let viewport = Rect {
            x: scroll_x,
            y: scroll_y,
            width: (f64::from(self.viewport.0) / zoom - metrics.row_width).max(1.0),
            height: (f64::from(self.viewport.1) / zoom - metrics.column_height).max(1.0),
        };
        let (x, y) = layout.scroll_to_reveal(reference, viewport);
        if (x, y) != (scroll_x, scroll_y) {
            self.scroll
                .set_offset(point(px(-(x * zoom) as f32), px(-(y * zoom) as f32)));
        }
    }

    fn select(&mut self, anchor: CellRef, focus: CellRef, cx: &mut Context<Self>) {
        self.anchor = anchor;
        self.focus_cell = focus;
        self.sheets.reach(focus);
        self.reveal(focus);
        cx.notify();
    }

    /// Moves the active cell (or, extending, the selection's far corner).
    fn step(&mut self, columns: i64, rows: i64, extend: bool, cx: &mut Context<Self>) {
        let from = if extend { self.focus_cell } else { self.anchor };
        let to = CellRef::new(
            (i64::from(from.column) + columns).clamp(0, i64::from(MAX_COLUMN)) as u32,
            (i64::from(from.row) + rows).clamp(0, i64::from(MAX_ROW)) as u32,
        );
        if extend {
            self.select(self.anchor, to, cx);
        } else {
            self.select(to, to, cx);
        }
    }

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus);
        self.commit_edit(cx);
        let Some((start, end)) = self.hit(event.position) else {
            return;
        };
        if event.modifiers.shift {
            self.anchor = self.anchor.min(start);
            self.focus_cell = end;
        } else {
            self.anchor = start;
            self.focus_cell = end;
        }
        self.dragging = true;
        if event.click_count >= 2 && start == end {
            self.begin_edit(None, true, cx);
        }
        cx.notify();
    }

    fn mouse_move(&mut self, event: &MouseMoveEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if !self.dragging || !event.dragging() {
            return;
        }
        if let Some((_, end)) = self.hit(event.position) {
            if end != self.focus_cell {
                self.focus_cell = end;
                cx.notify();
            }
        }
    }

    // ---- editing ---------------------------------------------------------

    fn begin_edit(&mut self, text: Option<String>, in_place: bool, cx: &mut Context<Self>) {
        let text = text.unwrap_or_else(|| self.sheets.edit_text(self.active()));
        self.edit = Some(CellEdit {
            text,
            marked: None,
            in_place,
        });
        self.reveal(self.active());
        cx.notify();
    }

    fn cancel_edit(&mut self, cx: &mut Context<Self>) {
        if self.edit.take().is_some() {
            cx.notify();
        }
    }

    /// Writes the cell being edited, if any. The write recalculates, so
    /// dependent formulas have moved before the next paint.
    fn commit_edit(&mut self, cx: &mut Context<Self>) {
        let Some(edit) = self.edit.take() else {
            return;
        };
        if edit.text == self.sheets.edit_text(self.active()) {
            cx.notify();
            return;
        }
        let sheet = self.sheets.name();
        let reference = reference_text(self.active());
        let result = self
            .sheets
            .book
            .set(&sheet, &reference, cell_input(&edit.text));
        self.after_write(result.map_err(|error| error.to_string()), cx);
    }

    fn after_write(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        match result.and_then(|()| self.sheets.lay_out()) {
            Ok(()) => {
                self.dirty = true;
                self.generation += 1;
                self.status = SharedString::from("");
            }
            Err(error) => {
                self.status =
                    SharedString::from(i18n::format("sheet.edit_failed", &[("error", &error)]))
            }
        }
        cx.notify();
    }

    fn commit_and_step(&mut self, columns: i64, rows: i64, cx: &mut Context<Self>) {
        self.commit_edit(cx);
        self.step(columns, rows, false, cx);
    }

    /// Delete: empties the selected cells that hold anything, keeping their
    /// formats.
    fn clear_selection(&mut self, cx: &mut Context<Self>) {
        let range = self.range();
        let values: Vec<(String, CellInput)> = self
            .sheets
            .cells_in(range)
            .filter(|cell| cell.formula.is_some() || !matches!(cell.value, CellValue::Empty))
            .map(|cell| (reference_text(cell.reference), CellInput::Blank))
            .collect();
        if values.is_empty() {
            return;
        }
        let sheet = self.sheets.name();
        let result = self.sheets.book.set_cells(&sheet, &values);
        self.after_write(result.map_err(|error| error.to_string()), cx);
    }

    fn step_history(&mut self, undo: bool, cx: &mut Context<Self>) {
        self.edit = None;
        let result = if undo {
            self.sheets.book.undo()
        } else {
            self.sheets.book.redo()
        };
        match result {
            Ok(true) => self.after_write(Ok(()), cx),
            Ok(false) => {}
            Err(error) => self.after_write(Err(error.to_string()), cx),
        }
    }

    // ---- clipboard -------------------------------------------------------

    /// The selection's displayed text, tab-separated by column and
    /// newline-separated by row — what Excel puts on the clipboard as text.
    fn selection_text(&self) -> String {
        let range = self.range();
        let used = self.sheets.used_end();
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
        let sheet = self.sheets.name();
        let mut out = String::new();
        for row in range.start.row..=end.row {
            let line: Vec<String> = (range.start.column..=end.column)
                .map(|column| {
                    self.sheets
                        .book
                        .cell(&sheet, &reference_text(CellRef::new(column, row)))
                        .map(|cell| cell.display)
                        .unwrap_or_default()
                })
                .collect();
            out.push_str(&line.join("\t"));
            out.push('\n');
        }
        out
    }

    fn copy(&mut self, cx: &mut Context<Self>) {
        let text = self.selection_text();
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }

    fn cut(&mut self, cx: &mut Context<Self>) {
        self.copy(cx);
        self.clear_selection(cx);
    }

    /// Pastes tab-separated text from the active cell down and right, and
    /// selects what it filled.
    fn paste(&mut self, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        if let Some(edit) = self.edit.as_mut() {
            edit.text.push_str(text.lines().next().unwrap_or(""));
            cx.notify();
            return;
        }
        let origin = self.active();
        let rows: Vec<&str> = text.trim_end_matches(['\n', '\r']).split('\n').collect();
        let mut values = Vec::new();
        let mut far = origin;
        for (row_offset, row) in rows.iter().enumerate() {
            for (column_offset, value) in row.trim_end_matches('\r').split('\t').enumerate() {
                let reference = CellRef::new(
                    origin.column + column_offset as u32,
                    origin.row + row_offset as u32,
                );
                far.column = far.column.max(reference.column);
                far.row = far.row.max(reference.row);
                values.push((reference_text(reference), cell_input(value)));
            }
        }
        if values.is_empty() {
            return;
        }
        let sheet = self.sheets.name();
        let result = self.sheets.book.set_cells(&sheet, &values);
        self.after_write(result.map_err(|error| error.to_string()), cx);
        self.select(origin, far, cx);
    }

    // ---- saving ----------------------------------------------------------

    fn save(&mut self, cx: &mut Context<Self>) {
        // Beside the original rather than over it, as the document view does:
        // this shell is not yet the thing to trust with the only copy.
        let stem = self
            .path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "book".to_string());
        let target = self.path.with_file_name(format!("{stem}.edited.xlsx"));
        self.write_to(target, cx);
    }

    fn save_as(&mut self, cx: &mut Context<Self>) {
        let directory = self
            .path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let suggested = self
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "book.xlsx".to_string());
        let prompt = cx.prompt_for_new_path(&directory, Some(&suggested));
        cx.spawn(async move |view, cx| {
            let Ok(Ok(Some(path))) = prompt.await else {
                return;
            };
            view.update(cx, |view, cx| view.write_to(path, cx)).ok();
        })
        .detach();
    }

    fn write_to(&mut self, target: PathBuf, cx: &mut Context<Self>) {
        self.commit_edit(cx);
        let result = self
            .sheets
            .book
            .save_bytes()
            .map_err(|error| error.to_string())
            .and_then(|bytes| std::fs::write(&target, bytes).map_err(|error| error.to_string()));
        self.status = SharedString::from(match result {
            Ok(()) => {
                self.dirty = false;
                i18n::format(
                    "sheet.saved",
                    &[("name", &crate::shell::file_name(&target))],
                )
            }
            Err(error) => i18n::format("sheet.save_failed", &[("error", &error)]),
        });
        cx.notify();
    }

    // ---- keys ------------------------------------------------------------

    fn handle_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let modifiers = keystroke.modifiers;
        if modifiers.platform || modifiers.control {
            return;
        }
        let shift = modifiers.shift;
        let key = keystroke.key.as_str();

        if let Some(edit) = self.edit.as_mut() {
            // Mid-composition the keys are the IME's.
            if edit.marked.is_some() {
                return;
            }
            let in_place = edit.in_place;
            match key {
                "enter" => self.commit_and_step(0, if shift { -1 } else { 1 }, cx),
                "tab" => self.commit_and_step(if shift { -1 } else { 1 }, 0, cx),
                "escape" => self.cancel_edit(cx),
                "backspace" => {
                    edit.text.pop();
                    cx.notify();
                }
                "up" if !in_place => self.commit_and_step(0, -1, cx),
                "down" if !in_place => self.commit_and_step(0, 1, cx),
                "left" if !in_place => self.commit_and_step(-1, 0, cx),
                "right" if !in_place => self.commit_and_step(1, 0, cx),
                _ => return,
            }
            cx.stop_propagation();
            return;
        }

        match key {
            "up" => self.step(0, -1, shift, cx),
            "down" => self.step(0, 1, shift, cx),
            "left" => self.step(-1, 0, shift, cx),
            "right" => self.step(1, 0, shift, cx),
            "pageup" => self.step(0, -20, shift, cx),
            "pagedown" => self.step(0, 20, shift, cx),
            "enter" => self.step(0, if shift { -1 } else { 1 }, false, cx),
            "tab" => self.step(if shift { -1 } else { 1 }, 0, false, cx),
            "home" => {
                let to = CellRef::new(0, self.anchor.row);
                self.select(to, to, cx);
            }
            "f2" => self.begin_edit(None, true, cx),
            "delete" | "backspace" => self.clear_selection(cx),
            // Anything else is text, which arrives through the input handler
            // and starts an edit there — so an IME composes into the cell.
            _ => return,
        }
        cx.stop_propagation();
    }

    fn select_all(&mut self, cx: &mut Context<Self>) {
        let layout = &self.sheets.layout;
        let end = CellRef::new(
            (layout.columns.len() as u32).saturating_sub(1),
            (layout.rows.len() as u32).saturating_sub(1),
        );
        self.anchor = CellRef::new(0, 0);
        self.focus_cell = end;
        cx.notify();
    }

    // ---- painting --------------------------------------------------------

    /// Repaints when the scroll position, the space, the zoom, the selection
    /// or the sheet has moved since the image was made.
    ///
    /// `viewport` is in window px. The image is painted at device pixels —
    /// the window's scale factor times the zoom per sheet px — so a sheet is
    /// as sharp on a Retina display as the text round it.
    fn repaint_if_needed(&mut self, viewport: (f32, f32), scale_factor: f32) {
        let offset = self.scroll.offset();
        let scale = scale_factor * self.zoom;
        let wanted = PaintedFor {
            sheet: self.sheets.index,
            // The scroll offset is negative going down; the sheet's
            // coordinates are not.
            scroll: (-f32::from(offset.x), -f32::from(offset.y)),
            size: (
                (viewport.0 * scale_factor).round().max(1.0) as i32,
                (viewport.1 * scale_factor).round().max(1.0) as i32,
            ),
            scale,
            selection: (self.anchor, self.focus_cell),
            generation: self.generation,
        };
        if self.painted == Some(wanted) && self.image.is_some() {
            return;
        }
        match self.paint(wanted) {
            Some(image) => {
                if let Some(previous) = self.image.replace(image) {
                    self.retired.push(previous);
                }
                self.painted = Some(wanted);
            }
            None => self.status = SharedString::from(i18n::text("sheet.paint_failed")),
        }
    }

    fn paint(&mut self, wanted: PaintedFor) -> Option<Arc<RenderImage>> {
        let (width, height) = wanted.size;
        let reusable = matches!(self.surface, Some((_, w, h)) if w == width && h == height);
        if !reusable {
            self.surface = Some((self.sheets.painter(width, height)?, width, height));
        }
        let (painter, _, _) = self.surface.as_ref()?;
        let scale = f64::from(wanted.scale);
        // The surface holds the header bands as well as the sheet, so the part
        // of the sheet on show is smaller than it by exactly the bands — in
        // sheet px, which is what the painter draws in under the scale.
        let layout = &self.sheets.layout;
        let metrics = layout.header_metrics();
        let viewport = Rect {
            x: (f64::from(wanted.scroll.0) / f64::from(self.zoom)).max(0.0),
            y: (f64::from(wanted.scroll.1) / f64::from(self.zoom)).max(0.0),
            width: (f64::from(width) / scale - metrics.row_width).max(0.0),
            height: (f64::from(height) / scale - metrics.column_height).max(0.0),
        };
        let selection = GridSelection {
            range: span(self.anchor, self.focus_cell),
            active: self.anchor,
            fill_preview: None,
            fill_handle: false,
        };
        painter.save();
        painter.scale(scale, scale).ok()?;
        let painted = render_grid(
            painter,
            layout,
            viewport,
            Some(selection),
            &RenderOptions::default(),
        );
        painter.restore();
        painted.ok()?;
        let (bgra, width, height) = painter.read_bgra_premultiplied()?;
        let buffer = image::ImageBuffer::from_raw(width as u32, height as u32, bgra)?;
        Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
    }

    fn formula_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let editing = self.edit.as_ref().map(|edit| edit.text.clone());
        let content = editing
            .clone()
            .unwrap_or_else(|| self.sheets.edit_text(self.active()));
        div()
            .h(px(FORMULA_BAR_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .bg(theme::panel())
            .border_b_1()
            .border_color(theme::border())
            .text_xs()
            .child(
                div()
                    .w(px(96.0))
                    .px_2()
                    .py_0p5()
                    .rounded_sm()
                    .border_1()
                    .border_color(theme::border())
                    .bg(theme::window())
                    .text_color(theme::text())
                    .child(self.selection_label()),
            )
            .child(
                div()
                    .px_1()
                    .italic()
                    .text_color(theme::text_faint())
                    .child("fx"),
            )
            .child(
                div()
                    .id("formula-bar")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .items_center()
                    .px_2()
                    .cursor_text()
                    .text_color(theme::text())
                    .truncate()
                    .child(content)
                    .when(editing.is_some(), |bar| {
                        bar.child(div().w(px(1.0)).h(px(14.0)).bg(theme::accent()))
                    })
                    .on_click(cx.listener(|view, _event, window, cx| {
                        window.focus(&view.focus);
                        if view.edit.is_none() {
                            view.begin_edit(None, true, cx);
                        }
                    })),
            )
    }

    /// The editor over the active cell while it is being typed into.
    fn cell_editor(&self, grid: Bounds<Pixels>) -> Option<impl IntoElement> {
        let edit = self.edit.as_ref()?;
        let cell = self.cell_bounds(self.active())?;
        let (before, marked, after) = match &edit.marked {
            Some(range) => (
                edit.text[..range.start].to_string(),
                edit.text[range.clone()].to_string(),
                edit.text[range.end..].to_string(),
            ),
            None => (edit.text.clone(), String::new(), String::new()),
        };
        let zoom = self.zoom;
        Some(
            div()
                .absolute()
                .left(cell.origin.x - grid.origin.x)
                .top(cell.origin.y - grid.origin.y)
                .min_w(cell.size.width)
                .h(cell.size.height)
                .flex()
                .items_center()
                .px(px(3.0 * zoom))
                .bg(gpui::white())
                .border_2()
                .border_color(gpui::rgb(0x217346))
                .text_color(gpui::black())
                .text_size(px(14.0 * zoom))
                .whitespace_nowrap()
                .child(before)
                .when(!marked.is_empty(), |editor| {
                    editor.child(div().underline().child(marked))
                })
                .child(div().w(px(1.0)).h(px(15.0 * zoom)).bg(gpui::black()))
                .child(after),
        )
    }
}

impl Editor for WorkbookView {
    fn status_items(&self) -> Vec<SharedString> {
        let mut items: Vec<SharedString> = vec![
            i18n::format("sheet.position", &[("name", &self.sheets.name())]).into(),
            self.selection_label().into(),
        ];
        // Excel's summary of a multi-cell selection: average, count, sum.
        let range = self.range();
        if range.start != range.end {
            let mut count = 0usize;
            let mut numbers = Vec::new();
            for cell in self.sheets.cells_in(range) {
                match cell.value {
                    CellValue::Empty => {}
                    CellValue::Number(number) => {
                        count += 1;
                        numbers.push(number);
                    }
                    _ => count += 1,
                }
            }
            if !numbers.is_empty() {
                let sum: f64 = numbers.iter().sum();
                items.push(
                    i18n::format(
                        "sheet.average",
                        &[("value", &format_number(sum / numbers.len() as f64))],
                    )
                    .into(),
                );
                items.push(i18n::format("sheet.cell_count", &[("n", &count)]).into());
                items.push(i18n::format("sheet.sum", &[("value", &format_number(sum))]).into());
            } else if count > 0 {
                items.push(i18n::format("sheet.cell_count", &[("n", &count)]).into());
            }
        }
        if !self.status.is_empty() {
            items.push(self.status.clone());
        }
        items
    }

    fn zoom(&self) -> f32 {
        self.zoom
    }

    fn set_zoom(&mut self, zoom: f32, cx: &mut Context<Self>) {
        let zoom = editor::clamp_zoom(zoom);
        // Keep the same cell at the top left: the scroll offset is in screen
        // px, which a zoom rescales.
        let ratio = zoom / self.zoom;
        let offset = self.scroll.offset();
        self.scroll
            .set_offset(point(offset.x * ratio, offset.y * ratio));
        self.zoom = zoom;
        cx.notify();
    }

    fn is_dirty(&self) -> bool {
        self.dirty
    }
}

impl EntityInputHandler for WorkbookView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let text = self
            .edit
            .as_ref()
            .map(|edit| edit.text.as_str())
            .unwrap_or("");
        let start = utf16_to_byte(text, range.start);
        let end = utf16_to_byte(text, range.end).max(start);
        *adjusted = Some(byte_to_utf16(text, start)..byte_to_utf16(text, end));
        Some(text.get(start..end)?.to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        // Typing appends: the caret is the end of the text.
        let text = self
            .edit
            .as_ref()
            .map(|edit| edit.text.as_str())
            .unwrap_or("");
        let caret = byte_to_utf16(text, text.len());
        Some(UTF16Selection {
            range: caret..caret,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        let edit = self.edit.as_ref()?;
        let marked = edit.marked.clone()?;
        Some(byte_to_utf16(&edit.text, marked.start)..byte_to_utf16(&edit.text, marked.end))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        if let Some(edit) = self.edit.as_mut() {
            edit.marked = None;
        }
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Typing into a selected cell replaces what is in it, as in Excel.
        if self.edit.is_none() {
            self.begin_edit(Some(String::new()), false, cx);
        }
        let Some(edit) = self.edit.as_mut() else {
            return;
        };
        let bytes = match range {
            Some(range) => {
                utf16_to_byte(&edit.text, range.start)..utf16_to_byte(&edit.text, range.end)
            }
            None => edit
                .marked
                .clone()
                .unwrap_or(edit.text.len()..edit.text.len()),
        };
        edit.text.replace_range(bytes, text);
        edit.marked = None;
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.edit.is_none() {
            self.begin_edit(Some(String::new()), false, cx);
        }
        let Some(edit) = self.edit.as_mut() else {
            return;
        };
        let bytes = match range {
            Some(range) => {
                utf16_to_byte(&edit.text, range.start)..utf16_to_byte(&edit.text, range.end)
            }
            None => edit
                .marked
                .clone()
                .unwrap_or(edit.text.len()..edit.text.len()),
        };
        let start = bytes.start;
        edit.text.replace_range(bytes, new_text);
        edit.marked = (!new_text.is_empty()).then(|| start..start + new_text.len());
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        // The candidate window goes under the active cell.
        self.cell_bounds(self.active())
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}

impl Focusable for WorkbookView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for WorkbookView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for image in self.retired.drain(..) {
            window.drop_image(image).ok();
        }

        // The part of the workspace this view is given, less the formula bar
        // and the sheet tabs.
        let (area_width, area_height) = workspace::editor_area(window, cx);
        let viewport = (
            area_width.max(1.0),
            (area_height - FORMULA_BAR_HEIGHT - SHEET_TABS_HEIGHT).max(1.0),
        );
        self.viewport = viewport;
        self.repaint_if_needed(viewport, window.scale_factor());

        let layout = &self.sheets.layout;
        let metrics = layout.header_metrics();
        let (sheet_width, sheet_height) = (
            (layout.content_width + metrics.row_width) as f32 * self.zoom,
            (layout.content_height + metrics.column_height) as f32 * self.zoom,
        );
        let current = self.sheets.index;
        let tabs: Vec<_> = self
            .sheet_names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let selected = index == current;
                div()
                    .id(("sheet-tab", index))
                    .relative()
                    .h_full()
                    .flex()
                    .items_center()
                    .px_4()
                    .text_xs()
                    .cursor_pointer()
                    .border_r_1()
                    .border_color(theme::border())
                    .text_color(if selected {
                        theme::accent()
                    } else {
                        theme::text_dim()
                    })
                    .when(selected, |tab| {
                        tab.bg(theme::window()).child(
                            div()
                                .absolute()
                                .bottom_0()
                                .left_1()
                                .right_1()
                                .h(px(2.0))
                                .rounded_t_sm()
                                .bg(theme::accent()),
                        )
                    })
                    .hover(|tab| tab.bg(theme::hovered()))
                    .child(name.clone())
                    .on_click(
                        cx.listener(move |view, _event, _window, cx| view.select_sheet(index, cx)),
                    )
            })
            .collect();

        let grid_store = Rc::clone(&self.grid_bounds);
        let entity = cx.entity();
        let editor = self
            .grid_bounds
            .get()
            .and_then(|grid| self.cell_editor(grid));

        div()
            .key_context("Workbook")
            .font_family(UI_FONT_FAMILY)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::handle_key))
            .on_action(cx.listener(|view, _: &Undo, _window, cx| view.step_history(true, cx)))
            .on_action(cx.listener(|view, _: &Redo, _window, cx| view.step_history(false, cx)))
            .on_action(cx.listener(|view, _: &Copy, _window, cx| view.copy(cx)))
            .on_action(cx.listener(|view, _: &Cut, _window, cx| view.cut(cx)))
            .on_action(cx.listener(|view, _: &Paste, _window, cx| view.paste(cx)))
            .on_action(cx.listener(|view, _: &SelectAll, _window, cx| view.select_all(cx)))
            .on_action(cx.listener(|view, _: &Save, _window, cx| view.save(cx)))
            .on_action(cx.listener(|view, _: &SaveAs, _window, cx| view.save_as(cx)))
            .flex()
            .flex_col()
            .size_full()
            .bg(theme::canvas())
            .text_color(theme::text())
            .child(self.formula_bar(cx))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .overflow_hidden()
                    .child(
                        // The painted window sits under the scroller, which is
                        // transparent and owns the scrollbars and the mouse.
                        div().absolute().inset_0().children(
                            self.image
                                .clone()
                                .map(|image| img(image).w(px(viewport.0)).h(px(viewport.1))),
                        ),
                    )
                    .child(
                        div()
                            .id("sheet-scroll")
                            .absolute()
                            .inset_0()
                            .overflow_scroll()
                            .track_scroll(&self.scroll)
                            .cursor(gpui::CursorStyle::Crosshair)
                            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
                            .on_mouse_move(cx.listener(Self::mouse_move))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _event, _window, _cx| view.dragging = false),
                            )
                            .child(div().w(px(sheet_width)).h(px(sheet_height)).flex_none()),
                    )
                    .children(editor)
                    // Records where the grid is, and registers the input
                    // handler so typing — an IME's included — reaches the
                    // cell.
                    .child(
                        canvas(
                            move |bounds, _window, _cx| {
                                grid_store.set(Some(bounds));
                                bounds
                            },
                            move |_bounds, grid, window, cx| {
                                window.handle_input(
                                    &entity.focus_handle(cx),
                                    ElementInputHandler::new(grid, entity.clone()),
                                    cx,
                                );
                            },
                        )
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full(),
                    ),
            )
            // Excel keeps its sheet tabs along the bottom edge.
            .child(
                div()
                    .h(px(SHEET_TABS_HEIGHT))
                    .flex_none()
                    .flex()
                    .items_center()
                    .bg(theme::panel())
                    .border_t_1()
                    .border_color(theme::border())
                    .children(tabs),
            )
    }
}

impl WorkbookView {
    fn open(path: &Path, cx: &mut App) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let sheets = Sheets::open(&bytes)?;
        let sheet_names = sheets.names();
        Ok(WorkbookView {
            path: path.to_path_buf(),
            sheets,
            sheet_names,
            anchor: CellRef::new(0, 0),
            focus_cell: CellRef::new(0, 0),
            edit: None,
            dragging: false,
            dirty: false,
            generation: 0,
            image: None,
            retired: Vec::new(),
            painted: None,
            surface: None,
            zoom: 1.0,
            viewport: (1.0, 1.0),
            grid_bounds: Rc::new(Cell::new(None)),
            scroll: ScrollHandle::new(),
            focus: cx.focus_handle(),
            status: SharedString::from(""),
        })
    }
}

/// Opens `path` as a spreadsheet view, for the workspace to show in a tab.
pub fn new_view(path: &Path, cx: &mut App) -> Result<Entity<WorkbookView>, String> {
    let view = WorkbookView::open(path, cx)?;
    Ok(cx.new(|_| view))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_text_becomes_the_right_kind_of_input() {
        assert!(matches!(cell_input("=SUM(A1:A3)"), CellInput::Formula(f) if f == "SUM(A1:A3)"));
        assert!(matches!(cell_input("42"), CellInput::Number(n) if n == 42.0));
        assert!(matches!(cell_input(""), CellInput::Blank));
        assert!(matches!(cell_input("名前"), CellInput::Text(t) if t == "名前"));
        // A lone `=` is text, not an empty formula.
        assert!(matches!(cell_input("="), CellInput::Text(_)));
    }

    #[test]
    fn a_span_is_normalised_whichever_corner_came_first() {
        let range = span(CellRef::new(3, 5), CellRef::new(1, 2));
        assert_eq!(range.start, CellRef::new(1, 2));
        assert_eq!(range.end, CellRef::new(3, 5));
    }

    #[test]
    fn numbers_format_for_the_status_bar() {
        assert_eq!(format_number(60.0), "60");
        assert_eq!(format_number(2.5), "2.5");
        assert_eq!(format_number(1.0 / 3.0), "0.3333");
        assert_eq!(reference_text(CellRef::new(27, 9)), "AB10");
    }

    #[test]
    fn an_edit_writes_recalculates_and_undoes() {
        let bytes =
            include_bytes!("../../../jade-core/public/fixtures/grid-operations-workbook.xlsx");
        let mut sheets = Sheets::open(bytes).unwrap();
        let name = sheets.name();
        sheets.book.set(&name, "B2", cell_input("100")).unwrap();
        assert_eq!(sheets.edit_text(CellRef::new(1, 1)), "100");
        sheets.book.set(&name, "E2", cell_input("=B2*2")).unwrap();
        assert_eq!(sheets.edit_text(CellRef::new(4, 1)), "=B2*2");
        assert_eq!(sheets.book.cell(&name, "E2").unwrap().display, "200");
        assert!(sheets.book.undo().unwrap());
        assert!(sheets.book.undo().unwrap());
        assert_ne!(sheets.edit_text(CellRef::new(1, 1)), "100");
        sheets.lay_out().unwrap();
    }

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../jade-core/public/fixtures"
        ))
        .join(name)
    }

    fn open_view(
        cx: &mut gpui::TestAppContext,
    ) -> (Entity<WorkbookView>, &mut gpui::VisualTestContext) {
        let path = fixture("grid-operations-workbook.xlsx");
        let (view, cx) = cx.add_window_view(|_window, cx| WorkbookView::open(&path, cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        (view, cx)
    }

    /// Arrow to a cell, type, press Enter: the value is written, the
    /// selection moves down, and the tab is marked unsaved.
    #[gpui::test]
    fn typing_into_a_cell_writes_it_and_moves_down(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("down down right");
        cx.simulate_input("123");
        view.read_with(cx, |view, _| {
            assert_eq!(
                view.edit.as_ref().map(|edit| edit.text.as_str()),
                Some("123")
            );
        });
        cx.simulate_keystrokes("enter");
        view.read_with(cx, |view, _| {
            assert!(view.edit.is_none());
            assert_eq!(view.sheets.edit_text(CellRef::new(1, 2)), "123");
            assert_eq!(view.active(), CellRef::new(1, 3));
            assert!(view.dirty);
        });
    }

    /// A formula typed into a cell calculates, and Undo takes it back.
    #[gpui::test]
    fn a_typed_formula_calculates_and_undoes(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("right right right right down");
        cx.simulate_input("=B2*10");
        cx.simulate_keystrokes("enter");
        view.read_with(cx, |view, _| {
            let sheet = view.sheets.name();
            assert_eq!(view.sheets.book.cell(&sheet, "E2").unwrap().display, "100");
        });
        cx.dispatch_action(Undo);
        view.read_with(cx, |view, _| {
            assert_eq!(view.sheets.edit_text(CellRef::new(4, 1)), "");
        });
        cx.dispatch_action(Redo);
        view.read_with(cx, |view, _| {
            assert_eq!(view.sheets.edit_text(CellRef::new(4, 1)), "=B2*10");
        });
    }

    /// Escape abandons an edit; Delete clears the selection.
    #[gpui::test]
    fn escape_cancels_and_delete_clears(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("down");
        let before = view.read_with(cx, |view, _| view.sheets.edit_text(CellRef::new(0, 1)));
        assert!(!before.is_empty());
        cx.simulate_input("xyz");
        cx.simulate_keystrokes("escape");
        view.read_with(cx, |view, _| {
            assert!(view.edit.is_none());
            assert_eq!(view.sheets.edit_text(CellRef::new(0, 1)), before);
        });
        cx.simulate_keystrokes("shift-down delete");
        view.read_with(cx, |view, _| {
            assert_eq!(view.sheets.edit_text(CellRef::new(0, 1)), "");
            assert_eq!(view.sheets.edit_text(CellRef::new(0, 2)), "");
        });
    }
}

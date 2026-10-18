//! The spreadsheet (XLSX) view: a sheet worked as Excel is worked, and
//! saved.
//!
//! The model is `jade-office-model`'s `Workbook` — jade-core's
//! `HeadlessWorkbook`, shared with the browser build — the same editing core the
//! browser's spreadsheet and the headless CLI use: a write patches the
//! workbook's bytes, recalculates, and keeps an undo history. This view lays
//! the selected sheet out with `jade-xlsx-layout` and paints it with
//! `jade-xlsx-render` through Skia, and puts the editing affordances Excel
//! has round it: a name box and formula bar, a cell editor over the active
//! cell, the selection, and the sheet tabs.
//!
//! What a key or a drag does to the selection and the workbook is the
//! model's (`grid::Selection`, `sheet::Workbook`), the same code the browser
//! shell runs: click, drag or Shift-click a range, a header for whole rows or
//! columns, the corner for the sheet; ⌘-arrows to the ends of the data;
//! Enter and Tab round a range; type, with arrows that leave the cell — or
//! point at one while a formula is being typed — and F2 to edit in place;
//! the fill handle, ⌘D, ⌘R and ⌘Enter; copies that paste formulas and
//! formats and cuts that move; bold and the number-format shortcuts; rows
//! and columns inserted, deleted and resized; sheets added, renamed and
//! deleted. This is the gpui round it.
//!
//! ## Why this paints a viewport rather than pages
//!
//! A sheet cannot be rasterised up front the way a document's pages are: a
//! real workbook is hundreds of thousands of px tall. So the surface stays
//! the size of the space the view has, and each repaint draws the part of the
//! sheet under the scroll position — at the display's pixel density and the
//! zoom — with the selection drawn by the same renderer.

use gpui::{
    canvas, div, img, point, prelude::*, px, size, App, Bounds, ClickEvent, ClipboardItem, Context,
    DispatchPhase, ElementInputHandler, Entity, EntityInputHandler, FocusHandle, Focusable,
    KeyDownEvent, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    RenderImage, ScrollHandle, SharedString, UTF16Selection, Window,
};
use jade_office_model::grid::{self, Direction, DragKind, Hit, Selection};
use jade_office_model::sheet::{
    column_label, format_number, range_text, reference_text, span, Axis, CellFormat, CellRange,
    CellRef, Copied, FontToggle, Overlay, ResizeGesture, Workbook, MAX_COLUMN, MAX_ROW,
};
use jade_office_model::skia::SkiaBackend;
use jade_office_model::text::TextEdit;
use jade_office_model::Backend;
use std::cell::Cell;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use crate::editor::{self, Editor};
use crate::{
    byte_to_utf16, i18n, theme, utf16_to_byte, workspace, Copy, Cut, Paste, Redo, Save, SaveAs,
    SelectAll, Undo, UI_FONT_FAMILY,
};

/// The name box and formula bar above the grid.
const FORMULA_BAR_HEIGHT: f32 = 32.0;
/// The strip of sheet tabs under the grid.
const SHEET_TABS_HEIGHT: f32 = 30.0;
/// How often a drag held past the grid's edge scrolls it.
const AUTO_SCROLL_TICK: Duration = Duration::from_millis(40);

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

/// Today's date, or the time now, as typed into a cell (`2026/10/19`,
/// `9:41`), for ⌃; and ⌘; — in the system's zone, the one
/// [`system_time_zone`] gives the engine, read through the C library rather
/// than a zone database of our own.
fn now_text(time: bool) -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as libc::time_t)
        .unwrap_or(0);
    // SAFETY: `localtime_r` writes only into the `tm` it is given, which is
    // plain data that all zeroes is a valid value of.
    let local = unsafe {
        let mut local: libc::tm = std::mem::zeroed();
        libc::localtime_r(&seconds, &mut local);
        local
    };
    if time {
        format!("{}:{:02}", local.tm_hour, local.tm_min)
    } else {
        format!(
            "{}/{}/{}",
            local.tm_year + 1900,
            local.tm_mon + 1,
            local.tm_mday
        )
    }
}

/// `B3`, `B3:D7`, or Excel's `1:3` and `A:C` for whole rows and columns.
fn range_label(range: CellRange) -> String {
    let rows = range.start.column == 0 && range.end.column == MAX_COLUMN;
    let columns = range.start.row == 0 && range.end.row == MAX_ROW;
    match (rows, columns) {
        (true, true) => format!("1:{}", MAX_ROW + 1),
        (true, false) => format!("{}:{}", range.start.row + 1, range.end.row + 1),
        (false, true) => format!(
            "{}:{}",
            column_label(range.start.column),
            column_label(range.end.column)
        ),
        (false, false) => range_text(range),
    }
}

/// `B3` or `B3:D7` typed into the name box.
fn parse_reference(text: &str) -> Option<CellRange> {
    let text = text.trim().replace('$', "").to_uppercase();
    let (start, end) = text.split_once(':').unwrap_or((&text, &text));
    let start = CellRef::parse(start).ok()?;
    let end = CellRef::parse(end).ok()?;
    (start.column <= MAX_COLUMN
        && end.column <= MAX_COLUMN
        && start.row <= MAX_ROW
        && end.row <= MAX_ROW)
        .then(|| span(start, end))
}

/// A cell being typed into.
struct CellEdit {
    text: TextEdit,
    /// Started with F2, a double-click or the formula bar — Excel's Edit
    /// mode — where the arrow keys move in the text rather than leaving the
    /// cell (Enter mode) or pointing at one (Point mode).
    in_place: bool,
    /// A reference the arrow keys or the mouse are pointing at, as a byte
    /// range of the text and the cells it names.
    point: Option<Pointing>,
}

#[derive(Clone)]
struct Pointing {
    bytes: Range<usize>,
    anchor: CellRef,
    focus: CellRef,
}

/// What a press on the grid started.
#[derive(Clone, Copy)]
enum SheetDrag {
    /// Selecting cells, or whole rows or columns from a header.
    Select(DragKind),
    /// The fill handle: where the source would be filled to.
    Fill {
        source: CellRange,
        target: CellRange,
    },
    /// A header edge.
    Resize(ResizeGesture),
    /// A range pointed at for the formula being typed.
    Point,
}

/// What the pointer is over, for its cursor.
#[derive(Clone, Copy, PartialEq)]
enum Hover {
    Cells,
    FillHandle,
    ResizeColumn,
    ResizeRow,
}

/// The context menu.
struct SheetMenu {
    /// Where it opened, in the grid's window px.
    at: (f32, f32),
    /// Opened on a sheet tab: its index.
    tab: Option<usize>,
}

#[derive(Clone, Copy)]
enum MenuCommand {
    Cut,
    Copy,
    Paste,
    Clear,
    InsertRows,
    DeleteRows,
    InsertColumns,
    DeleteColumns,
    FillDown,
    FillRight,
    SortAscending,
    SortDescending,
    MergeCenter,
    InsertSheet,
    RenameSheet(usize),
    DeleteSheet(usize),
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
    /// The selection, and what else is drawn over the cells.
    overlay: Overlay,
    /// Bumped by every write, so an edit repaints even with nothing else
    /// changed.
    generation: u64,
}

pub struct WorkbookView {
    path: PathBuf,
    /// The shared editing model (jade-office-model), painted with Skia.
    book: Workbook<SkiaBackend>,
    sheet_names: Vec<SharedString>,
    /// The selected range and the active cell in it.
    selection: Selection,
    edit: Option<CellEdit>,
    drag: Option<SheetDrag>,
    /// The pointer during a drag, in window px, for scrolling while it is
    /// held past the grid's edge.
    pointer: Option<gpui::Point<Pixels>>,
    auto_scrolling: bool,
    /// Copied or cut cells, outlined until pasted or let go.
    copied: Option<Copied>,
    /// The column Tab started from, where Enter goes back to.
    tab_start: Option<u32>,
    /// A sheet tab being renamed, with what has been typed.
    renaming: Option<(usize, TextEdit)>,
    /// The name box being typed into: a cell or range to go to.
    naming: Option<TextEdit>,
    /// The find bar (⌘F) and what is typed into it.
    finding: Option<TextEdit>,
    menu: Option<SheetMenu>,
    hover: Hover,
    /// The note under the pointer, and where its cell's top-right corner is
    /// in the grid's window px.
    note: Option<(CellRef, (f32, f32))>,
    generation: u64,
    image: Option<Arc<RenderImage>>,
    /// Textures replaced since the last frame. gpui wants them released with a
    /// window to hand, which only `render` has.
    retired: Vec<Arc<RenderImage>>,
    painted: Option<PaintedFor>,
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
        self.selection.active
    }

    fn range(&self) -> CellRange {
        self.selection.range()
    }

    /// The text being typed into: a tab's new name, the name box, or a cell.
    fn typing(&mut self) -> Option<&mut TextEdit> {
        if let Some((_, text)) = self.renaming.as_mut() {
            return Some(text);
        }
        if let Some(text) = self.naming.as_mut() {
            return Some(text);
        }
        if let Some(text) = self.finding.as_mut() {
            return Some(text);
        }
        self.edit.as_mut().map(|edit| &mut edit.text)
    }

    fn typed(&self) -> Option<&TextEdit> {
        if let Some((_, text)) = self.renaming.as_ref() {
            return Some(text);
        }
        if let Some(text) = self.naming.as_ref() {
            return Some(text);
        }
        if let Some(text) = self.finding.as_ref() {
            return Some(text);
        }
        self.edit.as_ref().map(|edit| &edit.text)
    }

    /// Something typed that is not a pointed reference ends pointing.
    fn typed_into(&mut self) {
        if let Some(edit) = self.edit.as_mut() {
            edit.point = None;
        }
    }

    /// Whether an arrow or a click would point at a cell for the formula
    /// being typed — Excel's Point mode — rather than leave the cell.
    fn pointable(&self) -> bool {
        self.edit.as_ref().is_some_and(|edit| {
            edit.point.is_some()
                || (!edit.in_place && grid::can_point(&edit.text.text, edit.text.caret()))
        })
    }

    /// What is painted over the cells: the selection, a fill or a pointed
    /// reference dashed, the copied cells' outline, a resize's guide.
    fn overlay(&self) -> Overlay {
        let preview = match self.drag {
            Some(SheetDrag::Fill { target, .. }) => Some(target),
            _ => self
                .edit
                .as_ref()
                .and_then(|edit| edit.point.as_ref())
                .map(|point| span(point.anchor, point.focus)),
        };
        Overlay {
            selection: Some(self.selection),
            preview,
            marquee: self
                .copied
                .as_ref()
                .filter(|copied| copied.sheet == self.book.sheet_index())
                .map(|copied| copied.range),
            resize: match self.drag {
                Some(SheetDrag::Resize(gesture)) => Some(gesture),
                _ => None,
            },
        }
    }

    /// The painted image no longer shows the sheet: let it go.
    fn retire_image(&mut self) {
        if let Some(old) = self.image.take() {
            self.retired.push(old);
        }
        self.painted = None;
    }

    fn select_sheet(&mut self, index: usize, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        if index != self.book.sheet_index() && index < self.sheet_names.len() {
            if let Err(error) = self.book.select_sheet(index) {
                self.status = SharedString::from(error);
            }
            self.selection = Selection::default();
            self.tab_start = None;
            self.scroll.set_offset(point(px(0.0), px(0.0)));
            // The old image belongs to the old sheet.
            self.retire_image();
        }
        cx.notify();
    }

    /// After a write that may have changed the sheets: their names again,
    /// and a repaint.
    fn sheets_changed(&mut self, result: Result<bool, String>, cx: &mut Context<Self>) {
        self.sheet_names = self
            .book
            .sheet_names()
            .into_iter()
            .map(SharedString::from)
            .collect();
        self.retire_image();
        self.after_write(result, cx);
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

    /// A window point in the grid's sheet px (headers included).
    fn grid_point(&self, position: gpui::Point<Pixels>) -> Option<(f64, f64)> {
        let bounds = self.grid_bounds.get()?;
        let zoom = f64::from(self.zoom);
        Some((
            f64::from(position.x - bounds.origin.x) / zoom,
            f64::from(position.y - bounds.origin.y) / zoom,
        ))
    }

    /// The cell's rectangle in the window, for the cell editor and the IME.
    fn cell_bounds(&self, reference: CellRef) -> Option<Bounds<Pixels>> {
        let grid = self.grid_bounds.get()?;
        let (x, y, width, height) = self.book.cell_rect(reference, self.sheet_scroll())?;
        let zoom = self.zoom;
        Some(Bounds {
            origin: point(
                grid.origin.x + px(x as f32 * zoom),
                grid.origin.y + px(y as f32 * zoom),
            ),
            size: size(px(width as f32 * zoom), px(height as f32 * zoom)),
        })
    }

    /// Scrolls just enough to show `reference`, growing the grid to reach
    /// it.
    fn reveal(&mut self, reference: CellRef) {
        self.book.reach(reference);
        let zoom = f64::from(self.zoom);
        let at = self.sheet_scroll();
        let (x, y) = self.book.reveal(
            reference,
            at,
            (
                f64::from(self.viewport.0) / zoom,
                f64::from(self.viewport.1) / zoom,
            ),
        );
        if (x, y) != at {
            self.scroll
                .set_offset(point(px(-(x * zoom) as f32), px(-(y * zoom) as f32)));
        }
    }

    /// The selection becomes `to` — a single cell — or, extending, keeps
    /// its anchor and active cell and reaches `to`.
    fn select_cell(&mut self, to: CellRef, extend: bool, cx: &mut Context<Self>) {
        if extend {
            self.selection.extend_to(to);
        } else {
            self.selection = Selection::cell(to);
        }
        self.tab_start = None;
        self.reveal(to);
        cx.notify();
    }

    /// The active cell, or with Shift the far corner, moved by whole cells
    /// — a page of them for Page Up and Page Down.
    fn step_cell(&mut self, columns: i64, rows: i64, extend: bool, cx: &mut Context<Self>) {
        let from = if extend {
            self.selection.focus
        } else {
            self.selection.active
        };
        self.select_cell(grid::offset(from, columns, rows), extend, cx);
    }

    /// An arrow: one cell that way, over merged cells and past hidden rows
    /// and columns; with Shift the far corner moves.
    fn arrow_cell(&mut self, direction: Direction, extend: bool, cx: &mut Context<Self>) {
        let from = if extend {
            self.selection.focus
        } else {
            self.selection.active
        };
        let to = self.book.step(from, direction);
        self.select_cell(to, extend, cx);
    }

    /// Find Next (Enter in the find bar; Shift+Enter goes back): the next
    /// cell showing what is typed, on this sheet or the next.
    fn find_next(&mut self, back: bool, cx: &mut Context<Self>) {
        let Some(needle) = self.finding.as_ref().map(|text| text.text.clone()) else {
            return;
        };
        let from = (self.book.sheet_index(), self.selection.active);
        match self.book.find_next(&needle, from, back) {
            Some((index, cell)) => {
                if index != self.book.sheet_index() {
                    self.select_sheet(index, cx);
                }
                self.select_cell(cell, false, cx);
                self.status = SharedString::from("");
            }
            None => self.status = SharedString::from(i18n::text("sheet.not_found")),
        }
        cx.notify();
    }

    /// AutoSum (⌘⇧T, or ⌥=): `=SUM(` round the numbers above or beside,
    /// typed into the active cell and pointed, for arrows to adjust.
    fn autosum(&mut self, cx: &mut Context<Self>) {
        let range = self.book.autosum_range(self.active());
        let mut text = TextEdit::new("=SUM(".to_string());
        let point = range.map(|range| {
            let reference = range_text(range);
            let start = text.text.len();
            text.insert(&reference);
            Pointing {
                bytes: start..start + reference.len(),
                anchor: range.start,
                focus: range.end,
            }
        });
        text.insert(")");
        if let Some(point) = &point {
            text.caret = point.bytes.end;
        }
        self.edit = Some(CellEdit {
            text,
            in_place: false,
            point,
        });
        self.reveal(self.active());
        cx.notify();
    }

    /// ⌘-arrow: to the end of the data that way, as Excel's Ctrl+arrow.
    fn jump_cell(&mut self, direction: Direction, extend: bool, cx: &mut Context<Self>) {
        let from = if extend {
            self.selection.focus
        } else {
            self.selection.active
        };
        let to = self.book.jump(from, direction);
        self.select_cell(to, extend, cx);
    }

    /// Enter or Tab outside an edit: round the range, or on to the next
    /// cell — Enter after Tabs going back to the column the Tabs started
    /// from, as in Excel.
    fn advance_cell(&mut self, by_row: bool, back: bool, cx: &mut Context<Self>) {
        if self.selection.is_cell() {
            match (by_row, self.tab_start) {
                (true, None) if !back => self.tab_start = Some(self.selection.active.column),
                (false, Some(column)) if !back => {
                    let to = CellRef::new(column, (self.selection.active.row + 1).min(MAX_ROW));
                    self.select_cell(to, false, cx);
                    return;
                }
                (false, _) => self.tab_start = None,
                _ => {}
            }
        }
        self.selection.advance(by_row, back);
        let to = self.selection.active;
        self.reveal(to);
        cx.notify();
    }

    /// ⌘A: the block of data round the active cell, then the whole sheet.
    fn select_all(&mut self, cx: &mut Context<Self>) {
        // The name box, the find bar or a tab's name: the caret to the end.
        if self.renaming.is_some() || self.naming.is_some() || self.finding.is_some() {
            if let Some(text) = self.typing() {
                text.to_end(true);
            }
            cx.notify();
            return;
        }
        // A cell being typed into has no text selection to take.
        if self.edit.is_some() {
            return;
        }
        let active = self.selection.active;
        let range = self.selection.range();
        self.selection = match self.book.current_region(active) {
            Some(region) if region != range => Selection {
                active,
                anchor: region.start,
                focus: region.end,
            },
            _ => Selection {
                active,
                ..Selection::all()
            },
        };
        cx.notify();
    }

    // ---- editing ---------------------------------------------------------

    fn begin_edit(&mut self, text: Option<String>, in_place: bool, cx: &mut Context<Self>) {
        let text = text.unwrap_or_else(|| self.book.edit_text(self.active()));
        self.edit = Some(CellEdit {
            text: TextEdit::new(text),
            in_place,
            point: None,
        });
        self.reveal(self.active());
        cx.notify();
    }

    /// Writes the cell being typed into, if any; the write recalculates, so
    /// dependent formulas have moved before the next paint. A formula Excel
    /// would refuse keeps the editor open and returns false.
    fn commit_cell(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(edit) = self.edit.take() else {
            return true;
        };
        let result = self.book.set_cell(self.active(), &edit.text.text);
        let refused = result.is_err();
        if refused {
            self.edit = Some(CellEdit {
                point: None,
                ..edit
            });
        } else if matches!(result, Ok(true)) {
            // Typing into a cell lets go of what was copied, as in Excel.
            self.copied = None;
        }
        self.after_write(result, cx);
        !refused
    }

    /// After a write: repaint, or say why it failed.
    fn after_write(&mut self, result: Result<bool, String>, cx: &mut Context<Self>) {
        match result {
            Ok(false) => {}
            Ok(true) => {
                self.generation += 1;
                self.status = SharedString::from("");
            }
            // A formula Excel would refuse: its words, not a write error.
            Err(error) if error == jade_office_model::typing::FORMULA_PROBLEM => {
                self.status = SharedString::from(i18n::text("sheet.formula_problem"))
            }
            Err(error) => {
                self.status =
                    SharedString::from(i18n::format("sheet.edit_failed", &[("error", &error)]))
            }
        }
        cx.notify();
    }

    fn step_history(&mut self, undo: bool, cx: &mut Context<Self>) {
        self.edit = None;
        self.renaming = None;
        self.naming = None;
        let result = if undo {
            self.book.undo()
        } else {
            self.book.redo()
        };
        // Undo can bring a sheet back or take one away.
        if self.book.sheet_index() >= self.book.sheet_names().len() {
            self.selection = Selection::default();
        }
        self.sheets_changed(result, cx);
    }

    // ---- pointing at cells in a formula ------------------------------------

    /// Points the formula being typed at `anchor`..`focus`: the reference
    /// goes in at the caret, or replaces the one being pointed with.
    fn point_at(&mut self, anchor: CellRef, focus: CellRef, cx: &mut Context<Self>) {
        let Some(edit) = self.edit.as_mut() else {
            return;
        };
        let reference = range_text(span(anchor, focus));
        let bytes = match &edit.point {
            Some(point) => point.bytes.clone(),
            None => {
                let caret = edit.text.caret();
                caret..caret
            }
        };
        edit.text.text.replace_range(bytes.clone(), &reference);
        edit.text.caret = bytes.start + reference.len();
        edit.point = Some(Pointing {
            bytes: bytes.start..bytes.start + reference.len(),
            anchor,
            focus,
        });
        self.reveal(focus);
        cx.notify();
    }

    /// An arrow while a formula can point: the pointed cell moves (or with
    /// Shift the pointed range grows, or with ⌘ jumps), starting from the
    /// cell being typed into.
    fn point_step(
        &mut self,
        direction: Direction,
        extend: bool,
        jump: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(edit) = self.edit.as_ref() else {
            return;
        };
        let (anchor, focus) = match &edit.point {
            Some(point) => (point.anchor, point.focus),
            None => (self.selection.active, self.selection.active),
        };
        let from = if extend { focus } else { anchor };
        let to = if jump {
            self.book.jump(from, direction)
        } else {
            let (columns, rows) = direction.delta();
            grid::offset(from, columns, rows)
        };
        if extend {
            self.point_at(anchor, to, cx);
        } else {
            self.point_at(to, to, cx);
        }
    }

    // ---- clipboard -------------------------------------------------------

    /// The selected cells onto the clipboard as Excel's text — tab-separated
    /// by column, newline-separated by row — kept for a paste here that
    /// moves formulas and carries formats; a cut moves them when pasted.
    fn sheet_copy(&mut self, cut: bool, cx: &mut Context<Self>) {
        let range = self.range();
        let copied = self.book.copy(range, cut);
        let text = copied
            .as_ref()
            .map(|copied| copied.text.clone())
            .unwrap_or_else(|| self.book.range_text(range));
        self.copied = copied;
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        cx.notify();
    }

    /// ⌘C and ⌘X: the cells, unless text is being typed, which has no
    /// selection of its own to take.
    fn copy(&mut self, cut: bool, cx: &mut Context<Self>) {
        if self.typed().is_none() {
            self.sheet_copy(cut, cx);
        }
    }

    /// ⌘V: into text being typed, the clipboard's first line, as in Excel;
    /// onto the sheet, the clipboard's cells.
    fn paste(&mut self, cx: &mut Context<Self>) {
        let text = cx.read_from_clipboard().and_then(|item| item.text());
        if let Some(typing) = self.typing() {
            if let Some(text) = text {
                typing.insert(text.lines().next().unwrap_or(""));
                self.typed_into();
                cx.notify();
            }
            return;
        }
        self.sheet_paste(text.as_deref(), cx);
    }

    /// A paste onto the selection: this workbook's own copy when the
    /// clipboard still holds it, otherwise the clipboard's text, from the
    /// selection's top-left or tiled over it.
    fn sheet_paste(&mut self, text: Option<&str>, cx: &mut Context<Self>) {
        let target = self.range();
        let own = self
            .copied
            .clone()
            .filter(|copied| text.is_none_or(|text| text.replace("\r\n", "\n") == copied.text));
        let result = match (own, text) {
            (Some(copied), _) => {
                let result = self.book.paste_copied(&copied, target);
                if copied.cut && result.is_ok() {
                    self.copied = None;
                }
                result.map(Some)
            }
            (None, Some(text)) => self.book.paste_over(target, text),
            (None, None) => return,
        };
        if let Ok(Some(filled)) = &result {
            let active = self.selection.active;
            self.selection = Selection {
                active: if filled.contains(active) {
                    active
                } else {
                    filled.start
                },
                anchor: filled.start,
                focus: filled.end,
            };
        }
        self.sheets_changed(result.map(|filled| filled.is_some()), cx);
    }

    // ---- commands ----------------------------------------------------------

    fn toggle(&mut self, style: FontToggle, cx: &mut Context<Self>) {
        let result = self.book.toggle(self.range(), self.active(), style);
        self.after_write(result, cx);
    }

    fn number_format(&mut self, code: &str, cx: &mut Context<Self>) {
        let change = CellFormat {
            number_format: Some(code.to_string()),
            ..Default::default()
        };
        let result = self.book.format(self.range(), &change);
        self.after_write(result, cx);
    }

    fn menu_command(&mut self, command: MenuCommand, from_keys: bool, cx: &mut Context<Self>) {
        self.menu = None;
        let selection = self.selection;
        let range = selection.range();
        let rows = range.end.row.saturating_sub(range.start.row) + 1;
        let columns = range.end.column.saturating_sub(range.start.column) + 1;
        // ⌘+ and ⌃- insert and delete whole columns when whole columns are
        // selected, rows otherwise.
        let command = match command {
            MenuCommand::InsertRows if from_keys && selection.is_whole_columns() => {
                MenuCommand::InsertColumns
            }
            MenuCommand::DeleteRows if from_keys && selection.is_whole_columns() => {
                MenuCommand::DeleteColumns
            }
            command => command,
        };
        let result = match command {
            MenuCommand::Cut => return self.sheet_copy(true, cx),
            MenuCommand::Copy => return self.sheet_copy(false, cx),
            MenuCommand::Paste => return self.sheet_paste(None, cx),
            MenuCommand::Clear => self.book.clear(range),
            MenuCommand::InsertRows => self.book.insert_rows(range.start.row, rows),
            MenuCommand::DeleteRows => self.book.delete_rows(range.start.row, rows),
            MenuCommand::InsertColumns => self.book.insert_columns(range.start.column, columns),
            MenuCommand::DeleteColumns => self.book.delete_columns(range.start.column, columns),
            MenuCommand::FillDown => self.book.fill_down(range),
            MenuCommand::FillRight => self.book.fill_right(range),
            MenuCommand::SortAscending => self.book.sort(range, selection.active, false),
            MenuCommand::SortDescending => self.book.sort(range, selection.active, true),
            // Merge & Center takes in every merged cell the range touches,
            // and the merge stays selected.
            MenuCommand::MergeCenter => {
                let range = self.book.with_merges(range);
                let result = self.book.merge_and_center(range);
                self.selection = Selection::range_from(range.start, range.end);
                result
            }
            MenuCommand::InsertSheet => {
                let result = self.book.add_sheet().map(|_| true);
                self.selection = Selection::default();
                self.scroll.set_offset(point(px(0.0), px(0.0)));
                result
            }
            MenuCommand::RenameSheet(index) => {
                let name = self.sheet_names.get(index).map(|name| name.to_string());
                self.renaming = name.map(|name| (index, TextEdit::selected(name)));
                cx.notify();
                return;
            }
            MenuCommand::DeleteSheet(index) => {
                let result = self.book.delete_sheet(index);
                self.selection = Selection::default();
                self.scroll.set_offset(point(px(0.0), px(0.0)));
                result
            }
        };
        // What was copied moved or went with the deleted cells.
        if matches!(
            command,
            MenuCommand::DeleteRows | MenuCommand::DeleteColumns | MenuCommand::DeleteSheet(_)
        ) {
            self.copied = None;
        }
        self.sheets_changed(result, cx);
    }

    // ---- saving ----------------------------------------------------------

    /// Saves over the file that was opened, as Excel does.
    fn save(&mut self, cx: &mut Context<Self>) {
        let target = self.path.clone();
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
        // A formula Excel would refuse stays in its editor, unsaved; the
        // rest of the workbook is saved.
        self.commit_cell(cx);
        let result = self.book.save_bytes().and_then(|bytes| {
            crate::shell::write_in_place(&target, &bytes).map_err(|error| error.to_string())
        });
        self.status = SharedString::from(match result {
            Ok(()) => i18n::format(
                "sheet.saved",
                &[("name", &crate::shell::file_name(&target))],
            ),
            Err(error) => i18n::format("sheet.save_failed", &[("error", &error)]),
        });
        cx.notify();
    }

    // ---- keys ------------------------------------------------------------

    /// Keys that are not actions in the keymap: the arrows and the other
    /// keys of the grid and of a cell being typed into, and Excel's ⌘ (or
    /// Ctrl) shortcuts. ⌘C, ⌘X, ⌘V, ⌘A, ⌘Z and ⌘S come as actions.
    fn handle_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if self.sheet_key(event, cx) {
            cx.stop_propagation();
        }
    }

    /// A key on the sheet; true when it was the sheet's.
    fn sheet_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let modifiers = event.keystroke.modifiers;
        let key = event.keystroke.key.as_str();
        // ⌘ on a Mac; Excel's Ctrl shortcuts are taken with Ctrl as well.
        let command = modifiers.platform || modifiers.control;
        let shift = modifiers.shift;
        if self.menu.take().is_some() {
            cx.notify();
        }

        // A tab being renamed, the name box or the find bar: Enter takes
        // it (or finds the next), Escape leaves it.
        if self.renaming.is_some() || self.naming.is_some() || self.finding.is_some() {
            if self.finding.is_some() && key == "enter" {
                self.find_next(shift, cx);
                return true;
            }
            if self.finding.is_some() && key == "f" && command {
                return true;
            }
            return self.name_key(key, command, cx);
        }

        let pointable = self.pointable();
        if let Some(edit) = self.edit.as_mut() {
            // Mid-composition the keys are the IME's.
            if edit.text.marked.is_some() {
                return false;
            }
            match key {
                // A line break in the cell: ⌥Enter, as Excel for Mac.
                "enter" if modifiers.alt => {
                    edit.text.insert("\n");
                    edit.point = None;
                }
                // ⌘Enter: what was typed into every selected cell.
                "enter" if command => {
                    let text = edit.text.text.clone();
                    self.edit = None;
                    let result = self.book.fill_with(self.range(), self.active(), &text);
                    if matches!(result, Ok(true)) {
                        self.copied = None;
                    }
                    self.after_write(result, cx);
                }
                "enter" => {
                    if self.commit_cell(cx) {
                        self.advance_cell(false, shift, cx);
                    }
                }
                "tab" => {
                    if self.commit_cell(cx) {
                        self.advance_cell(true, shift, cx);
                    }
                }
                "escape" => self.edit = None,
                "backspace" => {
                    edit.text.backspace();
                    edit.point = None;
                }
                "delete" => {
                    edit.text.delete();
                    edit.point = None;
                }
                // F2 switches between Edit mode and Enter mode, as in Excel.
                "f2" => {
                    edit.in_place = !edit.in_place;
                    edit.point = None;
                }
                // F4 cycles the reference at the caret through its `$`s.
                "f4" => {
                    if let Some((text, caret)) =
                        grid::cycle_reference(&edit.text.text, edit.text.caret())
                    {
                        edit.text.text = text;
                        edit.text.caret = caret;
                        edit.point = None;
                    }
                }
                "home" if edit.in_place || !command => edit.text.to_end(false),
                "end" if edit.in_place || !command => edit.text.to_end(true),
                // ⌃; today's date, ⌘; the time, at the caret.
                ";" if modifiers.control || (modifiers.platform && !shift) => {
                    let time = modifiers.platform || shift;
                    edit.text.insert(&now_text(time));
                    edit.point = None;
                }
                _ => {
                    let Some(direction) = Direction::of_key(key) else {
                        // Text arrives through the input handler.
                        return false;
                    };
                    if pointable {
                        self.point_step(direction, shift, command, cx);
                    } else if edit.in_place {
                        match direction {
                            Direction::Left => edit.text.step(false),
                            Direction::Right => edit.text.step(true),
                            Direction::Up => edit.text.to_end(false),
                            Direction::Down => edit.text.to_end(true),
                        }
                    } else if self.commit_cell(cx) {
                        // Enter mode: the arrow leaves the cell, as in Excel.
                        self.arrow_cell(direction, false, cx);
                    }
                }
            }
            cx.notify();
            return true;
        }

        if command {
            return self.sheet_command(key, modifiers, cx);
        }
        match key {
            "pageup" | "pagedown" => self.page(key == "pageup", modifiers, cx),
            "home" => {
                let row = if shift {
                    self.selection.focus.row
                } else {
                    self.selection.active.row
                };
                self.select_cell(CellRef::new(0, row), shift, cx);
            }
            "enter" => self.advance_cell(false, shift, cx),
            "tab" => self.advance_cell(true, shift, cx),
            "f2" => self.begin_edit(None, true, cx),
            "delete" | "backspace" => {
                let result = self.book.clear(self.range());
                self.after_write(result, cx);
            }
            // Escape lets go of what was copied.
            "escape" => {
                self.copied = None;
                cx.notify();
            }
            "=" if modifiers.alt => self.autosum(cx),
            // Shift+Space: the selection's rows, whole.
            "space" if shift => {
                self.selection.whole_rows();
                cx.notify();
            }
            _ => {
                let Some(direction) = Direction::of_key(key) else {
                    // Anything else is text, which arrives through the input
                    // handler and starts an edit there — so an IME composes
                    // into the cell.
                    return false;
                };
                self.arrow_cell(direction, shift, cx);
            }
        }
        true
    }

    /// Page Up and Page Down: a screenful of rows, the view moving with the
    /// active cell so it stays where it was on the screen; with ⌥, a
    /// screenful of columns sideways.
    fn page(&mut self, up: bool, modifiers: Modifiers, cx: &mut Context<Self>) {
        let shift = modifiers.shift;
        if modifiers.alt {
            self.step_cell(if up { -8 } else { 8 }, 0, shift, cx);
            return;
        }
        let from = if shift {
            self.selection.focus
        } else {
            self.selection.active
        };
        let zoom = f64::from(self.zoom.max(0.01));
        let rows = i64::from(
            self.book
                .page_rows(from.row, f64::from(self.viewport.1) / zoom),
        );
        let rows = if up { -rows } else { rows };
        let to = grid::offset(from, 0, rows);
        let at = self.sheet_scroll();
        let moved = self
            .book
            .cell_rect(to, (0.0, 0.0))
            .zip(self.book.cell_rect(from, (0.0, 0.0)))
            .map(|(to, from)| to.1 - from.1)
            .unwrap_or(0.0);
        let y = (at.1 + moved).max(0.0);
        self.book.reach(to);
        self.scroll
            .set_offset(point(px((-at.0 * zoom) as f32), px((-y * zoom) as f32)));
        self.step_cell(0, rows, shift, cx);
    }

    /// ⌘ (or Ctrl) with a key, on a sheet not being typed into.
    fn sheet_command(&mut self, key: &str, modifiers: Modifiers, cx: &mut Context<Self>) -> bool {
        let shift = modifiers.shift;
        // Ctrl+Shift with a digit row key: its shifted character, as the
        // number-format shortcuts are named.
        let key = match (shift, key) {
            (true, "1") => "!",
            (true, "2") => "@",
            (true, "3") => "#",
            (true, "4") => "$",
            (true, "5") => "%",
            (true, "6") => "^",
            (true, "`") => "~",
            (true, "=") => "+",
            _ => key,
        };
        if let Some(direction) = Direction::of_key(key) {
            self.jump_cell(direction, shift, cx);
            return true;
        }
        let range = self.range();
        let active = self.active();
        match key {
            "home" => self.select_cell(CellRef::new(0, 0), shift, cx),
            "t" if shift && modifiers.platform => self.autosum(cx),
            "f" => {
                self.finding = Some(TextEdit::new(String::new()));
                cx.notify();
            }
            "end" => {
                let end = self.book.used_end();
                self.select_cell(end, shift, cx);
            }
            // ⌘C, ⌘X, ⌘V and ⌘A are actions; these are their Ctrl forms.
            "c" if !modifiers.platform => self.sheet_copy(false, cx),
            "x" if !shift && !modifiers.platform => self.sheet_copy(true, cx),
            "v" if !modifiers.platform => self.paste(cx),
            "a" if !modifiers.platform => self.select_all(cx),
            "d" => {
                let result = self.book.fill_down(range);
                self.after_write(result, cx);
            }
            "r" => {
                let result = self.book.fill_right(range);
                self.after_write(result, cx);
            }
            "b" | "2" => self.toggle(FontToggle::Bold, cx),
            "i" | "3" => self.toggle(FontToggle::Italic, cx),
            "u" | "4" => self.toggle(FontToggle::Underline, cx),
            "x" | "5" => self.toggle(FontToggle::Strike, cx),
            "!" => self.number_format("#,##0.00", cx),
            "$" => self.number_format(i18n::text("sheet.format.currency"), cx),
            "%" => self.number_format("0%", cx),
            "#" => self.number_format(i18n::text("sheet.format.date"), cx),
            "@" => self.number_format(i18n::text("sheet.format.time"), cx),
            "^" => self.number_format("0.00E+00", cx),
            "~" => self.number_format("General", cx),
            // ⌃; today, ⌘; (or Ctrl+Shift+;) the time, into the active cell.
            ";" => {
                let time = modifiers.platform || shift;
                let result = self.book.set_cell(active, &now_text(time));
                self.after_write(result, cx);
            }
            // ⌘+ (⌘⇧=, ⌃⇧=) inserts; ⌃- deletes. ⌘- stays the app's zoom
            // out, a keymap action the sheet never sees.
            "+" => self.menu_command(MenuCommand::InsertRows, true, cx),
            "-" => self.menu_command(MenuCommand::DeleteRows, true, cx),
            // Ctrl+Space: the selection's columns, whole.
            "space" => {
                self.selection.whole_columns();
                cx.notify();
            }
            _ => return false,
        }
        true
    }

    /// Keys while a tab is renamed or the name box is typed into.
    fn name_key(&mut self, key: &str, command: bool, cx: &mut Context<Self>) -> bool {
        let Some(text) = self.typing() else {
            return false;
        };
        if text.marked.is_some() {
            return false;
        }
        match key {
            "enter" | "tab" => self.finish_naming(true, cx),
            "escape" => self.finish_naming(false, cx),
            "backspace" => text.backspace(),
            "delete" => text.delete(),
            "left" => text.step(false),
            "right" => text.step(true),
            "home" | "up" => text.to_end(false),
            "end" | "down" => text.to_end(true),
            "a" if command => text.whole = true,
            _ => return command,
        }
        cx.notify();
        true
    }

    /// Ends a tab rename or a name-box entry, taking it or not.
    fn finish_naming(&mut self, take: bool, cx: &mut Context<Self>) {
        if let Some((index, text)) = self.renaming.take() {
            let name = text.text.trim().to_string();
            if take && !name.is_empty() {
                let result = self.book.rename_sheet(index, &name);
                self.sheets_changed(result, cx);
            }
        } else if self.finding.take().is_some() {
            // Escape or Tab closes the find bar where the search got to.
        } else if let Some(text) = self.naming.take() {
            if take {
                match parse_reference(&text.text) {
                    Some(range) => {
                        self.selection = Selection::range_from(range.start, range.end);
                        self.tab_start = None;
                        self.reveal(range.start);
                    }
                    None => self.status = SharedString::from(i18n::text("sheet.no_reference")),
                }
            }
        }
        cx.notify();
    }

    // ---- the mouse -------------------------------------------------------

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus);
        self.menu = None;
        if self.renaming.is_some() || self.naming.is_some() {
            self.finish_naming(true, cx);
        }
        let Some((x, y)) = self.grid_point(event.position) else {
            return;
        };
        let scroll = self.sheet_scroll();
        let hit = self.book.hit(x, y, scroll);

        // While a formula can take a reference, a click points at the cell
        // and a drag at a range; Shift-click reaches from what is pointed.
        if self.pointable() {
            if let Some(Hit::Cell(cell)) = hit {
                self.drag = Some(SheetDrag::Point);
                let anchor = match (&self.edit, event.modifiers.shift) {
                    (
                        Some(CellEdit {
                            point: Some(point), ..
                        }),
                        true,
                    ) => point.anchor,
                    _ => cell,
                };
                self.point_at(anchor, cell, cx);
                return;
            }
        }
        if !self.commit_cell(cx) {
            return;
        }
        self.tab_start = None;

        // A header edge: a drag resizes, a double-click fits the content.
        if let Some(edge) = self.book.resize_hit(x, y, scroll) {
            if event.click_count >= 2 && edge.axis == Axis::Columns {
                let range = self.range();
                // Every selected column fits when the edge is one of them.
                let (first, last) = if self.selection.is_whole_columns()
                    && (range.start.column..=range.end.column).contains(&edge.index)
                {
                    (range.start.column, range.end.column)
                } else {
                    (edge.index, edge.index)
                };
                let result = self.book.auto_fit_columns(first, last);
                self.after_write(result, cx);
                return;
            }
            self.drag = Some(SheetDrag::Resize(ResizeGesture {
                hit: edge,
                pointer_start: if edge.axis == Axis::Columns { x } else { y },
                size_px: edge.size_px,
            }));
            cx.notify();
            return;
        }
        // The fill handle.
        let range = self.range();
        if !event.modifiers.shift && self.book.on_fill_handle(x, y, scroll, range) {
            // A double-click carries it down as far as the data beside it.
            if event.click_count >= 2 {
                if let Some(target) = self.book.fill_handle_extent(range) {
                    let result = self.book.fill_series(range, target);
                    self.selection.anchor = target.start;
                    self.selection.focus = target.end;
                    self.after_write(result, cx);
                }
                return;
            }
            self.drag = Some(SheetDrag::Fill {
                source: range,
                target: range,
            });
            cx.notify();
            return;
        }
        let Some(hit) = hit else {
            return;
        };
        let kind = self.selection.press(hit, event.modifiers.shift);
        self.drag = Some(SheetDrag::Select(kind));
        if let Hit::Cell(cell) = hit {
            self.book.reach(cell);
            if event.click_count >= 2 && !event.modifiers.shift {
                self.drag = None;
                self.begin_edit(None, true, cx);
            }
        }
        cx.notify();
    }

    /// A right-click: the cell (or header) under it is selected unless it
    /// is in the selection already, and the menu opens there.
    fn right_click(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus);
        if !self.commit_cell(cx) {
            return;
        }
        let Some((x, y)) = self.grid_point(event.position) else {
            return;
        };
        if let Some(hit) = self.book.hit(x, y, self.sheet_scroll()) {
            let range = self.range();
            let inside = match hit {
                Hit::Cell(cell) => range.contains(cell),
                Hit::Row(row) => {
                    self.selection.is_whole_rows()
                        && (range.start.row..=range.end.row).contains(&row)
                }
                Hit::Column(column) => {
                    self.selection.is_whole_columns()
                        && (range.start.column..=range.end.column).contains(&column)
                }
                Hit::All => false,
            };
            if !inside {
                self.selection.press(hit, false);
            }
        }
        let zoom = f64::from(self.zoom);
        self.menu = Some(SheetMenu {
            at: ((x * zoom) as f32, (y * zoom) as f32),
            tab: None,
        });
        cx.notify();
    }

    /// The pointer moving with no button held: what it is over, for the
    /// cursor.
    fn hover_at(&mut self, position: gpui::Point<Pixels>, cx: &mut Context<Self>) {
        if self.drag.is_some() {
            return;
        }
        let Some((x, y)) = self.grid_point(position) else {
            return;
        };
        let scroll = self.sheet_scroll();
        let hover = match self.book.resize_hit(x, y, scroll) {
            Some(edge) if edge.axis == Axis::Columns => Hover::ResizeColumn,
            Some(_) => Hover::ResizeRow,
            None if self.book.on_fill_handle(x, y, scroll, self.range()) => Hover::FillHandle,
            None => Hover::Cells,
        };
        if hover != self.hover {
            self.hover = hover;
            cx.notify();
        }
        // A cell with a note shows it beside the cell while the pointer is
        // on it, as Excel does.
        let zoom = self.zoom;
        let note = match self.book.hit(x, y, scroll) {
            Some(Hit::Cell(cell)) if self.book.note(cell).is_some() => self
                .book
                .cell_rect(cell, scroll)
                .map(|(left, top, width, _)| {
                    (cell, (((left + width) as f32) * zoom, top as f32 * zoom))
                }),
            _ => None,
        };
        if note.map(|(cell, _)| cell) != self.note.map(|(cell, _)| cell) {
            self.note = note;
            cx.notify();
        }
    }

    /// A drag reaching `position` (window px), wherever it is.
    fn drag_to(&mut self, position: gpui::Point<Pixels>, cx: &mut Context<Self>) {
        let Some(drag) = self.drag else {
            return;
        };
        let Some((x, y)) = self.grid_point(position) else {
            return;
        };
        self.pointer = Some(position);
        let scroll = self.sheet_scroll();
        let reached = self.book.reach_point(x, y, scroll);
        match drag {
            SheetDrag::Select(kind) => {
                let hit = self.book.hit(x, y, scroll).unwrap_or(Hit::Cell(reached));
                let hit = match (kind, hit) {
                    // Over a header mid-drag, a cell drag reaches the cell
                    // beside it.
                    (DragKind::Cells, Hit::Row(_) | Hit::Column(_) | Hit::All) => {
                        Hit::Cell(reached)
                    }
                    (_, hit) => hit,
                };
                let before = self.selection;
                self.selection.drag(kind, hit);
                if self.selection != before {
                    self.book.reach(reached);
                    cx.notify();
                }
            }
            SheetDrag::Fill { source, target } => {
                let next = Workbook::<SkiaBackend>::fill_target(source, reached);
                if next != target {
                    self.drag = Some(SheetDrag::Fill {
                        source,
                        target: next,
                    });
                    self.book.reach(reached);
                    cx.notify();
                }
            }
            SheetDrag::Resize(mut gesture) => {
                gesture.update(x, y);
                self.drag = Some(SheetDrag::Resize(gesture));
                cx.notify();
            }
            SheetDrag::Point => {
                let anchor = self
                    .edit
                    .as_ref()
                    .and_then(|edit| edit.point.as_ref())
                    .map_or(reached, |point| point.anchor);
                self.point_at(anchor, reached, cx);
            }
        }
        self.start_auto_scroll(cx);
    }

    /// The button let go: a fill lands, a resize sets the size.
    fn drag_end(&mut self, cx: &mut Context<Self>) {
        let Some(drag) = self.drag.take() else {
            return;
        };
        self.pointer = None;
        match drag {
            SheetDrag::Fill { source, target } => {
                let result = self.book.fill_series(source, target);
                if matches!(result, Ok(true)) {
                    self.selection.anchor = target.start;
                    self.selection.focus = target.end;
                    if !target.contains(self.selection.active) {
                        self.selection.active = target.start;
                    }
                }
                self.after_write(result, cx);
            }
            SheetDrag::Resize(gesture) => {
                let range = self.range();
                let index = gesture.hit.index;
                // Every selected column (or row) takes the size when the
                // dragged edge is one of them.
                let (first, last) = match gesture.hit.axis {
                    Axis::Columns
                        if self.selection.is_whole_columns()
                            && (range.start.column..=range.end.column).contains(&index) =>
                    {
                        (range.start.column, range.end.column)
                    }
                    Axis::Rows
                        if self.selection.is_whole_rows()
                            && (range.start.row..=range.end.row).contains(&index) =>
                    {
                        (range.start.row, range.end.row)
                    }
                    _ => (index, index),
                };
                let result = if (gesture.size_px - gesture.hit.size_px).abs() < 0.5 {
                    Ok(false)
                } else {
                    self.book
                        .resize(gesture.hit.axis, first, last, gesture.size_px)
                };
                self.after_write(result, cx);
            }
            SheetDrag::Select(_) | SheetDrag::Point => cx.notify(),
        }
    }

    /// While a drag is held past the grid's edge, the grid scrolls that
    /// way and the drag follows.
    fn start_auto_scroll(&mut self, cx: &mut Context<Self>) {
        if self.auto_scrolling || self.auto_scroll_step().is_none() {
            return;
        }
        self.auto_scrolling = true;
        let executor = cx.background_executor().clone();
        cx.spawn(async move |view, cx| loop {
            executor.timer(AUTO_SCROLL_TICK).await;
            let going = view
                .update(cx, |view, cx| {
                    let Some(step) = view.auto_scroll_step() else {
                        view.auto_scrolling = false;
                        return false;
                    };
                    let offset = view.scroll.offset();
                    let max = view.scroll.max_offset();
                    let x = (offset.x - px(step.0)).clamp(-max.width, px(0.0));
                    let y = (offset.y - px(step.1)).clamp(-max.height, px(0.0));
                    view.scroll.set_offset(point(x, y));
                    if let Some(pointer) = view.pointer {
                        view.drag_to(pointer, cx);
                    }
                    cx.notify();
                    true
                })
                .unwrap_or(false);
            if !going {
                break;
            }
        })
        .detach();
    }

    /// How far a held drag scrolls per tick (window px), or `None` when the
    /// pointer is inside the grid.
    fn auto_scroll_step(&self) -> Option<(f32, f32)> {
        self.drag?;
        let pointer = self.pointer?;
        let bounds = self.grid_bounds.get()?;
        let past = |value: Pixels, start: Pixels, end: Pixels| -> f32 {
            if value < start {
                f32::from(value - start)
            } else if value > end {
                f32::from(value - end)
            } else {
                0.0
            }
        };
        let header = px(20.0);
        let dx = past(
            pointer.x,
            bounds.origin.x + header * 2.0,
            bounds.origin.x + bounds.size.width,
        );
        let dy = past(
            pointer.y,
            bounds.origin.y + header,
            bounds.origin.y + bounds.size.height,
        );
        (dx != 0.0 || dy != 0.0).then(|| (dx.clamp(-60.0, 60.0), dy.clamp(-60.0, 60.0)))
    }

    // ---- painting --------------------------------------------------------

    /// Repaints when the scroll position, the space, the zoom, the overlay
    /// or the sheet has moved since the image was made.
    ///
    /// `viewport` is in window px. The image is painted at device pixels —
    /// the window's scale factor times the zoom per sheet px — so a sheet is
    /// as sharp on a Retina display as the text round it.
    fn repaint_if_needed(&mut self, viewport: (f32, f32), scale_factor: f32) {
        let offset = self.scroll.offset();
        let scale = scale_factor * self.zoom;
        let wanted = PaintedFor {
            sheet: self.book.sheet_index(),
            // The scroll offset is negative going down; the sheet's
            // coordinates are not.
            scroll: (-f32::from(offset.x), -f32::from(offset.y)),
            size: (
                (viewport.0 * scale_factor).round().max(1.0) as i32,
                (viewport.1 * scale_factor).round().max(1.0) as i32,
            ),
            scale,
            overlay: self.overlay(),
            generation: self.generation,
        };
        if self.painted == Some(wanted) && self.image.is_some() {
            return;
        }
        match self.paint(&wanted) {
            Some(image) => {
                if let Some(previous) = self.image.replace(image) {
                    self.retired.push(previous);
                }
                self.painted = Some(wanted);
            }
            None => self.status = SharedString::from(i18n::text("sheet.paint_failed")),
        }
    }

    fn paint(&mut self, wanted: &PaintedFor) -> Option<Arc<RenderImage>> {
        let zoom = f64::from(self.zoom);
        let painter = self.book.render_viewport(
            (
                f64::from(wanted.scroll.0) / zoom,
                f64::from(wanted.scroll.1) / zoom,
            ),
            wanted.size.0 as u32,
            wanted.size.1 as u32,
            f64::from(wanted.scale),
            &wanted.overlay,
        )?;
        let (bgra, width, height) = SkiaBackend::bgra(&painter)?;
        let buffer = image::ImageBuffer::from_raw(width, height, bgra)?;
        Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
    }

    fn formula_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let editing = self.edit.is_some();
        let formula = match &self.edit {
            Some(edit) => caret_line(&edit.text, theme::accent().into(), true),
            None => div().child(self.book.edit_text(self.active())),
        };
        // The name box: the active cell, the size of a range being dragged,
        // or what is being typed into it.
        let name_box = match (&self.naming, self.drag) {
            (Some(text), _) => caret_line(text, theme::accent().into(), false),
            (None, Some(SheetDrag::Select(_))) if !self.selection.is_cell() => {
                div().child(grid::size_label(self.range()))
            }
            _ => div().child(reference_text(self.active())),
        };
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
                    .id("name-box")
                    .w(px(96.0))
                    .px_2()
                    .py_0p5()
                    .rounded_sm()
                    .border_1()
                    .border_color(theme::border())
                    .bg(theme::window())
                    .text_color(theme::text())
                    .cursor_text()
                    .overflow_hidden()
                    .child(name_box)
                    // Typing a reference here and Enter goes to it.
                    .on_click(cx.listener(|view, _event, window, cx| {
                        window.focus(&view.focus);
                        view.menu = None;
                        if !view.commit_cell(cx) {
                            return;
                        }
                        if view.naming.is_none() {
                            view.naming = Some(TextEdit::selected(reference_text(view.active())));
                        }
                        cx.notify();
                    })),
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
                    .overflow_hidden()
                    .child(formula)
                    // A click in the formula bar edits in place, as in Excel.
                    .on_click(cx.listener(move |view, _event, window, cx| {
                        window.focus(&view.focus);
                        view.menu = None;
                        if !editing {
                            view.begin_edit(None, true, cx);
                        } else if let Some(edit) = view.edit.as_mut() {
                            edit.in_place = true;
                            edit.point = None;
                            cx.notify();
                        }
                    })),
            )
    }

    /// The editor over the active cell while it is being typed into, white
    /// like the cell under it.
    fn cell_editor(
        &self,
        grid: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let edit = self.edit.as_ref()?;
        let cell = self.cell_bounds(self.active())?;
        let zoom = self.zoom;
        let lines = edit.text.text.matches('\n').count() as f32 + 1.0;
        Some(
            div()
                .absolute()
                .left(cell.origin.x - grid.origin.x)
                .top(cell.origin.y - grid.origin.y)
                .min_w(cell.size.width)
                .min_h(cell.size.height.max(px(lines * 17.0 * zoom)))
                .flex()
                .items_center()
                .px(px(3.0 * zoom))
                .bg(gpui::white())
                .border_2()
                .border_color(gpui::rgb(0x217346))
                .text_color(gpui::black())
                .text_size(px(14.0 * zoom))
                .cursor_text()
                .child(caret_line(&edit.text, gpui::black(), true))
                // A click in the cell being typed into stays in it, in Edit
                // mode, rather than landing on the grid under it.
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|view, _event, _window, cx| {
                        if let Some(edit) = view.edit.as_mut() {
                            edit.in_place = true;
                            edit.point = None;
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }),
                ),
        )
    }

    /// The context menu over the grid, or over a sheet tab.
    fn context_menu(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let menu = self.menu.as_ref()?;
        let items: Vec<(MenuCommand, &'static str)> = match menu.tab {
            Some(index) => vec![
                (MenuCommand::InsertSheet, "sheet.menu.insert_sheet"),
                (MenuCommand::RenameSheet(index), "sheet.menu.rename_sheet"),
                (MenuCommand::DeleteSheet(index), "sheet.menu.delete_sheet"),
            ],
            None => {
                let mut items = vec![
                    (MenuCommand::Cut, "sheet.menu.cut"),
                    (MenuCommand::Copy, "sheet.menu.copy"),
                ];
                if self.copied.is_some() {
                    items.push((MenuCommand::Paste, "sheet.menu.paste"));
                }
                items.extend([
                    (MenuCommand::Clear, "sheet.menu.clear"),
                    (MenuCommand::FillDown, "sheet.menu.fill_down"),
                    (MenuCommand::FillRight, "sheet.menu.fill_right"),
                    (MenuCommand::SortAscending, "sheet.menu.sort_ascending"),
                    (MenuCommand::SortDescending, "sheet.menu.sort_descending"),
                    (MenuCommand::MergeCenter, "sheet.menu.merge_center"),
                ]);
                if !self.selection.is_whole_columns() {
                    items.extend([
                        (MenuCommand::InsertRows, "sheet.menu.insert_rows"),
                        (MenuCommand::DeleteRows, "sheet.menu.delete_rows"),
                    ]);
                }
                if !self.selection.is_whole_rows() {
                    items.extend([
                        (MenuCommand::InsertColumns, "sheet.menu.insert_columns"),
                        (MenuCommand::DeleteColumns, "sheet.menu.delete_columns"),
                    ]);
                }
                items
            }
        };
        Some(
            div()
                .id("sheet-menu")
                .absolute()
                .left(px(menu.at.0))
                .top(px(menu.at.1.max(0.0)))
                .min_w(px(180.0))
                .py_1()
                .rounded_md()
                .border_1()
                .border_color(theme::border())
                .bg(theme::panel())
                .shadow_lg()
                .text_xs()
                .text_color(theme::text())
                .occlude()
                .children(
                    items
                        .into_iter()
                        .enumerate()
                        .map(|(index, (command, label))| {
                            div()
                                .id(("sheet-menu-item", index))
                                .px_3()
                                .py_1()
                                .cursor_pointer()
                                .hover(|item| item.bg(theme::hovered()))
                                .child(i18n::text(label))
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(cx.listener(move |view, _event, window, cx| {
                                    window.focus(&view.focus);
                                    view.menu_command(command, false, cx);
                                }))
                        }),
                ),
        )
    }

    /// The sheet tabs and Excel's ⊕ after them.
    fn sheet_tabs(&self, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let current = self.book.sheet_index();
        let renaming = self.renaming.as_ref().map(|(index, text)| (*index, text));
        let mut tabs: Vec<gpui::AnyElement> = self
            .sheet_names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let selected = index == current;
                let label = match renaming {
                    Some((renamed, text)) if renamed == index => {
                        caret_line(text, theme::accent().into(), false)
                    }
                    _ => div().child(name.clone()),
                };
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
                    .child(label)
                    // A double-click renames, as in Excel.
                    .on_click(cx.listener(move |view, event: &ClickEvent, window, cx| {
                        window.focus(&view.focus);
                        view.menu = None;
                        if event.click_count() >= 2 {
                            view.menu_command(MenuCommand::RenameSheet(index), false, cx);
                        } else {
                            view.select_sheet(index, cx);
                        }
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |view, event: &MouseDownEvent, window, cx| {
                            window.focus(&view.focus);
                            view.select_sheet(index, cx);
                            let origin = view
                                .grid_bounds
                                .get()
                                .map(|bounds| bounds.origin)
                                .unwrap_or_default();
                            // Above the tab, where the menu has room.
                            view.menu = Some(SheetMenu {
                                at: (
                                    f32::from(event.position.x - origin.x),
                                    f32::from(event.position.y - origin.y) - 100.0,
                                ),
                                tab: Some(index),
                            });
                            cx.notify();
                        }),
                    )
                    .into_any_element()
            })
            .collect();
        tabs.push(
            div()
                .id("sheet-add")
                .h_full()
                .flex()
                .items_center()
                .px_3()
                .text_sm()
                .cursor_pointer()
                .text_color(theme::text_dim())
                .hover(|tab| tab.bg(theme::hovered()).text_color(theme::text()))
                .child("+")
                .on_click(cx.listener(|view, _event, window, cx| {
                    window.focus(&view.focus);
                    if view.commit_cell(cx) {
                        view.menu_command(MenuCommand::InsertSheet, false, cx);
                    }
                }))
                .into_any_element(),
        );
        tabs
    }
}

/// Text being typed, with its caret drawn where it is and its lines kept.
fn caret_line(text: &TextEdit, caret_color: gpui::Hsla, tall: bool) -> gpui::Div {
    if text.whole {
        let mut selected = caret_color;
        selected.a = 0.3;
        return div().flex().child(
            div()
                .whitespace_nowrap()
                .bg(selected)
                .child(text.text.clone()),
        );
    }
    let (before, after) = text.split();
    let height = if tall { 15.0 } else { 12.0 };
    let line = |text: &str| div().whitespace_nowrap().child(text.to_string());
    let mut caret = Some(div().flex_none().w(px(1.0)).h(px(height)).bg(caret_color));
    let mut lines = div().flex().flex_col();
    let before_lines: Vec<&str> = before.split('\n').collect();
    let after_lines: Vec<&str> = after.split('\n').collect();
    for (index, part) in before_lines.iter().enumerate() {
        if index + 1 < before_lines.len() {
            lines = lines.child(line(part));
            continue;
        }
        let mut row = div().flex().items_center().child(line(part));
        if let Some(caret) = caret.take() {
            row = row.child(caret);
        }
        lines = lines.child(row.child(line(after_lines[0])));
    }
    for part in after_lines.iter().skip(1) {
        lines = lines.child(line(part));
    }
    lines
}

impl Editor for WorkbookView {
    fn status_items(&self) -> Vec<SharedString> {
        let range = self.range();
        let mut items: Vec<SharedString> = vec![
            i18n::format("sheet.position", &[("name", &self.book.sheet_name())]).into(),
            range_label(range).into(),
        ];
        // Excel's summary of a multi-cell selection: average, count, sum.
        if range.start != range.end {
            let summary = self.book.summary(range);
            if let (Some(sum), Some(average)) = (summary.sum, summary.average) {
                items.push(
                    i18n::format("sheet.average", &[("value", &format_number(average))]).into(),
                );
                items.push(i18n::format("sheet.cell_count", &[("n", &summary.count)]).into());
                items.push(i18n::format("sheet.sum", &[("value", &format_number(sum))]).into());
            } else if summary.count > 0 {
                items.push(i18n::format("sheet.cell_count", &[("n", &summary.count)]).into());
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
        self.book.is_dirty()
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
        let text = self.typed().map(|edit| edit.text.as_str()).unwrap_or("");
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
        if let Some(edit) = self.typed().filter(|edit| edit.whole) {
            return Some(UTF16Selection {
                range: 0..byte_to_utf16(&edit.text, edit.text.len()),
                reversed: false,
            });
        }
        let caret = self
            .typed()
            .map(|edit| byte_to_utf16(&edit.text, edit.caret()))
            .unwrap_or(0);
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
        let edit = self.typed()?;
        let marked = edit.marked.clone()?;
        Some(byte_to_utf16(&edit.text, marked.start)..byte_to_utf16(&edit.text, marked.end))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        if let Some(edit) = self.typing() {
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
        if self.typed().is_none() {
            self.begin_edit(Some(String::new()), false, cx);
        }
        if let Some(edit) = self.typing() {
            edit.replace(range, text, false);
        }
        self.typed_into();
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
        if self.typed().is_none() {
            self.begin_edit(Some(String::new()), false, cx);
        }
        if let Some(edit) = self.typing() {
            edit.replace(range, new_text, true);
        }
        self.typed_into();
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        // The candidate window goes under the active cell, or at the grid's
        // corner for the name box and a tab's name.
        if self.edit.is_some() {
            if let Some(cell) = self.cell_bounds(self.active()) {
                return Some(cell);
            }
        }
        Some(Bounds {
            origin: element_bounds.origin,
            size: size(px(1.0), px(20.0)),
        })
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

        let (sheet_width, sheet_height) = self.book.sheet_size();
        let (sheet_width, sheet_height) = (
            sheet_width as f32 * self.zoom,
            sheet_height as f32 * self.zoom,
        );
        let tabs = self.sheet_tabs(cx);
        let grid_store = Rc::clone(&self.grid_bounds);
        let entity = cx.entity();
        let editor = self
            .grid_bounds
            .get()
            .and_then(|grid| self.cell_editor(grid, cx));
        let menu = self.context_menu(cx);
        let note = self.note.and_then(|(cell, (x, y))| {
            let text = self.book.note(cell)?.to_string();
            Some(
                div()
                    .absolute()
                    .left(px(x + 6.0))
                    .top(px(y))
                    .max_w(px(240.0))
                    .p_2()
                    .bg(gpui::rgb(0xfff9c4))
                    .border_1()
                    .border_color(gpui::rgb(0x9e9e9e))
                    .shadow_md()
                    .text_xs()
                    .text_color(gpui::black())
                    .child(text),
            )
        });
        let find_bar = self.finding.as_ref().map(|text| {
            div()
                .absolute()
                .top_2()
                .right_4()
                .w(px(260.0))
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .py_1()
                .rounded_md()
                .border_1()
                .border_color(theme::accent())
                .bg(theme::panel())
                .shadow_lg()
                .text_xs()
                .text_color(theme::text())
                .child(
                    div()
                        .text_color(theme::text_dim())
                        .child(i18n::text("sheet.find")),
                )
                .child(
                    div()
                        .flex_1()
                        .child(caret_line(text, theme::accent().into(), false)),
                )
        });
        let cursor = match self.hover {
            Hover::ResizeColumn => gpui::CursorStyle::ResizeLeftRight,
            Hover::ResizeRow => gpui::CursorStyle::ResizeUpDown,
            Hover::FillHandle | Hover::Cells => gpui::CursorStyle::Crosshair,
        };
        let dragging = self.drag.is_some();

        div()
            .key_context("Workbook")
            .font_family(UI_FONT_FAMILY)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::handle_key))
            .on_action(cx.listener(|view, _: &Undo, _window, cx| view.step_history(true, cx)))
            .on_action(cx.listener(|view, _: &Redo, _window, cx| view.step_history(false, cx)))
            .on_action(cx.listener(|view, _: &Copy, _window, cx| view.copy(false, cx)))
            .on_action(cx.listener(|view, _: &Cut, _window, cx| view.copy(true, cx)))
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
                            .cursor(cursor)
                            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
                            .on_mouse_down(MouseButton::Right, cx.listener(Self::right_click))
                            .on_mouse_move(cx.listener(
                                |view, event: &MouseMoveEvent, _window, cx| {
                                    // A release the window missed — a click
                                    // quicker than a frame — still ends the
                                    // drag.
                                    if !event.dragging() {
                                        view.drag_end(cx);
                                    }
                                    view.hover_at(event.position, cx)
                                },
                            ))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _event: &MouseUpEvent, _window, cx| {
                                    view.drag_end(cx)
                                }),
                            )
                            .on_mouse_up_out(
                                MouseButton::Left,
                                cx.listener(|view, _event: &MouseUpEvent, _window, cx| {
                                    view.drag_end(cx)
                                }),
                            )
                            .child(div().w(px(sheet_width)).h(px(sheet_height)).flex_none()),
                    )
                    .children(editor)
                    .children(note)
                    .children(find_bar)
                    .children(menu)
                    // Records where the grid is, registers the input handler
                    // so typing — an IME's included — reaches the cell, and
                    // while a drag is on follows the pointer anywhere in the
                    // window, so a drag past the grid's edge keeps going.
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
                                if dragging {
                                    let moving = entity.clone();
                                    window.on_mouse_event(
                                        move |event: &MouseMoveEvent, phase, _window, cx| {
                                            if phase == DispatchPhase::Bubble && event.dragging() {
                                                moving.update(cx, |view, cx| {
                                                    view.drag_to(event.position, cx)
                                                });
                                            }
                                        },
                                    );
                                    let releasing = entity.clone();
                                    window.on_mouse_event(
                                        move |event: &MouseUpEvent, phase, _window, cx| {
                                            if phase == DispatchPhase::Bubble
                                                && event.button == MouseButton::Left
                                            {
                                                releasing.update(cx, |view, cx| view.drag_end(cx));
                                            }
                                        },
                                    );
                                }
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
                    .id("sheet-tabs")
                    .h(px(SHEET_TABS_HEIGHT))
                    .flex_none()
                    .flex()
                    .items_center()
                    .overflow_x_scroll()
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
        let book = Workbook::open(&bytes, &system_time_zone(), crate::skia_backend()?)?;
        let sheet_names = book
            .sheet_names()
            .into_iter()
            .map(SharedString::from)
            .collect();
        Ok(WorkbookView {
            path: path.to_path_buf(),
            book,
            sheet_names,
            selection: Selection::default(),
            edit: None,
            drag: None,
            pointer: None,
            auto_scrolling: false,
            copied: None,
            tab_start: None,
            renaming: None,
            naming: None,
            finding: None,
            menu: None,
            hover: Hover::Cells,
            note: None,
            generation: 0,
            image: None,
            retired: Vec::new(),
            painted: None,
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
        // The app's keymap, so ⌘C, ⌘V and the rest arrive as they do in
        // the app: as actions where it binds them, as keys where it does not.
        cx.update(|cx| cx.bind_keys(crate::key_bindings()));
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
                view.edit.as_ref().map(|edit| edit.text.text.as_str()),
                Some("123")
            );
        });
        cx.simulate_keystrokes("enter");
        view.read_with(cx, |view, _| {
            assert!(view.edit.is_none());
            assert_eq!(view.book.edit_text(CellRef::new(1, 2)), "123");
            assert_eq!(view.active(), CellRef::new(1, 3));
            assert!(view.book.is_dirty());
        });
    }

    /// ⌘S saves over the file that was opened, as Excel does — a macro
    /// workbook stays one — leaving nothing else beside it.
    #[gpui::test]
    fn save_writes_the_file_that_was_opened(cx: &mut gpui::TestAppContext) {
        let path =
            crate::shell::scratch_copy(&fixture("grid-operations-workbook.xlsx"), "book.xlsm");
        let (view, cx) = cx.add_window_view(|_window, cx| WorkbookView::open(&path, cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        cx.simulate_keystrokes("down down right");
        cx.simulate_input("4242");
        cx.simulate_keystrokes("enter");
        cx.dispatch_action(Save);
        view.read_with(cx, |view, _| assert!(!view.book.is_dirty()));
        assert_eq!(crate::shell::siblings(&path), ["book.xlsm"]);
        let reopened = std::fs::read(&path).unwrap();
        let book = jade_office_model::sheet::Workbook::open(
            &reopened,
            "UTC",
            crate::skia_backend().unwrap(),
        )
        .unwrap();
        assert_eq!(book.edit_text(CellRef::new(1, 2)), "4242");
    }

    /// A formula typed into a cell calculates, and Undo takes it back.
    #[gpui::test]
    fn a_typed_formula_calculates_and_undoes(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("right right right right down");
        cx.simulate_input("=B2*10");
        cx.simulate_keystrokes("enter");
        view.read_with(cx, |view, _| {
            assert_eq!(view.book.display_text(CellRef::new(4, 1)), "100");
        });
        cx.dispatch_action(Undo);
        view.read_with(cx, |view, _| {
            assert_eq!(view.book.edit_text(CellRef::new(4, 1)), "");
        });
        cx.dispatch_action(Redo);
        view.read_with(cx, |view, _| {
            assert_eq!(view.book.edit_text(CellRef::new(4, 1)), "=B2*10");
        });
    }

    /// Escape abandons an edit; Delete clears the selection.
    #[gpui::test]
    fn escape_cancels_and_delete_clears(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("down");
        let before = view.read_with(cx, |view, _| view.book.edit_text(CellRef::new(0, 1)));
        assert!(!before.is_empty());
        cx.simulate_input("xyz");
        cx.simulate_keystrokes("escape");
        view.read_with(cx, |view, _| {
            assert!(view.edit.is_none());
            assert_eq!(view.book.edit_text(CellRef::new(0, 1)), before);
        });
        cx.simulate_keystrokes("shift-down delete");
        view.read_with(cx, |view, _| {
            assert_eq!(view.book.edit_text(CellRef::new(0, 1)), "");
            assert_eq!(view.book.edit_text(CellRef::new(0, 2)), "");
        });
    }

    fn at(column: u32, row: u32) -> CellRef {
        CellRef::new(column, row)
    }

    /// The fixture's numbers are B2:C4 (10 20 / 20 40 / 30 60) under a
    /// header row; B2 is reached with ↓→.
    fn edit_text(
        view: &Entity<WorkbookView>,
        cx: &mut gpui::VisualTestContext,
        cell: CellRef,
    ) -> String {
        view.read_with(cx, |view, _| view.book.edit_text(cell))
    }

    /// Shift-arrows grow the range from the active cell; ⌘⇧↓ reaches the
    /// end of the data; ⌘↓ alone moves the active cell there.
    #[gpui::test]
    fn shift_and_command_arrows_select_as_excel_does(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("down right shift-right");
        view.read_with(cx, |view, _| {
            assert_eq!(view.range(), span(at(1, 1), at(2, 1)));
            assert_eq!(view.active(), at(1, 1));
        });
        cx.simulate_keystrokes("cmd-shift-down");
        view.read_with(cx, |view, _| {
            assert_eq!(view.range(), span(at(1, 1), at(2, 3)));
            assert_eq!(view.active(), at(1, 1));
        });
        cx.simulate_keystrokes("cmd-down");
        view.read_with(cx, |view, _| {
            assert!(view.selection.is_cell());
            assert_eq!(view.active(), at(1, 3));
        });
        cx.simulate_keystrokes("cmd-home");
        view.read_with(cx, |view, _| assert_eq!(view.active(), at(0, 0)));
    }

    /// Enter walks the active cell down a range's columns and wraps, the
    /// range staying selected; after Tabs, Enter goes back to the column
    /// the Tabs started from.
    #[gpui::test]
    fn enter_walks_inside_a_range_and_returns_after_tabs(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("down right shift-right shift-down");
        let walked: Vec<CellRef> = (0..4)
            .map(|_| {
                cx.simulate_keystrokes("enter");
                view.read_with(cx, |view, _| view.active())
            })
            .collect();
        assert_eq!(walked, [at(1, 2), at(2, 1), at(2, 2), at(1, 1)]);
        view.read_with(cx, |view, _| {
            assert_eq!(view.range(), span(at(1, 1), at(2, 2)))
        });
        cx.simulate_keystrokes("right tab tab enter");
        view.read_with(cx, |view, _| assert_eq!(view.active(), at(2, 2)));
    }

    /// Typing then an arrow writes the cell and leaves it that way —
    /// Excel's Enter mode.
    #[gpui::test]
    fn an_arrow_after_typing_writes_and_leaves_the_cell(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("down right right right right right");
        cx.simulate_input("5");
        cx.simulate_keystrokes("right");
        view.read_with(cx, |view, _| {
            assert!(view.edit.is_none());
            assert_eq!(view.book.edit_text(at(5, 1)), "5");
            assert_eq!(view.active(), at(6, 1));
        });
    }

    /// F2 edits in place: the arrows move the caret in the text, and
    /// typing goes in at the caret.
    #[gpui::test]
    fn f2_arrows_move_the_caret(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("down right f2 left");
        view.read_with(cx, |view, _| {
            let edit = view.edit.as_ref().unwrap();
            assert_eq!(edit.text.text, "10");
            assert_eq!(edit.text.caret(), 1);
            assert_eq!(view.active(), at(1, 1));
        });
        cx.simulate_input("5");
        cx.simulate_keystrokes("enter");
        view.read_with(cx, |view, _| {
            assert_eq!(view.book.edit_text(at(1, 1)), "150");
            assert_eq!(view.active(), at(1, 2));
        });
    }

    /// After `=` the arrows point at a cell — Excel's Point mode — and the
    /// reference goes into the formula.
    #[gpui::test]
    fn arrows_point_a_formula_at_cells(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("down right right right right");
        cx.simulate_input("=");
        cx.simulate_keystrokes("left left left");
        view.read_with(cx, |view, _| {
            assert_eq!(view.edit.as_ref().unwrap().text.text, "=B2");
            assert_eq!(view.overlay().preview, Some(span(at(1, 1), at(1, 1))));
            assert_eq!(view.active(), at(4, 1), "the cell typed into stays active");
        });
        cx.simulate_keystrokes("shift-down");
        view.read_with(cx, |view, _| {
            assert_eq!(view.edit.as_ref().unwrap().text.text, "=B2:B3")
        });
        cx.simulate_keystrokes("down");
        cx.simulate_input("*10");
        cx.simulate_keystrokes("enter");
        view.read_with(cx, |view, _| {
            assert_eq!(view.book.edit_text(at(4, 1)), "=B3*10");
            assert_eq!(view.book.display_text(at(4, 1)), "200");
        });
    }

    /// ⌘D copies the top row down the range, the formula moving with it.
    #[gpui::test]
    fn command_d_fills_down(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("down right right right right");
        cx.simulate_input("=B2*10");
        cx.simulate_keystrokes("enter up shift-down shift-down cmd-d");
        view.read_with(cx, |view, _| {
            assert_eq!(view.book.edit_text(at(4, 2)), "=B3*10");
            assert_eq!(view.book.display_text(at(4, 3)), "300");
        });
        cx.dispatch_action(Undo);
        assert_eq!(edit_text(&view, cx, at(4, 2)), "");
    }

    /// ⌘C then ⌘V elsewhere pastes the formula moved, as Excel's paste
    /// does; the copy stays outlined for another paste.
    #[gpui::test]
    fn copy_and_paste_move_a_formula(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("down right right right right");
        cx.simulate_input("=B2*10");
        cx.simulate_keystrokes("enter up cmd-c");
        view.read_with(cx, |view, _| {
            assert_eq!(view.overlay().marquee, Some(span(at(4, 1), at(4, 1))))
        });
        cx.simulate_keystrokes("down right cmd-v");
        view.read_with(cx, |view, _| {
            assert_eq!(view.book.edit_text(at(5, 2)), "=C3*10");
            assert_eq!(view.book.display_text(at(5, 2)), "400");
            assert!(view.copied.is_some());
        });
        cx.simulate_keystrokes("escape");
        view.read_with(cx, |view, _| assert!(view.copied.is_none()));
    }

    /// ⌘X then ⌘V moves the cells: they leave where they were.
    #[gpui::test]
    fn cut_and_paste_move_cells(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("down right shift-right cmd-x");
        assert_eq!(edit_text(&view, cx, at(1, 1)), "10", "a cut moves on paste");
        cx.simulate_keystrokes("down down down right right right right right right cmd-v");
        view.read_with(cx, |view, _| {
            assert_eq!(view.book.edit_text(at(7, 4)), "10");
            // C2 was `=B2*2`: a cut moves it, its reference following B2.
            assert_eq!(view.book.edit_text(at(8, 4)), "=H5*2");
            assert_eq!(view.book.display_text(at(8, 4)), "20");
            assert_eq!(view.book.edit_text(at(1, 1)), "");
            assert_eq!(view.book.edit_text(at(2, 1)), "");
            assert!(view.copied.is_none(), "a cut pastes once");
            assert_eq!(view.range(), span(at(7, 4), at(8, 4)));
        });
    }

    /// ⌘B turns bold on through the selection, and off again.
    #[gpui::test]
    fn command_b_toggles_bold(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("down right cmd-b");
        view.read_with(cx, |view, _| {
            assert_eq!(view.book.format_at(at(1, 1)).bold, Some(true))
        });
        cx.simulate_keystrokes("cmd-b");
        view.read_with(cx, |view, _| {
            assert_ne!(view.book.format_at(at(1, 1)).bold, Some(true))
        });
    }

    /// ⌘+ inserts a row above the selection; ⌃- deletes it again (⌘- is
    /// the app's zoom out).
    #[gpui::test]
    fn command_plus_inserts_rows(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("down down right");
        assert_eq!(edit_text(&view, cx, at(1, 2)), "20");
        cx.simulate_keystrokes("cmd-shift-=");
        assert_eq!(edit_text(&view, cx, at(1, 2)), "");
        assert_eq!(edit_text(&view, cx, at(1, 3)), "20");
        cx.simulate_keystrokes("ctrl--");
        assert_eq!(edit_text(&view, cx, at(1, 2)), "20");
    }

    /// AutoSum types `=SUM(` round the numbers above, pointed, and Enter
    /// takes it.
    #[gpui::test]
    fn autosum_sums_the_numbers_above(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("down down down down right cmd-shift-t");
        view.read_with(cx, |view, _| {
            assert_eq!(view.edit.as_ref().unwrap().text.text, "=SUM(B2:B4)")
        });
        cx.simulate_keystrokes("enter");
        view.read_with(cx, |view, _| {
            assert_eq!(view.book.display_text(at(1, 4)), "60")
        });
    }

    /// The middle of a cell, in window px, as the mouse reaches it.
    fn cell_center(
        view: &Entity<WorkbookView>,
        cx: &mut gpui::VisualTestContext,
        cell: CellRef,
    ) -> gpui::Point<Pixels> {
        view.read_with(cx, |view, _| {
            let bounds = view.cell_bounds(cell).unwrap();
            bounds.center()
        })
    }

    /// A press on a cell and a drag to another selects the range between,
    /// the name box showing its size while the button is held; Shift-click
    /// moves the far corner.
    #[gpui::test]
    fn dragging_selects_a_range(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        let from = cell_center(&view, cx, at(1, 1));
        let to = cell_center(&view, cx, at(2, 3));
        cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(to, Some(MouseButton::Left), Modifiers::none());
        view.read_with(cx, |view, _| {
            assert!(view.drag.is_some());
            assert_eq!(view.range(), span(at(1, 1), at(2, 3)));
        });
        cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
        view.read_with(cx, |view, _| {
            assert!(view.drag.is_none());
            assert_eq!(view.active(), at(1, 1));
            assert_eq!(view.range(), span(at(1, 1), at(2, 3)));
        });
        let corner = cell_center(&view, cx, at(3, 1));
        cx.simulate_click(corner, Modifiers::shift());
        view.read_with(cx, |view, _| {
            assert_eq!(view.range(), span(at(1, 1), at(3, 1)));
            assert_eq!(view.active(), at(1, 1));
        });
    }
}

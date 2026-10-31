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
    canvas, div, img, point, prelude::*, px, size, svg, App, Bounds, ClickEvent, ClipboardItem,
    Context, DispatchPhase, ElementInputHandler, Entity, EntityInputHandler, FocusHandle,
    Focusable, KeyDownEvent, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    Pixels, RenderImage, ScrollHandle, SharedString, UTF16Selection, Window,
};
use jade_office_model::csv;
use jade_office_model::find::FindBar;
use jade_office_model::formula_complete;
use jade_office_model::grid::{self, Direction, DragKind, Hit, Selection};
use jade_office_model::links::LinkDialog;
use jade_office_model::number_formats::NUMBER_FORMATS;
use jade_office_model::paste_special::PasteSpecial;
use jade_office_model::pivots::PivotPanel;
use jade_office_model::print;
use jade_office_model::protected_edits::{attempt, ProtectedEdit};
use jade_office_model::protection::ProtectionDialog;
use jade_office_model::sheet::{
    column_label, format_number, range_text, reference_text, span, AutoFilterOption, Axis, Borders,
    CellFormat, CellRange, CellRef, FilterButton, FontToggle, Freeze, HorizontalAlignment, Link,
    OutlineChange, Overlay, ResizeGesture, SheetVisibility, Workbook, MAX_COLUMN, MAX_ROW,
};
use jade_office_model::skia::SkiaBackend;
use jade_office_model::tables::TableDialog;
use jade_office_model::text::TextEdit;
use jade_office_model::validation::{PendingValidation, ValidationDialog};
use jade_office_model::Backend;
use std::cell::Cell;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use crate::editor::{self, Editor};

mod charts;
mod conditional;
mod duplicates;
mod find_bar;
mod links;
mod page_setup;
mod paste_special;
mod pivots;
mod protection;
mod sort;
mod subtotal;
mod tables;
mod text_columns;
mod validation;
use crate::{
    byte_to_utf16, i18n, theme, utf16_to_byte, workspace, Copy, Cut, Paste, Print, Redo, Save,
    SaveAs, SelectAll, Undo, UI_FONT_FAMILY,
};
use charts::ChartTool;
use conditional::Conditional;
use jade_office_model::charts::{ChartAsk, Handle};
use jade_office_model::conditional_formatting::RuleAsk;

mod format_dialog;

/// The formatting strip over the formula bar: Excel's Home tab, short.
const SHEET_TOOLBAR_HEIGHT: f32 = 34.0;
/// The name box and formula bar above the grid.
const FORMULA_BAR_HEIGHT: f32 = 32.0;
/// The strip of sheet tabs under the grid.
const SHEET_TABS_HEIGHT: f32 = 30.0;
/// How often a drag held past the grid's edge scrolls it.
const AUTO_SCROLL_TICK: Duration = Duration::from_millis(40);
/// Excel's row of standard colours, under its theme colours in the fill and
/// font colour dropdowns.
const STANDARD_COLORS: &[&str] = &[
    "C00000", "FF0000", "FFC000", "FFFF00", "92D050", "00B050", "00B0F0", "0070C0", "002060",
    "7030A0", "000000", "7F7F7F", "D9D9D9", "FFFFFF",
];

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
    /// The function AutoComplete's list has picked with the arrows, as the
    /// word's start, the caret and the name's place: typing on starts
    /// again from the first.
    suggested: Option<(usize, usize, usize)>,
}

/// The name AutoComplete's list has picked: the one the arrows moved to
/// while the word stays as it was, else the first.
fn picked(edit: &CellEdit, list: &formula_complete::Completion) -> usize {
    match edit.suggested {
        Some((start, caret, at)) if start == list.start && caret == edit.text.caret => {
            at.min(list.names.len() - 1)
        }
        _ => 0,
    }
}

/// Function AutoComplete's list under the cell being typed into, the name
/// picked marked; a click puts one in.
fn completion_list(
    edit: &CellEdit,
    below: Pixels,
    cx: &mut Context<WorkbookView>,
) -> Option<gpui::Div> {
    let list = formula_complete::completion(&edit.text.text, edit.text.caret)?;
    let chosen = picked(edit, &list);
    let start = list.start;
    Some(
        div()
            .absolute()
            .left(px(-2.0))
            .top(below)
            .min_w(px(160.0))
            .py_1()
            .rounded_sm()
            .border_1()
            .border_color(theme::border())
            .bg(theme::panel())
            .shadow_md()
            .text_xs()
            .text_color(theme::text())
            .children(list.names.iter().enumerate().map(|(index, name)| {
                let name: &'static str = name;
                div()
                    .id(("formula-complete", index))
                    .px_2()
                    .cursor_pointer()
                    .when(index == chosen, |row| row.bg(theme::selected()))
                    .hover(|row| row.bg(theme::hovered()))
                    .child(name)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, _event, window, cx| {
                            window.focus(&view.focus);
                            if let Some(edit) = view.edit.as_mut() {
                                if let Some(list) =
                                    formula_complete::completion(&edit.text.text, edit.text.caret)
                                        .filter(|list| list.start == start)
                                {
                                    formula_complete::accept(&mut edit.text, &list, name);
                                    edit.point = None;
                                    edit.suggested = None;
                                }
                            }
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
            })),
    )
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
    /// The selection picked up by its border: where the pointer took it
    /// (offset from its top-left), where it would land, and whether ⌥
    /// copies rather than moves.
    Move {
        source: CellRange,
        grab: (i64, i64),
        target: CellRange,
        copy: bool,
    },
    /// A chart, picture or shape picked up by its border (no handle) or a
    /// handle: where the press started, where the object was, and where
    /// the drag has it (sheet px).
    Object {
        id: u32,
        handle: Option<Handle>,
        start: (f64, f64),
        bounds: (f64, f64, f64, f64),
        now: (f64, f64, f64, f64),
    },
}

/// What the pointer is over, for its cursor.
#[derive(Clone, Copy, PartialEq)]
enum Hover {
    Cells,
    FillHandle,
    ResizeColumn,
    ResizeRow,
    /// A cell with a hyperlink, which a click follows.
    Link,
    /// The selection's border, which a drag picks the cells up by.
    Border,
}

/// The context menu.
struct SheetMenu {
    /// Where it opened, in the grid's window px.
    at: (f32, f32),
    /// Opened on a sheet tab: its index.
    tab: Option<usize>,
}

/// The dropdown a filter button opens: sorts, a clear, and the column's
/// values to tick, as Excel's AutoFilter menu.
struct FilterPanel {
    button: FilterButton,
    options: Vec<AutoFilterOption>,
    checked: Vec<bool>,
    /// Under the button, in the grid's window px.
    at: (f32, f32),
}

#[derive(Clone, Copy)]
enum FilterCommand {
    Toggle(usize),
    All,
    Apply,
    Cancel,
    Sort(bool),
    Clear,
    /// Top 10 (true) or Bottom 10.
    Top(bool),
    Reapply,
}

/// A toolbar button's dropdown.
#[derive(Clone, Copy, PartialEq)]
enum Popup {
    NumberFormat,
    Fill,
    Font,
    Borders,
    Conditional,
    Chart,
}

/// What a toolbar button does to the selection.
#[derive(Clone, Copy)]
enum Tool {
    Font(FontToggle),
    Align(HorizontalAlignment),
    Wrap,
    Merge,
    Currency,
    Percent,
    Comma,
    /// A Number Format dropdown entry: the i18n key of its code.
    NumberFormat(&'static str),
    /// More decimals (true) or fewer.
    Decimals(bool),
    /// A size larger (true) or smaller.
    FontSize(bool),
    Open(Popup),
    /// `RRGGBB`, or no fill.
    Fill(Option<&'static str>),
    FontColor(&'static str),
    Borders(Borders),
    Filter,
    Conditional(Conditional),
    Chart(ChartTool),
}

#[derive(Clone, Copy)]
enum MenuCommand {
    FormatCells,
    Validation,
    ClearValidation,
    Link,
    RemoveLinks,
    Table,
    RefreshPivot,
    EditPivot,
    CreatePivot,
    DeletePivot,
    RefreshAllPivots,
    Cut,
    Copy,
    Paste,
    PasteSpecial,
    Clear,
    ClearFormats,
    ClearAll,
    InsertRows,
    DeleteRows,
    InsertColumns,
    DeleteColumns,
    ShiftCells(Axis, bool),
    FillDown,
    FillRight,
    SortAscending,
    SortDescending,
    MergeCenter,
    InsertSheet,
    ProtectWorkbook,
    ProtectSheet(usize),
    EditRanges(usize),
    CellLocked,
    CellHidden,
    RenameSheet(usize),
    DeleteSheet(usize),
    HideSheet(usize),
    UnhideSheet(usize),
    /// Tab Color: an `RRGGBB` for the sheet's tab, or none.
    TabColor(usize, Option<&'static str>),
    /// A sheet tab from one place to another.
    MoveSheet(usize, usize),
    /// New Note, or Edit Note on a cell that has one.
    EditNote,
    DeleteNote,
    /// The selected whole rows or columns.
    Hide,
    Unhide,
    /// The rows above and the columns left of the active cell.
    FreezePanes,
    UnfreezePanes,
    /// Data ▸ Group and Outline on the selected whole rows or columns.
    Outline(OutlineChange),
    /// The selection's formats picked up for the next selection.
    FormatPainter,
    /// Data ▸ Group ▸ Auto Outline.
    AutoOutline,
    /// Data ▸ Ungroup ▸ Clear Outline on the selected rows' or columns' axis.
    ClearOutline,
    /// Data ▸ Subtotal over the selection with the dialog's defaults.
    Subtotal,
    /// Data ▸ Remove Duplicates over the selection or the list round it.
    RemoveDuplicates,
    /// Data ▸ Text to Columns on the selection's column.
    TextToColumns,
    SortDialog,
    PageSetup,
    /// Formulas ▸ Calculation Options: manual (true) or automatic.
    ManualCalculation(bool),
    /// Formulas ▸ Calculate Now.
    CalculateNow,
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
    /// The column Tab started from, where Enter goes back to.
    tab_start: Option<u32>,
    /// A sheet tab being renamed, with what has been typed.
    renaming: Option<(usize, TextEdit)>,
    /// The name box being typed into: a cell or range to go to.
    naming: Option<TextEdit>,
    /// The name box was opened by Go To (F5, ⌃G): what is typed is only
    /// gone to, never defined as a name.
    going_to: bool,
    /// Find and Replace (⌘F, ⌃H): what is typed into it, and its options.
    finding: Option<FindBar>,
    /// A note being written on a cell.
    noting: Option<(CellRef, TextEdit)>,
    /// A conditional format's dialog, asking for its value.
    asking: Option<RuleAsk>,
    /// The conditional formatting Rules Manager, open.
    managing: bool,
    /// The selected chart, picture or shape, by id.
    object: Option<u32>,
    /// Chart Title… or Select Data…, asking.
    chart_asking: Option<ChartAsk>,
    /// The Paste Special dialog, open, with its choices.
    pasting: Option<PasteSpecial>,
    validation_dialog: Option<ValidationDialog>,
    link_dialog: Option<LinkDialog>,
    subtotal_dialog: Option<jade_office_model::subtotal::SubtotalDialog>,
    text_columns_dialog: Option<jade_office_model::text_columns::TextColumnsDialog>,
    sort_dialog: Option<jade_office_model::sort_dialog::SortDialog>,
    page_setup_dialog: Option<jade_office_model::page_setup_dialog::PageSetupDialog>,
    duplicates_dialog: Option<jade_office_model::duplicates::DuplicatesDialog>,
    protection_dialog: Option<ProtectionDialog>,
    table_dialog: Option<TableDialog>,
    pivot_dialog: Option<PivotPanel>,
    validation_pending: Option<PendingValidation>,
    /// The active cell's list of choices (its data validation), open.
    choices: Option<Vec<String>>,
    /// A hyperlink pressed: followed when the press lets go on it, as in
    /// Excel (a press held into a drag selects instead).
    pending_link: Option<(CellRef, Link)>,
    /// Format Painter: the cells whose formats the next selection takes.
    format_source: Option<CellRange>,
    /// Whether the painter stays on for selection after selection (a
    /// double-click on its button) until Escape.
    format_sticky: bool,
    menu: Option<SheetMenu>,
    /// The Format Cells dialog (⌘1), while it is open.
    format_dialog: Option<jade_office_model::format_cells::FormatCellsDialog>,
    /// A filter button's dropdown.
    filter: Option<FilterPanel>,
    /// A toolbar button's dropdown: colours or borders.
    popup: Option<Popup>,
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
        if self.validation_pending.is_some() {
            return None;
        }
        if let Some(dialog) = self.pivot_dialog.as_mut() {
            return Some(dialog.field_mut());
        }
        if let Some(dialog) = self.table_dialog.as_mut() {
            return Some(dialog.field_mut());
        }
        if let Some(dialog) = self.protection_dialog.as_mut() {
            return Some(dialog.field_mut());
        }
        if let Some(dialog) = self.link_dialog.as_mut() {
            return Some(dialog.field_mut());
        }
        if let Some(dialog) = self.validation_dialog.as_mut() {
            return Some(dialog.field_mut());
        }
        if let Some(ask) = self.chart_asking.as_mut() {
            return Some(&mut ask.text);
        }
        if let Some(ask) = self.asking.as_mut() {
            return ask.typing();
        }
        if let Some((_, text)) = self.renaming.as_mut() {
            return Some(text);
        }
        if let Some(text) = self.naming.as_mut() {
            return Some(text);
        }
        if let Some(bar) = self.finding.as_mut() {
            return Some(bar.field_mut());
        }
        if let Some((_, text)) = self.noting.as_mut() {
            return Some(text);
        }
        self.edit.as_mut().map(|edit| &mut edit.text)
    }

    fn typed(&self) -> Option<&TextEdit> {
        if self.validation_pending.is_some() {
            return None;
        }
        if let Some(dialog) = self.pivot_dialog.as_ref() {
            return Some(dialog.field());
        }
        if let Some(dialog) = self.table_dialog.as_ref() {
            return Some(dialog.field());
        }
        if let Some(dialog) = self.protection_dialog.as_ref() {
            return Some(dialog.field());
        }
        if let Some(dialog) = self.link_dialog.as_ref() {
            return Some(dialog.field());
        }
        if let Some(dialog) = self.validation_dialog.as_ref() {
            return Some(dialog.field());
        }
        if let Some(ask) = self.chart_asking.as_ref() {
            return Some(&ask.text);
        }
        if let Some(ask) = self.asking.as_ref() {
            return ask.typed();
        }
        if let Some((_, text)) = self.renaming.as_ref() {
            return Some(text);
        }
        if let Some(text) = self.naming.as_ref() {
            return Some(text);
        }
        if let Some(bar) = self.finding.as_ref() {
            return Some(bar.field());
        }
        if let Some((_, text)) = self.noting.as_ref() {
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
            Some(SheetDrag::Fill { target, .. }) | Some(SheetDrag::Move { target, .. }) => {
                Some(target)
            }
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
                .book
                .copied()
                .filter(|(sheet, _)| *sheet == self.book.sheet_index())
                .map(|(_, range)| range),
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
        if self.validation_dialog.is_some()
            || self.link_dialog.is_some()
            || self.protection_dialog.is_some()
            || self.table_dialog.is_some()
            || self.pivot_dialog.is_some()
            || self.subtotal_dialog.is_some()
            || self.text_columns_dialog.is_some()
            || self.sort_dialog.is_some()
            || self.page_setup_dialog.is_some()
            || self.duplicates_dialog.is_some()
        {
            return;
        }
        if !self.commit_cell(cx) {
            return;
        }
        if index != self.book.sheet_index() && index < self.sheet_names.len() {
            self.object = None;
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
        if !self.book.can_select_cell(to) {
            return;
        }
        let previous_selection = self.selection;
        if extend {
            self.selection.extend_to(to);
        } else {
            self.selection = Selection::cell(to);
        }
        if !self.book.can_select_range(self.selection.range()) {
            self.selection = previous_selection;
            return;
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
            suggested: None,
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
        let previous_selection = self.selection;
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
        if !self.book.can_select_range(self.selection.range()) {
            self.selection = previous_selection;
        }
        cx.notify();
    }

    // ---- editing ---------------------------------------------------------

    fn begin_edit(&mut self, text: Option<String>, in_place: bool, cx: &mut Context<Self>) {
        let text = text.unwrap_or_else(|| self.book.edit_text(self.active()));
        self.edit = Some(CellEdit {
            text: TextEdit::new(text),
            in_place,
            point: None,
            suggested: None,
        });
        self.reveal(self.active());
        cx.notify();
    }

    /// Writes the cell being typed into, if any; the write recalculates, so
    /// dependent formulas have moved before the next paint. A formula Excel
    /// would refuse keeps the editor open and returns false.
    fn commit_cell(&mut self, cx: &mut Context<Self>) -> bool {
        if self.validation_pending.is_some()
            || self.validation_dialog.is_some()
            || self.link_dialog.is_some()
            || self.protection_dialog.is_some()
            || self.table_dialog.is_some()
            || self.pivot_dialog.is_some()
            || self.subtotal_dialog.is_some()
            || self.text_columns_dialog.is_some()
            || self.sort_dialog.is_some()
            || self.page_setup_dialog.is_some()
            || self.duplicates_dialog.is_some()
        {
            return false;
        }
        let Some(edit) = self.edit.take() else {
            return true;
        };
        let result = self.book.set_cell(self.active(), &edit.text.text);
        if result
            .as_ref()
            .is_err_and(|e| jade_office_model::sheet::validation_refusal(e).is_some())
        {
            if let Ok(Some(alert)) = self.book.validation_alert(self.active(), &edit.text.text) {
                self.validation_pending = Some(PendingValidation {
                    at: self.active(),
                    text: edit.text.text.clone(),
                    alert,
                });
            }
        }
        if result
            .as_ref()
            .is_err_and(|e| jade_office_model::sheet::protected_refusal(e))
        {
            if let Ok(Some(dialog)) =
                ProtectionDialog::for_entry(&self.book, self.active(), &edit.text.text)
            {
                self.protection_dialog = Some(dialog);
                self.menu = None;
            }
        }
        let refused = result.is_err();
        if refused {
            self.edit = Some(CellEdit {
                point: None,
                suggested: None,
                ..edit
            });
        } else if matches!(result, Ok(true)) {
            // Typing into a cell lets go of what was copied, as in Excel.
            self.book.forget_copy();
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
            // An entry a data validation refuses: the rule's words, or
            // Excel's.
            // A protected sheet: Excel's words.
            Err(error) if jade_office_model::sheet::protected_refusal(&error) => {
                self.status = SharedString::from(i18n::text("sheet.protected"))
            }
            Err(error) if jade_office_model::sheet::validation_refusal(&error).is_some() => {
                let message =
                    jade_office_model::sheet::validation_refusal(&error).unwrap_or_default();
                self.status = SharedString::from(if message.is_empty() {
                    i18n::text("sheet.validation_refused").to_string()
                } else {
                    message.to_string()
                });
            }
            Err(error) => {
                self.status =
                    SharedString::from(i18n::format("sheet.edit_failed", &[("error", &error)]))
            }
        }
        cx.notify();
    }

    fn step_history(&mut self, undo: bool, cx: &mut Context<Self>) {
        if self.validation_dialog.is_some()
            || self.validation_pending.is_some()
            || self.link_dialog.is_some()
            || self.protection_dialog.is_some()
            || self.table_dialog.is_some()
            || self.pivot_dialog.is_some()
            || self.subtotal_dialog.is_some()
            || self.text_columns_dialog.is_some()
            || self.sort_dialog.is_some()
            || self.page_setup_dialog.is_some()
            || self.duplicates_dialog.is_some()
        {
            return;
        }
        self.edit = None;
        self.renaming = None;
        self.naming = None;
        self.noting = None;
        self.choices = None;
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
        let text = self.book.copy(range, cut);
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
        if self.validation_pending.is_some() {
            return;
        }
        let multiline = self.validation_dialog.as_ref().is_some_and(|d| {
            matches!(
                d.field,
                jade_office_model::validation::ValidationField::Prompt
                    | jade_office_model::validation::ValidationField::Error
            )
        });
        let text = cx.read_from_clipboard().and_then(|item| item.text());
        if let Some(typing) = self.typing() {
            if let Some(text) = text {
                typing.insert(if multiline {
                    &text
                } else {
                    text.lines().next().unwrap_or("")
                });
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
        if self.validation_pending.is_some()
            || self.validation_dialog.is_some()
            || self.link_dialog.is_some()
            || self.protection_dialog.is_some()
            || self.table_dialog.is_some()
            || self.pivot_dialog.is_some()
            || self.subtotal_dialog.is_some()
            || self.text_columns_dialog.is_some()
            || self.sort_dialog.is_some()
            || self.page_setup_dialog.is_some()
            || self.duplicates_dialog.is_some()
        {
            return;
        }
        let target = self.range();
        // From a menu there is no clipboard text: the copy made here.
        let Some(text) = text.map(str::to_string).or_else(|| self.book.copied_text()) else {
            return;
        };
        let result = attempt(
            &mut self.book,
            &mut self.protection_dialog,
            ProtectedEdit::Paste { target, text },
        )
        .map(|outcome| outcome.selection);
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
        if self.book.structure_protected()
            && matches!(
                command,
                MenuCommand::InsertSheet
                    | MenuCommand::RenameSheet(_)
                    | MenuCommand::DeleteSheet(_)
                    | MenuCommand::MoveSheet(_, _)
                    | MenuCommand::HideSheet(_)
                    | MenuCommand::UnhideSheet(_)
                    | MenuCommand::TabColor(_, _)
            )
        {
            self.status = i18n::text("sheet.workbook_protected").into();
            cx.notify();
            return;
        }
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
            MenuCommand::EditRanges(index) => return self.open_editable_ranges(index, cx),
            MenuCommand::ProtectWorkbook => return self.open_workbook_protection(cx),
            MenuCommand::ProtectSheet(index) => return self.open_sheet_protection(index, cx),
            MenuCommand::CellLocked => {
                let locked = self.book.format_at(self.active()).locked.unwrap_or(true);
                self.book.format(
                    range,
                    &CellFormat {
                        locked: Some(!locked),
                        ..Default::default()
                    },
                )
            }
            MenuCommand::CellHidden => {
                let hidden = self
                    .book
                    .format_at(self.active())
                    .formula_hidden
                    .unwrap_or(false);
                self.book.format(
                    range,
                    &CellFormat {
                        formula_hidden: Some(!hidden),
                        ..Default::default()
                    },
                )
            }
            MenuCommand::FormatCells => return self.open_format_dialog(cx),
            MenuCommand::FormatPainter => return self.pick_up_format(false, cx),
            MenuCommand::Validation => return self.open_validation(cx),
            MenuCommand::Table => return self.open_table_dialog(cx),
            MenuCommand::EditPivot => return self.open_pivot_dialog(cx),
            MenuCommand::CreatePivot => return self.open_create_pivot(cx),
            MenuCommand::DeletePivot => self.book.delete_pivot_at(selection.active),
            MenuCommand::RefreshPivot => self.book.refresh_pivot_at(selection.active),
            MenuCommand::RefreshAllPivots => self.book.refresh_all_pivots(),
            MenuCommand::Link => return self.open_link_dialog(cx),
            MenuCommand::RemoveLinks => self.book.clear_hyperlinks(&[range]),
            MenuCommand::ClearValidation => self.book.clear_data_validations(Some(range)),
            MenuCommand::Cut => return self.sheet_copy(true, cx),
            MenuCommand::Copy => return self.sheet_copy(false, cx),
            MenuCommand::Paste => return self.sheet_paste(None, cx),
            MenuCommand::PasteSpecial => return self.open_paste_special(cx),
            MenuCommand::Clear => attempt(
                &mut self.book,
                &mut self.protection_dialog,
                ProtectedEdit::Clear { range },
            )
            .map(|outcome| outcome.changed),
            MenuCommand::ClearFormats => self.book.clear_formats(range),
            MenuCommand::ClearAll => self.book.clear_all(range),
            MenuCommand::InsertRows => self.book.insert_rows(range.start.row, rows),
            MenuCommand::DeleteRows => self.book.delete_rows(range.start.row, rows),
            MenuCommand::InsertColumns => self.book.insert_columns(range.start.column, columns),
            MenuCommand::DeleteColumns => self.book.delete_columns(range.start.column, columns),
            MenuCommand::ShiftCells(axis, inserted) => self.book.shift_cells(range, axis, inserted),
            MenuCommand::FillDown => attempt(
                &mut self.book,
                &mut self.protection_dialog,
                ProtectedEdit::FillDown { range },
            )
            .map(|outcome| outcome.changed),
            MenuCommand::FillRight => attempt(
                &mut self.book,
                &mut self.protection_dialog,
                ProtectedEdit::FillRight { range },
            )
            .map(|outcome| outcome.changed),
            MenuCommand::SortAscending => self.book.sort(range, selection.active, false),
            MenuCommand::SortDescending => self.book.sort(range, selection.active, true),
            // Merge & Center takes in every merged cell the range touches,
            // and the merge stays selected.
            MenuCommand::MergeCenter => {
                let range = self.book.with_merges(range);
                let result = self.book.merge_and_center(range);
                if self.book.can_select_range(range) {
                    self.selection = Selection::range_from(range.start, range.end);
                }
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
            MenuCommand::HideSheet(index) => {
                let result = self
                    .book
                    .set_sheet_visibility(index, SheetVisibility::Hidden);
                self.selection = Selection::default();
                self.scroll.set_offset(point(px(0.0), px(0.0)));
                result
            }
            MenuCommand::UnhideSheet(index) => self
                .book
                .set_sheet_visibility(index, SheetVisibility::Visible),
            MenuCommand::TabColor(index, color) => self.book.set_tab_color(index, color),
            MenuCommand::MoveSheet(index, to) => {
                let result = self.book.move_sheet(index, to);
                self.selection = Selection::default();
                result
            }
            // The note's editor opens beside the cell, with what it says.
            MenuCommand::EditNote => {
                let at = selection.active;
                let text = self.book.note(at).unwrap_or_default().to_string();
                self.noting = Some((at, TextEdit::new(text)));
                cx.notify();
                return;
            }
            MenuCommand::DeleteNote => self.book.set_note(selection.active, "", "jade"),
            MenuCommand::Hide | MenuCommand::Unhide => {
                let (axis, first, last) = if selection.is_whole_columns() {
                    (Axis::Columns, range.start.column, range.end.column)
                } else {
                    (Axis::Rows, range.start.row, range.end.row)
                };
                if matches!(command, MenuCommand::Hide) {
                    self.book.hide(axis, first, last)
                } else {
                    self.book.unhide(axis, first, last)
                }
            }
            MenuCommand::FreezePanes => self.book.freeze_panes(Freeze::Panes(selection.active)),
            MenuCommand::UnfreezePanes => self.book.freeze_panes(Freeze::Unfreeze),
            MenuCommand::Subtotal => return self.open_subtotal_dialog(cx),
            MenuCommand::RemoveDuplicates => return self.open_duplicates_dialog(cx),
            MenuCommand::TextToColumns => return self.open_text_columns_dialog(cx),
            MenuCommand::SortDialog => return self.open_sort_dialog(cx),
            MenuCommand::PageSetup => return self.open_page_setup_dialog(cx),
            MenuCommand::ManualCalculation(manual) => self.book.set_manual_calculation(manual),
            MenuCommand::CalculateNow => self.book.calculate_now(),
            MenuCommand::AutoOutline => self.book.auto_outline(),
            MenuCommand::ClearOutline => self.book.clear_outline(if selection.is_whole_columns() {
                Axis::Columns
            } else {
                Axis::Rows
            }),
            MenuCommand::Outline(change) => {
                let (axis, first, last) = if selection.is_whole_columns() {
                    (Axis::Columns, range.start.column, range.end.column)
                } else {
                    (Axis::Rows, range.start.row, range.end.row)
                };
                self.book.outline(axis, first, last, change)
            }
        };
        // What was copied moved or went with the deleted cells.
        if matches!(
            command,
            MenuCommand::DeleteRows | MenuCommand::DeleteColumns | MenuCommand::DeleteSheet(_)
        ) {
            self.book.forget_copy();
        }
        self.sheets_changed(result, cx);
    }

    // ---- filters ---------------------------------------------------------

    /// A press on a filter button (▼) opens its dropdown under it.
    fn open_filter(&mut self, button: FilterButton, scroll: (f64, f64), cx: &mut Context<Self>) {
        match self.book.filter_options(&button) {
            Ok(options) => {
                let checked = options
                    .options
                    .iter()
                    .map(|option| option.selected.unwrap_or(true))
                    .collect();
                let metrics = self.book.header_metrics();
                let zoom = f64::from(self.zoom);
                let left = button.rect.x + metrics.row_width - scroll.0;
                let top = button.rect.y + button.rect.height + metrics.column_height - scroll.1;
                self.filter = Some(FilterPanel {
                    button,
                    options: options.options,
                    checked,
                    at: ((left * zoom) as f32, (top * zoom) as f32),
                });
            }
            Err(error) => {
                self.status =
                    SharedString::from(i18n::format("sheet.edit_failed", &[("error", &error)]))
            }
        }
        cx.notify();
    }

    /// The filter dropdown's OK: only the ticked values show — or, all
    /// ticked, the column is not filtered.
    fn apply_filter(&mut self, cx: &mut Context<Self>) {
        let Some(panel) = self.filter.take() else {
            return;
        };
        let result = if panel.checked.iter().all(|checked| *checked) {
            self.book.clear_filter(&panel.button)
        } else {
            let mut values = Vec::new();
            let mut blank = false;
            for (option, checked) in panel.options.iter().zip(&panel.checked) {
                if !checked {
                    continue;
                }
                if option.blank {
                    blank = true;
                } else {
                    values.extend(option.values.iter().cloned());
                }
            }
            self.book.filter_values(&panel.button, values, blank)
        };
        self.after_write(result, cx);
    }

    fn filter_command(&mut self, command: FilterCommand, cx: &mut Context<Self>) {
        match command {
            FilterCommand::Toggle(index) => {
                if let Some(checked) = self
                    .filter
                    .as_mut()
                    .and_then(|panel| panel.checked.get_mut(index))
                {
                    *checked = !*checked;
                }
                cx.notify();
            }
            // (Select All): everything ticked, or — all ticked — nothing.
            FilterCommand::All => {
                if let Some(panel) = self.filter.as_mut() {
                    let all = panel.checked.iter().all(|checked| *checked);
                    panel.checked.iter_mut().for_each(|checked| *checked = !all);
                }
                cx.notify();
            }
            FilterCommand::Apply => self.apply_filter(cx),
            FilterCommand::Cancel => {
                self.filter = None;
                cx.notify();
            }
            FilterCommand::Sort(descending) => {
                if let Some(panel) = self.filter.take() {
                    let result = self.book.sort_filter(&panel.button, descending);
                    self.after_write(result, cx);
                }
            }
            FilterCommand::Clear => {
                if let Some(panel) = self.filter.take() {
                    let result = self.book.clear_filter(&panel.button);
                    self.after_write(result, cx);
                }
            }
            FilterCommand::Top(top) => {
                if let Some(panel) = self.filter.take() {
                    let result = self.book.filter_top(&panel.button, 10, top);
                    self.after_write(result, cx);
                }
            }
            FilterCommand::Reapply => {
                if let Some(panel) = self.filter.take() {
                    let result = self.book.reapply_filter(&panel.button);
                    self.after_write(result, cx);
                }
            }
        }
    }

    // ---- the toolbar -------------------------------------------------------

    /// A toolbar button pressed, on the selection.
    fn tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        if let Tool::Open(popup) = tool {
            self.popup = (self.popup != Some(popup)).then_some(popup);
            cx.notify();
            return;
        }
        self.popup = None;
        let range = self.range();
        let active = self.active();
        let result = match tool {
            Tool::Font(style) => self.book.toggle(range, active, style),
            Tool::Align(horizontal) => self.book.align(range, active, horizontal),
            Tool::Wrap => self.book.toggle_wrap(range, active),
            Tool::Merge => return self.menu_command(MenuCommand::MergeCenter, false, cx),
            Tool::Currency => {
                return self.number_format(i18n::text("sheet.format.currency"), cx);
            }
            Tool::Percent => return self.number_format("0%", cx),
            Tool::Comma => return self.number_format("#,##0", cx),
            Tool::NumberFormat(key) => return self.number_format(i18n::text(key), cx),
            Tool::Decimals(more) => self.book.decimals(range, active, more),
            Tool::FontSize(larger) => self.book.grow_font(range, active, larger),
            Tool::Fill(color) => self.book.fill_color(range, color),
            Tool::FontColor(color) => self.book.font_color(range, color),
            Tool::Borders(borders) => self.book.borders(range, borders),
            Tool::Filter => self.book.toggle_filter(range),
            Tool::Conditional(tool) => return self.conditional(tool, cx),
            Tool::Chart(tool) => return self.chart_tool(tool, cx),
            Tool::Open(_) => return,
        };
        self.after_write(result, cx);
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
        // A formula Excel would refuse stays in its editor, and the save
        // waits for it to be fixed.
        if !self.commit_cell(cx) {
            return;
        }
        // By the name's extension, as Excel's Save As by its format: a
        // .pdf exports the active sheet's pages, leaving the workbook as it
        // was; a .csv holds the active sheet; anything else, the workbook.
        let name = crate::shell::file_name(&target);
        if print::is_pdf(&name) {
            let result = self.book.pdf(&self.print_fields()).and_then(|bytes| {
                crate::shell::write_in_place(&target, &bytes).map_err(|error| error.to_string())
            });
            self.status = SharedString::from(match result {
                Ok(()) => i18n::format("sheet.exported_pdf", &[("name", &name)]),
                Err(error) => print_failure(&error),
            });
            cx.notify();
            return;
        }
        let result = self.book.save_bytes_as(&name).and_then(|bytes| {
            crate::shell::write_in_place(&target, &bytes).map_err(|error| error.to_string())
        });
        self.status = SharedString::from(match result {
            Ok(()) => {
                // Saved as, it is that file from now on.
                self.path = target;
                let key = if csv::is_csv(&name) {
                    "sheet.saved_csv"
                } else {
                    "sheet.saved"
                };
                i18n::format(key, &[("name", &name)])
            }
            Err(error) => i18n::format("sheet.save_failed", &[("error", &error)]),
        });
        cx.notify();
    }

    /// What a header's or footer's fields print: today, now, the file.
    fn print_fields(&self) -> print::PrintFields {
        print::PrintFields {
            date: now_text(false),
            time: now_text(true),
            file: crate::shell::file_name(&self.path),
        }
    }

    /// ⌘P: the active sheet's pages, as Excel prints them at its defaults,
    /// in the system's print dialog.
    fn print(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        let result = self
            .book
            .pdf(&self.print_fields())
            .and_then(|pdf| crate::native_print::show_pdf_print_dialog(&pdf));
        if let Err(error) = result {
            self.status = SharedString::from(print_failure(&error));
            cx.notify();
        }
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
        if let Some(handled) = self.protection_key(key, shift, command, cx) {
            return handled;
        }
        if self
            .validation_dialog
            .as_ref()
            .is_some_and(|d| d.field().marked.is_some())
        {
            return false;
        }
        if self.pivot_editor_marked() {
            return false;
        }
        if self.pivot_key(key, shift, command, cx) {
            return true;
        }
        if self.table_editor_marked() {
            return false;
        }
        if self.table_key(key, shift, command, cx) {
            return true;
        }
        if self.link_editor_marked() {
            return false;
        }
        if self.link_key(key, shift, command, cx) {
            return true;
        }
        if self.subtotal_key(key, cx) {
            return true;
        }
        if self.text_columns_key(key, cx) {
            return true;
        }
        if self.sort_key(key, cx) {
            return true;
        }
        if self.page_setup_key(key, cx) {
            return true;
        }
        if self.duplicates_key(key, cx) {
            return true;
        }
        if self.validation_key(key, shift, command, modifiers.alt, cx) {
            return true;
        }
        if self.menu.take().is_some() {
            cx.notify();
        }
        // The Format Cells dialog takes every key while it is open.
        if self.format_dialog.is_some() {
            let key_char = event.keystroke.key_char.clone();
            self.format_dialog_key(key, key_char.as_deref(), cx);
            return true;
        }
        // An open list of choices: Escape closes it, any other key too
        // before doing what it does.
        if self.choices.take().is_some() {
            cx.notify();
            if key == "escape" {
                return true;
            }
        }
        // An open filter dropdown: Enter applies it, Escape closes it.
        if self.filter.is_some() {
            match key {
                "escape" => self.filter = None,
                "enter" => self.apply_filter(cx),
                _ => {}
            }
            cx.notify();
            return true;
        }

        // A chart's text dialog has the keys while it is open.
        if self.chart_asking.is_some() {
            return self.chart_key(key, modifiers, cx);
        }
        // A selected chart, picture or shape: Delete removes it.
        if self.object.is_some() && self.object_key(key, cx) {
            return true;
        }
        // A conditional format's dialog has the keys while it is open.
        if self.asking.is_some() {
            return self.rule_key(key, modifiers, cx);
        }
        if self.pasting.is_some() {
            return self.paste_special_key(key, modifiers, cx);
        }
        if self.managing && key == "escape" {
            self.managing = false;
            cx.notify();
            return true;
        }

        // A tab being renamed, the name box or the find bar: Enter takes
        // it (or finds the next), Escape leaves it.
        if self.finding.is_some() && self.renaming.is_none() && self.naming.is_none() {
            return self.find_key(key, modifiers, cx);
        }
        if self.renaming.is_some() || self.naming.is_some() || self.noting.is_some() {
            return self.name_key(key, modifiers, cx);
        }

        let pointable = self.pointable();
        if let Some(edit) = self.edit.as_mut() {
            // Mid-composition the keys are the IME's.
            if edit.text.marked.is_some() {
                return false;
            }
            let completion = formula_complete::completion(&edit.text.text, edit.text.caret);
            match key {
                // Function AutoComplete: Tab puts the name picked in, the
                // arrows move through the list.
                "tab" if !shift && completion.is_some() => {
                    if let Some(list) = completion {
                        let name = list.names[picked(edit, &list)];
                        formula_complete::accept(&mut edit.text, &list, name);
                        edit.point = None;
                        edit.suggested = None;
                    }
                }
                "up" | "down" if completion.is_some() => {
                    if let Some(list) = completion {
                        let count = list.names.len();
                        let at = picked(edit, &list);
                        let at = if key == "down" {
                            (at + 1) % count
                        } else {
                            (at + count - 1) % count
                        };
                        edit.suggested = Some((list.start, edit.text.caret, at));
                    }
                }
                // A line break in the cell: ⌥Enter, as Excel for Mac.
                "enter" if modifiers.alt => {
                    edit.text.insert("\n");
                    edit.point = None;
                }
                // ⌘Enter: what was typed into every selected cell.
                "enter" if command => {
                    let text = edit.text.text.clone();
                    let range = self.range();
                    let active = self.active();
                    let result = attempt(
                        &mut self.book,
                        &mut self.protection_dialog,
                        ProtectedEdit::FillWith {
                            range,
                            active,
                            text,
                        },
                    )
                    .map(|outcome| outcome.changed);
                    if result.is_ok() {
                        self.edit = None;
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
            "f5" => self.open_go_to(cx),
            // Calculate Now, as in Excel.
            "f9" => {
                let result = self.book.calculate_now();
                self.after_write(result, cx);
            }
            "delete" | "backspace" => {
                let range = self.range();
                let result = attempt(
                    &mut self.book,
                    &mut self.protection_dialog,
                    ProtectedEdit::Clear { range },
                )
                .map(|outcome| outcome.changed);
                self.after_write(result, cx);
            }
            // Escape lets go of what was copied.
            "escape" => {
                self.book.forget_copy();
                self.format_source = None;
                self.format_sticky = false;
                cx.notify();
            }
            "=" if modifiers.alt => self.autosum(cx),
            // ⌥↓: the active cell's list of choices.
            "down" if modifiers.alt => {
                self.choices = self.book.validation_dropdown(self.active());
                cx.notify();
            }
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
            // ⌘⇧F (Ctrl+Shift+L, as Excel for Windows): the filter
            // buttons on the data round the selection, or off again.
            "f" | "l" if shift => {
                let result = self.book.toggle_filter(range);
                self.after_write(result, cx);
            }
            "f" => self.open_find(false, cx),
            "t" => self.open_table_dialog(cx),
            "k" => self.open_link_dialog(cx),
            // ⌃H, as Excel for Mac: Find and Replace (⌘H hides the app).
            "h" if modifiers.control && !modifiers.platform => self.open_find(true, cx),
            // ⌃G, as F5: Go To.
            "g" if modifiers.control && !modifiers.platform => self.open_go_to(cx),
            "end" => {
                let end = self.book.used_end();
                self.select_cell(end, shift, cx);
            }
            // ⌘C, ⌘X, ⌘V and ⌘A are actions; these are their Ctrl forms.
            "c" if !modifiers.platform => self.sheet_copy(false, cx),
            "x" if !shift && !modifiers.platform => self.sheet_copy(true, cx),
            // ⌃⌘V, as Excel for Mac (Ctrl+Alt+V elsewhere): Paste Special.
            "v" if modifiers.control && (modifiers.platform || modifiers.alt) => {
                self.open_paste_special(cx)
            }
            "v" if !modifiers.platform => self.paste(cx),
            "a" if !modifiers.platform => self.select_all(cx),
            "d" => {
                let result = attempt(
                    &mut self.book,
                    &mut self.protection_dialog,
                    ProtectedEdit::FillDown { range },
                )
                .map(|outcome| outcome.changed);
                self.after_write(result, cx);
            }
            "r" => {
                let result = attempt(
                    &mut self.book,
                    &mut self.protection_dialog,
                    ProtectedEdit::FillRight { range },
                )
                .map(|outcome| outcome.changed);
                self.after_write(result, cx);
            }
            // ⌘1: Format Cells, as in Excel.
            "1" if !shift => self.open_format_dialog(cx),
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

    /// Keys while a tab is renamed, or the name box, the find bar or a
    /// note is typed into.
    fn name_key(&mut self, key: &str, modifiers: Modifiers, cx: &mut Context<Self>) -> bool {
        let command = modifiers.platform || modifiers.control;
        let noting = self.noting.is_some();
        let Some(text) = self.typing() else {
            return false;
        };
        // A note takes line breaks: Enter on its own ends it.
        if noting && key == "enter" && (modifiers.alt || modifiers.shift) {
            text.insert("\n");
            cx.notify();
            return true;
        }
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
    /// Go To (F5, ⌃G): the name box opened on the active cell, for a cell,
    /// range or name to go to; one that does not exist is said so.
    fn open_go_to(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        self.menu = None;
        self.naming = Some(TextEdit::selected(reference_text(self.active())));
        self.going_to = true;
        cx.notify();
    }

    fn finish_naming(&mut self, take: bool, cx: &mut Context<Self>) {
        if let Some((index, text)) = self.renaming.take() {
            let name = text.text.trim().to_string();
            if take && !name.is_empty() {
                let result = self.book.rename_sheet(index, &name);
                self.sheets_changed(result, cx);
            }
        } else if let Some((cell, text)) = self.noting.take() {
            if take {
                let result = self.book.set_note(cell, &text.text, "jade");
                self.after_write(result, cx);
            }
        } else if self.finding.take().is_some() {
            // Escape or Tab closes the find bar where the search got to.
        } else if let Some(text) = self.naming.take() {
            let going_to = std::mem::take(&mut self.going_to);
            if take {
                match self.book.resolve_reference(&text.text) {
                    Some((sheet, range)) => {
                        if !self.book.sheet_tabs().contains(&sheet) {
                            self.status = i18n::text("sheet.no_reference").into();
                            cx.notify();
                            return;
                        }
                        self.select_sheet(sheet, cx);
                        if self.book.can_select_range(range) {
                            self.selection = Selection::range_from(range.start, range.end);
                        }
                        self.tab_start = None;
                        self.reveal(range.start);
                    }
                    None if going_to => self.status = i18n::text("sheet.no_reference").into(),
                    None => {
                        let result = self
                            .book
                            .define_range_name(&text.text, self.selection.range());
                        self.after_write(result, cx);
                    }
                }
            }
        }
        cx.notify();
    }

    // ---- the mouse -------------------------------------------------------

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.validation_dialog.is_some()
            || self.validation_pending.is_some()
            || self.link_dialog.is_some()
            || self.protection_dialog.is_some()
            || self.table_dialog.is_some()
            || self.pivot_dialog.is_some()
            || self.subtotal_dialog.is_some()
            || self.text_columns_dialog.is_some()
            || self.sort_dialog.is_some()
            || self.page_setup_dialog.is_some()
            || self.duplicates_dialog.is_some()
        {
            return;
        }
        window.focus(&self.focus);
        self.menu = None;
        self.filter = None;
        self.popup = None;
        self.choices = None;
        if self.renaming.is_some() || self.naming.is_some() {
            self.finish_naming(true, cx);
        }
        let Some((x, y)) = self.grid_point(event.position) else {
            return;
        };
        let scroll = self.sheet_scroll();
        // The outline gutter's level buttons and boxes collapse and expand.
        if self.edit.is_none() {
            if let Some(result) = self.book.outline_click(x, y, scroll) {
                self.sheets_changed(result, cx);
                return;
            }
        }
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

        // A chart, picture or shape sits over the cells: a press on one
        // selects it, and its border or a handle drags it.
        if self.object_press(x, y, cx) {
            return;
        }

        // A filter button opens its dropdown.
        if let Some(button) = self.book.filter_button_at(x, y, scroll) {
            self.open_filter(button, scroll, cx);
            return;
        }
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
                    let result = attempt(
                        &mut self.book,
                        &mut self.protection_dialog,
                        ProtectedEdit::FillSeries {
                            source: range,
                            target,
                        },
                    )
                    .map(|outcome| outcome.changed);
                    if matches!(result, Ok(true)) {
                        self.selection.anchor = target.start;
                        self.selection.focus = target.end;
                    }
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
        // The selection's border: a drag moves the cells (⌥ copies them),
        // as Excel's drag and drop.
        if !event.modifiers.shift
            && !self.selection.is_whole_rows()
            && !self.selection.is_whole_columns()
            && self.book.on_selection_border(x, y, scroll, range)
        {
            let at = self.book.reach_point(x, y, scroll);
            self.drag = Some(SheetDrag::Move {
                source: range,
                grab: (
                    i64::from(at.column) - i64::from(range.start.column),
                    i64::from(at.row) - i64::from(range.start.row),
                ),
                target: range,
                copy: event.modifiers.alt,
            });
            cx.notify();
            return;
        }
        let Some(hit) = hit else {
            return;
        };
        let previous_selection = self.selection;
        let kind = self.selection.press(hit, event.modifiers.shift);
        if !self.book.can_select_range(self.selection.range()) {
            self.selection = previous_selection;
            return;
        }
        self.drag = Some(SheetDrag::Select(kind));
        // A hyperlink is followed when this press lets go on it.
        self.pending_link = match hit {
            Hit::Cell(cell) if !event.modifiers.shift && event.click_count == 1 => {
                self.book.link(cell).map(|link| (cell, link))
            }
            _ => None,
        };
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
                let previous_selection = self.selection;
                self.selection.press(hit, false);
                if !self.book.can_select_range(self.selection.range()) {
                    self.selection = previous_selection;
                    return;
                }
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
            None if !self.selection.is_whole_rows()
                && !self.selection.is_whole_columns()
                && self.book.on_selection_border(x, y, scroll, self.range()) =>
            {
                Hover::Border
            }
            None if matches!(
                self.book.hit(x, y, scroll),
                Some(Hit::Cell(cell)) if self.book.link(cell).is_some()
            ) =>
            {
                Hover::Link
            }
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
            Some(Hit::Cell(cell)) if self.book.hover_text(cell).is_some() => self
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
                if !self.book.can_select_range(self.selection.range()) {
                    self.selection = before;
                    return;
                }
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
            SheetDrag::Move {
                source,
                grab,
                target,
                copy,
            } => {
                // The range keeps its shape, its top-left where the
                // pointer's grab puts it.
                let start = grid::offset(reached, -grab.0, -grab.1);
                let next = CellRange {
                    start,
                    end: CellRef::new(
                        (start.column + (source.end.column - source.start.column)).min(MAX_COLUMN),
                        (start.row + (source.end.row - source.start.row)).min(MAX_ROW),
                    ),
                };
                if next != target {
                    self.drag = Some(SheetDrag::Move {
                        source,
                        grab,
                        target: next,
                        copy,
                    });
                    self.book.reach(next.end);
                    cx.notify();
                }
            }
            SheetDrag::Object {
                id,
                handle,
                start,
                bounds,
                ..
            } => {
                let now = self.object_drag(handle, start, bounds, x, y);
                self.drag = Some(SheetDrag::Object {
                    id,
                    handle,
                    start,
                    bounds,
                    now,
                });
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
                let result = attempt(
                    &mut self.book,
                    &mut self.protection_dialog,
                    ProtectedEdit::FillSeries { source, target },
                )
                .map(|outcome| outcome.changed);
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
            // A click — a press let go where it started — on a hyperlink
            // follows it.
            SheetDrag::Select(_) => {
                // The Format Painter's formats land on what was just selected.
                if let Some(source) = self.format_source {
                    if !self.format_sticky {
                        self.format_source = None;
                    }
                    let result = self.book.copy_format(source, self.range());
                    self.after_write(result, cx);
                    return;
                }
                let link = self.pending_link.take();
                if let Some((_, link)) = link
                    .filter(|(cell, _)| self.selection.is_cell() && self.selection.active == *cell)
                {
                    self.follow_link(link, cx);
                }
                cx.notify();
            }
            // The cells land where they were dropped, formulas following as
            // a cut and paste moves them; the selection goes with them.
            SheetDrag::Move {
                source,
                target,
                copy,
                ..
            } => {
                if target == source {
                    cx.notify();
                    return;
                }
                let result = attempt(
                    &mut self.book,
                    &mut self.protection_dialog,
                    ProtectedEdit::MoveRange {
                        source,
                        to: target.start,
                        copy,
                    },
                )
                .map(|outcome| outcome.selection.unwrap_or(source));
                if let Ok(moved) = &result {
                    let active = self.selection.active;
                    self.selection = Selection {
                        active: grid::offset(
                            moved.start,
                            i64::from(active.column) - i64::from(source.start.column),
                            i64::from(active.row) - i64::from(source.start.row),
                        ),
                        anchor: moved.start,
                        focus: moved.end,
                    };
                }
                self.after_write(result.map(|_| true), cx);
            }
            SheetDrag::Object {
                id, bounds, now, ..
            } => self.object_drop(id, bounds, now, cx),
            SheetDrag::Point => cx.notify(),
        }
    }

    /// Data ▸ Remove Duplicates over `range`, comparing `columns`, its
    /// first row a header when `headers`, and Excel's count in the status
    /// bar.
    fn remove_duplicates(
        &mut self,
        range: CellRange,
        columns: &[u32],
        headers: bool,
        cx: &mut Context<Self>,
    ) {
        match self.book.remove_duplicates(range, columns, headers) {
            Ok(outcome) => {
                self.after_write(Ok(outcome.removed > 0), cx);
                self.status = if outcome.removed == 0 {
                    i18n::text("sheet.duplicates_none").into()
                } else {
                    i18n::format(
                        "sheet.duplicates_removed",
                        &[
                            ("removed", &outcome.removed),
                            ("remaining", &outcome.remaining),
                        ],
                    )
                    .into()
                };
                cx.notify();
            }
            Err(error) => self.after_write(Err(error), cx),
        }
    }

    /// The Format Painter: the selection's formats picked up for the next
    /// selection, or for every one until Escape when `sticky`; a click while
    /// it is on puts it down.
    fn pick_up_format(&mut self, sticky: bool, cx: &mut Context<Self>) {
        if self.format_source.is_some() && !sticky {
            self.format_source = None;
            self.format_sticky = false;
        } else {
            self.format_source = Some(self.range());
            self.format_sticky = sticky;
        }
        cx.notify();
    }

    /// Follows a hyperlink: a web address opens in the browser, a place in
    /// the workbook is selected.
    fn follow_link(&mut self, link: Link, cx: &mut Context<Self>) {
        match link {
            Link::Url(url) => cx.open_url(&url),
            Link::Place { sheet, range } => {
                self.select_sheet(sheet, cx);
                if self.book.can_select_range(range) {
                    self.selection = Selection::range_from(range.start, range.end);
                }
                self.reveal(range.start);
                cx.notify();
            }
        }
    }

    /// A choice from the active cell's list goes into it.
    fn choose(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(choice) = self
            .choices
            .take()
            .and_then(|choices| choices.into_iter().nth(index))
        else {
            return;
        };
        let result = self.book.set_validation_choice(self.active(), &choice);
        if matches!(result, Ok(true)) {
            self.book.forget_copy();
        }
        self.after_write(result, cx);
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
                            view.going_to = false;
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
                .children(completion_list(edit, cell.size.height, cx))
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
            Some(index) => {
                let mut items = vec![
                    (MenuCommand::InsertSheet, "sheet.menu.insert_sheet"),
                    (MenuCommand::ProtectWorkbook, "protection.menu"),
                    (MenuCommand::ProtectSheet(index), "protection.sheet_menu"),
                    (MenuCommand::EditRanges(index), "protection.ranges_menu"),
                    (MenuCommand::RenameSheet(index), "sheet.menu.rename_sheet"),
                    (MenuCommand::DeleteSheet(index), "sheet.menu.delete_sheet"),
                    (MenuCommand::HideSheet(index), "sheet.menu.hide_sheet"),
                ];
                for hidden in self.book.hidden_sheets() {
                    items.push((MenuCommand::UnhideSheet(hidden), "sheet.menu.unhide_sheet"));
                }
                if index > 0 {
                    items.push((
                        MenuCommand::MoveSheet(index, index - 1),
                        "sheet.menu.move_left",
                    ));
                }
                if index + 1 < self.sheet_names.len() {
                    items.push((
                        MenuCommand::MoveSheet(index, index + 1),
                        "sheet.menu.move_right",
                    ));
                }
                items
            }
            None => {
                let mut items = vec![
                    (MenuCommand::Cut, "sheet.menu.cut"),
                    (MenuCommand::Copy, "sheet.menu.copy"),
                ];
                if self.book.copied().is_some() {
                    items.push((MenuCommand::Paste, "sheet.menu.paste"));
                }
                if self.book.can_paste_special(None) {
                    items.push((MenuCommand::PasteSpecial, "sheet.menu.paste_special"));
                }
                items.extend([
                    (MenuCommand::Clear, "sheet.menu.clear"),
                    (MenuCommand::ClearFormats, "sheet.menu.clear_formats"),
                    (MenuCommand::ClearAll, "sheet.menu.clear_all"),
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
                if !self.selection.is_whole_rows() && !self.selection.is_whole_columns() {
                    items.extend([
                        (
                            MenuCommand::ShiftCells(Axis::Rows, true),
                            "sheet.menu.insert_cells_down",
                        ),
                        (
                            MenuCommand::ShiftCells(Axis::Columns, true),
                            "sheet.menu.insert_cells_right",
                        ),
                        (
                            MenuCommand::ShiftCells(Axis::Rows, false),
                            "sheet.menu.delete_cells_up",
                        ),
                        (
                            MenuCommand::ShiftCells(Axis::Columns, false),
                            "sheet.menu.delete_cells_left",
                        ),
                    ]);
                }
                if self.selection.is_whole_rows() || self.selection.is_whole_columns() {
                    items.extend([
                        (MenuCommand::Hide, "sheet.menu.hide"),
                        (MenuCommand::Unhide, "sheet.menu.unhide"),
                    ]);
                    let axis = if self.selection.is_whole_columns() {
                        Axis::Columns
                    } else {
                        Axis::Rows
                    };
                    items.extend([
                        (
                            MenuCommand::Outline(OutlineChange::Group),
                            "sheet.menu.group",
                        ),
                        (
                            MenuCommand::Outline(OutlineChange::Ungroup),
                            "sheet.menu.ungroup",
                        ),
                    ]);
                    if self.book.has_outline(axis) {
                        items.extend([
                            (
                                MenuCommand::Outline(OutlineChange::HideDetail),
                                "sheet.menu.hide_detail",
                            ),
                            (
                                MenuCommand::Outline(OutlineChange::ShowDetail),
                                "sheet.menu.show_detail",
                            ),
                        ]);
                    }
                    items.push((MenuCommand::AutoOutline, "sheet.menu.auto_outline"));
                    if self.book.has_outline(axis) {
                        items.push((MenuCommand::ClearOutline, "sheet.menu.clear_outline"));
                    }
                }
                if self.book.note(self.active()).is_some() {
                    items.extend([
                        (MenuCommand::EditNote, "sheet.menu.edit_note"),
                        (MenuCommand::DeleteNote, "sheet.menu.delete_note"),
                    ]);
                } else {
                    items.push((MenuCommand::EditNote, "sheet.menu.insert_note"));
                }
                items.push(if self.book.frozen_panes().is_some() {
                    (MenuCommand::UnfreezePanes, "sheet.menu.unfreeze_panes")
                } else {
                    (MenuCommand::FreezePanes, "sheet.menu.freeze_panes")
                });
                if !self.selection.is_whole_rows()
                    && !self.selection.is_whole_columns()
                    && self.selection.range().end.row > self.selection.range().start.row
                    && self.selection.range().end.column > self.selection.range().start.column
                {
                    items.push((MenuCommand::Subtotal, "sheet.menu.subtotal"));
                    items.push((
                        MenuCommand::RemoveDuplicates,
                        "sheet.menu.remove_duplicates",
                    ));
                }
                items.push((MenuCommand::SortDialog, "sheet.menu.sort_dialog"));
                if self.range().start.column == self.range().end.column {
                    items.push((MenuCommand::TextToColumns, "sheet.menu.text_to_columns"));
                }
                if self.book.manual_calculation() {
                    items.push((MenuCommand::CalculateNow, "sheet.menu.calculate_now"));
                    items.push((
                        MenuCommand::ManualCalculation(false),
                        "sheet.menu.automatic_calculation",
                    ));
                } else {
                    items.push((
                        MenuCommand::ManualCalculation(true),
                        "sheet.menu.manual_calculation",
                    ));
                }
                items.push((MenuCommand::FormatPainter, "sheet.menu.format_painter"));
                items.push((MenuCommand::FormatCells, "sheet.menu.format_cells"));
                items.push((MenuCommand::CellLocked, "protection.cell_locked"));
                items.push((MenuCommand::CellHidden, "protection.cell_hidden"));
                items.push((
                    MenuCommand::EditRanges(self.book.sheet_index()),
                    "protection.ranges_menu",
                ));
                items.push((MenuCommand::CreatePivot, "pivot.create"));
                if self.book.has_pivots() {
                    items.push((MenuCommand::RefreshAllPivots, "pivot.refresh_all"));
                }
                if self.book.pivot_at(self.active()).is_some() {
                    items.push((MenuCommand::RefreshPivot, "pivot.refresh"));
                    items.push((MenuCommand::EditPivot, "pivot.edit"));
                    items.push((MenuCommand::DeletePivot, "pivot.delete"));
                }
                items.extend([
                    (MenuCommand::Table, "table.menu"),
                    (MenuCommand::Link, "link.dialog"),
                    (MenuCommand::RemoveLinks, "link.remove"),
                    (MenuCommand::Validation, "validation.dialog"),
                    (MenuCommand::ClearValidation, "validation.clear_selection"),
                    (MenuCommand::PageSetup, "page_setup.menu"),
                ]);
                items
            }
        };
        let max_height = (self.viewport.1 - 16.).max(80.);
        let swatch_rows = if menu.tab.is_some() { 3. } else { 0. };
        let height = ((items.len() as f32 + swatch_rows) * 26. + 8.).min(max_height);
        let top = menu.at.1.clamp(0., (self.viewport.1 - height).max(0.));
        Some(
            div()
                .id("sheet-menu")
                .absolute()
                .left(px(menu.at.0))
                .top(px(top))
                .min_w(px(180.0))
                .max_h(px(max_height))
                .overflow_y_scroll()
                .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
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
                            let label: SharedString = match command {
                                MenuCommand::UnhideSheet(sheet) => {
                                    format!("{}: {}", i18n::text(label), self.sheet_names[sheet])
                                        .into()
                                }
                                _ => i18n::text(label).into(),
                            };
                            div()
                                .id(("sheet-menu-item", index))
                                .px_3()
                                .py_1()
                                .cursor_pointer()
                                .hover(|item| item.bg(theme::hovered()))
                                .child(label)
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(cx.listener(move |view, _event, window, cx| {
                                    window.focus(&view.focus);
                                    view.menu_command(command, false, cx);
                                }))
                        }),
                )
                .when_some(menu.tab, |list, tab| {
                    // Tab Color: Excel's standard colours, and No Color.
                    let swatch = |id: usize, color: Option<&'static str>| {
                        div()
                            .id(("tab-color", id))
                            .size(px(16.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_sm()
                            .border_1()
                            .border_color(theme::border())
                            .cursor_pointer()
                            .when_some(color, |swatch, color| {
                                swatch.bg(gpui::rgb(u32::from_str_radix(color, 16).unwrap_or(0)))
                            })
                            .when(color.is_none(), |swatch| swatch.child("╱"))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(move |view, _event, window, cx| {
                                window.focus(&view.focus);
                                view.menu_command(MenuCommand::TabColor(tab, color), false, cx);
                            }))
                    };
                    list.child(
                        div()
                            .px_3()
                            .pt_1()
                            .text_color(theme::text_dim())
                            .child(i18n::text("sheet.menu.tab_color")),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_1()
                            .w(px(180.0))
                            .flex()
                            .flex_wrap()
                            .gap_1()
                            .children(
                                STANDARD_COLORS
                                    .iter()
                                    .enumerate()
                                    .map(|(id, color)| swatch(id, Some(color))),
                            )
                            .child(swatch(STANDARD_COLORS.len(), None)),
                    )
                }),
        )
    }

    /// The sheet tabs and Excel's ⊕ after them.
    fn sheet_tabs(&self, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let current = self.book.sheet_index();
        let renaming = self.renaming.as_ref().map(|(index, text)| (*index, text));
        // A hidden sheet has no tab, as in Excel.
        let visible = self.book.sheet_tabs();
        let mut tabs: Vec<gpui::AnyElement> = self
            .sheet_names
            .iter()
            .enumerate()
            .filter(|(index, _)| visible.contains(index))
            .map(|(index, name)| {
                let selected = index == current;
                // A coloured tab (sheetPr/tabColor) is filled with its colour,
                // and underlined with it when selected, as in Excel.
                let tab_color = self.book.tab_color(index);
                let tab_fill =
                    tab_color.map(|[r, g, b]| gpui::rgb(u32::from_be_bytes([0, r, g, b])));
                let light_tab = tab_color.is_some_and(|[r, g, b]| {
                    u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114 > 128_000
                });
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
                        theme::accent().into()
                    } else if tab_fill.is_some() {
                        if light_tab {
                            gpui::black()
                        } else {
                            gpui::white()
                        }
                    } else {
                        theme::text_dim().into()
                    })
                    .when(!selected, |tab| {
                        tab.when_some(tab_fill, |tab, fill| tab.bg(fill))
                    })
                    .when(selected, |tab| {
                        tab.bg(theme::window()).child(
                            div()
                                .absolute()
                                .bottom_0()
                                .left_1()
                                .right_1()
                                .h(px(if tab_fill.is_some() { 3.0 } else { 2.0 }))
                                .rounded_t_sm()
                                .bg(tab_fill
                                    .map_or_else(|| theme::accent().into(), gpui::Hsla::from)),
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

/// The toolbar and the dropdowns over the grid.
impl WorkbookView {
    /// The formatting strip: font styles, alignment, wrap and merge, number
    /// formats, decimals, font size, colours, borders and the filter — lit
    /// where the active cell has them.
    fn sheet_toolbar(&self, cx: &mut Context<Self>) -> gpui::Div {
        let look = self.book.format_at(self.active());
        let open = self.popup;
        // The open dropdown hangs under its button, painted over the grid.
        let mut dropdown = self.toolbar_popup(cx).map(gpui::deferred);
        let mut hang = move |which: Popup, button: gpui::Stateful<gpui::Div>| {
            let button = button.relative();
            match dropdown.take().filter(|_| open == Some(which)) {
                Some(dropdown) => button.child(dropdown),
                None => button,
            }
        };
        let button = |id: &'static str, on: bool, tool: Tool| {
            div()
                .id(id)
                .min_w(px(24.0))
                .h(px(24.0))
                .px_1()
                .flex()
                .items_center()
                .justify_center()
                .rounded_sm()
                .text_xs()
                .cursor_pointer()
                .text_color(theme::text())
                .when(on, |button| button.bg(theme::selected()))
                .hover(|button| button.bg(theme::hovered()))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |view, _event, window, cx| {
                    window.focus(&view.focus);
                    view.tool(tool, cx);
                }))
        };
        let icon = |name: &'static str| svg().path(name).size_3p5().text_color(theme::text());
        let separator = || div().w(px(1.0)).h(px(18.0)).mx_1().bg(theme::border());
        let horizontal = look.horizontal.unwrap_or_default();
        let fill = look
            .fill_color
            .as_deref()
            .filter(|color| !color.is_empty())
            .and_then(|color| u32::from_str_radix(color, 16).ok());
        let ink = look
            .font_color
            .as_deref()
            .and_then(|color| u32::from_str_radix(color, 16).ok());
        let popup = self.popup;
        div()
            .h(px(SHEET_TOOLBAR_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .gap_0p5()
            .px_2()
            .bg(theme::panel())
            .border_b_1()
            .border_color(theme::border())
            .child(
                div()
                    .id("tool-format-painter")
                    .min_w(px(24.0))
                    .h(px(24.0))
                    .px_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .cursor_pointer()
                    .when(self.format_source.is_some(), |button| {
                        button.bg(theme::selected())
                    })
                    .hover(|button| button.bg(theme::hovered()))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    // A double-click keeps painting until Escape, as Excel's
                    // brush does.
                    .on_click(cx.listener(move |view, event: &ClickEvent, window, cx| {
                        window.focus(&view.focus);
                        view.pick_up_format(event.click_count() >= 2, cx);
                    }))
                    .child(icon("icons/paintbrush.svg")),
            )
            .child(
                button(
                    "tool-bold",
                    look.bold == Some(true),
                    Tool::Font(FontToggle::Bold),
                )
                .child("B")
                .font_weight(gpui::FontWeight::BOLD),
            )
            .child(
                button(
                    "tool-italic",
                    look.italic == Some(true),
                    Tool::Font(FontToggle::Italic),
                )
                .child("I")
                .italic(),
            )
            .child(
                button(
                    "tool-underline",
                    look.underline == Some(true),
                    Tool::Font(FontToggle::Underline),
                )
                .child("U")
                .underline(),
            )
            .child(
                button(
                    "tool-strike",
                    look.strike == Some(true),
                    Tool::Font(FontToggle::Strike),
                )
                .child("S")
                .line_through(),
            )
            .child(separator())
            .child(button("tool-font-larger", false, Tool::FontSize(true)).child("A+"))
            .child(button("tool-font-smaller", false, Tool::FontSize(false)).child("A-"))
            .child(separator())
            .child(
                button(
                    "tool-left",
                    horizontal == HorizontalAlignment::Left,
                    Tool::Align(HorizontalAlignment::Left),
                )
                .child(icon("icons/align-left.svg")),
            )
            .child(
                button(
                    "tool-center",
                    horizontal == HorizontalAlignment::Center,
                    Tool::Align(HorizontalAlignment::Center),
                )
                .child(icon("icons/align-center.svg")),
            )
            .child(
                button(
                    "tool-right",
                    horizontal == HorizontalAlignment::Right,
                    Tool::Align(HorizontalAlignment::Right),
                )
                .child(icon("icons/align-right.svg")),
            )
            .child(
                button("tool-wrap", look.wrap_text == Some(true), Tool::Wrap)
                    .child(icon("icons/wrap-text.svg")),
            )
            .child(button("tool-merge", false, Tool::Merge).child(icon("icons/merge.svg")))
            .child(separator())
            .child(hang(
                Popup::NumberFormat,
                // Named for the active cell's format, as Excel's box is.
                button(
                    "tool-number-format",
                    popup == Some(Popup::NumberFormat),
                    Tool::Open(Popup::NumberFormat),
                )
                .gap_1()
                .min_w(px(72.0))
                .justify_between()
                .child(i18n::text(
                    self.book.number_format_name(self.active(), i18n::text),
                ))
                .child(icon("icons/chevron-down.svg").size_3()),
            ))
            .child(
                button("tool-currency", false, Tool::Currency)
                    .child(i18n::text("sheet.tool.currency_symbol")),
            )
            .child(button("tool-percent", false, Tool::Percent).child("%"))
            .child(button("tool-comma", false, Tool::Comma).child(","))
            .child(button("tool-more-decimals", false, Tool::Decimals(true)).child(".0→"))
            .child(button("tool-fewer-decimals", false, Tool::Decimals(false)).child("←.0"))
            .child(separator())
            .child(hang(
                Popup::Fill,
                // The colour under the bucket and the A is the active
                // cell's, as Excel's buttons show theirs.
                button(
                    "tool-fill",
                    popup == Some(Popup::Fill),
                    Tool::Open(Popup::Fill),
                )
                .flex_col()
                .child(icon("icons/paint-bucket.svg"))
                .child(
                    div()
                        .w(px(12.0))
                        .h(px(3.0))
                        .bg(fill.map_or(theme::text_dim(), gpui::rgb)),
                ),
            ))
            .child(hang(
                Popup::Font,
                button(
                    "tool-font-color",
                    popup == Some(Popup::Font),
                    Tool::Open(Popup::Font),
                )
                .flex_col()
                .child("A")
                .child(
                    div()
                        .w(px(12.0))
                        .h(px(3.0))
                        .bg(ink.map_or(theme::text(), gpui::rgb)),
                ),
            ))
            .child(hang(
                Popup::Borders,
                button(
                    "tool-borders",
                    popup == Some(Popup::Borders),
                    Tool::Open(Popup::Borders),
                )
                .child(icon("icons/borders.svg")),
            ))
            .child(hang(
                Popup::Conditional,
                button(
                    "tool-conditional",
                    popup == Some(Popup::Conditional),
                    Tool::Open(Popup::Conditional),
                )
                .child(icon("icons/highlighter.svg")),
            ))
            .child(hang(
                Popup::Chart,
                button(
                    "tool-chart",
                    popup == Some(Popup::Chart),
                    Tool::Open(Popup::Chart),
                )
                .child(icon("icons/chart-column.svg")),
            ))
            .child(separator())
            .child(button("tool-filter", false, Tool::Filter).child(icon("icons/filter.svg")))
    }

    /// Excel's ▼ beside an active cell with a list of choices (its data
    /// validation), and the list under it when open.
    fn list_button(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        if self.edit.is_some() {
            return None;
        }
        self.book.validation_dropdown(self.active())?;
        let grid = self.grid_bounds.get()?;
        let cell = self.cell_bounds(self.active())?;
        let (x, y) = (cell.origin.x - grid.origin.x, cell.origin.y - grid.origin.y);
        let (w, h) = (cell.size.width, cell.size.height);
        let items = self.choices.clone().unwrap_or_default();
        let open = self.choices.is_some();
        Some(
            div()
                .absolute()
                .left(x + w + px(1.0))
                .top(y)
                .child(
                    div()
                        .id("choices-button")
                        .w(h.max(px(14.0)))
                        .h(h)
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(gpui::rgb(0xf3f3f3))
                        .border_1()
                        .border_color(gpui::rgb(0xababab))
                        .cursor_pointer()
                        .child(
                            svg()
                                .path("icons/chevron-down.svg")
                                .size_3()
                                .text_color(gpui::black()),
                        )
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(|view, _event, window, cx| {
                            window.focus(&view.focus);
                            view.choices = if view.choices.is_some() {
                                None
                            } else {
                                view.book.validation_list(view.active())
                            };
                            cx.notify();
                        })),
                )
                .when(open, |anchor| {
                    anchor.child(
                        div()
                            .absolute()
                            .top(h)
                            .right_0()
                            .min_w(w.max(px(120.0)))
                            .max_h(px(220.0))
                            .bg(gpui::white())
                            .border_1()
                            .border_color(gpui::rgb(0x7a7a7a))
                            .shadow_md()
                            .text_xs()
                            .text_color(gpui::black())
                            .occlude()
                            .child(
                                div()
                                    .id("choices")
                                    .max_h(px(218.0))
                                    .overflow_y_scroll()
                                    .children(items.into_iter().enumerate().map(
                                        |(index, item)| {
                                            div()
                                                .id(("choice", index))
                                                .px_2()
                                                .py_0p5()
                                                .cursor_pointer()
                                                .hover(|row| row.bg(gpui::rgb(0xcce8ff)))
                                                .child(item)
                                                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                                    cx.stop_propagation()
                                                })
                                                .on_click(cx.listener(
                                                    move |view, _event, _window, cx| {
                                                        view.choose(index, cx)
                                                    },
                                                ))
                                        },
                                    )),
                            ),
                    )
                }),
        )
    }

    /// A toolbar button's dropdown: number formats, colours, borders or
    /// conditional formats.
    fn toolbar_popup(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let popup = self.popup?;
        let active = self.active();
        let swatch = |index: usize, color: &'static str, tool: Tool| {
            div()
                .id(("swatch", index))
                .size(px(18.0))
                .rounded_sm()
                .border_1()
                .border_color(theme::border())
                .bg(gpui::rgb(u32::from_str_radix(color, 16).unwrap_or(0)))
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |view, _event, _window, cx| view.tool(tool, cx)))
        };
        let item = |id: &'static str, label: &'static str, tool: Tool| {
            div()
                .id(id)
                .px_3()
                .py_1()
                .cursor_pointer()
                .hover(|item| item.bg(theme::hovered()))
                .child(i18n::text(label))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |view, _event, _window, cx| view.tool(tool, cx)))
        };
        let body = match popup {
            Popup::Fill | Popup::Font => {
                let swatches = STANDARD_COLORS.iter().enumerate().map(|(index, color)| {
                    swatch(
                        index,
                        color,
                        if popup == Popup::Fill {
                            Tool::Fill(Some(color))
                        } else {
                            Tool::FontColor(color)
                        },
                    )
                });
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_2()
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .w(px(160.0))
                            .gap_1()
                            .children(swatches),
                    )
                    .when(popup == Popup::Fill, |body| {
                        body.child(item("no-fill", "sheet.tool.no_fill", Tool::Fill(None)))
                    })
                    .when(popup == Popup::Font, |body| {
                        body.child(item(
                            "automatic",
                            "sheet.tool.automatic",
                            Tool::FontColor("000000"),
                        ))
                    })
            }
            Popup::NumberFormat => div().py_1().children(NUMBER_FORMATS.iter().map(|choice| {
                let code = i18n::text(choice.code);
                // Each with what the active cell would read as in it.
                let sample = self.book.number_format_sample(active, code);
                item(choice.id, choice.name, Tool::NumberFormat(choice.code))
                    .flex()
                    .justify_between()
                    .gap_4()
                    .children(sample.map(|sample| {
                        div()
                            .max_w(px(140.0))
                            .overflow_hidden()
                            .text_color(theme::text_dim())
                            .child(sample)
                    }))
            })),
            Popup::Borders => div()
                .py_1()
                .child(item(
                    "borders-bottom",
                    "sheet.tool.border_bottom",
                    Tool::Borders(Borders::Bottom),
                ))
                .child(item(
                    "borders-all",
                    "sheet.tool.border_all",
                    Tool::Borders(Borders::All),
                ))
                .child(item(
                    "borders-outline",
                    "sheet.tool.border_outline",
                    Tool::Borders(Borders::Outline),
                ))
                .child(item(
                    "borders-thick",
                    "sheet.tool.border_thick",
                    Tool::Borders(Borders::ThickOutline),
                ))
                .child(item(
                    "borders-none",
                    "sheet.tool.border_none",
                    Tool::Borders(Borders::None),
                )),
            Popup::Conditional => self.conditional_menu(cx),
            Popup::Chart => self.chart_menu(cx),
        };
        Some(
            div()
                .absolute()
                .top(px(26.0))
                .left_0()
                .min_w(px(150.0))
                .whitespace_nowrap()
                .rounded_md()
                .border_1()
                .border_color(theme::border())
                .bg(theme::panel())
                .shadow_lg()
                .text_xs()
                .text_color(theme::text())
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(body),
        )
    }

    /// A filter button's dropdown: Sort A to Z and Z to A, Clear Filter,
    /// (Select All) and the column's values to tick, OK and Cancel.
    fn filter_panel(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let panel = self.filter.as_ref()?;
        let action = |id: &'static str, label: &'static str, command: FilterCommand| {
            div()
                .id(id)
                .px_3()
                .py_1()
                .cursor_pointer()
                .hover(|item| item.bg(theme::hovered()))
                .child(i18n::text(label))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(
                    cx.listener(move |view, _event, _window, cx| view.filter_command(command, cx)),
                )
        };
        let tick = |checked: bool| if checked { "☑" } else { "☐" };
        let all = panel.checked.iter().all(|checked| *checked);
        let rows = panel.options.iter().zip(&panel.checked).enumerate().map(
            |(index, (option, checked))| {
                let label = if option.blank {
                    i18n::text("sheet.filter.blank").to_string()
                } else {
                    option.label.clone()
                };
                div()
                    .id(("filter-value", index))
                    .flex()
                    .gap_2()
                    .px_3()
                    .py_0p5()
                    .cursor_pointer()
                    .hover(|item| item.bg(theme::hovered()))
                    .child(tick(*checked))
                    .child(label)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |view, _event, _window, cx| {
                        view.filter_command(FilterCommand::Toggle(index), cx)
                    }))
            },
        );
        Some(
            div()
                .absolute()
                .left(px(panel.at.0.max(0.0)))
                .top(px(panel.at.1.max(0.0)))
                .w(px(220.0))
                .py_1()
                .rounded_md()
                .border_1()
                .border_color(theme::border())
                .bg(theme::panel())
                .shadow_lg()
                .text_xs()
                .text_color(theme::text())
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(action(
                    "filter-asc",
                    "sheet.menu.sort_ascending",
                    FilterCommand::Sort(false),
                ))
                .child(action(
                    "filter-desc",
                    "sheet.menu.sort_descending",
                    FilterCommand::Sort(true),
                ))
                .child(action(
                    "filter-clear",
                    "sheet.filter.clear",
                    FilterCommand::Clear,
                ))
                .child(action(
                    "filter-top",
                    "sheet.filter.top",
                    FilterCommand::Top(true),
                ))
                .child(action(
                    "filter-bottom",
                    "sheet.filter.bottom",
                    FilterCommand::Top(false),
                ))
                .child(action(
                    "filter-reapply",
                    "sheet.filter.reapply",
                    FilterCommand::Reapply,
                ))
                .child(div().my_1().h(px(1.0)).bg(theme::border()))
                .child(
                    div()
                        .id("filter-all")
                        .flex()
                        .gap_2()
                        .px_3()
                        .py_0p5()
                        .cursor_pointer()
                        .hover(|item| item.bg(theme::hovered()))
                        .child(tick(all))
                        .child(i18n::text("sheet.filter.all"))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(|view, _event, _window, cx| {
                            view.filter_command(FilterCommand::All, cx)
                        })),
                )
                .child(
                    div()
                        .id("filter-values")
                        .max_h(px(200.0))
                        .overflow_y_scroll()
                        .children(rows),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .px_3()
                        .pt_2()
                        .child(
                            div()
                                .id("filter-ok")
                                .px_3()
                                .py_0p5()
                                .rounded_sm()
                                .cursor_pointer()
                                .bg(theme::accent())
                                .text_color(gpui::white())
                                .child(i18n::text("sheet.filter.ok"))
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(cx.listener(|view, _event, _window, cx| {
                                    view.filter_command(FilterCommand::Apply, cx)
                                })),
                        )
                        .child(
                            div()
                                .id("filter-cancel")
                                .px_3()
                                .py_0p5()
                                .rounded_sm()
                                .cursor_pointer()
                                .border_1()
                                .border_color(theme::border())
                                .child(i18n::text("sheet.filter.cancel"))
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(cx.listener(|view, _event, _window, cx| {
                                    view.filter_command(FilterCommand::Cancel, cx)
                                })),
                        ),
                ),
        )
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
            // A dialog over the grid takes no typing into the cells.
            if self.asking.is_some()
                || self.managing
                || self.pasting.is_some()
                || self.validation_pending.is_some()
            {
                return;
            }
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
            // A dialog over the grid takes no typing into the cells.
            if self.asking.is_some()
                || self.managing
                || self.pasting.is_some()
                || self.validation_pending.is_some()
            {
                return;
            }
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
        if let Some(bounds) = self
            .validation_dialog
            .as_ref()
            .and_then(|d| d.input_bounds.get())
            .or_else(|| {
                self.protection_dialog
                    .as_ref()
                    .and_then(|d| d.input_bounds.get())
            })
            .or_else(|| self.link_dialog.as_ref().and_then(|d| d.input_bounds.get()))
            .or_else(|| {
                self.pivot_dialog
                    .as_ref()
                    .and_then(|d| d.input_bounds.get())
            })
            .or_else(|| {
                self.table_dialog
                    .as_ref()
                    .and_then(|d| d.input_bounds.get())
            })
        {
            return Some(Bounds {
                origin: point(px(bounds[0]), px(bounds[1])),
                size: size(px(bounds[2]), px(bounds[3])),
            });
        }
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
            (area_height - SHEET_TOOLBAR_HEIGHT - FORMULA_BAR_HEIGHT - SHEET_TABS_HEIGHT).max(1.0),
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
        let format_dialog = self.format_dialog_element(cx);
        let note = self.note.and_then(|(cell, (x, y))| {
            let text = self.book.hover_text(cell)?.to_string();
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
        let list_button = self.list_button(cx);
        let note_editor = self.noting.as_ref().and_then(|(cell, text)| {
            let grid = self.grid_bounds.get()?;
            let bounds = self.cell_bounds(*cell)?;
            Some(
                div()
                    .absolute()
                    .left(bounds.origin.x - grid.origin.x + bounds.size.width + px(6.0))
                    .top(bounds.origin.y - grid.origin.y)
                    .min_w(px(160.0))
                    .min_h(px(60.0))
                    .p_2()
                    .bg(gpui::rgb(0xfff9c4))
                    .border_1()
                    .border_color(gpui::rgb(0x217346))
                    .shadow_md()
                    .text_xs()
                    .text_color(gpui::black())
                    .child(caret_line(text, gpui::black(), false)),
            )
        });
        let find_bar = self.find_bar(cx);
        let cursor = match self.hover {
            Hover::ResizeColumn => gpui::CursorStyle::ResizeLeftRight,
            Hover::ResizeRow => gpui::CursorStyle::ResizeUpDown,
            Hover::FillHandle | Hover::Cells => gpui::CursorStyle::Crosshair,
            Hover::Link => gpui::CursorStyle::PointingHand,
            Hover::Border => gpui::CursorStyle::OpenHand,
        };
        let dragging = self.drag.is_some();
        let toolbar = self.sheet_toolbar(cx);
        let rule_dialog = self.rule_dialog(cx);
        let chart_dialog = self.chart_dialog(cx);
        let rules_manager = self.rules_manager(cx);
        let paste_special = self.paste_special_dialog(cx);
        let table_panel = self.table_panel(cx);
        let pivot_panel = self.pivot_panel(cx);
        let link_panel = self.link_panel(cx);
        let subtotal_panel = self.subtotal_panel(cx);
        let text_columns_panel = self.text_columns_panel(cx);
        let sort_panel = self.sort_panel(cx);
        let page_setup_panel = self.page_setup_panel(cx);
        let duplicates_panel = self.duplicates_panel(cx);
        let protection_panel = self.protection_panel(cx);
        let validation_panel = self.validation_panel(cx);
        let validation_alert = self.validation_alert_panel(cx);
        let validation_input = self.validation_input_panel();
        let filter_panel = self.filter_panel(cx);

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
            .on_action(cx.listener(|view, _: &Print, _window, cx| view.print(cx)))
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme::canvas())
            .text_color(theme::text())
            .child(toolbar)
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
                    .children(self.object_frame())
                    .children(note)
                    .children(note_editor)
                    .children(list_button)
                    .children(find_bar)
                    .children(filter_panel)
                    .children(menu)
                    .children(format_dialog)
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
            .children(rule_dialog)
            .children(rules_manager)
            .children(chart_dialog)
            .children(paste_special)
            .children(validation_input)
            .children(table_panel)
            .children(pivot_panel)
            .children(link_panel)
            .children(subtotal_panel)
            .children(text_columns_panel)
            .children(sort_panel)
            .children(page_setup_panel)
            .children(duplicates_panel)
            .children(protection_panel)
            .children(validation_panel)
            .children(validation_alert)
    }
}

impl WorkbookView {
    fn open(path: &Path, cx: &mut App) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let name = crate::shell::file_name(path);
        let book = if csv::is_csv(&name) {
            Workbook::open_csv(&name, &bytes, &system_time_zone(), crate::skia_backend()?)?
        } else {
            Workbook::open(&bytes, &system_time_zone(), crate::skia_backend()?)?
        };
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
            tab_start: None,
            renaming: None,
            naming: None,
            going_to: false,
            finding: None,
            noting: None,
            asking: None,
            managing: false,
            object: None,
            chart_asking: None,
            pasting: None,
            validation_dialog: None,
            link_dialog: None,
            subtotal_dialog: None,
            text_columns_dialog: None,
            sort_dialog: None,
            page_setup_dialog: None,
            duplicates_dialog: None,
            protection_dialog: None,
            table_dialog: None,
            pivot_dialog: None,
            validation_pending: None,
            choices: None,
            pending_link: None,
            format_source: None,
            format_sticky: false,
            menu: None,
            format_dialog: None,
            filter: None,
            popup: None,
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

/// Why printing failed, in the shell's words: an empty sheet as Excel
/// says it, anything else as it came.
fn print_failure(error: &str) -> String {
    if error == print::NOTHING_TO_PRINT {
        i18n::text("sheet.nothing_to_print").to_string()
    } else {
        i18n::format("sheet.print_failed", &[("error", &error)])
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

    #[gpui::test]
    fn cell_shift_menu_commands_move_only_the_selection_strip(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        view.update(cx, |view, cx| {
            for (cell, text) in [(at(7, 1), "2"), (at(7, 2), "3"), (at(8, 1), "20")] {
                view.book.set_cell(cell, text).unwrap();
            }
            view.selection = Selection::cell(at(7, 1));
            view.menu_command(MenuCommand::ShiftCells(Axis::Rows, true), false, cx);
            assert_eq!(view.book.number(at(7, 2)), Some(2.));
            assert_eq!(view.book.number(at(7, 3)), Some(3.));
            assert_eq!(view.book.number(at(8, 1)), Some(20.));
            view.step_history(true, cx);
            assert_eq!(view.book.number(at(7, 1)), Some(2.));
            assert_eq!(view.book.number(at(7, 2)), Some(3.));
            view.menu_command(MenuCommand::ShiftCells(Axis::Columns, false), false, cx);
            assert_eq!(view.book.number(at(7, 1)), Some(20.));
            assert_eq!(view.book.number(at(7, 2)), Some(3.));
        });
    }

    #[gpui::test]
    fn table_dialog_creates_totals_and_converts_with_one_undo(cx: &mut gpui::TestAppContext) {
        use jade_office_model::tables::TableField;
        let (view, cx) = open_view(cx);
        view.update(cx, |view, cx| {
            for (cell, text) in [
                (at(7, 1), "Item"),
                (at(8, 1), "Qty"),
                (at(7, 2), "A"),
                (at(8, 2), "2"),
                (at(7, 3), "B"),
                (at(8, 3), "3"),
            ] {
                view.book.set_cell(cell, text).unwrap();
            }
            view.selection = Selection::range_from(at(7, 1), at(8, 3));
            view.sheet_command(
                "t",
                Modifiers {
                    platform: true,
                    ..Default::default()
                },
                cx,
            );
            let d = view.table_dialog.as_mut().unwrap();
            d.focus(TableField::Name);
            d.field_mut().replace(None, "Sales", false);
            view.sheet_table_action(tables::TableAction::Take, cx);
            assert!(view.table_dialog.is_none());
            assert_eq!(view.book.table(at(7, 1)).unwrap().name, "Sales");
            view.selection = Selection::cell(at(8, 2));
            view.open_table_dialog(cx);
            let d = view.table_dialog.as_mut().unwrap();
            d.has_totals = true;
            d.total_kind = 1;
            view.sheet_table_action(tables::TableAction::Take, cx);
            assert!(view.table_dialog.is_none());
            assert_eq!(view.book.number(at(8, 4)), Some(5.));
            view.open_table_dialog(cx);
            view.sheet_table_action(tables::TableAction::Convert, cx);
            assert!(view.book.table(at(8, 2)).is_none());
            assert_eq!(view.book.number(at(8, 4)), Some(5.));
            view.step_history(true, cx);
            assert_eq!(view.book.table(at(8, 2)).unwrap().name, "Sales");
        });
    }

    #[gpui::test]
    fn hyperlink_dialog_edits_and_removes_a_link_without_replacing_a_formula(
        cx: &mut gpui::TestAppContext,
    ) {
        let (view, cx) = open_view(cx);
        view.update(cx, |view, cx| {
            let at = CellRef::new(7, 1);
            view.selection = Selection::cell(at);
            view.sheet_command(
                "k",
                Modifiers {
                    platform: true,
                    ..Default::default()
                },
                cx,
            );
            let dialog = view.link_dialog.as_mut().unwrap();
            dialog
                .field_mut()
                .replace(None, "https://example.com", false);
            dialog.focus(jade_office_model::links::LinkField::Label);
            dialog.field_mut().replace(None, "リンク", false);
            view.link_action(links::LinkAction::Take, cx);
            assert!(view.link_dialog.is_none());
            assert_eq!(view.book.display_text(at), "リンク");
            assert_eq!(
                view.book.link(at),
                Some(Link::Url("https://example.com".into()))
            );
            view.menu_command(MenuCommand::RemoveLinks, false, cx);
            assert!(view.book.link(at).is_none());
            assert_eq!(view.book.display_text(at), "リンク");
            view.step_history(true, cx);
            assert!(view.book.link(at).is_some());
            view.book.set_cell(at, "=1+2").unwrap();
            view.open_link_dialog(cx);
            let dialog = view.link_dialog.as_mut().unwrap();
            dialog.focus(jade_office_model::links::LinkField::Label);
            dialog.field_mut().replace(None, "保持", false);
            view.link_action(links::LinkAction::Take, cx);
            assert_eq!(view.book.edit_text(at), "=1+2");
        });
    }

    #[gpui::test]
    fn validation_ime_composition_is_not_treated_as_dialog_ok(cx: &mut gpui::TestAppContext) {
        use jade_office_model::validation::ValidationField;
        let (view, cx) = open_view(cx);
        view.update(cx, |view, cx| {
            view.selection = Selection::range_from(at(7, 1), at(7, 1));
            view.open_validation(cx);
            let dialog = view.validation_dialog.as_mut().unwrap();
            dialog.kind = 3;
            dialog.focus(ValidationField::First);
            dialog.field_mut().replace(None, "東京", true);
        });
        view.update(cx, |view, cx| {
            // IME key routing and the subsequent committed text are distinct
            // events. A synthetic plain Enter would inject a newline instead.
            let event = KeyDownEvent {
                keystroke: gpui::Keystroke::parse("enter").unwrap(),
                is_held: false,
            };
            assert!(!view.sheet_key(&event, cx));
            assert!(view.validation_dialog.is_some());
            assert_eq!(
                view.validation_dialog.as_ref().unwrap().field().text,
                "東京"
            );
            view.typing().unwrap().replace(None, "東京", false);
        });
        cx.simulate_keystrokes("enter");
        view.read_with(cx, |view, _| {
            assert!(view.validation_dialog.is_none());
            assert_eq!(
                view.book.validation_dropdown(at(7, 1)),
                Some(vec!["東京".into()])
            );
        });
    }

    #[gpui::test]
    fn validation_dialog_creates_a_list_and_clear_undo_restores_it(cx: &mut gpui::TestAppContext) {
        use jade_office_model::validation::{ValidationField, ValidationTab};
        let (view, cx) = open_view(cx);
        view.update(cx, |view, cx| {
            view.selection = Selection::range_from(at(7, 1), at(7, 3));
            view.menu_command(MenuCommand::Validation, false, cx);
            let dialog = view.validation_dialog.as_mut().unwrap();
            dialog.kind = 3;
            dialog.fields[1] = TextEdit::new("東京,大阪".into());
            dialog.select_tab(ValidationTab::Input);
            dialog.field_mut().insert("都市");
            dialog.focus(ValidationField::Prompt);
            dialog.field_mut().insert("一覧から選択してください");
            assert!(view.validation_key("enter", false, false, true, cx));
            view.validation_dialog
                .as_mut()
                .unwrap()
                .field_mut()
                .insert("候補は2件です");
            view.validation_action(validation::ValidationAction::Take, cx);
            assert!(view.validation_dialog.is_none());
            assert_eq!(
                view.book.validation_dropdown(at(7, 1)),
                Some(vec!["東京".into(), "大阪".into()])
            );
            assert_eq!(
                view.book.validation_prompt(at(7, 1)),
                Some((
                    "都市".into(),
                    "一覧から選択してください\n候補は2件です".into()
                ))
            );
            view.begin_edit(Some("福岡".into()), false, cx);
            assert!(!view.commit_cell(cx));
            assert!(view.validation_pending.is_some());
            view.answer_validation(validation::ValidationAnswer::Cancel, cx);
            assert_eq!(view.book.edit_text(at(7, 1)), "");
            view.menu_command(MenuCommand::ClearValidation, false, cx);
            assert!(view.book.data_validation_at(at(7, 1)).is_none());
            view.step_history(true, cx);
            assert!(view.book.data_validation_at(at(7, 1)).is_some());
        });
    }

    #[gpui::test]
    fn validation_warning_requires_a_choice_before_the_cell_changes(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        view.update(cx, |view, cx| {
            view.selection = Selection::range_from(at(7, 1), at(7, 1));
            view.menu_command(MenuCommand::Validation, false, cx);
            let dialog = view.validation_dialog.as_mut().unwrap();
            dialog.kind = 3;
            dialog.style = 1;
            dialog.fields[1] = TextEdit::new("東京,大阪".into());
            view.validation_action(validation::ValidationAction::Take, cx);
            view.begin_edit(Some("福岡".into()), false, cx);
            assert!(!view.commit_cell(cx));
            assert!(view.validation_pending.is_some());
            assert_eq!(view.book.edit_text(at(7, 1)), "");
            view.sheet_paste(Some("99"), cx);
            assert_eq!(view.book.edit_text(at(7, 1)), "");
            view.answer_validation(validation::ValidationAnswer::Retry, cx);
            assert!(view.edit.is_some());
            assert!(!view.commit_cell(cx));
            view.answer_validation(validation::ValidationAnswer::Accept, cx);
            assert_eq!(view.book.edit_text(at(7, 1)), "福岡");
            assert!(view.edit.is_none());
            view.step_history(true, cx);
            assert_eq!(view.book.edit_text(at(7, 1)), "");
        });
    }

    /// F5 opens Go To in the name box: a reference typed there is gone
    /// to, and one that does not exist is said so, never defined.
    #[gpui::test]
    fn f5_goes_to_a_reference_and_defines_nothing(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("f5");
        view.read_with(cx, |view, _| {
            assert!(view.going_to && view.naming.is_some())
        });
        cx.simulate_input("D9");
        cx.simulate_keystrokes("enter");
        view.update(cx, |view, cx| {
            assert_eq!(view.active(), at(3, 8));
            let names = view.book.defined_names().len();
            view.open_go_to(cx);
            view.naming = Some(TextEdit::new("NoSuchName".into()));
            view.finish_naming(true, cx);
            assert_eq!(view.book.defined_names().len(), names);
            assert_eq!(view.status.as_ref(), i18n::text("sheet.no_reference"));
            assert_eq!(view.active(), at(3, 8));
        });
    }

    /// Tab Color from the tab's menu colours the tab; No Color takes it
    /// away again.
    #[gpui::test]
    fn tab_color_from_the_tab_menu(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        view.update(cx, |view, cx| {
            let index = view.book.sheet_index();
            view.menu_command(MenuCommand::TabColor(index, Some("FF0000")), false, cx);
            assert_eq!(view.book.tab_color(index), Some([0xFF, 0, 0]));
            view.menu_command(MenuCommand::TabColor(index, None), false, cx);
            assert_eq!(view.book.tab_color(index), None);
        });
    }

    /// Typing a formula lists the functions the word starts; Down picks the
    /// next and Tab puts it in with its parenthesis, leaving the cell open.
    #[gpui::test]
    fn function_autocomplete_puts_the_name_picked_in(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("right right down down down down down down");
        cx.simulate_input("=SU");
        let second = view.read_with(cx, |view, _| {
            let edit = view.edit.as_ref().expect("typing into the cell");
            let list = formula_complete::completion(&edit.text.text, edit.text.caret)
                .expect("the list shows");
            assert_eq!(list.names[0], "SUBSTITUTE");
            list.names[1]
        });
        cx.simulate_keystrokes("down tab");
        view.read_with(cx, |view, _| {
            let edit = view.edit.as_ref().expect("still typing");
            assert_eq!(edit.text.text, format!("={second}("));
        });
        cx.simulate_input("4)");
        cx.simulate_keystrokes("escape");
        view.read_with(cx, |view, _| assert!(view.edit.is_none()));
    }

    #[gpui::test]
    fn names_and_tab_commands_work_through_the_view(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        view.update(cx, |view, cx| {
            let selected = span(at(1, 1), at(2, 3));
            view.selection = Selection::range_from(selected.start, selected.end);
            view.naming = Some(TextEdit::new("SalesRange".into()));
            view.finish_naming(true, cx);
            assert_eq!(
                view.book.resolve_reference("salesrange"),
                Some((view.book.sheet_index(), selected))
            );
            view.selection = Selection::default();
            view.naming = Some(TextEdit::new("SalesRange".into()));
            view.finish_naming(true, cx);
            assert_eq!(view.range(), selected);
            // Hide and unhide travel through the same commands as the tab menu.
            let hidden = view.book.sheet_index();
            if view.book.sheet_tabs().len() == 1 {
                view.book.add_sheet().unwrap();
                view.sheet_names = view
                    .book
                    .sheet_names()
                    .into_iter()
                    .map(Into::into)
                    .collect();
            }
            view.menu_command(MenuCommand::HideSheet(hidden), false, cx);
            assert!(!view.book.sheet_tabs().contains(&hidden));
            assert_ne!(view.book.sheet_index(), hidden);
            view.menu_command(MenuCommand::UnhideSheet(hidden), false, cx);
            assert!(view.book.sheet_tabs().contains(&hidden));
        });
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
        open_path(cx, fixture("grid-operations-workbook.xlsx"))
    }

    fn open_path(
        cx: &mut gpui::TestAppContext,
        path: PathBuf,
    ) -> (Entity<WorkbookView>, &mut gpui::VisualTestContext) {
        // The app's keymap, so ⌘C, ⌘V and the rest arrive as they do in
        // the app: as actions where it binds them, as keys where it does not.
        cx.update(|cx| cx.bind_keys(crate::key_bindings()));
        let (view, cx) = cx.add_window_view(|_window, cx| WorkbookView::open(&path, cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        (view, cx)
    }

    /// ⌘1 opens Format Cells on the active cell; Enter applies what was
    /// changed, Escape leaves the cell as it was, and on Custom the code is
    /// typed into the dialog, not the cell.
    #[gpui::test]
    fn command_1_opens_format_cells_and_enter_applies_it(cx: &mut gpui::TestAppContext) {
        use jade_office_model::format_cells::{NumberCategory, Tab};
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("down down right");
        cx.simulate_input("1234.5");
        cx.simulate_keystrokes("enter up cmd-1");
        let at = view.read_with(cx, |view, _| {
            let dialog = view.format_dialog.as_ref().expect("⌘1 opens the dialog");
            assert_eq!(dialog.number.category, NumberCategory::General);
            view.active()
        });
        // Escape cancels.
        view.update(cx, |view, _| {
            view.format_dialog.as_mut().unwrap().number.category = NumberCategory::Currency;
        });
        cx.simulate_keystrokes("escape");
        view.read_with(cx, |view, _| {
            assert!(view.format_dialog.is_none());
            assert_eq!(view.book.display_text(at), "1234.5");
        });
        // Currency and bold, then Enter.
        cx.simulate_keystrokes("cmd-1");
        view.update(cx, |view, _| {
            let dialog = view.format_dialog.as_mut().unwrap();
            dialog.number.category = NumberCategory::Currency;
            dialog.bold = true;
        });
        cx.simulate_keystrokes("enter");
        view.read_with(cx, |view, _| {
            assert!(view.format_dialog.is_none());
            assert_eq!(view.book.display_text(at), "¥1,235");
            assert_eq!(view.book.format_at(at).bold, Some(true));
        });
        // A custom code is typed into the dialog.
        cx.simulate_keystrokes("cmd-1");
        view.update(cx, |view, _| {
            let dialog = view.format_dialog.as_mut().unwrap();
            dialog.tab = Tab::Number;
            dialog.number.category = NumberCategory::Custom;
            dialog.number.pattern.clear();
        });
        cx.simulate_input("0.00");
        cx.simulate_keystrokes("enter");
        view.read_with(cx, |view, _| {
            assert_eq!(view.book.display_text(at), "1234.50");
            assert!(view.edit.is_none());
        });
        // One step each.
        cx.dispatch_action(Undo);
        view.read_with(cx, |view, _| {
            assert_eq!(view.book.display_text(at), "¥1,235")
        });
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

    /// A CSV opens as a sheet named for the file, its fields read as typed;
    /// ⌘S writes it back as CSV, and Save As to .xlsx writes a workbook
    /// that is the file from then on.
    #[gpui::test]
    fn a_csv_opens_and_saves_as_csv(cx: &mut gpui::TestAppContext) {
        let directory = std::env::temp_dir().join(format!("jade-csv-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("名簿.csv");
        std::fs::write(&path, "氏名,年齢\r\n山田太郎,42\r\n\"佐藤, 花子\",7\r\n").unwrap();
        let (view, cx) = cx.add_window_view(|_window, cx| WorkbookView::open(&path, cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        view.read_with(cx, |view, _| {
            assert_eq!(view.book.sheet_names(), vec!["名簿"]);
            assert_eq!(view.book.display_text(at(0, 2)), "佐藤, 花子");
            assert_eq!(view.book.number(at(1, 1)), Some(42.0));
        });
        cx.simulate_keystrokes("down down down");
        cx.simulate_input("鈴木");
        cx.simulate_keystrokes("enter");
        cx.dispatch_action(Save);
        let saved = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            saved,
            "\u{feff}氏名,年齢\r\n山田太郎,42\r\n\"佐藤, 花子\",7\r\n鈴木,\r\n"
        );
        let xlsx = directory.join("名簿.xlsx");
        view.update(cx, |view, cx| view.write_to(xlsx.clone(), cx));
        assert!(std::fs::read(&xlsx).unwrap().starts_with(b"PK"));
        view.read_with(cx, |view, _| assert_eq!(view.path, xlsx));
        std::fs::remove_dir_all(&directory).ok();
    }

    /// Save As to a .pdf exports the sheet's pages and leaves the workbook
    /// the file it was, unsaved changes and all.
    #[gpui::test]
    fn save_as_pdf_exports_the_pages(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        cx.simulate_input("changed");
        cx.simulate_keystrokes("enter");
        let directory = std::env::temp_dir().join(format!("jade-pdf-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let pdf = directory.join("book.pdf");
        let before = view.read_with(cx, |view, _| view.path.clone());
        view.update(cx, |view, cx| view.write_to(pdf.clone(), cx));
        let bytes = std::fs::read(&pdf).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        view.read_with(cx, |view, _| {
            assert_eq!(view.path, before);
            assert!(view.book.is_dirty(), "exporting is not saving");
        });
        std::fs::remove_dir_all(&directory).ok();
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
            assert!(view.book.copied().is_some());
        });
        cx.simulate_keystrokes("escape");
        view.read_with(cx, |view, _| assert!(view.book.copied().is_none()));
    }

    /// ⌃⌘V opens Paste Special: arrows choose Values, Transpose ticked,
    /// Return pastes the formulas' results turned on their side, as one
    /// undo; typing does not reach the cells while it is open.
    #[gpui::test]
    fn control_command_v_pastes_values_transposed(cx: &mut gpui::TestAppContext) {
        use jade_office_model::paste_special::PasteWhat;
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("down right right shift-down shift-down cmd-c");
        cx.simulate_keystrokes("right right ctrl-cmd-v");
        view.read_with(cx, |view, _| {
            assert_eq!(view.pasting, Some(PasteSpecial::default()))
        });
        cx.simulate_keystrokes("down down");
        cx.simulate_input("x");
        view.update(cx, |view, _| {
            assert!(view.edit.is_none(), "the dialog takes no typing");
            let special = view.pasting.as_mut().unwrap();
            assert_eq!(special.what, PasteWhat::Values);
            special.transpose = true;
        });
        cx.simulate_keystrokes("enter");
        view.read_with(cx, |view, _| {
            assert!(view.pasting.is_none());
            // C2:C4's =B2*2… pasted from E2 across: 20, 40, 60.
            assert_eq!(view.book.edit_text(at(4, 1)), "20");
            assert_eq!(view.book.edit_text(at(6, 1)), "60");
            assert_eq!(view.range(), span(at(4, 1), at(6, 1)));
        });
        cx.dispatch_action(Undo);
        view.read_with(cx, |view, _| {
            assert_eq!(view.book.edit_text(at(4, 1)), "");
            assert_eq!(view.book.edit_text(at(6, 1)), "");
        });
        // Escape leaves the dialog without pasting.
        cx.simulate_keystrokes("ctrl-cmd-v escape");
        view.read_with(cx, |view, _| {
            assert!(view.pasting.is_none());
            assert_eq!(view.book.edit_text(at(4, 1)), "");
        });
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
            assert!(view.book.copied().is_none(), "a cut pastes once");
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

    /// A press on a filter button (▼) opens its dropdown; a value unticked
    /// there and Enter hides its row. ⌘⇧F takes the buttons off again.
    #[gpui::test]
    fn a_filter_applied_through_its_dropdown_hides_a_row(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        // The fixture's data has a filter on it: the ▼ in A1's header cell.
        let filter_button = |view: &WorkbookView| {
            let cell = view.cell_bounds(at(0, 0)).unwrap();
            let y = cell.center().y;
            let mut x = cell.origin.x + cell.size.width - px(1.0);
            while x > cell.origin.x {
                let position = gpui::point(x, y);
                let (left, top) = view.grid_point(position).unwrap();
                if view
                    .book
                    .filter_button_at(left, top, view.sheet_scroll())
                    .is_some()
                {
                    return Some(position);
                }
                x -= px(1.0);
            }
            None
        };
        let button = view
            .read_with(cx, |view, _| filter_button(view))
            .expect("a filter button on A1");
        cx.simulate_click(button, Modifiers::none());
        let alpha = view.read_with(cx, |view, _| {
            let panel = view.filter.as_ref().expect("the dropdown is open");
            assert!(panel.checked.iter().all(|checked| *checked));
            panel
                .options
                .iter()
                .position(|option| option.label == "Alpha")
                .expect("Alpha is one of the column's values")
        });
        view.update(cx, |view, cx| {
            view.filter_command(FilterCommand::Toggle(alpha), cx)
        });
        cx.simulate_keystrokes("enter");
        view.read_with(cx, |view, _| {
            assert!(view.filter.is_none());
            // Alpha's row (2) is hidden: an arrow down from A1 skips it.
            assert_eq!(view.book.step(at(0, 0), Direction::Down), at(0, 2));
        });
        cx.simulate_keystrokes("cmd-shift-f");
        view.read_with(cx, |view, _| assert!(filter_button(view).is_none()));
    }

    /// The toolbar's Center, All Borders and Increase Decimal change the
    /// selected cell's format.
    #[gpui::test]
    fn toolbar_buttons_format_the_selection(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("down right");
        view.update(cx, |view, cx| {
            view.tool(Tool::Align(HorizontalAlignment::Center), cx);
            view.tool(Tool::Open(Popup::Borders), cx);
            assert!(view.popup == Some(Popup::Borders));
            view.tool(Tool::Borders(Borders::All), cx);
            view.tool(Tool::Decimals(true), cx);
        });
        view.read_with(cx, |view, _| {
            let look = view.book.format_at(at(1, 1));
            assert_eq!(look.horizontal, Some(HorizontalAlignment::Center));
            assert!(look.border_bottom.is_some() && look.border_left.is_some());
            assert!(view.popup.is_none(), "a choice closes the dropdown");
            assert_eq!(view.book.display_text(at(1, 1)), "10.0");
        });
        cx.dispatch_action(Undo);
        view.read_with(cx, |view, _| {
            assert_eq!(view.book.display_text(at(1, 1)), "10")
        });
    }

    /// The Number Format dropdown, as the browser's: its entries format
    /// the selection, and the button names the active cell's format.
    #[gpui::test]
    fn the_number_format_dropdown_formats_the_selection(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("down right");
        view.update(cx, |view, cx| {
            assert_eq!(
                view.book.number_format_name(at(1, 1), i18n::text),
                "sheet.format.general_name"
            );
            view.tool(Tool::Open(Popup::NumberFormat), cx);
            assert!(view.popup == Some(Popup::NumberFormat));
            view.tool(Tool::NumberFormat("sheet.format.percent"), cx);
        });
        view.read_with(cx, |view, _| {
            assert!(view.popup.is_none(), "a choice closes the dropdown");
            assert_eq!(view.book.display_text(at(1, 1)), "1000.00%");
            assert_eq!(
                view.book.number_format_name(at(1, 1), i18n::text),
                "sheet.format.percent_name"
            );
        });
        cx.dispatch_action(Undo);
        view.read_with(cx, |view, _| {
            assert_eq!(view.book.display_text(at(1, 1)), "10")
        });
    }

    /// ⌃H opens Find and Replace: what is typed goes into its fields, Tab
    /// between them; Enter finds, Replace changes the found cell, Replace
    /// All the rest in one undo; Escape closes the bar.
    #[gpui::test]
    fn control_h_finds_and_replaces(cx: &mut gpui::TestAppContext) {
        use find_bar::FindCommand;
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("ctrl-h");
        cx.simulate_input("20");
        cx.simulate_keystrokes("tab");
        cx.simulate_input("25");
        view.update(cx, |view, cx| {
            let bar = view.finding.as_ref().expect("the bar is open");
            assert!(bar.replacing);
            assert_eq!(
                (bar.find.text.as_str(), bar.replace.text.as_str()),
                ("20", "25")
            );
            assert!(view.edit.is_none(), "typing goes into the bar");
            view.find_command(FindCommand::EntireCell, cx);
        });
        cx.simulate_keystrokes("enter");
        view.update(cx, |view, cx| {
            // Replace looks in formulas: C2 shows 20 but holds =B2*2.
            assert_eq!(view.active(), at(1, 2));
            view.find_command(FindCommand::Replace, cx);
            assert_eq!(view.book.display_text(at(1, 2)), "25");
            assert_eq!(view.book.display_text(at(2, 2)), "50");
        });
        cx.dispatch_action(Undo);
        view.update(cx, |view, cx| {
            assert_eq!(view.book.display_text(at(1, 2)), "20");
            if let Some(bar) = view.finding.as_mut() {
                bar.options.entire_cell = false;
                bar.find.text = "0".into();
                bar.replace.text = "5".into();
            }
            view.find_command(FindCommand::ReplaceAll, cx);
            assert_eq!(view.book.display_text(at(1, 1)), "15");
            assert_eq!(view.book.display_text(at(1, 2)), "25");
        });
        cx.dispatch_action(Undo);
        view.read_with(cx, |view, _| {
            assert_eq!(view.book.display_text(at(1, 1)), "10", "one undo");
            assert_eq!(view.book.display_text(at(1, 2)), "20");
        });
        cx.simulate_keystrokes("escape");
        view.read_with(cx, |view, _| assert!(view.finding.is_none()));
    }

    /// Find All lists every match under the bar with the count in the
    /// status bar; a row clicked selects its cell.
    #[gpui::test]
    fn find_all_lists_the_matches_and_goes_to_one(cx: &mut gpui::TestAppContext) {
        use find_bar::FindCommand;
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("cmd-f");
        cx.simulate_input("20");
        view.update(cx, |view, cx| {
            view.find_command(FindCommand::FindAll, cx);
            let found = view
                .finding
                .as_ref()
                .and_then(|bar| bar.found.clone())
                .expect("Find All lists the matches");
            assert!(!found.is_empty());
            assert!(view.status.contains(&found.len().to_string()));
            let last = found.last().unwrap().cell;
            view.find_command(FindCommand::Go(found.len() - 1), cx);
            assert_eq!(view.active(), last);
            assert!(view.finding.is_some(), "the list stays open");
        });
    }

    /// With several cells selected, Find Next looks only there and moves
    /// the active cell through them, the selection staying.
    #[gpui::test]
    fn find_next_within_a_selection_keeps_it(cx: &mut gpui::TestAppContext) {
        use find_bar::FindCommand;
        let (view, cx) = open_view(cx);
        view.update(cx, |view, _| {
            for row in 40..44 {
                view.book.set_cell(at(7, row), "needle").unwrap();
            }
            view.selection = Selection::cell(at(7, 41));
            view.selection.extend_to(at(7, 42));
        });
        cx.simulate_keystrokes("cmd-f");
        cx.simulate_input("needle");
        view.update(cx, |view, cx| {
            let range = view.range();
            view.find_command(FindCommand::Next(false), cx);
            assert_eq!(view.active(), at(7, 42));
            view.find_command(FindCommand::Next(false), cx);
            assert_eq!(view.active(), at(7, 41), "round within the selection");
            assert_eq!(view.range(), range, "the selection stays");
        });
    }

    /// Conditional Formatting ▸ Greater Than asks for its value in a
    /// dialog filled in from the selection: what is typed goes there, not
    /// into a cell, and Enter adds the rule. A data bar applies at once;
    /// the Rules Manager lists both and Escape closes it.
    #[gpui::test]
    fn conditional_formats_come_from_the_toolbar(cx: &mut gpui::TestAppContext) {
        use jade_office_model::conditional_formatting::{NewConditionalRule, Preset, DATA_BARS};
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("down right shift-down shift-down");
        view.update(cx, |view, cx| {
            view.tool(Tool::Open(Popup::Conditional), cx);
            assert!(view.popup == Some(Popup::Conditional));
            view.tool(Tool::Conditional(Conditional::Ask(Preset::GreaterThan)), cx);
            assert!(view.popup.is_none());
            let ask = view.asking.as_ref().expect("the dialog is open");
            assert_eq!(ask.fields[0].text, "20", "the selection's average");
        });
        cx.simulate_input("15");
        view.read_with(cx, |view, _| {
            assert!(view.edit.is_none(), "typing goes into the dialog");
            assert_eq!(view.asking.as_ref().unwrap().fields[0].text, "15");
        });
        cx.simulate_keystrokes("enter");
        view.update(cx, |view, cx| {
            assert!(view.asking.is_none());
            let rules = view.book.conditional_formats();
            assert_eq!(rules.len(), 1);
            assert_eq!(rules[0].range, "B2:B4");
            assert_eq!(
                rules[0].rule,
                Some(NewConditionalRule::GreaterThan("15".into()))
            );
            view.tool(Tool::Conditional(Conditional::Apply(DATA_BARS[0])), cx);
            view.tool(Tool::Conditional(Conditional::Manage), cx);
            assert_eq!(view.book.conditional_formats().len(), 2);
            assert!(view.managing);
        });
        cx.simulate_keystrokes("escape");
        view.read_with(cx, |view, _| assert!(!view.managing));
        // Escape leaves a dialog without a rule.
        view.update(cx, |view, cx| {
            view.tool(Tool::Conditional(Conditional::Ask(Preset::TopItems)), cx);
        });
        cx.simulate_keystrokes("escape");
        view.read_with(cx, |view, _| {
            assert!(view.asking.is_none());
            assert_eq!(view.book.conditional_formats().len(), 2);
        });
    }

    /// The chart button inserts a chart over the selection and selects
    /// it; a press on it picks it up and a drag moves it; Chart Title…
    /// asks for its title; Delete removes it.
    #[gpui::test]
    fn a_chart_is_inserted_moved_retitled_and_deleted(cx: &mut gpui::TestAppContext) {
        use jade_office_model::charts::{ChartField, ChartKind};
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("shift-down shift-down shift-down shift-right shift-right");
        let id = view.update(cx, |view, cx| {
            view.tool(Tool::Open(Popup::Chart), cx);
            view.tool(Tool::Chart(ChartTool::Insert(ChartKind::Column)), cx);
            let id = view.object.expect("the new chart is selected");
            assert_eq!(view.book.chart(id).unwrap().data.as_deref(), Some("A1:C4"));
            id
        });
        // Picked up by a press inside it, then moved.
        view.update(cx, |view, cx| {
            view.object = None;
            let (x, y, width, height) = view.book.object_rect(id, view.sheet_scroll()).unwrap();
            let (x, y) = (x + width / 2.0, y + height / 2.0);
            assert!(view.object_press(x, y, cx));
            assert_eq!(view.object, Some(id));
            let Some(SheetDrag::Object {
                handle,
                start,
                bounds,
                ..
            }) = view.drag
            else {
                panic!("an object drag");
            };
            assert!(handle.is_none(), "inside it moves, not resizes");
            let now = view.object_drag(handle, start, bounds, x + 64.0, y + 40.0);
            view.drag = Some(SheetDrag::Object {
                id,
                handle,
                start,
                bounds,
                now,
            });
            view.drag_end(cx);
            let moved = view.book.object_bounds(id).unwrap();
            assert!(
                (moved.0 - bounds.0 - 64.0).abs() < 1.0 && (moved.1 - bounds.1 - 40.0).abs() < 1.0
            );
            view.chart_tool(ChartTool::Ask(ChartField::Title), cx);
            assert!(view.chart_asking.is_some());
        });
        cx.simulate_input("Sales");
        cx.simulate_keystrokes("enter");
        view.read_with(cx, |view, _| {
            assert!(view.chart_asking.is_none());
            assert!(view.edit.is_none(), "typing went into the dialog");
            assert_eq!(view.book.chart(id).unwrap().title, "Sales");
        });
        cx.simulate_keystrokes("delete");
        view.read_with(cx, |view, _| {
            assert!(view.object.is_none());
            assert!(view.book.chart(id).is_none());
        });
    }

    /// New Note opens an editor beside the cell; ⇧Enter breaks the line,
    /// Enter saves it on the cell.
    #[gpui::test]
    fn a_note_is_written_through_its_editor(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("down right");
        view.update(cx, |view, cx| {
            assert!(view.book.note(at(1, 1)).is_none());
            view.menu_command(MenuCommand::EditNote, false, cx);
        });
        cx.simulate_input("Checked");
        cx.simulate_keystrokes("shift-enter");
        cx.simulate_input("by me");
        view.read_with(cx, |view, _| {
            assert!(
                view.edit.is_none(),
                "typing goes into the note, not the cell"
            );
        });
        cx.simulate_keystrokes("enter");
        view.read_with(cx, |view, _| {
            assert!(view.noting.is_none());
            assert_eq!(view.book.note(at(1, 1)), Some("Checked\nby me"));
            assert_eq!(view.book.edit_text(at(1, 1)), "10");
        });
        view.update(cx, |view, cx| {
            view.menu_command(MenuCommand::DeleteNote, false, cx)
        });
        view.read_with(cx, |view, _| assert!(view.book.note(at(1, 1)).is_none()));
    }

    /// Text to Columns splits the selected column at the delimiter ticked
    /// and closes.
    #[gpui::test]
    fn the_text_to_columns_dialog_splits_at_the_delimiter_ticked(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        view.update(cx, |view, cx| {
            view.book.set_cell(at(7, 29), "東京,10").unwrap();
            view.book.set_cell(at(7, 30), "大阪,20").unwrap();
            view.selection = Selection::cell(at(7, 29));
            view.selection.focus = at(7, 30);
            view.open_text_columns_dialog(cx);
            assert!(view.text_columns_dialog.is_some());
            view.text_columns_action(text_columns::TextColumnsAction::Comma, cx);
            view.text_columns_action(text_columns::TextColumnsAction::Take, cx);
        });
        view.read_with(cx, |view, _| {
            assert!(view.text_columns_dialog.is_none());
            assert_eq!(view.book.display_text(at(7, 29)), "東京");
            assert_eq!(view.book.display_text(at(8, 30)), "20");
        });
    }

    /// Fixed width in Text to Columns cuts at the breaks guessed, and a
    /// click on the ruler takes one away.
    #[gpui::test]
    fn the_text_to_columns_dialog_cuts_fixed_width_fields(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        view.update(cx, |view, cx| {
            view.book.set_cell(at(7, 29), "Tokyo   1200  East").unwrap();
            view.book.set_cell(at(7, 30), "Osaka    800  West").unwrap();
            view.selection = Selection::cell(at(7, 29));
            view.open_text_columns_dialog(cx);
            view.text_columns_action(text_columns::TextColumnsAction::Fixed(true), cx);
            view.text_columns_action(text_columns::TextColumnsAction::Break(14), cx);
            let dialog = view.text_columns_dialog.as_ref().unwrap();
            assert_eq!(dialog.split.fixed_width.as_deref(), Some(&[8][..]));
            view.text_columns_action(text_columns::TextColumnsAction::Take, cx);
        });
        view.read_with(cx, |view, _| {
            assert_eq!(view.book.display_text(at(7, 30)), "Osaka");
            assert_eq!(view.book.display_text(at(8, 30)), "800  West");
        });
    }

    /// Clear Formats from the menu keeps the values; Clear All empties the
    /// cells too.
    #[gpui::test]
    fn clear_formats_and_clear_all_from_the_menu(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        view.update(cx, |view, cx| {
            view.book.set_cell(at(10, 40), "0.5").unwrap();
            let change = CellFormat {
                number_format: Some("0.00%".into()),
                ..CellFormat::default()
            };
            view.book
                .format(span(at(10, 40), at(10, 40)), &change)
                .unwrap();
            view.selection = Selection::cell(at(10, 40));
            view.menu_command(MenuCommand::ClearFormats, false, cx);
            assert_eq!(view.book.display_text(at(10, 40)), "0.5");
            view.menu_command(MenuCommand::ClearAll, false, cx);
            assert_eq!(view.book.display_text(at(10, 40)), "");
        });
    }

    /// Page Setup from the menu writes the orientation and fit picked to the
    /// sheet; Cancel leaves it as it was.
    #[gpui::test]
    fn the_page_setup_dialog_writes_to_the_sheet(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        view.update(cx, |view, cx| {
            view.menu_command(MenuCommand::PageSetup, false, cx);
            view.page_setup_action(page_setup::PageSetupAction::Landscape(true), cx);
            view.page_setup_action(page_setup::PageSetupAction::Cancel, cx);
            assert!(view.page_setup_dialog.is_none());
            let landscape = |view: &WorkbookView| {
                jade_office_model::page_setup_dialog::PageSetupDialog::open(&view.book).landscape()
            };
            assert!(!landscape(view));
            view.menu_command(MenuCommand::PageSetup, false, cx);
            view.page_setup_action(page_setup::PageSetupAction::Landscape(true), cx);
            view.page_setup_action(page_setup::PageSetupAction::Fit(0), cx);
            view.page_setup_action(page_setup::PageSetupAction::Take, cx);
            assert!(landscape(view));
            assert_eq!(view.book.page_setup().fit_to(), Some((1, 1)));
        });
    }

    /// The Sort dialog sorts the selection by the column and order picked,
    /// the header row staying on top, and closes.
    #[gpui::test]
    fn the_sort_dialog_sorts_by_the_order_picked(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        view.update(cx, |view, cx| {
            view.book.set_cell(at(7, 29), "Name").unwrap();
            view.book.set_cell(at(8, 29), "Sales").unwrap();
            for (row, (name, sales)) in [("A", "10"), ("B", "30"), ("C", "20")].iter().enumerate() {
                view.book.set_cell(at(7, 30 + row as u32), name).unwrap();
                view.book.set_cell(at(8, 30 + row as u32), sales).unwrap();
            }
            view.selection = Selection::cell(at(7, 29));
            view.selection.focus = at(8, 32);
            view.open_sort_dialog(cx);
            assert!(view
                .sort_dialog
                .as_ref()
                .is_some_and(|dialog| dialog.header));
            view.sort_action(sort::SortAction::Column(0, 1), cx);
            view.sort_action(sort::SortAction::Order(0, true), cx);
            view.sort_action(sort::SortAction::Take, cx);
        });
        view.read_with(cx, |view, _| {
            assert!(view.sort_dialog.is_none());
            assert_eq!(view.book.display_text(at(7, 29)), "Name");
            let names: Vec<String> = (30..=32)
                .map(|row| view.book.display_text(at(7, row)))
                .collect();
            assert_eq!(names, ["B", "C", "A"]);
        });
    }

    /// Remove Duplicates opens a dialog on the list, every column ticked;
    /// with one column left ticked it takes out the rows repeating it, and
    /// OK with none ticked keeps the dialog open.
    #[gpui::test]
    fn the_duplicates_dialog_compares_the_columns_ticked(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        view.update(cx, |view, cx| {
            for (row, (branch, sales)) in [
                ("Branch", "Sales"),
                ("Tokyo", "10"),
                ("Yokohama", "10"),
                ("Osaka", "20"),
            ]
            .iter()
            .enumerate()
            {
                view.book.set_cell(at(7, 29 + row as u32), branch).unwrap();
                view.book.set_cell(at(8, 29 + row as u32), sales).unwrap();
            }
            view.selection = Selection::cell(at(7, 30));
            view.menu_command(MenuCommand::RemoveDuplicates, false, cx);
            let dialog = view.duplicates_dialog.as_ref().unwrap();
            assert!(dialog.header);
            assert_eq!(dialog.compared, [true, true]);
            view.duplicates_action(duplicates::DuplicatesAction::SelectAll(false), cx);
            view.duplicates_action(duplicates::DuplicatesAction::Take, cx);
            assert!(view.duplicates_dialog.is_some());
            view.duplicates_action(duplicates::DuplicatesAction::Column(1), cx);
            view.duplicates_action(duplicates::DuplicatesAction::Take, cx);
        });
        view.read_with(cx, |view, _| {
            assert!(view.duplicates_dialog.is_none());
            let branches: Vec<String> = (29..=32)
                .map(|row| view.book.display_text(at(7, row)))
                .collect();
            assert_eq!(branches, ["Branch", "Tokyo", "Osaka", ""]);
        });
    }

    /// The Subtotal dialog totals the runs of the column it is told, with
    /// the function picked, and closes.
    #[gpui::test]
    fn the_subtotal_dialog_totals_with_the_function_picked(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        view.update(cx, |view, cx| {
            for (offset, (branch, sales)) in [
                ("支店", "売上"),
                ("東京", "10"),
                ("東京", "20"),
                ("大阪", "1"),
            ]
            .into_iter()
            .enumerate()
            {
                let row = 29 + offset as u32;
                view.book.set_cell(at(7, row), branch).unwrap();
                view.book.set_cell(at(8, row), sales).unwrap();
            }
            view.selection = Selection::cell(at(7, 29));
            view.selection.anchor = at(7, 29);
            view.selection.focus = at(8, 32);
            view.open_subtotal_dialog(cx);
            assert!(view.subtotal_dialog.is_some());
            // Average, the third function.
            view.subtotal_action(subtotal::SubtotalAction::Function(2), cx);
            view.subtotal_action(subtotal::SubtotalAction::Take, cx);
        });
        view.read_with(cx, |view, _| {
            assert!(view.subtotal_dialog.is_none());
            assert_eq!(view.book.edit_text(at(8, 32)), "=SUBTOTAL(1,I31:I32)");
            assert_eq!(view.book.display_text(at(8, 32)), "15");
        });
    }

    /// A double-click on the brush keeps painting: each selection after it
    /// takes the formats, until Escape puts the brush down.
    #[gpui::test]
    fn a_double_clicked_format_painter_paints_until_escape(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        let select = |view: &mut WorkbookView, at: CellRef, cx: &mut Context<WorkbookView>| {
            view.selection = Selection::cell(at);
            view.drag = Some(SheetDrag::Select(DragKind::Cells));
            view.drag_end(cx);
        };
        view.update(cx, |view, cx| {
            let source = at(6, 1);
            view.selection = Selection::cell(source);
            view.book
                .toggle(span(source, source), source, FontToggle::Bold)
                .unwrap();
            view.pick_up_format(true, cx);
            select(view, at(7, 1), cx);
            select(view, at(7, 3), cx);
        });
        view.read_with(cx, |view, _| {
            assert_eq!(view.book.format_at(at(7, 1)).bold, Some(true));
            assert_eq!(view.book.format_at(at(7, 3)).bold, Some(true));
            assert!(view.format_source.is_some());
        });
        cx.simulate_keystrokes("escape");
        view.update(cx, |view, cx| select(view, at(7, 5), cx));
        view.read_with(cx, |view, _| {
            assert!(view.format_source.is_none());
            assert_ne!(view.book.format_at(at(7, 5)).bold, Some(true));
        });
    }

    /// Hide on a selected whole row takes it out of view — the arrows step
    /// past it — and Unhide brings it back.
    #[gpui::test]
    fn hiding_a_selected_row(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("down down right shift-space");
        view.update(cx, |view, cx| {
            assert!(view.selection.is_whole_rows());
            view.menu_command(MenuCommand::Hide, false, cx);
        });
        view.read_with(cx, |view, _| {
            assert_eq!(view.book.step(at(1, 1), Direction::Down), at(1, 3))
        });
        view.update(cx, |view, cx| {
            view.menu_command(MenuCommand::Unhide, false, cx)
        });
        view.read_with(cx, |view, _| {
            assert_eq!(view.book.step(at(1, 1), Direction::Down), at(1, 2))
        });
    }

    /// A cell with a list validation: ⌥↓ opens its choices and a choice
    /// goes into the cell; typing something off the list is refused with
    /// Excel's words, the editor staying open.
    #[gpui::test]
    fn a_list_choice_enters_the_cell(cx: &mut gpui::TestAppContext) {
        let mut package = jade_opc::OpcPackage::open(
            &std::fs::read(fixture("grid-operations-workbook.xlsx")).unwrap(),
        )
        .unwrap();
        let sheet = package
            .part_str("xl/worksheets/sheet1.xml")
            .unwrap()
            .unwrap()
            .replace(
                "<tableParts",
                "<dataValidations count=\"1\">\
                 <dataValidation type=\"list\" allowBlank=\"1\" showErrorMessage=\"1\" sqref=\"D2:D4\"><formula1>\"Yes,No\"</formula1></dataValidation>\
                 </dataValidations><tableParts",
            );
        package.replace("xl/worksheets/sheet1.xml", sheet.into_bytes());
        let path = crate::shell::scratch_copy(
            &fixture("grid-operations-workbook.xlsx"),
            "validation.xlsx",
        );
        std::fs::write(&path, package.write().unwrap()).unwrap();
        let (view, cx) = open_path(cx, path);
        cx.simulate_keystrokes("down down right right right alt-down");
        view.read_with(cx, |view, _| {
            assert_eq!(view.active(), at(3, 2));
            assert_eq!(
                view.choices,
                Some(vec!["Yes".to_string(), "No".to_string()])
            );
        });
        view.update(cx, |view, cx| view.choose(1, cx));
        view.read_with(cx, |view, _| {
            assert!(view.choices.is_none());
            assert_eq!(view.book.edit_text(at(3, 2)), "No");
        });
        cx.simulate_input("Maybe");
        cx.simulate_keystrokes("enter");
        view.read_with(cx, |view, _| {
            assert!(view.edit.is_some(), "a refused entry stays in its editor");
            assert_eq!(view.status.as_ref(), i18n::text("sheet.validation_refused"));
            assert_eq!(view.book.edit_text(at(3, 2)), "No");
        });
    }

    /// A drag by the selection's border moves the cells, and a formula that
    /// read them follows them to where they landed.
    #[gpui::test]
    fn dragging_the_border_moves_cells_and_formulas_follow(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("down right right right right");
        cx.simulate_input("=B2*10");
        cx.simulate_keystrokes("enter up left left left");
        view.read_with(cx, |view, _| assert_eq!(view.active(), at(1, 1)));
        // B2's left edge, just inside: its border.
        let border = view.read_with(cx, |view, _| {
            let cell = view.cell_bounds(at(1, 1)).unwrap();
            let position = gpui::point(cell.origin.x + px(1.0), cell.center().y);
            let (x, y) = view.grid_point(position).unwrap();
            assert!(view
                .book
                .on_selection_border(x, y, view.sheet_scroll(), view.range()));
            position
        });
        let to = cell_center(&view, cx, at(7, 5));
        cx.simulate_mouse_down(border, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(to, Some(MouseButton::Left), Modifiers::none());
        view.read_with(cx, |view, _| {
            assert_eq!(view.overlay().preview, Some(span(at(7, 5), at(7, 5))))
        });
        cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
        view.read_with(cx, |view, _| {
            assert!(view.drag.is_none());
            assert_eq!(view.book.edit_text(at(7, 5)), "10");
            assert_eq!(view.book.edit_text(at(1, 1)), "");
            assert_eq!(view.book.edit_text(at(4, 1)), "=H6*10");
            assert_eq!(view.book.display_text(at(4, 1)), "100");
            assert_eq!(view.active(), at(7, 5), "the selection goes with them");
        });
    }
}

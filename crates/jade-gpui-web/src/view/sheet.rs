//! A workbook in the browser, worked as Excel is worked: click, drag or
//! Shift-click a range, a header for whole rows or columns, the corner for
//! the sheet; ⌘-arrows to the ends of the data; Enter and Tab round a
//! range; type, with arrows that leave the cell — or point at one while a
//! formula is being typed — and F2 to edit in place; the fill handle, ⌘D,
//! ⌘R and ⌘Enter; copies that paste formulas and formats and cuts that
//! move; bold and the number-format shortcuts; rows and columns inserted,
//! deleted and resized; sheets added, renamed and deleted.
//!
//! What each of those does to the workbook is `jade-office-model`'s
//! (`sheet::Workbook`, `grid::Selection`), the same code the native app
//! runs; this is the gpui around it.

use jade_web::links::LinkDialog;
use jade_web::pivots::PivotPanel;
use jade_web::protected_edits::{attempt, ProtectedEdit};
use jade_web::protection::ProtectionDialog;
use jade_web::tables::TableDialog;
use std::collections::HashMap;
use std::time::Duration;

use gpui::{DispatchPhase, MouseUpEvent, Point};
use jade_web::grid::{self, Direction, DragKind, Hit, Selection};
use jade_web::number_formats::NUMBER_FORMATS;
use jade_web::sheet::{
    column_label, range_text, reference_text, span, AutoFilterOption, Axis, Borders, CellFormat,
    CellRange, FilterButton, FontToggle, Freeze, HorizontalAlignment, Link, OutlineChange, Overlay,
    ResizeGesture, SheetVisibility, MAX_COLUMN, MAX_ROW,
};

use super::*;

mod charts;
mod conditional;
mod duplicates;
mod find_bar;
mod links;
mod paste_special;
mod pivots;
mod protection;
mod save_as;
mod sort;
mod subtotal;
mod tables;
mod text_columns;
mod validation;
use charts::ChartTool;
use conditional::Conditional;
use jade_web::charts::{ChartAsk, Handle};
use jade_web::conditional_formatting::RuleAsk;
use jade_web::find::FindBar;
use jade_web::paste_special::PasteSpecial;
use jade_web::validation::{PendingValidation, ValidationDialog};

/// How far past the grid's edge the pointer has to be for a drag to scroll
/// it, and how often it scrolls while held there.
const AUTO_SCROLL_TICK: Duration = Duration::from_millis(40);
/// The formatting strip over the formula bar: Excel's Home tab, short.
const SHEET_TOOLBAR_HEIGHT: f32 = 34.0;

mod format_dialog;

/// Excel's standard colours, the ones under its colour buttons.
const STANDARD_COLORS: &[&str] = &[
    "C00000", "FF0000", "FFC000", "FFFF00", "92D050", "00B050", "00B0F0", "0070C0", "002060",
    "7030A0", "000000", "7F7F7F", "D9D9D9", "FFFFFF",
];

/// A workbook open in the view.
pub(super) struct Sheet {
    pub book: jade_web::Workbook,
    pub names: Vec<SharedString>,
    pub selection: Selection,
    pub edit: Option<CellEdit>,
    pub scroll: ScrollHandle,
    painted: Option<PaintedSheet>,
    /// The cells as tiles and the header bands, kept between frames.
    tiles: SheetTiles,
    /// The grid's size in window px, from the last frame.
    viewport: (f32, f32),
    drag: Option<SheetDrag>,
    /// The pointer during a drag, in window px, for scrolling while it is
    /// held past the grid's edge.
    pointer: Option<Point<Pixels>>,
    auto_scrolling: bool,
    /// The column Tab started from, where Enter goes back to.
    tab_start: Option<u32>,
    /// A sheet tab being renamed, with what has been typed.
    pub renaming: Option<(usize, TextEdit)>,
    /// The name box being typed into: a cell or range to go to.
    pub naming: Option<TextEdit>,
    /// The name box was opened by Go To (F5, Ctrl+G): what is typed is
    /// only gone to, never defined as a name.
    pub going_to: bool,
    /// Find and Replace (⌘F, Ctrl+H): what is typed into it, and its
    /// options.
    pub finding: Option<FindBar>,
    /// A note being written on a cell.
    pub noting: Option<(CellRef, TextEdit)>,
    /// Save As (⌘⇧S): the file's name being typed.
    pub saving_as: Option<TextEdit>,
    /// A conditional format's dialog, asking for its value.
    pub asking: Option<RuleAsk>,
    /// The conditional formatting Rules Manager, open.
    pub managing: bool,
    /// The selected chart, picture or shape, by id.
    pub object: Option<u32>,
    /// Chart Title… or Select Data…, asking.
    pub chart_asking: Option<ChartAsk>,
    /// The Paste Special dialog, open, with its choices.
    pub pasting: Option<PasteSpecial>,
    pub(super) validation_dialog: Option<ValidationDialog>,
    pub(super) link_dialog: Option<LinkDialog>,
    pub(super) subtotal_dialog: Option<jade_web::subtotal::SubtotalDialog>,
    pub(super) text_columns_dialog: Option<jade_web::text_columns::TextColumnsDialog>,
    pub(super) sort_dialog: Option<jade_web::sort_dialog::SortDialog>,
    pub(super) duplicates_dialog: Option<jade_web::duplicates::DuplicatesDialog>,
    pub(super) protection_dialog: Option<ProtectionDialog>,
    pub(super) table_dialog: Option<TableDialog>,
    pub(super) pivot_dialog: Option<PivotPanel>,
    pub(super) validation_pending: Option<PendingValidation>,
    /// The note under the pointer, and where its cell's top-right corner
    /// is in the grid's px.
    note: Option<(CellRef, (f32, f32))>,
    menu: Option<SheetMenu>,
    /// A filter button's dropdown.
    filter: Option<FilterPanel>,
    /// The active cell's list of choices (its data validation), open.
    choices: Option<Vec<String>>,
    /// A toolbar button's dropdown: colours or borders.
    popup: Option<Popup>,
    /// The Format Cells dialog (⌘1), while it is open.
    format_dialog: Option<jade_web::format_cells::FormatCellsDialog>,
    /// What the pointer is over, for its cursor.
    hover: Hover,
    /// A hyperlink pressed: followed when the press lets go on it, as in
    /// Excel (a press held into a drag selects instead).
    pending_link: Option<(CellRef, Link)>,
    /// Format Painter: the cells whose formats the next selection takes.
    format_source: Option<CellRange>,
    /// Whether the painter stays on until Escape (a double-click on it).
    format_sticky: bool,
}

/// A cell being typed into.
pub(super) struct CellEdit {
    pub text: TextEdit,
    /// Started with F2, a double-click or the formula bar: the arrow keys
    /// move in the text rather than leaving the cell.
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

#[derive(Clone, Copy)]
enum SheetDrag {
    Select(DragKind),
    Fill {
        source: CellRange,
        target: CellRange,
    },
    Resize(ResizeGesture),
    Point,
    /// The selection picked up by its border: where the pointer took it
    /// (offset from its top-left), where it would land, and whether ⌥ copies.
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

#[derive(Clone, Copy, PartialEq)]
enum Hover {
    Cells,
    FillHandle,
    Border,
    Link,
    ResizeColumn,
    ResizeRow,
}

struct SheetMenu {
    /// Where it opened, in the grid's px.
    at: (f32, f32),
    /// Opened on a sheet tab: its index.
    tab: Option<usize>,
}

/// The dropdown a filter button opens: sorts, a clear, and the column's
/// values to tick.
struct FilterPanel {
    button: FilterButton,
    options: Vec<AutoFilterOption>,
    checked: Vec<bool>,
    /// Under the button, in the grid's px.
    at: (f32, f32),
}

/// A sheet viewport's pixels and what they were painted for.
struct PaintedSheet {
    scroll: (f32, f32),
    device: (u32, u32),
    zoom: f32,
    overlay: Overlay,
    generation: u64,
    image: Arc<RenderImage>,
}

/// Side of a tile of cells, in device px.
const SHEET_TILE: u32 = 512;

/// The selection's wash and border, as `jade_xlsx_render`'s default
/// `RenderOptions` paints them.
const SELECTION_FILL: gpui::Rgba = gpui::Rgba {
    r: 47.0 / 255.0,
    g: 85.0 / 255.0,
    b: 151.0 / 255.0,
    a: 0.12,
};
const SELECTION_BORDER: u32 = 0x2f5597;

/// What the cell tiles were painted for: a sheet, at so many device px per
/// sheet px. Within that, an edit repaints the tiles it changed.
#[derive(Clone, Copy, PartialEq)]
struct TileKey {
    sheet: usize,
    scale: f64,
}

/// What the header bands were painted for.
#[derive(Clone, Copy, PartialEq)]
struct BandKey {
    tiles: TileKey,
    revision: u64,
    scroll: (f64, f64),
    device: (u32, u32),
    selection: Selection,
}

/// The sheet in pieces that outlive a frame: its cells as tiles fixed in
/// sheet space, which scrolling only moves, so a frame paints just the tiles
/// it brings into view; and the header bands, small enough to repaint when
/// they change. The selection is drawn over the tiles rather than into them,
/// so moving it paints nothing but the bands' highlight.
#[derive(Default)]
struct SheetTiles {
    key: Option<TileKey>,
    tiles: HashMap<(i64, i64), Arc<RenderImage>>,
    bands: Option<(BandKey, Arc<RenderImage>, Arc<RenderImage>)>,
}

impl SheetTiles {
    /// The grid as layers over a viewport of `size` window px scrolled to
    /// `scroll` device px: each pane's tiles with the selection over them,
    /// then the header bands. Paints what is not painted yet, and lets go of
    /// tiles well out of view.
    #[allow(clippy::too_many_arguments)]
    fn layers(
        &mut self,
        book: &mut jade_web::Workbook,
        scroll: (f64, f64),
        size: (f32, f32),
        scale: f32,
        zoom: f32,
        selection: Selection,
        retired: &mut Vec<Arc<RenderImage>>,
    ) -> Option<Vec<gpui::AnyElement>> {
        let (scale, zoom) = (f64::from(scale), f64::from(zoom));
        // Device px per sheet px, and sheet px per tile.
        let density = scale * zoom;
        let span = f64::from(SHEET_TILE) / density;
        let key = TileKey {
            sheet: book.sheet_index(),
            scale: density,
        };
        // What the sheet changed since the last frame: the tiles over it go.
        let changes = book.take_changes();
        if self.key != Some(key) || changes.is_none() {
            retired.extend(self.tiles.drain().map(|(_, tile)| tile));
            self.key = Some(key);
        } else if let Some(changes) = changes.filter(|changes| !changes.is_empty()) {
            let touched: Vec<_> = self
                .tiles
                .keys()
                .copied()
                .filter(|&(column, row)| {
                    let (x, y) = (column as f64 * span, row as f64 * span);
                    changes.iter().any(|&(cx, cy, cw, ch)| {
                        cx < x + span && x < cx + cw && cy < y + span && y < cy + ch
                    })
                })
                .collect();
            for at in touched {
                retired.extend(self.tiles.remove(&at));
            }
        }
        let scroll = (scroll.0 / density, scroll.1 / density);
        let view = (f64::from(size.0) / zoom, f64::from(size.1) / zoom);
        let panes = book.panes(scroll, view);

        // The tiles each pane shows, painting the ones missing.
        let mut shown = Vec::new();
        for &(_, (x, y, width, height)) in &panes {
            let columns = (x / span).floor() as i64..=((x + width) / span).ceil() as i64 - 1;
            let rows = (y / span).floor() as i64..=((y + height) / span).ceil() as i64 - 1;
            for row in rows.clone() {
                for column in columns.clone() {
                    if let std::collections::hash_map::Entry::Vacant(slot) =
                        self.tiles.entry((column, row))
                    {
                        slot.insert(
                            book.render_cells(
                                (column as f64 * span, row as f64 * span, span, span),
                                SHEET_TILE,
                                SHEET_TILE,
                                density,
                            )
                            .and_then(image)?,
                        );
                    }
                }
            }
            shown.push((columns, rows));
        }
        // A tile a scroll back would bring in again is kept; one further off
        // goes.
        let near = |&(column, row): &(i64, i64)| {
            shown.iter().any(|(columns, rows)| {
                (columns.start() - 1..=columns.end() + 1).contains(&column)
                    && (rows.start() - 1..=rows.end() + 1).contains(&row)
            })
        };
        let far: Vec<_> = self.tiles.keys().copied().filter(|at| !near(at)).collect();
        for at in far {
            retired.extend(self.tiles.remove(&at));
        }

        let shape = book.selection_shape(selection);
        let tile_size = px((f64::from(SHEET_TILE) / scale) as f32);
        let mut layers: Vec<gpui::AnyElement> = Vec::new();
        for (&((left, top), (x, y, width, height)), (columns, rows)) in panes.iter().zip(&shown) {
            // Sheet px → window px from the pane's top-left.
            let at = |value: f64, origin: f64| px(((value - origin) * zoom) as f32);
            let mut pane = div()
                .absolute()
                .left(px((left * zoom) as f32))
                .top(px((top * zoom) as f32))
                .w(px((width * zoom) as f32))
                .h(px((height * zoom) as f32))
                .overflow_hidden();
            for row in rows.clone() {
                for column in columns.clone() {
                    if let Some(tile) = self.tiles.get(&(column, row)) {
                        pane = pane.child(
                            img(tile.clone())
                                .absolute()
                                .left(at(column as f64 * span, x))
                                .top(at(row as f64 * span, y))
                                .w(tile_size)
                                .h(tile_size),
                        );
                    }
                }
            }
            if let Some(((rx, ry, rw, rh), (ax, ay, aw, ah))) = shape {
                // The wash, leaving the active cell clear.
                let strips = [
                    (rx, ry, rw, ay - ry),
                    (rx, ay + ah, rw, ry + rh - (ay + ah)),
                    (rx, ay, ax - rx, ah),
                    (ax + aw, ay, rx + rw - (ax + aw), ah),
                ];
                for (sx, sy, sw, sh) in strips {
                    if sw > 0.0 && sh > 0.0 {
                        pane = pane.child(
                            div()
                                .absolute()
                                .left(at(sx, x))
                                .top(at(sy, y))
                                .w(px((sw * zoom) as f32))
                                .h(px((sh * zoom) as f32))
                                .bg(SELECTION_FILL),
                        );
                    }
                }
                // A 2 px border centred on the range's edge, and the fill
                // handle on its corner.
                pane = pane
                    .child(
                        div()
                            .absolute()
                            .left(at(rx - 1.0, x))
                            .top(at(ry - 1.0, y))
                            .w(px(((rw + 2.0) * zoom) as f32))
                            .h(px(((rh + 2.0) * zoom) as f32))
                            .border(px((2.0 * zoom) as f32))
                            .border_color(rgb(SELECTION_BORDER)),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(at(rx + rw - 3.0, x))
                            .top(at(ry + rh - 3.0, y))
                            .size(px((6.0 * zoom) as f32))
                            .bg(rgb(SELECTION_BORDER)),
                    );
            }
            layers.push(pane.into_any_element());
        }

        // The header bands, over the panes' edges: the rows', then the
        // columns' with the corner over it.
        let metrics = book.header_metrics();
        let device = (
            (f64::from(size.0) * scale).round() as u32,
            (f64::from(size.1) * scale).round() as u32,
        );
        let below = (metrics.column_height * density).round() as u32;
        let rows_height = device.1.saturating_sub(below).max(1);
        let band_key = BandKey {
            tiles: key,
            revision: book.layout_revision(),
            scroll,
            device,
            selection,
        };
        if self
            .bands
            .as_ref()
            .is_none_or(|(known, ..)| *known != band_key)
        {
            let columns = book
                .render_column_headers(scroll, view, device.0, density, Some(selection))
                .and_then(image)?;
            let rows = book
                .render_row_headers(scroll, view, rows_height, density, Some(selection))
                .and_then(image)?;
            if let Some((_, old_columns, old_rows)) = self.bands.replace((band_key, columns, rows))
            {
                retired.push(old_columns);
                retired.push(old_rows);
            }
        }
        if let Some((_, columns, rows)) = &self.bands {
            // Each band at its own pixel size.
            let shown = |image: &Arc<RenderImage>| {
                let size = image.size(0);
                (
                    px((f64::from(size.width.0) / scale) as f32),
                    px((f64::from(size.height.0) / scale) as f32),
                )
            };
            let (width, height) = shown(rows);
            layers.push(
                img(rows.clone())
                    .absolute()
                    .left_0()
                    .top(px((f64::from(below) / scale) as f32))
                    .w(width)
                    .h(height)
                    .into_any_element(),
            );
            let (width, height) = shown(columns);
            layers.push(
                img(columns.clone())
                    .absolute()
                    .left_0()
                    .top_0()
                    .w(width)
                    .h(height)
                    .into_any_element(),
            );
        }
        Some(layers)
    }

    /// Everything painted, to be let go of.
    fn clear(&mut self, retired: &mut Vec<Arc<RenderImage>>) {
        retired.extend(self.tiles.drain().map(|(_, tile)| tile));
        if let Some((_, columns, rows)) = self.bands.take() {
            retired.push(columns);
            retired.push(rows);
        }
        self.key = None;
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Popup {
    NumberFormat,
    Fill,
    Font,
    Borders,
    Conditional,
    Chart,
}

/// What a toolbar button does.
#[derive(Clone, Copy)]
enum Tool {
    Font(FontToggle),
    Align(HorizontalAlignment),
    Wrap,
    Merge,
    Currency,
    Percent,
    Comma,
    NumberFormat(&'static str),
    Decimals(bool),
    FontSize(bool),
    Open(Popup),
    Fill(Option<&'static str>),
    FontColor(&'static str),
    Borders(Borders),
    Filter,
    Conditional(Conditional),
    Chart(ChartTool),
}

#[derive(Clone, Copy)]
enum FilterCommand {
    Toggle(usize),
    All,
    Apply,
    Cancel,
    Sort(bool),
    Clear,
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
    MoveSheet(usize, usize),
    EditNote,
    DeleteNote,
    Hide,
    Unhide,
    FreezePanes,
    UnfreezePanes,
    Outline(OutlineChange),
    FormatPainter,
    AutoOutline,
    ClearOutline,
    Subtotal,
    RemoveDuplicates,
    TextToColumns,
    SortDialog,
    ManualCalculation(bool),
    CalculateNow,
    DeleteSheet(usize),
    HideSheet(usize),
    UnhideSheet(usize),
    /// Tab Color: an `RRGGBB` for the sheet's tab, or none.
    TabColor(usize, Option<&'static str>),
}

impl Sheet {
    pub fn open(book: jade_web::Workbook) -> Self {
        Self {
            names: book
                .sheet_names()
                .into_iter()
                .map(SharedString::from)
                .collect(),
            book,
            selection: Selection::default(),
            edit: None,
            scroll: ScrollHandle::new(),
            painted: None,
            tiles: SheetTiles::default(),
            viewport: (1.0, 1.0),
            drag: None,
            pointer: None,
            auto_scrolling: false,
            tab_start: None,
            renaming: None,
            naming: None,
            going_to: false,
            finding: None,
            noting: None,
            saving_as: None,
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
            duplicates_dialog: None,
            protection_dialog: None,
            table_dialog: None,
            pivot_dialog: None,
            validation_pending: None,
            note: None,
            menu: None,
            filter: None,
            choices: None,
            popup: None,
            format_dialog: None,
            hover: Hover::Cells,
            pending_link: None,
            format_source: None,
            format_sticky: false,
        }
    }

    /// The text being typed into: a tab's new name, the name box, or a cell.
    pub fn typing(&mut self) -> Option<&mut TextEdit> {
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
        if let Some(text) = self.saving_as.as_mut() {
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

    pub fn typed(&self) -> Option<&TextEdit> {
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
        if let Some(text) = self.saving_as.as_ref() {
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
    pub fn typed_into(&mut self) {
        if let Some(edit) = self.edit.as_mut() {
            edit.point = None;
        }
    }

    /// The status bar's words: the sheet, the range, Excel's summary.
    pub fn status_items(&self, items: &mut Vec<SharedString>) {
        items.push(i18n::format("sheet.position", &[("name", &self.book.sheet_name())]).into());
        let range = self.selection.range();
        items.push(range_label(range).into());
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
    }

    fn pointable(&self) -> bool {
        self.edit.as_ref().is_some_and(|edit| {
            edit.point.is_some()
                || (!edit.in_place && grid::can_point(&edit.text.text, edit.text.caret()))
        })
    }

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

/// Today's date, or the time now, as typed into a cell (`2026/10/19`,
/// `9:41`), for ⌃; and ⌘;.
fn now_text(time: bool) -> String {
    let now = js_sys::Date::new_0();
    if time {
        format!("{}:{:02}", now.get_hours(), now.get_minutes())
    } else {
        format!(
            "{}/{}/{}",
            now.get_full_year(),
            now.get_month() + 1,
            now.get_date()
        )
    }
}

impl DocView {
    fn sheet_mut(&mut self) -> Option<&mut Sheet> {
        match &mut self.document {
            Document::Workbook(sheet) => Some(sheet),
            _ => None,
        }
    }

    fn sheet_ref(&self) -> Option<&Sheet> {
        match &self.document {
            Document::Workbook(sheet) => Some(sheet),
            _ => None,
        }
    }

    /// The scroll position in sheet px (the offset is in screen px).
    fn sheet_scroll(scroll: &ScrollHandle, zoom: f32) -> (f64, f64) {
        let offset = scroll.offset();
        let zoom = f64::from(zoom);
        (
            (-f64::from(offset.x) / zoom).max(0.0),
            (-f64::from(offset.y) / zoom).max(0.0),
        )
    }

    /// A window point in the grid's sheet px (headers included).
    fn grid_point(&self, position: Point<Pixels>) -> Option<(f64, f64)> {
        let bounds = self.surface.get()?;
        let zoom = f64::from(self.shown_zoom);
        Some((
            f64::from(position.x - bounds.origin.x) / zoom,
            f64::from(position.y - bounds.origin.y) / zoom,
        ))
    }

    /// Scrolls just enough to show `to`, growing the grid to reach it.
    fn reveal_cell(&mut self, to: CellRef) {
        let zoom = self.shown_zoom;
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        sheet.book.reach(to);
        let at = Self::sheet_scroll(&sheet.scroll, zoom);
        let zoom = f64::from(zoom);
        let (x, y) = sheet.book.reveal(
            to,
            at,
            (
                f64::from(sheet.viewport.0) / zoom,
                f64::from(sheet.viewport.1) / zoom,
            ),
        );
        if (x, y) != at {
            sheet
                .scroll
                .set_offset(point(px((-x * zoom) as f32), px((-y * zoom) as f32)));
        }
    }

    /// The selection becomes `to` — a single cell — or, extending, keeps
    /// its anchor and active cell and reaches `to`.
    fn select_cell(&mut self, to: CellRef, extend: bool, cx: &mut Context<Self>) {
        if !self.sheet_ref().is_some_and(|s| s.book.can_select_cell(to)) {
            return;
        }
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let previous_selection = sheet.selection;
        if extend {
            sheet.selection.extend_to(to);
        } else {
            sheet.selection = Selection::cell(to);
        }
        if !sheet.book.can_select_range(sheet.selection.range()) {
            sheet.selection = previous_selection;
            return;
        }
        sheet.tab_start = None;
        self.reveal_cell(to);
        cx.notify();
    }

    /// An arrow: the active cell, or with Shift the far corner, one step.
    fn step_cell(&mut self, columns: i64, rows: i64, extend: bool, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_ref() else {
            return;
        };
        let from = if extend {
            sheet.selection.focus
        } else {
            sheet.selection.active
        };
        self.select_cell(grid::offset(from, columns, rows), extend, cx);
    }

    /// An arrow: one cell that way, over merged cells and past hidden rows
    /// and columns; with Shift the far corner moves.
    fn arrow_cell(&mut self, direction: Direction, extend: bool, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_ref() else {
            return;
        };
        let from = if extend {
            sheet.selection.focus
        } else {
            sheet.selection.active
        };
        let to = sheet.book.step(from, direction);
        self.select_cell(to, extend, cx);
    }

    /// The filter dropdown's OK: only the ticked values show — or, all
    /// ticked, the column is not filtered.
    fn apply_filter(&mut self, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let Some(panel) = sheet.filter.take() else {
            return;
        };
        let result = if panel.checked.iter().all(|checked| *checked) {
            sheet.book.clear_filter(&panel.button)
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
            sheet.book.filter_values(&panel.button, values, blank)
        };
        self.edited(result, cx);
    }

    fn filter_command(&mut self, command: FilterCommand, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        match command {
            FilterCommand::Toggle(index) => {
                if let Some(panel) = sheet.filter.as_mut() {
                    if let Some(checked) = panel.checked.get_mut(index) {
                        *checked = !*checked;
                    }
                }
                cx.notify();
            }
            FilterCommand::All => {
                if let Some(panel) = sheet.filter.as_mut() {
                    let all = panel.checked.iter().all(|checked| *checked);
                    panel.checked.iter_mut().for_each(|checked| *checked = !all);
                }
                cx.notify();
            }
            FilterCommand::Apply => self.apply_filter(cx),
            FilterCommand::Cancel => {
                sheet.filter = None;
                cx.notify();
            }
            FilterCommand::Sort(descending) => {
                if let Some(panel) = sheet.filter.take() {
                    let result = sheet.book.sort_filter(&panel.button, descending);
                    self.edited(result, cx);
                }
            }
            FilterCommand::Clear => {
                if let Some(panel) = sheet.filter.take() {
                    let result = sheet.book.clear_filter(&panel.button);
                    self.edited(result, cx);
                }
            }
        }
    }

    /// A toolbar button pressed, on the selection.
    fn tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let range = sheet.selection.range();
        let active = sheet.selection.active;
        if let Tool::Open(popup) = tool {
            sheet.popup = (sheet.popup != Some(popup)).then_some(popup);
            cx.notify();
            return;
        }
        sheet.popup = None;
        let result = match tool {
            Tool::Font(style) => sheet.book.toggle(range, active, style),
            Tool::Align(horizontal) => sheet.book.align(range, active, horizontal),
            Tool::Wrap => sheet.book.toggle_wrap(range, active),
            Tool::Merge => return self.sheet_menu_command(MenuCommand::MergeCenter, false, cx),
            Tool::Currency => {
                return self.sheet_number_format(i18n::text("sheet.format.currency"), cx)
            }
            Tool::Percent => return self.sheet_number_format("0%", cx),
            Tool::Comma => return self.sheet_number_format("#,##0", cx),
            Tool::NumberFormat(key) => return self.sheet_number_format(i18n::text(key), cx),
            Tool::Decimals(more) => sheet.book.decimals(range, active, more),
            Tool::FontSize(larger) => sheet.book.grow_font(range, active, larger),
            Tool::Fill(color) => sheet.book.fill_color(range, color),
            Tool::FontColor(color) => sheet.book.font_color(range, color),
            Tool::Borders(borders) => sheet.book.borders(range, borders),
            Tool::Filter => sheet.book.toggle_filter(range),
            Tool::Conditional(tool) => return self.conditional(tool, cx),
            Tool::Chart(tool) => return self.chart_tool(tool, cx),
            Tool::Open(_) => unreachable!(),
        };
        self.edited(result, cx);
    }

    /// The formatting strip: font styles, alignment, wrap and merge, number
    /// formats, decimals, font size, colours, borders and the filter —
    /// lit where the active cell has them.
    fn sheet_toolbar(&self, cx: &mut Context<Self>) -> gpui::Div {
        let Some(sheet) = self.sheet_ref() else {
            return div();
        };
        let look = sheet.book.format_at(sheet.selection.active);
        let open = sheet.popup;
        // The open dropdown hangs under its button, painted over the grid.
        let mut dropdown = self.toolbar_popup(cx).map(gpui::deferred);
        // Only the open one's button takes it: every other button leaves it
        // for the one it belongs to.
        let mut hang = move |which: Popup, button: gpui::Stateful<gpui::Div>| {
            let button = button.relative();
            let mine = if open == Some(which) {
                dropdown.take()
            } else {
                None
            };
            match mine {
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
                .text_color(palette::text())
                .when(on, |button| button.bg(palette::selected()))
                .hover(|button| button.bg(palette::hovered()))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |view, _event, window, cx| {
                    window.focus(&view.focus, cx);
                    view.tool(tool, cx);
                }))
        };
        let icon = |name: &'static str| svg().path(name).size_3p5().text_color(palette::text());
        let separator = || div().w(px(1.0)).h(px(18.0)).mx_1().bg(palette::border());
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
        let popup = open;
        div()
            .h(px(SHEET_TOOLBAR_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .gap_0p5()
            .px_2()
            .bg(palette::panel())
            .border_b_1()
            .border_color(palette::border())
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
                    .when(sheet.format_source.is_some(), |button| {
                        button.bg(palette::selected())
                    })
                    .hover(|button| button.bg(palette::hovered()))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    // A double-click keeps painting until Escape, as Excel's
                    // brush does.
                    .on_click(
                        cx.listener(move |view, event: &gpui::ClickEvent, window, cx| {
                            window.focus(&view.focus, cx);
                            view.pick_up_sheet_format(event.click_count() >= 2, cx);
                        }),
                    )
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
                button(
                    "tool-number-format",
                    open == Some(Popup::NumberFormat),
                    Tool::Open(Popup::NumberFormat),
                )
                .gap_1()
                .min_w(px(72.0))
                .justify_between()
                // Named for the active cell's format, as Excel's box is.
                .child(i18n::text(
                    sheet
                        .book
                        .number_format_name(sheet.selection.active, i18n::text),
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
                // The colour under the bucket and the A is the one last
                // seen on the active cell, as Excel's buttons show theirs.
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
                        .bg(fill.map_or(palette::dim(), rgb)),
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
                        .bg(ink.map_or(palette::text(), rgb)),
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

    /// A colour or borders dropdown under its toolbar button.
    fn toolbar_popup(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let popup = self.sheet_ref()?.popup?;
        let sheet_active = self.sheet_ref()?.selection.active;
        let swatch = |index: usize, color: &'static str, tool: Tool| {
            div()
                .id(("swatch", index))
                .size(px(18.0))
                .rounded_sm()
                .border_1()
                .border_color(palette::border())
                .bg(rgb(u32::from_str_radix(color, 16).unwrap_or(0)))
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
                .hover(|item| item.bg(palette::hovered()))
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
            Popup::NumberFormat => {
                let active = sheet_active;
                div().py_1().children(NUMBER_FORMATS.iter().map(|choice| {
                    let code = i18n::text(choice.code);
                    // Each with what the active cell would read as in it.
                    let sample = self
                        .sheet_ref()
                        .and_then(|sheet| sheet.book.number_format_sample(active, code));
                    item(choice.id, choice.name, Tool::NumberFormat(choice.code))
                        .flex()
                        .justify_between()
                        .gap_4()
                        .children(sample.map(|sample| {
                            div()
                                .max_w(px(140.0))
                                .overflow_hidden()
                                .text_color(palette::dim())
                                .child(sample)
                        }))
                }))
            }
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
                .border_color(palette::border())
                .bg(palette::panel())
                .shadow_lg()
                .text_xs()
                .text_color(palette::text())
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(body),
        )
    }

    /// AutoSum (⌘⇧T, Alt+= elsewhere): `=SUM(` round the numbers above or
    /// beside, typed into the active cell and pointed, for arrows to adjust.
    fn autosum(&mut self, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let active = sheet.selection.active;
        let range = sheet.book.autosum_range(active);
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
        sheet.edit = Some(CellEdit {
            text,
            in_place: false,
            point,
        });
        cx.notify();
    }

    /// ⌘-arrow: to the end of the data that way.
    fn jump_cell(&mut self, direction: Direction, extend: bool, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_ref() else {
            return;
        };
        let from = if extend {
            sheet.selection.focus
        } else {
            sheet.selection.active
        };
        let to = sheet.book.jump(from, direction);
        self.select_cell(to, extend, cx);
    }

    /// Enter or Tab outside an edit: round the range, or on to the next
    /// cell — Enter after Tabs going back to the column the Tabs started
    /// from, as in Excel.
    fn advance_cell(&mut self, by_row: bool, back: bool, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        if sheet.selection.is_cell() {
            match (by_row, sheet.tab_start) {
                (true, None) if !back => sheet.tab_start = Some(sheet.selection.active.column),
                (false, Some(column)) if !back => {
                    let to = CellRef::new(column, sheet.selection.active.row + 1);
                    self.select_cell(to, false, cx);
                    return;
                }
                (false, _) => sheet.tab_start = None,
                _ => {}
            }
            let tab_start = sheet.tab_start;
            sheet.selection.advance(by_row, back);
            let to = sheet.selection.active;
            self.reveal_cell(to);
            if let Some(sheet) = self.sheet_mut() {
                sheet.tab_start = tab_start;
            }
        } else {
            sheet.selection.advance(by_row, back);
            let to = sheet.selection.active;
            self.reveal_cell(to);
        }
        cx.notify();
    }

    /// Writes the cell being typed into, if any; the write recalculates.
    /// A formula Excel would refuse keeps the editor open and returns false.
    pub(super) fn commit_cell(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(sheet) = self.sheet_mut() else {
            return true;
        };
        if sheet.validation_pending.is_some()
            || sheet.validation_dialog.is_some()
            || sheet.link_dialog.is_some()
            || sheet.protection_dialog.is_some()
            || sheet.table_dialog.is_some()
            || sheet.pivot_dialog.is_some()
            || sheet.subtotal_dialog.is_some()
            || sheet.text_columns_dialog.is_some()
            || sheet.sort_dialog.is_some()
            || sheet.duplicates_dialog.is_some()
        {
            return false;
        }
        let Some(edit) = sheet.edit.take() else {
            return true;
        };
        let result = sheet.book.set_cell(sheet.selection.active, &edit.text.text);
        if result
            .as_ref()
            .is_err_and(|e| jade_web::sheet::validation_refusal(e).is_some())
        {
            if let Ok(Some(alert)) = sheet
                .book
                .validation_alert(sheet.selection.active, &edit.text.text)
            {
                sheet.validation_pending = Some(PendingValidation {
                    at: sheet.selection.active,
                    text: edit.text.text.clone(),
                    alert,
                });
            }
        }
        if result
            .as_ref()
            .is_err_and(|e| jade_web::sheet::protected_refusal(e))
        {
            if let Ok(Some(dialog)) =
                ProtectionDialog::for_entry(&sheet.book, sheet.selection.active, &edit.text.text)
            {
                sheet.protection_dialog = Some(dialog);
                sheet.menu = None;
            }
        }
        let refused = result.is_err();
        if let Some(sheet) = self.sheet_mut() {
            if refused {
                sheet.edit = Some(CellEdit {
                    point: None,
                    ..edit
                });
            } else if matches!(result, Ok(true)) {
                // Typing into a cell lets go of what was copied, as in Excel.
                sheet.book.forget_copy();
            }
        }
        self.edited(result, cx);
        !refused
    }

    pub(super) fn begin_cell_edit(
        &mut self,
        text: Option<String>,
        in_place: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let text = text.unwrap_or_else(|| sheet.book.edit_text(sheet.selection.active));
        sheet.edit = Some(CellEdit {
            text: TextEdit::new(text),
            in_place,
            point: None,
        });
        let active = sheet.selection.active;
        self.reveal_cell(active);
        cx.notify();
    }

    fn select_sheet(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.sheet_ref().is_some_and(|s| {
            s.validation_dialog.is_some()
                || s.link_dialog.is_some()
                || s.protection_dialog.is_some()
                || s.table_dialog.is_some()
                || s.pivot_dialog.is_some()
                || s.subtotal_dialog.is_some()
                || s.text_columns_dialog.is_some()
                || s.sort_dialog.is_some()
                || s.duplicates_dialog.is_some()
        }) {
            return;
        }
        if !self.commit_cell(cx) {
            return;
        }
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        if index != sheet.book.sheet_index() && sheet.book.select_sheet(index).is_ok() {
            sheet.object = None;
            sheet.selection = Selection::default();
            sheet.tab_start = None;
            if let Some(old) = sheet.painted.take() {
                self.retired.push(old.image);
            }
            if let Some(sheet) = self.sheet_mut() {
                sheet.scroll.set_offset(point(px(0.0), px(0.0)));
            }
        }
        cx.notify();
    }

    /// Data ▸ Remove Duplicates over `range`, comparing `columns`, its
    /// first row a header when `headers`, and Excel's count in the status
    /// bar.
    fn remove_sheet_duplicates(
        &mut self,
        range: CellRange,
        columns: &[u32],
        headers: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        match sheet.book.remove_duplicates(range, columns, headers) {
            Ok(outcome) => {
                self.edited(Ok(outcome.removed > 0), cx);
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
            Err(error) => self.edited(Err(error), cx),
        }
    }

    /// The Format Painter: the selection's formats picked up for the next
    /// selection, or for every one until Escape when `sticky`; a click while
    /// it is on puts it down.
    fn pick_up_sheet_format(&mut self, sticky: bool, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        if sheet.format_source.is_some() && !sticky {
            sheet.format_source = None;
            sheet.format_sticky = false;
        } else {
            sheet.format_source = Some(sheet.selection.range());
            sheet.format_sticky = sticky;
        }
        cx.notify();
    }

    /// After a write that may have changed the sheets: their names again,
    /// and the selection kept on the sheet.
    pub(super) fn sheets_changed(&mut self, result: Result<bool, String>, cx: &mut Context<Self>) {
        if let Some(sheet) = self.sheet_mut() {
            sheet.names = sheet
                .book
                .sheet_names()
                .into_iter()
                .map(SharedString::from)
                .collect();
            if let Some(old) = sheet.painted.take() {
                self.retired.push(old.image);
            }
        }
        self.edited(result, cx);
    }

    // ---- pointing at cells in a formula -------------------------------------

    /// Points the formula being typed at `anchor`..`focus`: the reference
    /// goes in at the caret, or replaces the one being pointed with.
    fn point_at(&mut self, anchor: CellRef, focus: CellRef, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let Some(edit) = sheet.edit.as_mut() else {
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
        self.reveal_cell(focus);
        cx.notify();
    }

    /// An arrow while a formula can point: the pointed cell moves (or with
    /// Shift the pointed range grows), starting from the cell being typed
    /// into.
    fn point_step(
        &mut self,
        direction: Direction,
        extend: bool,
        jump: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(sheet) = self.sheet_ref() else {
            return;
        };
        let Some(edit) = sheet.edit.as_ref() else {
            return;
        };
        let (anchor, focus) = match &edit.point {
            Some(point) => (point.anchor, point.focus),
            None => (sheet.selection.active, sheet.selection.active),
        };
        let from = if extend { focus } else { anchor };
        let to = if jump {
            sheet.book.jump(from, direction)
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

    // ---- keys -----------------------------------------------------------

    /// A key on a workbook; true when it was the sheet's.
    pub(super) fn sheet_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let modifiers = event.keystroke.modifiers;
        let key = event.keystroke.key.as_str();
        let command = modifiers.platform || modifiers.control;
        let shift = modifiers.shift;
        if let Some(handled) = self.protection_key(key, shift, command, cx) {
            return handled;
        }
        if self
            .sheet_ref()
            .and_then(|s| s.validation_dialog.as_ref())
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
        if self.duplicates_key(key, cx) {
            return true;
        }
        if self.validation_key(key, shift, command, modifiers.alt, cx) {
            return true;
        }
        let zoom = self.shown_zoom.max(0.01);
        let Some(sheet) = self.sheet_mut() else {
            return false;
        };
        sheet.menu = None;
        // The Format Cells dialog takes every key while it is open.
        if sheet.format_dialog.is_some() {
            let key_char = event.keystroke.key_char.clone();
            self.format_dialog_key(key, key_char.as_deref(), cx);
            return true;
        }
        if sheet.choices.is_some() {
            if key == "escape" {
                sheet.choices = None;
                cx.notify();
                return true;
            }
            sheet.choices = None;
        }
        // A chart's text dialog has the keys while it is open.
        if sheet.chart_asking.is_some() {
            return self.chart_key(key, modifiers, cx);
        }
        // A selected chart, picture or shape: Delete removes it.
        if sheet.object.is_some() && self.object_key(key, cx) {
            return true;
        }
        let Some(sheet) = self.sheet_mut() else {
            return false;
        };
        // A conditional format's dialog has the keys while it is open.
        if sheet.asking.is_some() {
            return self.rule_key(key, modifiers, cx);
        }
        if sheet.pasting.is_some() {
            return self.paste_special_key(key, modifiers, cx);
        }
        if sheet.saving_as.is_some() {
            return self.save_as_key(key, modifiers, cx);
        }
        if sheet.managing && key == "escape" {
            sheet.managing = false;
            cx.notify();
            return true;
        }
        if sheet.filter.is_some() {
            match key {
                "escape" => sheet.filter = None,
                "enter" => self.apply_filter(cx),
                _ => {}
            }
            cx.notify();
            return true;
        }

        // A tab being renamed, or the name box: Enter takes it, Escape
        // leaves it.
        if sheet.finding.is_some() && sheet.renaming.is_none() && sheet.naming.is_none() {
            return self.sheet_find_key(key, modifiers, cx);
        }
        if sheet.renaming.is_some() || sheet.naming.is_some() || sheet.noting.is_some() {
            return self.name_key(key, modifiers, cx);
        }

        let pointable = sheet.pointable();
        if let Some(edit) = sheet.edit.as_mut() {
            // Mid-composition the keys are the IME's.
            if edit.text.marked.is_some() {
                return false;
            }
            match key {
                // A line break in the cell: ⌥Enter (Alt+Enter elsewhere).
                "enter" if modifiers.alt => {
                    edit.text.insert("\n");
                    edit.point = None;
                }
                // ⌘Enter: what was typed into every selected cell.
                "enter" if command => {
                    let text = edit.text.text.clone();
                    let range = sheet.selection.range();
                    let active = sheet.selection.active;
                    let result = attempt(
                        &mut sheet.book,
                        &mut sheet.protection_dialog,
                        ProtectedEdit::FillWith {
                            range,
                            active,
                            text,
                        },
                    )
                    .map(|outcome| outcome.changed);
                    if result.is_ok() {
                        sheet.edit = None;
                    }
                    self.edited(result, cx);
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
                "escape" => sheet.edit = None,
                "backspace" => {
                    edit.text.backspace();
                    edit.point = None;
                }
                "delete" => {
                    edit.text.delete();
                    edit.point = None;
                }
                "f2" => {
                    edit.in_place = !edit.in_place;
                    edit.point = None;
                }
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
                ";" if modifiers.control || (modifiers.platform && !shift) => {
                    let time = modifiers.platform || shift;
                    edit.text.insert(&now_text(time));
                    edit.point = None;
                }
                _ => {
                    let Some(direction) = Direction::of_key(key) else {
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
        let viewport_height = f64::from(sheet.viewport.1) / f64::from(zoom);
        match key {
            "pageup" | "pagedown" => {
                let Some(sheet) = self.sheet_ref() else {
                    return false;
                };
                let from = if shift {
                    sheet.selection.focus
                } else {
                    sheet.selection.active
                };
                let rows = i64::from(sheet.book.page_rows(from.row, viewport_height));
                if modifiers.alt {
                    let columns = 8;
                    let columns = if key == "pageup" { -columns } else { columns };
                    self.step_cell(columns, 0, shift, cx);
                } else {
                    // The view moves by the page too, keeping the active
                    // cell where it was on the screen.
                    let zoom = self.shown_zoom;
                    if let Some(sheet) = self.sheet_mut() {
                        let at = Self::sheet_scroll(&sheet.scroll, zoom);
                        let to = grid::offset(from, 0, if key == "pageup" { -rows } else { rows });
                        let moved = sheet
                            .book
                            .cell_rect(to, (0.0, 0.0))
                            .zip(sheet.book.cell_rect(from, (0.0, 0.0)))
                            .map(|(to, from)| to.1 - from.1)
                            .unwrap_or(0.0);
                        let y = (at.1 + moved).max(0.0);
                        sheet.book.reach(to);
                        sheet.scroll.set_offset(point(
                            px((-at.0 * f64::from(zoom)) as f32),
                            px((-y * f64::from(zoom)) as f32),
                        ));
                    }
                    self.step_cell(0, if key == "pageup" { -rows } else { rows }, shift, cx);
                }
            }
            "home" => {
                let Some(sheet) = self.sheet_ref() else {
                    return false;
                };
                let row = if shift {
                    sheet.selection.focus.row
                } else {
                    sheet.selection.active.row
                };
                self.select_cell(CellRef::new(0, row), shift, cx);
            }
            "enter" => self.advance_cell(false, shift, cx),
            "tab" => self.advance_cell(true, shift, cx),
            "f2" => self.begin_cell_edit(None, true, cx),
            "f5" => self.open_go_to(cx),
            // Calculate Now, as in Excel.
            "f9" => {
                let result = sheet.book.calculate_now();
                self.edited(result, cx);
            }
            "delete" | "backspace" => {
                let Some(sheet) = self.sheet_mut() else {
                    return false;
                };
                let range = sheet.selection.range();
                let result = attempt(
                    &mut sheet.book,
                    &mut sheet.protection_dialog,
                    ProtectedEdit::Clear { range },
                )
                .map(|outcome| outcome.changed);
                self.edited(result, cx);
            }
            "escape" => {
                sheet.book.forget_copy();
                sheet.format_source = None;
                sheet.format_sticky = false;
                cx.notify();
            }
            "=" if modifiers.alt => self.autosum(cx),
            // ⌥↓: the active cell's list of choices.
            "down" if modifiers.alt => {
                let list = sheet.book.validation_dropdown(sheet.selection.active);
                sheet.choices = list;
                cx.notify();
            }
            // Shift+Space: the selection's rows, whole.
            "space" if shift => {
                sheet.selection.whole_rows();
                cx.notify();
            }
            _ => {
                let Some(direction) = Direction::of_key(key) else {
                    // Anything else is text, which arrives through the input
                    // handler and starts an edit there.
                    return false;
                };
                self.arrow_cell(direction, shift, cx);
            }
        }
        true
    }

    /// ⌘ (Ctrl elsewhere) with a key, on a sheet not being typed into.
    fn sheet_command(
        &mut self,
        key: &str,
        modifiers: gpui::Modifiers,
        cx: &mut Context<Self>,
    ) -> bool {
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
        let Some(sheet) = self.sheet_mut() else {
            return false;
        };
        let range = sheet.selection.range();
        let active = sheet.selection.active;
        match key {
            "home" => self.select_cell(CellRef::new(0, 0), shift, cx),
            "t" if shift && modifiers.platform => self.autosum(cx),
            "end" => {
                let end = sheet.book.used_end();
                self.select_cell(end, shift, cx);
            }
            "c" => self.sheet_copy(false, cx),
            // ⌘⇧F (Ctrl+Shift+L elsewhere): filter buttons on or off.
            "f" | "l" if shift => {
                let range = sheet.selection.range();
                let result = sheet.book.toggle_filter(range);
                self.edited(result, cx);
            }
            "f" => self.open_sheet_find(false, cx),
            "t" => self.open_sheet_table_dialog(cx),
            "k" => self.open_sheet_link_dialog(cx),
            // ⌘⇧S: Save As, its extension picking the format.
            "s" if shift => self.open_save_as(cx),
            // ⌘P: the pages, as a PDF to print.
            "p" => self.print_sheet(cx),
            // Ctrl+H, as Excel: Find and Replace.
            "h" if modifiers.control && !modifiers.platform => self.open_sheet_find(true, cx),
            // Ctrl+G, as F5: Go To.
            "g" if modifiers.control && !modifiers.platform => self.open_go_to(cx),
            "x" if !shift => self.sheet_copy(true, cx),
            // ⌃⌘V (Ctrl+Alt+V elsewhere): Paste Special.
            "v" if modifiers.control && (modifiers.platform || modifiers.alt) => {
                self.open_paste_special(cx)
            }
            // ⌘A: the block of data round the active cell, then the sheet.
            "a" => {
                let region = sheet.book.current_region(active);
                let previous_selection = sheet.selection;
                sheet.selection = match region {
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
                if !sheet.book.can_select_range(sheet.selection.range()) {
                    sheet.selection = previous_selection;
                }
                cx.notify();
            }
            "d" => {
                let result = attempt(
                    &mut sheet.book,
                    &mut sheet.protection_dialog,
                    ProtectedEdit::FillDown { range },
                )
                .map(|outcome| outcome.changed);
                self.edited(result, cx);
            }
            "r" => {
                let result = attempt(
                    &mut sheet.book,
                    &mut sheet.protection_dialog,
                    ProtectedEdit::FillRight { range },
                )
                .map(|outcome| outcome.changed);
                self.edited(result, cx);
            }
            // ⌘1: Format Cells, as in Excel.
            "1" if !shift => self.open_format_dialog(cx),
            "b" | "2" => self.sheet_toggle(FontToggle::Bold, cx),
            "i" | "3" => self.sheet_toggle(FontToggle::Italic, cx),
            "u" | "4" => self.sheet_toggle(FontToggle::Underline, cx),
            "x" | "5" => self.sheet_toggle(FontToggle::Strike, cx),
            "!" => self.sheet_number_format("#,##0.00", cx),
            "$" => self.sheet_number_format(i18n::text("sheet.format.currency"), cx),
            "%" => self.sheet_number_format("0%", cx),
            "#" => self.sheet_number_format(i18n::text("sheet.format.date"), cx),
            "@" => self.sheet_number_format(i18n::text("sheet.format.time"), cx),
            "^" => self.sheet_number_format("0.00E+00", cx),
            "~" => self.sheet_number_format("General", cx),
            // ⌃; today, ⌘; (or Ctrl+Shift+;) the time, into the active cell.
            ";" => {
                let time = modifiers.platform || shift;
                let result = sheet.book.set_cell(active, &now_text(time));
                self.edited(result, cx);
            }
            "+" => self.sheet_menu_command(MenuCommand::InsertRows, true, cx),
            "-" => self.sheet_menu_command(MenuCommand::DeleteRows, true, cx),
            "space" => {
                sheet.selection.whole_columns();
                cx.notify();
            }
            _ => return false,
        }
        true
    }

    /// Keys while a tab is renamed or the name box is typed into.
    fn name_key(&mut self, key: &str, modifiers: gpui::Modifiers, cx: &mut Context<Self>) -> bool {
        let command = modifiers.platform || modifiers.control;
        let Some(sheet) = self.sheet_mut() else {
            return false;
        };
        let text = if let Some((_, text)) = sheet.renaming.as_mut() {
            text
        } else if let Some(text) = sheet.naming.as_mut() {
            text
        } else if let Some(bar) = sheet.finding.as_mut() {
            bar.field_mut()
        } else if let Some((_, text)) = sheet.noting.as_mut() {
            // A note takes line breaks: Enter on its own ends it.
            if key == "enter" && (modifiers.alt || modifiers.shift) {
                text.insert("\n");
                cx.notify();
                return true;
            }
            text
        } else {
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
            "home" => text.to_end(false),
            "end" => text.to_end(true),
            "a" if command => text.whole = true,
            _ => return command,
        }
        cx.notify();
        true
    }

    /// Ends a tab rename or a name-box entry, taking it or not.
    /// Go To (F5, Ctrl+G): the name box opened on the active cell, for a
    /// cell, range or name to go to; one that does not exist is said so.
    fn open_go_to(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        if let Some(sheet) = self.sheet_mut() {
            sheet.menu = None;
            sheet.naming = Some(TextEdit::selected(reference_text(sheet.selection.active)));
            sheet.going_to = true;
        }
        cx.notify();
    }

    fn finish_naming(&mut self, take: bool, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        if let Some((index, text)) = sheet.renaming.take() {
            let name = text.text.trim().to_string();
            if take && !name.is_empty() {
                let result = sheet.book.rename_sheet(index, &name);
                self.sheets_changed(result, cx);
            }
        } else if let Some((cell, text)) = sheet.noting.take() {
            if take {
                let result = sheet.book.set_note(cell, &text.text, "jade");
                self.edited(result, cx);
            }
        } else if sheet.finding.take().is_some() {
            // Escape or Tab closes the find bar where the search got to.
        } else if let Some(text) = sheet.naming.take() {
            let going_to = std::mem::take(&mut sheet.going_to);
            if take {
                match sheet.book.resolve_reference(&text.text) {
                    Some((index, range)) => {
                        if !sheet.book.sheet_tabs().contains(&index) {
                            self.status = i18n::text("sheet.no_reference").into();
                            cx.notify();
                            return;
                        }
                        self.select_sheet(index, cx);
                        let Some(sheet) = self.sheet_mut() else {
                            return;
                        };
                        if sheet.book.can_select_range(range) {
                            sheet.selection = Selection::range_from(range.start, range.end);
                        }
                        self.reveal_cell(range.start);
                    }
                    None if going_to => self.status = i18n::text("sheet.no_reference").into(),
                    None => {
                        let result = sheet
                            .book
                            .define_range_name(&text.text, sheet.selection.range());
                        self.edited(result, cx);
                    }
                }
            }
        }
        cx.notify();
    }

    // ---- commands ---------------------------------------------------------

    fn sheet_toggle(&mut self, style: FontToggle, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let range = sheet.selection.range();
        let result = sheet.book.toggle(range, sheet.selection.active, style);
        self.edited(result, cx);
    }

    fn sheet_number_format(&mut self, code: &str, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let change = CellFormat {
            number_format: Some(code.to_string()),
            ..Default::default()
        };
        let range = sheet.selection.range();
        let result = sheet.book.format(range, &change);
        self.edited(result, cx);
    }

    /// The selected cells onto the clipboard as Excel's text, kept for a
    /// paste here that moves formulas and carries formats; a cut moves them
    /// when pasted.
    pub(super) fn sheet_copy(&mut self, cut: bool, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let range = sheet.selection.range();
        let text = sheet.book.copy(range, cut);
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        cx.notify();
    }

    /// A paste onto the selection: this workbook's own copy when the
    /// clipboard still holds it, otherwise the clipboard's text, from the
    /// selection's top-left or tiled over it.
    pub(super) fn sheet_paste(&mut self, text: Option<&str>, cx: &mut Context<Self>) {
        if self.sheet_ref().is_some_and(|s| {
            s.validation_pending.is_some()
                || s.validation_dialog.is_some()
                || s.link_dialog.is_some()
                || s.protection_dialog.is_some()
                || s.table_dialog.is_some()
                || s.pivot_dialog.is_some()
                || s.subtotal_dialog.is_some()
                || s.text_columns_dialog.is_some()
                || s.sort_dialog.is_some()
                || s.duplicates_dialog.is_some()
        }) {
            return;
        }
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let target = sheet.selection.range();
        // From a menu there is no clipboard text: the copy made here.
        let Some(text) = text
            .map(str::to_string)
            .or_else(|| sheet.book.copied_text())
        else {
            return;
        };
        let result = attempt(
            &mut sheet.book,
            &mut sheet.protection_dialog,
            ProtectedEdit::Paste { target, text },
        )
        .map(|outcome| outcome.selection);
        if let Ok(Some(filled)) = &result {
            let active = sheet.selection.active;
            sheet.selection = Selection {
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

    fn sheet_menu_command(
        &mut self,
        command: MenuCommand,
        from_keys: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        sheet.menu = None;
        if sheet.book.structure_protected()
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
        let selection = sheet.selection;
        let range = selection.range();
        let rows = range.end.row.saturating_sub(range.start.row) + 1;
        let columns = range.end.column.saturating_sub(range.start.column) + 1;
        // ⌘+ and ⌘- insert and delete whole columns when whole columns are
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
                let locked = sheet
                    .book
                    .format_at(sheet.selection.active)
                    .locked
                    .unwrap_or(true);
                sheet.book.format(
                    range,
                    &CellFormat {
                        locked: Some(!locked),
                        ..Default::default()
                    },
                )
            }
            MenuCommand::CellHidden => {
                let hidden = sheet
                    .book
                    .format_at(sheet.selection.active)
                    .formula_hidden
                    .unwrap_or(false);
                sheet.book.format(
                    range,
                    &CellFormat {
                        formula_hidden: Some(!hidden),
                        ..Default::default()
                    },
                )
            }
            MenuCommand::FormatCells => return self.open_format_dialog(cx),
            MenuCommand::FormatPainter => return self.pick_up_sheet_format(false, cx),
            MenuCommand::Validation => return self.open_validation(cx),
            MenuCommand::Table => return self.open_sheet_table_dialog(cx),
            MenuCommand::EditPivot => return self.open_pivot_dialog(cx),
            MenuCommand::CreatePivot => return self.open_create_pivot(cx),
            MenuCommand::DeletePivot => sheet.book.delete_pivot_at(selection.active),
            MenuCommand::RefreshPivot => sheet.book.refresh_pivot_at(sheet.selection.active),
            MenuCommand::RefreshAllPivots => sheet.book.refresh_all_pivots(),
            MenuCommand::Link => return self.open_sheet_link_dialog(cx),
            MenuCommand::RemoveLinks => sheet.book.clear_hyperlinks(&[range]),
            MenuCommand::ClearValidation => sheet.book.clear_data_validations(Some(range)),
            MenuCommand::Cut => return self.sheet_copy(true, cx),
            MenuCommand::Copy => return self.sheet_copy(false, cx),
            MenuCommand::Paste => return self.sheet_paste(None, cx),
            MenuCommand::PasteSpecial => return self.open_paste_special(cx),
            MenuCommand::Clear => attempt(
                &mut sheet.book,
                &mut sheet.protection_dialog,
                ProtectedEdit::Clear { range },
            )
            .map(|outcome| outcome.changed),
            MenuCommand::ClearFormats => sheet.book.clear_formats(range),
            MenuCommand::ClearAll => sheet.book.clear_all(range),
            MenuCommand::InsertRows => sheet.book.insert_rows(range.start.row, rows),
            MenuCommand::DeleteRows => sheet.book.delete_rows(range.start.row, rows),
            MenuCommand::InsertColumns => sheet.book.insert_columns(range.start.column, columns),
            MenuCommand::DeleteColumns => sheet.book.delete_columns(range.start.column, columns),
            MenuCommand::ShiftCells(axis, inserted) => {
                sheet.book.shift_cells(range, axis, inserted)
            }
            MenuCommand::FillDown => attempt(
                &mut sheet.book,
                &mut sheet.protection_dialog,
                ProtectedEdit::FillDown { range },
            )
            .map(|outcome| outcome.changed),
            MenuCommand::FillRight => attempt(
                &mut sheet.book,
                &mut sheet.protection_dialog,
                ProtectedEdit::FillRight { range },
            )
            .map(|outcome| outcome.changed),
            MenuCommand::SortAscending => sheet.book.sort(range, selection.active, false),
            MenuCommand::SortDescending => sheet.book.sort(range, selection.active, true),
            MenuCommand::MergeCenter => {
                let range = sheet.book.with_merges(range);
                let result = sheet.book.merge_and_center(range);
                if sheet.book.can_select_range(range) {
                    sheet.selection = Selection::range_from(range.start, range.end);
                }
                result
            }
            MenuCommand::InsertSheet => {
                let result = sheet.book.add_sheet().map(|_| true);
                sheet.selection = Selection::default();
                sheet.scroll.set_offset(point(px(0.0), px(0.0)));
                result
            }
            MenuCommand::MoveSheet(index, to) => {
                let result = sheet.book.move_sheet(index, to);
                sheet.selection = Selection::default();
                result
            }
            MenuCommand::EditNote => {
                let at = selection.active;
                let text = sheet.book.note(at).unwrap_or_default().to_string();
                sheet.noting = Some((at, TextEdit::new(text)));
                cx.notify();
                return;
            }
            MenuCommand::DeleteNote => sheet.book.set_note(selection.active, "", "jade"),
            MenuCommand::Hide | MenuCommand::Unhide => {
                let hide = matches!(command, MenuCommand::Hide);
                let (axis, first, last) = if selection.is_whole_columns() {
                    (Axis::Columns, range.start.column, range.end.column)
                } else {
                    (Axis::Rows, range.start.row, range.end.row)
                };
                if hide {
                    sheet.book.hide(axis, first, last)
                } else {
                    sheet.book.unhide(axis, first, last)
                }
            }
            MenuCommand::FreezePanes => sheet.book.freeze_panes(Freeze::Panes(selection.active)),
            MenuCommand::UnfreezePanes => sheet.book.freeze_panes(Freeze::Unfreeze),
            MenuCommand::Subtotal => return self.open_subtotal_dialog(cx),
            MenuCommand::RemoveDuplicates => return self.open_duplicates_dialog(cx),
            MenuCommand::TextToColumns => return self.open_text_columns_dialog(cx),
            MenuCommand::SortDialog => return self.open_sort_dialog(cx),
            MenuCommand::ManualCalculation(manual) => sheet.book.set_manual_calculation(manual),
            MenuCommand::CalculateNow => sheet.book.calculate_now(),
            MenuCommand::AutoOutline => sheet.book.auto_outline(),
            MenuCommand::ClearOutline => {
                sheet.book.clear_outline(if selection.is_whole_columns() {
                    Axis::Columns
                } else {
                    Axis::Rows
                })
            }
            MenuCommand::Outline(change) => {
                let (axis, first, last) = if selection.is_whole_columns() {
                    (Axis::Columns, range.start.column, range.end.column)
                } else {
                    (Axis::Rows, range.start.row, range.end.row)
                };
                sheet.book.outline(axis, first, last, change)
            }
            MenuCommand::RenameSheet(index) => {
                let name = sheet.names.get(index).map(|name| name.to_string());
                sheet.renaming = name.map(|name| (index, TextEdit::selected(name)));
                cx.notify();
                return;
            }
            MenuCommand::DeleteSheet(index) => {
                let result = sheet.book.delete_sheet(index);
                sheet.selection = Selection::default();
                sheet.scroll.set_offset(point(px(0.0), px(0.0)));
                result
            }
            MenuCommand::HideSheet(index) => {
                let result = sheet
                    .book
                    .set_sheet_visibility(index, SheetVisibility::Hidden);
                sheet.selection = Selection::default();
                sheet.scroll.set_offset(point(px(0.0), px(0.0)));
                result
            }
            MenuCommand::UnhideSheet(index) => sheet
                .book
                .set_sheet_visibility(index, SheetVisibility::Visible),
            MenuCommand::TabColor(index, color) => sheet.book.set_tab_color(index, color),
        };
        // Rows or columns gone from under a copy let go of it, as in Excel.
        if matches!(
            command,
            MenuCommand::DeleteRows | MenuCommand::DeleteColumns
        ) {
            if let Some(sheet) = self.sheet_mut() {
                sheet.book.forget_copy();
            }
        }
        self.sheets_changed(result, cx);
    }

    // ---- the mouse --------------------------------------------------------

    pub(super) fn sheet_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.sheet_ref().is_some_and(|s| {
            s.validation_dialog.is_some()
                || s.validation_pending.is_some()
                || s.link_dialog.is_some()
                || s.protection_dialog.is_some()
                || s.table_dialog.is_some()
                || s.pivot_dialog.is_some()
                || s.subtotal_dialog.is_some()
                || s.text_columns_dialog.is_some()
                || s.sort_dialog.is_some()
                || s.duplicates_dialog.is_some()
        }) {
            return;
        }
        window.focus(&self.focus, cx);
        let Some((x, y)) = self.grid_point(event.position) else {
            return;
        };
        let zoom = self.shown_zoom;
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        sheet.menu = None;
        sheet.filter = None;
        sheet.popup = None;
        sheet.choices = None;
        if sheet.renaming.is_some() || sheet.naming.is_some() {
            self.finish_naming(true, cx);
        }
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let scroll = Self::sheet_scroll(&sheet.scroll, zoom);
        // The outline gutter's level buttons and boxes collapse and expand.
        let clicked = if sheet.edit.is_none() {
            sheet.book.outline_click(x, y, scroll)
        } else {
            None
        };
        if let Some(result) = clicked {
            self.sheets_changed(result, cx);
            return;
        }
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let hit = sheet.book.hit(x, y, scroll);

        // While a formula can take a reference, a click points at the cell.
        if sheet.pointable() {
            if let Some(Hit::Cell(cell)) = hit {
                sheet.drag = Some(SheetDrag::Point);
                let anchor = match (&sheet.edit, event.modifiers.shift) {
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
        // A chart, picture or shape sits over the cells: a press on one
        // selects it, and its border or a handle drags it.
        if self.object_press(x, y, cx) {
            return;
        }
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        sheet.tab_start = None;

        // A filter button opens its dropdown.
        if let Some(button) = sheet.book.filter_button_at(x, y, scroll) {
            match sheet.book.filter_options(&button) {
                Ok(options) => {
                    let checked = options
                        .options
                        .iter()
                        .map(|option| option.selected.unwrap_or(true))
                        .collect();
                    let metrics = sheet.book.header_metrics();
                    let left = button.rect.x + metrics.row_width - scroll.0;
                    let top = button.rect.y + button.rect.height + metrics.column_height - scroll.1;
                    sheet.filter = Some(FilterPanel {
                        button,
                        options: options.options,
                        checked,
                        at: (
                            (left * f64::from(zoom)) as f32,
                            (top * f64::from(zoom)) as f32,
                        ),
                    });
                    cx.notify();
                }
                Err(error) => {
                    self.status = i18n::format("sheet.edit_failed", &[("error", &error)]).into();
                    cx.notify();
                }
            }
            return;
        }
        // A header edge: a drag resizes, a double-click fits the content.
        if let Some(edge) = sheet.book.resize_hit(x, y, scroll) {
            if event.click_count >= 2 && edge.axis == Axis::Columns {
                let range = sheet.selection.range();
                let (first, last) = if sheet.selection.is_whole_columns()
                    && (range.start.column..=range.end.column).contains(&edge.index)
                {
                    (range.start.column, range.end.column)
                } else {
                    (edge.index, edge.index)
                };
                let result = sheet.book.auto_fit_columns(first, last);
                self.edited(result, cx);
                return;
            }
            sheet.drag = Some(SheetDrag::Resize(ResizeGesture {
                hit: edge,
                pointer_start: if edge.axis == Axis::Columns { x } else { y },
                size_px: edge.size_px,
            }));
            cx.notify();
            return;
        }
        // The fill handle.
        let range = sheet.selection.range();
        if !event.modifiers.shift && sheet.book.on_fill_handle(x, y, scroll, range) {
            if event.click_count >= 2 {
                if let Some(target) = sheet.book.fill_handle_extent(range) {
                    let result = attempt(
                        &mut sheet.book,
                        &mut sheet.protection_dialog,
                        ProtectedEdit::FillSeries {
                            source: range,
                            target,
                        },
                    )
                    .map(|outcome| outcome.changed);
                    if matches!(result, Ok(true)) {
                        sheet.selection.focus = target.end;
                        sheet.selection.anchor = target.start;
                    }
                    self.edited(result, cx);
                }
                return;
            }
            sheet.drag = Some(SheetDrag::Fill {
                source: range,
                target: range,
            });
            cx.notify();
            return;
        }
        // The selection's border: a drag moves the cells (⌥ copies them).
        if !event.modifiers.shift
            && !sheet.selection.is_whole_rows()
            && !sheet.selection.is_whole_columns()
            && sheet.book.on_selection_border(x, y, scroll, range)
        {
            let at = sheet.book.reach_point(x, y, scroll);
            sheet.drag = Some(SheetDrag::Move {
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
        let previous_selection = sheet.selection;
        let kind = sheet.selection.press(hit, event.modifiers.shift);
        if !sheet.book.can_select_range(sheet.selection.range()) {
            sheet.selection = previous_selection;
            return;
        }
        sheet.drag = Some(SheetDrag::Select(kind));
        sheet.pending_link = match hit {
            Hit::Cell(cell) if !event.modifiers.shift && event.click_count == 1 => {
                sheet.book.link(cell).map(|link| (cell, link))
            }
            _ => None,
        };
        if let Hit::Cell(cell) = hit {
            sheet.book.reach(cell);
            if event.click_count >= 2 && !event.modifiers.shift {
                sheet.drag = None;
                self.begin_cell_edit(None, true, cx);
            }
        }
        cx.notify();
    }

    /// A choice from the active cell's list goes into it.
    fn choose(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let Some(choice) = sheet
            .choices
            .take()
            .and_then(|choices| choices.into_iter().nth(index))
        else {
            return;
        };
        let active = sheet.selection.active;
        let result = sheet.book.set_validation_choice(active, &choice);
        self.edited(result, cx);
    }

    /// Follows a hyperlink: a web address opens in the browser, a place in
    /// the workbook is selected.
    fn follow_link(&mut self, link: Link, cx: &mut Context<Self>) {
        match link {
            Link::Url(url) => cx.open_url(&url),
            Link::Place { sheet, range } => {
                self.select_sheet(sheet, cx);
                if let Some(state) = self.sheet_mut() {
                    state.selection = Selection::range_from(range.start, range.end);
                }
                self.reveal_cell(range.start);
                cx.notify();
            }
        }
    }

    /// A right-click: the cell (or header) under it is selected unless it
    /// is in the selection already, and the menu opens there.
    pub(super) fn sheet_right_click(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        if !self.commit_cell(cx) {
            return;
        }
        let Some((x, y)) = self.grid_point(event.position) else {
            return;
        };
        let zoom = self.shown_zoom;
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let scroll = Self::sheet_scroll(&sheet.scroll, zoom);
        if let Some(hit) = sheet.book.hit(x, y, scroll) {
            let inside = match hit {
                Hit::Cell(cell) => sheet.selection.range().contains(cell),
                Hit::Row(row) => {
                    sheet.selection.is_whole_rows()
                        && (sheet.selection.range().start.row..=sheet.selection.range().end.row)
                            .contains(&row)
                }
                Hit::Column(column) => {
                    sheet.selection.is_whole_columns()
                        && (sheet.selection.range().start.column
                            ..=sheet.selection.range().end.column)
                            .contains(&column)
                }
                Hit::All => false,
            };
            if !inside {
                let previous_selection = sheet.selection;
                sheet.selection.press(hit, false);
                if !sheet.book.can_select_range(sheet.selection.range()) {
                    sheet.selection = previous_selection;
                    return;
                }
            }
        }
        sheet.menu = Some(SheetMenu {
            at: ((x * f64::from(zoom)) as f32, (y * f64::from(zoom)) as f32),
            tab: None,
        });
        cx.notify();
    }

    /// The pointer moving with no button held: what it is over, for the
    /// cursor.
    fn sheet_hover(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some((x, y)) = self.grid_point(position) else {
            return;
        };
        let zoom = self.shown_zoom;
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        if sheet.drag.is_some() {
            return;
        }
        let scroll = Self::sheet_scroll(&sheet.scroll, zoom);
        let hover = match sheet.book.resize_hit(x, y, scroll) {
            Some(edge) if edge.axis == Axis::Columns => Hover::ResizeColumn,
            Some(_) => Hover::ResizeRow,
            None if sheet
                .book
                .on_fill_handle(x, y, scroll, sheet.selection.range()) =>
            {
                Hover::FillHandle
            }
            None if !sheet.selection.is_whole_rows()
                && !sheet.selection.is_whole_columns()
                && sheet
                    .book
                    .on_selection_border(x, y, scroll, sheet.selection.range()) =>
            {
                Hover::Border
            }
            None if matches!(
                sheet.book.hit(x, y, scroll),
                Some(Hit::Cell(cell)) if sheet.book.link(cell).is_some()
            ) =>
            {
                Hover::Link
            }
            None => Hover::Cells,
        };
        if hover != sheet.hover {
            sheet.hover = hover;
            cx.notify();
        }
        let note = match sheet.book.hit(x, y, scroll) {
            Some(Hit::Cell(cell)) if sheet.book.hover_text(cell).is_some() => sheet
                .book
                .cell_rect(cell, scroll)
                .map(|(cx_, cy, w, _)| (cell, (((cx_ + w) as f32) * zoom, cy as f32 * zoom))),
            _ => None,
        };
        if note.map(|(cell, _)| cell) != sheet.note.map(|(cell, _)| cell) {
            sheet.note = note;
            cx.notify();
        }
    }

    /// A drag reaching `position` (window px), wherever it is.
    fn sheet_drag_to(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some((x, y)) = self.grid_point(position) else {
            return;
        };
        let zoom = self.shown_zoom;
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let Some(drag) = sheet.drag else {
            return;
        };
        sheet.pointer = Some(position);
        let scroll = Self::sheet_scroll(&sheet.scroll, zoom);
        let reached = sheet.book.reach_point(x, y, scroll);
        match drag {
            SheetDrag::Select(kind) => {
                let hit = sheet.book.hit(x, y, scroll).unwrap_or(Hit::Cell(reached));
                let hit = match (kind, hit) {
                    // Over a header mid-drag, a cell drag reaches the cell
                    // beside it.
                    (DragKind::Cells, Hit::Row(_) | Hit::Column(_) | Hit::All) => {
                        Hit::Cell(reached)
                    }
                    (_, hit) => hit,
                };
                let before = sheet.selection;
                sheet.selection.drag(kind, hit);
                if !sheet.book.can_select_range(sheet.selection.range()) {
                    sheet.selection = before;
                    return;
                }
                if sheet.selection != before {
                    sheet.book.reach(reached);
                    cx.notify();
                }
            }
            SheetDrag::Fill { source, target } => {
                let next = jade_web::Workbook::fill_target(source, reached);
                if next != target {
                    sheet.drag = Some(SheetDrag::Fill {
                        source,
                        target: next,
                    });
                    sheet.book.reach(reached);
                    cx.notify();
                }
            }
            SheetDrag::Resize(mut gesture) => {
                gesture.update(x, y);
                sheet.drag = Some(SheetDrag::Resize(gesture));
                cx.notify();
            }
            SheetDrag::Move {
                source,
                grab,
                target,
                copy,
            } => {
                let start = grid::offset(reached, -grab.0, -grab.1);
                let next = CellRange {
                    start,
                    end: CellRef::new(
                        start.column + (source.end.column - source.start.column),
                        start.row + (source.end.row - source.start.row),
                    ),
                };
                if next != target {
                    sheet.drag = Some(SheetDrag::Move {
                        source,
                        grab,
                        target: next,
                        copy,
                    });
                    sheet.book.reach(next.end);
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
                let now = charts::object_drag(handle, start, bounds, x, y);
                sheet.drag = Some(SheetDrag::Object {
                    id,
                    handle,
                    start,
                    bounds,
                    now,
                });
                cx.notify();
            }
            SheetDrag::Point => {
                let anchor = sheet
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
    fn sheet_drag_end(&mut self, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let Some(drag) = sheet.drag.take() else {
            return;
        };
        sheet.pointer = None;
        match drag {
            SheetDrag::Fill { source, target } => {
                let result = attempt(
                    &mut sheet.book,
                    &mut sheet.protection_dialog,
                    ProtectedEdit::FillSeries { source, target },
                )
                .map(|outcome| outcome.changed);
                if matches!(result, Ok(true)) {
                    sheet.selection.anchor = target.start;
                    sheet.selection.focus = target.end;
                    if !target.contains(sheet.selection.active) {
                        sheet.selection.active = target.start;
                    }
                }
                self.edited(result, cx);
            }
            SheetDrag::Resize(gesture) => {
                let range = sheet.selection.range();
                let index = gesture.hit.index;
                // Every selected column (or row) takes the size when the
                // dragged edge is one of them.
                let (first, last) = match gesture.hit.axis {
                    Axis::Columns
                        if sheet.selection.is_whole_columns()
                            && (range.start.column..=range.end.column).contains(&index) =>
                    {
                        (range.start.column, range.end.column)
                    }
                    Axis::Rows
                        if sheet.selection.is_whole_rows()
                            && (range.start.row..=range.end.row).contains(&index) =>
                    {
                        (range.start.row, range.end.row)
                    }
                    _ => (index, index),
                };
                let result = if (gesture.size_px - gesture.hit.size_px).abs() < 0.5 {
                    Ok(false)
                } else {
                    sheet
                        .book
                        .resize(gesture.hit.axis, first, last, gesture.size_px)
                };
                self.edited(result, cx);
            }
            SheetDrag::Select(_) => {
                // The Format Painter's formats land on what was just selected.
                if let Some(source) = sheet.format_source {
                    if !sheet.format_sticky {
                        sheet.format_source = None;
                    }
                    let target = sheet.selection.range();
                    let result = sheet.book.copy_format(source, target);
                    self.edited(result, cx);
                    return;
                }
                let link = sheet.pending_link.take();
                if let Some((_, link)) = link.filter(|(cell, _)| {
                    sheet.selection.is_cell() && sheet.selection.active == *cell
                }) {
                    self.follow_link(link, cx);
                }
                cx.notify();
            }
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
                    &mut sheet.book,
                    &mut sheet.protection_dialog,
                    ProtectedEdit::MoveRange {
                        source,
                        to: target.start,
                        copy,
                    },
                )
                .map(|outcome| outcome.selection.unwrap_or(source));
                if let Ok(moved) = &result {
                    let active = sheet.selection.active;
                    sheet.selection = Selection {
                        active: grid::offset(
                            moved.start,
                            i64::from(active.column) - i64::from(source.start.column),
                            i64::from(active.row) - i64::from(source.start.row),
                        ),
                        anchor: moved.start,
                        focus: moved.end,
                    };
                }
                self.edited(result.map(|_| true), cx);
            }
            SheetDrag::Object {
                id, bounds, now, ..
            } => self.object_drop(id, bounds, now, cx),
            SheetDrag::Point => cx.notify(),
        }
    }

    /// While a drag is held past the grid's edge, the grid scrolls that
    /// way and the drag follows.
    fn start_auto_scroll(&mut self, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        if sheet.auto_scrolling || self.auto_scroll_step().is_none() {
            return;
        }
        if let Some(sheet) = self.sheet_mut() {
            sheet.auto_scrolling = true;
        }
        let executor = cx.background_executor().clone();
        cx.spawn(async move |view, cx| loop {
            executor.timer(AUTO_SCROLL_TICK).await;
            let going = view
                .update(cx, |view, cx| {
                    let Some(step) = view.auto_scroll_step() else {
                        if let Some(sheet) = view.sheet_mut() {
                            sheet.auto_scrolling = false;
                        }
                        return false;
                    };
                    let Some(sheet) = view.sheet_mut() else {
                        return false;
                    };
                    let offset = sheet.scroll.offset();
                    let max = sheet.scroll.max_offset();
                    let x = (offset.x - px(step.0)).clamp(-max.x, px(0.0));
                    let y = (offset.y - px(step.1)).clamp(-max.y, px(0.0));
                    sheet.scroll.set_offset(point(x, y));
                    if let Some(pointer) = sheet.pointer {
                        view.sheet_drag_to(pointer, cx);
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
        let sheet = self.sheet_ref()?;
        sheet.drag?;
        let pointer = sheet.pointer?;
        let bounds = self.surface.get()?;
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

    // ---- drawing ----------------------------------------------------------

    pub(super) fn workbook_body(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let generation = self.generation;
        let zoom = self.zoom.unwrap_or(1.0);
        self.shown_zoom = zoom;
        let toolbar = self.sheet_toolbar(cx);
        let format_dialog = self
            .format_dialog_element(cx)
            .map(|dialog| dialog.into_any_element());
        let (area_width, area_height) = editor_area(window, cx);
        let scale = window.scale_factor();
        let Document::Workbook(sheet) = &mut self.document else {
            unreachable!()
        };
        let size = (
            area_width.max(1.0),
            (area_height - SHEET_TOOLBAR_HEIGHT - FORMULA_BAR_HEIGHT - SHEET_TABS_HEIGHT).max(1.0),
        );
        sheet.viewport = size;
        let offset = sheet.scroll.offset();
        let at = (-f32::from(offset.x), -f32::from(offset.y));
        let device = (
            (size.0 * scale).round() as u32,
            (size.1 * scale).round() as u32,
        );
        let overlay = sheet.overlay();
        // The cells as tiles with the selection drawn over them; while a
        // copy, a fill or a resize shows over the cells, one picture of the
        // viewport with all of it painted in.
        let tiled =
            overlay.preview.is_none() && overlay.marquee.is_none() && overlay.resize.is_none();
        let mut layers = None;
        if tiled {
            if let Some(old) = sheet.painted.take() {
                self.retired.push(old.image);
            }
            layers = sheet.tiles.layers(
                &mut sheet.book,
                (
                    (f64::from(at.0) * f64::from(scale)).round(),
                    (f64::from(at.1) * f64::from(scale)).round(),
                ),
                size,
                scale,
                zoom,
                sheet.selection,
                &mut self.retired,
            );
            if layers.is_none() {
                self.status = i18n::text("sheet.paint_failed").into();
            }
        } else {
            sheet.tiles.clear(&mut self.retired);
            let fresh = sheet.painted.as_ref().is_some_and(|p| {
                p.scroll == at
                    && p.device == device
                    && p.zoom == zoom
                    && p.overlay == overlay
                    && p.generation == generation
            });
            if !fresh {
                let painter = sheet.book.render_viewport(
                    (f64::from(at.0 / zoom), f64::from(at.1 / zoom)),
                    device.0,
                    device.1,
                    f64::from(scale * zoom),
                    &overlay,
                );
                match painter.and_then(image) {
                    Some(image) => {
                        if let Some(old) = sheet.painted.replace(PaintedSheet {
                            scroll: at,
                            device,
                            zoom,
                            overlay,
                            generation,
                            image,
                        }) {
                            self.retired.push(old.image);
                        }
                    }
                    None => self.status = i18n::text("sheet.paint_failed").into(),
                }
            }
        }
        let rule_dialog = self.rule_dialog(cx);
        let rules_manager = self.rules_manager(cx);
        let chart_dialog = self.chart_dialog(cx);
        let object_frame = self.object_frame();
        let find_bar = self.sheet_find_bar(cx);
        let paste_special = self.paste_special_dialog(cx);
        let table_panel = self.table_panel(cx);
        let pivot_panel = self.pivot_panel(cx);
        let link_panel = self.link_panel(cx);
        let subtotal_panel = self.subtotal_panel(cx);
        let text_columns_panel = self.text_columns_panel(cx);
        let sort_panel = self.sort_panel(cx);
        let duplicates_panel = self.duplicates_panel(cx);
        let protection_panel = self.protection_panel(cx);
        let validation_panel = self.validation_panel(cx);
        let validation_alert = self.validation_alert_panel(cx);
        let validation_input = self.validation_input_panel();
        let save_as_bar = self.save_as_bar();
        let Document::Workbook(sheet) = &mut self.document else {
            unreachable!()
        };
        let (sheet_width, sheet_height) = sheet.book.sheet_size();
        let grid: Vec<gpui::AnyElement> = match layers {
            Some(layers) => layers,
            None => sheet
                .painted
                .as_ref()
                .map(|p| {
                    img(p.image.clone())
                        .w(px(size.0))
                        .h(px(size.1))
                        .into_any_element()
                })
                .into_iter()
                .collect(),
        };
        let current = sheet.book.sheet_index();
        let active = sheet.selection.active;

        let editing = sheet.edit.is_some();
        let formula = match &sheet.edit {
            Some(edit) => caret_line(&edit.text, palette::accent().into(), true),
            None => div().child(sheet.book.edit_text(active)),
        };
        // The name box: the active cell, the size of a range being dragged,
        // or what is being typed into it.
        let name_box = match (&sheet.naming, sheet.drag) {
            (Some(text), _) => caret_line(text, palette::accent().into(), false),
            (None, Some(SheetDrag::Select(_))) if !sheet.selection.is_cell() => {
                div().child(grid::size_label(sheet.selection.range()))
            }
            _ => div().child(reference_text(active)),
        };

        // The editor over the active cell, white like the cell under it.
        let cell_editor = sheet.edit.as_ref().and_then(|edit| {
            let (x, y, w, h) = sheet
                .book
                .cell_rect(active, (f64::from(at.0 / zoom), f64::from(at.1 / zoom)))?;
            let lines = edit.text.text.matches('\n').count() as f32 + 1.0;
            Some(
                div()
                    .absolute()
                    .left(px(x as f32 * zoom))
                    .top(px(y as f32 * zoom))
                    .min_w(px(w as f32 * zoom))
                    .min_h(px((h as f32 * zoom).max(lines * 17.0 * zoom)))
                    .flex()
                    .items_center()
                    .px(px(3.0 * zoom))
                    .bg(gpui::white())
                    .border_2()
                    .border_color(rgb(0x217346))
                    .text_color(gpui::black())
                    .text_size(px(14.0 * zoom))
                    .child(caret_line(&edit.text, gpui::black(), true)),
            )
        });

        let renaming = sheet.renaming.as_ref().map(|(index, text)| (*index, text));
        // A hidden sheet has no tab, as in Excel.
        let visible = sheet.book.sheet_tabs();
        let mut tabs: Vec<gpui::AnyElement> = sheet
            .names
            .iter()
            .enumerate()
            .filter(|(index, _)| visible.contains(index))
            .map(|(index, name)| {
                let selected = index == current;
                // A coloured tab (sheetPr/tabColor) is filled with its colour,
                // and underlined with it when selected, as in Excel.
                let tab_color = sheet.book.tab_color(index);
                let tab_fill =
                    tab_color.map(|[r, g, b]| gpui::rgb(u32::from_be_bytes([0, r, g, b])));
                let light_tab = tab_color.is_some_and(|[r, g, b]| {
                    u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114 > 128_000
                });
                let label = match renaming {
                    Some((renamed, text)) if renamed == index => {
                        caret_line(text, palette::accent().into(), false)
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
                    .border_color(palette::border())
                    .text_color(if selected {
                        palette::accent().into()
                    } else if tab_fill.is_some() {
                        if light_tab {
                            gpui::black()
                        } else {
                            gpui::white()
                        }
                    } else {
                        palette::dim().into()
                    })
                    .when(!selected, |tab| {
                        tab.when_some(tab_fill, |tab, fill| tab.bg(fill))
                    })
                    .when(selected, |tab| {
                        tab.bg(palette::window()).child(
                            div()
                                .absolute()
                                .bottom_0()
                                .left_1()
                                .right_1()
                                .h(px(if tab_fill.is_some() { 3.0 } else { 2.0 }))
                                .rounded_t_sm()
                                .bg(tab_fill
                                    .map_or_else(|| palette::accent().into(), gpui::Hsla::from)),
                        )
                    })
                    .hover(|tab| tab.bg(palette::hovered()))
                    .child(label)
                    .on_click(
                        cx.listener(move |view, event: &gpui::ClickEvent, window, cx| {
                            window.focus(&view.focus, cx);
                            if event.click_count() >= 2 {
                                view.sheet_menu_command(MenuCommand::RenameSheet(index), false, cx);
                            } else {
                                view.select_sheet(index, cx);
                            }
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |view, event: &MouseDownEvent, window, cx| {
                            window.focus(&view.focus, cx);
                            view.select_sheet(index, cx);
                            let origin = view.surface.get().map(|b| b.origin).unwrap_or_default();
                            if let Some(sheet) = view.sheet_mut() {
                                sheet.menu = Some(SheetMenu {
                                    at: (
                                        f32::from(event.position.x - origin.x),
                                        f32::from(event.position.y - origin.y) - 150.0,
                                    ),
                                    tab: Some(index),
                                });
                            }
                            cx.notify();
                        }),
                    )
                    .into_any_element()
            })
            .collect();
        // Excel's ⊕ after the tabs: a new sheet.
        tabs.push(
            div()
                .id("sheet-add")
                .h_full()
                .flex()
                .items_center()
                .px_3()
                .text_sm()
                .cursor_pointer()
                .text_color(palette::dim())
                .hover(|tab| tab.bg(palette::hovered()).text_color(palette::text()))
                .child("+")
                .on_click(cx.listener(|view, _event, window, cx| {
                    window.focus(&view.focus, cx);
                    view.sheet_menu_command(MenuCommand::InsertSheet, false, cx);
                }))
                .into_any_element(),
        );

        let menu = sheet.menu.as_ref().map(|menu| {
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
                    for hidden in sheet.book.hidden_sheets() {
                        items.push((MenuCommand::UnhideSheet(hidden), "sheet.menu.unhide_sheet"));
                    }
                    if index > 0 {
                        items.push((
                            MenuCommand::MoveSheet(index, index - 1),
                            "sheet.menu.move_left",
                        ));
                    }
                    if index + 1 < sheet.names.len() {
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
                    if sheet.book.copied().is_some() {
                        items.push((MenuCommand::Paste, "sheet.menu.paste"));
                    }
                    if sheet.book.can_paste_special(None) {
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
                    if !sheet.selection.is_whole_columns() {
                        items.extend([
                            (MenuCommand::InsertRows, "sheet.menu.insert_rows"),
                            (MenuCommand::DeleteRows, "sheet.menu.delete_rows"),
                        ]);
                    }
                    if !sheet.selection.is_whole_rows() {
                        items.extend([
                            (MenuCommand::InsertColumns, "sheet.menu.insert_columns"),
                            (MenuCommand::DeleteColumns, "sheet.menu.delete_columns"),
                        ]);
                    }
                    if !sheet.selection.is_whole_rows() && !sheet.selection.is_whole_columns() {
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
                    if sheet.selection.is_whole_rows() || sheet.selection.is_whole_columns() {
                        items.extend([
                            (MenuCommand::Hide, "sheet.menu.hide"),
                            (MenuCommand::Unhide, "sheet.menu.unhide"),
                        ]);
                        let axis = if sheet.selection.is_whole_columns() {
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
                        if sheet.book.has_outline(axis) {
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
                        if sheet.book.has_outline(axis) {
                            items.push((MenuCommand::ClearOutline, "sheet.menu.clear_outline"));
                        }
                    }
                    if sheet.book.note(sheet.selection.active).is_some() {
                        items.extend([
                            (MenuCommand::EditNote, "sheet.menu.edit_note"),
                            (MenuCommand::DeleteNote, "sheet.menu.delete_note"),
                        ]);
                    } else {
                        items.push((MenuCommand::EditNote, "sheet.menu.insert_note"));
                    }
                    items.push(if sheet.book.frozen_panes().is_some() {
                        (MenuCommand::UnfreezePanes, "sheet.menu.unfreeze_panes")
                    } else {
                        (MenuCommand::FreezePanes, "sheet.menu.freeze_panes")
                    });
                    if !sheet.selection.is_whole_rows()
                        && !sheet.selection.is_whole_columns()
                        && sheet.selection.range().end.row > sheet.selection.range().start.row
                        && sheet.selection.range().end.column > sheet.selection.range().start.column
                    {
                        items.push((MenuCommand::Subtotal, "sheet.menu.subtotal"));
                        items.push((
                            MenuCommand::RemoveDuplicates,
                            "sheet.menu.remove_duplicates",
                        ));
                    }
                    let selected = sheet.selection.range();
                    items.push((MenuCommand::SortDialog, "sheet.menu.sort_dialog"));
                    if selected.start.column == selected.end.column {
                        items.push((MenuCommand::TextToColumns, "sheet.menu.text_to_columns"));
                    }
                    if sheet.book.manual_calculation() {
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
                        MenuCommand::EditRanges(sheet.book.sheet_index()),
                        "protection.ranges_menu",
                    ));
                    items.push((MenuCommand::CreatePivot, "pivot.create"));
                    if sheet.book.has_pivots() {
                        items.push((MenuCommand::RefreshAllPivots, "pivot.refresh_all"));
                    }
                    if sheet.book.pivot_at(sheet.selection.active).is_some() {
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
                    ]);
                    items
                }
            };
            let max_height = (sheet.viewport.1 - 16.).max(80.);
            let swatch_rows = if menu.tab.is_some() { 3. } else { 0. };
            let height = ((items.len() as f32 + swatch_rows) * 26. + 8.).min(max_height);
            let top = menu.at.1.clamp(0., (sheet.viewport.1 - height).max(0.));
            div()
                .id("sheet-context-menu")
                .absolute()
                .left(px(menu.at.0))
                .top(px(top))
                .min_w(px(180.0))
                .max_h(px(max_height))
                .overflow_y_scroll()
                .occlude()
                .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                .py_1()
                .rounded_md()
                .border_1()
                .border_color(palette::border())
                .bg(palette::panel())
                .shadow_lg()
                .text_xs()
                .text_color(palette::text())
                .children(
                    items
                        .into_iter()
                        .enumerate()
                        .map(|(index, (command, label))| {
                            let label: SharedString = match command {
                                MenuCommand::UnhideSheet(index) => {
                                    format!("{}: {}", i18n::text(label), sheet.names[index]).into()
                                }
                                _ => i18n::text(label).into(),
                            };
                            div()
                                .id(("sheet-menu", index))
                                .px_3()
                                .py_1()
                                .cursor_pointer()
                                .hover(|item| item.bg(palette::hovered()))
                                .child(label)
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(cx.listener(move |view, _event, window, cx| {
                                    window.focus(&view.focus, cx);
                                    view.sheet_menu_command(command, false, cx);
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
                            .border_color(palette::border())
                            .cursor_pointer()
                            .when_some(color, |swatch, color| {
                                swatch.bg(gpui::rgb(u32::from_str_radix(color, 16).unwrap_or(0)))
                            })
                            .when(color.is_none(), |swatch| swatch.child("╱"))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(move |view, _event, window, cx| {
                                window.focus(&view.focus, cx);
                                view.sheet_menu_command(
                                    MenuCommand::TabColor(tab, color),
                                    false,
                                    cx,
                                );
                            }))
                    };
                    list.child(
                        div()
                            .px_3()
                            .pt_1()
                            .text_color(palette::text_dim())
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
                })
        });

        let note = sheet.note.and_then(|(cell, (x, y))| {
            let text = sheet.book.hover_text(cell)?.to_string();
            Some(
                div()
                    .absolute()
                    .left(px(x + 6.0))
                    .top(px(y))
                    .max_w(px(240.0))
                    .p_2()
                    .bg(rgb(0xfff9c4))
                    .border_1()
                    .border_color(rgb(0x9e9e9e))
                    .shadow_md()
                    .text_xs()
                    .text_color(gpui::black())
                    .child(text),
            )
        });
        // A cell with a list of choices shows Excel's ▼ beside it, and the
        // list under it when open.
        let list_button = (sheet.edit.is_none())
            .then(|| sheet.book.validation_dropdown(active))
            .flatten()
            .and_then(|_| {
                let (x, y, w, h) = sheet
                    .book
                    .cell_rect(active, (f64::from(at.0 / zoom), f64::from(at.1 / zoom)))?;
                let (x, y, w, h) = (
                    x as f32 * zoom,
                    y as f32 * zoom,
                    w as f32 * zoom,
                    h as f32 * zoom,
                );
                let items = sheet.choices.clone().unwrap_or_default();
                let open = sheet.choices.is_some();
                Some(
                    div()
                        .absolute()
                        .left(px(x + w + 1.0))
                        .top(px(y))
                        .child(
                            div()
                                .id("choices-button")
                                .w(px(h.max(14.0)))
                                .h(px(h))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(rgb(0xf3f3f3))
                                .border_1()
                                .border_color(rgb(0xababab))
                                .cursor_pointer()
                                .child(
                                    svg()
                                        .path("icons/chevron-down.svg")
                                        .size_3()
                                        .text_color(gpui::black()),
                                )
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(cx.listener(move |view, _event, window, cx| {
                                    window.focus(&view.focus, cx);
                                    if let Some(sheet) = view.sheet_mut() {
                                        sheet.choices = if sheet.choices.is_some() {
                                            None
                                        } else {
                                            sheet.book.validation_dropdown(sheet.selection.active)
                                        };
                                    }
                                    cx.notify();
                                })),
                        )
                        .when(open, |anchor| {
                            anchor.child(
                                div()
                                    .absolute()
                                    .top(px(h))
                                    .right_0()
                                    .min_w(px(w.max(120.0)))
                                    .max_h(px(220.0))
                                    .bg(gpui::white())
                                    .border_1()
                                    .border_color(rgb(0x7a7a7a))
                                    .shadow_md()
                                    .text_xs()
                                    .text_color(gpui::black())
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
                                                        .hover(|row| row.bg(rgb(0xcce8ff)))
                                                        .child(item)
                                                        .on_mouse_down(
                                                            MouseButton::Left,
                                                            |_, _, cx| cx.stop_propagation(),
                                                        )
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
            });
        let note_editor = sheet.noting.as_ref().and_then(|(cell, text)| {
            let (x, y, w, _) = sheet
                .book
                .cell_rect(*cell, (f64::from(at.0 / zoom), f64::from(at.1 / zoom)))?;
            Some(
                div()
                    .absolute()
                    .left(px((x + w) as f32 * zoom + 6.0))
                    .top(px(y as f32 * zoom))
                    .min_w(px(160.0))
                    .min_h(px(60.0))
                    .p_2()
                    .bg(rgb(0xfff9c4))
                    .border_1()
                    .border_color(rgb(0x217346))
                    .shadow_md()
                    .text_xs()
                    .text_color(gpui::black())
                    .child(caret_line(text, gpui::black(), false)),
            )
        });
        let filter_panel = sheet.filter.as_ref().map(|panel| {
            let action = |id: &'static str, label: &'static str, command: FilterCommand| {
                div()
                    .id(id)
                    .px_3()
                    .py_1()
                    .cursor_pointer()
                    .hover(|item| item.bg(palette::hovered()))
                    .child(i18n::text(label))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |view, _event, _window, cx| {
                        view.filter_command(command, cx)
                    }))
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
                        .hover(|item| item.bg(palette::hovered()))
                        .child(tick(*checked))
                        .child(label)
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |view, _event, _window, cx| {
                            view.filter_command(FilterCommand::Toggle(index), cx)
                        }))
                },
            );
            div()
                .absolute()
                .left(px(panel.at.0.max(0.0)))
                .top(px(panel.at.1.max(0.0)))
                .w(px(220.0))
                .py_1()
                .rounded_md()
                .border_1()
                .border_color(palette::border())
                .bg(palette::panel())
                .shadow_lg()
                .text_xs()
                .text_color(palette::text())
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
                .child(div().my_1().h(px(1.0)).bg(palette::border()))
                .child(
                    div()
                        .id("filter-all")
                        .flex()
                        .gap_2()
                        .px_3()
                        .py_0p5()
                        .cursor_pointer()
                        .hover(|item| item.bg(palette::hovered()))
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
                                .bg(palette::accent())
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
                                .border_color(palette::border())
                                .child(i18n::text("sheet.filter.cancel"))
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(cx.listener(|view, _event, _window, cx| {
                                    view.filter_command(FilterCommand::Cancel, cx)
                                })),
                        ),
                )
        });

        let cursor = match sheet.hover {
            Hover::ResizeColumn => gpui::CursorStyle::ResizeLeftRight,
            Hover::ResizeRow => gpui::CursorStyle::ResizeUpDown,
            Hover::FillHandle => gpui::CursorStyle::Crosshair,
            Hover::Link => gpui::CursorStyle::PointingHand,
            Hover::Border => gpui::CursorStyle::OpenHand,
            Hover::Cells => gpui::CursorStyle::Crosshair,
        };
        let dragging = sheet.drag.is_some();

        let store = Rc::clone(&self.surface);
        let entity = cx.entity();
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(palette::canvas())
            .child(toolbar)
            // The name box and the formula bar.
            .child(
                div()
                    .h(px(FORMULA_BAR_HEIGHT))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .bg(palette::panel())
                    .border_b_1()
                    .border_color(palette::border())
                    .text_xs()
                    .child(
                        div()
                            .id("name-box")
                            .w(px(96.0))
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .border_1()
                            .border_color(palette::border())
                            .bg(palette::window())
                            .text_color(palette::text())
                            .cursor_text()
                            .child(name_box)
                            .on_click(cx.listener(|view, _event, window, cx| {
                                window.focus(&view.focus, cx);
                                if !view.commit_cell(cx) {
                                    return;
                                }
                                if let Some(sheet) = view.sheet_mut() {
                                    if sheet.naming.is_none() {
                                        sheet.naming = Some(TextEdit::selected(reference_text(
                                            sheet.selection.active,
                                        )));
                                        sheet.going_to = false;
                                    }
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .px_1()
                            .italic()
                            .text_color(palette::faint())
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
                            .text_color(palette::text())
                            .overflow_hidden()
                            .child(formula)
                            .on_click(cx.listener(move |view, _event, window, cx| {
                                window.focus(&view.focus, cx);
                                if !editing {
                                    view.begin_cell_edit(None, true, cx);
                                } else if let Some(sheet) = view.sheet_mut() {
                                    if let Some(edit) = sheet.edit.as_mut() {
                                        edit.in_place = true;
                                        edit.point = None;
                                    }
                                    cx.notify();
                                }
                            })),
                    ),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .overflow_hidden()
                    .child(
                        // The painted window sits under the scroller, which
                        // is transparent and owns the scrollbars and the
                        // mouse.
                        div().absolute().inset_0().children(grid),
                    )
                    .child(
                        div()
                            .id("sheet-scroll")
                            .absolute()
                            .inset_0()
                            .overflow_scroll()
                            .track_scroll(&sheet.scroll)
                            .cursor(cursor)
                            .on_mouse_down(MouseButton::Left, cx.listener(Self::sheet_mouse_down))
                            .on_mouse_down(MouseButton::Right, cx.listener(Self::sheet_right_click))
                            .on_mouse_move(cx.listener(
                                |view, event: &MouseMoveEvent, _window, cx| {
                                    // A release the window missed — a click
                                    // quicker than a frame — still ends the drag.
                                    if !event.dragging() {
                                        view.sheet_drag_end(cx);
                                    }
                                    view.sheet_hover(event.position, cx);
                                },
                            ))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _event: &MouseUpEvent, _window, cx| {
                                    view.sheet_drag_end(cx)
                                }),
                            )
                            .on_mouse_up_out(
                                MouseButton::Left,
                                cx.listener(|view, _event: &MouseUpEvent, _window, cx| {
                                    view.sheet_drag_end(cx)
                                }),
                            )
                            .child(
                                div()
                                    .w(px(sheet_width as f32 * zoom))
                                    .h(px(sheet_height as f32 * zoom))
                                    .flex_none(),
                            ),
                    )
                    .children(cell_editor)
                    .children(note)
                    .children(note_editor)
                    .children(list_button)
                    .children(find_bar)
                    .children(save_as_bar)
                    .children(filter_panel)
                    .children(rule_dialog)
                    .children(rules_manager)
                    .children(chart_dialog)
                    .children(object_frame)
                    .children(paste_special)
                    .children(validation_input)
                    .children(table_panel)
                    .children(pivot_panel)
                    .children(link_panel)
                    .children(subtotal_panel)
                    .children(text_columns_panel)
                    .children(sort_panel)
                    .children(duplicates_panel)
                    .children(protection_panel)
                    .children(validation_panel)
                    .children(validation_alert)
                    .children(menu)
                    .children(format_dialog)
                    // Records where the grid is, registers the input handler
                    // so typing — an IME's included — reaches the cell, and
                    // while a drag is on follows the pointer anywhere in the
                    // window.
                    .child(
                        canvas(
                            move |bounds, _window, _cx| {
                                store.set(Some(bounds));
                                bounds
                            },
                            move |_bounds, grid, window, cx| {
                                let focus = entity.read(cx).focus.clone();
                                window.handle_input(
                                    &focus,
                                    ElementInputHandler::new(grid, entity.clone()),
                                    cx,
                                );
                                if dragging {
                                    let moving = entity.clone();
                                    window.on_mouse_event(
                                        move |event: &MouseMoveEvent, phase, _window, cx| {
                                            if phase == DispatchPhase::Bubble && event.dragging() {
                                                moving.update(cx, |view, cx| {
                                                    view.sheet_drag_to(event.position, cx)
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
                                                releasing
                                                    .update(cx, |view, cx| view.sheet_drag_end(cx));
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
                    .bg(palette::panel())
                    .border_t_1()
                    .border_color(palette::border())
                    .children(tabs),
            )
    }
}

/// Text being typed, with its caret drawn where it is and its lines kept.
pub(super) fn caret_line(text: &TextEdit, caret_color: gpui::Hsla, tall: bool) -> gpui::Div {
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
    let caret = div().flex_none().w(px(1.0)).h(px(height)).bg(caret_color);
    let line = |text: &str| div().whitespace_nowrap().child(text.to_string());
    let mut lines = div().flex().flex_col();
    let mut caret = Some(caret);
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
        row = row.child(line(after_lines[0]));
        lines = lines.child(row);
    }
    for part in after_lines.iter().skip(1) {
        lines = lines.child(line(part));
    }
    lines
}

fn format_number(value: f64) -> String {
    jade_web::sheet::format_number(value)
}

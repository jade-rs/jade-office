//! The spreadsheet (XLSX) view: a sheet you can select in, type into, and
//! save.
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
use jade_office_model::sheet::{
    format_number, range_text, span, CellRange, CellRef, Workbook, MAX_COLUMN, MAX_ROW,
};
use jade_office_model::skia::SkiaBackend;
use jade_office_model::Backend;
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

/// The name box and formula bar above the grid.
const FORMULA_BAR_HEIGHT: f32 = 32.0;
/// The strip of sheet tabs under the grid.
const SHEET_TABS_HEIGHT: f32 = 30.0;

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
    /// The shared editing model (jade-office-model), painted with Skia.
    book: Workbook<SkiaBackend>,
    sheet_names: Vec<SharedString>,
    /// Where the selection started; also the active cell.
    anchor: CellRef,
    /// The corner the selection was extended to.
    focus_cell: CellRef,
    edit: Option<CellEdit>,
    dragging: bool,
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
        self.anchor
    }

    fn range(&self) -> CellRange {
        span(self.anchor, self.focus_cell)
    }

    fn selection_label(&self) -> String {
        range_text(self.range())
    }

    fn select_sheet(&mut self, index: usize, cx: &mut Context<Self>) {
        if index == self.book.sheet_index() || index >= self.sheet_names.len() {
            return;
        }
        if !self.commit_edit(cx) {
            return;
        }
        if let Err(error) = self.book.select_sheet(index) {
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
        self.book.hit(
            f64::from(position.x - bounds.origin.x) / zoom,
            f64::from(position.y - bounds.origin.y) / zoom,
            self.sheet_scroll(),
        )
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

    /// Scrolls just enough to show `reference`.
    fn reveal(&mut self, reference: CellRef) {
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

    fn select(&mut self, anchor: CellRef, focus: CellRef, cx: &mut Context<Self>) {
        self.anchor = anchor;
        self.focus_cell = focus;
        self.book.reach(focus);
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
        if !self.commit_edit(cx) {
            return;
        }
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
        let text = text.unwrap_or_else(|| self.book.edit_text(self.active()));
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
    ///
    /// Returns whether the edit is done. A formula Excel would refuse keeps
    /// its editor open with what was typed, as Excel does, so the caller
    /// stays where it is rather than moving on without it.
    fn commit_edit(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(edit) = self.edit.take() else {
            return true;
        };
        let result = self.book.set_cell(self.active(), &edit.text);
        let refused =
            matches!(&result, Err(error) if error == jade_office_model::typing::FORMULA_PROBLEM);
        self.after_write(result, cx);
        if refused {
            self.edit = Some(edit);
        }
        cx.notify();
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

    fn commit_and_step(&mut self, columns: i64, rows: i64, cx: &mut Context<Self>) {
        if self.commit_edit(cx) {
            self.step(columns, rows, false, cx);
        }
    }

    /// Delete: empties the selected cells that hold anything, keeping their
    /// formats.
    fn clear_selection(&mut self, cx: &mut Context<Self>) {
        let result = self.book.clear(self.range());
        self.after_write(result, cx);
    }

    fn step_history(&mut self, undo: bool, cx: &mut Context<Self>) {
        self.edit = None;
        let result = if undo {
            self.book.undo()
        } else {
            self.book.redo()
        };
        self.after_write(result, cx);
    }

    // ---- clipboard -------------------------------------------------------

    /// The selection's displayed text, tab-separated by column and
    /// newline-separated by row — what Excel puts on the clipboard as text.
    fn selection_text(&self) -> String {
        self.book.range_text(self.range())
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
        match self.book.paste(origin, &text) {
            Ok(Some(far)) => {
                self.after_write(Ok(true), cx);
                self.select(origin, far, cx);
            }
            Ok(None) => {}
            Err(error) => self.after_write(Err(error), cx),
        }
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
        if !self.commit_edit(cx) {
            return;
        }
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
        self.anchor = CellRef::new(0, 0);
        self.focus_cell = self.book.grid_end();
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
            sheet: self.book.sheet_index(),
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
        let zoom = f64::from(self.zoom);
        let painter = self.book.render_viewport(
            (
                f64::from(wanted.scroll.0) / zoom,
                f64::from(wanted.scroll.1) / zoom,
            ),
            wanted.size.0 as u32,
            wanted.size.1 as u32,
            f64::from(wanted.scale),
            Some((self.anchor, self.focus_cell)),
        )?;
        let (bgra, width, height) = SkiaBackend::bgra(&painter)?;
        let buffer = image::ImageBuffer::from_raw(width, height, bgra)?;
        Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
    }

    fn formula_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let editing = self.edit.as_ref().map(|edit| edit.text.clone());
        let content = editing
            .clone()
            .unwrap_or_else(|| self.book.edit_text(self.active()));
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
            i18n::format("sheet.position", &[("name", &self.book.sheet_name())]).into(),
            self.selection_label().into(),
        ];
        // Excel's summary of a multi-cell selection: average, count, sum.
        let range = self.range();
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

        let (sheet_width, sheet_height) = self.book.sheet_size();
        let (sheet_width, sheet_height) = (
            sheet_width as f32 * self.zoom,
            sheet_height as f32 * self.zoom,
        );
        let current = self.book.sheet_index();
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
            anchor: CellRef::new(0, 0),
            focus_cell: CellRef::new(0, 0),
            edit: None,
            dragging: false,
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
}

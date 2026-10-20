//! The presentation (PPTX) view.
//!
//! Laid out the way PowerPoint's Normal view is: slide thumbnails down the
//! left, the selected slide as large as the space allows, its speaker notes
//! underneath. A slide show opens its own full-screen window.
//!
//! Editing goes through jade-pptx-writer's `EditSession`, the same one the
//! browser editor uses: every change rewrites the slide part in the package
//! bytes and keeps undo history, and the view reads the deck back from those
//! bytes to paint it. On a slide you can select a shape (a click, or Tab to
//! step through them), move it (drag, or the arrow keys), resize it from its
//! corners, delete it, and type into it — an IME's composition included.
//!
//! ## Why a slide is repainted at the size it is shown
//!
//! A slide is a fixed-size canvas, so unlike a sheet it could be painted once
//! and scaled. Scaling a bitmap blurs text, though, and a slide is mostly text
//! at sizes where that shows. The layout is cached by the deck, so painting
//! again at a new width is only the Skia pass.

use gpui::{
    anchored, canvas, deferred, div, img, point, prelude::*, px, rgb, size, svg, AnyElement, App,
    Bounds, ClickEvent, Context, Div, ElementId, ElementInputHandler, Entity, EntityInputHandler,
    FocusHandle, Focusable, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels,
    RenderImage, ScrollHandle, SharedString, Stateful, UTF16Selection, Window, WindowBounds,
    WindowOptions,
};
use jade_office_model::deck::{
    Deck, DragMode, FormatStrip, Insert, Motion, ParagraphAlignment, ShapeDrag,
    ShapeRect as SlideRect, ShapeText, ShapeTransformInfo, TableChange, TextCommand,
    TextSelectionGeometry, TextTarget, FONT_SIZES, SHAPE_PRESETS,
};
use jade_office_model::skia::SkiaBackend;
use jade_office_model::text::TextEdit as NotesEdit;
use jade_office_model::Backend;
use std::cell::Cell;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use crate::editor::{self, Editor};
use crate::tooltip::Tooltip;
use crate::{
    byte_to_utf16, i18n, native_print, theme, utf16_to_byte, workspace, Print, Redo, Save, SaveAs,
    SelectAll, StartSlideshow, Undo, UI_FONT_FAMILY,
};

/// The thumbnail strip's width, and the width each thumbnail is shown at.
const SIDEBAR_WIDTH: f32 = 200.0;
const THUMBNAIL_WIDTH: f32 = 152.0;
/// The bar above the slide: slide number, print, slide show.
const TOOLBAR_HEIGHT: f32 = 40.0;
/// The format strip under it: font and paragraph.
const FORMAT_BAR_HEIGHT: f32 = 36.0;
/// The speaker-notes pane under the slide.
const NOTES_HEIGHT: f32 = 110.0;
/// Space left round the slide inside its canvas.
const SLIDE_MARGIN: f32 = 28.0;
/// How far an arrow key moves a shape, in slide px; ⌥ moves one px.
const NUDGE_PX: f64 = 6.0;
/// The size of a selection's corner handles, in window px.
const HANDLE_SIZE: f32 = 9.0;

/// A painted slide and what it was painted for.
struct Painted {
    index: usize,
    device_width: i32,
    image: Arc<RenderImage>,
}

/// Paints slide `index` of `deck` `device_width` px wide, for gpui.
fn slide_image(
    deck: &mut Deck<SkiaBackend>,
    index: usize,
    device_width: i32,
) -> Option<Arc<RenderImage>> {
    let painter = deck.render_slide(index, device_width.max(1) as u32)?;
    let (bgra, width, height) = SkiaBackend::bgra(&painter)?;
    let buffer = image::ImageBuffer::from_raw(width, height, bgra)?;
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
}

/// The slide's on-screen size to fit `available`, keeping its aspect.
fn fitted(deck: &Deck<SkiaBackend>, available: (f32, f32)) -> (f32, f32) {
    let (slide_width, slide_height) = deck.slide_size();
    let (slide_width, slide_height) = (slide_width as f32, slide_height as f32);
    let scale = (available.0 / slide_width.max(1.0))
        .min(available.1 / slide_height.max(1.0))
        .max(0.05);
    (slide_width * scale, slide_height * scale)
}

/// Repaints `painted` unless it already shows slide `index` at `device_width`,
/// queueing the texture it replaces for release.
fn repaint(
    deck: &mut Deck<SkiaBackend>,
    painted: &mut Option<Painted>,
    retired: &mut Vec<Arc<RenderImage>>,
    index: usize,
    device_width: i32,
) -> bool {
    if matches!(painted, Some(p) if p.index == index && p.device_width == device_width) {
        return true;
    }
    let Some(image) = slide_image(deck, index, device_width) else {
        return false;
    };
    if let Some(previous) = painted.replace(Painted {
        index,
        device_width,
        image,
    }) {
        retired.push(previous.image);
    }
    true
}

/// Text being typed into a shape, or a cell of a table.
struct TextEdit {
    target: TextTarget,
    /// Where the text is shown, in slide px: the shape, or the cell.
    rect: SlideRect,
    text: String,
    /// The IME's composition, as a byte range of `text`.
    marked: Option<Range<usize>>,
    /// All the text is selected, as after Tab into a table cell: what is
    /// typed next replaces it.
    whole: bool,
}

impl TextEdit {
    /// Ends a whole-text selection, emptying the text it selected when
    /// something replaces it.
    fn take_whole(&mut self, replace: bool) {
        if std::mem::take(&mut self.whole) && replace {
            self.text.clear();
        }
    }
}

/// A list open under a toolbar button.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Popup {
    /// The layouts the current slide can take.
    Layouts,
    Font,
    Size,
    Color,
    Spacing,
    /// Insert's shapes.
    Shapes,
}

/// A thumbnail being dragged to another place in the list.
#[derive(Clone, Copy)]
struct DraggedSlide(usize);

/// What follows the pointer while a thumbnail is dragged.
struct DraggedSlideView(usize);

impl Render for DraggedSlideView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(theme::accent())
            .text_xs()
            .text_color(gpui::white())
            .font_family(UI_FONT_FAMILY)
            .child(i18n::format("deck.slide_n", &[("n", &(self.0 + 1))]))
    }
}

/// Cells selected across a table, as by Shift+click: what Merge Cells merges.
struct CellSelection {
    id: u32,
    from: (usize, usize),
    to: (usize, usize),
    /// The cells' box, in slide px.
    rect: SlideRect,
}

pub struct DeckView {
    path: PathBuf,
    /// The shared editing model (jade-office-model), painted with Skia.
    deck: Deck<SkiaBackend>,
    current: usize,
    /// The current slide's shapes as the writer sees them: what can be
    /// selected, moved, resized.
    shapes: Vec<ShapeTransformInfo>,
    selected: Option<u32>,
    drag: Option<ShapeDrag>,
    /// Text being typed into a shape where it lies: its caret, selection
    /// and an IME's composition.
    typing: Option<ShapeText>,
    /// A press in the text being typed into is being dragged, selecting.
    selecting_text: bool,
    /// What the next drag on the slide draws: a text box or a shape.
    inserting: Option<Insert>,
    /// That drag, from where it started to where it is, in slide px.
    drawing: Option<((f64, f64), (f64, f64))>,
    /// Text being typed into a table cell, in a box over it.
    text_edit: Option<TextEdit>,
    /// The speaker notes being typed into, written when they are left.
    notes_edit: Option<NotesEdit>,
    cells: Option<CellSelection>,
    /// One per slide, painted on the first frame (it needs the window's scale
    /// factor) and kept: a thumbnail only changes when the slide does.
    thumbnails: Vec<Option<Arc<RenderImage>>>,
    slide: Option<Painted>,
    /// Textures replaced since the last frame, released in `render`, which is
    /// the only place with a window to hand.
    retired: Vec<Arc<RenderImage>>,
    show_notes: bool,
    /// `None` fits the slide to the space; `Some` is a zoom the reader chose.
    zoom: Option<f32>,
    /// The scale the slide was last shown at, window px per slide px.
    shown_scale: f32,
    /// Where the slide sits in the window, recorded at paint.
    slide_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    thumbnail_scroll: ScrollHandle,
    /// The list open under a toolbar button, if any.
    popup: Option<Popup>,
    /// Keys go to the slide list — Delete deletes the slide — after a click
    /// on a thumbnail, as in PowerPoint's thumbnail pane.
    slides_focused: bool,
    focus: FocusHandle,
    status: SharedString,
}

impl DeckView {
    fn go_to(&mut self, index: usize, cx: &mut Context<Self>) {
        let index = index.min(self.deck.slide_count().saturating_sub(1));
        if index != self.current {
            self.commit_text(cx);
            self.current = index;
            self.selected = None;
            self.drag = None;
            self.refresh_shapes();
            self.thumbnail_scroll.scroll_to_item(index);
            cx.notify();
        }
    }

    fn step(&mut self, delta: isize, cx: &mut Context<Self>) {
        self.go_to(self.current.saturating_add_signed(delta), cx);
    }

    fn start_slideshow(&mut self, from: usize, cx: &mut Context<Self>) {
        self.commit_text(cx);
        if let Err(error) = open_slideshow(self.deck.bytes().to_vec(), from, cx) {
            self.status = SharedString::from(error);
            cx.notify();
        }
    }

    fn print(&mut self, cx: &mut Context<Self>) {
        let result = self
            .deck
            .pdf()
            .and_then(|pdf| native_print::show_pdf_print_dialog(&pdf));
        if let Err(error) = result {
            self.status =
                SharedString::from(i18n::format("deck.print_failed", &[("error", &error)]));
            cx.notify();
        }
    }

    // ---- slides ----------------------------------------------------------

    /// Runs a change to the deck's slides; when it changes something, shows
    /// slide `then` with every thumbnail repainted, as slides have moved.
    fn change_slides(
        &mut self,
        then: usize,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Deck<SkiaBackend>) -> Result<bool, String>,
    ) -> bool {
        self.commit_text(cx);
        self.popup = None;
        match change(&mut self.deck) {
            Ok(true) => {
                self.selected = None;
                self.drag = None;
                self.cells = None;
                self.current = then;
                self.reload(true, cx);
                self.thumbnail_scroll.scroll_to_item(self.current);
                true
            }
            Ok(false) => false,
            Err(error) => {
                self.status =
                    SharedString::from(i18n::format("deck.edit_failed", &[("error", &error)]));
                cx.notify();
                false
            }
        }
    }

    /// New Slide: an empty one after the current slide, on its layout.
    fn new_slide(&mut self, cx: &mut Context<Self>) {
        let at = self.current + 1;
        self.change_slides(at, cx, |deck| deck.insert_slide(at));
    }

    fn duplicate_slide(&mut self, cx: &mut Context<Self>) {
        let index = self.current;
        self.change_slides(index + 1, cx, |deck| deck.duplicate_slide(index));
    }

    /// Deletes the current slide, showing the one after it — or before, for
    /// the last.
    fn delete_slide(&mut self, cx: &mut Context<Self>) {
        let index = self.current;
        self.change_slides(index, cx, |deck| deck.delete_slide(index));
    }

    /// A thumbnail dropped on another: the slide moves to its place.
    fn move_slide(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        self.change_slides(to, cx, |deck| deck.move_slide(from, to));
    }

    fn toggle_hidden(&mut self, cx: &mut Context<Self>) {
        let index = self.current;
        let hidden = !self.deck.is_hidden(index);
        self.change_slides(index, cx, |deck| deck.set_slide_hidden(index, hidden));
    }

    fn set_layout(&mut self, part_name: &str, cx: &mut Context<Self>) {
        let index = self.current;
        self.change_slides(index, cx, |deck| deck.set_slide_layout(index, part_name));
    }

    // ---- formatting ------------------------------------------------------

    /// What the format strip shows: the text typed into, or all of the
    /// selected shape's — `None` when neither has text to format.
    fn format_strip(&mut self) -> Option<FormatStrip> {
        if let Some(typing) = &self.typing {
            return Some(typing.format_strip(&self.deck));
        }
        let id = self
            .selected
            .filter(|_| self.text_edit.is_none() && self.cells.is_none())?;
        self.deck.shape_format(self.current, id)
    }

    /// A format strip command — or ⌘B and the rest — on the text typed
    /// into, or on all of the selected shape's.
    fn format_slide_text(&mut self, command: TextCommand, cx: &mut Context<Self>) {
        self.popup = None;
        if self.typing.is_some() {
            self.edit_typing(cx, |typing, deck| typing.format(deck, &command));
            return;
        }
        let Some(id) = self
            .selected
            .filter(|_| self.text_edit.is_none() && self.cells.is_none())
        else {
            cx.notify();
            return;
        };
        self.write(cx, |deck, slide| deck.format_shape(slide, id, &command));
        cx.notify();
    }

    fn format_shortcut(&mut self, key: &str, shift: bool, cx: &mut Context<Self>) -> bool {
        let command = match (key, shift) {
            ("b", false) => TextCommand::Bold,
            ("i", false) => TextCommand::Italic,
            ("u", false) => TextCommand::Underline,
            ("l", false) => TextCommand::Align(ParagraphAlignment::Left),
            ("e", false) => TextCommand::Align(ParagraphAlignment::Center),
            ("r", false) => TextCommand::Align(ParagraphAlignment::Right),
            ("j", false) => TextCommand::Align(ParagraphAlignment::Justify),
            (".", true) | (">", _) => TextCommand::Grow,
            (",", true) | ("<", _) => TextCommand::Shrink,
            _ => return false,
        };
        if self.format_strip().is_none() {
            return false;
        }
        self.format_slide_text(command, cx);
        true
    }

    // ---- inserting -------------------------------------------------------

    /// Text Box, or a shape from Shapes: the next drag on the slide draws it,
    /// as in PowerPoint; a click drops it at its usual size.
    fn start_insert(&mut self, insert: Insert, cx: &mut Context<Self>) {
        self.commit_text(cx);
        self.popup = None;
        self.inserting = Some(insert);
        self.select(None, cx);
        cx.notify();
    }

    /// Draws what is being inserted over `rect` and selects it — a text box
    /// with the caret in it.
    fn insert_at(&mut self, insert: Insert, rect: SlideRect, cx: &mut Context<Self>) {
        self.inserting = None;
        let result = self.deck.insert(self.current, insert, rect);
        let Some(id) = self.inserted(result, cx) else {
            return;
        };
        if insert == Insert::TextBox {
            self.type_in_place(id, None, cx);
        }
    }

    /// Picture: a PNG or JPEG file, placed in the middle of the slide.
    fn pick_picture(&mut self, cx: &mut Context<Self>) {
        self.commit_text(cx);
        self.popup = None;
        let prompt = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |view, cx| {
            let Ok(Ok(Some(paths))) = prompt.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            view.update(cx, |view, cx| match std::fs::read(&path) {
                Ok(bytes) => view.insert_picture(&bytes, cx),
                Err(error) => {
                    let error = format!("{}: {error}", path.display());
                    view.status = SharedString::from(i18n::format(
                        "deck.picture_failed",
                        &[("error", &error)],
                    ));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn insert_picture(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        let result = self.deck.insert_picture(self.current, bytes);
        if let Err(error) = &result {
            self.status =
                SharedString::from(i18n::format("deck.picture_failed", &[("error", error)]));
            cx.notify();
            return;
        }
        self.inserted(result, cx);
    }

    /// After an insert: the slide repainted and the new object selected.
    fn inserted(&mut self, result: Result<u32, String>, cx: &mut Context<Self>) -> Option<u32> {
        match result {
            Ok(id) => {
                self.reload(false, cx);
                self.select(Some(id), cx);
                Some(id)
            }
            Err(error) => {
                self.status =
                    SharedString::from(i18n::format("deck.edit_failed", &[("error", &error)]));
                cx.notify();
                None
            }
        }
    }

    fn toggle_popup(&mut self, popup: Popup, cx: &mut Context<Self>) {
        self.popup = (self.popup != Some(popup)).then_some(popup);
        cx.notify();
    }

    // ---- the model -------------------------------------------------------

    fn refresh_shapes(&mut self) {
        self.shapes = self.deck.shapes(self.current).to_vec();
        if let Some(id) = self.selected {
            if !self.shapes.iter().any(|shape| shape.id == id) {
                self.selected = None;
            }
        }
    }

    fn shape(&self, id: u32) -> Option<&ShapeTransformInfo> {
        self.shapes.iter().find(|shape| shape.id == id)
    }

    /// Throws away the pixels that showed the deck before a change — the
    /// model has already read itself back. `all_slides` is for undo and
    /// redo, which may have changed a slide other than this one.
    fn reload(&mut self, all_slides: bool, cx: &mut Context<Self>) {
        if let Some(painted) = self.slide.take() {
            self.retired.push(painted.image);
        }
        let count = self.deck.slide_count();
        self.thumbnails.resize(count, None);
        for (index, thumbnail) in self.thumbnails.iter_mut().enumerate() {
            if all_slides || index == self.current {
                if let Some(image) = thumbnail.take() {
                    self.retired.push(image);
                }
            }
        }
        self.current = self.current.min(count.saturating_sub(1));
        self.refresh_shapes();
        self.status = SharedString::from("");
        cx.notify();
    }

    /// Runs a change against the model and, when it changed something,
    /// repaints from the result.
    fn write(
        &mut self,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Deck<SkiaBackend>, usize) -> Result<bool, String>,
    ) -> bool {
        match change(&mut self.deck, self.current) {
            Ok(true) => {
                self.reload(false, cx);
                true
            }
            Ok(false) => false,
            Err(error) => {
                self.status =
                    SharedString::from(i18n::format("deck.edit_failed", &[("error", &error)]));
                cx.notify();
                false
            }
        }
    }

    fn step_history(&mut self, undo: bool, cx: &mut Context<Self>) {
        // Text typed in place is already in the deck: Undo takes it back
        // and the caret stays, as in PowerPoint.
        let typing = self.typing.take();
        self.commit_text(cx);
        self.typing = typing;
        let result = if undo {
            self.deck.undo()
        } else {
            self.deck.redo()
        };
        if let Some(typing) = self.typing.as_mut() {
            typing.composition = None;
            typing.clamp(&self.deck);
        }
        match result {
            Ok(true) => self.reload(true, cx),
            Ok(false) => {}
            Err(error) => {
                self.status = SharedString::from(error);
                cx.notify();
            }
        }
    }

    // ---- selection and moving --------------------------------------------

    /// The slide-px point under a window position.
    fn slide_point(&self, position: gpui::Point<Pixels>) -> Option<(f64, f64)> {
        let bounds = self.slide_bounds.get()?;
        let scale = f64::from(self.shown_scale).max(0.0001);
        Some((
            f64::from(position.x - bounds.origin.x) / scale,
            f64::from(position.y - bounds.origin.y) / scale,
        ))
    }

    /// The selectable shape at a slide point: the top-most of the slide's
    /// own shapes there, or the group it belongs to — the thing PowerPoint
    /// selects on a first click.
    fn shape_at(&mut self, x: f64, y: f64) -> Option<u32> {
        self.deck.shape_at(self.current, x, y)
    }

    fn select(&mut self, id: Option<u32>, cx: &mut Context<Self>) {
        if id.is_some() {
            self.slides_focused = false;
        }
        if self.selected != id {
            self.commit_text(cx);
            self.selected = id;
            cx.notify();
        }
    }

    /// Tab and ⇧Tab step through the slide's shapes, as in PowerPoint.
    fn cycle_selection(&mut self, backwards: bool, cx: &mut Context<Self>) {
        let selectable: Vec<u32> = self
            .shapes
            .iter()
            .filter(|shape| shape.can_select && shape.transform.is_some())
            .map(|shape| shape.id)
            .collect();
        if selectable.is_empty() {
            return;
        }
        let position = self
            .selected
            .and_then(|id| selectable.iter().position(|candidate| *candidate == id));
        let next = match (position, backwards) {
            (None, false) => 0,
            (None, true) => selectable.len() - 1,
            (Some(at), false) => (at + 1) % selectable.len(),
            (Some(at), true) => (at + selectable.len() - 1) % selectable.len(),
        };
        self.select(Some(selectable[next]), cx);
    }

    /// Moves or sizes shape `id` to `rect`, as one undoable change.
    fn place(&mut self, id: u32, rect: SlideRect, cx: &mut Context<Self>) {
        self.write(cx, |deck, slide| deck.set_shape_rect(slide, id, rect));
    }

    fn nudge(&mut self, dx: f64, dy: f64, cx: &mut Context<Self>) {
        let Some(shape) = self.selected.and_then(|id| self.shape(id)) else {
            return;
        };
        if !shape.can_move {
            return;
        }
        let (id, Some(transform)) = (shape.id, shape.transform) else {
            return;
        };
        let rect = SlideRect::of(&transform).dragged(DragMode::Move, dx, dy);
        self.place(id, rect, cx);
    }

    fn delete_selected(&mut self, cx: &mut Context<Self>) {
        let Some(shape) = self.selected.and_then(|id| self.shape(id)) else {
            return;
        };
        if !shape.can_delete {
            return;
        }
        let id = shape.id;
        self.selected = None;
        self.write(cx, |deck, slide| deck.delete_shape(slide, id));
    }

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus);
        self.slides_focused = false;
        self.popup = None;
        let Some((x, y)) = self.slide_point(event.position) else {
            return;
        };
        if self.inserting.is_some() {
            self.drawing = Some(((x, y), (x, y)));
            cx.notify();
            return;
        }
        // Shift+click in a table whose cell is being typed into, or whose
        // cells are selected, selects the cells between, as in PowerPoint.
        if event.modifiers.shift && self.select_cells_to(x, y, cx) {
            return;
        }
        self.cells = None;
        if let Some(edit) = &self.text_edit {
            if edit.rect.contains(x, y) {
                return;
            }
        }
        // In the text being typed into, a click puts the caret, ⇧ or a drag
        // selects, and a double-click selects a word.
        if let Some(id) = self.typing.as_ref().map(|typing| typing.id) {
            if self
                .deck
                .shape_rect(self.current, id)
                .is_some_and(|rect| rect.contains(x, y))
            {
                let shift = event.modifiers.shift;
                let double = event.click_count >= 2;
                self.edit_typing(cx, |typing, deck| {
                    if double {
                        typing.select_word(deck, x, y);
                    } else {
                        typing.click(deck, x, y, shift);
                    }
                    Ok(false)
                });
                self.selecting_text = !double;
                return;
            }
        }
        self.commit_text(cx);
        let hit = self.shape_at(x, y);
        self.select(hit, cx);
        let Some(id) = hit else {
            return;
        };
        if event.click_count >= 2 {
            // In a table, the cell under the click.
            self.begin_text(id, Some((x, y)), cx);
            return;
        }
        if self.shape(id).is_some_and(|shape| shape.can_move) {
            self.drag = Some(ShapeDrag::new(id, DragMode::Move, (x, y)));
        }
    }

    fn start_resize(
        &mut self,
        left: bool,
        top: bool,
        event: &MouseDownEvent,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        let Some(id) = self.selected else {
            return;
        };
        if !self.shape(id).is_some_and(|shape| shape.can_resize) {
            return;
        }
        let Some(from) = self.slide_point(event.position) else {
            return;
        };
        self.commit_text(cx);
        self.drag = Some(ShapeDrag::new(id, DragMode::Resize { left, top }, from));
    }

    fn mouse_move(&mut self, event: &MouseMoveEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if !event.dragging() {
            return;
        }
        let Some((x, y)) = self.slide_point(event.position) else {
            return;
        };
        if let Some((_, to)) = self.drawing.as_mut() {
            *to = (x, y);
            cx.notify();
            return;
        }
        if self.selecting_text {
            self.edit_typing(cx, |typing, deck| {
                typing.click(deck, x, y, true);
                Ok(false)
            });
            return;
        }
        if let Some(drag) = self.drag.as_mut() {
            drag.drag_to(x, y);
            cx.notify();
        }
    }

    fn mouse_up(&mut self, cx: &mut Context<Self>) {
        self.selecting_text = false;
        if let (Some((from, to)), Some(insert)) = (self.drawing.take(), self.inserting) {
            self.insert_at(insert, insert.rect(from, to), cx);
            return;
        }
        let Some(drag) = self.drag.take() else {
            return;
        };
        if !drag.moved() {
            cx.notify();
            return;
        }
        self.write(cx, |deck, slide| deck.drag_shape(slide, &drag));
    }

    // ---- text ------------------------------------------------------------

    /// Starts typing into shape `id`: in a table, into the cell at `point`,
    /// or the first cell without one; anything else where it lies, the
    /// caret at `point` or at the end of the text.
    fn begin_text(&mut self, id: u32, point: Option<(f64, f64)>, cx: &mut Context<Self>) {
        match self.deck.text_target(self.current, id, point) {
            TextTarget::Shape(id) => self.type_in_place(id, point, cx),
            target => self.edit(target, false, cx),
        }
    }

    fn type_in_place(&mut self, id: u32, point: Option<(f64, f64)>, cx: &mut Context<Self>) {
        match ShapeText::begin(&mut self.deck, self.current, id, point) {
            Some(typing) => {
                self.drag = None;
                self.typing = Some(typing);
            }
            None => self.status = SharedString::from(i18n::text("deck.text_locked")),
        }
        cx.notify();
    }

    /// Runs `edit` on the text being typed in place, repainting the slide
    /// when it changed the deck.
    fn edit_typing(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut ShapeText, &mut Deck<SkiaBackend>) -> Result<bool, String>,
    ) {
        let Some(mut typing) = self.typing.take() else {
            return;
        };
        let result = edit(&mut typing, &mut self.deck);
        self.typing = Some(typing);
        match result {
            Ok(true) => self.reload(false, cx),
            Ok(false) => cx.notify(),
            Err(error) => {
                self.status =
                    SharedString::from(i18n::format("deck.edit_failed", &[("error", &error)]));
                cx.notify();
            }
        }
    }

    /// The keys of text being typed in place: the caret keys — ⇧ selecting,
    /// ⌥ by words, ⌘ to the ends of the line and the text — Backspace,
    /// Delete, Enter for a paragraph and ⇧Enter for a line break, Tab.
    fn typing_key(
        &mut self,
        key: &str,
        modifiers: gpui::Modifiers,
        cx: &mut Context<Self>,
    ) -> bool {
        let shift = modifiers.shift;
        let motion = match key {
            "left" if modifiers.platform => Some(Motion::LineStart),
            "right" if modifiers.platform => Some(Motion::LineEnd),
            "up" if modifiers.platform => Some(Motion::Start),
            "down" if modifiers.platform => Some(Motion::End),
            "left" if modifiers.alt => Some(Motion::WordLeft),
            "right" if modifiers.alt => Some(Motion::WordRight),
            "left" => Some(Motion::Left),
            "right" => Some(Motion::Right),
            "up" => Some(Motion::Up),
            "down" => Some(Motion::Down),
            "home" => Some(Motion::LineStart),
            "end" => Some(Motion::LineEnd),
            _ => None,
        };
        if let Some(motion) = motion {
            self.edit_typing(cx, |typing, deck| {
                typing.move_caret(deck, motion, shift);
                Ok(false)
            });
            return true;
        }
        if modifiers.platform || modifiers.control {
            return false;
        }
        match key {
            // As in PowerPoint, Escape leaves the text, the shape still
            // selected.
            "escape" => self.commit_text(cx),
            "enter" if shift => self.edit_typing(cx, |typing, deck| typing.line_break(deck)),
            "enter" => self.edit_typing(cx, |typing, deck| typing.type_text(deck, "\n")),
            "tab" => self.edit_typing(cx, |typing, deck| typing.type_text(deck, "\t")),
            "backspace" => self.edit_typing(cx, |typing, deck| typing.backspace(deck)),
            "delete" => self.edit_typing(cx, |typing, deck| typing.delete(deck)),
            _ => return false,
        }
        true
    }

    /// Starts typing into `target` from the text it has — all of it
    /// selected, when `whole`.
    fn edit(&mut self, target: TextTarget, whole: bool, cx: &mut Context<Self>) {
        let (Some(text), Some(rect)) = (
            self.deck.text(self.current, target),
            self.deck.text_rect(self.current, target),
        ) else {
            self.status = SharedString::from(i18n::text("deck.text_locked"));
            cx.notify();
            return;
        };
        self.drag = None;
        self.text_edit = Some(TextEdit {
            target,
            rect,
            text,
            marked: None,
            whole,
        });
        cx.notify();
    }

    /// Writes the text being typed, if any, as one undoable change. Text
    /// typed in place is in the deck already; it just stops being typed
    /// into, an IME's composition kept.
    fn commit_text(&mut self, cx: &mut Context<Self>) {
        self.commit_notes(cx);
        self.selecting_text = false;
        if let Some(mut typing) = self.typing.take() {
            if let Ok(true) = typing.unmark(&mut self.deck) {
                self.reload(false, cx);
            }
            self.deck.end_typing();
            cx.notify();
        }
        let Some(edit) = self.text_edit.take() else {
            return;
        };
        let (target, text) = (edit.target, edit.text);
        self.write(cx, |deck, slide| deck.set_text(slide, target, &text));
        cx.notify();
    }

    // ---- speaker notes -----------------------------------------------------

    /// A click in the notes pane: typing into the slide's notes, the caret
    /// at their end.
    fn begin_notes(&mut self, cx: &mut Context<Self>) {
        if self.notes_edit.is_some() {
            return;
        }
        self.commit_text(cx);
        self.select(None, cx);
        match self.deck.notes_text(self.current) {
            Some(text) => self.notes_edit = Some(NotesEdit::new(text)),
            None => self.status = SharedString::from(i18n::text("deck.text_locked")),
        }
        cx.notify();
    }

    /// Writes the notes being typed, as one step of Undo.
    fn commit_notes(&mut self, cx: &mut Context<Self>) {
        let Some(edit) = self.notes_edit.take() else {
            return;
        };
        let text = edit.text;
        self.write(cx, |deck, slide| deck.set_notes(slide, &text));
        cx.notify();
    }

    /// The keys of the notes being typed: Escape leaves them, Enter starts
    /// a paragraph; the caret keys and deletion.
    fn notes_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let Some(edit) = self.notes_edit.as_mut() else {
            return false;
        };
        if edit.marked.is_some() {
            return false;
        }
        match key {
            "escape" => {
                self.commit_notes(cx);
                return true;
            }
            "enter" => edit.insert("\n"),
            "backspace" => edit.backspace(),
            "delete" => edit.delete(),
            "left" => edit.step(false),
            "right" => edit.step(true),
            "home" | "up" => edit.to_end(false),
            "end" | "down" => edit.to_end(true),
            _ => return false,
        }
        cx.notify();
        true
    }

    /// Tab in a table: the text written, then the next cell (⇧Tab: the one
    /// before). Tab in the last cell adds a row and goes to its first, as
    /// in PowerPoint.
    fn tab_in_table(&mut self, backwards: bool, cx: &mut Context<Self>) {
        let Some(from @ TextTarget::Cell { id, row, .. }) =
            self.text_edit.as_ref().map(|edit| edit.target)
        else {
            return;
        };
        self.commit_text(cx);
        match self.deck.next_cell(self.current, from, backwards) {
            // The next cell's text selected, as PowerPoint's Tab does.
            Some(next) => self.edit(next, true, cx),
            None if !backwards => {
                let added = self.write(cx, |deck, slide| {
                    deck.change_table(slide, id, TableChange::InsertRow(row + 1))
                });
                if !added {
                    return;
                }
                self.edit(
                    TextTarget::Cell {
                        id,
                        row: row + 1,
                        column: 0,
                    },
                    false,
                    cx,
                );
            }
            None => self.edit(from, true, cx),
        }
    }

    /// A row or column added or removed beside the cell being typed into,
    /// its text written first.
    fn change_table(
        &mut self,
        change: impl FnOnce(usize, usize) -> TableChange,
        cx: &mut Context<Self>,
    ) {
        let Some(TextTarget::Cell { id, row, column }) =
            self.text_edit.as_ref().map(|edit| edit.target)
        else {
            return;
        };
        self.commit_text(cx);
        let change = change(row, column);
        self.write(cx, |deck, slide| deck.change_table(slide, id, change));
    }

    /// Extends a cell selection — from the cell being typed into, or the
    /// selection's first — to the cell at (`x`, `y`) of the same table.
    fn select_cells_to(&mut self, x: f64, y: f64, cx: &mut Context<Self>) -> bool {
        let anchor = match (&self.cells, self.text_edit.as_ref().map(|edit| edit.target)) {
            (Some(cells), _) => (cells.id, cells.from),
            (None, Some(TextTarget::Cell { id, row, column })) => (id, (row, column)),
            _ => return false,
        };
        let (id, from) = anchor;
        let TextTarget::Cell { row, column, .. } =
            self.deck.text_target(self.current, id, Some((x, y)))
        else {
            return false;
        };
        let Some(rect) = self
            .deck
            .cell_range_rect(self.current, id, from, (row, column))
        else {
            return false;
        };
        self.commit_text(cx);
        self.cells = Some(CellSelection {
            id,
            from,
            to: (row, column),
            rect,
        });
        cx.notify();
        true
    }

    /// Merge Cells: the selected cells become one, typed into next.
    fn merge_cells(&mut self, cx: &mut Context<Self>) {
        let Some(cells) = self.cells.take() else {
            return;
        };
        let change = jade_office_model::deck::merge_cells(cells.from, cells.to);
        let merged = self.write(cx, |deck, slide| deck.change_table(slide, cells.id, change));
        if let (true, TableChange::MergeCells { row, column, .. }) = (merged, change) {
            self.edit(
                TextTarget::Cell {
                    id: cells.id,
                    row,
                    column,
                },
                false,
                cx,
            );
        }
        cx.notify();
    }

    /// Split Cells: the merged cell being typed into becomes its cells.
    fn split_cell(&mut self, cx: &mut Context<Self>) {
        let Some(target @ TextTarget::Cell { id, row, column }) =
            self.text_edit.as_ref().map(|edit| edit.target)
        else {
            return;
        };
        self.commit_text(cx);
        if self.write(cx, |deck, slide| {
            deck.change_table(slide, id, TableChange::SplitCell { row, column })
        }) {
            self.edit(target, false, cx);
        }
    }

    // ---- keys ------------------------------------------------------------

    fn handle_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let modifiers = keystroke.modifiers;
        let key = keystroke.key.as_str();
        if self.notes_edit.is_some() {
            if !modifiers.platform && self.notes_key(key, cx) {
                cx.stop_propagation();
            }
            return;
        }
        // PowerPoint's formatting keys: ⌘B, ⌘I, ⌘U, ⌘L, ⌘E, ⌘R, ⌘J, and ⌘⇧>
        // and ⌘⇧< for the font size — not bound as actions, which would take
        // them from the Word view.
        if modifiers.platform && self.format_shortcut(key, modifiers.shift, cx) {
            cx.stop_propagation();
            return;
        }
        if self.typing.is_some() {
            // An IME composing has the keys to itself.
            let composing = self
                .typing
                .as_ref()
                .is_some_and(|typing| typing.composition.is_some());
            if !composing && self.typing_key(key, modifiers, cx) {
                cx.stop_propagation();
            }
            return;
        }
        if modifiers.platform && self.text_edit.is_none() {
            // PowerPoint for Mac's New Slide (⌘⇧N) and Duplicate (⌘D).
            match (key, modifiers.shift) {
                ("n", true) => self.new_slide(cx),
                ("d", false) if self.selected.is_none() => self.duplicate_slide(cx),
                _ => return,
            }
            cx.stop_propagation();
            return;
        }
        if modifiers.platform || modifiers.control {
            return;
        }

        if self.cells.is_some() && key == "escape" {
            self.cells = None;
            cx.notify();
            cx.stop_propagation();
            return;
        }

        if let Some(edit) = self.text_edit.as_mut() {
            if edit.marked.is_some() {
                return;
            }
            match key {
                // As in PowerPoint, Escape leaves the text keeping what was
                // typed, with the shape still selected.
                "escape" => self.commit_text(cx),
                "tab" if matches!(edit.target, TextTarget::Cell { .. }) => {
                    self.tab_in_table(modifiers.shift, cx)
                }
                "enter" => {
                    edit.text.push('\n');
                    cx.notify();
                }
                "backspace" => {
                    if edit.whole {
                        edit.take_whole(true);
                    } else {
                        edit.text.pop();
                    }
                    cx.notify();
                }
                _ => return,
            }
            cx.stop_propagation();
            return;
        }

        if let Some(id) = self.selected {
            let step = if modifiers.alt { 1.0 } else { NUDGE_PX };
            match key {
                "left" => self.nudge(-step, 0.0, cx),
                "right" => self.nudge(step, 0.0, cx),
                "up" => self.nudge(0.0, -step, cx),
                "down" => self.nudge(0.0, step, cx),
                "delete" | "backspace" => self.delete_selected(cx),
                "enter" | "f2" => self.begin_text(id, None, cx),
                "escape" => self.select(None, cx),
                "tab" => self.cycle_selection(modifiers.shift, cx),
                // Text arrives through the input handler, which starts an
                // edit — so an IME composes into the shape.
                _ => return,
            }
            cx.stop_propagation();
            return;
        }

        match key {
            // In the slide list, Delete deletes the slide.
            "delete" | "backspace" if self.slides_focused => self.delete_slide(cx),
            "escape" if self.inserting.is_some() => {
                self.inserting = None;
                self.drawing = None;
                cx.notify();
            }
            "escape" if self.popup.is_some() => {
                self.popup = None;
                cx.notify();
            }
            "down" | "right" | "pagedown" | "space" => self.step(1, cx),
            "up" | "left" | "pageup" => self.step(-1, cx),
            "home" => self.go_to(0, cx),
            "end" => self.go_to(usize::MAX, cx),
            "tab" => self.cycle_selection(modifiers.shift, cx),
            _ => return,
        }
        cx.stop_propagation();
    }

    // ---- saving ----------------------------------------------------------

    /// Saves over the file that was opened, as PowerPoint does.
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
            .unwrap_or_else(|| "deck.pptx".to_string());
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
        self.commit_text(cx);
        self.status = SharedString::from(
            match crate::shell::write_in_place(&target, self.deck.bytes()) {
                Ok(()) => {
                    self.deck.save_bytes();
                    i18n::format("deck.saved", &[("name", &crate::shell::file_name(&target))])
                }
                Err(error) => i18n::format("deck.save_failed", &[("error", &error)]),
            },
        );
        cx.notify();
    }

    // ---- painting --------------------------------------------------------

    fn paint_thumbnails_if_needed(&mut self, scale_factor: f32) {
        let device_width = (THUMBNAIL_WIDTH * scale_factor).round() as i32;
        for index in 0..self.thumbnails.len() {
            if self.thumbnails[index].is_none() {
                self.thumbnails[index] = slide_image(&mut self.deck, index, device_width);
            }
        }
    }

    /// One slide in the list: its number — struck through with an eye for a
    /// hidden slide, which is also dimmed, as PowerPoint marks it — and its
    /// picture. A click shows it; a drag onto another moves it there.
    fn thumbnail(&self, index: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = index == self.current;
        let hidden = self.deck.is_hidden(index);
        let (slide_width, slide_height) = self.deck.slide_size();
        let height = THUMBNAIL_WIDTH * (slide_height / slide_width.max(1.0)) as f32;
        let number_color = if selected {
            theme::accent()
        } else {
            theme::text_faint()
        };
        div()
            .id(("slide-thumbnail", index))
            .flex()
            .gap_2()
            .px_2()
            .py_1()
            .cursor_pointer()
            .on_click(cx.listener(move |view, _event, window, cx| {
                window.focus(&view.focus);
                view.slides_focused = true;
                view.go_to(index, cx);
                cx.notify();
            }))
            .on_drag(DraggedSlide(index), |dragged, _offset, _window, cx| {
                let index = dragged.0;
                cx.new(|_| DraggedSlideView(index))
            })
            .drag_over::<DraggedSlide>(|style, _, _, _| style.bg(theme::hovered()))
            .on_drop(
                cx.listener(move |view, dragged: &DraggedSlide, _window, cx| {
                    view.move_slide(dragged.0, index, cx)
                }),
            )
            .child(
                div()
                    .w(px(16.0))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .text_xs()
                    .text_color(number_color)
                    .child(
                        div()
                            .when(hidden, |number| number.line_through())
                            .child(format!("{}", index + 1)),
                    )
                    .when(hidden, |column| {
                        column.child(
                            svg()
                                .path("icons/eye-off.svg")
                                .size_3()
                                .text_color(number_color),
                        )
                    }),
            )
            .child(
                div()
                    .w(px(THUMBNAIL_WIDTH))
                    .h(px(height))
                    .flex_none()
                    .bg(gpui::white())
                    .border_2()
                    .rounded_sm()
                    .overflow_hidden()
                    .when(hidden, |thumbnail| thumbnail.opacity(0.45))
                    .border_color(if selected {
                        theme::accent()
                    } else {
                        theme::border()
                    })
                    .hover(|thumbnail| thumbnail.border_color(theme::text_faint()))
                    .children(
                        self.thumbnails
                            .get(index)
                            .cloned()
                            .flatten()
                            .map(|image| img(image).size_full()),
                    ),
            )
    }

    /// The selection outline and its corner handles, or the text editor,
    /// over the slide.
    fn overlays(
        &self,
        scale: f32,
        typed: Option<&TextSelectionGeometry>,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        let mut out = Vec::new();
        // A text box or shape being drawn.
        if let (Some((from, to)), Some(insert)) = (self.drawing, self.inserting) {
            let rect = insert.rect(from, to);
            out.push(
                div()
                    .absolute()
                    .left(px(rect.x as f32 * scale))
                    .top(px(rect.y as f32 * scale))
                    .w(px(rect.width as f32 * scale))
                    .h(px(rect.height as f32 * scale))
                    .border_1()
                    .border_color(theme::accent())
                    .into_any_element(),
            );
        }
        // The selected cells, shaded as PowerPoint shades them.
        if let Some(cells) = &self.cells {
            let rect = cells.rect;
            out.push(
                div()
                    .absolute()
                    .left(px(rect.x as f32 * scale))
                    .top(px(rect.y as f32 * scale))
                    .w(px(rect.width as f32 * scale))
                    .h(px(rect.height as f32 * scale))
                    .bg(gpui::rgba(0x3b82f633))
                    .border_1()
                    .border_color(theme::accent())
                    .into_any_element(),
            );
        }
        let Some(id) = self.selected else {
            return out;
        };
        let Some(shape) = self.shape(id) else {
            return out;
        };
        let Some(transform) = shape.transform else {
            return out;
        };
        let mut rect = SlideRect::of(&transform);
        if let Some(drag) = self.drag.as_ref().filter(|drag| drag.id == id) {
            rect = drag.rect(rect);
        }
        let (left, top, width, height) = (
            rect.x as f32 * scale,
            rect.y as f32 * scale,
            rect.width as f32 * scale,
            rect.height as f32 * scale,
        );

        // Text typed in place: the shape's outline, the selection shaded
        // over the text, and the caret — or an IME's composition there.
        if let Some(typing) = self.typing.as_ref().filter(|typing| typing.id == id) {
            out.push(
                div()
                    .absolute()
                    .left(px(left))
                    .top(px(top))
                    .w(px(width))
                    .h(px(height))
                    .border_1()
                    .border_color(theme::accent())
                    .into_any_element(),
            );
            let Some(geometry) = typed else {
                return out;
            };
            for quad in &geometry.highlights {
                let (x0, y0, x1, y1) = quad.iter().fold(
                    (f64::MAX, f64::MAX, f64::MIN, f64::MIN),
                    |(x0, y0, x1, y1), p| (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)),
                );
                out.push(
                    div()
                        .absolute()
                        .left(px(x0 as f32 * scale))
                        .top(px(y0 as f32 * scale))
                        .w(px((x1 - x0) as f32 * scale))
                        .h(px((y1 - y0) as f32 * scale))
                        .bg(gpui::rgba(0x3b82f655))
                        .into_any_element(),
                );
            }
            let [a, b] = geometry.caret;
            let (x, y, line) = (a.x.min(b.x), a.y.min(b.y), (a.y - b.y).abs());
            let caret = div()
                .absolute()
                .left(px(x as f32 * scale))
                .top(px(y as f32 * scale))
                .h(px(line as f32 * scale));
            out.push(match &typing.composition {
                Some(composition) => caret
                    .flex()
                    .items_center()
                    .px_0p5()
                    .bg(gpui::white())
                    .text_color(gpui::black())
                    .text_size(px((line as f32 * scale * 0.7).max(10.0)))
                    .child(div().underline().child(composition.clone()))
                    .into_any_element(),
                None => caret.w(px(1.5)).bg(gpui::black()).into_any_element(),
            });
            return out;
        }

        if let Some(edit) = self
            .text_edit
            .as_ref()
            .filter(|edit| edit.target.id() == id)
        {
            let (left, top, width, height) = (
                edit.rect.x as f32 * scale,
                edit.rect.y as f32 * scale,
                edit.rect.width as f32 * scale,
                edit.rect.height as f32 * scale,
            );
            let (before, marked, after) = match &edit.marked {
                Some(range) => (
                    edit.text[..range.start].to_string(),
                    edit.text[range.clone()].to_string(),
                    edit.text[range.end..].to_string(),
                ),
                None => (edit.text.clone(), String::new(), String::new()),
            };
            let whole = edit.whole;
            out.push(
                div()
                    .absolute()
                    .left(px(left))
                    .top(px(top))
                    .min_w(px(width))
                    .min_h(px(height))
                    .p(px(6.0))
                    .bg(gpui::rgba(0xffffffee))
                    .border_1()
                    .border_color(theme::accent())
                    .text_color(gpui::black())
                    .text_size(px((18.0 * scale).max(11.0)))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_end()
                            .child(if whole {
                                div().bg(theme::selected()).child(before)
                            } else {
                                div().child(before)
                            })
                            .when(!marked.is_empty(), |line| {
                                line.child(div().underline().child(marked))
                            })
                            .child(
                                div()
                                    .w(px(1.5))
                                    .h(px((20.0 * scale).max(12.0)))
                                    .bg(gpui::black()),
                            )
                            .child(after),
                    )
                    .into_any_element(),
            );
            return out;
        }

        out.push(
            div()
                .absolute()
                .left(px(left))
                .top(px(top))
                .w(px(width))
                .h(px(height))
                .border_1()
                .border_color(theme::accent())
                .into_any_element(),
        );
        if shape.can_resize {
            for (index, (is_left, is_top)) in
                [(true, true), (false, true), (true, false), (false, false)]
                    .into_iter()
                    .enumerate()
            {
                let x = if is_left { left } else { left + width } - HANDLE_SIZE / 2.0;
                let y = if is_top { top } else { top + height } - HANDLE_SIZE / 2.0;
                out.push(
                    div()
                        .id(("resize-handle", index))
                        .absolute()
                        .left(px(x))
                        .top(px(y))
                        .size(px(HANDLE_SIZE))
                        .rounded_full()
                        .bg(gpui::white())
                        .border_1()
                        .border_color(theme::accent())
                        .cursor(if is_left == is_top {
                            gpui::CursorStyle::ResizeUpLeftDownRight
                        } else {
                            gpui::CursorStyle::ResizeUpRightDownLeft
                        })
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |view, event: &MouseDownEvent, _window, cx| {
                                view.start_resize(is_left, is_top, event, cx)
                            }),
                        )
                        .into_any_element(),
                );
            }
        }
        out
    }
}

impl DeckView {
    /// Shapes ▾: the shapes Insert offers; one picked is drawn next.
    fn shapes_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let open = self.popup == Some(Popup::Shapes);
        let drawing_shape = matches!(self.inserting, Some(Insert::Shape(_)));
        let menu = open.then(|| {
            menu_panel("deck-shapes")
                .children(SHAPE_PRESETS.iter().enumerate().map(|(index, &preset)| {
                    menu_row(
                        ("deck-shape", index),
                        i18n::text(shape_name(preset)),
                        self.inserting == Some(Insert::Shape(preset)),
                    )
                    .on_click(cx.listener(move |view, _event, _window, cx| {
                        view.start_insert(Insert::Shape(preset), cx)
                    }))
                }))
                .into_any_element()
        });
        with_menu(
            toolbar_button(
                "insert-shape",
                "icons/shapes.svg",
                i18n::text("deck.shapes"),
            )
            .when(open || drawing_shape, |button| button.bg(theme::selected()))
            .on_click(cx.listener(|view, _event, _window, cx| view.toggle_popup(Popup::Shapes, cx)))
            .into_any_element(),
            menu,
        )
    }

    /// Layout ▾: the layouts the slide can take, its own checked.
    fn layouts_button(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let open = self.popup == Some(Popup::Layouts);
        let menu = open.then(|| {
            let rows: Vec<_> =
                self.deck
                    .slide_layouts(self.current)
                    .into_iter()
                    .enumerate()
                    .map(|(index, layout)| {
                        let part = layout.part_name.clone();
                        div()
                            .id(("slide-layout", index))
                            .flex()
                            .items_center()
                            .gap_2()
                            .h(px(26.0))
                            .pl_1()
                            .pr_3()
                            .rounded_md()
                            .whitespace_nowrap()
                            .cursor_pointer()
                            .hover(|row| row.bg(theme::hovered()))
                            .on_click(cx.listener(move |view, _event, _window, cx| {
                                view.set_layout(&part, cx)
                            }))
                            .child(div().size_4().flex_none().when(layout.current, |cell| {
                                cell.child(
                                    svg()
                                        .path("icons/check.svg")
                                        .size_4()
                                        .text_color(theme::accent()),
                                )
                            }))
                            .child(if layout.name.is_empty() {
                                layout.kind
                            } else {
                                layout.name
                            })
                    })
                    .collect();
            div()
                .id("slide-layouts")
                .occlude()
                .mt_1()
                .p_1()
                .flex()
                .flex_col()
                .max_h(px(360.0))
                .overflow_y_scroll()
                .rounded_md()
                .bg(theme::panel())
                .border_1()
                .border_color(theme::border())
                .shadow_lg()
                .text_xs()
                .text_color(theme::text())
                .children(rows)
        });
        div()
            .relative()
            .flex_none()
            .child(
                toolbar_button(
                    "slide-layout",
                    "icons/panels-top-left.svg",
                    i18n::text("deck.layout"),
                )
                .when(open, |button| button.bg(theme::hovered()))
                .on_click(
                    cx.listener(|view, _event, _window, cx| view.toggle_popup(Popup::Layouts, cx)),
                ),
            )
            .children(menu.map(|menu| {
                div().absolute().top_full().left_0().child(
                    deferred(anchored().snap_to_window_with_margin(px(8.0)).child(menu))
                        .with_priority(2),
                )
            }))
            .into_any_element()
    }
}

/// A click listener, as the format strip hands them round.
type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// The font list the format strip offers, before the text's own.
const FONT_FAMILIES: [&str; 9] = [
    "游ゴシック",
    "メイリオ",
    "Hiragino Sans",
    "Noto Sans JP",
    "游明朝",
    "Calibri",
    "Arial",
    "Georgia",
    "Times New Roman",
];
/// PowerPoint's standard colours, under its font colour button.
const FONT_COLORS: [u32; 12] = [
    0x000000, 0x7f7f7f, 0xc00000, 0xff0000, 0xffc000, 0xffff00, 0x92d050, 0x00b050, 0x00b0f0,
    0x0070c0, 0x002060, 0x7030a0,
];
/// Line spacing (multiples of single) and space after (points), PowerPoint's
/// Line Spacing list.
const LINE_SPACINGS: [f64; 4] = [1.0, 1.15, 1.5, 2.0];
const SPACES_AFTER: [f64; 3] = [0.0, 6.0, 12.0];

/// A format strip button: an icon, shaded while what it sets is on.
fn format_button(
    id: &'static str,
    icon: &'static str,
    tip: &'static str,
    on: bool,
) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(26.0))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .rounded_md()
        .cursor_pointer()
        .when(on, |button| button.bg(theme::selected()))
        .hover(|button| button.bg(theme::hovered()))
        .tooltip(Tooltip::text(i18n::text(tip)))
        .child(svg().path(icon).size_4().text_color(theme::text_dim()))
}

/// A list trigger: its value and a chevron.
fn format_list(id: &'static str, width: f32, value: String, open: bool) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_between()
        .w(px(width))
        .h(px(24.0))
        .pl_2()
        .pr_1()
        .rounded_md()
        .border_1()
        .border_color(if open {
            theme::accent()
        } else {
            theme::border()
        })
        .cursor_pointer()
        .hover(|button| button.bg(theme::hovered()))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_xs()
                .child(value),
        )
        .child(
            svg()
                .path("icons/chevron-down.svg")
                .size_3()
                .text_color(theme::text_dim()),
        )
}

/// A trigger with its list under it while open.
fn with_menu(trigger: AnyElement, menu: Option<AnyElement>) -> AnyElement {
    div()
        .relative()
        .flex_none()
        .child(trigger)
        .children(menu.map(|menu| {
            div().absolute().top_full().left_0().child(
                deferred(anchored().snap_to_window_with_margin(px(8.0)).child(menu))
                    .with_priority(2),
            )
        }))
        .into_any_element()
}

/// The panel a list is drawn on.
fn menu_panel(id: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .occlude()
        .mt_1()
        .p_1()
        .flex()
        .flex_col()
        .max_h(px(360.0))
        .overflow_y_scroll()
        .rounded_md()
        .bg(theme::panel())
        .border_1()
        .border_color(theme::border())
        .shadow_lg()
        .text_xs()
        .text_color(theme::text())
}

/// One row of a list, checked when it is the current value.
fn menu_row(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    checked: bool,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_2()
        .h(px(26.0))
        .pl_1()
        .pr_3()
        .rounded_md()
        .whitespace_nowrap()
        .cursor_pointer()
        .hover(|row| row.bg(theme::hovered()))
        .child(div().size_4().flex_none().when(checked, |cell| {
            cell.child(
                svg()
                    .path("icons/check.svg")
                    .size_4()
                    .text_color(theme::accent()),
            )
        }))
        .child(label.into())
}

fn size_label(size: f32) -> String {
    if size.fract() == 0.0 {
        format!("{size:.0}")
    } else {
        format!("{size}")
    }
}

impl DeckView {
    fn on_format(command: TextCommand, cx: &mut Context<Self>) -> ClickHandler {
        Box::new(cx.listener(move |view, _event: &ClickEvent, _window, cx| {
            view.format_slide_text(command.clone(), cx)
        }))
    }

    fn on_popup(popup: Popup, cx: &mut Context<Self>) -> ClickHandler {
        Box::new(
            cx.listener(move |view, _event: &ClickEvent, _window, cx| view.toggle_popup(popup, cx)),
        )
    }

    /// The format strip, PowerPoint's Font and Paragraph groups, over the
    /// text typed into or all of the selected shape's.
    fn format_bar(&self, strip: &FormatStrip, open: Option<Popup>, cx: &mut Context<Self>) -> Div {
        let font = strip.font.clone().unwrap_or_default();
        let font_menu = (open == Some(Popup::Font)).then(|| {
            let mut fonts: Vec<String> = FONT_FAMILIES.iter().map(|f| f.to_string()).collect();
            if !font.is_empty() && !fonts.contains(&font) {
                fonts.insert(0, font.clone());
            }
            menu_panel("deck-fonts")
                .children(fonts.into_iter().enumerate().map(|(index, name)| {
                    menu_row(("deck-font", index), name.clone(), name == font)
                        .font_family(SharedString::from(name.clone()))
                        .on_click(Self::on_format(TextCommand::Font(name), cx))
                }))
                .into_any_element()
        });
        let size_menu = (open == Some(Popup::Size)).then(|| {
            menu_panel("deck-sizes")
                .children(FONT_SIZES.iter().enumerate().map(|(index, &size)| {
                    menu_row(
                        ("deck-size", index),
                        size_label(size),
                        strip.size == Some(size),
                    )
                    .on_click(Self::on_format(TextCommand::Size(size), cx))
                }))
                .into_any_element()
        });
        let color_menu = (open == Some(Popup::Color)).then(|| {
            menu_panel("deck-colors")
                .child(div().w(px(6.0 * 24.0)).flex().flex_wrap().gap_1().children(
                    FONT_COLORS.iter().enumerate().map(|(index, &color)| {
                        div()
                            .id(("deck-color", index))
                            .size(px(20.0))
                            .rounded_sm()
                            .bg(rgb(color))
                            .border_1()
                            .border_color(theme::border())
                            .cursor_pointer()
                            .hover(|square| square.border_color(theme::accent()))
                            .on_click(Self::on_format(
                                TextCommand::Color(format!("#{color:06X}")),
                                cx,
                            ))
                    }),
                ))
                .into_any_element()
        });
        let spacing_menu = (open == Some(Popup::Spacing)).then(|| {
            menu_panel("deck-spacing")
                .children(LINE_SPACINGS.iter().enumerate().map(|(index, &lines)| {
                    menu_row(
                        ("deck-line-spacing", index),
                        format!("{lines:.2}")
                            .trim_end_matches('0')
                            .trim_end_matches('.')
                            .to_string(),
                        false,
                    )
                    .on_click(Self::on_format(TextCommand::LineSpacing(lines), cx))
                }))
                .children(SPACES_AFTER.iter().enumerate().map(|(index, &points)| {
                    menu_row(
                        ("deck-space-after", index),
                        i18n::format("deck.space_after", &[("n", &points)]),
                        false,
                    )
                    .on_click(Self::on_format(TextCommand::SpaceAfter(points), cx))
                }))
                .into_any_element()
        });
        let separator = || div().w(px(1.0)).h(px(18.0)).mx_1().bg(theme::border());
        let alignment = strip.alignment;
        div()
            .h(px(FORMAT_BAR_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .gap_0p5()
            .px_3()
            .bg(theme::panel())
            .border_b_1()
            .border_color(theme::border())
            .text_color(theme::text())
            .child(with_menu(
                format_list("deck-font", 150.0, font, open == Some(Popup::Font))
                    .tooltip(Tooltip::text(i18n::text("deck.font")))
                    .on_click(Self::on_popup(Popup::Font, cx))
                    .into_any_element(),
                font_menu,
            ))
            .child(with_menu(
                format_list(
                    "deck-size",
                    56.0,
                    strip.size.map(size_label).unwrap_or_default(),
                    open == Some(Popup::Size),
                )
                .tooltip(Tooltip::text(i18n::text("deck.font_size")))
                .on_click(Self::on_popup(Popup::Size, cx))
                .into_any_element(),
                size_menu,
            ))
            .child(
                format_button("deck-grow", "icons/a-arrow-up.svg", "deck.grow_font", false)
                    .on_click(Self::on_format(TextCommand::Grow, cx)),
            )
            .child(
                format_button(
                    "deck-shrink",
                    "icons/a-arrow-down.svg",
                    "deck.shrink_font",
                    false,
                )
                .on_click(Self::on_format(TextCommand::Shrink, cx)),
            )
            .child(separator())
            .child(
                format_button("deck-bold", "icons/bold.svg", "deck.bold", strip.bold)
                    .on_click(Self::on_format(TextCommand::Bold, cx)),
            )
            .child(
                format_button(
                    "deck-italic",
                    "icons/italic.svg",
                    "deck.italic",
                    strip.italic,
                )
                .on_click(Self::on_format(TextCommand::Italic, cx)),
            )
            .child(
                format_button(
                    "deck-underline",
                    "icons/underline.svg",
                    "deck.underline",
                    strip.underline,
                )
                .on_click(Self::on_format(TextCommand::Underline, cx)),
            )
            .child(with_menu(
                format_button(
                    "deck-color",
                    "icons/baseline.svg",
                    "deck.font_color",
                    open == Some(Popup::Color),
                )
                .on_click(Self::on_popup(Popup::Color, cx))
                .into_any_element(),
                color_menu,
            ))
            .child(separator())
            .children(
                [
                    (
                        ParagraphAlignment::Left,
                        "deck-align-left",
                        "icons/align-left.svg",
                        "deck.align_left",
                    ),
                    (
                        ParagraphAlignment::Center,
                        "deck-align-center",
                        "icons/align-center.svg",
                        "deck.align_center",
                    ),
                    (
                        ParagraphAlignment::Right,
                        "deck-align-right",
                        "icons/align-right.svg",
                        "deck.align_right",
                    ),
                    (
                        ParagraphAlignment::Justify,
                        "deck-justify",
                        "icons/align-justify.svg",
                        "deck.justify",
                    ),
                ]
                .into_iter()
                .map(|(value, id, icon, tip)| {
                    format_button(id, icon, tip, alignment == Some(value))
                        .on_click(Self::on_format(TextCommand::Align(value), cx))
                }),
            )
            .child(separator())
            .child(
                format_button(
                    "deck-bullets",
                    "icons/list.svg",
                    "deck.bullets",
                    strip.bullets,
                )
                .on_click(Self::on_format(TextCommand::Bullets, cx)),
            )
            .child(
                format_button(
                    "deck-numbering",
                    "icons/list-ordered.svg",
                    "deck.numbering",
                    strip.numbering,
                )
                .on_click(Self::on_format(TextCommand::Numbering, cx)),
            )
            .child(with_menu(
                format_button(
                    "deck-line-spacing",
                    "icons/chevrons-up-down.svg",
                    "deck.line_spacing",
                    open == Some(Popup::Spacing),
                )
                .on_click(Self::on_popup(Popup::Spacing, cx))
                .into_any_element(),
                spacing_menu,
            ))
    }
}

/// The notes being typed, a row a line, the caret drawn in its line.
fn notes_lines(edit: &NotesEdit) -> Vec<gpui::AnyElement> {
    let caret = edit.caret();
    let mut start = 0;
    let mut rows = Vec::new();
    for line in edit.text.split('\n') {
        let end = start + line.len();
        let row = div().flex().min_h(px(18.0));
        rows.push(if (start..=end).contains(&caret) {
            let at = caret - start;
            row.child(line[..at].to_string())
                .child(div().w(px(1.5)).h(px(16.0)).bg(theme::text()))
                .child(line[at..].to_string())
                .into_any_element()
        } else {
            row.child(line.to_string()).into_any_element()
        });
        start = end + 1;
    }
    rows
}

/// The words for a shape in Insert's list, as PowerPoint names them.
fn shape_name(preset: jade_office_model::deck::ShapePreset) -> &'static str {
    use jade_office_model::deck::ShapePreset;
    match preset {
        ShapePreset::Rect => "deck.shape.rect",
        ShapePreset::RoundRect => "deck.shape.round_rect",
        ShapePreset::Ellipse => "deck.shape.ellipse",
        ShapePreset::Triangle => "deck.shape.triangle",
        ShapePreset::Diamond => "deck.shape.diamond",
        ShapePreset::Hexagon => "deck.shape.hexagon",
    }
}

fn toolbar_button(
    id: &'static str,
    icon: &'static str,
    label: &'static str,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_1p5()
        .px_2p5()
        .py_1()
        .rounded_md()
        .text_xs()
        .text_color(theme::text_dim())
        .cursor_pointer()
        .hover(|button| button.bg(theme::hovered()).text_color(theme::text()))
        .child(svg().path(icon).size_3p5().text_color(theme::text_dim()))
        .child(label)
}

impl Editor for DeckView {
    fn status_items(&self) -> Vec<SharedString> {
        let mut items: Vec<SharedString> = vec![i18n::format(
            "deck.position",
            &[
                ("current", &(self.current + 1)),
                ("count", &self.deck.slide_count()),
            ],
        )
        .into()];
        if let Some(shape) = self.selected.and_then(|id| self.shape(id)) {
            items.push(i18n::format("deck.selected_shape", &[("name", &shape.name)]).into());
        }
        if !self.status.is_empty() {
            items.push(self.status.clone());
        }
        items
    }

    fn zoom(&self) -> f32 {
        self.zoom.unwrap_or(self.shown_scale)
    }

    fn set_zoom(&mut self, zoom: f32, cx: &mut Context<Self>) {
        self.zoom = Some(editor::clamp_zoom(zoom));
        cx.notify();
    }

    fn zoom_to_fit(&mut self, cx: &mut Context<Self>) {
        self.zoom = None;
        cx.notify();
    }

    fn is_dirty(&self) -> bool {
        self.deck.is_dirty()
    }
}

impl EntityInputHandler for DeckView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        if let Some(edit) = &self.notes_edit {
            let text = edit.text.as_str();
            let start = utf16_to_byte(text, range.start);
            let end = utf16_to_byte(text, range.end).max(start);
            *adjusted = Some(byte_to_utf16(text, start)..byte_to_utf16(text, end));
            return Some(text.get(start..end)?.to_string());
        }
        if let Some(typing) = &self.typing {
            let (text, _, _) = typing.input_text(&self.deck);
            let start = utf16_to_byte(&text, range.start);
            let end = utf16_to_byte(&text, range.end).max(start);
            *adjusted = Some(byte_to_utf16(&text, start)..byte_to_utf16(&text, end));
            return Some(text.get(start..end)?.to_string());
        }
        let text = self
            .text_edit
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
        if let Some(edit) = &self.notes_edit {
            let caret = byte_to_utf16(&edit.text, edit.caret());
            return Some(UTF16Selection {
                range: caret..caret,
                reversed: false,
            });
        }
        if let Some(typing) = &self.typing {
            let (_, range, marked) = typing.input_text(&self.deck);
            return Some(UTF16Selection {
                range,
                reversed: marked.is_none() && typing.focus < typing.anchor,
            });
        }
        let text = self
            .text_edit
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
        if let Some(edit) = &self.notes_edit {
            let marked = edit.marked.clone()?;
            return Some(
                byte_to_utf16(&edit.text, marked.start)..byte_to_utf16(&edit.text, marked.end),
            );
        }
        if let Some(typing) = &self.typing {
            return typing.input_text(&self.deck).2;
        }
        let edit = self.text_edit.as_ref()?;
        let marked = edit.marked.clone()?;
        Some(byte_to_utf16(&edit.text, marked.start)..byte_to_utf16(&edit.text, marked.end))
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(edit) = self.notes_edit.as_mut() {
            edit.marked = None;
            return;
        }
        self.edit_typing(cx, |typing, deck| typing.unmark(deck));
        if let Some(edit) = self.text_edit.as_mut() {
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
        if let Some(edit) = self.notes_edit.as_mut() {
            edit.replace(range, text, false);
            cx.notify();
            return;
        }
        if self.text_edit.is_none() && self.typing.is_none() {
            // Typing with a shape selected types into it.
            let Some(id) = self.selected else {
                return;
            };
            self.begin_text(id, None, cx);
        }
        if self.typing.is_some() {
            self.edit_typing(cx, |typing, deck| typing.commit(deck, range, text));
            return;
        }
        let Some(edit) = self.text_edit.as_mut() else {
            return;
        };
        edit.take_whole(range.is_none() && edit.marked.is_none());
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
        if let Some(edit) = self.notes_edit.as_mut() {
            edit.replace(range, new_text, true);
            cx.notify();
            return;
        }
        if self.text_edit.is_none() && self.typing.is_none() {
            let Some(id) = self.selected else {
                return;
            };
            self.begin_text(id, None, cx);
        }
        if let Some(typing) = self.typing.as_mut() {
            typing.compose(range, new_text);
            cx.notify();
            return;
        }
        let Some(edit) = self.text_edit.as_mut() else {
            return;
        };
        edit.take_whole(range.is_none() && edit.marked.is_none());
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
        // The candidate window goes at the caret, or under the cell being
        // typed into.
        let slide = self.slide_bounds.get()?;
        let scale = self.shown_scale;
        if let Some(typing) = &self.typing {
            let [a, b] = typing.geometry(&mut self.deck)?.caret;
            return Some(Bounds {
                origin: point(
                    slide.origin.x + px(a.x.min(b.x) as f32 * scale),
                    slide.origin.y + px(a.y.min(b.y) as f32 * scale),
                ),
                size: size(px(1.0), px((a.y - b.y).abs() as f32 * scale)),
            });
        }
        let rect = self.text_edit.as_ref()?.rect;
        Some(Bounds {
            origin: point(
                slide.origin.x + px(rect.x as f32 * scale),
                slide.origin.y + px(rect.y as f32 * scale),
            ),
            size: size(
                px(rect.width as f32 * scale),
                px(rect.height as f32 * scale),
            ),
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

impl Focusable for DeckView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for DeckView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for image in self.retired.drain(..) {
            window.drop_image(image).ok();
        }
        self.paint_thumbnails_if_needed(window.scale_factor());
        let editing_cell = self
            .text_edit
            .as_ref()
            .is_some_and(|edit| matches!(edit.target, TextTarget::Cell { .. }));
        let editing_merged = self.text_edit.as_ref().is_some_and(|edit| {
            self.deck
                .cell_span(self.current, edit.target)
                .is_some_and(|span| span != (1, 1))
        });
        let selecting_cells = self.cells.is_some();

        let (area_width, area_height) = workspace::editor_area(window, cx);
        let notes_height = if self.show_notes { NOTES_HEIGHT } else { 0.0 };
        let available = (
            (area_width - SIDEBAR_WIDTH - SLIDE_MARGIN * 2.0).max(40.0),
            (area_height - TOOLBAR_HEIGHT - FORMAT_BAR_HEIGHT - notes_height - SLIDE_MARGIN * 2.0)
                .max(40.0),
        );
        let (width, height) = match self.zoom {
            None => fitted(&self.deck, available),
            Some(zoom) => {
                let (slide_width, slide_height) = self.deck.slide_size();
                (slide_width as f32 * zoom, slide_height as f32 * zoom)
            }
        };
        self.shown_scale = width / (self.deck.slide_size().0 as f32).max(1.0);
        let device_width = (width * window.scale_factor()).round() as i32;
        if !repaint(
            &mut self.deck,
            &mut self.slide,
            &mut self.retired,
            self.current,
            device_width,
        ) {
            self.status = SharedString::from(i18n::format(
                "deck.paint_failed",
                &[("n", &(self.current + 1))],
            ));
        }

        let strip = self.format_strip();
        let format_bar = self
            .format_bar(&strip.clone().unwrap_or_default(), self.popup, cx)
            .when(strip.is_none(), |bar| bar.opacity(0.5));
        let count = self.deck.slide_count();
        let current_hidden = self.deck.is_hidden(self.current);
        let thumbnails: Vec<_> = (0..count).map(|index| self.thumbnail(index, cx)).collect();
        let notes = self
            .deck
            .notes(self.current)
            .filter(|notes| !notes.is_empty())
            .map(|notes| SharedString::from(notes.to_string()));
        let notes_lines = self.notes_edit.as_ref().map(notes_lines);
        let show_notes = self.show_notes;
        let typed = match &self.typing {
            Some(typing) => typing.geometry(&mut self.deck),
            None => None,
        };
        let overlays = self.overlays(self.shown_scale, typed.as_ref(), cx);
        let bounds_store = Rc::clone(&self.slide_bounds);
        let entity = cx.entity();

        div()
            .id("deck-root")
            .size_full()
            .flex()
            .bg(theme::window())
            .text_color(theme::text())
            .font_family(UI_FONT_FAMILY)
            .track_focus(&self.focus)
            .key_context("Deck")
            .on_key_down(cx.listener(Self::handle_key))
            .on_action(cx.listener(|view, _: &Print, _window, cx| view.print(cx)))
            .on_action(cx.listener(|view, _: &StartSlideshow, _window, cx| {
                view.start_slideshow(view.deck.first_shown(), cx)
            }))
            .on_action(cx.listener(|view, _: &Undo, _window, cx| view.step_history(true, cx)))
            .on_action(cx.listener(|view, _: &SelectAll, _window, cx| {
                view.edit_typing(cx, |typing, deck| {
                    typing.select_all(deck);
                    Ok(false)
                })
            }))
            .on_action(cx.listener(|view, _: &Redo, _window, cx| view.step_history(false, cx)))
            .on_action(cx.listener(|view, _: &Save, _window, cx| view.save(cx)))
            .on_action(cx.listener(|view, _: &SaveAs, _window, cx| view.save_as(cx)))
            // Moves and ups anywhere in the view, so a drag that leaves the
            // slide still follows the pointer and still lands.
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _event, _window, cx| view.mouse_up(cx)),
            )
            .child(
                div()
                    .w(px(SIDEBAR_WIDTH))
                    .h_full()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .bg(theme::sidebar())
                    .border_r_1()
                    .border_color(theme::border())
                    .child(
                        div()
                            .px_3()
                            .pt_3()
                            .pb_2()
                            .text_xs()
                            .text_color(theme::text_faint())
                            .child(i18n::format("deck.slides", &[("n", &count)])),
                    )
                    .child(
                        div()
                            .id("slide-thumbnails")
                            .flex_1()
                            .overflow_y_scroll()
                            .track_scroll(&self.thumbnail_scroll)
                            .flex()
                            .flex_col()
                            .gap_1()
                            .pb_3()
                            .children(thumbnails),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .h(px(TOOLBAR_HEIGHT))
                            .flex_none()
                            .flex()
                            .items_center()
                            .gap_1()
                            .px_3()
                            .bg(theme::panel())
                            .border_b_1()
                            .border_color(theme::border())
                            .child(div().px_1().text_xs().text_color(theme::text_dim()).child(
                                i18n::format(
                                    "deck.position",
                                    &[("current", &(self.current + 1)), ("count", &count)],
                                ),
                            ))
                            .child(div().w(px(1.0)).h(px(18.0)).mx_1().bg(theme::border()))
                            .child(
                                toolbar_button(
                                    "new-slide",
                                    "icons/file-plus.svg",
                                    i18n::text("deck.new_slide"),
                                )
                                .on_click(
                                    cx.listener(|view, _event, _window, cx| view.new_slide(cx)),
                                ),
                            )
                            .child(self.layouts_button(cx))
                            .child(
                                toolbar_button(
                                    "duplicate-slide",
                                    "icons/files.svg",
                                    i18n::text("deck.duplicate_slide"),
                                )
                                .on_click(
                                    cx.listener(|view, _event, _window, cx| {
                                        view.duplicate_slide(cx)
                                    }),
                                ),
                            )
                            .child(
                                toolbar_button(
                                    "delete-slide",
                                    "icons/trash-2.svg",
                                    i18n::text("deck.delete_slide"),
                                )
                                .on_click(
                                    cx.listener(|view, _event, _window, cx| view.delete_slide(cx)),
                                ),
                            )
                            .child(
                                toolbar_button(
                                    "hide-slide",
                                    "icons/eye-off.svg",
                                    i18n::text(if current_hidden {
                                        "deck.show_slide"
                                    } else {
                                        "deck.hide_slide"
                                    }),
                                )
                                .when(current_hidden, |button| {
                                    button.bg(theme::selected()).text_color(theme::text())
                                })
                                .on_click(
                                    cx.listener(|view, _event, _window, cx| view.toggle_hidden(cx)),
                                ),
                            )
                            .child(div().w(px(1.0)).h(px(18.0)).mx_1().bg(theme::border()))
                            .child(
                                toolbar_button(
                                    "insert-text-box",
                                    "icons/type.svg",
                                    i18n::text("deck.text_box"),
                                )
                                .when(self.inserting == Some(Insert::TextBox), |button| {
                                    button.bg(theme::selected())
                                })
                                .on_click(cx.listener(
                                    |view, _event, _window, cx| {
                                        view.start_insert(Insert::TextBox, cx)
                                    },
                                )),
                            )
                            .child(self.shapes_button(cx))
                            .child(
                                toolbar_button(
                                    "insert-picture",
                                    "icons/image.svg",
                                    i18n::text("deck.picture"),
                                )
                                .on_click(
                                    cx.listener(|view, _event, _window, cx| view.pick_picture(cx)),
                                ),
                            )
                            .child(div().flex_1())
                            // A table's layout, while one of its cells is
                            // typed into: PowerPoint's Layout tab.
                            .when(editing_cell, |bar| {
                                bar.child(
                                    toolbar_button(
                                        "row-below",
                                        "icons/arrow-down-to-line.svg",
                                        i18n::text("deck.row_below"),
                                    )
                                    .on_click(cx.listener(
                                        |view, _event, _window, cx| {
                                            view.change_table(
                                                |row, _| TableChange::InsertRow(row + 1),
                                                cx,
                                            )
                                        },
                                    )),
                                )
                                .child(
                                    toolbar_button(
                                        "column-right",
                                        "icons/arrow-right-to-line.svg",
                                        i18n::text("deck.column_right"),
                                    )
                                    .on_click(cx.listener(
                                        |view, _event, _window, cx| {
                                            view.change_table(
                                                |_, column| TableChange::InsertColumn(column + 1),
                                                cx,
                                            )
                                        },
                                    )),
                                )
                                .child(
                                    toolbar_button(
                                        "delete-row",
                                        "icons/trash-2.svg",
                                        i18n::text("deck.delete_row"),
                                    )
                                    .on_click(cx.listener(
                                        |view, _event, _window, cx| {
                                            view.change_table(
                                                |row, _| TableChange::DeleteRow(row),
                                                cx,
                                            )
                                        },
                                    )),
                                )
                                .child(
                                    toolbar_button(
                                        "delete-column",
                                        "icons/trash-2.svg",
                                        i18n::text("deck.delete_column"),
                                    )
                                    .on_click(cx.listener(
                                        |view, _event, _window, cx| {
                                            view.change_table(
                                                |_, column| TableChange::DeleteColumn(column),
                                                cx,
                                            )
                                        },
                                    )),
                                )
                            })
                            .when(selecting_cells, |bar| {
                                bar.child(
                                    toolbar_button(
                                        "merge-cells",
                                        "icons/merge.svg",
                                        i18n::text("deck.merge_cells"),
                                    )
                                    .on_click(
                                        cx.listener(|view, _event, _window, cx| {
                                            view.merge_cells(cx)
                                        }),
                                    ),
                                )
                            })
                            .when(editing_merged, |bar| {
                                bar.child(
                                    toolbar_button(
                                        "split-cell",
                                        "icons/split.svg",
                                        i18n::text("deck.split_cell"),
                                    )
                                    .on_click(
                                        cx.listener(|view, _event, _window, cx| {
                                            view.split_cell(cx)
                                        }),
                                    ),
                                )
                            })
                            .child(
                                toolbar_button(
                                    "notes",
                                    "icons/file-text.svg",
                                    i18n::text("deck.notes"),
                                )
                                .on_click(cx.listener(
                                    |view, _event, _window, cx| {
                                        view.show_notes = !view.show_notes;
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                toolbar_button(
                                    "print",
                                    "icons/printer.svg",
                                    i18n::text("deck.print"),
                                )
                                .on_click(cx.listener(|view, _event, _window, cx| view.print(cx))),
                            )
                            .child(
                                toolbar_button(
                                    "from-current",
                                    "icons/play.svg",
                                    i18n::text("deck.from_current"),
                                )
                                .on_click(cx.listener(
                                    |view, _event, _window, cx| {
                                        view.start_slideshow(view.current, cx)
                                    },
                                )),
                            )
                            .child(
                                div()
                                    .id("slideshow")
                                    .flex()
                                    .items_center()
                                    .gap_1p5()
                                    .ml_1()
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .text_xs()
                                    .text_color(gpui::white())
                                    .bg(theme::accent())
                                    .cursor_pointer()
                                    .hover(|button| button.opacity(0.9))
                                    .child(
                                        svg()
                                            .path("icons/play.svg")
                                            .size_3()
                                            .text_color(gpui::white()),
                                    )
                                    .child(i18n::text("deck.slideshow"))
                                    .on_click(cx.listener(|view, _event, _window, cx| {
                                        view.start_slideshow(view.deck.first_shown(), cx)
                                    })),
                            ),
                    )
                    .child(format_bar)
                    .child(
                        // Scrolls once a chosen zoom makes the slide bigger
                        // than the space; while fitting there is nothing to
                        // scroll. A click on the canvas off the slide clears
                        // the selection.
                        div()
                            .id("slide-canvas")
                            .flex_1()
                            .min_h_0()
                            .overflow_scroll()
                            .bg(theme::canvas())
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|view, _event, window, cx| {
                                    window.focus(&view.focus);
                                    view.slides_focused = false;
                                    view.popup = None;
                                    view.select(None, cx);
                                }),
                            )
                            .child(
                                div()
                                    .min_w_full()
                                    .min_h_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .p(px(SLIDE_MARGIN))
                                    .child(
                                        div()
                                            .id("slide")
                                            .when(self.inserting.is_some(), |slide| {
                                                slide.cursor(gpui::CursorStyle::Crosshair)
                                            })
                                            .relative()
                                            .flex_none()
                                            .w(px(width))
                                            .h(px(height))
                                            .bg(gpui::white())
                                            .shadow_lg()
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(
                                                    |view, event: &MouseDownEvent, window, cx| {
                                                        cx.stop_propagation();
                                                        view.mouse_down(event, window, cx);
                                                    },
                                                ),
                                            )
                                            .children(self.slide.as_ref().map(|painted| {
                                                img(painted.image.clone())
                                                    .w(px(width))
                                                    .h(px(height))
                                            }))
                                            .children(overlays)
                                            // Records where the slide is and
                                            // registers the input handler, so
                                            // typing reaches a shape.
                                            .child(
                                                canvas(
                                                    move |bounds, _window, _cx| {
                                                        bounds_store.set(Some(bounds));
                                                        bounds
                                                    },
                                                    move |_bounds, slide, window, cx| {
                                                        window.handle_input(
                                                            &entity.focus_handle(cx),
                                                            ElementInputHandler::new(
                                                                slide,
                                                                entity.clone(),
                                                            ),
                                                            cx,
                                                        );
                                                    },
                                                )
                                                .absolute()
                                                .top_0()
                                                .left_0()
                                                .size_full(),
                                            ),
                                    ),
                            ),
                    )
                    .when(show_notes, |column| {
                        column.child(
                            div()
                                .id("slide-notes")
                                .h(px(NOTES_HEIGHT))
                                .flex_none()
                                .overflow_y_scroll()
                                .border_t_1()
                                .border_color(theme::border())
                                .bg(theme::panel())
                                .px_4()
                                .py_2()
                                .text_sm()
                                .cursor(gpui::CursorStyle::IBeam)
                                .when(notes_lines.is_some(), |pane| {
                                    pane.border_t_2().border_color(theme::accent())
                                })
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|view, _event, window, cx| {
                                        window.focus(&view.focus);
                                        view.begin_notes(cx);
                                    }),
                                )
                                .child(match (notes_lines, notes) {
                                    (Some(lines), _) => div().children(lines),
                                    (None, Some(notes)) => div().child(notes),
                                    (None, None) => div()
                                        .text_color(theme::text_faint())
                                        .child(i18n::text("deck.add_notes")),
                                }),
                        )
                    }),
            )
    }
}

impl DeckView {
    fn open(path: &Path, cx: &mut App) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let deck = Deck::open(&bytes, crate::skia_backend()?)
            .map_err(|_| i18n::format("deck.empty", &[("name", &crate::shell::file_name(path))]))?;
        let mut view = DeckView {
            path: path.to_path_buf(),
            thumbnails: vec![None; deck.slide_count()],
            deck,
            current: 0,
            shapes: Vec::new(),
            selected: None,
            drag: None,
            typing: None,
            selecting_text: false,
            inserting: None,
            drawing: None,
            text_edit: None,
            notes_edit: None,
            cells: None,
            slide: None,
            retired: Vec::new(),
            show_notes: true,
            zoom: None,
            shown_scale: 1.0,
            slide_bounds: Rc::new(Cell::new(None)),
            thumbnail_scroll: ScrollHandle::new(),
            popup: None,
            slides_focused: false,
            focus: cx.focus_handle(),
            status: SharedString::from(""),
        };
        view.refresh_shapes();
        Ok(view)
    }
}

/// Opens `path` as a presentation view, for the workspace to show in a tab.
pub fn new_view(path: &Path, cx: &mut App) -> Result<Entity<DeckView>, String> {
    let view = DeckView::open(path, cx)?;
    Ok(cx.new(|_| view))
}

/// A slide show: one slide at a time, full screen, on black.
///
/// Its own window rather than a mode of the deck view, so the workspace —
/// explorer, tabs, status bar — stays as it was behind it, and ending the show
/// is closing a window.
struct SlideshowView {
    /// Its own copy, opened from the deck's bytes when the show started.
    deck: Deck<SkiaBackend>,
    current: usize,
    slide: Option<Painted>,
    retired: Vec<Arc<RenderImage>>,
    focus: FocusHandle,
}

impl SlideshowView {
    /// The next slide shown, or the one before: hidden slides are skipped.
    fn step(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        match self.deck.next_shown(self.current, forward) {
            Some(next) => {
                self.current = next;
                cx.notify();
            }
            // Past the last slide the show is over, as in PowerPoint.
            None if forward => window.remove_window(),
            None => {}
        }
    }

    fn handle_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "down" | "right" | "pagedown" | "space" | "enter" | "n" => self.step(true, window, cx),
            "up" | "left" | "pageup" | "backspace" | "p" => self.step(false, window, cx),
            "home" => {
                self.current = self.deck.first_shown();
                cx.notify();
            }
            "end" => {
                let count = self.deck.slide_count();
                self.current = self.deck.next_shown(count, false).unwrap_or(self.current);
                cx.notify();
            }
            "escape" => window.remove_window(),
            _ => return,
        }
        cx.stop_propagation();
    }
}

impl Focusable for SlideshowView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for SlideshowView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for image in self.retired.drain(..) {
            window.drop_image(image).ok();
        }
        let viewport = window.viewport_size();
        let (width, height) = fitted(
            &self.deck,
            (f32::from(viewport.width), f32::from(viewport.height)),
        );
        let device_width = (width * window.scale_factor()).round() as i32;
        repaint(
            &mut self.deck,
            &mut self.slide,
            &mut self.retired,
            self.current,
            device_width,
        );
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::black())
            .track_focus(&self.focus)
            .key_context("Slideshow")
            .on_key_down(cx.listener(Self::handle_key))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _event, window, cx| view.step(true, window, cx)),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|view, _event, window, cx| view.step(false, window, cx)),
            )
            .children(
                self.slide
                    .as_ref()
                    .map(|painted| img(painted.image.clone()).w(px(width)).h(px(height))),
            )
    }
}

fn open_slideshow(bytes: Vec<u8>, from: usize, cx: &mut App) -> Result<(), String> {
    let deck = Deck::open(&bytes, crate::skia_backend()?)?;
    let bounds = Bounds::maximized(None, cx);
    let window = cx
        .open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Fullscreen(bounds)),
                titlebar: None,
                ..Default::default()
            },
            |_window, cx| {
                cx.new(|cx| SlideshowView {
                    deck,
                    current: from,
                    slide: None,
                    retired: Vec::new(),
                    focus: cx.focus_handle(),
                })
            },
        )
        .map_err(|error| i18n::format("deck.slideshow_failed", &[("error", &error)]))?;
    window
        .update(cx, |view, window, cx| window.focus(&view.focus_handle(cx)))
        .map_err(|error| i18n::format("deck.slideshow_failed", &[("error", &error)]))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_view(
        cx: &mut gpui::TestAppContext,
    ) -> (Entity<DeckView>, &mut gpui::VisualTestContext) {
        open_fixture(cx, "sample-deck.pptx")
    }

    fn open_fixture<'a>(
        cx: &'a mut gpui::TestAppContext,
        name: &str,
    ) -> (Entity<DeckView>, &'a mut gpui::VisualTestContext) {
        let path = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../jade-core/public/fixtures"
        ))
        .join(name);
        let (view, cx) = cx.add_window_view(|_window, cx| DeckView::open(&path, cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        (view, cx)
    }

    fn rect_of(view: &Entity<DeckView>, cx: &mut gpui::VisualTestContext, id: u32) -> SlideRect {
        view.read_with(cx, |view, _| {
            SlideRect::of(&view.shape(id).unwrap().transform.unwrap())
        })
    }

    /// Tab selects a shape; the arrows move it, each one undoable step.
    #[gpui::test]
    fn arrows_move_the_selected_shape_and_undo_puts_it_back(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("tab");
        let id = view.read_with(cx, |view, _| view.selected.expect("Tab selects a shape"));
        let before = rect_of(&view, cx, id);
        cx.simulate_keystrokes("right right down");
        let after = rect_of(&view, cx, id);
        assert!((after.x - before.x - 2.0 * NUDGE_PX).abs() < 0.01);
        assert!((after.y - before.y - NUDGE_PX).abs() < 0.01);
        assert_eq!(after.width, before.width);
        assert!(view.read_with(cx, |view, _| view.is_dirty()));

        cx.dispatch_action(Undo);
        cx.dispatch_action(Undo);
        cx.dispatch_action(Undo);
        assert_eq!(rect_of(&view, cx, id), before);
        assert!(!view.read_with(cx, |view, _| view.is_dirty()));
    }

    /// With a text shape selected, Enter starts typing into it and Escape
    /// keeps what was typed.
    #[gpui::test]
    fn save_writes_the_file_that_was_opened(cx: &mut gpui::TestAppContext) {
        let source = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../jade-core/public/fixtures/sample-deck.pptx"
        ));
        let path = crate::shell::scratch_copy(&source, "deck.pptm");
        let (view, cx) = cx.add_window_view(|_window, cx| DeckView::open(&path, cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        let id = view.read_with(cx, |view, _| {
            view.shapes
                .iter()
                .find(|shape| {
                    view.deck
                        .shape_text(0, shape.id)
                        .is_some_and(|text| !text.is_empty())
                })
                .map(|shape| shape.id)
                .expect("the first slide has a text shape")
        });
        view.update(cx, |view, cx| view.select(Some(id), cx));
        cx.simulate_keystrokes("enter");
        cx.simulate_input(" saved");
        cx.simulate_keystrokes("escape");
        cx.dispatch_action(Save);
        view.read_with(cx, |view, _| assert!(!view.deck.is_dirty()));
        assert_eq!(crate::shell::siblings(&path), ["deck.pptm"]);
        let deck = Deck::open(
            &std::fs::read(&path).unwrap(),
            crate::skia_backend().unwrap(),
        )
        .unwrap();
        assert!(deck.shape_text(0, id).unwrap().ends_with(" saved"));
    }

    #[gpui::test]
    fn typed_text_lands_in_the_shape(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        let id = view.read_with(cx, |view, _| {
            view.shapes
                .iter()
                .find(|shape| {
                    view.deck
                        .shape_text(0, shape.id)
                        .is_some_and(|text| !text.is_empty())
                })
                .map(|shape| shape.id)
                .expect("the first slide has a text shape")
        });
        view.update(cx, |view, cx| view.select(Some(id), cx));
        cx.simulate_keystrokes("enter");
        cx.simulate_input(" 2026");
        cx.simulate_keystrokes("escape");
        let text = view.read_with(cx, |view, _| {
            assert!(view.text_edit.is_none());
            view.deck.shape_text(0, id).unwrap()
        });
        assert!(text.ends_with(" 2026"), "{text:?}");
    }

    /// Text is typed where the caret is, in the slide itself: ⌘↑ goes to
    /// the start, → one character on, ⇧→ selects, and what is typed
    /// replaces the selection; the caret is drawn where the text lies, and
    /// an IME's composition at it until committed. Escape leaves the text
    /// with the shape selected; Undo takes the typing back.
    #[gpui::test]
    fn text_is_typed_where_the_caret_is(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        let (id, before) = view.read_with(cx, |view, _| {
            view.shapes
                .iter()
                .find_map(|shape| {
                    view.deck
                        .shape_text(0, shape.id)
                        .filter(|text| text.chars().count() >= 4)
                        .map(|text| (shape.id, text))
                })
                .expect("the first slide has a text shape")
        });
        view.update(cx, |view, cx| view.select(Some(id), cx));
        cx.simulate_keystrokes("enter cmd-up right");
        cx.simulate_input("新");
        let first: String = before.chars().take(1).collect();
        let rest: String = before.chars().skip(1).collect();
        view.update(cx, |view, _| {
            assert_eq!(
                view.deck.shape_text(0, id).unwrap(),
                format!("{first}新{rest}")
            );
            let typing = view.typing.as_ref().expect("typing in place");
            assert_eq!(typing.focus, first.encode_utf16().count() + 1);
            assert!(typing.geometry(&mut view.deck).is_some());
        });
        cx.simulate_keystrokes("shift-right shift-right");
        cx.simulate_input("X");
        let skipped: String = rest.chars().skip(2).collect();
        view.read_with(cx, |view, _| {
            assert_eq!(
                view.deck.shape_text(0, id).unwrap(),
                format!("{first}新X{skipped}")
            );
        });
        // An IME composes at the caret; nothing goes in until it commits.
        view.update(cx, |view, cx| {
            if let Some(typing) = view.typing.as_mut() {
                typing.compose(None, "にほ");
            }
            cx.notify();
        });
        view.read_with(cx, |view, _| {
            assert_eq!(
                view.deck.shape_text(0, id).unwrap(),
                format!("{first}新X{skipped}")
            );
        });
        cx.simulate_input("日本");
        cx.simulate_keystrokes("escape");
        view.read_with(cx, |view, _| {
            assert!(view.typing.is_none());
            assert_eq!(view.selected, Some(id));
            assert_eq!(
                view.deck.shape_text(0, id).unwrap(),
                format!("{first}新X日本{skipped}")
            );
        });
        for _ in 0..3 {
            cx.dispatch_action(Undo);
        }
        view.read_with(cx, |view, _| {
            assert_eq!(view.deck.shape_text(0, id).unwrap(), before);
            assert!(!view.deck.is_dirty());
        });
    }

    /// With text selected, ⌘B and the format strip format it; with the
    /// shape selected and not typed into, all of its text. Each is a step
    /// of Undo.
    #[gpui::test]
    fn the_format_strip_formats_the_selection_or_the_whole_shape(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        let id = view.read_with(cx, |view, _| {
            view.shapes
                .iter()
                .find(|shape| {
                    view.deck
                        .shape_text(0, shape.id)
                        .is_some_and(|text| text.chars().count() >= 4)
                })
                .map(|shape| shape.id)
                .expect("the first slide has a text shape")
        });
        view.update(cx, |view, cx| view.select(Some(id), cx));
        let before = view.update(cx, |view, _| view.format_strip().unwrap());
        cx.simulate_keystrokes("enter cmd-up shift-right shift-right cmd-b");
        view.update(cx, |view, cx| {
            view.toggle_popup(Popup::Size, cx);
            view.format_slide_text(TextCommand::Size(40.0), cx);
            view.format_slide_text(TextCommand::Align(ParagraphAlignment::Center), cx);
        });
        view.update(cx, |view, _| {
            assert_eq!(view.popup, None);
            let strip = view.format_strip().unwrap();
            assert_eq!(strip.bold, !before.bold);
            assert_eq!(strip.size, Some(40.0));
            assert_eq!(strip.alignment, Some(ParagraphAlignment::Center));
            // The characters after the selection keep theirs.
            let after = view.deck.text_format(0, id, 3..4).unwrap();
            assert_ne!(after.size_pt, Some(40.0));
        });
        // Escape: the shape selected, the strip formats all its text.
        cx.simulate_keystrokes("escape cmd-i");
        view.update(cx, |view, _| {
            assert!(view.typing.is_none());
            assert_eq!(view.format_strip().unwrap().italic, !before.italic);
        });
        for _ in 0..4 {
            cx.dispatch_action(Undo);
        }
        view.read_with(cx, |view, _| assert!(!view.deck.is_dirty()));
    }

    /// Text Box, then a drag on the slide: a text box there, with the caret
    /// in it. A shape from Shapes, dropped by a click at its usual size. A
    /// picture, in the middle of the slide. Each is selected once in.
    #[gpui::test]
    fn text_boxes_shapes_and_pictures_are_inserted(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        let count = view.read_with(cx, |view, _| view.shapes.len());
        view.update(cx, |view, cx| {
            view.start_insert(Insert::TextBox, cx);
            view.drawing = Some(((100.0, 100.0), (300.0, 160.0)));
            view.mouse_up(cx);
        });
        cx.simulate_input("見出し");
        let text_box = view.read_with(cx, |view, _| {
            assert_eq!(view.inserting, None);
            assert_eq!(view.shapes.len(), count + 1);
            let id = view.typing.as_ref().expect("typing into the text box").id;
            assert_eq!(view.selected, Some(id));
            assert_eq!(view.deck.shape_text(0, id).as_deref(), Some("見出し"));
            id
        });
        cx.simulate_keystrokes("escape");
        view.update(cx, |view, cx| {
            view.toggle_popup(Popup::Shapes, cx);
            view.start_insert(
                Insert::Shape(jade_office_model::deck::ShapePreset::Ellipse),
                cx,
            );
            view.drawing = Some(((50.0, 50.0), (50.0, 50.0)));
            view.mouse_up(cx);
        });
        view.read_with(cx, |view, _| {
            let id = view.selected.expect("the shape is selected");
            assert_ne!(id, text_box);
            let rect = SlideRect::of(&view.shape(id).unwrap().transform.unwrap());
            assert!((rect.width - 96.0).abs() < 0.01 && (rect.x - 50.0).abs() < 0.01);
        });
        let mut png = Vec::new();
        image::RgbImage::from_pixel(40, 20, image::Rgb([200, 40, 40]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        view.update(cx, |view, cx| view.insert_picture(&png, cx));
        view.read_with(cx, |view, _| {
            assert_eq!(view.shapes.len(), count + 3);
            let picture = view.shape(view.selected.unwrap()).unwrap();
            assert_eq!(picture.kind, "picture");
        });
        // Not a picture: nothing goes in, and the status says why.
        view.update(cx, |view, cx| view.insert_picture(b"not a picture", cx));
        view.read_with(cx, |view, _| {
            assert_eq!(view.shapes.len(), count + 3);
            assert!(!view.status.is_empty());
        });
    }

    /// A click in the notes pane types into the slide's notes; Escape — or
    /// going to another slide — writes them, as one step of Undo.
    #[gpui::test]
    fn speaker_notes_are_typed_into_the_notes_pane(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        let before = view.read_with(cx, |view, _| view.deck.notes_text(0).unwrap());
        view.update(cx, |view, cx| view.begin_notes(cx));
        cx.simulate_input("話すこと");
        cx.simulate_keystrokes("enter");
        cx.simulate_input("次へ");
        cx.simulate_keystrokes("backspace escape");
        view.read_with(cx, |view, _| {
            assert!(view.notes_edit.is_none());
            assert_eq!(
                view.deck.notes_text(0).unwrap(),
                format!("{before}話すこと\n次")
            );
        });
        view.update(cx, |view, cx| view.begin_notes(cx));
        cx.simulate_input("!");
        view.update(cx, |view, cx| view.go_to(1, cx));
        view.read_with(cx, |view, _| {
            assert!(view.notes_edit.is_none());
            assert!(view.deck.notes_text(0).unwrap().ends_with("次!"));
        });
        cx.dispatch_action(Undo);
        cx.dispatch_action(Undo);
        view.read_with(cx, |view, _| {
            assert_eq!(view.deck.notes_text(0).unwrap(), before);
            assert!(!view.deck.is_dirty());
        });
    }

    /// In a table made in PowerPoint, a double-click types into the cell
    /// under it, and Enter on the selected table into its first cell; the
    /// other cells keep their text.
    #[gpui::test]
    fn typed_text_lands_in_the_table_cell(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_fixture(cx, "poi/table-with-different-font-colors.pptx");
        let cell = |row, column| TextTarget::Cell { id: 4, row, column };
        let point = view.update(cx, |view, _| {
            let rect = view.deck.text_rect(0, cell(2, 1)).unwrap();
            (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0)
        });
        view.update(cx, |view, cx| {
            view.select(Some(4), cx);
            view.begin_text(4, Some(point), cx);
        });
        cx.simulate_input(" 前年比");
        cx.simulate_keystrokes("escape");
        cx.simulate_keystrokes("enter");
        cx.simulate_input("!");
        cx.simulate_keystrokes("escape");
        view.read_with(cx, |view, _| {
            assert!(view.text_edit.is_none());
            assert_eq!(view.selected, Some(4));
            let text = |row, column| view.deck.text(0, cell(row, column)).unwrap();
            assert_eq!(text(2, 1), "Text 10 前年比");
            assert_eq!(text(0, 0), "Text 1!");
            assert_eq!(text(2, 0), "Text 9");
        });
    }

    /// Tab goes to the next cell, writing what was typed, with the cell's
    /// text selected — typing replaces it, as in PowerPoint; Tab in the
    /// last cell adds a row and goes to its first. The Layout buttons add
    /// and remove rows and columns beside the cell being typed into.
    #[gpui::test]
    fn tab_walks_the_table_and_adds_a_row_at_its_end(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_fixture(cx, "poi/table-with-different-font-colors.pptx");
        let cell = |row, column| TextTarget::Cell { id: 4, row, column };
        view.update(cx, |view, cx| {
            view.select(Some(4), cx);
            view.edit(cell(3, 2), false, cx);
        });
        cx.simulate_input("!");
        cx.simulate_keystrokes("tab");
        cx.simulate_input("?");
        cx.simulate_keystrokes("tab");
        cx.simulate_input("新しい行");
        cx.simulate_keystrokes("shift-tab");
        view.read_with(cx, |view, _| {
            let text = |row, column| view.deck.text(0, cell(row, column)).unwrap();
            assert_eq!(text(3, 2), "Text 15!");
            assert_eq!(text(3, 3), "?");
            assert_eq!(text(4, 0), "新しい行");
            assert_eq!(
                view.text_edit.as_ref().map(|edit| edit.target),
                Some(cell(3, 3))
            );
        });
        view.update(cx, |view, cx| {
            view.change_table(|_, column| TableChange::InsertColumn(column + 1), cx);
            view.edit(cell(0, 0), false, cx);
            view.change_table(|row, _| TableChange::DeleteRow(row), cx);
        });
        view.read_with(cx, |view, _| {
            assert!(view.text_edit.is_none());
            let text = |row, column| view.deck.text(0, cell(row, column)).unwrap();
            assert_eq!(text(0, 0), "Text 5");
            assert_eq!(text(0, 4), "");
            assert_eq!(text(3, 0), "新しい行");
        });
    }

    /// Shift+click from the cell being typed into selects the cells between;
    /// Merge Cells makes them one, typed into next, and Split Cells undoes
    /// it. Escape drops a selection.
    #[gpui::test]
    fn shift_click_selects_cells_to_merge_and_split(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_fixture(cx, "poi/table-with-different-font-colors.pptx");
        let cell = |row, column| TextTarget::Cell { id: 4, row, column };
        let (x, y) = view.update(cx, |view, _| {
            let rect = view.deck.text_rect(0, cell(2, 2)).unwrap();
            (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0)
        });
        view.update(cx, |view, cx| {
            view.select(Some(4), cx);
            view.edit(cell(1, 1), false, cx);
            assert!(view.select_cells_to(x, y, cx));
            assert!(view.text_edit.is_none());
        });
        cx.simulate_keystrokes("escape");
        view.update(cx, |view, cx| {
            assert!(view.cells.is_none());
            view.edit(cell(1, 1), false, cx);
            assert!(view.select_cells_to(x, y, cx));
            view.merge_cells(cx);
        });
        view.read_with(cx, |view, _| {
            assert_eq!(
                view.deck.text(0, cell(1, 1)).unwrap(),
                "Text 6\nText 7\nText 10\nText 11"
            );
            assert_eq!(view.deck.cell_span(0, cell(1, 1)), Some((2, 2)));
            assert_eq!(
                view.text_edit.as_ref().map(|edit| edit.target),
                Some(cell(1, 1))
            );
        });
        view.update(cx, |view, cx| view.split_cell(cx));
        view.read_with(cx, |view, _| {
            assert_eq!(view.deck.cell_span(0, cell(1, 1)), Some((1, 1)));
            assert_eq!(view.deck.text(0, cell(2, 2)).unwrap(), "");
        });
    }

    /// ⌘⇧N adds a slide after the one shown and shows it; ⌘D duplicates
    /// it; a thumbnail dropped on another moves its slide there; Delete in
    /// the slide list deletes one. Each is one step of Undo.
    #[gpui::test]
    fn the_slide_list_adds_duplicates_moves_and_deletes_slides(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        let count = view.read_with(cx, |view, _| view.deck.slide_count());
        cx.simulate_keystrokes("cmd-shift-n");
        view.read_with(cx, |view, _| {
            assert_eq!(view.deck.slide_count(), count + 1);
            assert_eq!(view.current, 1);
            assert_eq!(view.thumbnails.len(), count + 1);
        });
        cx.simulate_keystrokes("cmd-d");
        view.read_with(cx, |view, _| {
            assert_eq!(view.deck.slide_count(), count + 2);
            assert_eq!(view.current, 2);
        });
        view.update(cx, |view, cx| view.move_slide(2, 0, cx));
        view.read_with(cx, |view, _| assert_eq!(view.current, 0));
        view.update(cx, |view, cx| {
            view.go_to(count + 1, cx);
            view.slides_focused = true;
        });
        cx.simulate_keystrokes("delete");
        view.read_with(cx, |view, _| {
            assert_eq!(view.deck.slide_count(), count + 1);
            // The last slide gone, the one before it shows.
            assert_eq!(view.current, count);
        });
        for _ in 0..4 {
            cx.dispatch_action(Undo);
        }
        view.read_with(cx, |view, _| {
            assert_eq!(view.deck.slide_count(), count);
            assert_eq!(view.thumbnails.len(), count);
            assert!(!view.deck.is_dirty());
        });
    }

    /// Hide Slide marks the slide in the list; a slide show steps over it.
    #[gpui::test]
    fn a_hidden_slide_is_skipped_by_the_slide_show(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        view.update(cx, |view, cx| {
            view.go_to(1, cx);
            view.toggle_hidden(cx);
        });
        let bytes = view.read_with(cx, |view, _| {
            assert!(view.deck.is_hidden(1));
            assert_eq!(view.current, 1);
            view.deck.bytes().to_vec()
        });
        let (show, cx) = cx.add_window_view(|_window, cx| SlideshowView {
            deck: Deck::open(&bytes, crate::skia_backend().unwrap()).unwrap(),
            current: 0,
            slide: None,
            retired: Vec::new(),
            focus: cx.focus_handle(),
        });
        cx.update(|window, cx| window.focus(&show.focus_handle(cx)));
        cx.simulate_keystrokes("right");
        show.read_with(cx, |show, _| assert_eq!(show.current, 2));
        cx.simulate_keystrokes("left");
        show.read_with(cx, |show, _| assert_eq!(show.current, 0));
    }

    /// Layout ▾ lists the slide's layouts; picking one puts the slide on it.
    #[gpui::test]
    fn a_slide_is_put_on_another_layout(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        let other = view.update(cx, |view, cx| {
            view.go_to(1, cx);
            view.toggle_popup(Popup::Layouts, cx);
            view.deck
                .slide_layouts(1)
                .into_iter()
                .find(|layout| !layout.current)
                .expect("the deck has another layout")
                .part_name
        });
        view.update(cx, |view, cx| view.set_layout(&other, cx));
        view.read_with(cx, |view, _| {
            assert_eq!(view.popup, None);
            assert_eq!(view.current, 1);
            let current = view.deck.slide_layouts(1).into_iter().find(|l| l.current);
            assert_eq!(current.map(|layout| layout.part_name), Some(other));
        });
    }

    /// Delete removes the selected shape; Undo brings it back.
    #[gpui::test]
    fn delete_removes_the_shape_and_undo_restores_it(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_view(cx);
        cx.simulate_keystrokes("tab");
        let id = view.read_with(cx, |view, _| view.selected.unwrap());
        let count = view.read_with(cx, |view, _| view.shapes.len());
        cx.simulate_keystrokes("delete");
        view.read_with(cx, |view, _| {
            assert_eq!(view.shapes.len(), count - 1);
            assert!(view.shape(id).is_none());
            assert!(view.selected.is_none());
        });
        cx.dispatch_action(Undo);
        view.read_with(cx, |view, _| assert!(view.shape(id).is_some()));
    }
}

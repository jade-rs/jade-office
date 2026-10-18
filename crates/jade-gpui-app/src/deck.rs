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
    canvas, div, img, point, prelude::*, px, size, svg, App, Bounds, Context, ElementInputHandler,
    Entity, EntityInputHandler, FocusHandle, Focusable, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, Pixels, RenderImage, ScrollHandle, SharedString, UTF16Selection, Window,
    WindowBounds, WindowOptions,
};
use jade_office_model::deck::{Deck, ShapeRect as SlideRect, ShapeTransformInfo};
use jade_office_model::skia::SkiaBackend;
use jade_office_model::Backend;
use std::cell::Cell;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use crate::editor::{self, Editor};
use crate::{
    byte_to_utf16, i18n, native_print, theme, utf16_to_byte, workspace, Print, Redo, Save, SaveAs,
    StartSlideshow, Undo, UI_FONT_FAMILY,
};

/// The thumbnail strip's width, and the width each thumbnail is shown at.
const SIDEBAR_WIDTH: f32 = 200.0;
const THUMBNAIL_WIDTH: f32 = 152.0;
/// The bar above the slide: slide number, print, slide show.
const TOOLBAR_HEIGHT: f32 = 40.0;
/// The speaker-notes pane under the slide.
const NOTES_HEIGHT: f32 = 110.0;
/// Space left round the slide inside its canvas.
const SLIDE_MARGIN: f32 = 28.0;
/// How far an arrow key moves a shape, in slide px; ⌥ moves one px.
const NUDGE_PX: f64 = 6.0;
/// The smallest a resize leaves a shape, in slide px.
const MIN_SHAPE_PX: f64 = 4.0;
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

/// What a drag on the slide does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DragMode {
    Move,
    /// A corner handle: which edges it moves, as (left?, top?).
    Resize {
        left: bool,
        top: bool,
    },
}

/// `rect` after dragging by `(dx, dy)` slide px in `mode`.
fn dragged(rect: SlideRect, mode: DragMode, dx: f64, dy: f64) -> SlideRect {
    match mode {
        DragMode::Move => SlideRect {
            x: rect.x + dx,
            y: rect.y + dy,
            ..rect
        },
        DragMode::Resize { left, top } => {
            let (x, width) = if left {
                let width = (rect.width - dx).max(MIN_SHAPE_PX);
                (rect.x + rect.width - width, width)
            } else {
                (rect.x, (rect.width + dx).max(MIN_SHAPE_PX))
            };
            let (y, height) = if top {
                let height = (rect.height - dy).max(MIN_SHAPE_PX);
                (rect.y + rect.height - height, height)
            } else {
                (rect.y, (rect.height + dy).max(MIN_SHAPE_PX))
            };
            SlideRect {
                x,
                y,
                width,
                height,
            }
        }
    }
}

struct ShapeDrag {
    id: u32,
    mode: DragMode,
    /// Where the drag started, in slide px.
    from: (f64, f64),
    /// How far it has gone, in slide px.
    delta: (f64, f64),
}

/// Text being typed into a shape.
struct TextEdit {
    id: u32,
    text: String,
    /// The IME's composition, as a byte range of `text`.
    marked: Option<Range<usize>>,
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
    text_edit: Option<TextEdit>,
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
    ) {
        match change(&mut self.deck, self.current) {
            Ok(true) => self.reload(false, cx),
            Ok(false) => {}
            Err(error) => {
                self.status =
                    SharedString::from(i18n::format("deck.edit_failed", &[("error", &error)]));
                cx.notify();
            }
        }
    }

    fn step_history(&mut self, undo: bool, cx: &mut Context<Self>) {
        self.commit_text(cx);
        let result = if undo {
            self.deck.undo()
        } else {
            self.deck.redo()
        };
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
        let rect = dragged(SlideRect::of(&transform), DragMode::Move, dx, dy);
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
        let Some((x, y)) = self.slide_point(event.position) else {
            return;
        };
        if let Some(edit) = &self.text_edit {
            let inside = self
                .shape(edit.id)
                .and_then(|shape| shape.transform)
                .is_some_and(|transform| SlideRect::of(&transform).contains(x, y));
            if inside {
                return;
            }
        }
        let hit = self.shape_at(x, y);
        self.select(hit, cx);
        let Some(id) = hit else {
            return;
        };
        if event.click_count >= 2 {
            self.begin_text(id, None, cx);
            return;
        }
        if self.shape(id).is_some_and(|shape| shape.can_move) {
            self.drag = Some(ShapeDrag {
                id,
                mode: DragMode::Move,
                from: (x, y),
                delta: (0.0, 0.0),
            });
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
        self.drag = Some(ShapeDrag {
            id,
            mode: DragMode::Resize { left, top },
            from,
            delta: (0.0, 0.0),
        });
    }

    fn mouse_move(&mut self, event: &MouseMoveEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if !event.dragging() {
            return;
        }
        let Some((x, y)) = self.slide_point(event.position) else {
            return;
        };
        if let Some(drag) = self.drag.as_mut() {
            drag.delta = (x - drag.from.0, y - drag.from.1);
            cx.notify();
        }
    }

    fn mouse_up(&mut self, cx: &mut Context<Self>) {
        let Some(drag) = self.drag.take() else {
            return;
        };
        if drag.delta.0.abs() < 0.5 && drag.delta.1.abs() < 0.5 {
            cx.notify();
            return;
        }
        let Some(transform) = self.shape(drag.id).and_then(|shape| shape.transform) else {
            return;
        };
        let rect = dragged(
            SlideRect::of(&transform),
            drag.mode,
            drag.delta.0,
            drag.delta.1,
        );
        self.place(drag.id, rect, cx);
    }

    // ---- text ------------------------------------------------------------

    /// Starts typing into shape `id`, from its text plus `typed`.
    fn begin_text(&mut self, id: u32, typed: Option<&str>, cx: &mut Context<Self>) {
        let Some(text) = self.deck.shape_text(self.current, id) else {
            self.status = SharedString::from(i18n::text("deck.text_locked"));
            cx.notify();
            return;
        };
        let mut text = text;
        if let Some(typed) = typed {
            text.push_str(typed);
        }
        self.drag = None;
        self.text_edit = Some(TextEdit {
            id,
            text,
            marked: None,
        });
        cx.notify();
    }

    /// Writes the text being typed, if any, as one undoable change.
    fn commit_text(&mut self, cx: &mut Context<Self>) {
        let Some(edit) = self.text_edit.take() else {
            return;
        };
        let (id, text) = (edit.id, edit.text);
        self.write(cx, |deck, slide| deck.set_shape_text(slide, id, &text));
        cx.notify();
    }

    // ---- keys ------------------------------------------------------------

    fn handle_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let modifiers = keystroke.modifiers;
        if modifiers.platform || modifiers.control {
            return;
        }
        let key = keystroke.key.as_str();

        if let Some(edit) = self.text_edit.as_mut() {
            if edit.marked.is_some() {
                return;
            }
            match key {
                // As in PowerPoint, Escape leaves the text keeping what was
                // typed, with the shape still selected.
                "escape" => self.commit_text(cx),
                "enter" => {
                    edit.text.push('\n');
                    cx.notify();
                }
                "backspace" => {
                    edit.text.pop();
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

    fn thumbnail(&self, index: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = index == self.current;
        let (slide_width, slide_height) = self.deck.slide_size();
        let height = THUMBNAIL_WIDTH * (slide_height / slide_width.max(1.0)) as f32;
        div()
            .id(("slide-thumbnail", index))
            .flex()
            .gap_2()
            .px_2()
            .py_1()
            .cursor_pointer()
            .on_click(cx.listener(move |view, _event, _window, cx| view.go_to(index, cx)))
            .child(
                div()
                    .w(px(16.0))
                    .text_xs()
                    .text_color(if selected {
                        theme::accent()
                    } else {
                        theme::text_faint()
                    })
                    .child(format!("{}", index + 1)),
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
    fn overlays(&self, scale: f32, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let mut out = Vec::new();
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
            rect = dragged(rect, drag.mode, drag.delta.0, drag.delta.1);
        }
        let (left, top, width, height) = (
            rect.x as f32 * scale,
            rect.y as f32 * scale,
            rect.width as f32 * scale,
            rect.height as f32 * scale,
        );

        if let Some(edit) = self.text_edit.as_ref().filter(|edit| edit.id == id) {
            let (before, marked, after) = match &edit.marked {
                Some(range) => (
                    edit.text[..range.start].to_string(),
                    edit.text[range.clone()].to_string(),
                    edit.text[range.end..].to_string(),
                ),
                None => (edit.text.clone(), String::new(), String::new()),
            };
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
                            .child(before)
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
        let edit = self.text_edit.as_ref()?;
        let marked = edit.marked.clone()?;
        Some(byte_to_utf16(&edit.text, marked.start)..byte_to_utf16(&edit.text, marked.end))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
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
        if self.text_edit.is_none() {
            // Typing with a shape selected types into it.
            let Some(id) = self.selected else {
                return;
            };
            self.begin_text(id, None, cx);
        }
        let Some(edit) = self.text_edit.as_mut() else {
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
        if self.text_edit.is_none() {
            let Some(id) = self.selected else {
                return;
            };
            self.begin_text(id, None, cx);
        }
        let Some(edit) = self.text_edit.as_mut() else {
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
        // The candidate window goes under the shape being typed into.
        let slide = self.slide_bounds.get()?;
        let id = self.text_edit.as_ref()?.id;
        let rect = SlideRect::of(&self.shape(id)?.transform?);
        let scale = self.shown_scale;
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

        let (area_width, area_height) = workspace::editor_area(window, cx);
        let notes_height = if self.show_notes { NOTES_HEIGHT } else { 0.0 };
        let available = (
            (area_width - SIDEBAR_WIDTH - SLIDE_MARGIN * 2.0).max(40.0),
            (area_height - TOOLBAR_HEIGHT - notes_height - SLIDE_MARGIN * 2.0).max(40.0),
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

        let count = self.deck.slide_count();
        let thumbnails: Vec<_> = (0..count).map(|index| self.thumbnail(index, cx)).collect();
        let notes = self
            .deck
            .notes(self.current)
            .map(|notes| SharedString::from(notes.to_string()));
        let show_notes = self.show_notes;
        let overlays = self.overlays(self.shown_scale, cx);
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
            .on_action(
                cx.listener(|view, _: &StartSlideshow, _window, cx| view.start_slideshow(0, cx)),
            )
            .on_action(cx.listener(|view, _: &Undo, _window, cx| view.step_history(true, cx)))
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
                            .child(div().flex_1())
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
                                        view.start_slideshow(0, cx)
                                    })),
                            ),
                    )
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
                                .child(match notes {
                                    Some(notes) => div().child(notes),
                                    None => div()
                                        .text_color(theme::text_faint())
                                        .child(i18n::text("deck.no_notes")),
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
            text_edit: None,
            slide: None,
            retired: Vec::new(),
            show_notes: true,
            zoom: None,
            shown_scale: 1.0,
            slide_bounds: Rc::new(Cell::new(None)),
            thumbnail_scroll: ScrollHandle::new(),
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
    fn step(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let next = self.current as isize + delta;
        if next >= self.deck.slide_count() as isize {
            // Past the last slide the show is over, as in PowerPoint.
            window.remove_window();
            return;
        }
        self.current = next.max(0) as usize;
        cx.notify();
    }

    fn handle_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "down" | "right" | "pagedown" | "space" | "enter" | "n" => self.step(1, window, cx),
            "up" | "left" | "pageup" | "backspace" | "p" => self.step(-1, window, cx),
            "home" => {
                self.current = 0;
                cx.notify();
            }
            "end" => {
                self.current = self.deck.slide_count().saturating_sub(1);
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
                cx.listener(|view, _event, window, cx| view.step(1, window, cx)),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|view, _event, window, cx| view.step(-1, window, cx)),
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
        let path = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../jade-core/public/fixtures/sample-deck.pptx"
        ));
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

    #[test]
    fn a_corner_drag_resizes_from_the_opposite_corner() {
        let rect = SlideRect {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 50.0,
        };
        let grown = dragged(
            rect,
            DragMode::Resize {
                left: true,
                top: true,
            },
            -5.0,
            -10.0,
        );
        assert_eq!(grown.x + grown.width, rect.x + rect.width);
        assert_eq!(grown.y + grown.height, rect.y + rect.height);
        assert_eq!((grown.width, grown.height), (105.0, 60.0));
        // A drag past the far edge stops at the minimum size.
        let squashed = dragged(
            rect,
            DragMode::Resize {
                left: false,
                top: false,
            },
            -500.0,
            0.0,
        );
        assert_eq!(squashed.width, MIN_SHAPE_PX);
        let moved = dragged(rect, DragMode::Move, 3.0, 4.0);
        assert_eq!((moved.x, moved.y, moved.width), (13.0, 24.0, 100.0));
    }
}

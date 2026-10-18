//! jade in the browser, through gpui.
//!
//! The page fetches the fonts and a document and calls [`run`]. From there it
//! is a gpui application like the native one: gpui-pre's web platform owns a
//! canvas and draws with WebGPU (or WebGL2), and the document's pixels come
//! from `jade-web` — the engine's renderers on the pure-Rust painter — as
//! `RenderImage`s, the same way the native views take Skia's.
//!
//! Sheets and slides can be edited, through the same editing cores the
//! native app uses (`jade-web`'s `Workbook` and `Deck`): select a cell and
//! type, an IME's composition included; select a shape and type into it,
//! move it with the arrow keys, delete it; undo and redo; and save, which
//! hands the bytes to the page to download. Word documents are viewed.

#![cfg(target_arch = "wasm32")]

use std::borrow::Cow;
use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    canvas, div, img, point, prelude::*, px, rgb, size, App, Application, Bounds, Context,
    ElementInputHandler, EntityInputHandler, FocusHandle, KeyDownEvent, MouseButton,
    MouseDownEvent, Pixels, RenderImage, ScrollHandle, SharedString, UTF16Selection, Window,
    WindowOptions,
};
use gpui_web::{CanvasFontFallback, WebBackendPreference, WebPlatform};
use jade_tiny_skia_painter::TinySkiaPainter;
use jade_web::sheet::{reference_text, span, CellRef, MAX_COLUMN, MAX_ROW};
use wasm_bindgen::prelude::*;

const UI_FONT: &str = "Noto Sans JP";
const THUMBNAIL_WIDTH: f32 = 150.0;
const SIDEBAR_WIDTH: f32 = 186.0;
const HEADER_HEIGHT: f32 = 40.0;
const FORMULA_BAR_HEIGHT: f32 = 30.0;
const TABS_HEIGHT: f32 = 30.0;
/// How far an arrow key moves a shape, in slide px; ⌥ moves one.
const NUDGE_PX: f64 = 6.0;

const ACCENT: u32 = 0x818cf8;
const BORDER: u32 = 0x27272a;
const PANEL: u32 = 0x0d0d0f;
const TEXT: u32 = 0xe4e4e7;
const DIM: u32 = 0xa1a1aa;

fn image(painter: TinySkiaPainter) -> Option<Arc<RenderImage>> {
    let (width, height) = (painter.width(), painter.height());
    let buffer = image::ImageBuffer::from_raw(width, height, painter.bgra_premultiplied())?;
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
}

/// UTF-16 offset → byte offset in `text`, clamped to its end.
fn utf16_to_byte(text: &str, offset: usize) -> usize {
    let mut units = 0;
    for (byte, ch) in text.char_indices() {
        if units >= offset {
            return byte;
        }
        units += ch.len_utf16();
    }
    text.len()
}

fn byte_to_utf16(text: &str, byte: usize) -> usize {
    text[..byte.min(text.len())].encode_utf16().count()
}

/// Text being typed: into a cell or into a shape.
struct TextEdit {
    text: String,
    /// The IME's composition, as a byte range of `text`.
    marked: Option<Range<usize>>,
}

impl TextEdit {
    fn new(text: String) -> Self {
        Self { text, marked: None }
    }

    /// Replaces a UTF-16 range — or the composition, or nothing at the end —
    /// with `text`, marking it as the composition when `mark` is set.
    fn replace(&mut self, range: Option<Range<usize>>, text: &str, mark: bool) {
        let bytes = match range {
            Some(range) => {
                utf16_to_byte(&self.text, range.start)..utf16_to_byte(&self.text, range.end)
            }
            None => self
                .marked
                .clone()
                .unwrap_or(self.text.len()..self.text.len()),
        };
        let start = bytes.start;
        self.text.replace_range(bytes, text);
        self.marked = (mark && !text.is_empty()).then(|| start..start + text.len());
    }
}

/// A sheet viewport's pixels and what they were painted for.
struct PaintedSheet {
    scroll: (f32, f32),
    device: (u32, u32),
    selection: (CellRef, CellRef),
    generation: u64,
    image: Arc<RenderImage>,
}

/// A slide's pixels and what they were painted for: slide, device width,
/// edit generation.
type PaintedSlide = (usize, u32, u64, Arc<RenderImage>);

enum Document {
    /// Word: every page painted at open, shown as a column of pages.
    Pages {
        pages: Vec<Arc<RenderImage>>,
        page_size: (f32, f32),
    },
    Deck {
        deck: jade_web::Deck,
        current: usize,
        thumbnails: Vec<Option<Arc<RenderImage>>>,
        slide: Option<PaintedSlide>,
        selected: Option<u32>,
        edit: Option<(u32, TextEdit)>,
        /// Window px per slide px of the slide on show.
        scale: f32,
    },
    Workbook {
        book: jade_web::Workbook,
        sheets: Vec<SharedString>,
        anchor: CellRef,
        focus: CellRef,
        edit: Option<TextEdit>,
        scroll: ScrollHandle,
        painted: Option<PaintedSheet>,
        viewport: (f32, f32),
    },
}

struct Viewer {
    name: SharedString,
    document: Document,
    retired: Vec<Arc<RenderImage>>,
    /// Bumped by every edit, so pictures painted before it are repainted.
    generation: u64,
    status: SharedString,
    /// Where the editable surface — the grid or the slide — sits in the
    /// window, recorded at paint: clicks arrive in window coordinates.
    surface: Rc<Cell<Option<Bounds<Pixels>>>>,
    focus: FocusHandle,
    /// The page's download hook, called with the file name and bytes.
    on_save: js_sys::Function,
}

impl Viewer {
    /// After an edit: repaint what showed the old content.
    fn edited(&mut self, result: Result<bool, String>, cx: &mut Context<Self>) {
        match result {
            Ok(true) => {
                self.generation += 1;
                self.status = SharedString::from("");
                // Undo can touch any slide; thumbnails are cheap to repaint.
                if let Document::Deck { thumbnails, .. } = &mut self.document {
                    for thumbnail in thumbnails.iter_mut() {
                        if let Some(old) = thumbnail.take() {
                            self.retired.push(old);
                        }
                    }
                }
            }
            Ok(false) => {}
            Err(error) => self.status = SharedString::from(error),
        }
        cx.notify();
    }

    fn is_dirty(&self) -> bool {
        match &self.document {
            Document::Pages { .. } => false,
            Document::Deck { deck, .. } => deck.is_dirty(),
            Document::Workbook { book, .. } => book.is_dirty(),
        }
    }

    /// Commits any edit in progress and hands the file's bytes to the page.
    fn save(&mut self, cx: &mut Context<Self>) {
        self.commit_cell(cx);
        self.commit_slide_text(cx);
        let bytes = match &mut self.document {
            Document::Pages { .. } => return,
            Document::Deck { deck, .. } => Ok(deck.save_bytes()),
            Document::Workbook { book, .. } => book.save_bytes(),
        };
        match bytes {
            Ok(bytes) => {
                let array = js_sys::Uint8Array::from(bytes.as_slice());
                self.on_save
                    .call2(&JsValue::NULL, &JsValue::from_str(&self.name), &array)
                    .ok();
                self.status = SharedString::from("保存しました");
            }
            Err(error) => self.status = SharedString::from(error),
        }
        cx.notify();
    }

    fn step_history(&mut self, undo: bool, cx: &mut Context<Self>) {
        let result = match &mut self.document {
            Document::Pages { .. } => return,
            Document::Deck { deck, edit, .. } => {
                *edit = None;
                if undo {
                    deck.undo()
                } else {
                    deck.redo()
                }
            }
            Document::Workbook { book, edit, .. } => {
                *edit = None;
                if undo {
                    book.undo()
                } else {
                    book.redo()
                }
            }
        };
        self.edited(result, cx);
    }

    // ---- the workbook ---------------------------------------------------

    fn sheet_scroll(scroll: &ScrollHandle) -> (f64, f64) {
        let offset = scroll.offset();
        (
            (-f64::from(offset.x)).max(0.0),
            (-f64::from(offset.y)).max(0.0),
        )
    }

    fn select_cell(&mut self, to: CellRef, extend: bool, cx: &mut Context<Self>) {
        let Document::Workbook {
            book,
            anchor,
            focus,
            scroll,
            viewport,
            ..
        } = &mut self.document
        else {
            return;
        };
        if extend {
            *focus = to;
        } else {
            *anchor = to;
            *focus = to;
        }
        book.reach(to);
        let at = Self::sheet_scroll(scroll);
        let (x, y) = book.reveal(to, at, (f64::from(viewport.0), f64::from(viewport.1)));
        if (x, y) != at {
            scroll.set_offset(point(px(-x as f32), px(-y as f32)));
        }
        cx.notify();
    }

    fn step_cell(&mut self, columns: i64, rows: i64, extend: bool, cx: &mut Context<Self>) {
        let Document::Workbook { anchor, focus, .. } = &self.document else {
            return;
        };
        let from = if extend { *focus } else { *anchor };
        let to = CellRef::new(
            (i64::from(from.column) + columns).clamp(0, i64::from(MAX_COLUMN)) as u32,
            (i64::from(from.row) + rows).clamp(0, i64::from(MAX_ROW)) as u32,
        );
        self.select_cell(to, extend, cx);
    }

    /// Writes the cell being typed into, if any; the write recalculates.
    fn commit_cell(&mut self, cx: &mut Context<Self>) {
        let Document::Workbook {
            book, edit, anchor, ..
        } = &mut self.document
        else {
            return;
        };
        let Some(text) = edit.take() else {
            return;
        };
        let result = book.set_cell(*anchor, &text.text);
        self.edited(result, cx);
    }

    fn begin_cell_edit(&mut self, text: Option<String>, cx: &mut Context<Self>) {
        if let Document::Workbook {
            book, edit, anchor, ..
        } = &mut self.document
        {
            let text = text.unwrap_or_else(|| book.edit_text(*anchor));
            *edit = Some(TextEdit::new(text));
            cx.notify();
        }
    }

    fn sheet_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let shift = event.keystroke.modifiers.shift;
        let key = event.keystroke.key.as_str();
        let Document::Workbook {
            book,
            edit,
            anchor,
            focus,
            ..
        } = &mut self.document
        else {
            return false;
        };
        if let Some(text) = edit.as_mut() {
            // Mid-composition the keys are the IME's.
            if text.marked.is_some() {
                return false;
            }
            match key {
                "enter" => {
                    self.commit_cell(cx);
                    self.step_cell(0, if shift { -1 } else { 1 }, false, cx);
                }
                "tab" => {
                    self.commit_cell(cx);
                    self.step_cell(if shift { -1 } else { 1 }, 0, false, cx);
                }
                "escape" => {
                    *edit = None;
                    cx.notify();
                }
                "backspace" => {
                    text.text.pop();
                    cx.notify();
                }
                _ => return false,
            }
            return true;
        }
        match key {
            "up" => self.step_cell(0, -1, shift, cx),
            "down" => self.step_cell(0, 1, shift, cx),
            "left" => self.step_cell(-1, 0, shift, cx),
            "right" => self.step_cell(1, 0, shift, cx),
            "enter" => self.step_cell(0, if shift { -1 } else { 1 }, false, cx),
            "tab" => self.step_cell(if shift { -1 } else { 1 }, 0, false, cx),
            "f2" => self.begin_cell_edit(None, cx),
            "delete" | "backspace" => {
                let result = book.clear(span(*anchor, *focus));
                self.edited(result, cx);
            }
            // Anything else is text, which arrives through the input handler
            // and starts an edit there.
            _ => return false,
        }
        true
    }

    fn sheet_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        self.commit_cell(cx);
        let Some(bounds) = self.surface.get() else {
            return;
        };
        let Document::Workbook {
            book,
            anchor,
            focus,
            scroll,
            ..
        } = &mut self.document
        else {
            return;
        };
        let x = f64::from(event.position.x - bounds.origin.x);
        let y = f64::from(event.position.y - bounds.origin.y);
        let Some((start, end)) = book.hit(x, y, Self::sheet_scroll(scroll)) else {
            return;
        };
        if event.modifiers.shift {
            *focus = end;
        } else {
            *anchor = start;
            *focus = end;
        }
        if event.click_count >= 2 && start == end {
            self.begin_cell_edit(None, cx);
        }
        cx.notify();
    }

    fn workbook_body(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui::Div {
        let generation = self.generation;
        let Document::Workbook {
            book,
            sheets,
            anchor,
            focus,
            edit,
            scroll,
            painted,
            viewport,
        } = &mut self.document
        else {
            unreachable!()
        };
        let window_size = window.viewport_size();
        let scale = window.scale_factor();
        let size = (
            f32::from(window_size.width).max(1.0),
            (f32::from(window_size.height) - HEADER_HEIGHT - FORMULA_BAR_HEIGHT - TABS_HEIGHT)
                .max(1.0),
        );
        *viewport = size;
        let offset = scroll.offset();
        let at = (-f32::from(offset.x), -f32::from(offset.y));
        let device = ((size.0 * scale) as u32, (size.1 * scale) as u32);
        let selection = (*anchor, *focus);
        let fresh = painted.as_ref().is_some_and(|p| {
            p.scroll == at
                && p.device == device
                && p.selection == selection
                && p.generation == generation
        });
        if !fresh {
            if let Some(image) = book
                .render_viewport(
                    (f64::from(at.0), f64::from(at.1)),
                    device.0,
                    device.1,
                    f64::from(scale),
                    Some(selection),
                )
                .and_then(image)
            {
                if let Some(old) = painted.replace(PaintedSheet {
                    scroll: at,
                    device,
                    selection,
                    generation,
                    image,
                }) {
                    self.retired.push(old.image);
                }
            }
        }
        let (sheet_width, sheet_height) = book.sheet_size();
        let picture = painted.as_ref().map(|p| p.image.clone());
        let current = book.sheet_index();

        // The name box and formula bar.
        let range = span(*anchor, *focus);
        let name_box = if range.start == range.end {
            reference_text(range.start)
        } else {
            format!(
                "{}:{}",
                reference_text(range.start),
                reference_text(range.end)
            )
        };
        let content = match edit {
            Some(text) => text.text.clone(),
            None => book.edit_text(*anchor),
        };
        let editing = edit.is_some();

        // The cell editor over the active cell.
        let cell_editor = edit.as_ref().and_then(|text| {
            let (x, y, w, h) = book.cell_rect(*anchor, (f64::from(at.0), f64::from(at.1)))?;
            Some(
                div()
                    .absolute()
                    .left(px(x as f32))
                    .top(px(y as f32))
                    .min_w(px(w as f32))
                    .h(px(h as f32))
                    .flex()
                    .items_center()
                    .px(px(3.0))
                    .bg(gpui::white())
                    .border_2()
                    .border_color(rgb(0x217346))
                    .text_color(gpui::black())
                    .text_size(px(14.0))
                    .whitespace_nowrap()
                    .child(text.text.clone())
                    .child(div().w(px(1.0)).h(px(15.0)).bg(gpui::black())),
            )
        });

        let tabs: Vec<_> = sheets
            .iter()
            .enumerate()
            .map(|(index, name)| {
                div()
                    .id(("sheet", index))
                    .px_4()
                    .h_full()
                    .flex()
                    .items_center()
                    .text_xs()
                    .cursor_pointer()
                    .text_color(if index == current {
                        rgb(ACCENT)
                    } else {
                        rgb(DIM)
                    })
                    .child(name.clone())
                    .on_click(cx.listener(move |viewer, _event, _window, cx| {
                        viewer.commit_cell(cx);
                        if let Document::Workbook {
                            book,
                            anchor,
                            focus,
                            scroll,
                            painted,
                            ..
                        } = &mut viewer.document
                        {
                            if book.select_sheet(index).is_ok() {
                                *anchor = CellRef::new(0, 0);
                                *focus = *anchor;
                                *painted = None;
                                scroll.set_offset(point(px(0.0), px(0.0)));
                            }
                        }
                        cx.notify();
                    }))
            })
            .collect();

        let store = Rc::clone(&self.surface);
        let entity = cx.entity();
        div()
            .flex_1()
            .flex()
            .flex_col()
            .min_h_0()
            .child(
                div()
                    .h(px(FORMULA_BAR_HEIGHT))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .bg(rgb(PANEL))
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .text_xs()
                    .child(
                        div()
                            .w(px(90.0))
                            .px_2()
                            .border_1()
                            .border_color(rgb(BORDER))
                            .child(name_box),
                    )
                    .child(div().text_color(rgb(DIM)).child("fx"))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .child(content)
                            .when(editing, |bar| {
                                bar.child(div().w(px(1.0)).h(px(13.0)).bg(rgb(ACCENT)))
                            }),
                    ),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .overflow_hidden()
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .children(picture.map(|image| img(image).w(px(size.0)).h(px(size.1)))),
                    )
                    .child(
                        div()
                            .id("sheet-scroll")
                            .absolute()
                            .inset_0()
                            .overflow_scroll()
                            .track_scroll(scroll)
                            .on_mouse_down(MouseButton::Left, cx.listener(Self::sheet_mouse_down))
                            .child(
                                div()
                                    .w(px(sheet_width as f32))
                                    .h(px(sheet_height as f32))
                                    .flex_none(),
                            ),
                    )
                    .children(cell_editor)
                    // Records where the grid is, and registers the input
                    // handler so typing — an IME's included — reaches the
                    // cell.
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
                            },
                        )
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full(),
                    ),
            )
            .child(
                div()
                    .h(px(TABS_HEIGHT))
                    .flex_none()
                    .flex()
                    .bg(rgb(PANEL))
                    .border_t_1()
                    .border_color(rgb(BORDER))
                    .children(tabs),
            )
    }

    // ---- the deck -------------------------------------------------------

    /// Writes the text being typed into a shape, if any.
    fn commit_slide_text(&mut self, cx: &mut Context<Self>) {
        let Document::Deck {
            deck,
            edit,
            current,
            ..
        } = &mut self.document
        else {
            return;
        };
        let Some((id, text)) = edit.take() else {
            return;
        };
        let result = deck.set_shape_text(*current, id, &text.text);
        self.edited(result, cx);
    }

    fn begin_slide_text(&mut self, cx: &mut Context<Self>) {
        let Document::Deck {
            deck,
            edit,
            current,
            selected,
            ..
        } = &mut self.document
        else {
            return;
        };
        let Some(id) = *selected else {
            return;
        };
        match deck.shape_text(*current, id) {
            Some(text) => *edit = Some((id, TextEdit::new(text))),
            None => self.status = SharedString::from("この図形の文字は編集できません"),
        }
        cx.notify();
    }

    fn deck_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let modifiers = event.keystroke.modifiers;
        let key = event.keystroke.key.as_str();
        let Document::Deck {
            deck,
            current,
            selected,
            edit,
            ..
        } = &mut self.document
        else {
            return false;
        };
        if let Some((_, text)) = edit.as_mut() {
            if text.marked.is_some() {
                return false;
            }
            match key {
                // As in PowerPoint, Escape leaves the text, keeping it.
                "escape" => self.commit_slide_text(cx),
                "enter" => {
                    text.text.push('\n');
                    cx.notify();
                }
                "backspace" => {
                    text.text.pop();
                    cx.notify();
                }
                _ => return false,
            }
            return true;
        }
        let slide = *current;
        if let Some(id) = *selected {
            let step = if modifiers.alt { 1.0 } else { NUDGE_PX };
            let result = match key {
                "left" => deck.move_shape(slide, id, -step, 0.0),
                "right" => deck.move_shape(slide, id, step, 0.0),
                "up" => deck.move_shape(slide, id, 0.0, -step),
                "down" => deck.move_shape(slide, id, 0.0, step),
                "delete" | "backspace" => {
                    *selected = None;
                    deck.delete_shape(slide, id)
                }
                "enter" | "f2" => {
                    self.begin_slide_text(cx);
                    return true;
                }
                "escape" => {
                    *selected = None;
                    cx.notify();
                    return true;
                }
                "tab" => {
                    *selected = deck.next_shape(slide, Some(id), modifiers.shift);
                    cx.notify();
                    return true;
                }
                _ => return false,
            };
            self.edited(result, cx);
            return true;
        }
        let last = deck.slide_count().saturating_sub(1);
        match key {
            "down" | "right" | "pagedown" => *current = (*current + 1).min(last),
            "up" | "left" | "pageup" => *current = current.saturating_sub(1),
            "tab" => *selected = deck.next_shape(slide, None, modifiers.shift),
            _ => return false,
        }
        cx.notify();
        true
    }

    fn slide_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        cx.stop_propagation();
        let Some(bounds) = self.surface.get() else {
            return;
        };
        let Document::Deck {
            deck,
            current,
            edit,
            scale,
            ..
        } = &mut self.document
        else {
            return;
        };
        let to_slide = |value: Pixels| f64::from(value) / f64::from(*scale);
        let x = to_slide(event.position.x - bounds.origin.x);
        let y = to_slide(event.position.y - bounds.origin.y);
        // A click inside the shape being typed into stays in the text.
        if let Some((id, _)) = edit {
            let inside = deck
                .shape_rect(*current, *id)
                .is_some_and(|rect| rect.contains(x, y));
            if inside {
                return;
            }
        }
        let hit = deck.shape_at(*current, x, y);
        self.commit_slide_text(cx);
        if let Document::Deck { selected, .. } = &mut self.document {
            *selected = hit;
        }
        if event.click_count >= 2 && hit.is_some() {
            self.begin_slide_text(cx);
        }
        cx.notify();
    }

    fn deck_body(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui::Div {
        let generation = self.generation;
        let Document::Deck {
            deck,
            current,
            thumbnails,
            slide,
            selected,
            edit,
            scale: shown_scale,
        } = &mut self.document
        else {
            unreachable!()
        };
        let scale = window.scale_factor();
        for (index, thumbnail) in thumbnails.iter_mut().enumerate() {
            if thumbnail.is_none() {
                *thumbnail = deck
                    .render_slide(index, (THUMBNAIL_WIDTH * scale) as u32)
                    .and_then(image);
            }
        }
        let viewport = window.viewport_size();
        let (slide_width, slide_height) = deck.slide_size();
        let available = (
            f32::from(viewport.width) - SIDEBAR_WIDTH - 48.0,
            f32::from(viewport.height) - HEADER_HEIGHT - 48.0,
        );
        let fit = (available.0 / slide_width as f32).min(available.1 / slide_height as f32);
        *shown_scale = fit;
        let (width, height) = (slide_width as f32 * fit, slide_height as f32 * fit);
        let device_width = (width * scale).round() as u32;
        let fresh = matches!(slide, Some((index, w, g, _))
            if *index == *current && *w == device_width && *g == generation);
        if !fresh {
            if let Some(painted) = deck.render_slide(*current, device_width).and_then(image) {
                if let Some((_, _, _, old)) =
                    slide.replace((*current, device_width, generation, painted))
                {
                    self.retired.push(old);
                }
            }
        }
        let current_index = *current;
        let thumbnail_height = THUMBNAIL_WIDTH * (slide_height / slide_width) as f32;
        let rows: Vec<_> = thumbnails
            .iter()
            .enumerate()
            .map(|(index, thumbnail)| {
                div()
                    .id(("thumbnail", index))
                    .w(px(THUMBNAIL_WIDTH))
                    .h(px(thumbnail_height))
                    .flex_none()
                    .bg(gpui::white())
                    .border_2()
                    .border_color(if index == current_index {
                        rgb(ACCENT)
                    } else {
                        rgb(BORDER)
                    })
                    .cursor_pointer()
                    .children(thumbnail.clone().map(|image| img(image).size_full()))
                    .on_click(cx.listener(move |viewer, _event, _window, cx| {
                        viewer.commit_slide_text(cx);
                        if let Document::Deck {
                            current, selected, ..
                        } = &mut viewer.document
                        {
                            *current = index;
                            *selected = None;
                        }
                        cx.notify();
                    }))
            })
            .collect();
        let slide_image = slide.as_ref().map(|(_, _, _, image)| image.clone());

        // The selection outline, or the text editor, over the slide.
        let overlay = selected.and_then(|id| {
            let rect = deck.shape_rect(current_index, id)?;
            let (x, y, w, h) = (rect.x, rect.y, rect.width, rect.height);
            let (x, y, w, h) = (
                x as f32 * fit,
                y as f32 * fit,
                w as f32 * fit,
                h as f32 * fit,
            );
            Some(match edit.as_ref().filter(|(edited, _)| *edited == id) {
                Some((_, text)) => div()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .min_w(px(w))
                    .min_h(px(h))
                    .p(px(6.0))
                    .bg(gpui::rgba(0xffffffee))
                    .border_1()
                    .border_color(rgb(ACCENT))
                    .text_color(gpui::black())
                    .text_size(px((18.0 * fit).max(11.0)))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_end()
                            .child(text.text.clone())
                            .child(
                                div()
                                    .w(px(1.5))
                                    .h(px((20.0 * fit).max(12.0)))
                                    .bg(gpui::black()),
                            ),
                    ),
                None => div()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(w))
                    .h(px(h))
                    .border_1()
                    .border_color(rgb(ACCENT)),
            })
        });

        let store = Rc::clone(&self.surface);
        let entity = cx.entity();
        div()
            .flex_1()
            .flex()
            .min_h_0()
            .child(
                div()
                    .id("thumbnails")
                    .w(px(SIDEBAR_WIDTH))
                    .flex_none()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .py_2()
                    .bg(rgb(0x0a0a0c))
                    .border_r_1()
                    .border_color(rgb(BORDER))
                    .children(rows),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(0x141416))
                    // A click off the slide clears the selection.
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|viewer, _event, window, cx| {
                            window.focus(&viewer.focus, cx);
                            viewer.commit_slide_text(cx);
                            if let Document::Deck { selected, .. } = &mut viewer.document {
                                *selected = None;
                            }
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .relative()
                            .w(px(width))
                            .h(px(height))
                            .bg(gpui::white())
                            .shadow_lg()
                            .on_mouse_down(MouseButton::Left, cx.listener(Self::slide_mouse_down))
                            .children(
                                slide_image.map(|image| img(image).w(px(width)).h(px(height))),
                            )
                            .children(overlay)
                            .child(
                                canvas(
                                    move |bounds, _window, _cx| {
                                        store.set(Some(bounds));
                                        bounds
                                    },
                                    move |_bounds, slide, window, cx| {
                                        let focus = entity.read(cx).focus.clone();
                                        window.handle_input(
                                            &focus,
                                            ElementInputHandler::new(slide, entity.clone()),
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
            )
    }

    // ---- pages ----------------------------------------------------------

    fn pages_body(&self) -> gpui::Div {
        let Document::Pages { pages, page_size } = &self.document else {
            unreachable!()
        };
        let (width, height) = *page_size;
        div().flex_1().min_h_0().bg(rgb(0x141416)).child(
            div()
                .id("pages")
                .size_full()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .items_center()
                .gap_6()
                .py_6()
                .children(pages.iter().map(|page| {
                    div()
                        .flex_none()
                        .w(px(width))
                        .h(px(height))
                        .bg(gpui::white())
                        .shadow_lg()
                        .child(img(page.clone()).w(px(width)).h(px(height)))
                })),
        )
    }

    fn header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let editable = !matches!(self.document, Document::Pages { .. });
        let dirty = self.is_dirty();
        let button = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .px_2()
                .py_0p5()
                .rounded_md()
                .text_xs()
                .cursor_pointer()
                .border_1()
                .border_color(rgb(BORDER))
                .hover(|button| button.bg(rgb(0x18181b)))
                .child(label)
        };
        div()
            .h(px(HEADER_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .bg(rgb(PANEL))
            .border_b_1()
            .border_color(rgb(BORDER))
            .child(div().size(px(20.0)).rounded_md().bg(gpui::linear_gradient(
                135.0,
                gpui::linear_color_stop(rgb(0x6366f1), 0.0),
                gpui::linear_color_stop(rgb(0xec4899), 1.0),
            )))
            .child(div().text_sm().child("jade"))
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(DIM))
                    .child(self.name.clone()),
            )
            .when(dirty, |header| {
                header.child(div().size(px(7.0)).rounded_full().bg(rgb(DIM)))
            })
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(ACCENT))
                    .child(self.status.clone()),
            )
            .child(div().flex_1())
            .when(editable, |header| {
                header
                    .child(button("undo", "元に戻す").on_click(
                        cx.listener(|viewer, _event, _window, cx| viewer.step_history(true, cx)),
                    ))
                    .child(button("redo", "やり直し").on_click(
                        cx.listener(|viewer, _event, _window, cx| viewer.step_history(false, cx)),
                    ))
                    .child(
                        button("save", "保存 (ダウンロード)")
                            .on_click(cx.listener(|viewer, _event, _window, cx| viewer.save(cx))),
                    )
            })
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x71717a))
                    .child("gpui-pre web · WebGPU · tiny-skia"),
            )
    }

    fn handle_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
        let key = event.keystroke.key.as_str();
        // ⌘ on a Mac, Ctrl elsewhere.
        if modifiers.platform || modifiers.control {
            match key {
                "z" => self.step_history(!modifiers.shift, cx),
                "y" => self.step_history(false, cx),
                "s" => self.save(cx),
                _ => return,
            }
            cx.stop_propagation();
            return;
        }
        let handled = match self.document {
            Document::Workbook { .. } => self.sheet_key(event, cx),
            Document::Deck { .. } => self.deck_key(event, cx),
            Document::Pages { .. } => false,
        };
        if handled {
            cx.stop_propagation();
        }
    }

    /// The text being typed into, starting an edit when typing begins one:
    /// into the selected cell (replacing it, as in Excel) or shape.
    fn typing_target(&mut self, cx: &mut Context<Self>) -> Option<&mut TextEdit> {
        match &self.document {
            Document::Workbook { edit: None, .. } => self.begin_cell_edit(Some(String::new()), cx),
            Document::Deck {
                edit: None,
                selected: Some(_),
                ..
            } => self.begin_slide_text(cx),
            _ => {}
        }
        match &mut self.document {
            Document::Workbook { edit, .. } => edit.as_mut(),
            Document::Deck { edit, .. } => edit.as_mut().map(|(_, text)| text),
            Document::Pages { .. } => None,
        }
    }

    fn current_text(&self) -> Option<&TextEdit> {
        match &self.document {
            Document::Workbook { edit, .. } => edit.as_ref(),
            Document::Deck { edit, .. } => edit.as_ref().map(|(_, text)| text),
            Document::Pages { .. } => None,
        }
    }
}

impl EntityInputHandler for Viewer {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let text = self
            .current_text()
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
            .current_text()
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
        let edit = self.current_text()?;
        let marked = edit.marked.clone()?;
        Some(byte_to_utf16(&edit.text, marked.start)..byte_to_utf16(&edit.text, marked.end))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        match &mut self.document {
            Document::Workbook {
                edit: Some(edit), ..
            } => edit.marked = None,
            Document::Deck {
                edit: Some((_, edit)),
                ..
            } => edit.marked = None,
            _ => {}
        }
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(edit) = self.typing_target(cx) {
            edit.replace(range, text, false);
            cx.notify();
        }
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(edit) = self.typing_target(cx) {
            edit.replace(range, new_text, true);
            cx.notify();
        }
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        // The candidate window goes at the editing surface's top-left: never
        // off-screen, if not beside the caret yet.
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

impl gpui::Focusable for Viewer {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Viewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for image in self.retired.drain(..) {
            window.drop_image(image).ok();
        }
        let body = match self.document {
            Document::Pages { .. } => self.pages_body(),
            Document::Deck { .. } => self.deck_body(window, cx),
            Document::Workbook { .. } => self.workbook_body(window, cx),
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x09090b))
            .text_color(rgb(TEXT))
            .font_family(UI_FONT)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::handle_key))
            .child(self.header(cx))
            .child(body)
    }
}

/// Opens `file` (a .docx, .pptx or .xlsx) in a gpui window on the page.
/// `scale` is the page's `devicePixelRatio`, which Word pages are painted at;
/// `time_zone` is the browser's, which formulas calculate in; `on_save` is
/// called with the file name and the saved bytes.
#[wasm_bindgen]
pub fn run(
    file_name: String,
    file: Vec<u8>,
    regular: Vec<u8>,
    bold: Vec<u8>,
    scale: f64,
    time_zone: String,
    on_save: js_sys::Function,
) -> Result<(), JsError> {
    console_error_panic_hook::set_once();
    let backend =
        jade_web::backend(regular.clone(), Some(bold.clone())).map_err(|e| JsError::new(&e))?;
    let lower = file_name.to_lowercase();
    let document = if lower.ends_with(".docx") {
        let measurer =
            jade_web::document::measurer(&regular, Some(&bold)).map_err(|e| JsError::new(&e))?;
        let opened = jade_web::document::Document::open(&file, scale, backend.fonts(), measurer)
            .map_err(|e| JsError::new(&e))?;
        let (width, height) = opened.page_size();
        Document::Pages {
            pages: (0..opened.page_count())
                .filter_map(|index| opened.page(index).and_then(image))
                .collect(),
            page_size: (width as f32, height as f32),
        }
    } else if lower.ends_with(".pptx") {
        let deck = jade_web::Deck::open(&file, backend.clone()).map_err(|e| JsError::new(&e))?;
        Document::Deck {
            thumbnails: vec![None; deck.slide_count()],
            deck,
            current: 0,
            slide: None,
            selected: None,
            edit: None,
            scale: 1.0,
        }
    } else {
        let book =
            jade_web::Workbook::open(&file, &time_zone, backend).map_err(|e| JsError::new(&e))?;
        Document::Workbook {
            sheets: book
                .sheet_names()
                .into_iter()
                .map(SharedString::from)
                .collect(),
            book,
            anchor: CellRef::new(0, 0),
            focus: CellRef::new(0, 0),
            edit: None,
            scroll: ScrollHandle::new(),
            painted: None,
            viewport: (1.0, 1.0),
        }
    };

    // Single-threaded, and CJK the bundled face lacks falls back to the
    // browser's own fonts through Canvas2D.
    let platform = WebPlatform::new_with_backend_and_font_fallback(
        false,
        WebBackendPreference::Auto,
        CanvasFontFallback::EmojiAndCjk,
    );
    let handle = Application::with_platform(Rc::new(platform)).run_embedded(move |cx: &mut App| {
        cx.text_system().add_fonts(vec![Cow::Owned(regular)]).ok();
        let window = cx.open_window(WindowOptions::default(), |_window, cx| {
            cx.new(|cx| Viewer {
                name: SharedString::from(file_name),
                document,
                retired: Vec::new(),
                generation: 0,
                status: SharedString::from(""),
                surface: Rc::new(Cell::new(None)),
                focus: cx.focus_handle(),
                on_save,
            })
        });
        if let Ok(window) = window {
            window
                .update(cx, |viewer, window, cx| window.focus(&viewer.focus, cx))
                .ok();
        }
    });
    // The page's event loop drives the app from here; it lives as long as
    // the page does.
    std::mem::forget(handle);
    Ok(())
}

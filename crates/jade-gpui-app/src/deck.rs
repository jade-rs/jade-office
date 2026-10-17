//! The presentation (PPTX) view.
//!
//! The third view beside the document and the workbook, laid out the way
//! PowerPoint's Normal view is: slide thumbnails down the left, the selected
//! slide as large as the space allows, its speaker notes underneath. A slide
//! show opens its own full-screen window and shows one slide at a time.
//!
//! ## Why a slide is repainted at the size it is shown
//!
//! A slide is a fixed-size canvas, so unlike a sheet it could be painted once
//! and scaled. Scaling a bitmap blurs text, though, and a slide is mostly text
//! at sizes where that shows. The layout is cached by the deck, so painting
//! again at a new width is only the Skia pass.

use gpui::{
    div, img, prelude::*, px, svg, App, Bounds, Context, Entity, FocusHandle, Focusable,
    KeyDownEvent, MouseButton, RenderImage, ScrollHandle, SharedString, Window, WindowBounds,
    WindowOptions,
};
use jade_pptx_native_render::{aliased_faces, bundled_font_paths, NativeDeck};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use crate::editor::{self, Editor};
use crate::{i18n, native_print, theme, workspace, Print, StartSlideshow, UI_FONT_FAMILY};

/// The thumbnail strip's width, and the width each thumbnail is shown at.
const SIDEBAR_WIDTH: f32 = 200.0;
const THUMBNAIL_WIDTH: f32 = 152.0;
/// The bar above the slide: slide number, print, slide show.
const TOOLBAR_HEIGHT: f32 = 40.0;
/// The speaker-notes pane under the slide.
const NOTES_HEIGHT: f32 = 110.0;
/// Space left round the slide inside its canvas.
const SLIDE_MARGIN: f32 = 28.0;

/// A painted slide and what it was painted for.
struct Painted {
    index: usize,
    device_width: i32,
    image: Arc<RenderImage>,
}

/// Paints slide `index` of `deck` `device_width` px wide, for gpui.
fn slide_image(deck: &NativeDeck, index: usize, device_width: i32) -> Option<Arc<RenderImage>> {
    let (bgra, width, height) = deck.slide_pixels(index, device_width)?;
    let buffer = image::ImageBuffer::from_raw(width as u32, height as u32, bgra)?;
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
}

/// The slide's on-screen size to fit `available`, keeping its aspect.
fn fitted(deck: &NativeDeck, available: (f32, f32)) -> (f32, f32) {
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
    deck: &NativeDeck,
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

pub struct DeckView {
    deck: Rc<NativeDeck>,
    current: usize,
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
    /// The scale the slide was last shown at, which is what the status bar
    /// shows while fitting.
    shown_scale: f32,
    thumbnail_scroll: ScrollHandle,
    focus: FocusHandle,
    status: SharedString,
}

impl DeckView {
    fn go_to(&mut self, index: usize, cx: &mut Context<Self>) {
        let index = index.min(self.deck.slide_count().saturating_sub(1));
        if index != self.current {
            self.current = index;
            self.thumbnail_scroll.scroll_to_item(index);
            cx.notify();
        }
    }

    fn step(&mut self, delta: isize, cx: &mut Context<Self>) {
        self.go_to(self.current.saturating_add_signed(delta), cx);
    }

    fn start_slideshow(&mut self, from: usize, cx: &mut Context<Self>) {
        if let Err(error) = open_slideshow(self.deck.clone(), from, cx) {
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

    fn handle_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        if keystroke.modifiers.platform || keystroke.modifiers.control {
            return;
        }
        match keystroke.key.as_str() {
            "down" | "right" | "pagedown" | "space" => self.step(1, cx),
            "up" | "left" | "pageup" => self.step(-1, cx),
            "home" => self.go_to(0, cx),
            "end" => self.go_to(usize::MAX, cx),
            _ => return,
        }
        cx.stop_propagation();
    }

    fn paint_thumbnails_if_needed(&mut self, scale_factor: f32) {
        let device_width = (THUMBNAIL_WIDTH * scale_factor).round() as i32;
        for index in 0..self.thumbnails.len() {
            if self.thumbnails[index].is_none() {
                self.thumbnails[index] = slide_image(&self.deck, index, device_width);
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
        vec![i18n::format(
            "deck.position",
            &[
                ("current", &(self.current + 1)),
                ("count", &self.deck.slide_count()),
            ],
        )
        .into()]
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
            &self.deck,
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
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme::accent())
                                    .child(self.status.clone()),
                            )
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
                        // scroll.
                        div()
                            .id("slide-canvas")
                            .flex_1()
                            .min_h_0()
                            .overflow_scroll()
                            .bg(theme::canvas())
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
                                            .flex_none()
                                            .w(px(width))
                                            .h(px(height))
                                            .bg(gpui::white())
                                            .shadow_lg()
                                            .children(self.slide.as_ref().map(|painted| {
                                                img(painted.image.clone())
                                                    .w(px(width))
                                                    .h(px(height))
                                            })),
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

/// Opens `path` as a presentation view, for the workspace to show in a tab.
pub fn new_view(path: &Path, cx: &mut App) -> Result<Entity<DeckView>, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let (regular, bold) = bundled_font_paths(&core);
    let deck = NativeDeck::open(&bytes, &aliased_faces(&regular, &bold))?;
    if deck.slide_count() == 0 {
        return Err(i18n::format(
            "deck.empty",
            &[("name", &crate::shell::file_name(path))],
        ));
    }
    Ok(cx.new(|cx| DeckView {
        thumbnails: vec![None; deck.slide_count()],
        deck: Rc::new(deck),
        current: 0,
        slide: None,
        retired: Vec::new(),
        show_notes: true,
        zoom: None,
        shown_scale: 1.0,
        thumbnail_scroll: ScrollHandle::new(),
        focus: cx.focus_handle(),
        status: SharedString::from(""),
    }))
}

/// A slide show: one slide at a time, full screen, on black.
///
/// Its own window rather than a mode of the deck view, so the workspace —
/// explorer, tabs, status bar — stays as it was behind it, and ending the show
/// is closing a window.
struct SlideshowView {
    deck: Rc<NativeDeck>,
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
            &self.deck,
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

fn open_slideshow(deck: Rc<NativeDeck>, from: usize, cx: &mut App) -> Result<(), String> {
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

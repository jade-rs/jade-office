//! jade in the browser, through gpui.
//!
//! The page fetches the fonts and a document and calls [`run`]. From there it
//! is a gpui application like the native one: gpui-pre's web platform owns a
//! canvas and draws with WebGPU (or WebGL2), and the document's pixels come
//! from `jade-web` — the engine's renderers on the pure-Rust painter — as
//! `RenderImage`s, the same way the native views take Skia's.

#![cfg(target_arch = "wasm32")]

use std::borrow::Cow;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    div, img, point, prelude::*, px, rgb, App, Application, Context, RenderImage, ScrollHandle,
    SharedString, Window, WindowOptions,
};
use gpui_web::{CanvasFontFallback, WebBackendPreference, WebPlatform};
use jade_tiny_skia_painter::TinySkiaPainter;
use wasm_bindgen::prelude::*;

const UI_FONT: &str = "Noto Sans JP";
const THUMBNAIL_WIDTH: f32 = 150.0;
const SIDEBAR_WIDTH: f32 = 186.0;
const HEADER_HEIGHT: f32 = 40.0;
const TABS_HEIGHT: f32 = 30.0;

fn image(painter: TinySkiaPainter) -> Option<Arc<RenderImage>> {
    let (width, height) = (painter.width(), painter.height());
    let buffer = image::ImageBuffer::from_raw(width, height, painter.bgra_premultiplied())?;
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
}

/// A sheet viewport's pixels, with the scroll position and device size they
/// were painted for.
type PaintedViewport = ((f32, f32), (u32, u32), Arc<RenderImage>);

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
        slide: Option<(usize, u32, Arc<RenderImage>)>,
    },
    Workbook {
        book: jade_web::Workbook,
        sheets: Vec<SharedString>,
        current: usize,
        scroll: ScrollHandle,
        painted: Option<PaintedViewport>,
    },
}

struct Viewer {
    name: SharedString,
    document: Document,
    retired: Vec<Arc<RenderImage>>,
}

impl Viewer {
    fn header(&self) -> impl IntoElement {
        div()
            .h(px(HEADER_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .bg(rgb(0x0d0d0f))
            .border_b_1()
            .border_color(rgb(0x27272a))
            .child(div().size(px(20.0)).rounded_md().bg(gpui::linear_gradient(
                135.0,
                gpui::linear_color_stop(rgb(0x6366f1), 0.0),
                gpui::linear_color_stop(rgb(0xec4899), 1.0),
            )))
            .child(div().text_sm().child("jade"))
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0xa1a1aa))
                    .child(self.name.clone()),
            )
            .child(div().flex_1())
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x71717a))
                    .child("gpui-pre web · WebGPU · tiny-skia"),
            )
    }

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

    fn deck_body(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui::Div {
        let Document::Deck {
            deck,
            current,
            thumbnails,
            slide,
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
        let (width, height) = (slide_width as f32 * fit, slide_height as f32 * fit);
        let device_width = (width * scale).round() as u32;
        if !matches!(slide, Some((index, w, _)) if *index == *current && *w == device_width) {
            if let Some(painted) = deck.render_slide(*current, device_width).and_then(image) {
                if let Some((_, _, old)) = slide.replace((*current, device_width, painted)) {
                    self.retired.push(old);
                }
            }
        }
        let current = *current;
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
                    .border_color(if index == current {
                        rgb(0x818cf8)
                    } else {
                        rgb(0x27272a)
                    })
                    .cursor_pointer()
                    .children(thumbnail.clone().map(|image| img(image).size_full()))
                    .on_click(cx.listener(move |viewer, _event, _window, cx| {
                        if let Document::Deck { current, .. } = &mut viewer.document {
                            *current = index;
                        }
                        cx.notify();
                    }))
            })
            .collect();
        let slide_image = slide.as_ref().map(|(_, _, image)| image.clone());
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
                    .border_color(rgb(0x27272a))
                    .children(rows),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(0x141416))
                    .child(
                        div()
                            .w(px(width))
                            .h(px(height))
                            .bg(gpui::white())
                            .shadow_lg()
                            .children(
                                slide_image.map(|image| img(image).w(px(width)).h(px(height))),
                            ),
                    ),
            )
    }

    fn workbook_body(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui::Div {
        let Document::Workbook {
            book,
            sheets,
            current,
            scroll,
            painted,
        } = &mut self.document
        else {
            unreachable!()
        };
        let viewport = window.viewport_size();
        let scale = window.scale_factor();
        let size = (
            f32::from(viewport.width).max(1.0),
            (f32::from(viewport.height) - HEADER_HEIGHT - TABS_HEIGHT).max(1.0),
        );
        let offset = scroll.offset();
        let at = (-f32::from(offset.x), -f32::from(offset.y));
        let device = ((size.0 * scale) as u32, (size.1 * scale) as u32);
        if !matches!(painted, Some((p, d, _)) if *p == at && *d == device) {
            if let Some(image) = book
                .render_viewport(
                    (f64::from(at.0), f64::from(at.1)),
                    device.0,
                    device.1,
                    f64::from(scale),
                )
                .and_then(image)
            {
                if let Some((_, _, old)) = painted.replace((at, device, image)) {
                    self.retired.push(old);
                }
            }
        }
        let (sheet_width, sheet_height) = book.sheet_size();
        let image = painted.as_ref().map(|(_, _, image)| image.clone());
        let current = *current;
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
                        rgb(0x818cf8)
                    } else {
                        rgb(0xa1a1aa)
                    })
                    .child(name.clone())
                    .on_click(cx.listener(move |viewer, _event, _window, cx| {
                        if let Document::Workbook {
                            book,
                            current,
                            scroll,
                            painted,
                            ..
                        } = &mut viewer.document
                        {
                            if book.select_sheet(index).is_ok() {
                                *current = index;
                                *painted = None;
                                scroll.set_offset(point(px(0.0), px(0.0)));
                            }
                        }
                        cx.notify();
                    }))
            })
            .collect();
        div()
            .flex_1()
            .flex()
            .flex_col()
            .min_h_0()
            .child(
                div()
                    .relative()
                    .flex_1()
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .children(image.map(|image| img(image).w(px(size.0)).h(px(size.1)))),
                    )
                    .child(
                        div()
                            .id("sheet-scroll")
                            .absolute()
                            .inset_0()
                            .overflow_scroll()
                            .track_scroll(scroll)
                            .child(
                                div()
                                    .w(px(sheet_width as f32))
                                    .h(px(sheet_height as f32))
                                    .flex_none(),
                            ),
                    ),
            )
            .child(
                div()
                    .h(px(TABS_HEIGHT))
                    .flex_none()
                    .flex()
                    .bg(rgb(0x0d0d0f))
                    .border_t_1()
                    .border_color(rgb(0x27272a))
                    .children(tabs),
            )
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
            .text_color(rgb(0xe4e4e7))
            .font_family(UI_FONT)
            .child(self.header())
            .child(body)
    }
}

/// Opens `file` (a .docx, .pptx or .xlsx) in a gpui window on the page.
/// `scale` is the page's `devicePixelRatio`, which Word pages are painted at.
#[wasm_bindgen]
pub fn run(
    file_name: String,
    file: Vec<u8>,
    regular: Vec<u8>,
    bold: Vec<u8>,
    scale: f64,
) -> Result<(), JsError> {
    console_error_panic_hook::set_once();
    let fonts =
        jade_web::font_book(regular.clone(), Some(bold.clone())).map_err(|e| JsError::new(&e))?;
    let lower = file_name.to_lowercase();
    let document = if lower.ends_with(".docx") {
        let measurer =
            jade_web::document::measurer(&regular, Some(&bold)).map_err(|e| JsError::new(&e))?;
        let opened = jade_web::document::Document::open(&file, scale, Rc::clone(&fonts), measurer)
            .map_err(|e| JsError::new(&e))?;
        let (width, height) = opened.page_size();
        Document::Pages {
            pages: (0..opened.page_count())
                .filter_map(|index| opened.page(index).and_then(image))
                .collect(),
            page_size: (width as f32, height as f32),
        }
    } else if lower.ends_with(".pptx") {
        let deck = jade_web::Deck::open(&file, Rc::clone(&fonts)).map_err(|e| JsError::new(&e))?;
        Document::Deck {
            thumbnails: vec![None; deck.slide_count()],
            deck,
            current: 0,
            slide: None,
        }
    } else {
        let book = jade_web::Workbook::open(&file, fonts).map_err(|e| JsError::new(&e))?;
        Document::Workbook {
            sheets: book
                .sheet_names()
                .into_iter()
                .map(SharedString::from)
                .collect(),
            book,
            current: 0,
            scroll: ScrollHandle::new(),
            painted: None,
        }
    };
    let viewer = Viewer {
        name: SharedString::from(file_name),
        document,
        retired: Vec::new(),
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
        cx.open_window(WindowOptions::default(), |_window, cx| cx.new(|_| viewer))
            .ok();
    });
    // The page's event loop drives the app from here; it lives as long as
    // the page does.
    std::mem::forget(handle);
    Ok(())
}

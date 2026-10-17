//! A native jade window for spreadsheets.
//!
//! The counterpart of the DOCX window, and deliberately not the same view: a
//! workbook has no pages to scroll and nothing Word's ribbon would apply to.
//! What it has is one sheet at a time, a scroll position, and tabs.
//!
//! ## Why this paints a viewport rather than pages
//!
//! The DOCX view rasterises every page once and lets gpui scroll finished
//! images. A sheet cannot work that way: the fixture in this repository is
//! 125,000 px tall, and a real workbook is worse. So the surface stays the size
//! of the window and each frame paints the part of the sheet under the scroll
//! position — the same thing the browser shell does, through the same
//! `render_viewport`.

use gpui::{
    div, img, point, prelude::*, px, size, App, Application, Bounds, Context, FocusHandle,
    Focusable, RenderImage, ScrollHandle, SharedString, TitlebarOptions, Window,
    WindowBackgroundAppearance, WindowBounds, WindowOptions,
};
use jade_xlsx_native_render::{aliased_faces, bundled_font_paths, NativeWorkbook};
use std::path::PathBuf;
use std::sync::Arc;

use crate::theme;

/// Chrome above the sheet: the header row and the sheet tabs.
const CHROME_HEIGHT: f32 = 92.0;
/// Padding around the sheet area, matched by the element below.
const CANVAS_INSET: f32 = 12.0;

/// What a painted image was painted for. A frame that matches it needs no new
/// pixels, which is what keeps scrolling from re-rendering the same view.
#[derive(Clone, Copy, PartialEq)]
struct PaintedFor {
    sheet: usize,
    scroll: (f32, f32),
    size: (i32, i32),
}

pub struct WorkbookView {
    title: SharedString,
    workbook: NativeWorkbook,
    sheet_names: Vec<SharedString>,
    image: Option<Arc<RenderImage>>,
    /// Textures replaced since the last frame. gpui wants them released with a
    /// window to hand, which only `render` has.
    retired: Vec<Arc<RenderImage>>,
    painted: Option<PaintedFor>,
    scroll: ScrollHandle,
    focus: FocusHandle,
    status: SharedString,
}

impl WorkbookView {
    fn select_sheet(&mut self, index: usize, cx: &mut Context<Self>) {
        match self.workbook.select_sheet(index) {
            Ok(()) => {
                self.scroll.set_offset(point(px(0.0), px(0.0)));
                // The old image belongs to the old sheet; forcing a repaint is
                // what stops the previous sheet showing under the new tab.
                self.painted = None;
                self.status = SharedString::from("");
            }
            Err(error) => self.status = SharedString::from(error),
        }
        cx.notify();
    }

    /// Repaints when the scroll position, the window size or the sheet has
    /// moved since the image was made.
    fn repaint_if_needed(&mut self, viewport: (i32, i32)) {
        let offset = self.scroll.offset();
        let wanted = PaintedFor {
            sheet: self.workbook.sheet_index(),
            // The scroll offset is negative going down; the sheet's coordinates
            // are not.
            scroll: (-f32::from(offset.x), -f32::from(offset.y)),
            size: viewport,
        };
        if self.painted == Some(wanted) && self.image.is_some() {
            return;
        }

        let pixels = self.workbook.viewport_pixels(
            (f64::from(wanted.scroll.0), f64::from(wanted.scroll.1)),
            viewport,
        );
        let Some((bgra, width, height)) = pixels else {
            self.status = SharedString::from("cannot paint the sheet");
            return;
        };
        let Some(buffer) = image::ImageBuffer::from_raw(width as u32, height as u32, bgra) else {
            self.status = SharedString::from("the surface was the wrong size");
            return;
        };
        if let Some(previous) = self
            .image
            .replace(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
        {
            self.retired.push(previous);
        }
        self.painted = Some(wanted);
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

        let window_size = window.viewport_size();
        let viewport = (
            (f32::from(window_size.width) - CANVAS_INSET * 2.0).max(1.0) as i32,
            (f32::from(window_size.height) - CHROME_HEIGHT - CANVAS_INSET).max(1.0) as i32,
        );
        self.repaint_if_needed(viewport);

        let (sheet_width, sheet_height) = self.workbook.sheet_size();
        let current = self.workbook.sheet_index();
        let tabs: Vec<_> = self
            .sheet_names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let selected = index == current;
                div()
                    .id(("sheet-tab", index))
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .text_sm()
                    .cursor_pointer()
                    .text_color(if selected {
                        theme::text()
                    } else {
                        theme::text_dim()
                    })
                    .when(selected, |tab| tab.bg(theme::selected()))
                    .hover(|tab| tab.bg(theme::hovered()))
                    .child(name.clone())
                    .on_click(
                        cx.listener(move |view, _event, _window, cx| view.select_sheet(index, cx)),
                    )
            })
            .collect();

        div()
            .key_context("Workbook")
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .size_full()
            .bg(theme::window())
            .text_color(theme::text())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_4()
                    .pt_8()
                    .pb_2()
                    .child(div().text_sm().child(self.title.clone()))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::text_faint())
                            .child(format!("{sheet_width:.0} × {sheet_height:.0} px")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::accent())
                            .child(self.status.clone()),
                    ),
            )
            .child(div().flex().gap_1().px_4().pb_2().children(tabs))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .mx(px(CANVAS_INSET))
                    .mb(px(CANVAS_INSET))
                    .bg(theme::canvas())
                    .child(
                        // The painted window sits under the scroller, which is
                        // transparent and exists only to own the scrollbars.
                        div()
                            .absolute()
                            .inset_0()
                            .children(self.image.clone().map(|image| {
                                img(image).w(px(viewport.0 as f32)).h(px(viewport.1 as f32))
                            })),
                    )
                    .child(
                        div()
                            .id("sheet-scroll")
                            .absolute()
                            .inset_0()
                            .overflow_scroll()
                            .track_scroll(&self.scroll)
                            .child(
                                div()
                                    .w(px(sheet_width as f32))
                                    .h(px(sheet_height as f32))
                                    .flex_none(),
                            ),
                    ),
            )
    }
}

/// Opens `path` in a spreadsheet window.
pub fn run(path: String) {
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("{path}: {error}");
            std::process::exit(1);
        }
    };
    let root = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let (regular, bold) = bundled_font_paths(&root);
    let workbook = match NativeWorkbook::open(&bytes, &aliased_faces(&regular, &bold)) {
        Ok(workbook) => workbook,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };

    let title = SharedString::from(
        PathBuf::from(&path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or(path.clone()),
    );
    // Owned up front: `SharedString` borrows a `&'static str` or takes a
    // `String`, and these are neither until copied.
    let sheet_names: Vec<SharedString> = workbook
        .sheet_names()
        .iter()
        .map(|name| SharedString::from(name.to_string()))
        .collect();

    Application::new().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1320.0), px(900.0)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some(SharedString::from("jade")),
                        appears_transparent: true,
                        traffic_light_position: Some(point(px(16.0), px(16.0))),
                    }),
                    window_background: WindowBackgroundAppearance::Transparent,
                    ..Default::default()
                },
                |_window, cx| {
                    cx.new(|cx| WorkbookView {
                        title,
                        workbook,
                        sheet_names,
                        image: None,
                        retired: Vec::new(),
                        painted: None,
                        scroll: ScrollHandle::new(),
                        focus: cx.focus_handle(),
                        status: SharedString::from(""),
                    })
                },
            )
            .expect("open window");

        window
            .update(cx, |view, window, cx| {
                window.focus(&view.focus_handle(cx));
            })
            .expect("focus the workbook");
        cx.activate(true);
    });
}

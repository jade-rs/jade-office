//! The spreadsheet (XLSX) view.
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
    div, img, point, prelude::*, px, App, Context, Entity, FocusHandle, Focusable, RenderImage,
    ScrollHandle, SharedString, Window,
};
use jade_painter::Painter;
use jade_skia_painter::SkiaPainter;
use jade_xlsx_layout::Rect;
use jade_xlsx_native_render::{aliased_faces, bundled_font_paths, NativeWorkbook};
use jade_xlsx_render::{render_grid, RenderOptions};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::editor::{self, Editor};
use crate::{i18n, theme, workspace, UI_FONT_FAMILY};

/// The strip of sheet tabs under the grid.
const SHEET_TABS_HEIGHT: f32 = 30.0;

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
}

pub struct WorkbookView {
    workbook: NativeWorkbook,
    sheet_names: Vec<SharedString>,
    image: Option<Arc<RenderImage>>,
    /// Textures replaced since the last frame. gpui wants them released with a
    /// window to hand, which only `render` has.
    retired: Vec<Arc<RenderImage>>,
    painted: Option<PaintedFor>,
    /// Reused between frames while the device size stays the same.
    surface: Option<(SkiaPainter, i32, i32)>,
    zoom: f32,
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

    /// Repaints when the scroll position, the space, the zoom or the sheet has
    /// moved since the image was made.
    ///
    /// `viewport` is in window px. The image is painted at device pixels —
    /// the window's scale factor times the zoom per sheet px — so a sheet is
    /// as sharp on a Retina display as the text round it.
    fn repaint_if_needed(&mut self, viewport: (f32, f32), scale_factor: f32) {
        let offset = self.scroll.offset();
        let scale = scale_factor * self.zoom;
        let wanted = PaintedFor {
            sheet: self.workbook.sheet_index(),
            // The scroll offset is negative going down; the sheet's
            // coordinates are not.
            scroll: (-f32::from(offset.x), -f32::from(offset.y)),
            size: (
                (viewport.0 * scale_factor).round().max(1.0) as i32,
                (viewport.1 * scale_factor).round().max(1.0) as i32,
            ),
            scale,
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
        let (width, height) = wanted.size;
        let reusable = matches!(self.surface, Some((_, w, h)) if w == width && h == height);
        if !reusable {
            self.surface = Some((self.workbook.new_surface(width, height)?, width, height));
        }
        let (painter, _, _) = self.surface.as_ref()?;
        let scale = f64::from(wanted.scale);
        // The surface holds the header bands as well as the sheet, so the part
        // of the sheet on show is smaller than it by exactly the bands — in
        // sheet px, which is what the painter draws in under the scale.
        let metrics = self.workbook.header_metrics();
        let viewport = Rect {
            x: (f64::from(wanted.scroll.0) / f64::from(self.zoom)).max(0.0),
            y: (f64::from(wanted.scroll.1) / f64::from(self.zoom)).max(0.0),
            width: (f64::from(width) / scale - metrics.row_width).max(0.0),
            height: (f64::from(height) / scale - metrics.column_height).max(0.0),
        };
        painter.save();
        painter.scale(scale, scale).ok()?;
        let painted = render_grid(
            painter,
            self.workbook.layout(),
            viewport,
            None,
            &RenderOptions::default(),
        );
        painter.restore();
        painted.ok()?;
        let (bgra, width, height) = painter.read_bgra_premultiplied()?;
        let buffer = image::ImageBuffer::from_raw(width as u32, height as u32, bgra)?;
        Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
    }
}

impl Editor for WorkbookView {
    fn status_items(&self) -> Vec<SharedString> {
        let name = self
            .sheet_names
            .get(self.workbook.sheet_index())
            .cloned()
            .unwrap_or_default();
        vec![
            i18n::format("sheet.position", &[("name", &name)]).into(),
            i18n::format("sheet.count", &[("n", &self.sheet_names.len())]).into(),
        ]
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

        // The part of the workspace this view is given, less the sheet tabs.
        let (area_width, area_height) = workspace::editor_area(window, cx);
        let viewport = (
            area_width.max(1.0),
            (area_height - SHEET_TABS_HEIGHT).max(1.0),
        );
        self.repaint_if_needed(viewport, window.scale_factor());

        let (sheet_width, sheet_height) = self.workbook.sheet_size();
        let (sheet_width, sheet_height) = (
            sheet_width as f32 * self.zoom,
            sheet_height as f32 * self.zoom,
        );
        let current = self.workbook.sheet_index();
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

        div()
            .key_context("Workbook")
            .font_family(UI_FONT_FAMILY)
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .size_full()
            .bg(theme::canvas())
            .text_color(theme::text())
            .child(
                div()
                    .relative()
                    .flex_1()
                    .child(
                        // The painted window sits under the scroller, which is
                        // transparent and exists only to own the scrollbars.
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
                            .child(div().w(px(sheet_width)).h(px(sheet_height)).flex_none()),
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
                    .children(tabs)
                    .child(div().flex_1())
                    .child(
                        div()
                            .px_3()
                            .text_xs()
                            .text_color(theme::text_faint())
                            .children((!self.status.is_empty()).then(|| self.status.clone())),
                    ),
            )
    }
}

/// Opens `path` as a spreadsheet view, for the workspace to show in a tab.
pub fn new_view(path: &Path, cx: &mut App) -> Result<Entity<WorkbookView>, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    // The fonts ship with the engine, in the `jade-core` submodule.
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let (regular, bold) = bundled_font_paths(&core);
    let workbook = NativeWorkbook::open(&bytes, &aliased_faces(&regular, &bold))?;

    // Owned up front: `SharedString` borrows a `&'static str` or takes a
    // `String`, and these are neither until copied.
    let sheet_names: Vec<SharedString> = workbook
        .sheet_names()
        .iter()
        .map(|name| SharedString::from(name.to_string()))
        .collect();

    Ok(cx.new(|cx| WorkbookView {
        workbook,
        sheet_names,
        image: None,
        retired: Vec::new(),
        painted: None,
        surface: None,
        zoom: 1.0,
        scroll: ScrollHandle::new(),
        focus: cx.focus_handle(),
        status: SharedString::from(""),
    }))
}

//! Word documents on the pure-Rust painter.
//!
//! The docx renderer is `jade-docx-editor`'s `CanvasRenderer` — the code the
//! browser editor and the native app both run. It reaches outside itself
//! through a `CanvasRenderHost`: a surface to paint each page on, and a
//! measurer to lay text out against. This is the host `jade-docx-native-render`
//! builds on Skia, rebuilt on [`TinySkiaPainter`] so it compiles for
//! `wasm32-unknown-unknown`:
//!
//! - one tiny-skia surface per page, handed out by `painter()`;
//! - measurement through `jade-docx-layout`'s shaping measurer (rustybuzz), as
//!   the Skia host measures — the measurer, not the painter, decides line
//!   breaks, which is what the engine's Chromium-parity fixtures pin;
//! - pictures decoded in Rust, as `data:` URIs the renderer hands over.
//!
//! What the Skia host does that this does not: record pages for PDF export,
//! and time its measurement calls (`std::time::Instant` panics in wasm).

use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::rc::Rc;

use jade_docx_editor::canvas_renderer::{CanvasRenderer, FullLayoutDebugSnapshot};
use jade_docx_editor::render_host::{CanvasRenderHost, ImageCropMapping};
use jade_docx_editor::word_document::WordDocument;
use jade_docx_editor::Options;
use jade_docx_layout::font_measurer::RegisteredFontStyle;
use jade_docx_layout::layout_cache::TextMeasurer;
pub use jade_docx_layout::shaping_measurer::ShapingMeasurer;
use jade_docx_layout::shaping_measurer::ShapingOptions;
use jade_painter::{ImageRect, Painter, TextMetrics};
use jade_tiny_skia_painter::{FontBook, TextMeasure, TinySkiaPainter};
use wasm_bindgen::JsValue;

/// Letter, the box a render starts from before the document's own section
/// properties say otherwise.
pub(crate) const INITIAL_PAGE: (f64, f64) = (816.0, 1056.0);

pub struct TinySkiaRenderHost {
    /// A surface per page, in document order, shared so they stay readable
    /// after the renderer has taken the host. A page outside [`Self::resident`]
    /// has a one-pixel placeholder instead.
    pub(crate) pages: Rc<RefCell<Vec<TinySkiaPainter>>>,
    /// Each page's box, in CSS px, shared so a page can be given its surface
    /// again when it comes back into [`Self::resident`].
    pub(crate) page_sizes: Rc<RefCell<Vec<(f64, f64)>>>,
    /// The pages (0-based) kept as pixels. A page at 2x is megabytes, so a
    /// shell showing a long document keeps only the pages on screen and a
    /// few round them: every page of a 174-page document ran a browser out
    /// of memory. Drawing meant for any other page goes to the scratch
    /// surface. All of them unless a shell says otherwise.
    pub(crate) resident: Rc<RefCell<Range<usize>>>,
    current_page: Cell<usize>,
    /// How many pages the last render produced; surfaces past it are a pool.
    pub(crate) rendered_pages: Rc<Cell<usize>>,
    /// Whether the render in progress paginates the whole document: a
    /// bounded repaint stops early, and must not shrink the page count to
    /// where it stopped.
    pub(crate) pass_covers_whole_document: Rc<Cell<bool>>,
    /// The last page (0-based) a bounded repaint may paint. Laying out up to
    /// its bound, the renderer starts the page after it — clears it and
    /// draws its first lines — before it stops; that page gets a one-pixel
    /// scratch surface instead, and keeps the pixels it had.
    pub(crate) last_paintable_page: Rc<Cell<usize>>,
    scratch: RefCell<Option<TinySkiaPainter>>,
    measurer: Rc<ShapingMeasurer>,
    fonts: Rc<FontBook>,
    render_error: RefCell<Option<String>>,
    page_width: f64,
    page_height: f64,
    /// Device pixels per CSS px. The surfaces are that much larger and their
    /// painters start pre-scaled, as a browser host scales a canvas backing
    /// and its transform; the renderer only reads it to align clips.
    scale: f64,
    /// Whether the pages show the document on a dark page. Shared so the
    /// document can switch it after the renderer has taken the host, and read
    /// when a page is added so it matches the others.
    pub(crate) dark_paper: Rc<Cell<bool>>,
}

impl TinySkiaRenderHost {
    pub fn new(
        page_width: f64,
        page_height: f64,
        scale: f64,
        fonts: Rc<FontBook>,
        measurer: Rc<ShapingMeasurer>,
    ) -> Option<Self> {
        Self::with_resident(
            page_width,
            page_height,
            scale,
            fonts,
            measurer,
            0..usize::MAX,
        )
    }

    /// [`Self::new`], keeping only the pages in `resident` as pixels.
    pub fn with_resident(
        page_width: f64,
        page_height: f64,
        scale: f64,
        fonts: Rc<FontBook>,
        measurer: Rc<ShapingMeasurer>,
        resident: Range<usize>,
    ) -> Option<Self> {
        let first = if resident.contains(&0) {
            surface(&fonts, &measurer, page_width, page_height, scale)?
        } else {
            placeholder(&fonts, &measurer, scale)?
        };
        Some(Self {
            pages: Rc::new(RefCell::new(vec![first])),
            page_sizes: Rc::new(RefCell::new(vec![(page_width, page_height)])),
            resident: Rc::new(RefCell::new(resident)),
            current_page: Cell::new(0),
            rendered_pages: Rc::new(Cell::new(1)),
            pass_covers_whole_document: Rc::new(Cell::new(true)),
            last_paintable_page: Rc::new(Cell::new(usize::MAX)),
            scratch: RefCell::new(None),
            measurer,
            fonts,
            render_error: RefCell::new(None),
            page_width,
            page_height,
            scale,
            dark_paper: Rc::new(Cell::new(false)),
        })
    }

    fn ensure_page_size(&mut self, index: usize, width: f64, height: f64) {
        let resident = self.resident.borrow().contains(&index);
        let held = self.pages.borrow().get(index).map(is_placeholder);
        // Already the right box, and pixels exactly when it is resident.
        if self.page_sizes.borrow().get(index) == Some(&(width, height)) && held == Some(!resident)
        {
            return;
        }
        let page = if resident {
            surface(&self.fonts, &self.measurer, width, height, self.scale)
                .expect("a page-sized surface")
        } else {
            placeholder(&self.fonts, &self.measurer, self.scale).expect("a one-pixel surface")
        };
        page.set_dark_paper(self.dark_paper.get());
        let mut sizes = self.page_sizes.borrow_mut();
        if index == sizes.len() {
            self.pages.borrow_mut().push(page);
            sizes.push((width, height));
        } else {
            self.pages.borrow_mut()[index] = page;
            sizes[index] = (width, height);
        }
    }
}

/// Tracked changes as Word shows them: with their markup — insertions
/// coloured and underlined, deletions struck through — unless the document
/// was saved with a view that leaves them out.
pub(crate) fn show_changes_as_saved(options: &mut Options, word_document: &WordDocument) {
    let saved = word_document
        .document
        .as_ref()
        .and_then(|document| document.display_content_revisions);
    options.set_render_changes(saved.unwrap_or(true));
}

/// A page's surface while it is not kept as pixels: one pixel, which every
/// draw lands outside of.
pub(crate) fn placeholder(
    fonts: &Rc<FontBook>,
    measurer: &Rc<ShapingMeasurer>,
    scale: f64,
) -> Option<TinySkiaPainter> {
    surface(fonts, measurer, 1.0 / scale, 1.0 / scale, scale)
}

/// Whether `page` is a [`placeholder`] rather than a page's pixels.
pub(crate) fn is_placeholder(page: &TinySkiaPainter) -> bool {
    page.width() <= 1 && page.height() <= 1
}

/// A page's surface, measuring text with `measurer` as the host does: the
/// renderer measures a table cell's lines through the painter, and they
/// have to break as body text in the same font does.
pub(crate) fn surface(
    fonts: &Rc<FontBook>,
    measurer: &Rc<ShapingMeasurer>,
    width: f64,
    height: f64,
    scale: f64,
) -> Option<TinySkiaPainter> {
    let painter = TinySkiaPainter::new(
        (width * scale).ceil() as u32,
        (height * scale).ceil() as u32,
        Rc::clone(fonts),
    )?;
    painter.scale(scale, scale).ok()?;
    painter.measure_text_with(Rc::new(MeasurerText(Rc::clone(measurer))));
    Some(painter)
}

/// The shaping measurer as a painter measures text: widths, and the font and
/// ink boxes canvas `measureText` reports.
struct MeasurerText(Rc<ShapingMeasurer>);

impl TextMeasure for MeasurerText {
    fn width(&self, text: &str, font: &str) -> Option<f64> {
        Some(self.0.measure_text(text, font).unwrap_or(0.0))
    }

    fn metrics(&self, text: &str, font: &str) -> Option<TextMetrics> {
        let width = self.width(text, font)?;
        let (font_ascent, font_descent) = self.0.font_bounding_box(font).unwrap_or((0.0, 0.0));
        let (ink_ascent, ink_descent) = self
            .0
            .measure_font_vertical_metrics(font)
            .unwrap_or((font_ascent, font_descent));
        Some(TextMetrics {
            width,
            font_bounding_box_ascent: font_ascent,
            font_bounding_box_descent: font_descent,
            actual_bounding_box_ascent: ink_ascent,
            actual_bounding_box_descent: ink_descent,
            actual_bounding_box_right: width,
        })
    }
}

impl CanvasRenderHost for TinySkiaRenderHost {
    type Painter = TinySkiaPainter;

    fn painter(&self) -> Self::Painter {
        let pages = self.pages.borrow();
        let index = self.current_page.get().min(pages.len().saturating_sub(1));
        if index > self.last_paintable_page.get() || !self.resident.borrow().contains(&index) {
            // One pixel: whatever is drawn there lands outside it, so the
            // clear and the lines the renderer starts the page with cost
            // nothing rather than a page's worth of pixels.
            let mut scratch = self.scratch.borrow_mut();
            if scratch.is_none() {
                *scratch = surface(
                    &self.fonts,
                    &self.measurer,
                    1.0 / self.scale,
                    1.0 / self.scale,
                    self.scale,
                );
            }
            if let Some(scratch) = scratch.as_ref() {
                return scratch.clone();
            }
        }
        pages[index].clone()
    }

    fn set_font(&self, font_descriptor: &str) {
        self.painter().set_font(font_descriptor);
    }

    fn supports_deferred_painting(&self) -> bool {
        true
    }

    fn draw_image_data_uri(
        &self,
        data_uri: &str,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        crop: Option<ImageCropMapping>,
    ) -> bool {
        let Some(bytes) = decode_data_uri(data_uri) else {
            self.render_error
                .replace(Some("invalid image data URI".to_string()));
            return true;
        };
        let (source, destination) = match crop {
            Some(crop) => (
                Some([
                    crop.source_x,
                    crop.source_y,
                    crop.source_width,
                    crop.source_height,
                ]),
                ImageRect {
                    x: x + width * crop.destination_x,
                    y: y + height * crop.destination_y,
                    width: width * crop.destination_width,
                    height: height * crop.destination_height,
                },
            ),
            None => (
                None,
                ImageRect {
                    x,
                    y,
                    width,
                    height,
                },
            ),
        };
        // Keyed by a hash of the URI: the same picture on every page decodes
        // once per surface rather than per draw.
        let key = format!("docx-image-{:016x}", fnv1a(data_uri.as_bytes()));
        let painter = self.painter();
        if !painter.draw_encoded_image_rect(&key, &bytes, source, destination) {
            // A picture in a format this build cannot draw costs that
            // picture, not the document: an empty frame where it sits, as
            // Word shows one it cannot display.
            painter.save();
            painter.set_stroke_style_str("#c0c0c0");
            painter.set_line_width(1.0);
            painter.stroke_rect(x, y, width, height);
            painter.restore();
        }
        true
    }

    fn take_render_error(&self) -> Option<String> {
        self.render_error.borrow_mut().take()
    }

    // Through the painter, which measures with the shaping measurer (see
    // `surface`), in the font the renderer last set on it.
    fn measure_text_width(&self, text: &str) -> Result<f64, JsValue> {
        let Ok(width) = self.painter().measure_text_width(text);
        Ok(width)
    }

    fn measure_text_metrics(&self, text: &str) -> Result<TextMetrics, JsValue> {
        let Ok(metrics) = self.painter().measure_text_metrics(text);
        Ok(metrics)
    }

    fn device_pixel_ratio(&self) -> f64 {
        self.scale
    }

    fn update_device_pixel_ratio(&mut self) -> Result<bool, JsValue> {
        Ok(false)
    }

    fn device_to_css(&self, value: f64) -> f64 {
        value / self.scale
    }

    fn canvas_css_size(&self) -> (f64, f64) {
        (self.page_width, self.page_height)
    }

    fn page_count(&self) -> usize {
        self.pages.borrow().len()
    }

    fn page_dimensions(&self, page_index: usize) -> Option<(f64, f64)> {
        self.page_sizes.borrow().get(page_index).copied()
    }

    fn reset_pages(&mut self) -> Result<(), JsValue> {
        self.render_error.borrow_mut().take();
        self.current_page.set(0);
        if self.pass_covers_whole_document.get() {
            // Only a render that paginates everything may shrink the count.
            self.rendered_pages.set(1);
        }
        Ok(())
    }

    fn configure_base_canvas(
        &mut self,
        width: f64,
        height: f64,
        _pageless: bool,
        _margin_top: f64,
    ) -> Result<(), JsValue> {
        self.ensure_page_size(0, width, height);
        self.page_width = width;
        self.page_height = height;
        Ok(())
    }

    fn clear(
        &mut self,
        _page_width: f64,
        _pageless: bool,
        _full_height: f64,
    ) -> Result<(), JsValue> {
        Ok(())
    }

    fn ensure_pageless_height(
        &mut self,
        _required_height: f64,
        _margin_bottom: f64,
    ) -> Result<(), JsValue> {
        Ok(())
    }

    fn start_new_page(&mut self, width: f64, height: f64, _margin_top: f64) -> Result<(), JsValue> {
        let index = self.current_page.get() + 1;
        self.ensure_page_size(index, width, height);
        self.current_page.set(index);
        self.rendered_pages
            .set(self.rendered_pages.get().max(index + 1));
        Ok(())
    }

    // The browser's page-bitmap cache: no canvas here, and the renderer must
    // not take a path that needs one. Failing loudly beats drawing nothing.
    fn context(&self) -> &web_sys::CanvasRenderingContext2d {
        unreachable!("tiny-skia host: the renderer must not reach for a canvas")
    }

    fn context_mut(&mut self) -> &mut web_sys::CanvasRenderingContext2d {
        unreachable!("tiny-skia host: the renderer must not reach for a canvas")
    }

    fn create_page_bitmap(&self, _page_index: usize) -> Result<js_sys::Promise, JsValue> {
        unreachable!("tiny-skia host: page bitmaps are a browser-side cache")
    }

    fn take_paged_snapshots(&mut self) -> Result<Option<Vec<JsValue>>, JsValue> {
        Ok(None)
    }

    fn take_paged_snapshots_for_pages(
        &mut self,
        _pages: Option<&[usize]>,
    ) -> Result<Option<Vec<JsValue>>, JsValue> {
        Ok(None)
    }
}

/// FNV-1a, for a short stable key from a long data URI.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// The bytes behind a base64 `data:` URI; `None` for anything else.
fn decode_data_uri(data_uri: &str) -> Option<Vec<u8>> {
    let (_, payload) = data_uri.split_once("base64,")?;
    let mut out = Vec::with_capacity(payload.len() * 3 / 4);
    let (mut buffer, mut bits) = (0u32, 0u32);
    for byte in payload.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b'\n' | b'\r' => continue,
            _ => return None,
        } as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    Some(out)
}

/// A shaping measurer with the regular and bold faces. One family is enough:
/// a family the measurer has not got falls back to the nearest weight, which
/// is what the painter does too — and every registration copies the file.
pub fn measurer(regular: &[u8], bold: Option<&[u8]>) -> Result<Rc<ShapingMeasurer>, String> {
    measurer_with(regular, bold, None)
}

/// [`measurer`], with Noto Sans Thai registered too: a character the run's
/// families lack — Thai in a Japanese run — is measured in a registered
/// face that has it, as Chromium falls back to one.
pub fn measurer_with(
    regular: &[u8],
    bold: Option<&[u8]>,
    thai: Option<&[u8]>,
) -> Result<Rc<ShapingMeasurer>, String> {
    let mut measurer = ShapingMeasurer::with_options(12.0, ShapingOptions::CHROMIUM_PARITY);
    measurer
        .register_font_bytes("Noto Sans JP", 400, RegisteredFontStyle::Normal, regular)
        .map_err(|error| format!("the regular face: {error}"))?;
    if let Some(bold) = bold {
        measurer
            .register_font_bytes("Noto Sans JP", 700, RegisteredFontStyle::Normal, bold)
            .map_err(|error| format!("the bold face: {error}"))?;
    }
    if let Some(thai) = thai {
        measurer
            .register_font_bytes("Noto Sans Thai", 400, RegisteredFontStyle::Normal, thai)
            .map_err(|error| format!("the Thai face: {error}"))?;
    }
    Ok(Rc::new(measurer))
}

/// An open Word document, laid out and painted, one surface per page.
pub struct Document {
    pages: Rc<RefCell<Vec<TinySkiaPainter>>>,
    page_count: usize,
    /// The first page's size in CSS px.
    page_size: (f64, f64),
}

impl Document {
    /// Reads `.docx` bytes and paints every page at `scale` device px per
    /// CSS px.
    pub fn open(
        bytes: &[u8],
        scale: f64,
        fonts: Rc<FontBook>,
        measurer: Rc<ShapingMeasurer>,
    ) -> Result<Self, String> {
        let laid_out = lay_out(bytes, scale, fonts, measurer, Changes::AsSaved)?;
        let page_count = laid_out.rendered.get().min(laid_out.pages.borrow().len());
        Ok(Self {
            pages: laid_out.pages,
            page_count,
            page_size: laid_out.page_size,
        })
    }

    pub fn page_count(&self) -> usize {
        self.page_count
    }

    pub fn page_size(&self) -> (f64, f64) {
        self.page_size
    }

    /// Page `index`'s surface, at the scale the document was opened with.
    pub fn page(&self, index: usize) -> Option<TinySkiaPainter> {
        if index >= self.page_count {
            return None;
        }
        self.pages.borrow().get(index).cloned()
    }
}

/// Every line of `bytes` laid out as [`Document::open`] lays it out, from
/// page one, as the browser editor's `captureFullLayoutDebug()` reports it:
/// what a Word glyph reference is compared with (`bin/word_glyph_sweep.rs`).
/// Tracked changes are laid out without their markup, the view Word's
/// references are printed in.
pub fn full_layout_debug(
    bytes: &[u8],
    fonts: Rc<FontBook>,
    measurer: Rc<ShapingMeasurer>,
) -> Result<FullLayoutDebugSnapshot, String> {
    lay_out(bytes, 1.0, fonts, measurer, Changes::Hidden)?
        .renderer
        .capture_full_layout_debug()
        .map_err(|_| "the layout capture failed".to_string())
}

/// A document laid out and painted, with the renderer that did it.
struct LaidOut {
    renderer: CanvasRenderer<TinySkiaPainter>,
    pages: Rc<RefCell<Vec<TinySkiaPainter>>>,
    rendered: Rc<Cell<usize>>,
    /// The first page's size in CSS px.
    page_size: (f64, f64),
}

/// How tracked changes are laid out.
#[derive(Clone, Copy)]
enum Changes {
    /// As the document was saved to show them: see [`show_changes_as_saved`].
    AsSaved,
    /// The final document, without markup.
    Hidden,
}

fn lay_out(
    bytes: &[u8],
    scale: f64,
    fonts: Rc<FontBook>,
    measurer: Rc<ShapingMeasurer>,
    changes: Changes,
) -> Result<LaidOut, String> {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    // A viewer: no formatting marks.
    let mut options = Options::default();
    options.show_formatting_marks = false;
    let word_document = futures::executor::block_on(WordDocument::load(bytes, &options))
        .map_err(|error| format!("cannot read the document: {error}"))?;
    match changes {
        Changes::AsSaved => show_changes_as_saved(&mut options, &word_document),
        Changes::Hidden => options.set_render_changes(false),
    }
    let document = word_document
        .document
        .as_ref()
        .ok_or_else(|| "the document has no main part".to_string())?
        .clone();

    let render = |width: f64, height: f64| -> Result<LaidOut, String> {
        let host = TinySkiaRenderHost::new(
            width,
            height,
            scale,
            Rc::clone(&fonts),
            Rc::clone(&measurer),
        )
        .ok_or_else(|| "cannot allocate a page surface".to_string())?;
        let pages = Rc::clone(&host.pages);
        let rendered = Rc::clone(&host.rendered_pages);
        let mut renderer = CanvasRenderer::<TinySkiaPainter>::with_host(Box::new(host), false)
            .map_err(|_| "renderer setup failed".to_string())?;
        renderer.sync_word_document(&word_document);
        // There is no overlay to draw the selection on; and without a
        // declared full repaint the first render leaves the first
        // paragraph of later pages unpainted (see jade-docx-native-render).
        renderer.set_use_selection_overlay(false);
        renderer.mark_full_repaint();
        renderer
            .render_with_numbering(
                &document,
                &options,
                word_document.numbering_part.as_ref(),
                word_document.header_lines.as_deref(),
                word_document.footer_lines.as_deref(),
                Some(&word_document.comments),
            )
            .map_err(|_| "render failed".to_string())?;
        if let Some(error) = renderer.take_render_error() {
            return Err(error);
        }
        Ok(LaidOut {
            page_size: renderer.layout_page_size(),
            renderer,
            pages,
            rendered,
        })
    };

    // The section properties decide the page box; a first pass finds it,
    // and a host sized to it paints the pages — as the native app does.
    let first = render(INITIAL_PAGE.0, INITIAL_PAGE.1)?;
    let (width, height) = first.page_size;
    if (width, height) != INITIAL_PAGE && width > 1.0 && height > 1.0 {
        render(width, height)
    } else {
        Ok(first)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The contract fixture opens, paginates into its three pages, and every
    /// page carries ink.
    #[test]
    fn a_document_paints_every_page() {
        let core =
            std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
        let fonts_dir = core.join("fonts/Noto_Sans_JP/static");
        let regular = std::fs::read(fonts_dir.join("NotoSansJP-Regular.ttf")).unwrap();
        let bold = std::fs::read(fonts_dir.join("NotoSansJP-Bold.ttf")).unwrap();
        let fonts = crate::backend(regular.clone(), Some(bold.clone()))
            .unwrap()
            .fonts();
        let measurer = measurer(&regular, Some(&bold)).unwrap();
        let bytes = std::fs::read(core.join("contract.docx")).unwrap();
        let document = Document::open(&bytes, 1.0, fonts, measurer).unwrap();
        assert_eq!(document.page_count(), 3);
        for index in 0..document.page_count() {
            let page = document.page(index).unwrap();
            let ink = page
                .rgba_premultiplied()
                .chunks_exact(4)
                .filter(|pixel| pixel[0] < 128)
                .count();
            assert!(ink > 2000, "page {} has {ink} dark pixels", index + 1);
        }
        assert!(document.page(3).is_none());
    }

    #[test]
    fn a_base64_payload_decodes_to_its_bytes() {
        assert_eq!(
            decode_data_uri("data:image/png;base64,TWFu").unwrap(),
            b"Man"
        );
        assert_eq!(decode_data_uri("data:image/png;base64,TQ==").unwrap(), b"M");
        assert!(decode_data_uri("data:image/png,%89PNG").is_none());
    }
}

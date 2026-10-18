//! The browser API: wasm-bindgen classes over [`crate::Deck`] and
//! [`crate::Workbook`], handing pixels back as RGBA a canvas `ImageData` takes.

use std::rc::Rc;

use jade_docx_layout::shaping_measurer::ShapingMeasurer;
use jade_tiny_skia_painter::{FontBook, TinySkiaPainter};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

/// Font files, fetched by the page and handed over once: the painter's faces,
/// and the shaping measurer documents are laid out with.
#[wasm_bindgen]
pub struct WebFonts {
    book: Rc<FontBook>,
    measurer: Rc<ShapingMeasurer>,
}

#[wasm_bindgen]
impl WebFonts {
    #[wasm_bindgen(constructor)]
    pub fn new(regular: Vec<u8>, bold: Option<Vec<u8>>) -> Result<WebFonts, JsError> {
        let measurer =
            crate::document::measurer(&regular, bold.as_deref()).map_err(|e| JsError::new(&e))?;
        Ok(Self {
            book: crate::font_book(regular, bold).map_err(|e| JsError::new(&e))?,
            measurer,
        })
    }
}

/// Painted pixels: straight (not premultiplied) RGBA, row by row, which is
/// what `new ImageData(...)` expects.
#[wasm_bindgen]
pub struct Rendered {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

#[wasm_bindgen]
impl Rendered {
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.width
    }
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.height
    }
    /// Moves the pixels out; call once.
    #[wasm_bindgen(js_name = takeRgba)]
    pub fn take_rgba(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.rgba)
    }
}

fn rendered(painter: TinySkiaPainter) -> Rendered {
    let mut rgba = painter.rgba_premultiplied();
    for pixel in rgba.chunks_exact_mut(4) {
        let alpha = u16::from(pixel[3]);
        if alpha != 0 && alpha != 255 {
            for channel in &mut pixel[..3] {
                *channel = ((u16::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8;
            }
        }
    }
    Rendered {
        width: painter.width(),
        height: painter.height(),
        rgba,
    }
}

#[wasm_bindgen]
pub struct WebDeck(crate::Deck);

#[wasm_bindgen]
impl WebDeck {
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: &[u8], fonts: &WebFonts) -> Result<WebDeck, JsError> {
        Ok(Self(
            crate::Deck::open(bytes, Rc::clone(&fonts.book)).map_err(|e| JsError::new(&e))?,
        ))
    }
    #[wasm_bindgen(js_name = slideCount)]
    pub fn slide_count(&self) -> usize {
        self.0.slide_count()
    }
    #[wasm_bindgen(js_name = slideWidth)]
    pub fn slide_width(&self) -> f64 {
        self.0.slide_size().0
    }
    #[wasm_bindgen(js_name = slideHeight)]
    pub fn slide_height(&self) -> f64 {
        self.0.slide_size().1
    }
    pub fn notes(&self, index: usize) -> Option<String> {
        self.0.notes(index).map(str::to_string)
    }
    #[wasm_bindgen(js_name = renderSlide)]
    pub fn render_slide(&mut self, index: usize, width: u32) -> Option<Rendered> {
        self.0.render_slide(index, width).map(rendered)
    }
}

#[wasm_bindgen]
pub struct WebWorkbook(crate::Workbook);

#[wasm_bindgen]
impl WebWorkbook {
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: &[u8], fonts: &WebFonts) -> Result<WebWorkbook, JsError> {
        Ok(Self(
            crate::Workbook::open(bytes, "UTC", Rc::clone(&fonts.book))
                .map_err(|e| JsError::new(&e))?,
        ))
    }
    #[wasm_bindgen(js_name = sheetNames)]
    pub fn sheet_names(&self) -> Vec<String> {
        self.0.sheet_names()
    }
    #[wasm_bindgen(js_name = selectSheet)]
    pub fn select_sheet(&mut self, index: usize) -> Result<(), JsError> {
        self.0.select_sheet(index).map_err(|e| JsError::new(&e))
    }
    #[wasm_bindgen(js_name = sheetWidth)]
    pub fn sheet_width(&self) -> f64 {
        self.0.sheet_size().0
    }
    #[wasm_bindgen(js_name = sheetHeight)]
    pub fn sheet_height(&self) -> f64 {
        self.0.sheet_size().1
    }
    #[wasm_bindgen(js_name = renderViewport)]
    pub fn render_viewport(
        &self,
        scroll_x: f64,
        scroll_y: f64,
        width: u32,
        height: u32,
        scale: f64,
    ) -> Option<Rendered> {
        self.0
            .render_viewport((scroll_x, scroll_y), width, height, scale, None)
            .map(rendered)
    }
}

#[wasm_bindgen]
pub struct WebDocument(crate::document::Document);

#[wasm_bindgen]
impl WebDocument {
    /// Reads and paints every page, at `scale` device px per CSS px (the
    /// page's `devicePixelRatio`).
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: &[u8], scale: f64, fonts: &WebFonts) -> Result<WebDocument, JsError> {
        Ok(Self(
            crate::document::Document::open(
                bytes,
                scale,
                Rc::clone(&fonts.book),
                Rc::clone(&fonts.measurer),
            )
            .map_err(|e| JsError::new(&e))?,
        ))
    }
    #[wasm_bindgen(js_name = pageCount)]
    pub fn page_count(&self) -> usize {
        self.0.page_count()
    }
    #[wasm_bindgen(js_name = pageWidth)]
    pub fn page_width(&self) -> f64 {
        self.0.page_size().0
    }
    #[wasm_bindgen(js_name = pageHeight)]
    pub fn page_height(&self) -> f64 {
        self.0.page_size().1
    }
    #[wasm_bindgen(js_name = renderPage)]
    pub fn render_page(&self, index: usize) -> Option<Rendered> {
        self.0.page(index).map(rendered)
    }
}

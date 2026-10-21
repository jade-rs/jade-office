//! The browser API: wasm-bindgen classes over [`crate::Deck`] and
//! [`crate::Workbook`], handing pixels back as RGBA a canvas `ImageData` takes.

use std::rc::Rc;

use jade_docx_layout::shaping_measurer::ShapingMeasurer;
use jade_tiny_skia_painter::TinySkiaPainter;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    set_word_layout_platform_from_page();
}

/// Sets the Word the page's documents are laid out like from its
/// `?word_platform=mac|windows|auto`, or from the browser's platform without
/// one: a Mac lays documents out like Word for Mac. A page with a setting of
/// its own calls `setWordLayoutPlatform("mac" | "windows")` before opening
/// a document — jade-docx-editor's export, which this bundle carries.
pub fn set_word_layout_platform_from_page() {
    use jade_office_model::word::platform;
    let setting = query_parameter("word_platform").unwrap_or_default();
    platform::set_word_layout_platform(platform::word_layout_platform_for(
        &setting,
        &browser_platform(),
    ));
}

/// `navigator.userAgentData.platform`, else `navigator.platform`, else the
/// user agent; empty where there is no navigator.
fn browser_platform() -> String {
    let navigator = property(&js_sys::global(), "navigator");
    let user_agent_data = property(&navigator, "userAgentData");
    let platform = [
        (&user_agent_data, "platform"),
        (&navigator, "platform"),
        (&navigator, "userAgent"),
    ]
    .into_iter()
    .find_map(|(object, name)| property(object, name).as_string().filter(|v| !v.is_empty()));
    platform.unwrap_or_default()
}

/// The value of `name` in the page's query string, undecoded: the settings
/// it carries are plain words.
fn query_parameter(name: &str) -> Option<String> {
    let search = property(&property(&js_sys::global(), "location"), "search").as_string()?;
    search
        .trim_start_matches('?')
        .split('&')
        .find_map(|pair| match pair.split_once('=') {
            Some((key, value)) if key == name => Some(value.to_string()),
            _ => None,
        })
}

fn property(object: &JsValue, name: &str) -> JsValue {
    if object.is_undefined() || object.is_null() {
        return JsValue::UNDEFINED;
    }
    js_sys::Reflect::get(object, &JsValue::from_str(name)).unwrap_or(JsValue::UNDEFINED)
}

/// Font files, fetched by the page and handed over once: the painter's faces,
/// and the shaping measurer documents are laid out with.
#[wasm_bindgen]
pub struct WebFonts {
    backend: crate::TinySkiaBackend,
    measurer: Rc<ShapingMeasurer>,
}

#[wasm_bindgen]
impl WebFonts {
    #[wasm_bindgen(constructor)]
    pub fn new(regular: Vec<u8>, bold: Option<Vec<u8>>) -> Result<WebFonts, JsError> {
        let measurer =
            crate::document::measurer(&regular, bold.as_deref()).map_err(|e| JsError::new(&e))?;
        Ok(Self {
            backend: crate::backend(regular, bold).map_err(|e| JsError::new(&e))?,
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
            crate::Deck::open(bytes, fonts.backend.clone()).map_err(|e| JsError::new(&e))?,
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
            crate::Workbook::open(bytes, "UTC", fonts.backend.clone())
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
            .render_viewport(
                (scroll_x, scroll_y),
                width,
                height,
                scale,
                &crate::sheet::Overlay::default(),
            )
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
                fonts.backend.fonts(),
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

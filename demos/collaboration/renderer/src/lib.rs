//! The collaboration demo uses jade-office's actual DOCX layout and painter.
use std::rc::Rc;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct PreviewFonts {
    backend: jade_web::TinySkiaBackend,
    measurer: Rc<jade_web::document::ShapingMeasurer>,
}

#[wasm_bindgen]
impl PreviewFonts {
    #[wasm_bindgen(constructor)]
    pub fn new(regular: Vec<u8>, bold: Vec<u8>) -> Result<PreviewFonts, String> {
        let measurer = jade_web::document::measurer(&regular, Some(&bold))?;
        let backend = jade_web::backend(regular, Some(bold))?;
        Ok(Self { backend, measurer })
    }
}

#[wasm_bindgen]
pub struct OfficePreview {
    document: jade_web::document::Document,
    width: u32,
    height: u32,
}

#[wasm_bindgen]
impl OfficePreview {
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: &[u8], fonts: &PreviewFonts) -> Result<OfficePreview, String> {
        let document = jade_web::document::Document::open(
            bytes,
            0.85,
            fonts.backend.fonts(),
            Rc::clone(&fonts.measurer),
        )?;
        Ok(Self {
            document,
            width: 0,
            height: 0,
        })
    }
    pub fn page_count(&self) -> usize {
        self.document.page_count()
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn render_page(&mut self, index: usize) -> Result<Vec<u8>, String> {
        let painter = self.document.page(index).ok_or("Page does not exist")?;
        self.width = painter.width();
        self.height = painter.height();
        let mut pixels = painter.rgba_premultiplied();
        for p in pixels.chunks_exact_mut(4) {
            let alpha = u16::from(p[3]);
            if alpha != 0 && alpha != 255 {
                for c in &mut p[..3] {
                    *c = ((u16::from(*c) * 255 + alpha / 2) / alpha).min(255) as u8;
                }
            }
        }
        Ok(pixels)
    }
}

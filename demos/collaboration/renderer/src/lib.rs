//! The collaboration demo uses jade-office's actual DOCX layout and painter.
use jade_web::editing::{EditableDocument, Motion};
use serde_json::json;
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
        for p in pixels.as_chunks_mut::<4>().0 {
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

/// The same editable Word document, hit testing and layout used by the
/// Office app. The page is its input/rendering shell; it owns no text model.
#[wasm_bindgen]
pub struct OfficeEditor {
    document: EditableDocument,
    width: u32,
    height: u32,
}

fn byte_at(text: &str, units: usize) -> Result<usize, String> {
    let mut offset = 0;
    for (byte, ch) in text.char_indices() {
        if offset == units {
            return Ok(byte);
        }
        offset += ch.len_utf16();
    }
    if offset == units {
        Ok(text.len())
    } else {
        Err("Invalid UTF-16 selection boundary".into())
    }
}

#[wasm_bindgen]
impl OfficeEditor {
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: &[u8], fonts: &PreviewFonts) -> Result<OfficeEditor, String> {
        let mut document = EditableDocument::open(
            bytes,
            1.0,
            fonts.backend.fonts(),
            Rc::clone(&fonts.measurer),
        )?;
        document.set_formatting_marks(false)?;
        document.place_caret(0, 0);
        Ok(Self {
            document,
            width: 0,
            height: 0,
        })
    }

    pub fn state(&self) -> String {
        let paragraphs: Vec<_> = (0..self.document.paragraph_count())
            .map(|p| self.document.paragraph_text(p).unwrap_or_default())
            .collect();
        let point = |(p, byte): (usize, usize)| (p, paragraphs[p][..byte].encode_utf16().count());
        let selection = self
            .document
            .selection()
            .or_else(|| self.document.caret().map(|caret| (caret, caret)))
            .map(|(a, f)| json!({"anchor":point(a), "focus":point(f)}));
        let caret = self.document.caret_rect().map(|rect| {
            let (page, y) = self.document.page_and_offset(rect.document_y);
            json!({"page":page,"x":rect.x,"y":y,"width":rect.width,"height":rect.height})
        });
        let highlights: Vec<_> = self
            .document
            .selection_rects()
            .iter()
            .map(|r| json!({"page":r.page,"x":r.x,"y":r.y,"width":r.width,"height":r.height}))
            .collect();
        json!({"paragraphs":paragraphs,"selection":selection,"caret":caret,"highlights":highlights,"composing":self.document.composition().is_some(),"pages":self.document.page_count()}).to_string()
    }

    pub fn select(&mut self, ap: usize, au: usize, fp: usize, fu: usize) -> Result<(), String> {
        let a = self
            .document
            .paragraph_text(ap)
            .ok_or("Missing anchor paragraph")?;
        let f = self
            .document
            .paragraph_text(fp)
            .ok_or("Missing focus paragraph")?;
        self.document
            .select((ap, byte_at(&a, au)?), (fp, byte_at(&f, fu)?))
    }

    pub fn hit_test(&self, page: usize, x: f64, y: f64) -> String {
        let point = self
            .document
            .position_at(x, self.document.document_y(page, y))
            .map(|(p, byte)| {
                let text = self.document.paragraph_text(p).unwrap_or_default();
                (p, text[..byte].encode_utf16().count())
            });
        serde_json::to_string(&point).expect("hit position serializes")
    }

    pub fn select_word(&mut self, paragraph: usize, offset: usize) -> Result<(), String> {
        let text = self
            .document
            .paragraph_text(paragraph)
            .ok_or("Missing paragraph")?;
        self.document
            .select_word((paragraph, byte_at(&text, offset)?))
    }

    pub fn navigate(&mut self, motion: &str, extend: bool) -> Result<(), String> {
        let motion = match motion {
            "left" => Motion::Left,
            "right" => Motion::Right,
            "up" => Motion::Up,
            "down" => Motion::Down,
            "home" => Motion::LineStart,
            "end" => Motion::LineEnd,
            "wordLeft" => Motion::WordLeft,
            "wordRight" => Motion::WordRight,
            "start" => Motion::DocumentStart,
            "finish" => Motion::DocumentEnd,
            _ => return Err("Unknown caret motion".into()),
        };
        self.document.move_caret(motion, extend)
    }

    pub fn insert(&mut self, text: &str) -> Result<(), String> {
        self.document.insert_text(text)
    }
    pub fn delete(&mut self, backwards: bool) -> Result<(), String> {
        self.document.delete(backwards)
    }
    pub fn compose(&mut self, text: &str) -> Result<(), String> {
        self.document.set_composition(text)
    }
    pub fn commit(&mut self, text: &str) -> Result<(), String> {
        self.document.commit_composition(text)
    }
    pub fn selected_text(&self) -> String {
        self.document.selected_text().unwrap_or_default()
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
        for p in pixels.as_chunks_mut::<4>().0 {
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

//! The pure-Rust backend: tiny-skia surfaces, for the browser build.

use std::rc::Rc;

use jade_tiny_skia_painter::{FontBook, TinySkiaPainter};

use crate::{Backend, Face};

#[derive(Clone)]
pub struct TinySkiaBackend {
    fonts: Rc<FontBook>,
}

impl TinySkiaBackend {
    pub fn new(faces: &[Face]) -> Result<Self, String> {
        let mut book = FontBook::new();
        for face in faces {
            book.register(face.family.clone(), face.weight, face.data.clone())?;
        }
        Ok(Self {
            fonts: Rc::new(book),
        })
    }

    /// The font book, for code that paints outside the models (documents).
    pub fn fonts(&self) -> Rc<FontBook> {
        Rc::clone(&self.fonts)
    }
}

impl Backend for TinySkiaBackend {
    type Painter = TinySkiaPainter;

    fn surface(&self, width: u32, height: u32) -> Option<TinySkiaPainter> {
        TinySkiaPainter::new(width.max(1), height.max(1), Rc::clone(&self.fonts))
    }

    fn bgra(painter: &TinySkiaPainter) -> Option<(Vec<u8>, u32, u32)> {
        Some((
            painter.bgra_premultiplied(),
            painter.width(),
            painter.height(),
        ))
    }

    fn png(painter: &TinySkiaPainter) -> Option<Vec<u8>> {
        painter.encode_png()
    }
}

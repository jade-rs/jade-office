//! The pure-Rust backend: tiny-skia surfaces, for the browser build.

use std::rc::Rc;

use jade_painter::Painter;
use jade_tiny_skia_painter::{FontBook, TinySkiaPainter};

use crate::{Backend, Face};

/// Pixels to the px on a printed page: 192 to the inch.
const PAGE_SCALE: f64 = 2.0;

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

    fn page(&self, width: f64, height: f64) -> Option<TinySkiaPainter> {
        let page = self.surface(
            (width * PAGE_SCALE).ceil() as u32,
            (height * PAGE_SCALE).ceil() as u32,
        )?;
        page.scale(PAGE_SCALE, PAGE_SCALE).ok()?;
        Some(page)
    }

    fn pdf(pages: &[TinySkiaPainter], size: (f64, f64)) -> Result<Vec<u8>, String> {
        let pictures: Vec<crate::pdf::Picture> = pages
            .iter()
            .map(|page| {
                // Premultiplied BGRA over white paper, as RGB.
                let rgb = page
                    .bgra_premultiplied()
                    .chunks_exact(4)
                    .flat_map(|pixel| {
                        let paper = 255 - pixel[3];
                        [
                            pixel[2].saturating_add(paper),
                            pixel[1].saturating_add(paper),
                            pixel[0].saturating_add(paper),
                        ]
                    })
                    .collect();
                crate::pdf::Picture {
                    width_pt: size.0 * 0.75,
                    height_pt: size.1 * 0.75,
                    pixel_width: page.width(),
                    pixel_height: page.height(),
                    rgb,
                }
            })
            .collect();
        crate::pdf::of_pictures(&pictures)
    }
}

//! A deck rendered natively, without a browser.
//!
//! The counterpart of `jade-pptx-editor`: same reader, same layout, same
//! renderer, a different `jade_painter::Painter` underneath. What this crate
//! adds is the native half — a Skia surface, font files from disk, picture
//! bytes straight out of the package, and pixels in the byte order a native
//! shell wants.
//!
//! A slide is laid out once, in slide px, and painted at whatever scale the
//! caller asks for. Layouts are cached per slide; the browser editor does the
//! same, and it is what keeps a thumbnail strip and a zoomed slide from laying
//! the same text out twice.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use jade_painter::Painter;
use jade_pptx_layout::{layout_deck_slide, PlacedShape, SlideLayout};
use jade_pptx_reader::Deck;
use jade_pptx_render::{render_slide_with, PainterMeasure, RenderOptions};
use jade_skia_painter::SkiaPainter;

/// A font file to register under a family name, so a deck that asks for
/// Calibri gets something rather than nothing.
#[derive(Debug, Clone, Copy)]
pub struct FaceSpec<'a> {
    pub family: &'a str,
    pub weight: u16,
    pub path: &'a Path,
}

/// The families a deck is likely to name. Decks ask for Office's fonts, which
/// a machine outside Office does not have; aliasing the bundled face onto all
/// of them beats rendering a slide without text.
pub const ALIASED_FAMILIES: &[&str] = &[
    "Calibri",
    "Calibri Light",
    "Arial",
    "Segoe UI",
    "Helvetica",
    "Meiryo",
    "メイリオ",
    "Meiryo UI",
    "Yu Gothic",
    "游ゴシック",
    "Yu Gothic UI",
    "MS PGothic",
    "ＭＳ Ｐゴシック",
    "MS Gothic",
    "ＭＳ ゴシック",
    "BIZ UDPGothic",
    "Noto Sans JP",
];

/// The bundled Noto faces, as (regular, bold), relative to a jade-core root.
pub fn bundled_font_paths(core_root: &Path) -> (PathBuf, PathBuf) {
    let directory = core_root.join("fonts/Noto_Sans_JP/static");
    (
        directory.join("NotoSansJP-Regular.ttf"),
        directory.join("NotoSansJP-Bold.ttf"),
    )
}

/// [`ALIASED_FAMILIES`] at both weights, pointing at the two files.
pub fn aliased_faces<'a>(regular: &'a Path, bold: &'a Path) -> Vec<FaceSpec<'a>> {
    ALIASED_FAMILIES
        .iter()
        .flat_map(|family| {
            [
                FaceSpec {
                    family,
                    weight: 400,
                    path: regular,
                },
                FaceSpec {
                    family,
                    weight: 700,
                    path: bold,
                },
            ]
        })
        .collect()
}

/// Font faces parsed once and shared by every surface that paints with them.
///
/// Registering a face from bytes makes Skia parse and copy the file; the
/// aliases above point a dozen families at the same two files, so doing that
/// per family and per surface copied tens of megabytes for every thumbnail.
/// Here each file is read and parsed once, and a new surface gets refcounted
/// handles to the parsed faces.
#[derive(Clone)]
pub struct FontSet {
    /// A 1×1 surface that only holds the registrations.
    template: SkiaPainter,
}

impl FontSet {
    pub fn load(faces: &[FaceSpec<'_>]) -> Result<Self, String> {
        let template =
            SkiaPainter::new(1, 1).ok_or_else(|| "cannot create a font surface".to_string())?;
        // The parsed face for each file already registered, by path.
        let mut parsed: Vec<(&Path, usize)> = Vec::new();
        for face in faces {
            match parsed.iter().find(|(path, _)| *path == face.path) {
                Some((_, index)) => {
                    let (_, _, typeface) = template.registered_faces().swap_remove(*index);
                    template.register_face(face.family, face.weight, typeface);
                }
                None => {
                    let bytes = std::fs::read(face.path)
                        .map_err(|error| format!("{}: {error}", face.path.display()))?;
                    let index = template.registered_faces().len();
                    template
                        .register_font_bytes(face.family, face.weight, &bytes)
                        .ok_or_else(|| format!("{}: not a font", face.path.display()))?;
                    parsed.push((face.path, index));
                }
            }
        }
        Ok(Self { template })
    }

    /// A raster surface with every face registered.
    pub fn painter(&self, width: i32, height: i32) -> Option<SkiaPainter> {
        let painter = SkiaPainter::new(width, height)?;
        self.register_on(&painter);
        Some(painter)
    }

    /// Registers every face on an existing surface, such as a PDF page.
    pub fn register_on(&self, painter: &SkiaPainter) {
        for (family, weight, typeface) in self.template.registered_faces() {
            painter.register_face(family, weight, typeface);
        }
    }
}

/// An open deck: the parsed slides, the package's media, and the faces every
/// surface it paints on gets.
pub struct NativeDeck {
    deck: Deck,
    /// Every part of the package, for pictures. A deck's media is what a
    /// slide draws most of its pixels from, so it is kept rather than
    /// re-read from the zip per frame.
    parts: HashMap<String, Vec<u8>>,
    fonts: FontSet,
    options: RenderOptions,
    layouts: RefCell<HashMap<usize, Rc<SlideLayout>>>,
    /// One surface reused for measuring; layout cannot be done against the
    /// surface it will paint on, which is sized only after the slide is.
    measure: PainterMeasure<SkiaPainter>,
}

impl NativeDeck {
    /// Reads `.pptx` bytes. Slides are laid out lazily, on first paint.
    pub fn open(bytes: &[u8], faces: &[FaceSpec<'_>]) -> Result<Self, String> {
        Self::with_fonts(bytes, FontSet::load(faces)?)
    }

    /// [`Self::open`] with fonts already loaded — what reopening a deck after
    /// an edit wants, since the fonts have not changed.
    pub fn with_fonts(bytes: &[u8], fonts: FontSet) -> Result<Self, String> {
        let deck = jade_pptx_reader::read_deck(bytes)
            .map_err(|error| format!("cannot read the presentation: {error}"))?;

        let package = jade_opc::OpcPackage::open(bytes)
            .map_err(|error| format!("cannot open the package: {error}"))?;
        let mut parts = HashMap::new();
        let mut wanted = Vec::new();
        for slide in &deck.slides {
            let trees = [
                Some(&slide.content),
                slide.layout_content.as_ref(),
                slide.master_content.as_ref(),
            ];
            for tree in trees.into_iter().flatten() {
                collect_media_parts(&tree.shapes, &mut wanted);
                if let Some(jade_pptx_reader::Fill::Picture {
                    image_part: Some(part),
                    ..
                }) = &tree.background
                {
                    if !wanted.contains(part) {
                        wanted.push(part.clone());
                    }
                }
            }
        }
        for part in wanted {
            let name = part.trim_start_matches('/');
            if let Some(bytes) = package.part(name).or_else(|| package.part(&part)) {
                parts.insert(part.clone(), bytes.to_vec());
            }
        }

        let options = RenderOptions::default();
        let measuring = fonts
            .painter(1, 1)
            .ok_or_else(|| "cannot create a measuring surface".to_string())?;
        let measure = PainterMeasure::new(measuring, options.default_font.clone());
        Ok(Self {
            deck,
            parts,
            fonts,
            options,
            layouts: RefCell::new(HashMap::new()),
            measure,
        })
    }

    /// The same deck read again from edited bytes, keeping the fonts.
    pub fn reopen(&self, bytes: &[u8]) -> Result<Self, String> {
        Self::with_fonts(bytes, self.fonts.clone())
    }

    pub fn slide_count(&self) -> usize {
        self.deck.slides.len()
    }

    /// The slide's size in px at 96 dpi.
    pub fn slide_size(&self) -> (f64, f64) {
        (
            self.deck.slide_size.width_px(),
            self.deck.slide_size.height_px(),
        )
    }

    /// The slide's speaker notes, when it has any.
    pub fn notes(&self, index: usize) -> Option<&str> {
        self.deck.slides.get(index)?.notes.as_deref()
    }

    /// The text of the slide's own shapes, title first as PowerPoint orders
    /// them, for an outline or a search.
    pub fn slide_text(&self, index: usize) -> Vec<String> {
        let Some(layout) = self.layout(index) else {
            return Vec::new();
        };
        layout
            .shapes
            .iter()
            .filter(|shape| shape.origin == jade_pptx_layout::PlacedOrigin::Slide)
            .map(|shape| {
                shape
                    .lines
                    .iter()
                    .map(|line| {
                        line.runs
                            .iter()
                            .map(|run| run.text.as_str())
                            .collect::<String>()
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .filter(|text| !text.trim().is_empty())
            .collect()
    }

    /// The laid-out slide, measured once and cached.
    pub fn layout(&self, index: usize) -> Option<Rc<SlideLayout>> {
        if let Some(cached) = self.layouts.borrow().get(&index) {
            return Some(cached.clone());
        }
        let layout = Rc::new(layout_deck_slide(&self.deck, index, &self.measure)?);
        self.layouts.borrow_mut().insert(index, layout.clone());
        Some(layout)
    }

    /// Paints slide `index` scaled to `width` px wide (height follows the
    /// slide's aspect) and returns BGRA premultiplied pixels with the size.
    pub fn slide_pixels(&self, index: usize, width: i32) -> Option<(Vec<u8>, i32, i32)> {
        let (slide_width, slide_height) = self.slide_size();
        let width = width.max(1);
        let scale = f64::from(width) / slide_width.max(1.0);
        let height = ((slide_height * scale).round() as i32).max(1);
        let painter = self.fonts.painter(width, height)?;
        self.paint(&painter, index, scale).ok()?;
        painter.read_bgra_premultiplied()
    }

    /// PNG bytes of slide `index`, `width` px wide.
    pub fn slide_png(&self, index: usize, width: i32) -> Option<Vec<u8>> {
        let (slide_width, slide_height) = self.slide_size();
        let width = width.max(1);
        let scale = f64::from(width) / slide_width.max(1.0);
        let height = ((slide_height * scale).round() as i32).max(1);
        let painter = self.fonts.painter(width, height)?;
        self.paint(&painter, index, scale).ok()?;
        painter.encode_png()
    }

    /// Every slide as one vector PDF page, at the slide's own size.
    pub fn pdf(&self) -> Result<Vec<u8>, String> {
        let (width, height) = self.slide_size();
        let mut pages = Vec::with_capacity(self.slide_count());
        for index in 0..self.slide_count() {
            let page = SkiaPainter::new_recording(width, height)
                .ok_or_else(|| "cannot create a PDF page".to_string())?;
            self.fonts.register_on(&page);
            self.paint(&page, index, 1.0)
                .map_err(|_| format!("cannot paint slide {}", index + 1))?;
            pages.push(page);
        }
        SkiaPainter::encode_pdf(&pages)
    }

    fn paint(&self, painter: &SkiaPainter, index: usize, scale: f64) -> Result<(), ()> {
        let layout = self.layout(index).ok_or(())?;
        painter.save();
        painter.scale(scale, scale).map_err(|_| ())?;
        let mut draw_image = |shape: &PlacedShape| -> bool { self.draw_picture(painter, shape) };
        let painted = render_slide_with(painter, &layout, &self.options, &mut draw_image);
        painter.restore();
        painted.map_err(|_| ())
    }

    /// Draws a picture's bitmap where the renderer asks, under the transform
    /// it has already applied for the shape's rotation and flips.
    fn draw_picture(&self, painter: &SkiaPainter, shape: &PlacedShape) -> bool {
        let Some(bytes) = shape
            .image_part
            .as_deref()
            .and_then(|part| self.parts.get(part))
        else {
            return false;
        };
        // The crop is expressed against the bitmap's own size; normalising it
        // against a unit square gives the fractions Skia's source rectangle
        // takes, without decoding the image here just to learn its size.
        let Some((source, destination)) = shape.image_rectangles(1.0, 1.0) else {
            return false;
        };
        painter.draw_encoded_image_rect(
            bytes,
            Some([source.x, source.y, source.width, source.height]),
            destination.x,
            destination.y,
            destination.width,
            destination.height,
        )
    }
}

/// Every `image_part` under `shapes`, depth first, each once.
fn collect_media_parts(shapes: &[jade_pptx_reader::ShapeItem], out: &mut Vec<String>) {
    use jade_pptx_reader::ShapeItem;
    for item in shapes {
        let part = match item {
            ShapeItem::Picture(picture) => picture.image_part.as_deref(),
            ShapeItem::Shape(shape) => shape.image_part.as_deref(),
            ShapeItem::Group(group) => {
                collect_media_parts(&group.children, out);
                None
            }
            ShapeItem::Table(_) | ShapeItem::Chart(_) => None,
        };
        if let Some(part) = part {
            if !out.iter().any(|seen| seen == part) {
                out.push(part.to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(name: &str) -> NativeDeck {
        let core = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../jade-core");
        let bytes = std::fs::read(core.join("public/fixtures").join(name)).unwrap();
        let (regular, bold) = bundled_font_paths(&core);
        NativeDeck::open(&bytes, &aliased_faces(&regular, &bold)).unwrap()
    }

    #[test]
    fn every_fixture_slide_paints_ink() {
        for name in [
            "sample-deck.pptx",
            "grouped-deck.pptx",
            "inherited-deck.pptx",
            "themed-deck.pptx",
        ] {
            let deck = open(name);
            assert!(deck.slide_count() > 0, "{name} has no slides");
            for index in 0..deck.slide_count() {
                let (pixels, width, height) = deck.slide_pixels(index, 320).unwrap();
                assert_eq!(width, 320);
                assert!(height > 0);
                let first = &pixels[..4];
                assert!(
                    pixels.chunks_exact(4).any(|pixel| pixel != first),
                    "{name} slide {} is one flat colour",
                    index + 1
                );
            }
        }
    }

    #[test]
    fn the_deck_exports_one_pdf_page_per_slide() {
        let deck = open("sample-deck.pptx");
        let pdf = deck.pdf().unwrap();
        assert!(pdf.starts_with(b"%PDF"));
    }
}

//! A `jade_painter::Painter` drawn by tiny-skia, in pure Rust.
//!
//! The engine's renderers paint through [`jade_painter::Painter`], and the
//! native shell hands them a Skia surface. Skia is C++, though, and that is
//! what keeps those renderers out of a `wasm32-unknown-unknown` build. This
//! backend implements the same trait with crates that compile anywhere:
//!
//! - tiny-skia rasterises paths, fills, strokes, dashes and clips;
//! - ttf-parser supplies glyph outlines and advances, so text is drawn as
//!   paths — there is no font rasteriser to port. Each glyph is rasterised
//!   once per size, slant and quarter-pixel position and blended after that,
//!   as Skia and the browsers cache theirs;
//! - a rectangular clip — nearly every clip the renderers make — is kept as
//!   a rectangle, not a mask the size of the page.
//! - `image` decodes pictures.
//!
//! ## Matching the Skia backend
//!
//! Layout measures text through the same painter that draws it, so what has
//! to agree with the Skia backend is mostly measurement. Both take a run's
//! width as the plain sum of its glyphs' advances — no kerning, no shaping —
//! from the font the CSS shorthand names, and both fall back per run to the
//! first registered face that has the run's first character. Font metrics
//! come from `hhea`, rounded per half the way Chromium's canvas rounds them.
//!
//! Not drawn yet: canvas shadows and the blur filter (Word's shadow, emboss
//! and glow effects), and SVG pictures. Those calls are accepted and ignored.

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
use std::convert::Infallible;
use std::hash::{BuildHasherDefault, Hasher};
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use jade_painter::{ImageRect, ImageSource, Painter, TextMetrics};
use tiny_skia::{
    BlendMode, Color, FillRule, LineCap, LineJoin, Mask, Paint, Path, PathBuilder, PathSegment,
    Pixmap, PixmapPaint, Rect, Stroke, StrokeDash, Transform,
};

// ---- fonts ------------------------------------------------------------------

struct Face {
    family: String,
    weight: u16,
    data: Arc<Vec<u8>>,
    units_per_em: f32,
    ascender: f32,
    descender: f32,
}

/// The faces a painter can draw with, shared by every painter made from it.
///
/// A face is kept as its file's bytes and parsed on use: ttf-parser's parse
/// only reads the table directory, and keeping bytes rather than a borrowed
/// `Face` keeps this `'static` without leaking. Glyph outlines, the expensive
/// part, are cached here per face and glyph.
/// A face's bytes as the SVG font database takes them, shared, not copied.
struct SharedBytes(Arc<Vec<u8>>);

impl AsRef<[u8]> for SharedBytes {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// rustc's FxHash: a rotate, an xor and a multiply per word. The caches
/// below are looked up for every glyph drawn, and the standard library's
/// SipHash — made to resist keys chosen to collide, which glyph ids are not
/// — was a twelfth of a keystroke's repaint.
#[derive(Default, Clone, Copy)]
struct FastHasher(u64);

impl FastHasher {
    fn add(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
}

impl Hasher for FastHasher {
    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut word = [0; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.add(u64::from_le_bytes(word));
        }
    }

    fn write_u8(&mut self, value: u8) {
        self.add(u64::from(value));
    }

    fn write_u16(&mut self, value: u16) {
        self.add(u64::from(value));
    }

    fn write_u32(&mut self, value: u32) {
        self.add(u64::from(value));
    }

    fn write_u64(&mut self, value: u64) {
        self.add(value);
    }

    fn write_usize(&mut self, value: usize) {
        self.add(value as u64);
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

type FastMap<K, V> = HashMap<K, V, BuildHasherDefault<FastHasher>>;

/// What a CSS font shorthand comes to: the face, the size in px, and the
/// shorthand itself as the painter hands it back.
type ResolvedFont = (usize, f32, Rc<str>);

#[derive(Default)]
pub struct FontBook {
    faces: Vec<Face>,
    outlines: RefCell<FastMap<(usize, u16), Option<Path>>>,
    /// Glyphs rasterised once, shared by every page: see [`GlyphCoverage`].
    glyphs: RefCell<FastMap<GlyphKey, Option<Rc<GlyphCoverage>>>>,
    /// Each character's glyph and advance per face, and the face a run
    /// starting with it falls back to. Asking the face means parsing its
    /// table directory, which per character was a tenth of a repaint.
    characters: RefCell<FastMap<(usize, char), CharGlyph>>,
    covering: RefCell<FastMap<(usize, char), usize>>,
    /// Each font shorthand set, parsed and resolved to a face once: a
    /// renderer sets one per run of text, and parsing it every time was a
    /// twentieth of a keystroke's repaint.
    resolved: RefCell<FastMap<Box<str>, ResolvedFont>>,
    /// The faces as a font database for SVG text, built on first use.
    svg_fonts: RefCell<Option<Arc<resvg::usvg::fontdb::Database>>>,
    /// Decoded pictures, shared by every page as the glyphs are: a slide's
    /// background is decoded once, not on every repaint. Keyed by the
    /// caller's key and a hash of the bytes, since the key — a part name —
    /// repeats from one document to the next. `None` for bytes that would
    /// not decode, so they are not retried every frame.
    pictures: RefCell<HashMap<PictureKey, Option<Rc<Pixmap>>>>,
    /// The decoded pictures' pixel bytes, against [`PICTURE_CACHE_BYTES`].
    picture_bytes: Cell<usize>,
}

impl FontBook {
    pub fn new() -> Self {
        Self::default()
    }

    /// The picture `bytes` decoded, from the cache when it has been before.
    fn picture(&self, key: &str, bytes: &[u8]) -> Option<Rc<Pixmap>> {
        let id = (key.to_string(), content_hash(bytes));
        if let Some(found) = self.pictures.borrow().get(&id) {
            return found.clone();
        }
        let decoded = decode(bytes).map(Rc::new);
        let size = decoded.as_ref().map_or(0, |pixmap| pixmap.data().len());
        let mut pictures = self.pictures.borrow_mut();
        if self.picture_bytes.get() + size > PICTURE_CACHE_BYTES {
            pictures.clear();
            self.picture_bytes.set(0);
        }
        self.picture_bytes.set(self.picture_bytes.get() + size);
        pictures.insert(id, decoded.clone());
        decoded
    }

    /// Registers a font file under a family name and weight. The same bytes
    /// may be registered under several names; they are shared, not copied.
    pub fn register(
        &mut self,
        family: impl Into<String>,
        weight: u16,
        data: Arc<Vec<u8>>,
    ) -> Result<(), String> {
        let face =
            ttf_parser::Face::parse(&data, 0).map_err(|error| format!("not a font: {error}"))?;
        let units_per_em = f32::from(face.units_per_em().max(1));
        let (ascender, descender) = (f32::from(face.ascender()), f32::from(face.descender()));
        self.faces.push(Face {
            family: family.into(),
            weight,
            data,
            units_per_em,
            ascender,
            descender,
        });
        Ok(())
    }

    /// The registered faces as SVG text sees them. Every generic family — the
    /// fallback for a family nobody registered, such as an equation's
    /// Cambria Math — is the first face.
    fn svg_fonts(&self) -> Arc<resvg::usvg::fontdb::Database> {
        if let Some(database) = self.svg_fonts.borrow().as_ref() {
            return Arc::clone(database);
        }
        let mut database = resvg::usvg::fontdb::Database::new();
        for face in &self.faces {
            database.load_font_source(resvg::usvg::fontdb::Source::Binary(Arc::new(SharedBytes(
                Arc::clone(&face.data),
            ))));
        }
        if let Some(first) = self.faces.first() {
            let family = first.family.clone();
            database.set_serif_family(family.clone());
            database.set_sans_serif_family(family.clone());
            database.set_monospace_family(family.clone());
            database.set_cursive_family(family.clone());
            database.set_fantasy_family(family);
        }
        let database = Arc::new(database);
        *self.svg_fonts.borrow_mut() = Some(Arc::clone(&database));
        database
    }

    fn with_face<R>(&self, index: usize, f: impl FnOnce(&ttf_parser::Face<'_>) -> R) -> Option<R> {
        let face = self.faces.get(index)?;
        let parsed = ttf_parser::Face::parse(&face.data, 0).ok()?;
        Some(f(&parsed))
    }

    /// The face for a CSS family list and weight: the first family with a
    /// registered face wins, at the nearest weight; failing all, the face of
    /// the nearest weight overall — the docx measurer's fallback, so a bold
    /// run in an unknown family is measured and drawn bold alike.
    fn resolve(&self, families: &str, weight: u16) -> usize {
        for family in families.split(',') {
            let name = family.trim().trim_matches('\'').trim_matches('"');
            if name.is_empty() {
                continue;
            }
            let best = self
                .faces
                .iter()
                .enumerate()
                .filter(|(_, face)| face.family.eq_ignore_ascii_case(name))
                .min_by_key(|(_, face)| (i32::from(face.weight) - i32::from(weight)).abs());
            if let Some((index, _)) = best {
                return index;
            }
        }
        self.faces
            .iter()
            .enumerate()
            .min_by_key(|(_, face)| (i32::from(face.weight) - i32::from(weight)).abs())
            .map(|(index, _)| index)
            .unwrap_or(0)
    }

    /// `font`, unless it lacks `ch` and another face has it — or, when no
    /// face has it, [`MARK_FONT`] for a formatting mark it draws.
    fn covering(&self, font: usize, ch: char) -> usize {
        if let Some(&cached) = self.covering.borrow().get(&(font, ch)) {
            return cached;
        }
        let covering = self.find_covering(font, ch);
        self.covering.borrow_mut().insert((font, ch), covering);
        covering
    }

    fn find_covering(&self, font: usize, ch: char) -> usize {
        let has = |index: usize| {
            self.with_face(index, |face| face.glyph_index(ch).is_some())
                .unwrap_or(false)
        };
        if has(font) {
            return font;
        }
        (0..self.faces.len())
            .find(|index| has(*index))
            .unwrap_or_else(|| {
                if mark_glyph(ch).is_some() {
                    MARK_FONT
                } else {
                    font
                }
            })
    }

    /// The glyph's outline in font units, y up, cached.
    fn outline(&self, font: usize, glyph: ttf_parser::GlyphId) -> Option<Path> {
        if let Some(cached) = self.outlines.borrow().get(&(font, glyph.0)) {
            return cached.clone();
        }
        let path = if font == MARK_FONT {
            mark_outline(glyph)
        } else {
            self.with_face(font, |face| {
                let mut builder = OutlineBuilder(PathBuilder::new());
                face.outline_glyph(glyph, &mut builder)?;
                builder.0.finish()
            })
            .flatten()
        };
        self.outlines
            .borrow_mut()
            .insert((font, glyph.0), path.clone());
        path
    }
}

// ---- formatting marks -------------------------------------------------------

/// A face of its own for Word's formatting-mark arrows, standing in when no
/// registered face has them. The paragraph mark (U+21B5) and the page-break
/// mark (U+21D3) are not in a Japanese text face, and in a browser there is
/// no system face to fall back to, so without these they would draw as tofu.
/// Its glyphs are drawn here, in units of [`MARK_UNITS_PER_EM`], y up.
const MARK_FONT: usize = usize::MAX;

/// The mark face's em: the size the painter assumes for a face it has no
/// metrics of.
const MARK_UNITS_PER_EM: u16 = 1000;

/// The mark face's glyph for `ch`, and the glyph's advance in font units.
fn mark_glyph(ch: char) -> Option<(u16, u16)> {
    let id = match ch {
        '\u{21B5}' => 1,
        '\u{21D3}' => 2,
        '\u{2193}' => 3,
        _ => return None,
    };
    Some((id, MARK_UNITS_PER_EM))
}

/// The mark face's outline for `glyph`, as a single polygon.
fn mark_outline(glyph: ttf_parser::GlyphId) -> Option<Path> {
    let points: &[(f32, f32)] = match glyph.0 {
        // ↵: down the right, then left to a solid head.
        1 => &[
            (170.0, 195.0),
            (340.0, 335.0),
            (340.0, 230.0),
            (720.0, 230.0),
            (720.0, 720.0),
            (790.0, 720.0),
            (790.0, 160.0),
            (340.0, 160.0),
            (340.0, 55.0),
        ],
        // ⇓: two stems over one wide head.
        2 => &[
            (500.0, 20.0),
            (800.0, 320.0),
            (660.0, 320.0),
            (660.0, 720.0),
            (600.0, 720.0),
            (600.0, 320.0),
            (400.0, 320.0),
            (400.0, 720.0),
            (340.0, 720.0),
            (340.0, 320.0),
            (200.0, 320.0),
        ],
        // ↓
        3 => &[
            (500.0, 20.0),
            (680.0, 220.0),
            (535.0, 220.0),
            (535.0, 720.0),
            (465.0, 720.0),
            (465.0, 220.0),
            (320.0, 220.0),
        ],
        _ => return None,
    };
    let mut builder = PathBuilder::new();
    let (first, rest) = points.split_first()?;
    builder.move_to(first.0, first.1);
    for point in rest {
        builder.line_to(point.0, point.1);
    }
    builder.close();
    builder.finish()
}

struct OutlineBuilder(PathBuilder);

impl ttf_parser::OutlineBuilder for OutlineBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to(x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.0.quad_to(x1, y1, x, y);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.0.cubic_to(x1, y1, x2, y2, x, y);
    }
    fn close(&mut self) {
        self.0.close();
    }
}

/// One glyph to draw.
/// A glyph as coverage at one size, slant and quarter-pixel position.
///
/// A page of text is thousands of glyphs and a few dozen distinct ones.
/// Scan-converting each outline wherever it is drawn made text nearly all of
/// a repaint — a keystroke repaints its page — where a browser or Skia
/// blends a cached mask. So does this: a glyph is rasterised the first time
/// it is drawn at a size, slant and sub-pixel phase, and blended after that.
/// A quarter pixel is as fine as the phase goes across, so a glyph lands
/// within an eighth of a pixel of where its outline would along the line.
/// Its baseline is rounded to a whole pixel, as Skia's glyph cache does for
/// upright text: lines sit at many fractional heights, and a phase in each
/// direction had every glyph rasterised up to sixteen times, half of what
/// opening a document cost.
struct GlyphCoverage {
    /// The coverage's top-left, from the pixel the pen's phase is counted in.
    left: i32,
    top: i32,
    width: u32,
    height: u32,
    alpha: Vec<u8>,
}

/// What a [`GlyphCoverage`] depends on: the face and glyph, the outline's
/// scale and slant in device px (as bits, so it hashes), synthetic bold, and
/// the quarter-pixel phase of the pen along the line.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct GlyphKey {
    font: usize,
    glyph: u16,
    matrix: [u32; 3],
    bold: bool,
    phase: u8,
}

/// The cache is dropped whole when it grows past this, which only a document
/// at many sizes and zooms reaches.
const GLYPH_CACHE_LIMIT: usize = 16_384;

/// The decoded pictures are dropped whole past this many pixel bytes: a
/// dozen full-HD photos.
const PICTURE_CACHE_BYTES: usize = 96 * 1024 * 1024;

/// A picture's key from the caller, and a hash of its bytes.
type PictureKey = (String, u64);

fn content_hash(bytes: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

/// The quarter-pixel phase of a device coordinate, and the pixel it is
/// counted from.
fn phase(value: f32) -> (i32, u8) {
    let quarters = (value * 4.0).round();
    let pixel = (quarters / 4.0).floor();
    (pixel as i32, (quarters - pixel * 4.0) as u8)
}

/// `a × b / 255`, rounded.
fn scale_u8(a: u8, b: u8) -> u8 {
    let product = u32::from(a) * u32::from(b) + 128;
    ((product + (product >> 8)) >> 8) as u8
}

/// Rasterises `outline` under `transform` (device px, the phase included)
/// as the painter draws a glyph: filled, and stroked as well for a
/// synthetic bold of `bold_width` font units.
fn rasterize_glyph(
    outline: &Path,
    transform: Transform,
    bold_width: Option<f32>,
) -> Option<GlyphCoverage> {
    let bounds = device_bounds(outline.bounds(), transform);
    let grow = bold_width.map_or(0.0, |width| {
        width * transform.sx.abs().max(transform.sy.abs()) * 2.0
    }) + 2.0;
    let left = (bounds[0] - grow).floor() as i32;
    let top = (bounds[1] - grow).floor() as i32;
    let width = ((bounds[2] + grow).ceil() as i32 - left).max(1) as u32;
    let height = ((bounds[3] + grow).ceil() as i32 - top).max(1) as u32;
    let local = transform.post_translate(-left as f32, -top as f32);
    let alpha = match bold_width {
        None => area_coverage(outline, local, width as usize, height as usize),
        Some(bold) => {
            let mut pixmap = Pixmap::new(width, height)?;
            let mut paint = Paint::default();
            paint.set_color(Color::BLACK);
            paint.anti_alias = true;
            pixmap.fill_path(outline, &paint, FillRule::Winding, local, None);
            let stroke = Stroke {
                width: bold,
                line_join: LineJoin::Round,
                ..Stroke::default()
            };
            pixmap.stroke_path(outline, &paint, &stroke, local, None);
            pixmap.data().iter().skip(3).step_by(4).copied().collect()
        }
    };
    // Trimmed to its ink: the margin left for anti-aliasing is mostly
    // empty, and every empty pixel kept is one more visited per draw.
    let (columns, rows) = (width as usize, height as usize);
    let inked = |column: usize, row: usize| alpha[row * columns + column] != 0;
    let row_inked = |row: usize| (0..columns).any(|column| inked(column, row));
    let empty = GlyphCoverage {
        left,
        top,
        width: 0,
        height: 0,
        alpha: Vec::new(),
    };
    let Some(first_row) = (0..rows).find(|&row| row_inked(row)) else {
        return Some(empty);
    };
    let last_row = (0..rows).rev().find(|&row| row_inked(row))?;
    let column_inked = |column: usize| (first_row..=last_row).any(|row| inked(column, row));
    let first_column = (0..columns).find(|&column| column_inked(column))?;
    let last_column = (0..columns).rev().find(|&column| column_inked(column))?;
    let mut trimmed = Vec::new();
    for row in first_row..=last_row {
        trimmed
            .extend_from_slice(&alpha[row * columns + first_column..=row * columns + last_column]);
    }
    Some(GlyphCoverage {
        left: left + first_column as i32,
        top: top + first_row as i32,
        width: (last_column - first_column + 1) as u32,
        height: (last_row - first_row + 1) as u32,
        alpha: trimmed,
    })
}

/// The coverage of `outline`, filled under `transform`, of each pixel of a
/// `width` × `height` grid (the outline may run off it):
/// the exact area each pixel has inside, by accumulating every edge's
/// signed area along its rows (as font-rs and ab_glyph do) instead of the
/// path rasteriser's sixteen samples. Rasterising glyphs was half of what
/// opening a document cost.
fn area_coverage(outline: &Path, transform: Transform, width: usize, height: usize) -> Vec<u8> {
    // Two cells past each row's end take what an edge right of the grid
    // leaves, so that it never reaches into the next row.
    let stride = width + 2;
    let mut area = AreaAccumulator {
        width,
        height,
        stride,
        cells: vec![0.0; stride * height],
    };
    let place = |point: tiny_skia::Point| {
        let mut point = point;
        transform.map_point(&mut point);
        (point.x, point.y)
    };
    let (mut start, mut last) = ((0.0, 0.0), (0.0, 0.0));
    for segment in outline.segments() {
        match segment {
            PathSegment::MoveTo(point) => {
                area.line(last, start);
                start = place(point);
                last = start;
            }
            PathSegment::LineTo(point) => {
                let point = place(point);
                area.line(last, point);
                last = point;
            }
            PathSegment::QuadTo(control, point) => {
                let (control, point) = (place(control), place(point));
                area.quad(last, control, point);
                last = point;
            }
            PathSegment::CubicTo(first, second, point) => {
                let (first, second, point) = (place(first), place(second), place(point));
                area.cubic(last, first, second, point);
                last = point;
            }
            PathSegment::Close => {
                area.line(last, start);
                last = start;
            }
        }
    }
    area.line(last, start);
    // Each row's signed areas sum to its winding: covered where it is not
    // zero, as the non-zero rule fills.
    let mut alpha = Vec::with_capacity(width * height);
    for row in area.cells.chunks_exact(stride) {
        let mut sum = 0.0_f32;
        alpha.extend(row[..width].iter().map(|cell| {
            sum += cell;
            (sum.abs().min(1.0) * 255.0 + 0.5) as u8
        }));
    }
    alpha
}

/// The signed area each edge adds to the pixels it crosses, row by row.
struct AreaAccumulator {
    width: usize,
    height: usize,
    stride: usize,
    cells: Vec<f32>,
}

impl AreaAccumulator {
    fn line(&mut self, from: (f32, f32), to: (f32, f32)) {
        if from.1 == to.1 {
            return;
        }
        let (direction, from, to) = if from.1 < to.1 {
            (1.0, from, to)
        } else {
            (-1.0, to, from)
        };
        let slope = (to.0 - from.0) / (to.1 - from.1);
        let mut x = from.0;
        let first = from.1.max(0.0);
        if from.1 < 0.0 {
            x -= from.1 * slope;
        }
        let last = (to.1.ceil().max(0.0) as usize).min(self.height);
        for row in first as usize..last {
            let start = row * self.stride;
            let height = (row as f32 + 1.0).min(to.1) - (row as f32).max(from.1);
            let next = x + slope * height;
            let signed = height * direction;
            // Kept on the grid, should an edge stray off it.
            let edge = self.width as f32;
            let (left, right) = if x < next { (x, next) } else { (next, x) };
            let (left, right) = (left.clamp(0.0, edge), right.clamp(0.0, edge));
            let left_floor = left.floor();
            let left_index = left_floor as usize;
            let right_ceil = right.ceil();
            let right_index = right_ceil as usize;
            let cells = &mut self.cells;
            if right_index <= left_index + 1 {
                // Within one pixel: split by where the edge crosses it.
                let middle = (0.5 * (x + next)).clamp(0.0, edge) - left_floor;
                cells[start + left_index] += signed - signed * middle;
                cells[start + left_index + 1] += signed * middle;
            } else {
                let step = (right - left).recip();
                let left_fraction = left - left_floor;
                let first_area = 0.5 * step * (1.0 - left_fraction) * (1.0 - left_fraction);
                let right_fraction = right - right_ceil + 1.0;
                let last_area = 0.5 * step * right_fraction * right_fraction;
                cells[start + left_index] += signed * first_area;
                if right_index == left_index + 2 {
                    cells[start + left_index + 1] += signed * (1.0 - first_area - last_area);
                } else {
                    let second_area = step * (1.5 - left_fraction);
                    cells[start + left_index + 1] += signed * (second_area - first_area);
                    for index in left_index + 2..right_index - 1 {
                        cells[start + index] += signed * step;
                    }
                    let before_last = second_area + (right_index - left_index - 3) as f32 * step;
                    cells[start + right_index - 1] += signed * (1.0 - before_last - last_area);
                }
                cells[start + right_index] += signed * last_area;
            }
            x = next;
        }
    }

    fn quad(&mut self, from: (f32, f32), control: (f32, f32), to: (f32, f32)) {
        let deviation = (
            from.0 - 2.0 * control.0 + to.0,
            from.1 - 2.0 * control.1 + to.1,
        );
        let steps = curve_steps(deviation.0 * deviation.0 + deviation.1 * deviation.1);
        let mut last = from;
        for step in 1..=steps {
            let t = step as f32 / steps as f32;
            let u = 1.0 - t;
            let point = (
                u * u * from.0 + 2.0 * u * t * control.0 + t * t * to.0,
                u * u * from.1 + 2.0 * u * t * control.1 + t * t * to.1,
            );
            self.line(last, point);
            last = point;
        }
    }

    fn cubic(&mut self, from: (f32, f32), first: (f32, f32), second: (f32, f32), to: (f32, f32)) {
        let bend = |a: (f32, f32), b: (f32, f32), c: (f32, f32)| {
            let (x, y) = (a.0 - 2.0 * b.0 + c.0, a.1 - 2.0 * b.1 + c.1);
            x * x + y * y
        };
        let deviation = bend(from, first, second).max(bend(first, second, to));
        let steps = curve_steps(deviation);
        let mut last = from;
        for step in 1..=steps {
            let t = step as f32 / steps as f32;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            let point = (
                a * from.0 + b * first.0 + c * second.0 + d * to.0,
                a * from.1 + b * first.1 + c * second.1 + d * to.1,
            );
            self.line(last, point);
            last = point;
        }
    }
}

/// How many lines a curve whose second difference is `deviation` squared
/// is flattened into, so none strays a fraction of a pixel from it.
fn curve_steps(deviation: f32) -> usize {
    1 + (3.0 * deviation).sqrt().sqrt().floor() as usize
}

/// A character's glyph in a face, its advance in font units, and the
/// synthetic styles a Mathematical Alphanumeric Symbol stands for.
#[derive(Clone, Copy)]
struct CharGlyph {
    id: u16,
    advance: u16,
    bold: bool,
    italic: bool,
}

struct Glyph {
    id: ttf_parser::GlyphId,
    advance: f32,
    bold: bool,
    italic: bool,
}

/// A Mathematical Alphanumeric Symbol (U+1D400–U+1D7FF, and ℎ) as its base
/// Latin letter or digit, with whether its style is bold and italic.
/// Word's equations write their letters this way; Cambria Math has them, the
/// faces a page usually brings do not. Greek and the styles with no plain
/// counterpart (script, fraktur, double-struck) fall back to the plain letter.
fn math_letter(ch: char) -> Option<(char, bool, bool)> {
    let code = ch as u32;
    match code {
        0x210E => Some(('h', false, true)),
        0x1D400..=0x1D6A3 => {
            let offset = code - 0x1D400;
            let index = offset % 52;
            let base = if index < 26 {
                b'A' + index as u8
            } else {
                b'a' + (index - 26) as u8
            };
            // Bold, italic, bold italic, script, bold script, fraktur,
            // double-struck, bold fraktur, sans, sans bold, sans italic,
            // sans bold italic, monospace.
            let (bold, italic) = match offset / 52 {
                0 | 4 | 7 | 9 => (true, false),
                1 | 10 => (false, true),
                2 | 11 => (true, true),
                _ => (false, false),
            };
            Some((base as char, bold, italic))
        }
        0x1D7CE..=0x1D7FF => {
            let offset = code - 0x1D7CE;
            // Bold, double-struck, sans, sans bold, monospace.
            let bold = matches!(offset / 10, 0 | 3);
            Some(((b'0' + (offset % 10) as u8) as char, bold, false))
        }
        _ => None,
    }
}

// ---- state ------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum TextBaseline {
    #[default]
    Alphabetic,
    Top,
    Hanging,
    Middle,
    Ideographic,
    Bottom,
}

impl TextBaseline {
    fn parse(value: &str) -> Self {
        match value {
            "top" => Self::Top,
            "hanging" => Self::Hanging,
            "middle" => Self::Middle,
            "ideographic" => Self::Ideographic,
            "bottom" => Self::Bottom,
            _ => Self::Alphabetic,
        }
    }

    /// How far below `y` the alphabetic baseline sits, for a face's ascent
    /// and descent in px (both positive).
    fn offset(self, ascent: f32, descent: f32) -> f32 {
        match self {
            Self::Alphabetic | Self::Ideographic => 0.0,
            Self::Top => ascent,
            Self::Hanging => ascent * 0.8,
            Self::Middle => (ascent - descent) / 2.0,
            Self::Bottom => -descent,
        }
    }
}

#[derive(Clone)]
struct State {
    transform: Transform,
    fill: Color,
    stroke: Color,
    line_width: f32,
    global_alpha: f32,
    dash: Vec<f32>,
    font: usize,
    font_size_px: f32,
    /// The CSS shorthand last set, as given: hosts that measure with their
    /// own shaper read it back.
    font_descriptor: Rc<str>,
    baseline: TextBaseline,
    clip: Option<Rc<Clip>>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            transform: Transform::identity(),
            fill: Color::BLACK,
            stroke: Color::BLACK,
            line_width: 1.0,
            global_alpha: 1.0,
            dash: Vec::new(),
            font: 0,
            font_size_px: 16.0,
            font_descriptor: Rc::from("16px sans-serif"),
            baseline: TextBaseline::Alphabetic,
            clip: None,
        }
    }
}

/// Parses the CSS colour strings jade emits: `#rgb`, `#rrggbb`, `#rrggbbaa`,
/// `rgb()`/`rgba()`, and `transparent`. Anything else is opaque black, as a
/// browser keeps a style it cannot parse rather than failing.
fn parse_color(value: &str) -> Color {
    let text = value.trim();
    if text.eq_ignore_ascii_case("transparent") {
        return Color::TRANSPARENT;
    }
    if let Some(hex) = text.strip_prefix('#') {
        let digit = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
        let pair = |i: usize| digit(hex.as_bytes()[i]) * 16 + digit(hex.as_bytes()[i + 1]);
        return match hex.len() {
            3 => {
                let b = hex.as_bytes();
                Color::from_rgba8(digit(b[0]) * 17, digit(b[1]) * 17, digit(b[2]) * 17, 255)
            }
            6 => Color::from_rgba8(pair(0), pair(2), pair(4), 255),
            8 => Color::from_rgba8(pair(0), pair(2), pair(4), pair(6)),
            _ => Color::BLACK,
        };
    }
    if let Some(inner) = text
        .strip_prefix("rgba(")
        .or_else(|| text.strip_prefix("rgb("))
        .and_then(|rest| rest.strip_suffix(')'))
    {
        let parts: Vec<f32> = inner
            .split(',')
            .filter_map(|part| part.trim().parse::<f32>().ok())
            .collect();
        if parts.len() >= 3 {
            return Color::from_rgba(
                (parts[0] / 255.0).clamp(0.0, 1.0),
                (parts[1] / 255.0).clamp(0.0, 1.0),
                (parts[2] / 255.0).clamp(0.0, 1.0),
                parts.get(3).copied().unwrap_or(1.0).clamp(0.0, 1.0),
            )
            .unwrap_or(Color::BLACK);
        }
    }
    Color::BLACK
}

/// Splits a CSS font shorthand into (bold?, size in px, family list). Only
/// what jade emits: optional `bold`/`italic`, a `<n>pt` or `<n>px` size, then
/// the families.
fn parse_font(descriptor: &str) -> (u16, f32, String) {
    let weight = if descriptor.to_ascii_lowercase().contains("bold") {
        700
    } else {
        400
    };
    let tokens: Vec<&str> = descriptor.split_whitespace().collect();
    for (index, token) in tokens.iter().enumerate() {
        let size = if let Some(value) = token.strip_suffix("pt") {
            value.parse::<f32>().ok().map(|points| points * 96.0 / 72.0)
        } else if let Some(value) = token.strip_suffix("px") {
            value.parse::<f32>().ok()
        } else {
            None
        };
        if let Some(size) = size {
            return (weight, size, tokens[index + 1..].join(" "));
        }
    }
    (weight, 16.0, String::new())
}

// ---- the painter ------------------------------------------------------------

struct Inner {
    pixmap: RefCell<Pixmap>,
    fonts: Rc<FontBook>,
    state: RefCell<State>,
    stack: RefCell<Vec<State>>,
    path: RefCell<PathBuilder>,
    /// Parsed SVG pictures by content key, likewise; rasterized per draw at
    /// the size they land at.
    svgs: RefCell<HashMap<String, Option<Rc<resvg::usvg::Tree>>>>,
    /// The device-px bounds of everything drawn since the last
    /// [`TinySkiaPainter::take_damaged_rows`]: what a reader keeping a copy
    /// of the pixels has to read again.
    damage: Cell<Option<Bounds>>,
    /// Whether colours are mapped onto a dark page as they are drawn. See
    /// [`TinySkiaPainter::set_dark_paper`].
    dark_paper: Cell<bool>,
    /// Whether every pixel is still the paper — nothing drawn since the
    /// surface was made or its paper changed — so clearing has nothing to
    /// do. A new page was filled white, then cleared white again before
    /// anything was drawn on it.
    blank: Cell<bool>,
    /// Whether the paper has yet to be painted in: the pixels are still the
    /// zeroes the surface was allocated with, and are filled with paper
    /// before anything reads them or is drawn on them — unless the first
    /// drawing covers every pixel opaquely, as a slide's or a sheet's
    /// background does, and the fill would only be painted over.
    unpainted: Cell<bool>,
    /// The blends of the text colours drawn lately, newest first.
    blends: RefCell<Vec<Rc<BlendTable>>>,
    /// What measures text in place of the faces, when a host gave one. See
    /// [`TinySkiaPainter::measure_text_with`].
    measure: RefCell<Option<Rc<dyn TextMeasure>>>,
}

/// Text measurement a host puts in place of the painter's own.
///
/// The docx renderer measures most text through its host, which lays it out
/// with the shaping measurer, but some — the lines of a table cell — through
/// the painter it draws with. In a browser both are the canvas, so they
/// agree; here the painter would measure a sum of advances in whichever of
/// its faces resolves, and a cell's lines would break where body text in the
/// same font would not. A host that lays text out with a measurer of its own
/// hands it to every surface, and the two agree again.
pub trait TextMeasure {
    /// `text`'s advance, in px, in the CSS font shorthand `font`.
    fn width(&self, text: &str, font: &str) -> Option<f64>;
    /// `text`'s metrics in `font`, as a canvas's `measureText` gives them.
    fn metrics(&self, text: &str, font: &str) -> Option<TextMetrics>;
}

/// The most pixels a path's coverage buffer covers: past it, a large shape's
/// whole spans go through the rasteriser faster than a cell per pixel.
const AREA_FILL_LIMIT: f32 = 512.0 * 512.0;

/// How many text colours keep their [`BlendTable`].
const BLEND_TABLES: usize = 8;

/// A text colour blended at every coverage, worked out once: the colour
/// scaled by each, how much of the pixel under it each keeps, and the
/// result over the paper, which lies under most of a glyph's edge pixels.
/// Scaling every channel of every pixel as it was blended was most of
/// what drawing text cost.
struct BlendTable {
    source: [u8; 4],
    paper: [u8; 4],
    scaled: [[u8; 4]; 256],
    keep: [u8; 256],
    over_paper: [[u8; 4]; 256],
}

impl BlendTable {
    fn new(source: [u8; 4], paper: [u8; 4]) -> Self {
        let mut table = Self {
            source,
            paper,
            scaled: [[0; 4]; 256],
            keep: [0; 256],
            over_paper: [[0; 4]; 256],
        };
        for alpha in 0..=255u8 {
            let index = usize::from(alpha);
            table.scaled[index] = source.map(|channel| scale_u8(channel, alpha));
            table.keep[index] = 255 - scale_u8(source[3], alpha);
            let mut over = paper;
            table.blend_with(&mut over, alpha);
            table.over_paper[index] = over;
        }
        table
    }

    /// Source-over of the colour at `alpha` coverage onto `pixel`.
    #[inline(always)]
    fn blend(&self, pixel: &mut [u8; 4], alpha: u8) {
        if *pixel == self.paper {
            *pixel = self.over_paper[usize::from(alpha)];
        } else {
            self.blend_with(pixel, alpha);
        }
    }

    #[inline(always)]
    fn blend_with(&self, pixel: &mut [u8; 4], alpha: u8) {
        let (scaled, keep) = (
            &self.scaled[usize::from(alpha)],
            self.keep[usize::from(alpha)],
        );
        for (value, scaled) in pixel.iter_mut().zip(scaled) {
            *value = scaled.saturating_add(scale_u8(*value, keep));
        }
    }
}

/// A raster surface with a canvas-style painter on it. Cheap to clone: the
/// clones are the same surface, as `SkiaPainter`'s are.
#[derive(Clone)]
pub struct TinySkiaPainter {
    inner: Rc<Inner>,
}

impl TinySkiaPainter {
    /// A `width` × `height` surface filled with opaque white, as the Skia
    /// backend starts.
    pub fn new(width: u32, height: u32, fonts: Rc<FontBook>) -> Option<Self> {
        let pixmap = Pixmap::new(width.max(1), height.max(1))?;
        Some(Self {
            inner: Rc::new(Inner {
                pixmap: RefCell::new(pixmap),
                fonts,
                state: RefCell::new(State::default()),
                stack: RefCell::new(Vec::new()),
                path: RefCell::new(PathBuilder::new()),
                svgs: RefCell::new(HashMap::new()),
                damage: Cell::new(None),
                dark_paper: Cell::new(false),
                blank: Cell::new(true),
                unpainted: Cell::new(true),
                blends: RefCell::new(Vec::new()),
                measure: RefCell::new(None),
            }),
        })
    }

    /// Measures text with `measure` rather than the faces from here on; the
    /// faces still draw it. See [`TextMeasure`].
    pub fn measure_text_with(&self, measure: Rc<dyn TextMeasure>) {
        *self.inner.measure.borrow_mut() = Some(measure);
    }

    /// What a host's [`TextMeasure`] makes of `text` in the current font.
    fn host_measure<T>(
        &self,
        text: &str,
        measure: impl FnOnce(&dyn TextMeasure, &str, &str) -> Option<T>,
    ) -> Option<T> {
        let host = self.inner.measure.borrow().clone()?;
        let font = self.font_descriptor();
        measure(host.as_ref(), text, &font)
    }

    /// The surface's pixels, the paper painted in first if it is not yet.
    fn pixels(&self) -> std::cell::RefMut<'_, Pixmap> {
        let mut pixmap = self.inner.pixmap.borrow_mut();
        if self.inner.unpainted.replace(false) {
            pixmap.fill(self.paper());
        }
        pixmap
    }

    /// The pixels to read, the paper painted in first if it is not yet.
    fn pixels_to_read(&self) -> std::cell::Ref<'_, Pixmap> {
        drop(self.pixels());
        self.inner.pixmap.borrow()
    }

    /// Notes that drawing may have changed the pixels within `bounds`
    /// (device px), less whatever the clip hides of them.
    fn note_damage(&self, bounds: Bounds, clip: Option<&Clip>) {
        self.inner.blank.set(false);
        let bounds = match clip {
            Some(clip) => [
                bounds[0].max(clip.rect[0]),
                bounds[1].max(clip.rect[1]),
                bounds[2].min(clip.rect[2]),
                bounds[3].min(clip.rect[3]),
            ],
            None => bounds,
        };
        if !(bounds[2] > bounds[0] && bounds[3] > bounds[1]) {
            return;
        }
        let damage = match self.inner.damage.get() {
            Some(damage) => [
                damage[0].min(bounds[0]),
                damage[1].min(bounds[1]),
                damage[2].max(bounds[2]),
                damage[3].max(bounds[3]),
            ],
            None => bounds,
        };
        self.inner.damage.set(Some(damage));
    }

    /// The rows drawing has touched since the last call, whole pixels with
    /// one to spare for anti-aliasing, or `None` if nothing was drawn.
    pub fn take_damaged_rows(&self) -> Option<std::ops::Range<u32>> {
        let damage = self.inner.damage.take()?;
        let height = self.height();
        let top = (damage[1].floor() - 1.0).max(0.0) as u32;
        let bottom = ((damage[3].ceil() + 1.0).max(0.0) as u32).min(height);
        (bottom > top).then_some(top..bottom)
    }

    /// Rows `rows` of the surface as BGRA with premultiplied alpha, as
    /// [`Self::bgra_premultiplied`] gives the whole of it.
    pub fn bgra_premultiplied_rows(&self, rows: std::ops::Range<u32>) -> Vec<u8> {
        let pixmap = self.pixels_to_read();
        let stride = pixmap.width() as usize * 4;
        let rows = rows.start.min(pixmap.height()) as usize..rows.end.min(pixmap.height()) as usize;
        let bytes = &pixmap.data()[rows.start * stride..rows.end * stride];
        // Written in place, a word at a time: collecting a flat-mapped
        // iterator was a third of the time an edit took to reach the screen.
        let mut data = vec![0; bytes.len()];
        for (out, pixel) in data.chunks_exact_mut(4).zip(bytes.chunks_exact(4)) {
            let rgba = u32::from_le_bytes([pixel[0], pixel[1], pixel[2], pixel[3]]);
            let bgra = (rgba & 0xff00_ff00) | ((rgba >> 16) & 0xff) | ((rgba & 0xff) << 16);
            out.copy_from_slice(&bgra.to_le_bytes());
        }
        data
    }

    /// A 64-bit fingerprint of rows `rows`' pixels: equal pixels, equal
    /// fingerprints. A reader keeping a copy of the surface compares these to
    /// tell which of the rows drawing touched actually changed — a repaint
    /// that draws the same pixels again need not be copied anywhere.
    pub fn rows_fingerprint(&self, rows: std::ops::Range<u32>) -> u64 {
        let pixmap = self.pixels_to_read();
        let stride = pixmap.width() as usize * 4;
        let rows = rows.start.min(pixmap.height()) as usize..rows.end.min(pixmap.height()) as usize;
        let bytes = &pixmap.data()[rows.start * stride..rows.end * stride];
        // FxHash's step over words: not for adversaries, fast for pixels. Four
        // lanes, so the multiplies overlap rather than wait on each other: a
        // page that repainted whole is megabytes to go through per keystroke.
        const K: u64 = 0x517c_c1b7_2722_0a95;
        let step = |hash: u64, word: &[u8]| {
            let word = u64::from_le_bytes(word.try_into().unwrap_or([0; 8]));
            (hash.rotate_left(5) ^ word).wrapping_mul(K)
        };
        let mut lanes = [bytes.len() as u64, 1, 2, 3];
        let mut blocks = bytes.chunks_exact(32);
        for block in &mut blocks {
            lanes[0] = step(lanes[0], &block[0..8]);
            lanes[1] = step(lanes[1], &block[8..16]);
            lanes[2] = step(lanes[2], &block[16..24]);
            lanes[3] = step(lanes[3], &block[24..32]);
        }
        let mut hash = lanes.iter().fold(0, |hash: u64, &lane| {
            (hash.rotate_left(5) ^ lane).wrapping_mul(K)
        });
        for &byte in blocks.remainder() {
            hash = (hash.rotate_left(5) ^ u64::from(byte)).wrapping_mul(K);
        }
        hash
    }

    /// Shows the page dark for reading on screen, or as the document has it.
    ///
    /// Every colour drawn from then on goes through
    /// [`jade_painter::paper::to_dark_paper`], and clearing paints the dark
    /// paper rather than white; pictures are drawn as they are. The surface is
    /// cleared to the new paper, so the caller repaints after changing it.
    pub fn set_dark_paper(&self, on: bool) {
        if self.inner.dark_paper.replace(on) == on {
            return;
        }
        let whole = {
            let pixmap = self.inner.pixmap.borrow();
            [0.0, 0.0, pixmap.width() as f32, pixmap.height() as f32]
        };
        self.note_damage(whole, None);
        self.inner.blank.set(true);
        self.inner.unpainted.set(true);
    }

    /// `source` (premultiplied) blended at every coverage, over this page's
    /// paper, from the last few colours' tables.
    fn blend_table(&self, source: [u8; 4]) -> Rc<BlendTable> {
        let paper = self.paper().premultiply().to_color_u8();
        let paper = [paper.red(), paper.green(), paper.blue(), paper.alpha()];
        let mut tables = self.inner.blends.borrow_mut();
        let found = tables
            .iter()
            .position(|table| table.source == source && table.paper == paper);
        let table = match found {
            Some(index) => tables.remove(index),
            None => Rc::new(BlendTable::new(source, paper)),
        };
        tables.insert(0, table.clone());
        tables.truncate(BLEND_TABLES);
        table
    }

    /// The colour a cleared page shows.
    fn paper(&self) -> Color {
        if self.inner.dark_paper.get() {
            let [r, g, b] = jade_painter::paper::dark_paper();
            Color::from_rgba(r, g, b, 1.0).unwrap_or(Color::BLACK)
        } else {
            Color::WHITE
        }
    }

    /// The CSS font shorthand currently set.
    pub fn font_descriptor(&self) -> Rc<str> {
        Rc::clone(&self.inner.state.borrow().font_descriptor)
    }

    pub fn width(&self) -> u32 {
        self.inner.pixmap.borrow().width()
    }

    pub fn height(&self) -> u32 {
        self.inner.pixmap.borrow().height()
    }

    /// The pixels as RGBA, premultiplied — tiny-skia's own order.
    pub fn rgba_premultiplied(&self) -> Vec<u8> {
        self.pixels_to_read().data().to_vec()
    }

    /// The pixels as BGRA, premultiplied: what gpui's `RenderImage` holds.
    pub fn bgra_premultiplied(&self) -> Vec<u8> {
        // One pass, read and written as words: a page at 2x is millions of
        // pixels, read back after every edit that repaints it.
        self.bgra_premultiplied_rows(0..self.height())
    }

    pub fn encode_png(&self) -> Option<Vec<u8>> {
        self.pixels_to_read().encode_png().ok()
    }

    /// Draws encoded image bytes into a rectangle, cropping to a normalised
    /// source rectangle. Returns `false` when the bytes do not decode.
    pub fn draw_encoded_image_rect(
        &self,
        key: &str,
        bytes: &[u8],
        source: Option<[f64; 4]>,
        destination: ImageRect,
    ) -> bool {
        let image = if is_svg(bytes) {
            self.rasterize_svg(key, bytes, source, &destination)
        } else {
            self.inner.fonts.picture(key, bytes)
        };
        let Some(image) = image else {
            return false;
        };
        let [sx, sy, sw, sh] = source.unwrap_or([0.0, 0.0, 1.0, 1.0]);
        let (iw, ih) = (f64::from(image.width()), f64::from(image.height()));
        let (src_x, src_y, src_w, src_h) =
            (sx * iw, sy * ih, (sw * iw).max(1e-6), (sh * ih).max(1e-6));
        if destination.width <= 0.0 || destination.height <= 0.0 {
            return true;
        }
        let state = self.inner.state.borrow();
        // Image space → destination: scale the source rectangle onto the
        // destination, then the painter's transform.
        let scale_x = destination.width / src_w;
        let scale_y = destination.height / src_h;
        let local = Transform::from_row(
            scale_x as f32,
            0.0,
            0.0,
            scale_y as f32,
            (destination.x - src_x * scale_x) as f32,
            (destination.y - src_y * scale_y) as f32,
        );
        let transform = state.transform.pre_concat(local);
        // Cropping is a clip to the destination rectangle.
        let crop = Rect::from_xywh(
            destination.x as f32,
            destination.y as f32,
            destination.width as f32,
            destination.height as f32,
        )
        .map(PathBuilder::from_rect);
        let mut pixmap = self.pixels();
        let size = (pixmap.width(), pixmap.height());
        // The crop is one more clip, in force for this picture alone.
        let cropped = crop
            .as_ref()
            .map(|crop| Clip::new(state.clip.as_deref(), Some(crop), state.transform, size));
        let clip = cropped.as_ref().or(state.clip.as_deref());
        let image_bounds = Rect::from_xywh(0.0, 0.0, image.width() as f32, image.height() as f32)
            .map(|rect| device_bounds(rect, transform));
        let mask = match (clip, image_bounds) {
            (None, _) => None,
            (Some(clip), Some(bounds)) => match clip.coverage(bounds) {
                Coverage::Hidden => return true,
                Coverage::Whole => None,
                Coverage::Partial => Some(clip.whole_mask()),
            },
            (Some(clip), None) => Some(clip.whole_mask()),
        };
        let paint = PixmapPaint {
            opacity: state.global_alpha,
            blend_mode: BlendMode::SourceOver,
            quality: tiny_skia::FilterQuality::Bilinear,
        };
        pixmap.draw_pixmap(0, 0, image.as_ref().as_ref(), &paint, transform, mask);
        drop(pixmap);
        if let Some(bounds) = image_bounds {
            self.note_damage(bounds, clip);
        }
        true
    }

    fn color(&self, color: Color) -> Color {
        let alpha = self.inner.state.borrow().global_alpha;
        let mut color = if self.inner.dark_paper.get() {
            let [r, g, b] =
                jade_painter::paper::to_dark_paper([color.red(), color.green(), color.blue()]);
            Color::from_rgba(r, g, b, color.alpha()).unwrap_or(color)
        } else {
            color
        };
        color.apply_opacity(alpha);
        color
    }

    fn paint(&self, color: Color) -> Paint<'static> {
        let mut paint = Paint::default();
        paint.set_color(self.color(color));
        paint.anti_alias = true;
        paint
    }

    fn stroke_style(&self) -> Stroke {
        let state = self.inner.state.borrow();
        Stroke {
            width: state.line_width,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            dash: (!state.dash.is_empty())
                .then(|| {
                    let mut dash = state.dash.clone();
                    // Canvas repeats an odd-length pattern to make it even.
                    if dash.len() % 2 == 1 {
                        dash.extend_from_within(..);
                    }
                    StrokeDash::new(dash, 0.0)
                })
                .flatten(),
            ..Stroke::default()
        }
    }

    fn current_path(&self) -> Option<Path> {
        self.inner.path.borrow().clone().finish()
    }

    /// Fills `rect` (local, placed by `transform`) with `paint` under the
    /// clip. A rectangle under a rectangular clip is their intersection,
    /// filled without a mask; and when `opaque` is the colour `paint` lays
    /// down whole, whole pixels of it are written straight in — a page clear
    /// is one, every repaint, and through the raster pipeline it was a fifth
    /// of the repaint.
    fn fill_rect_clipped(
        &self,
        rect: Rect,
        paint: &Paint<'_>,
        transform: Transform,
        opaque: Option<Color>,
    ) {
        let clip = self.inner.state.borrow().clip.clone();
        let rectangular = clip.as_deref().is_none_or(|clip| clip.path.is_none());
        if !rectangular || transform.has_skew() {
            return self.clipped(rect, transform, |pixmap, transform, mask| {
                pixmap.fill_rect(rect, paint, transform, mask);
            });
        }
        let size = {
            let pixmap = self.inner.pixmap.borrow();
            (pixmap.width() as f32, pixmap.height() as f32)
        };
        let surface = [0.0, 0.0, size.0, size.1];
        let [left, top, right, bottom] = clip.as_deref().map_or(surface, |clip| clip.rect);
        let device = device_bounds(rect, transform);
        let visible = [
            device[0].max(left).max(0.0),
            device[1].max(top).max(0.0),
            device[2].min(right).min(surface[2]),
            device[3].min(bottom).min(surface[3]),
        ];
        if visible[2] <= visible[0] || visible[3] <= visible[1] {
            return;
        }
        // The paper's own colour over nothing but paper — a sheet's white
        // background on a new surface — changes no pixel.
        if self.inner.blank.get()
            && opaque.is_some_and(|color| color.to_color_u8() == self.paper().to_color_u8())
        {
            return;
        }
        // Opaque over every pixel: the paper would only be painted over.
        if opaque.is_some() && visible == surface {
            self.inner.unpainted.set(false);
        }
        let mut pixmap = self.pixels();
        self.note_damage(visible, None);
        let Some(color) = opaque else {
            if let Some(visible) = Rect::from_ltrb(visible[0], visible[1], visible[2], visible[3]) {
                pixmap.fill_rect(visible, paint, Transform::identity(), None);
            }
            return;
        };
        // Whole pixels are written straight in, a row at a time; the pixels
        // its fractional edges cross are blended by how much of each it
        // covers, as anti-aliasing would.
        let color = color.premultiply().to_color_u8();
        let value = [color.red(), color.green(), color.blue(), color.alpha()];
        let overlap = |index: usize, from: f32, to: f32| -> u8 {
            let (start, end) = (index as f32, index as f32 + 1.0);
            ((end.min(to) - start.max(from)).clamp(0.0, 1.0) * 255.0).round() as u8
        };
        let lerp = |pixel: &mut [u8], cover: u8| {
            for (channel, slot) in pixel.iter_mut().enumerate() {
                *slot =
                    scale_u8(value[channel], cover).saturating_add(scale_u8(*slot, 255 - cover));
            }
        };
        let [x0, y0, x1, y1] = visible;
        let columns = x0.floor() as usize..(x1.ceil() as usize).min(pixmap.width() as usize);
        let rows = y0.floor() as usize..(y1.ceil() as usize).min(pixmap.height() as usize);
        let width = pixmap.width() as usize;
        // The columns every row covers whole, between the part-covered edge
        // columns (if any) at either end.
        let whole = {
            let first = columns.start + usize::from(overlap(columns.start, x0, x1) != 255);
            let last = columns.end - usize::from(overlap(columns.end - 1, x0, x1) != 255);
            first..last.max(first)
        };
        let edges: Vec<(usize, u8)> = columns
            .clone()
            .filter(|column| !whole.contains(column))
            .map(|column| (column, overlap(column, x0, x1)))
            .collect();
        let row_of_value: Vec<u8> = value.repeat(whole.len());
        let data = pixmap.data_mut();
        for row in rows {
            let row_cover = overlap(row, y0, y1);
            let at = |column: usize| (row * width + column) * 4;
            if row_cover == 255 {
                data[at(whole.start)..at(whole.end)].copy_from_slice(&row_of_value);
            } else {
                for pixel in data[at(whole.start)..at(whole.end)].chunks_exact_mut(4) {
                    lerp(pixel, row_cover);
                }
            }
            for &(column, column_cover) in &edges {
                let cover = scale_u8(row_cover, column_cover);
                let pixel = &mut data[at(column)..at(column) + 4];
                if cover == 255 {
                    pixel.copy_from_slice(&value);
                } else if cover != 0 {
                    lerp(pixel, cover);
                }
            }
        }
    }

    /// Runs `draw` — onto a pixmap, under a transform and inside a mask it
    /// is handed — for drawing within `bounds` (local, placed by
    /// `transform`), or not at all when the clip hides it.
    ///
    /// Small drawing that crosses the edge of a rectangular clip is drawn on
    /// a layer the size of the drawing, inside a mask that size. A mask the
    /// size of the surface — made for every clip a drawing crossed, a
    /// sheet's gridlines crossing every cell's — was a fifth of what
    /// painting a sheet cost.
    fn clipped(
        &self,
        bounds: Rect,
        transform: Transform,
        draw: impl FnOnce(&mut Pixmap, Transform, Option<&Mask>),
    ) {
        let clip = self.inner.state.borrow().clip.clone();
        let device = device_bounds(bounds, transform);
        let mut pixmap = self.pixels();
        match clip.as_deref() {
            None => draw(&mut pixmap, transform, None),
            Some(clip) => match clip.coverage(device) {
                Coverage::Hidden => return,
                Coverage::Whole => draw(&mut pixmap, transform, None),
                // Small drawing — a gridline, a crop mark — on a layer of its
                // own; anything bigger through the clip's one mask, which
                // every drawing under the clip shares.
                Coverage::Partial
                    if clip.path.is_none()
                        && layer_area(device) * LAYER_SHARE
                            <= (pixmap.width() * pixmap.height()) as f32 =>
                {
                    let size = (pixmap.width(), pixmap.height());
                    let Some((x, y, mut layer, mask)) = layer_for(clip.rect, device, size) else {
                        return;
                    };
                    // The surface's pixels under the layer go in first and
                    // come back drawn on: every pixel is blended exactly as
                    // drawing on the surface itself blends it.
                    copy_region(&mut pixmap, &mut layer, (x, y), false);
                    draw(
                        &mut layer,
                        transform.post_translate(-(x as f32), -(y as f32)),
                        Some(&mask),
                    );
                    copy_region(&mut pixmap, &mut layer, (x, y), true);
                }
                Coverage::Partial => draw(&mut pixmap, transform, Some(clip.whole_mask())),
            },
        }
        drop(pixmap);
        self.note_damage(device, clip.as_deref());
    }

    fn fill_path_with(&self, path: &Path, color: Color, transform: Transform) {
        if self.fill_area(path, color, transform) {
            return;
        }
        let paint = self.paint(color);
        self.clipped(path.bounds(), transform, |pixmap, transform, mask| {
            pixmap.fill_path(path, &paint, FillRule::Winding, transform, mask);
        });
    }

    /// `path`'s bounds grown by what a stroke of `width` can add: half the
    /// width, or up to twice it at a mitred corner.
    fn stroke_bounds(path: &Path, width: f32) -> Rect {
        let bounds = path.bounds();
        let grow = width.max(1.0) * 2.0;
        Rect::from_ltrb(
            bounds.left() - grow,
            bounds.top() - grow,
            bounds.right() + grow,
            bounds.bottom() + grow,
        )
        .unwrap_or(bounds)
    }

    fn stroke_path_with(&self, path: &Path, color: Color, transform: Transform) {
        // Gridlines and cell borders are runs of separate straight,
        // axis-aligned segments: each is its width's rectangle, filled
        // without the path rasteriser.
        let half = self.inner.state.borrow().line_width / 2.0;
        if axis_segments(path, half)
            .is_some_and(|rects| self.fill_stroke_rects(&rects, color, transform))
        {
            return;
        }
        let stroke = self.stroke_style();
        // The stroke's outline, filled as any other path is.
        let resolution = tiny_skia::PathStroker::compute_resolution_scale(&transform);
        let dashed = match &stroke.dash {
            Some(dash) => path.dash(dash, resolution),
            None => None,
        };
        // A stroke no wider than a pixel the rasteriser draws as a hairline,
        // more cheaply than its outline can be made and filled.
        let hairline = stroke.width * resolution <= 1.0;
        let outline = dashed
            .as_ref()
            .or((stroke.dash.is_none()).then_some(path))
            .filter(|_| !hairline)
            .and_then(|path| path.stroke(&stroke, resolution));
        if outline.is_some_and(|outline| self.fill_area(&outline, color, transform)) {
            return;
        }
        let paint = self.paint(color);
        let bounds = Self::stroke_bounds(path, stroke.width);
        self.clipped(bounds, transform, |pixmap, transform, mask| {
            pixmap.stroke_path(path, &paint, &stroke, transform, mask);
        });
    }

    /// Fills `path` (non-zero) by the area each pixel has inside it, as a
    /// glyph is, and blends that under the clip; `false`, drawing nothing,
    /// when the part of it on the surface is too large for a coverage
    /// buffer the rasteriser's spans would beat.
    fn fill_area(&self, path: &Path, color: Color, transform: Transform) -> bool {
        let (surface_width, surface_height) = {
            let pixmap = self.inner.pixmap.borrow();
            (pixmap.width() as f32, pixmap.height() as f32)
        };
        let clip = self.inner.state.borrow().clip.clone();
        let [left, top, right, bottom] = clip
            .as_deref()
            .map_or([0.0, 0.0, surface_width, surface_height], |clip| clip.rect);
        let device = device_bounds(path.bounds(), transform);
        if !device.iter().all(|value| value.is_finite()) {
            return false;
        }
        // Unclipped, or inside the clip, the rasteriser's spans draw straight
        // onto the surface; across a clip's edge it needs a layer and a mask.
        let crosses_clip = clip
            .as_deref()
            .is_some_and(|clip| matches!(clip.coverage(device), Coverage::Partial));
        if !crosses_clip {
            return false;
        }
        let x0 = (device[0] - 1.0).max(left).max(0.0).floor();
        let y0 = (device[1] - 1.0).max(top).max(0.0).floor();
        let x1 = (device[2] + 1.0).min(right).min(surface_width).ceil();
        let y1 = (device[3] + 1.0).min(bottom).min(surface_height).ceil();
        if x1 <= x0 || y1 <= y0 {
            return true;
        }
        if (x1 - x0) * (y1 - y0) > AREA_FILL_LIMIT {
            return false;
        }
        let (width, height) = ((x1 - x0) as usize, (y1 - y0) as usize);
        let placed = transform.post_translate(-x0, -y0);
        let coverage = GlyphCoverage {
            left: x0 as i32,
            top: y0 as i32,
            width: width as u32,
            height: height as u32,
            alpha: area_coverage(path, placed, width, height),
        };
        self.blend_coverage(&coverage, 0, 0, self.color(color));
        true
    }

    /// Fills the rectangles a plain stroke covers, if the stroke is one
    /// they draw: solid, under a transform that keeps axes axes. Pieces
    /// that cross (gridlines, a box's corners) would blend a translucent
    /// colour twice where they meet, so only an opaque one draws them so.
    fn fill_stroke_rects(&self, rects: &[Rect], color: Color, transform: Transform) -> bool {
        {
            let state = self.inner.state.borrow();
            if !state.dash.is_empty() || state.line_width <= 0.0 || transform.has_skew() {
                return false;
            }
        }
        let paint = self.paint(color);
        let color = self.color(color);
        let opaque = (color.alpha() >= 1.0).then_some(color);
        if opaque.is_none() && rects.len() > 1 {
            return false;
        }
        for rect in rects {
            self.fill_rect_clipped(*rect, &paint, transform, opaque);
        }
        true
    }

    /// A synthetic bold: the glyph outlined once more, a thirtieth of an em
    /// wide. `units_per_em` is the outline's em in its own units.
    fn embolden(&self, path: &Path, color: Color, transform: Transform, units_per_em: f32) {
        let paint = self.paint(color);
        let stroke = Stroke {
            width: units_per_em / 30.0,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        let bounds = Self::stroke_bounds(path, stroke.width);
        self.clipped(bounds, transform, |pixmap, transform, mask| {
            pixmap.stroke_path(path, &paint, &stroke, transform, mask);
        });
    }

    /// Ascent and descent of `font` at `size` px, both positive.
    fn font_extent(&self, font: usize, size: f32) -> (f32, f32) {
        match self.inner.fonts.faces.get(font) {
            Some(face) => (
                face.ascender / face.units_per_em * size,
                -face.descender / face.units_per_em * size,
            ),
            None => (size * 0.8, size * 0.2),
        }
    }

    /// `text` split where the face drawing it changes: each character in
    /// the styled face, or the first face that has it — the Skia backend's
    /// rule, and a browser's.
    fn segments<'t>(&self, text: &'t str) -> Vec<(usize, &'t str)> {
        let font = self.inner.state.borrow().font;
        let mut segments: Vec<(usize, &'t str)> = Vec::new();
        let mut start = 0;
        let mut current = None;
        for (at, ch) in text.char_indices() {
            let covering = self.inner.fonts.covering(font, ch);
            if current.is_some_and(|face| face != covering) {
                segments.push((current.unwrap_or(font), &text[start..at]));
                start = at;
            }
            current = Some(covering);
        }
        if let Some(face) = current {
            segments.push((face, &text[start..]));
        }
        segments
    }

    /// The width of `text` at `size` px, each segment in its face.
    fn text_width(&self, size: f32, text: &str) -> f32 {
        self.segments(text)
            .into_iter()
            .flat_map(|(font, run)| self.glyphs(font, size, run))
            .map(|(_, advance)| advance)
            .sum()
    }

    /// Glyph ids and advances (px) for `text` in `font` at `size`.
    fn glyphs(&self, font: usize, size: f32, text: &str) -> Vec<(ttf_parser::GlyphId, f32)> {
        self.styled_glyphs(font, size, text)
            .into_iter()
            .map(|glyph| (glyph.id, glyph.advance))
            .collect()
    }

    /// [`Self::glyphs`], with how each is drawn: a math letter the face
    /// lacks is its base letter, slanted or emboldened as its style says.
    fn styled_glyphs(&self, font: usize, size: f32, text: &str) -> Vec<Glyph> {
        let fonts = &self.inner.fonts;
        let units = fonts
            .faces
            .get(font)
            .map(|face| face.units_per_em)
            .unwrap_or(1000.0);
        let missing = {
            let characters = fonts.characters.borrow();
            text.chars().any(|ch| !characters.contains_key(&(font, ch)))
        };
        if missing && font == MARK_FONT {
            let mut characters = fonts.characters.borrow_mut();
            for ch in text.chars() {
                let (id, advance) = mark_glyph(ch).unwrap_or((0, 0));
                let glyph = CharGlyph {
                    id,
                    advance,
                    bold: false,
                    italic: false,
                };
                characters.insert((font, ch), glyph);
            }
        } else if missing {
            let found: Vec<(char, CharGlyph)> = fonts
                .with_face(font, |face| {
                    text.chars()
                        .map(|ch| {
                            let (id, bold, italic) = match face.glyph_index(ch) {
                                Some(id) => (id, false, false),
                                None => match math_letter(ch) {
                                    Some((base, bold, italic)) => (
                                        face.glyph_index(base).unwrap_or(ttf_parser::GlyphId(0)),
                                        bold,
                                        italic,
                                    ),
                                    None => (ttf_parser::GlyphId(0), false, false),
                                },
                            };
                            let advance = face.glyph_hor_advance(id).unwrap_or(0);
                            (
                                ch,
                                CharGlyph {
                                    id: id.0,
                                    advance,
                                    bold,
                                    italic,
                                },
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            let mut characters = fonts.characters.borrow_mut();
            for (ch, glyph) in found {
                characters.insert((font, ch), glyph);
            }
        }
        let characters = fonts.characters.borrow();
        text.chars()
            .filter_map(|ch| characters.get(&(font, ch)))
            .map(|glyph| Glyph {
                id: ttf_parser::GlyphId(glyph.id),
                advance: f32::from(glyph.advance) / units * size,
                bold: glyph.bold,
                italic: glyph.italic,
            })
            .collect()
    }

    /// Draws a filled glyph from the cache, rasterising it on first use.
    /// `false` when the glyph is turned or skewed vertically, which the
    /// cache does not key: the caller draws the outline instead.
    fn blit_glyph(
        &self,
        font: usize,
        glyph: ttf_parser::GlyphId,
        transform: Transform,
        bold_width: Option<f32>,
        color: Color,
    ) -> bool {
        if transform.ky != 0.0 || !transform.tx.is_finite() || !transform.ty.is_finite() {
            return false;
        }
        let (x, phase_x) = phase(transform.tx);
        let y = transform.ty.round() as i32;
        let key = GlyphKey {
            font,
            glyph: glyph.0,
            matrix: [
                transform.sx.to_bits(),
                transform.kx.to_bits(),
                transform.sy.to_bits(),
            ],
            bold: bold_width.is_some(),
            phase: phase_x,
        };
        let fonts = &self.inner.fonts;
        let cached = fonts.glyphs.borrow().get(&key).cloned();
        let coverage = match cached {
            Some(coverage) => coverage,
            None => {
                let placed = Transform::from_row(
                    transform.sx,
                    0.0,
                    transform.kx,
                    transform.sy,
                    f32::from(phase_x) / 4.0,
                    0.0,
                );
                let coverage = fonts
                    .outline(font, glyph)
                    .and_then(|outline| rasterize_glyph(&outline, placed, bold_width))
                    .map(Rc::new);
                let mut glyphs = fonts.glyphs.borrow_mut();
                if glyphs.len() >= GLYPH_CACHE_LIMIT {
                    glyphs.clear();
                }
                glyphs.insert(key, coverage.clone());
                coverage
            }
        };
        if let Some(coverage) = coverage {
            self.blend_coverage(&coverage, x, y, self.color(color));
        }
        true
    }

    /// Blends `color` through `coverage` placed at pixel (`x`, `y`), under
    /// the clip: source-over, as the path fill it stands in for.
    fn blend_coverage(&self, coverage: &GlyphCoverage, x: i32, y: i32, color: Color) {
        if coverage.alpha.is_empty() {
            return;
        }
        let (left, top) = (x + coverage.left, y + coverage.top);
        let (width, height) = (coverage.width as i32, coverage.height as i32);
        let clip = self.inner.state.borrow().clip.clone();
        let mut pixmap = self.pixels();
        let (surface_width, surface_height) = (pixmap.width() as i32, pixmap.height() as i32);
        let mut columns = left.max(0)..(left + width).min(surface_width);
        let mut rows = top.max(0)..(top + height).min(surface_height);
        // A rectangle's coverage of each column and row, for the clip's
        // fractional edges; a path's coverage of each pixel.
        let mut edges: Option<(Bounds, Range<i32>, Range<i32>)> = None;
        let mut mask: Option<&[u8]> = None;
        if let Some(clip) = clip.as_deref() {
            let bounds = [
                left as f32,
                top as f32,
                (left + width) as f32,
                (top + height) as f32,
            ];
            match clip.coverage(bounds) {
                Coverage::Hidden => return,
                Coverage::Whole => {}
                Coverage::Partial if clip.path.is_none() => {
                    let rect = clip.rect;
                    columns = columns.start.max(rect[0].floor() as i32)
                        ..columns.end.min(rect[2].ceil() as i32);
                    rows =
                        rows.start.max(rect[1].floor() as i32)..rows.end.min(rect[3].ceil() as i32);
                    edges = Some((rect, columns.clone(), rows.clone()));
                }
                Coverage::Partial => mask = Some(clip.whole_mask().data()),
            }
        }
        // How much of pixel `index` along an axis lies within `from..to`,
        // rounded without `round`, a call into libm on some targets.
        let overlap = |index: i32, from: f32, to: f32| -> u8 {
            let (start, end) = (index as f32, index as f32 + 1.0);
            ((end.min(to) - start.max(from)).clamp(0.0, 1.0) * 255.0 + 0.5) as u8
        };
        let source = color.premultiply().to_color_u8();
        let source = [source.red(), source.green(), source.blue(), source.alpha()];
        let table = self.blend_table(source);
        let blend = |pixel: &mut [u8; 4], alpha: u8| table.blend(pixel, alpha);
        if columns.is_empty() || rows.is_empty() {
            return;
        }
        self.note_damage(
            [
                columns.start as f32,
                rows.start as f32,
                columns.end as f32,
                rows.end as f32,
            ],
            None,
        );
        let data = pixmap.data_mut();
        let span = (columns.start - left) as usize..(columns.end - left) as usize;
        for row in rows {
            let glyph_row = ((row - top) * width) as usize;
            let surface_row = (row * surface_width) as usize;
            let alphas = &coverage.alpha[glyph_row + span.start..glyph_row + span.end];
            let first = surface_row + columns.start as usize;
            let pixels = data[first * 4..(surface_row + columns.end as usize) * 4]
                .as_chunks_mut::<4>()
                .0;
            match (&edges, mask) {
                // The usual case, kept free of per-pixel checks.
                (None, None) => {
                    for (&alpha, pixel) in alphas.iter().zip(pixels.iter_mut()) {
                        if alpha != 0 {
                            blend(pixel, alpha);
                        }
                    }
                }
                // Only the first and last columns, and rows at the clip's
                // top and bottom, can be cut by its edges: a path filled
                // across the clip covers many pixels between them.
                (Some((rect, _, _)), _) => {
                    let row_cover = overlap(row, rect[1], rect[3]);
                    let (first_cover, last_cover) = (
                        overlap(columns.start, rect[0], rect[2]),
                        overlap(columns.end - 1, rect[0], rect[2]),
                    );
                    let last = alphas.len() - 1;
                    for (index, (&alpha, pixel)) in alphas.iter().zip(pixels.iter_mut()).enumerate()
                    {
                        if alpha == 0 {
                            continue;
                        }
                        let column_cover = match index {
                            0 => first_cover,
                            _ if index == last => last_cover,
                            _ => 255,
                        };
                        let cover = match (row_cover, column_cover) {
                            (255, 255) => 255,
                            (row, 255) => row,
                            (row, column) => scale_u8(row, column),
                        };
                        blend(
                            pixel,
                            if cover == 255 {
                                alpha
                            } else {
                                scale_u8(alpha, cover)
                            },
                        );
                    }
                }
                (None, Some(mask)) => {
                    let mask = &mask[first..first + alphas.len()];
                    for ((&alpha, &clip), pixel) in alphas.iter().zip(mask).zip(pixels.iter_mut()) {
                        if alpha != 0 {
                            blend(pixel, scale_u8(alpha, clip));
                        }
                    }
                }
            }
        }
    }

    fn draw_text(&self, text: &str, x: f64, y: f64, stroke: bool) {
        let segments = self.segments(text);
        let Some(&(first, _)) = segments.first() else {
            return;
        };
        let (size, baseline, transform, color) = {
            let state = self.inner.state.borrow();
            (
                state.font_size_px,
                state.baseline,
                state.transform,
                if stroke { state.stroke } else { state.fill },
            )
        };
        // One baseline for the whole text: the first segment's face's.
        let (ascent, descent) = self.font_extent(first, size);
        let baseline_y = y as f32 + baseline.offset(ascent, descent);
        let mut pen = x as f32;
        for (font, run) in segments {
            pen = self.draw_run(font, run, pen, baseline_y, (size, transform, color), stroke);
        }
    }

    /// One face's run of [`Self::draw_text`], from `pen`; the pen after it.
    fn draw_run(
        &self,
        font: usize,
        text: &str,
        mut pen: f32,
        baseline_y: f32,
        (size, transform, color): (f32, Transform, Color),
        stroke: bool,
    ) -> f32 {
        let units = self
            .inner
            .fonts
            .faces
            .get(font)
            .map(|face| face.units_per_em)
            .unwrap_or(1000.0);
        let scale = size / units;
        for glyph in self.styled_glyphs(font, size, text) {
            // Font units, y up → px at the pen, y down; a synthetic italic
            // leans 12° as the letter rises.
            let slant = if glyph.italic { 0.21 * scale } else { 0.0 };
            let glyph_transform = transform.pre_concat(Transform::from_row(
                scale, 0.0, slant, -scale, pen, baseline_y,
            ));
            let bold_width = glyph.bold.then_some(size / scale / 30.0);
            if !stroke && self.blit_glyph(font, glyph.id, glyph_transform, bold_width, color) {
                pen += glyph.advance;
                continue;
            }
            if let Some(outline) = self.inner.fonts.outline(font, glyph.id) {
                if stroke {
                    self.stroke_path_with(&outline, color, glyph_transform);
                } else {
                    self.fill_path_with(&outline, color, glyph_transform);
                    if glyph.bold {
                        self.embolden(&outline, color, glyph_transform, size / scale);
                    }
                }
            }
            pen += glyph.advance;
        }
        pen
    }

    /// Appends an elliptical arc as cubic Béziers, joining it to the current
    /// point with a line as canvas does.
    #[allow(clippy::too_many_arguments)]
    fn arc_segments(
        &self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        rotation: f64,
        start: f64,
        end: f64,
        anticlockwise: bool,
    ) {
        let tau = std::f64::consts::TAU;
        let mut sweep = end - start;
        if !anticlockwise {
            if sweep >= tau {
                sweep = tau;
            } else {
                sweep = sweep.rem_euclid(tau);
                if sweep == 0.0 && end != start {
                    sweep = tau;
                }
            }
        } else if -sweep >= tau {
            sweep = -tau;
        } else {
            sweep = -((start - end).rem_euclid(tau));
            if sweep == 0.0 && end != start {
                sweep = -tau;
            }
        }
        let (sin_r, cos_r) = rotation.sin_cos();
        let point = |angle: f64| {
            let (sin, cos) = angle.sin_cos();
            let (x, y) = (rx * cos, ry * sin);
            (cx + x * cos_r - y * sin_r, cy + x * sin_r + y * cos_r)
        };
        let derivative = |angle: f64| {
            let (sin, cos) = angle.sin_cos();
            let (x, y) = (-rx * sin, ry * cos);
            (x * cos_r - y * sin_r, x * sin_r + y * cos_r)
        };
        let mut path = self.inner.path.borrow_mut();
        let (sx, sy) = point(start);
        if path.is_empty() {
            path.move_to(sx as f32, sy as f32);
        } else {
            path.line_to(sx as f32, sy as f32);
        }
        let segments = ((sweep.abs() / (std::f64::consts::FRAC_PI_2)).ceil() as usize).max(1);
        let step = sweep / segments as f64;
        let k = 4.0 / 3.0 * (step / 4.0).tan();
        let mut angle = start;
        for _ in 0..segments {
            let next = angle + step;
            let (x0, y0) = point(angle);
            let (x1, y1) = point(next);
            let (dx0, dy0) = derivative(angle);
            let (dx1, dy1) = derivative(next);
            path.cubic_to(
                (x0 + k * dx0) as f32,
                (y0 + k * dy0) as f32,
                (x1 - k * dx1) as f32,
                (y1 - k * dy1) as f32,
                x1 as f32,
                y1 as f32,
            );
            angle = next;
        }
    }
}

/// Device-px bounds as `[left, top, right, bottom]`.
type Bounds = [f32; 4];

/// A transparent layer for drawing within `device` that crosses the edge of
/// the clip rectangle `clip` (both device px), on a surface of `size`:
/// where it goes on the surface, the layer, and the clip's coverage of it.
/// The layer is the drawing's own extent, cut only by the surface — as the
/// surface cuts it — so the rasteriser sees the same edges and draws the
/// same pixels; the clip is the mask's to apply. `None` when there is
/// nothing to draw.
fn layer_for(clip: Bounds, device: Bounds, size: (u32, u32)) -> Option<(i32, i32, Pixmap, Mask)> {
    // A pixel of anti-aliasing round the drawing, as `Clip::coverage` allows.
    let left = (device[0].floor() - 1.0).max(0.0);
    let top = (device[1].floor() - 1.0).max(0.0);
    let right = (device[2].ceil() + 1.0).min(size.0 as f32);
    let bottom = (device[3].ceil() + 1.0).min(size.1 as f32);
    let apart = right <= clip[0].floor()
        || left >= clip[2].ceil()
        || bottom <= clip[1].floor()
        || top >= clip[3].ceil();
    if right <= left || bottom <= top || apart {
        return None;
    }
    let (x, y) = (left as i32, top as i32);
    let (width, height) = ((right - left) as u32, (bottom - top) as u32);
    let layer = Pixmap::new(width, height)?;
    let mut mask = Mask::new(width, height)?;
    let inside = Rect::from_ltrb(clip[0] - left, clip[1] - top, clip[2] - left, clip[3] - top)?;
    mask.fill_path(
        &PathBuilder::from_rect(inside),
        FillRule::Winding,
        true,
        Transform::identity(),
    );
    Some((x, y, layer, mask))
}

/// How many times smaller than the surface drawing must be to go on a layer
/// of its own when it crosses a rectangle clip's edge: a layer is made, and
/// the surface under it copied in and back, per drawing, so a large one is
/// cheaper through the clip's shared mask.
const LAYER_SHARE: f32 = 8.0;

/// The area of `device` bounds, in device px.
fn layer_area(device: Bounds) -> f32 {
    ((device[2] - device[0]).max(0.0) + 3.0) * ((device[3] - device[1]).max(0.0) + 3.0)
}

/// Copies the pixels of the `layer`-sized region of `surface` at `at` into
/// `layer`, or with `back` the layer's pixels onto that region.
fn copy_region(surface: &mut Pixmap, layer: &mut Pixmap, at: (i32, i32), back: bool) {
    let stride = surface.width() as usize * 4;
    let span = layer.width() as usize * 4;
    let first = at.1 as usize * stride + at.0 as usize * 4;
    let rows = layer.data_mut().chunks_exact_mut(span);
    let surface = surface.data_mut();
    for (row, line) in rows.enumerate() {
        let start = first + row * stride;
        let region = &mut surface[start..start + span];
        if back {
            region.copy_from_slice(line);
        } else {
            line.copy_from_slice(region);
        }
    }
}

/// The rectangles a butt-capped stroke `half` a width either side of
/// `path` covers, if the path is only separate straight segments along
/// the axes — no joins, curves or closes to shape.
fn axis_segments(path: &Path, half: f32) -> Option<Vec<Rect>> {
    let mut rects = Vec::new();
    let mut start = None;
    for segment in path.segments() {
        match segment {
            PathSegment::MoveTo(point) => start = Some(point),
            PathSegment::LineTo(end) => {
                let from = start.take()?;
                let rect = if from.x == end.x {
                    Rect::from_ltrb(
                        from.x - half,
                        from.y.min(end.y),
                        from.x + half,
                        from.y.max(end.y),
                    )
                } else if from.y == end.y {
                    Rect::from_ltrb(
                        from.x.min(end.x),
                        from.y - half,
                        from.x.max(end.x),
                        from.y + half,
                    )
                } else {
                    return None;
                };
                // A segment of no length leaves no mark under butt caps.
                rects.extend(rect.filter(|rect| rect.width() > 0.0 && rect.height() > 0.0));
            }
            _ => return None,
        }
    }
    Some(rects)
}

/// The mitred outline of `rect` stroked `half` a width either side, as
/// bands that cross at the corners: full-width top and bottom, full-height
/// sides. Bands that only abut would each blend half of a pixel their edge
/// splits, and leave it short of covered; crossing ones blend as one.
fn rect_outline(rect: Rect, half: f32) -> Option<Vec<Rect>> {
    if rect.width() <= 0.0 || rect.height() <= 0.0 || half <= 0.0 {
        return None;
    }
    let [left, top, right, bottom] = [rect.left(), rect.top(), rect.right(), rect.bottom()];
    let outer = Rect::from_ltrb(left - half, top - half, right + half, bottom + half)?;
    if half * 2.0 >= rect.width() || half * 2.0 >= rect.height() {
        return Some(vec![outer]);
    }
    Some(vec![
        Rect::from_ltrb(outer.left(), outer.top(), outer.right(), top + half)?,
        Rect::from_ltrb(outer.left(), bottom - half, outer.right(), outer.bottom())?,
        Rect::from_ltrb(outer.left(), outer.top(), left + half, outer.bottom())?,
        Rect::from_ltrb(right - half, outer.top(), outer.right(), outer.bottom())?,
    ])
}

/// The bounds of `rect` once `transform` has placed it, in device px.
fn device_bounds(rect: Rect, transform: Transform) -> Bounds {
    let corners = [
        (rect.left(), rect.top()),
        (rect.right(), rect.top()),
        (rect.right(), rect.bottom()),
        (rect.left(), rect.bottom()),
    ];
    let mut bounds = [
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    ];
    for (x, y) in corners {
        let dx = transform.sx * x + transform.kx * y + transform.tx;
        let dy = transform.ky * x + transform.sy * y + transform.ty;
        bounds = [
            bounds[0].min(dx),
            bounds[1].min(dy),
            bounds[2].max(dx),
            bounds[3].max(dy),
        ];
    }
    bounds
}

/// The device rectangle `path` is, if it is exactly one axis-aligned
/// rectangle under `transform` — what `rect()` then `clip()` makes.
fn rect_of(path: &Path, transform: Transform) -> Option<Bounds> {
    use PathSegment::{Close, LineTo, MoveTo};
    if transform.has_skew() {
        return None;
    }
    let mut segments = path.segments();
    let (Some(MoveTo(a)), Some(LineTo(b)), Some(LineTo(c)), Some(LineTo(d)), Some(Close), None) = (
        segments.next(),
        segments.next(),
        segments.next(),
        segments.next(),
        segments.next(),
        segments.next(),
    ) else {
        return None;
    };
    let axis_aligned = (a.y == b.y && b.x == c.x && c.y == d.y && d.x == a.x)
        || (a.x == b.x && b.y == c.y && c.x == d.x && d.y == a.y);
    axis_aligned.then(|| device_bounds(path.bounds(), transform))
}

/// `a`'s coverage, multiplied by `b`'s: two clips in force at once.
fn multiply(a: &mut Mask, b: &Mask) {
    for (a, b) in a.data_mut().iter_mut().zip(b.data()) {
        *a = ((u16::from(*a) * u16::from(*b) + 127) / 255) as u8;
    }
}

/// The clip in force.
///
/// Nearly everything the renderers clip to is a rectangle — a page band, a
/// table cell, a line — and a coverage mask the size of the surface per clip
/// cost more than the drawing it bounded: every clip went over every pixel of
/// the page, twice, millions of them at 2x. So a rectangle stays a rectangle.
/// Drawing that lies where the rectangle covers whole pixels needs no mask,
/// drawing wholly outside it is skipped, and only drawing across its edge
/// takes a mask, made once per clip the first time one does.
struct Clip {
    /// Device px; everything the clip lets through lies within it.
    rect: Bounds,
    /// Exact coverage, once some clip in force was not a rectangle.
    path: Option<Mask>,
    /// `rect` as coverage, made on first use.
    rect_mask: OnceCell<Mask>,
    size: (u32, u32),
}

/// What drawing has to go through under the clip.
enum Coverage {
    /// Nothing: the clip hides it.
    Hidden,
    /// Nothing: the clip covers every pixel it can touch.
    Whole,
    /// The clip's edge crosses it: [`Clip::whole_mask`], or for a
    /// rectangle the coverage of the pixels its edge is in.
    Partial,
}

impl Clip {
    /// The clip after clipping `previous` (if any) to `path` under
    /// `transform`, on a surface of `size`. No path clips everything away.
    fn new(
        previous: Option<&Clip>,
        path: Option<&Path>,
        transform: Transform,
        size: (u32, u32),
    ) -> Self {
        let (width, height) = size;
        let surface = [0.0, 0.0, width as f32, height as f32];
        let outer = previous.map_or(surface, |clip| clip.rect);
        let Some(path) = path else {
            return Self {
                rect: [0.0; 4],
                path: None,
                rect_mask: OnceCell::new(),
                size,
            };
        };
        let intersect = |a: Bounds, b: Bounds| -> Bounds {
            let rect = [
                a[0].max(b[0]),
                a[1].max(b[1]),
                a[2].min(b[2]),
                a[3].min(b[3]),
            ];
            // An empty intersection stays empty rather than inside out.
            [rect[0], rect[1], rect[2].max(rect[0]), rect[3].max(rect[1])]
        };
        if let Some(rect) = rect_of(path, transform) {
            let mut mask = previous.and_then(|clip| clip.path.as_ref()).cloned();
            if let Some(mask) = &mut mask {
                mask.intersect_path(path, FillRule::Winding, true, transform);
            }
            return Self {
                rect: intersect(outer, rect),
                path: mask,
                rect_mask: OnceCell::new(),
                size,
            };
        }
        let mut mask = Mask::new(width, height).expect("a mask the size of the surface");
        mask.fill_path(path, FillRule::Winding, true, transform);
        if let Some(previous) = previous {
            multiply(&mut mask, previous.whole_mask());
        }
        Self {
            rect: intersect(outer, device_bounds(path.bounds(), transform)),
            path: Some(mask),
            rect_mask: OnceCell::new(),
            size,
        }
    }

    /// The clip's exact coverage over the whole surface.
    fn whole_mask(&self) -> &Mask {
        if let Some(mask) = &self.path {
            return mask;
        }
        self.rect_mask.get_or_init(|| {
            let (width, height) = self.size;
            let mut mask = Mask::new(width, height).expect("a mask the size of the surface");
            let [left, top, right, bottom] = self.rect;
            if let Some(rect) = Rect::from_ltrb(left, top, right, bottom) {
                mask.fill_path(
                    &PathBuilder::from_rect(rect),
                    FillRule::Winding,
                    true,
                    Transform::identity(),
                );
            }
            mask
        })
    }

    /// What drawing within `bounds` (device px) goes through. A pixel's
    /// coverage reaches it only from the pixel a fractional edge is in, so
    /// the comparison is on whole pixels, with one to spare for
    /// anti-aliasing.
    fn coverage(&self, bounds: Bounds) -> Coverage {
        let [left, top, right, bottom] = self.rect;
        let touched = [
            bounds[0].floor() - 1.0,
            bounds[1].floor() - 1.0,
            bounds[2].ceil() + 1.0,
            bounds[3].ceil() + 1.0,
        ];
        if !touched.iter().all(|edge| edge.is_finite()) {
            return Coverage::Partial;
        }
        let hidden = touched[2] <= left.floor()
            || touched[0] >= right.ceil()
            || touched[3] <= top.floor()
            || touched[1] >= bottom.ceil();
        if hidden {
            return Coverage::Hidden;
        }
        let whole = self.path.is_none()
            && touched[0] >= left.ceil()
            && touched[1] >= top.ceil()
            && touched[2] <= right.floor()
            && touched[3] <= bottom.floor();
        if whole {
            Coverage::Whole
        } else {
            Coverage::Partial
        }
    }
}

/// Decodes PNG, JPEG, GIF or BMP bytes to a premultiplied pixmap.
/// Whether `bytes` are an SVG document rather than a raster image.
fn is_svg(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(1024)];
    let text = String::from_utf8_lossy(head);
    let text = text.trim_start_matches('\u{feff}').trim_start();
    (text.starts_with("<?xml") || text.starts_with("<svg") || text.starts_with("<!--"))
        && text.contains("<svg")
}

impl TinySkiaPainter {
    /// The SVG picture under `key` rendered at the device size `destination`
    /// lands at (so it stays sharp at any zoom), whole, for the raster path
    /// to crop and place.
    fn rasterize_svg(
        &self,
        key: &str,
        bytes: &[u8],
        source: Option<[f64; 4]>,
        destination: &ImageRect,
    ) -> Option<Rc<Pixmap>> {
        let tree = {
            let mut svgs = self.inner.svgs.borrow_mut();
            svgs.entry(key.to_string())
                .or_insert_with(|| {
                    let mut options = resvg::usvg::Options {
                        font_family: self
                            .inner
                            .fonts
                            .faces
                            .first()
                            .map(|face| face.family.clone())
                            .unwrap_or_default(),
                        ..Default::default()
                    };
                    options.fontdb = self.inner.fonts.svg_fonts();
                    resvg::usvg::Tree::from_data(bytes, &options)
                        .ok()
                        .map(Rc::new)
                })
                .clone()?
        };
        let transform = self.inner.state.borrow().transform;
        let device_x =
            f64::from((transform.sx * transform.sx + transform.ky * transform.ky).sqrt());
        let device_y =
            f64::from((transform.kx * transform.kx + transform.sy * transform.sy).sqrt());
        let [_, _, sw, sh] = source.unwrap_or([0.0, 0.0, 1.0, 1.0]);
        const LIMIT: f64 = 8192.0;
        let width = (destination.width * device_x / sw.max(1e-6))
            .ceil()
            .clamp(1.0, LIMIT);
        let height = (destination.height * device_y / sh.max(1e-6))
            .ceil()
            .clamp(1.0, LIMIT);
        let mut pixmap = Pixmap::new(width as u32, height as u32)?;
        let size = tree.size();
        resvg::render(
            &tree,
            Transform::from_scale(
                (width / f64::from(size.width())) as f32,
                (height / f64::from(size.height())) as f32,
            ),
            &mut pixmap.as_mut(),
        );
        Some(Rc::new(pixmap))
    }
}

fn decode(bytes: &[u8]) -> Option<Pixmap> {
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (width, height) = image.dimensions();
    let mut data = image.into_raw();
    for pixel in data.chunks_exact_mut(4) {
        let alpha = u16::from(pixel[3]);
        for channel in &mut pixel[..3] {
            *channel = ((u16::from(*channel) * alpha + 127) / 255) as u8;
        }
    }
    Pixmap::from_vec(data, tiny_skia::IntSize::from_wh(width, height)?)
}

impl Painter for TinySkiaPainter {
    type Error = Infallible;

    fn draw_image(
        &self,
        image: ImageSource<'_>,
        source: Option<[f64; 4]>,
        destination: ImageRect,
    ) -> Result<bool, Self::Error> {
        Ok(self.draw_encoded_image_rect(image.key, image.bytes, source, destination))
    }

    fn save(&self) {
        let state = self.inner.state.borrow().clone();
        self.inner.stack.borrow_mut().push(state);
    }

    fn restore(&self) {
        if let Some(state) = self.inner.stack.borrow_mut().pop() {
            *self.inner.state.borrow_mut() = state;
        }
    }

    fn translate(&self, x: f64, y: f64) -> Result<(), Self::Error> {
        let mut state = self.inner.state.borrow_mut();
        state.transform = state.transform.pre_translate(x as f32, y as f32);
        Ok(())
    }

    fn scale(&self, x: f64, y: f64) -> Result<(), Self::Error> {
        let mut state = self.inner.state.borrow_mut();
        state.transform = state.transform.pre_scale(x as f32, y as f32);
        Ok(())
    }

    fn rotate(&self, angle: f64) -> Result<(), Self::Error> {
        let mut state = self.inner.state.borrow_mut();
        state.transform = state
            .transform
            .pre_concat(Transform::from_rotate(angle.to_degrees() as f32));
        Ok(())
    }

    fn set_fill_style_str(&self, value: &str) {
        self.inner.state.borrow_mut().fill = parse_color(value);
    }

    fn set_stroke_style_str(&self, value: &str) {
        self.inner.state.borrow_mut().stroke = parse_color(value);
    }

    fn set_line_width(&self, width: f64) {
        if width.is_finite() && width > 0.0 {
            self.inner.state.borrow_mut().line_width = width as f32;
        }
    }

    fn set_global_alpha(&self, alpha: f64) {
        if alpha.is_finite() {
            self.inner.state.borrow_mut().global_alpha = alpha.clamp(0.0, 1.0) as f32;
        }
    }

    fn set_line_dash(&self, segments: &[f64]) {
        self.inner.state.borrow_mut().dash = segments
            .iter()
            .filter(|segment| segment.is_finite() && **segment >= 0.0)
            .map(|segment| *segment as f32)
            .collect();
    }

    // Shadows and blur are not drawn yet; see the module docs.
    fn set_shadow_color(&self, _value: &str) {}
    fn set_shadow_blur(&self, _value: f64) {}
    fn set_shadow_offset_x(&self, _value: f64) {}
    fn set_shadow_offset_y(&self, _value: f64) {}
    fn set_blur_filter(&self, _radius_px: f64) {}

    fn set_font(&self, value: &str) {
        let fonts = &self.inner.fonts;
        let cached = fonts.resolved.borrow().get(value).cloned();
        let (font, size, descriptor) = cached.unwrap_or_else(|| {
            let (weight, size, families) = parse_font(value);
            let resolved = (fonts.resolve(&families, weight), size, Rc::from(value));
            fonts
                .resolved
                .borrow_mut()
                .insert(Box::from(value), resolved.clone());
            resolved
        });
        let mut state = self.inner.state.borrow_mut();
        state.font = font;
        state.font_size_px = size;
        state.font_descriptor = descriptor;
    }

    fn set_text_baseline(&self, value: &str) {
        self.inner.state.borrow_mut().baseline = TextBaseline::parse(value);
    }

    fn fill_text(&self, text: &str, x: f64, y: f64) -> Result<(), Self::Error> {
        self.draw_text(text, x, y, false);
        Ok(())
    }

    fn stroke_text(&self, text: &str, x: f64, y: f64) -> Result<(), Self::Error> {
        self.draw_text(text, x, y, true);
        Ok(())
    }

    fn measure_text_width(&self, text: &str) -> Result<f64, Self::Error> {
        if let Some(width) = self.host_measure(text, |host, text, font| host.width(text, font)) {
            return Ok(width);
        }
        let size = self.inner.state.borrow().font_size_px;
        Ok(f64::from(self.text_width(size, text)))
    }

    fn measure_text_metrics(&self, text: &str) -> Result<TextMetrics, Self::Error> {
        if let Some(metrics) = self.host_measure(text, |host, text, font| host.metrics(text, font))
        {
            return Ok(metrics);
        }
        let (font, size) = {
            let state = self.inner.state.borrow();
            (state.font, state.font_size_px)
        };
        let width = self.text_width(size, text);
        let (ascent, descent) = self.font_extent(font, size);
        // The ink box: each glyph's bounding box, at its pen position, in
        // the face that draws it.
        let (mut top, mut bottom, mut right) = (0.0_f32, 0.0_f32, 0.0_f32);
        let mut pen = 0.0_f32;
        for (run_font, run) in self.segments(text) {
            let glyphs = self.glyphs(run_font, size, run);
            let units = self
                .inner
                .fonts
                .faces
                .get(run_font)
                .map(|face| face.units_per_em)
                .unwrap_or(1000.0);
            let scale = size / units;
            self.inner.fonts.with_face(run_font, |face| {
                for (glyph, advance) in &glyphs {
                    if let Some(rect) = face.glyph_bounding_box(*glyph) {
                        top = top.max(f32::from(rect.y_max) * scale);
                        bottom = bottom.max(-f32::from(rect.y_min) * scale);
                        right = right.max(pen + f32::from(rect.x_max) * scale);
                    }
                    pen += advance;
                }
            });
        }
        Ok(TextMetrics {
            width: f64::from(width),
            font_bounding_box_ascent: f64::from(ascent.round()),
            font_bounding_box_descent: f64::from(descent.round()),
            actual_bounding_box_ascent: f64::from(top),
            actual_bounding_box_descent: f64::from(bottom),
            actual_bounding_box_right: f64::from(right),
        })
    }

    fn fill_rect(&self, x: f64, y: f64, width: f64, height: f64) {
        let Some(rect) = Rect::from_xywh(x as f32, y as f32, width as f32, height as f32) else {
            return;
        };
        let (color, transform) = {
            let state = self.inner.state.borrow();
            (state.fill, state.transform)
        };
        let paint = self.paint(color);
        let color = self.color(color);
        let opaque = (color.alpha() >= 1.0).then_some(color);
        self.fill_rect_clipped(rect, &paint, transform, opaque);
    }

    /// Paints the rectangle opaque paper rather than transparent: the Skia
    /// backend does, because the renderers clear a page to paper, and a
    /// canvas only gets away with transparency by having a white page behind
    /// it in CSS.
    fn clear_rect(&self, x: f64, y: f64, width: f64, height: f64) {
        if self.inner.blank.get() {
            return;
        }
        let Some(rect) = Rect::from_xywh(x as f32, y as f32, width as f32, height as f32) else {
            return;
        };
        let transform = self.inner.state.borrow().transform;
        let mut paint = Paint::default();
        let paper = self.paper();
        paint.set_color(paper);
        paint.blend_mode = BlendMode::Source;
        self.fill_rect_clipped(rect, &paint, transform, Some(paper));
    }

    fn stroke_rect(&self, x: f64, y: f64, width: f64, height: f64) {
        let Some(rect) = Rect::from_xywh(x as f32, y as f32, width as f32, height as f32) else {
            return;
        };
        let (color, transform) = {
            let state = self.inner.state.borrow();
            (state.stroke, state.transform)
        };
        let half = self.inner.state.borrow().line_width / 2.0;
        if rect_outline(rect, half)
            .is_some_and(|rects| self.fill_stroke_rects(&rects, color, transform))
        {
            return;
        }
        self.stroke_path_with(&PathBuilder::from_rect(rect), color, transform);
    }

    fn begin_path(&self) {
        *self.inner.path.borrow_mut() = PathBuilder::new();
    }

    fn close_path(&self) {
        self.inner.path.borrow_mut().close();
    }

    fn move_to(&self, x: f64, y: f64) {
        self.inner.path.borrow_mut().move_to(x as f32, y as f32);
    }

    fn line_to(&self, x: f64, y: f64) {
        let mut path = self.inner.path.borrow_mut();
        // Canvas treats a line from nowhere as a move.
        if path.is_empty() {
            path.move_to(x as f32, y as f32);
        } else {
            path.line_to(x as f32, y as f32);
        }
    }

    fn rect(&self, x: f64, y: f64, width: f64, height: f64) {
        let mut path = self.inner.path.borrow_mut();
        let (x, y, w, h) = (x as f32, y as f32, width as f32, height as f32);
        path.move_to(x, y);
        path.line_to(x + w, y);
        path.line_to(x + w, y + h);
        path.line_to(x, y + h);
        path.close();
    }

    fn arc(
        &self,
        x: f64,
        y: f64,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
    ) -> Result<(), Self::Error> {
        self.ellipse(x, y, radius, radius, 0.0, start_angle, end_angle)
    }

    fn quadratic_curve_to(&self, cp_x: f64, cp_y: f64, x: f64, y: f64) {
        self.inner
            .path
            .borrow_mut()
            .quad_to(cp_x as f32, cp_y as f32, x as f32, y as f32);
    }

    fn bezier_curve_to(&self, cp1_x: f64, cp1_y: f64, cp2_x: f64, cp2_y: f64, x: f64, y: f64) {
        self.inner.path.borrow_mut().cubic_to(
            cp1_x as f32,
            cp1_y as f32,
            cp2_x as f32,
            cp2_y as f32,
            x as f32,
            y as f32,
        );
    }

    fn ellipse(
        &self,
        x: f64,
        y: f64,
        radius_x: f64,
        radius_y: f64,
        rotation: f64,
        start_angle: f64,
        end_angle: f64,
    ) -> Result<(), Self::Error> {
        self.ellipse_with_anticlockwise(
            x,
            y,
            radius_x,
            radius_y,
            rotation,
            start_angle,
            end_angle,
            false,
        )
    }

    fn ellipse_with_anticlockwise(
        &self,
        x: f64,
        y: f64,
        radius_x: f64,
        radius_y: f64,
        rotation: f64,
        start_angle: f64,
        end_angle: f64,
        anticlockwise: bool,
    ) -> Result<(), Self::Error> {
        if radius_x.is_finite() && radius_y.is_finite() && radius_x >= 0.0 && radius_y >= 0.0 {
            self.arc_segments(
                x,
                y,
                radius_x,
                radius_y,
                rotation,
                start_angle,
                end_angle,
                anticlockwise,
            );
        }
        Ok(())
    }

    fn fill(&self) {
        let Some(path) = self.current_path() else {
            return;
        };
        let (color, transform) = {
            let state = self.inner.state.borrow();
            (state.fill, state.transform)
        };
        self.fill_path_with(&path, color, transform);
    }

    fn stroke(&self) {
        let Some(path) = self.current_path() else {
            return;
        };
        let (color, transform) = {
            let state = self.inner.state.borrow();
            (state.stroke, state.transform)
        };
        self.stroke_path_with(&path, color, transform);
    }

    fn clip(&self) {
        let size = {
            let pixmap = self.inner.pixmap.borrow();
            (pixmap.width(), pixmap.height())
        };
        let path = self.current_path();
        let mut state = self.inner.state.borrow_mut();
        // Clipping to an empty path clips everything away.
        let clip = Clip::new(state.clip.as_deref(), path.as_ref(), state.transform, size);
        state.clip = Some(Rc::new(clip));
    }

    fn use_fill_style_for_stroke(&self) {
        let mut state = self.inner.state.borrow_mut();
        state.stroke = state.fill;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fonts() -> Rc<FontBook> {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../jade-core/fonts/Noto_Sans_JP/static/NotoSansJP-Regular.ttf"
        );
        let mut book = FontBook::new();
        book.register("Noto Sans JP", 400, Arc::new(std::fs::read(path).unwrap()))
            .unwrap();
        Rc::new(book)
    }

    fn pixel(painter: &TinySkiaPainter, x: u32, y: u32) -> [u8; 4] {
        let data = painter.rgba_premultiplied();
        let index = ((y * painter.width() + x) * 4) as usize;
        [
            data[index],
            data[index + 1],
            data[index + 2],
            data[index + 3],
        ]
    }

    fn thai() -> Arc<Vec<u8>> {
        Arc::new(
            std::fs::read(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../jade-core/fonts/Noto_Sans_Thai/NotoSansThai-VariableFont_wdth,wght.ttf"
            ))
            .unwrap(),
        )
    }

    fn width(fonts: Rc<FontBook>, font: &str, text: &str) -> f64 {
        let painter = TinySkiaPainter::new(1, 1, fonts).unwrap();
        painter.set_font(font);
        painter.measure_text_width(text).unwrap()
    }

    /// Each character falls back to a face that has it, as a browser's
    /// text does: Thai after Latin in one run is measured — and drawn — in
    /// the Thai face, not as the styled face's missing-glyph boxes.
    #[test]
    fn a_character_the_face_lacks_falls_back_on_its_own() {
        let mut book = FontBook::new();
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../jade-core/fonts/Noto_Sans_JP/static/NotoSansJP-Regular.ttf"
        );
        book.register("Noto Sans JP", 400, Arc::new(std::fs::read(path).unwrap()))
            .unwrap();
        book.register("Noto Sans Thai", 400, thai()).unwrap();
        let both = Rc::new(book);
        let mut thai_only = FontBook::new();
        thai_only.register("Noto Sans Thai", 400, thai()).unwrap();
        let thai_only = Rc::new(thai_only);

        let latin = width(Rc::clone(&both), "16px Noto Sans JP", "Total ");
        let baht = width(thai_only, "16px Noto Sans Thai", "บาท");
        let mixed = width(Rc::clone(&both), "16px Noto Sans JP", "Total บาท");
        assert!(
            (mixed - (latin + baht)).abs() < 0.01,
            "{mixed} vs {latin} + {baht}"
        );

        // And it draws: ink where the Thai is.
        let painter = TinySkiaPainter::new(120, 24, both).unwrap();
        painter.set_font("16px Noto Sans JP");
        painter.set_fill_style_str("#000");
        painter.fill_text("Total บาท", 0.0, 18.0).unwrap();
        let start = latin.ceil() as u32;
        let inked = (start..start + baht.floor() as u32)
            .flat_map(|x| (0..24).map(move |y| (x, y)))
            .filter(|&(x, y)| pixel(&painter, x, y)[0] < 128)
            .count();
        assert!(inked > 20, "{inked}");
    }

    /// A one-colour 4 × 4 PNG.
    fn solid_png(rgba: [u8; 4]) -> Vec<u8> {
        let mut pixmap = Pixmap::new(4, 4).unwrap();
        pixmap.fill(Color::from_rgba8(rgba[0], rgba[1], rgba[2], rgba[3]));
        pixmap.encode_png().unwrap()
    }

    fn draw_whole(painter: &TinySkiaPainter, key: &str, bytes: &[u8]) {
        let destination = ImageRect {
            x: 0.0,
            y: 0.0,
            width: 8.0,
            height: 8.0,
        };
        assert!(painter.draw_encoded_image_rect(key, bytes, None, destination));
    }

    /// Every slide over a decoded background, and every repaint after an
    /// edit, draws on a new surface: the picture is decoded once for all of
    /// them, as the glyphs are rasterised once.
    #[test]
    fn a_picture_is_decoded_once_for_every_surface() {
        let fonts = fonts();
        let red = solid_png([255, 0, 0, 255]);
        for _ in 0..3 {
            let painter = TinySkiaPainter::new(8, 8, Rc::clone(&fonts)).unwrap();
            draw_whole(&painter, "ppt/media/image1.png", &red);
            assert_eq!(pixel(&painter, 4, 4), [255, 0, 0, 255]);
        }
        assert_eq!(fonts.pictures.borrow().len(), 1);
    }

    /// The key is a part name, which the next document reuses for its own
    /// picture: that one draws, not the first document's.
    #[test]
    fn the_same_part_name_in_another_document_draws_its_own_picture() {
        let fonts = fonts();
        let painter = TinySkiaPainter::new(8, 8, Rc::clone(&fonts)).unwrap();
        draw_whole(
            &painter,
            "ppt/media/image1.png",
            &solid_png([255, 0, 0, 255]),
        );
        let painter = TinySkiaPainter::new(8, 8, Rc::clone(&fonts)).unwrap();
        draw_whole(
            &painter,
            "ppt/media/image1.png",
            &solid_png([0, 0, 255, 255]),
        );
        assert_eq!(pixel(&painter, 4, 4), [0, 0, 255, 255]);
    }

    /// Word converts WMF/EMF pictures (old equations among them) to SVG; the
    /// painter draws them as vectors at the size they land at, text in a
    /// font the page does not have falling back to the registered face.
    #[test]
    fn svg_pictures_draw_at_their_destination_with_their_text() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50">
            <rect x="0" y="0" width="50" height="50" fill="#ff0000"/>
            <text x="55" y="40" font-family="Cambria Math" font-size="40" fill="#000000">x</text>
        </svg>"##;
        let painter = TinySkiaPainter::new(240, 120, fonts()).unwrap();
        painter.clear_rect(0.0, 0.0, 240.0, 120.0);
        assert!(painter.draw_encoded_image_rect(
            "svg",
            svg,
            None,
            ImageRect {
                x: 20.0,
                y: 10.0,
                width: 200.0,
                height: 100.0,
            },
        ));
        // The rectangle is the left half of the destination, scaled 2×.
        assert_eq!(pixel(&painter, 60, 60), [255, 0, 0, 255]);
        assert_eq!(pixel(&painter, 10, 60), [255, 255, 255, 255]);
        // The glyph lands in the right half.
        let data = painter.rgba_premultiplied();
        let ink = (120..220)
            .flat_map(|x| (10..110).map(move |y| (x, y)))
            .filter(|&(x, y)| data[((y * 240 + x) * 4) as usize] < 128)
            .count();
        assert!(ink > 50, "the text drew {ink} dark pixels");
    }

    fn dark_pixels(painter: &TinySkiaPainter) -> usize {
        painter
            .rgba_premultiplied()
            .chunks_exact(4)
            .filter(|pixel| pixel[0] < 128)
            .count()
    }

    /// Word's equations write letters as Mathematical Alphanumeric Symbols
    /// (𝑃 for an italic P); a face without them draws the base letter,
    /// slanted for the italic styles, rather than a missing-glyph box.
    #[test]
    fn math_letters_a_face_lacks_draw_as_their_base_letters() {
        let draw = |text: &str| {
            let painter = TinySkiaPainter::new(200, 60, fonts()).unwrap();
            painter.clear_rect(0.0, 0.0, 200.0, 60.0);
            painter.set_font("40px 'Noto Sans JP'");
            painter.set_fill_style_str("#000000");
            painter.fill_text(text, 10.0, 45.0).unwrap();
            painter
        };
        let plain = dark_pixels(&draw("Pab"));
        let italic = draw("\u{1D443}\u{1D44E}\u{1D44F}");
        let ink = dark_pixels(&italic);
        assert!(plain > 200);
        // Same letters, slanted: about the same ink, not three boxes.
        assert!(
            (ink as f64 - plain as f64).abs() < plain as f64 * 0.25,
            "plain {plain}, italic {ink}"
        );
        assert_eq!(
            italic.measure_text_width("\u{1D443}").unwrap(),
            draw("P").measure_text_width("P").unwrap()
        );
        assert_eq!(math_letter('\u{1D7D0}'), Some(('2', true, false)));
        assert_eq!(math_letter('\u{210E}'), Some(('h', false, true)));
        assert_eq!(math_letter('P'), None);
    }

    #[test]
    fn fills_honour_the_transform_and_the_saved_state() {
        let painter = TinySkiaPainter::new(40, 40, fonts()).unwrap();
        painter.save();
        painter.translate(10.0, 10.0).unwrap();
        painter.set_fill_style_str("#ff0000");
        painter.fill_rect(0.0, 0.0, 10.0, 10.0);
        painter.restore();
        painter.fill_rect(30.0, 30.0, 5.0, 5.0);
        assert_eq!(pixel(&painter, 15, 15), [255, 0, 0, 255]);
        assert_eq!(pixel(&painter, 5, 5), [255, 255, 255, 255]);
        // The fill colour was restored to black with the rest of the state.
        assert_eq!(pixel(&painter, 32, 32), [0, 0, 0, 255]);
    }

    /// What is drawn is reported as damaged rows, once; what the clip hides
    /// is not.
    #[test]
    fn drawing_reports_the_rows_it_touched() {
        let painter = TinySkiaPainter::new(100, 100, fonts()).unwrap();
        assert_eq!(painter.take_damaged_rows(), None);
        painter.fill_rect(10.0, 20.0, 30.0, 10.0);
        assert_eq!(painter.take_damaged_rows(), Some(19..31));
        assert_eq!(painter.take_damaged_rows(), None);
        painter.save();
        painter.begin_path();
        painter.rect(0.0, 50.0, 100.0, 10.0);
        painter.clip();
        painter.fill_rect(0.0, 0.0, 100.0, 100.0);
        painter.set_font("12px 'Noto Sans JP'");
        painter.fill_text("あ", 10.0, 58.0).unwrap();
        painter.restore();
        assert_eq!(painter.take_damaged_rows(), Some(49..61));
        assert_eq!(painter.bgra_premultiplied_rows(49..61).len(), 100 * 12 * 4);
    }

    /// A rectangle clipped to, under no transform at all (tiny-skia's
    /// `is_scale_translate` says no to the identity), is kept as a rectangle.
    #[test]
    fn a_rectangle_clip_is_kept_as_a_rectangle() {
        let mut builder = PathBuilder::new();
        builder.push_rect(Rect::from_ltrb(0.0, 130.0, 793.6, 231.0).unwrap());
        let path = builder.finish().unwrap();
        assert_eq!(
            rect_of(&path, Transform::identity()),
            Some([0.0, 130.0, 793.6, 231.0])
        );
        assert_eq!(
            rect_of(&path, Transform::from_scale(2.0, 2.0)),
            Some([0.0, 260.0, 1587.2, 462.0])
        );
        assert_eq!(
            rect_of(&path, Transform::from_row(1.0, 0.1, 0.0, 1.0, 0.0, 0.0)),
            None
        );
    }

    /// A stroke and a fill across the edges of a rectangle clip, drawn by
    /// the rasteriser on a layer of their own, come out exactly as drawing
    /// them through a mask of the clip over the whole surface does.
    #[test]
    fn drawing_across_a_rectangle_clip_matches_a_whole_mask() {
        let (width, height) = (120, 90);
        let clip = Rect::from_ltrb(10.5, 7.25, 50.3, 40.7).unwrap();
        let scale = Transform::from_scale(2.0, 2.0);
        let mut line = PathBuilder::new();
        line.move_to(0.0, 20.0);
        line.line_to(60.0, 30.0);
        let line = line.finish().unwrap();
        let disc = PathBuilder::from_circle(45.0, 38.0, 12.0).unwrap();

        let painter = TinySkiaPainter::new(width, height, fonts()).unwrap();
        painter.scale(2.0, 2.0).unwrap();
        painter.begin_path();
        painter.rect(
            f64::from(clip.left()),
            f64::from(clip.top()),
            f64::from(clip.width()),
            f64::from(clip.height()),
        );
        painter.clip();
        // Through the layer itself: paths filled by area never reach it.
        let rasterise = |path: &Path, color: &str, stroke: Option<&Stroke>| {
            let paint = painter.paint(parse_color(color));
            let bounds = TinySkiaPainter::stroke_bounds(path, 3.0);
            painter.clipped(bounds, scale, |pixmap, transform, mask| match stroke {
                Some(stroke) => pixmap.stroke_path(path, &paint, stroke, transform, mask),
                None => pixmap.fill_path(path, &paint, FillRule::Winding, transform, mask),
            });
        };
        let line_stroke = Stroke {
            width: 3.0,
            ..Stroke::default()
        };
        rasterise(&line, "#204080", Some(&line_stroke));
        rasterise(&disc, "rgba(200, 30, 30, 0.6)", None);

        let mut expected = Pixmap::new(width, height).unwrap();
        expected.fill(Color::WHITE);
        let mut mask = Mask::new(width, height).unwrap();
        mask.fill_path(
            &PathBuilder::from_rect(clip),
            FillRule::Winding,
            true,
            scale,
        );
        let mut paint = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        paint.set_color(parse_color("#204080"));
        let stroke = Stroke {
            width: 3.0,
            ..Stroke::default()
        };
        expected.stroke_path(&line, &paint, &stroke, scale, Some(&mask));
        paint.set_color(parse_color("rgba(200, 30, 30, 0.6)"));
        expected.fill_path(&disc, &paint, FillRule::Winding, scale, Some(&mask));

        let drawn = painter.rgba_premultiplied();
        let far = drawn
            .chunks_exact(4)
            .zip(expected.data().chunks_exact(4))
            .filter(|(a, b)| a.iter().zip(b.iter()).any(|(a, b)| a != b))
            .count();
        assert_eq!(far, 0, "{far} pixels differ");
        let inked = drawn.chunks_exact(4).filter(|pixel| pixel[0] < 200).count();
        assert!(inked > 100, "{inked} inked pixels");
    }

    /// Straight strokes along the axes — gridlines, cell borders, boxes —
    /// are filled as rectangles, and land where the path rasteriser puts
    /// them, at fractional scales too. A translucent box still goes
    /// through the rasteriser, its corners blended once.
    #[test]
    fn axis_aligned_strokes_match_the_rasteriser() {
        let (width, height) = (160, 120);
        for scale in [1.0_f32, 1.5, 2.0] {
            let painter = TinySkiaPainter::new(width, height, fonts()).unwrap();
            painter.scale(f64::from(scale), f64::from(scale)).unwrap();
            let mut expected = Pixmap::new(width, height).unwrap();
            expected.fill(Color::WHITE);
            let transform = Transform::from_scale(scale, scale);
            let mut paint = Paint {
                anti_alias: true,
                ..Paint::default()
            };

            // A grid in one path, opaque, on the half pixel as sheets draw it.
            let mut grid = PathBuilder::new();
            painter.begin_path();
            for step in 0..6 {
                let at = 10.0 + step as f32 * 13.0 + 0.5;
                for (from, to) in [((at, 5.0), (at, 75.0)), ((5.0, at), (90.0, at))] {
                    grid.move_to(from.0, from.1);
                    grid.line_to(to.0, to.1);
                    painter.move_to(f64::from(from.0), f64::from(from.1));
                    painter.line_to(f64::from(to.0), f64::from(to.1));
                }
            }
            painter.set_line_width(1.0);
            painter.set_stroke_style_str("#c0c0c0");
            painter.stroke();
            paint.set_color(parse_color("#c0c0c0"));
            let stroke = Stroke::default();
            expected.stroke_path(&grid.finish().unwrap(), &paint, &stroke, transform, None);

            // A thick border off the pixel grid, then a translucent box.
            let mut border = PathBuilder::new();
            border.move_to(20.3, 40.0);
            border.line_to(80.6, 40.0);
            let border = border.finish().unwrap();
            painter.begin_path();
            painter.move_to(20.3, 40.0);
            painter.line_to(80.6, 40.0);
            painter.set_line_width(2.5);
            painter.set_stroke_style_str("#102030");
            painter.stroke();
            let stroke = Stroke {
                width: 2.5,
                ..Stroke::default()
            };
            paint.set_color(parse_color("#102030"));
            expected.stroke_path(&border, &paint, &stroke, transform, None);

            let rect = Rect::from_xywh(30.0, 50.0, 40.0, 25.0).unwrap();
            painter.set_line_width(3.0);
            painter.set_stroke_style_str("rgba(200, 30, 30, 0.5)");
            painter.stroke_rect(30.0, 50.0, 40.0, 25.0);
            let stroke = Stroke {
                width: 3.0,
                ..Stroke::default()
            };
            paint.set_color(parse_color("rgba(200, 30, 30, 0.5)"));
            expected.stroke_path(
                &PathBuilder::from_rect(rect),
                &paint,
                &stroke,
                transform,
                None,
            );

            let drawn = painter.rgba_premultiplied();
            let worst = drawn
                .iter()
                .zip(expected.data())
                .map(|(a, b)| a.abs_diff(*b))
                .max()
                .unwrap();
            // The rasteriser counts coverage in quarters of a pixel; filled
            // rectangles take the exact area, so an edge pixel can differ
            // by a quarter of the ink's contrast with the paper.
            assert!(worst <= 64, "scale {scale}: worst difference {worst}");
        }
    }

    /// A glyph's coverage is each pixel's exact area inside the outline:
    /// whole inside, the fraction at an edge, nothing in a hole wound the
    /// other way.
    #[test]
    fn area_coverage_is_the_area_inside() {
        let mut ring = PathBuilder::new();
        for (x, y) in [(1.25, 1.0), (9.75, 1.0), (9.75, 9.5), (1.25, 9.5)] {
            if x == 1.25 && y == 1.0 {
                ring.move_to(x, y);
            } else {
                ring.line_to(x, y);
            }
        }
        ring.close();
        // The hole, the other way round.
        ring.move_to(4.0, 4.0);
        ring.line_to(4.0, 7.0);
        ring.line_to(7.0, 7.0);
        ring.line_to(7.0, 4.0);
        ring.close();
        let ring = ring.finish().unwrap();
        let alpha = area_coverage(&ring, Transform::identity(), 11, 11);
        let at = |x: usize, y: usize| alpha[y * 11 + x];
        assert_eq!(at(0, 5), 0);
        assert_eq!(at(1, 5), 191, "three quarters of the left edge column");
        assert_eq!(at(2, 5), 255);
        assert_eq!(at(9, 5), 191);
        assert_eq!(at(5, 9), 128, "half of the bottom edge row");
        assert_eq!(at(5, 5), 0, "inside the hole");
        assert_eq!(at(3, 5), 255);
    }

    /// Glyphs come out nearer their true coverage — the outline drawn
    /// sixteen times finer and averaged — than the path rasteriser's
    /// sixteen samples to a pixel put them, with the same ink.
    #[test]
    fn glyph_coverage_is_nearer_the_outline_than_the_rasteriser() {
        let fonts = fonts();
        let scale = 28.0 / 1000.0;
        let (width, height, fine) = (36, 40, 16);
        let paint = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        for (index, ch) in "秘密保持契約書はAgé".chars().enumerate() {
            let glyph = fonts
                .with_face(0, |face| face.glyph_index(ch))
                .flatten()
                .unwrap();
            let outline = fonts.outline(0, glyph).unwrap();
            let transform =
                Transform::from_row(scale, 0.0, 0.0, -scale, 2.0 + index as f32 / 4.0, 30.0);
            let rasterised = {
                let mut pixmap = Pixmap::new(width, height).unwrap();
                pixmap.fill_path(&outline, &paint, FillRule::Winding, transform, None);
                pixmap
                    .data()
                    .iter()
                    .skip(3)
                    .step_by(4)
                    .copied()
                    .collect::<Vec<u8>>()
            };
            let mut finer = Pixmap::new(width * fine, height * fine).unwrap();
            let finer_transform = transform.post_scale(fine as f32, fine as f32);
            finer.fill_path(&outline, &paint, FillRule::Winding, finer_transform, None);
            let (width, fine) = (width as usize, fine as usize);
            let truth: Vec<f64> = (0..width * height as usize)
                .map(|index| {
                    let (x, y) = (index % width, index / width);
                    let rows = y * fine..(y + 1) * fine;
                    let sum: f64 = rows
                        .flat_map(|row| {
                            let start = (row * width * fine + x * fine) * 4;
                            finer.data()[start..start + fine * 4]
                                .iter()
                                .skip(3)
                                .step_by(4)
                        })
                        .map(|&alpha| f64::from(alpha))
                        .sum();
                    sum / (fine * fine) as f64
                })
                .collect();
            let error = |alpha: &[u8]| -> f64 {
                alpha
                    .iter()
                    .zip(&truth)
                    .map(|(alpha, truth)| (f64::from(*alpha) - truth).abs())
                    .sum()
            };
            let drawn = area_coverage(&outline, transform, width, height as usize);
            assert!(
                error(&drawn) < error(&rasterised),
                "{ch}: off by {} where the rasteriser is off by {}",
                error(&drawn),
                error(&rasterised)
            );
            let ink = drawn.iter().map(|&alpha| f64::from(alpha)).sum::<f64>();
            let ratio = ink / truth.iter().sum::<f64>();
            assert!((0.98..1.02).contains(&ratio), "{ch}: ink {ratio}");
        }
    }

    /// The table blends exactly as scaling each channel does, over the
    /// paper and over anything else.
    #[test]
    fn blend_tables_blend_as_the_channels_scale() {
        let paper = [255, 255, 255, 255];
        let sources = [
            [0, 0, 0, 255],
            [32, 64, 128, 255],
            [100, 15, 15, 128],
            [0, 0, 0, 0],
        ];
        let unders = [paper, [0, 0, 0, 255], [17, 200, 90, 255], [40, 40, 40, 100]];
        for source in sources {
            let table = BlendTable::new(source, paper);
            for under in unders {
                for alpha in 0..=255u8 {
                    let keep = 255 - scale_u8(source[3], alpha);
                    let expected: [u8; 4] = std::array::from_fn(|channel| {
                        scale_u8(source[channel], alpha)
                            .saturating_add(scale_u8(under[channel], keep))
                    });
                    let mut pixel = under;
                    table.blend(&mut pixel, alpha);
                    assert_eq!(pixel, expected, "{source:?} at {alpha} over {under:?}");
                }
            }
        }
    }

    /// Clearing a surface nothing has been drawn on leaves it as it is,
    /// and notes no damage; once something is drawn, clearing paints the
    /// paper over it again.
    #[test]
    fn clearing_a_blank_surface_does_nothing() {
        let painter = TinySkiaPainter::new(20, 20, fonts()).unwrap();
        painter.take_damaged_rows();
        painter.clear_rect(0.0, 0.0, 20.0, 20.0);
        assert_eq!(painter.take_damaged_rows(), None);
        painter.set_fill_style_str("#000000");
        painter.fill_rect(5.0, 5.0, 4.0, 4.0);
        assert_eq!(pixel(&painter, 6, 6), [0, 0, 0, 255]);
        painter.clear_rect(0.0, 0.0, 20.0, 20.0);
        assert_eq!(pixel(&painter, 6, 6), [255, 255, 255, 255]);
        painter.set_dark_paper(true);
        let dark = pixel(&painter, 6, 6);
        assert_ne!(dark, [255, 255, 255, 255]);
        painter.fill_rect(5.0, 5.0, 4.0, 4.0);
        painter.clear_rect(0.0, 0.0, 20.0, 20.0);
        assert_eq!(pixel(&painter, 6, 6), dark);
    }

    /// The paper is painted in when the surface is first read or drawn
    /// on, in the paper it has then; an opaque fill over every pixel takes
    /// its place.
    #[test]
    fn the_paper_is_painted_in_when_first_needed() {
        let painter = TinySkiaPainter::new(8, 8, fonts()).unwrap();
        assert_eq!(pixel(&painter, 3, 3), [255, 255, 255, 255]);

        let dark = TinySkiaPainter::new(8, 8, fonts()).unwrap();
        dark.set_dark_paper(true);
        let paper = dark.paper().premultiply().to_color_u8();
        let paper = [paper.red(), paper.green(), paper.blue(), paper.alpha()];
        dark.set_fill_style_str("#ff0000");
        dark.fill_rect(2.0, 2.0, 2.0, 2.0);
        assert_eq!(pixel(&dark, 0, 0), paper);
        assert_ne!(pixel(&dark, 2, 2), paper);

        // The paper's colour on a blank surface leaves it blank: nothing to
        // read back, and a clear after it has nothing to do.
        let papered = TinySkiaPainter::new(8, 8, fonts()).unwrap();
        papered.take_damaged_rows();
        papered.set_fill_style_str("#ffffff");
        papered.fill_rect(0.0, 0.0, 6.0, 6.0);
        assert_eq!(papered.take_damaged_rows(), None);
        assert_eq!(pixel(&papered, 3, 3), [255, 255, 255, 255]);

        let covered = TinySkiaPainter::new(8, 8, fonts()).unwrap();
        covered.set_fill_style_str("#336699");
        covered.fill_rect(-1.0, -1.0, 10.0, 10.0);
        let data = covered.rgba_premultiplied();
        assert!(data
            .chunks_exact(4)
            .all(|pixel| pixel == [0x33, 0x66, 0x99, 255]));
    }

    /// Paths drawn across a rectangle clip's edge are filled by area: what
    /// lies outside the clip is untouched, and inside they land where the
    /// rasteriser puts them, give or take its sixteen samples to a pixel.
    #[test]
    fn paths_across_a_clip_are_filled_by_area_inside_it() {
        let (width, height) = (120, 90);
        let clip = Rect::from_ltrb(10.5, 7.25, 50.3, 40.7).unwrap();
        let scale = Transform::from_scale(2.0, 2.0);
        let mut line = PathBuilder::new();
        line.move_to(0.0, 20.0);
        line.line_to(30.0, 32.0);
        line.line_to(60.0, 30.0);
        let line = line.finish().unwrap();
        let disc = PathBuilder::from_circle(45.0, 38.0, 12.0).unwrap();

        let painter = TinySkiaPainter::new(width, height, fonts()).unwrap();
        painter.scale(2.0, 2.0).unwrap();
        painter.begin_path();
        painter.rect(
            f64::from(clip.left()),
            f64::from(clip.top()),
            f64::from(clip.width()),
            f64::from(clip.height()),
        );
        painter.clip();
        painter.set_line_width(3.0);
        painter.stroke_path_with(&line, parse_color("#204080"), scale);
        painter.fill_path_with(&disc, parse_color("rgba(200, 30, 30, 0.6)"), scale);

        let mut expected = Pixmap::new(width, height).unwrap();
        expected.fill(Color::WHITE);
        let mut mask = Mask::new(width, height).unwrap();
        mask.fill_path(
            &PathBuilder::from_rect(clip),
            FillRule::Winding,
            true,
            scale,
        );
        let mut paint = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        paint.set_color(parse_color("#204080"));
        let stroke = Stroke {
            width: 3.0,
            ..Stroke::default()
        };
        expected.stroke_path(&line, &paint, &stroke, scale, Some(&mask));
        paint.set_color(parse_color("rgba(200, 30, 30, 0.6)"));
        expected.fill_path(&disc, &paint, FillRule::Winding, scale, Some(&mask));

        let drawn = painter.rgba_premultiplied();
        let device = [21, 14, 101, 82];
        for (index, (a, b)) in drawn
            .chunks_exact(4)
            .zip(expected.data().chunks_exact(4))
            .enumerate()
        {
            let (x, y) = (index % width as usize, index / width as usize);
            let outside = x < device[0] || y < device[1] || x >= device[2] || y >= device[3];
            if outside {
                assert_eq!(a, [255, 255, 255, 255], "({x}, {y}) is outside the clip");
            }
            let worst = a.iter().zip(b).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
            assert!(worst <= 64, "({x}, {y}): {a:?} against {b:?}");
        }
        let inked = drawn.chunks_exact(4).filter(|pixel| pixel[0] < 200).count();
        assert!(inked > 100, "{inked} inked pixels");
    }

    #[test]
    fn a_clip_bounds_later_drawing_until_restore() {
        let painter = TinySkiaPainter::new(40, 40, fonts()).unwrap();
        painter.save();
        painter.begin_path();
        painter.rect(0.0, 0.0, 20.0, 40.0);
        painter.clip();
        painter.set_fill_style_str("rgb(0, 0, 255)");
        painter.fill_rect(0.0, 0.0, 40.0, 40.0);
        painter.restore();
        assert_eq!(pixel(&painter, 10, 10), [0, 0, 255, 255]);
        assert_eq!(pixel(&painter, 30, 10), [255, 255, 255, 255]);
    }

    /// The paragraph mark is not in the Japanese face, so it comes from the
    /// painter's own mark glyphs rather than drawing as tofu.
    #[test]
    fn a_paragraph_mark_missing_from_every_face_is_drawn_as_an_arrow() {
        let painter = TinySkiaPainter::new(40, 50, fonts()).unwrap();
        painter.set_font("30px 'Noto Sans JP'");
        // One em wide, as the mark face's advance says.
        let width = painter.measure_text_width("\u{21B5}").unwrap();
        assert!((width - 30.0).abs() < 0.01, "{width}");
        painter.fill_text("\u{21B5}", 0.0, 40.0).unwrap();
        let dark = |x, y| pixel(&painter, x, y)[0] < 128;
        // The stem on the right and the head on the left, and nothing above
        // the head where a tofu box would have its side.
        assert!(dark(22, 25));
        assert!(dark(8, 34));
        assert!(!dark(12, 22));
        assert!(!dark(3, 22));
    }

    #[test]
    fn text_measures_by_advances_and_draws_ink() {
        let painter = TinySkiaPainter::new(200, 60, fonts()).unwrap();
        painter.set_font("12pt 'Noto Sans JP'");
        let one = painter.measure_text_width("あ").unwrap();
        let two = painter.measure_text_width("ああ").unwrap();
        // A full-width glyph is one em: 12pt is 16px.
        assert!((one - 16.0).abs() < 0.01, "{one}");
        assert!((two - 2.0 * one).abs() < 1e-9);
        painter.fill_text("日本語 text", 4.0, 40.0).unwrap();
        let data = painter.rgba_premultiplied();
        let dark = data.chunks_exact(4).filter(|pixel| pixel[0] < 128).count();
        assert!(dark > 50, "{dark} dark pixels");
        let metrics = painter.measure_text_metrics("Hg").unwrap();
        assert!(metrics.font_bounding_box_ascent > metrics.actual_bounding_box_descent);
        assert!(metrics.actual_bounding_box_descent > 0.0);
    }

    #[test]
    fn css_colours_parse_like_the_skia_backend() {
        assert_eq!(parse_color("#fff"), Color::WHITE);
        assert_eq!(parse_color("#00ff0080").alpha(), 128.0 / 255.0);
        assert_eq!(parse_color("rgba(255, 0, 0, 0.5)").alpha(), 0.5);
        assert_eq!(parse_color("nonsense"), Color::BLACK);
        assert_eq!(parse_font("bold 10.5pt 'Yu Gothic', serif").0, 700);
        assert_eq!(parse_font("14px Arial").1, 14.0);
    }

    #[test]
    fn arcs_close_into_a_filled_circle() {
        let painter = TinySkiaPainter::new(40, 40, fonts()).unwrap();
        painter.begin_path();
        painter
            .arc(20.0, 20.0, 10.0, 0.0, std::f64::consts::TAU)
            .unwrap();
        painter.fill();
        assert_eq!(pixel(&painter, 20, 20), [0, 0, 0, 255]);
        assert_eq!(pixel(&painter, 2, 2), [255, 255, 255, 255]);
    }

    #[test]
    fn dark_paper_maps_colours_and_clears_dark_until_switched_back() {
        let painter = TinySkiaPainter::new(40, 40, fonts()).unwrap();
        painter.set_dark_paper(true);
        let paper = jade_painter::paper::to_dark_paper_u8([255, 255, 255]);
        let ink = jade_painter::paper::to_dark_paper_u8([0, 0, 0]);
        let near = |actual: [u8; 4], expected: [u8; 3]| {
            actual[3] == 255 && (0..3).all(|i| actual[i].abs_diff(expected[i]) <= 1)
        };
        assert!(
            near(pixel(&painter, 2, 2), paper),
            "{:?}",
            pixel(&painter, 2, 2)
        );
        // Black ink, the canvas default, comes out light.
        painter.fill_rect(10.0, 10.0, 5.0, 5.0);
        assert!(
            near(pixel(&painter, 12, 12), ink),
            "{:?}",
            pixel(&painter, 12, 12)
        );
        painter.clear_rect(10.0, 10.0, 5.0, 5.0);
        assert!(near(pixel(&painter, 12, 12), paper));

        painter.set_dark_paper(false);
        assert_eq!(pixel(&painter, 2, 2), [255, 255, 255, 255]);
        painter.fill_rect(10.0, 10.0, 5.0, 5.0);
        assert_eq!(pixel(&painter, 12, 12), [0, 0, 0, 255]);
    }
}

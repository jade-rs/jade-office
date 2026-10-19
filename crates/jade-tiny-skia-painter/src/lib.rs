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

#[derive(Default)]
pub struct FontBook {
    faces: Vec<Face>,
    outlines: RefCell<HashMap<(usize, u16), Option<Path>>>,
    /// Glyphs rasterised once, shared by every page: see [`GlyphCoverage`].
    glyphs: RefCell<HashMap<GlyphKey, Option<Rc<GlyphCoverage>>>>,
    /// Each character's glyph and advance per face, and the face a run
    /// starting with it falls back to. Asking the face means parsing its
    /// table directory, which per character was a tenth of a repaint.
    characters: RefCell<HashMap<(usize, char), CharGlyph>>,
    covering: RefCell<HashMap<(usize, char), usize>>,
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

    /// `font`, unless it lacks `ch` and another face has it.
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
            .unwrap_or(font)
    }

    /// The glyph's outline in font units, y up, cached.
    fn outline(&self, font: usize, glyph: ttf_parser::GlyphId) -> Option<Path> {
        if let Some(cached) = self.outlines.borrow().get(&(font, glyph.0)) {
            return cached.clone();
        }
        let path = self
            .with_face(font, |face| {
                let mut builder = OutlineBuilder(PathBuilder::new());
                face.outline_glyph(glyph, &mut builder)?;
                builder.0.finish()
            })
            .flatten();
        self.outlines
            .borrow_mut()
            .insert((font, glyph.0), path.clone());
        path
    }
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
/// A quarter pixel is as fine as the phase goes, so a glyph lands within an
/// eighth of a pixel of where its outline would.
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
/// the quarter-pixel phase of the pen.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct GlyphKey {
    font: usize,
    glyph: u16,
    matrix: [u32; 3],
    bold: bool,
    phase: (u8, u8),
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
    let mut pixmap = Pixmap::new(width, height)?;
    let local = transform.post_translate(-left as f32, -top as f32);
    let mut paint = Paint::default();
    paint.set_color(Color::BLACK);
    paint.anti_alias = true;
    pixmap.fill_path(outline, &paint, FillRule::Winding, local, None);
    if let Some(width) = bold_width {
        let stroke = Stroke {
            width,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        pixmap.stroke_path(outline, &paint, &stroke, local, None);
    }
    // Trimmed to its ink: the margin left for anti-aliasing is mostly
    // empty, and every empty pixel kept is one more visited per draw.
    let alpha: Vec<u8> = pixmap
        .data()
        .chunks_exact(4)
        .map(|pixel| pixel[3])
        .collect();
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
        let mut pixmap = Pixmap::new(width.max(1), height.max(1))?;
        pixmap.fill(Color::WHITE);
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
            }),
        })
    }

    /// Notes that drawing may have changed the pixels within `bounds`
    /// (device px), less whatever the clip hides of them.
    fn note_damage(&self, bounds: Bounds, clip: Option<&Clip>) {
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
        let pixmap = self.inner.pixmap.borrow();
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
        let pixmap = self.inner.pixmap.borrow();
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
        let paper = self.paper();
        let mut pixmap = self.inner.pixmap.borrow_mut();
        pixmap.fill(paper);
        let whole = [0.0, 0.0, pixmap.width() as f32, pixmap.height() as f32];
        drop(pixmap);
        self.note_damage(whole, None);
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
        self.inner.pixmap.borrow().data().to_vec()
    }

    /// The pixels as BGRA, premultiplied: what gpui's `RenderImage` holds.
    pub fn bgra_premultiplied(&self) -> Vec<u8> {
        // One pass, read and written as words: a page at 2x is millions of
        // pixels, read back after every edit that repaints it.
        self.bgra_premultiplied_rows(0..self.height())
    }

    pub fn encode_png(&self) -> Option<Vec<u8>> {
        self.inner.pixmap.borrow().encode_png().ok()
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
        let mut pixmap = self.inner.pixmap.borrow_mut();
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
            return self.clipped(rect, transform, |pixmap, mask| {
                pixmap.fill_rect(rect, paint, transform, mask);
            });
        }
        let mut pixmap = self.inner.pixmap.borrow_mut();
        let surface = [0.0, 0.0, pixmap.width() as f32, pixmap.height() as f32];
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

    /// Runs `draw` with the mask the clip needs for drawing within `bounds`
    /// (local, placed by `transform`), or not at all when the clip hides it.
    fn clipped(
        &self,
        bounds: Rect,
        transform: Transform,
        draw: impl FnOnce(&mut Pixmap, Option<&Mask>),
    ) {
        let clip = self.inner.state.borrow().clip.clone();
        let device = device_bounds(bounds, transform);
        let mut pixmap = self.inner.pixmap.borrow_mut();
        match clip.as_deref() {
            None => draw(&mut pixmap, None),
            Some(clip) => match clip.coverage(device) {
                Coverage::Hidden => return,
                Coverage::Whole => draw(&mut pixmap, None),
                Coverage::Partial => draw(&mut pixmap, Some(clip.whole_mask())),
            },
        }
        drop(pixmap);
        self.note_damage(device, clip.as_deref());
    }

    fn fill_path_with(&self, path: &Path, color: Color, transform: Transform) {
        let paint = self.paint(color);
        self.clipped(path.bounds(), transform, |pixmap, mask| {
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
        let paint = self.paint(color);
        let stroke = self.stroke_style();
        let bounds = Self::stroke_bounds(path, stroke.width);
        self.clipped(bounds, transform, |pixmap, mask| {
            pixmap.stroke_path(path, &paint, &stroke, transform, mask);
        });
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
        self.clipped(bounds, transform, |pixmap, mask| {
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

    /// The face a run is drawn and measured in: the styled one, or the first
    /// that has the run's first character — the Skia backend's rule.
    fn run_font(&self, text: &str) -> usize {
        let font = self.inner.state.borrow().font;
        match text.chars().next() {
            Some(first) => self.inner.fonts.covering(font, first),
            None => font,
        }
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
        if missing {
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
        let (y, phase_y) = phase(transform.ty);
        let key = GlyphKey {
            font,
            glyph: glyph.0,
            matrix: [
                transform.sx.to_bits(),
                transform.kx.to_bits(),
                transform.sy.to_bits(),
            ],
            bold: bold_width.is_some(),
            phase: (phase_x, phase_y),
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
                    f32::from(phase_y) / 4.0,
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
        let mut pixmap = self.inner.pixmap.borrow_mut();
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
        // How much of pixel `index` along an axis lies within `from..to`.
        let overlap = |index: i32, from: f32, to: f32| -> u8 {
            let (start, end) = (index as f32, index as f32 + 1.0);
            ((end.min(to) - start.max(from)).clamp(0.0, 1.0) * 255.0).round() as u8
        };
        let source = color.premultiply().to_color_u8();
        let source = [source.red(), source.green(), source.blue(), source.alpha()];
        let opaque = source[3] == 255;
        let blend = |pixel: &mut [u8], alpha: u8| {
            if alpha == 255 && opaque {
                pixel.copy_from_slice(&source);
                return;
            }
            let keep = 255 - scale_u8(source[3], alpha);
            for (channel, value) in pixel.iter_mut().enumerate() {
                *value = scale_u8(source[channel], alpha).saturating_add(scale_u8(*value, keep));
            }
        };
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
            let pixels = &mut data[first * 4..(surface_row + columns.end as usize) * 4];
            match (&edges, mask) {
                // The usual case, kept free of per-pixel checks.
                (None, None) => {
                    for (&alpha, pixel) in alphas.iter().zip(pixels.chunks_exact_mut(4)) {
                        if alpha != 0 {
                            blend(pixel, alpha);
                        }
                    }
                }
                (Some((rect, _, _)), _) => {
                    let row_cover = overlap(row, rect[1], rect[3]);
                    for ((column, &alpha), pixel) in
                        columns.clone().zip(alphas).zip(pixels.chunks_exact_mut(4))
                    {
                        if alpha != 0 {
                            let cover = scale_u8(row_cover, overlap(column, rect[0], rect[2]));
                            blend(pixel, scale_u8(alpha, cover));
                        }
                    }
                }
                (None, Some(mask)) => {
                    let mask = &mask[first..first + alphas.len()];
                    for ((&alpha, &clip), pixel) in
                        alphas.iter().zip(mask).zip(pixels.chunks_exact_mut(4))
                    {
                        if alpha != 0 {
                            blend(pixel, scale_u8(alpha, clip));
                        }
                    }
                }
            }
        }
    }

    fn draw_text(&self, text: &str, x: f64, y: f64, stroke: bool) {
        if text.is_empty() {
            return;
        }
        let font = self.run_font(text);
        let (size, baseline, transform, color) = {
            let state = self.inner.state.borrow();
            (
                state.font_size_px,
                state.baseline,
                state.transform,
                if stroke { state.stroke } else { state.fill },
            )
        };
        let (ascent, descent) = self.font_extent(font, size);
        let baseline_y = y as f32 + baseline.offset(ascent, descent);
        let units = self
            .inner
            .fonts
            .faces
            .get(font)
            .map(|face| face.units_per_em)
            .unwrap_or(1000.0);
        let scale = size / units;
        let mut pen = x as f32;
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
        let (weight, size, families) = parse_font(value);
        let font = self.inner.fonts.resolve(&families, weight);
        let mut state = self.inner.state.borrow_mut();
        state.font = font;
        state.font_size_px = size;
        state.font_descriptor = Rc::from(value);
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
        let (font, size) = {
            let state = self.inner.state.borrow();
            (state.font, state.font_size_px)
        };
        Ok(self
            .glyphs(font, size, text)
            .iter()
            .map(|(_, advance)| f64::from(*advance))
            .sum())
    }

    fn measure_text_metrics(&self, text: &str) -> Result<TextMetrics, Self::Error> {
        let (font, size) = {
            let state = self.inner.state.borrow();
            (state.font, state.font_size_px)
        };
        let glyphs = self.glyphs(font, size, text);
        let width: f32 = glyphs.iter().map(|(_, advance)| *advance).sum();
        let (ascent, descent) = self.font_extent(font, size);
        let units = self
            .inner
            .fonts
            .faces
            .get(font)
            .map(|face| face.units_per_em)
            .unwrap_or(1000.0);
        let scale = size / units;
        // The ink box: each glyph's bounding box, at its pen position.
        let (mut top, mut bottom, mut right) = (0.0_f32, 0.0_f32, 0.0_f32);
        let mut pen = 0.0_f32;
        self.inner.fonts.with_face(font, |face| {
            for (glyph, advance) in &glyphs {
                if let Some(rect) = face.glyph_bounding_box(*glyph) {
                    top = top.max(f32::from(rect.y_max) * scale);
                    bottom = bottom.max(-f32::from(rect.y_min) * scale);
                    right = right.max(pen + f32::from(rect.x_max) * scale);
                }
                pen += advance;
            }
        });
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

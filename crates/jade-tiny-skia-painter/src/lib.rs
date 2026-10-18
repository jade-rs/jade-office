//! A `jade_painter::Painter` drawn by tiny-skia, in pure Rust.
//!
//! The engine's renderers paint through [`jade_painter::Painter`], and the
//! native shell hands them a Skia surface. Skia is C++, though, and that is
//! what keeps those renderers out of a `wasm32-unknown-unknown` build. This
//! backend implements the same trait with crates that compile anywhere:
//!
//! - tiny-skia rasterises paths, fills, strokes, dashes and clips;
//! - ttf-parser supplies glyph outlines and advances, so text is drawn as
//!   paths — there is no font rasteriser to port;
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

use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::Infallible;
use std::rc::Rc;
use std::sync::Arc;

use jade_painter::{ImageRect, ImageSource, Painter, TextMetrics};
use tiny_skia::{
    BlendMode, Color, FillRule, LineCap, LineJoin, Mask, Paint, Path, PathBuilder, Pixmap,
    PixmapPaint, Rect, Stroke, StrokeDash, Transform,
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
#[derive(Default)]
pub struct FontBook {
    faces: Vec<Face>,
    outlines: RefCell<HashMap<(usize, u16), Option<Path>>>,
}

impl FontBook {
    pub fn new() -> Self {
        Self::default()
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
    clip: Option<Rc<Mask>>,
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
    /// Decoded pictures by the caller's content key; `None` for bytes that
    /// would not decode, so they are not retried every frame.
    images: RefCell<HashMap<String, Option<Rc<Pixmap>>>>,
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
                images: RefCell::new(HashMap::new()),
            }),
        })
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
        let mut data = self.rgba_premultiplied();
        for pixel in data.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        data
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
        let image = {
            let mut images = self.inner.images.borrow_mut();
            images
                .entry(key.to_string())
                .or_insert_with(|| decode(bytes).map(Rc::new))
                .clone()
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
        let mask = match (&crop, &state.clip) {
            (Some(crop), clip) => {
                let mut mask = match clip {
                    Some(clip) => (**clip).clone(),
                    None => full_mask(pixmap.width(), pixmap.height()),
                };
                mask.intersect_path(crop, FillRule::Winding, true, state.transform);
                Some(mask)
            }
            (None, clip) => clip.as_deref().cloned(),
        };
        let paint = PixmapPaint {
            opacity: state.global_alpha,
            blend_mode: BlendMode::SourceOver,
            quality: tiny_skia::FilterQuality::Bilinear,
        };
        pixmap.draw_pixmap(
            0,
            0,
            image.as_ref().as_ref(),
            &paint,
            transform,
            mask.as_ref(),
        );
        true
    }

    fn color(&self, color: Color) -> Color {
        let alpha = self.inner.state.borrow().global_alpha;
        let mut color = color;
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

    fn fill_path_with(&self, path: &Path, color: Color, transform: Transform) {
        let paint = self.paint(color);
        let clip = self.inner.state.borrow().clip.clone();
        self.inner.pixmap.borrow_mut().fill_path(
            path,
            &paint,
            FillRule::Winding,
            transform,
            clip.as_deref(),
        );
    }

    fn stroke_path_with(&self, path: &Path, color: Color, transform: Transform) {
        let paint = self.paint(color);
        let stroke = self.stroke_style();
        let clip = self.inner.state.borrow().clip.clone();
        self.inner.pixmap.borrow_mut().stroke_path(
            path,
            &paint,
            &stroke,
            transform,
            clip.as_deref(),
        );
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
        let units = self
            .inner
            .fonts
            .faces
            .get(font)
            .map(|face| face.units_per_em)
            .unwrap_or(1000.0);
        self.inner
            .fonts
            .with_face(font, |face| {
                text.chars()
                    .map(|ch| {
                        let glyph = face.glyph_index(ch).unwrap_or(ttf_parser::GlyphId(0));
                        let advance = f32::from(face.glyph_hor_advance(glyph).unwrap_or(0));
                        (glyph, advance / units * size)
                    })
                    .collect()
            })
            .unwrap_or_default()
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
        for (glyph, advance) in self.glyphs(font, size, text) {
            if let Some(outline) = self.inner.fonts.outline(font, glyph) {
                // Font units, y up → px at the pen, y down.
                let glyph_transform = transform.pre_concat(Transform::from_row(
                    scale, 0.0, 0.0, -scale, pen, baseline_y,
                ));
                if stroke {
                    self.stroke_path_with(&outline, color, glyph_transform);
                } else {
                    self.fill_path_with(&outline, color, glyph_transform);
                }
            }
            pen += advance;
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

fn full_mask(width: u32, height: u32) -> Mask {
    let mut mask = Mask::new(width, height).expect("a mask the size of the surface");
    if let Some(rect) = Rect::from_xywh(0.0, 0.0, width as f32, height as f32) {
        mask.fill_path(
            &PathBuilder::from_rect(rect),
            FillRule::Winding,
            false,
            Transform::identity(),
        );
    }
    mask
}

/// Decodes PNG, JPEG, GIF or BMP bytes to a premultiplied pixmap.
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
        let (color, transform, clip) = {
            let state = self.inner.state.borrow();
            (state.fill, state.transform, state.clip.clone())
        };
        let paint = self.paint(color);
        self.inner
            .pixmap
            .borrow_mut()
            .fill_rect(rect, &paint, transform, clip.as_deref());
    }

    /// Paints the rectangle opaque white rather than transparent: the Skia
    /// backend does, because the renderers clear a page to paper, and a
    /// canvas only gets away with transparency by having a white page behind
    /// it in CSS.
    fn clear_rect(&self, x: f64, y: f64, width: f64, height: f64) {
        let Some(rect) = Rect::from_xywh(x as f32, y as f32, width as f32, height as f32) else {
            return;
        };
        let (transform, clip) = {
            let state = self.inner.state.borrow();
            (state.transform, state.clip.clone())
        };
        let mut paint = Paint::default();
        paint.set_color(Color::WHITE);
        paint.blend_mode = BlendMode::Source;
        self.inner
            .pixmap
            .borrow_mut()
            .fill_rect(rect, &paint, transform, clip.as_deref());
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
        let (width, height) = {
            let pixmap = self.inner.pixmap.borrow();
            (pixmap.width(), pixmap.height())
        };
        let path = self.current_path();
        let mut state = self.inner.state.borrow_mut();
        let mut mask = match &state.clip {
            Some(clip) => (**clip).clone(),
            None => full_mask(width, height),
        };
        match path {
            Some(path) => mask.intersect_path(&path, FillRule::Winding, true, state.transform),
            // Clipping to an empty path clips everything away.
            None => mask = Mask::new(width, height).expect("a mask the size of the surface"),
        }
        state.clip = Some(Rc::new(mask));
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
}

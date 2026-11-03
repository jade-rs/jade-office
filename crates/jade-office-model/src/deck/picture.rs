//! PowerPoint for Mac's 図の形式 tab, through the model both shells share:
//! 図の変更, 修整, 色, 透明度, アート効果, 背景の削除, 図のスタイル, 図の
//! 枠線, 図の効果, トリミング (handles, 縦横比, 図形に合わせて), 回転 and
//! 反転, サイズ with 縦横比を固定, 図のリセット, 図の圧縮, plus SVG
//! pictures, アイコン, スクリーンショット and a フォト アルバム. Each
//! command is one Undo.

pub use jade_pptx_reader::SourceRect;
pub use jade_pptx_writer::{
    PictureEdit, PictureEffect, PictureFormatInfo, PictureStyle, Recolor, PICTURE_STYLES,
};

use super::{Deck, ShapeRect};
use crate::Backend;

/// 色 › 色の変更's presets, as the gallery lists them: Japanese and
/// English labels and the edit.
pub fn recolor_presets() -> Vec<(String, String, Recolor)> {
    let mut presets: Vec<(String, String, Recolor)> = [
        ("色変更なし", "No Recolor", Recolor::None),
        ("グレースケール", "Grayscale", Recolor::Grayscale),
        ("セピア", "Sepia", Recolor::Sepia),
        ("ウォッシュアウト", "Washout", Recolor::Washout),
        ("白黒: 25%", "Black and White: 25%", Recolor::BlackWhite(25)),
        ("白黒: 50%", "Black and White: 50%", Recolor::BlackWhite(50)),
        ("白黒: 75%", "Black and White: 75%", Recolor::BlackWhite(75)),
    ]
    .into_iter()
    .map(|(ja, en, recolor)| (ja.to_string(), en.to_string(), recolor))
    .collect();
    for n in 1..=6 {
        let accent = format!("accent{n}");
        presets.push((
            format!("アクセント {n} (濃色)"),
            format!("Accent {n} Dark"),
            Recolor::Dark(accent.clone()),
        ));
        presets.push((
            format!("アクセント {n} (淡色)"),
            format!("Accent {n} Light"),
            Recolor::Light(accent),
        ));
    }
    presets
}

/// アート効果's gallery: the effect's element name and label.
/// (element, Japanese, English).
pub const ARTISTIC_EFFECTS: &[(&str, &str, &str)] = &[
    ("artisticMarker", "マーカー", "Marker"),
    ("artisticPencilGrayscale", "鉛筆: 白黒", "Pencil Grayscale"),
    ("artisticPencilSketch", "鉛筆: スケッチ", "Pencil Sketch"),
    ("artisticLineDrawing", "線画", "Line Drawing"),
    ("artisticChalkSketch", "チョーク: スケッチ", "Chalk Sketch"),
    ("artisticPaintStrokes", "塗りつぶし: 描線", "Paint Strokes"),
    ("artisticPaintBrush", "絵筆", "Paint Brush"),
    ("artisticGlowDiffused", "光彩: 拡散", "Glow Diffused"),
    ("artisticBlur", "ぼかし", "Blur"),
    ("artisticLightScreen", "明るいスクリーン", "Light Screen"),
    (
        "artisticWatercolorSponge",
        "水彩画: スポンジ",
        "Watercolor Sponge",
    ),
    ("artisticFilmGrain", "フィルム粒子", "Film Grain"),
    (
        "artisticMosaicBubbles",
        "モザイク: バブル",
        "Mosaic Bubbles",
    ),
    ("artisticGlass", "ガラス", "Glass"),
    ("artisticCement", "セメント", "Cement"),
    ("artisticTexturizer", "テクスチャライザー", "Texturizer"),
    (
        "artisticCrisscrossEtching",
        "十字模様: エッチング",
        "Crisscross Etching",
    ),
    (
        "artisticPastelsSmooth",
        "パステル: 滑らか",
        "Pastels Smooth",
    ),
    ("artisticPlasticWrap", "ラップ", "Plastic Wrap"),
    ("artisticCutout", "カットアウト", "Cutout"),
    ("artisticPhotocopy", "コピー", "Photocopy"),
    ("artisticGlowEdges", "光彩: エッジ", "Glow Edges"),
];

/// トリミング › 縦横比's presets: label and width:height.
pub const CROP_ASPECTS: &[(&str, f64, f64)] = &[
    ("1:1", 1.0, 1.0),
    ("2:3", 2.0, 3.0),
    ("3:4", 3.0, 4.0),
    ("3:5", 3.0, 5.0),
    ("4:5", 4.0, 5.0),
    ("3:2", 3.0, 2.0),
    ("4:3", 4.0, 3.0),
    ("5:3", 5.0, 3.0),
    ("5:4", 5.0, 4.0),
    ("16:9", 16.0, 9.0),
    ("16:10", 16.0, 10.0),
];

/// 図の圧縮's resolutions: label and pixels per inch.
/// (Japanese, English, ppi).
pub const COMPRESS_RESOLUTIONS: &[(&str, &str, u32)] = &[
    ("高品質: 330 ppi", "High fidelity: 330 ppi", 330),
    ("印刷用: 220 ppi", "Print: 220 ppi", 220),
    ("Web: 150 ppi", "Web: 150 ppi", 150),
    ("電子メール用: 96 ppi", "Email: 96 ppi", 96),
];

/// The icons 挿入 › アイコン offers: name and SVG.
/// (Japanese, English, SVG).
pub const ICONS: &[(&str, &str, &str)] = &[
    (
        "チェック",
        "Check",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 96 96" width="96" height="96"><path d="M36 64 18 46l-6 6 24 24 48-48-6-6z" fill="#000000"/></svg>"##,
    ),
    (
        "星",
        "Star",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 96 96" width="96" height="96"><path d="M48 6l12.4 28.6 31 2.8-23.5 20.4 7 30.4L48 72.2 21.1 88.2l7-30.4L4.6 37.4l31-2.8z" fill="#000000"/></svg>"##,
    ),
    (
        "ハート",
        "Heart",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 96 96" width="96" height="96"><path d="M48 84 41.6 78C18 56.6 6 45.8 6 31.6 6 20 15 11 26.6 11c6.6 0 12.9 3 17 7.9h8.8C56.5 14 62.8 11 69.4 11 81 11 90 20 90 31.6c0 14.2-12 25-35.6 46.4z" fill="#000000"/></svg>"##,
    ),
    (
        "電球",
        "Light bulb",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 96 96" width="96" height="96"><path d="M48 6C31.4 6 18 19.4 18 36c0 10.6 5.5 19.9 13.8 25.2V72h32.4V61.2C72.5 55.9 78 46.6 78 36 78 19.4 64.6 6 48 6zM34 78h28v6H34zm6 10h16v4H40z" fill="#000000"/></svg>"##,
    ),
    (
        "人",
        "Person",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 96 96" width="96" height="96"><circle cx="48" cy="26" r="18" fill="#000000"/><path d="M14 90c0-20 15.2-36 34-36s34 16 34 36z" fill="#000000"/></svg>"##,
    ),
    (
        "家",
        "House",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 96 96" width="96" height="96"><path d="M48 8 6 44h12v44h24V62h12v26h24V44h12z" fill="#000000"/></svg>"##,
    ),
    (
        "メール",
        "Mail",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 96 96" width="96" height="96"><path d="M8 18h80v60H8zm6 6v4l34 22 34-22v-4zm0 11v37h68V35L48 57z" fill="#000000"/></svg>"##,
    ),
    (
        "時計",
        "Clock",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 96 96" width="96" height="96"><path d="M48 6a42 42 0 1 0 0 84 42 42 0 0 0 0-84zm0 8a34 34 0 1 1 0 68 34 34 0 0 1 0-68zm-4 8v28l22 13 4-7-18-10.5V22z" fill="#000000"/></svg>"##,
    ),
    (
        "グラフ",
        "Chart",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 96 96" width="96" height="96"><path d="M8 84h80v6H8zm6-30h14v26H14zm22-20h14v46H36zm22-20h14v66H58z" fill="#000000"/></svg>"##,
    ),
    (
        "矢印",
        "Arrow",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 96 96" width="96" height="96"><path d="M8 42h58L44 20l8-8 36 36-36 36-8-8 22-22H8z" fill="#000000"/></svg>"##,
    ),
];

/// The picture's frame rect scaled so its width or height becomes the
/// given one; with the aspect ratio locked the other follows.
pub fn sized(rect: ShapeRect, width: Option<f64>, height: Option<f64>, locked: bool) -> ShapeRect {
    let ratio = if rect.height > 0.0 {
        rect.width / rect.height
    } else {
        1.0
    };
    let (mut w, mut h) = (rect.width, rect.height);
    if let Some(width) = width {
        w = width.max(1.0);
        if locked {
            h = w / ratio;
        }
    }
    if let Some(height) = height {
        h = height.max(1.0);
        if locked {
            w = h * ratio;
        }
    }
    ShapeRect {
        width: w,
        height: h,
        ..rect
    }
}

/// The largest centred crop of a picture shown `px_width` × `px_height`
/// with aspect `w:h`, in thousandths of a percent.
pub fn aspect_crop(px_width: f64, px_height: f64, w: f64, h: f64) -> SourceRect {
    let target = w / h;
    let own = px_width / px_height.max(1.0);
    let mut crop = SourceRect::default();
    if own > target {
        let keep = target / own;
        let side = ((1.0 - keep) / 2.0 * 100_000.0).round() as i32;
        crop.left = side;
        crop.right = side;
    } else if own < target {
        let keep = own / target;
        let side = ((1.0 - keep) / 2.0 * 100_000.0).round() as i32;
        crop.top = side;
        crop.bottom = side;
    }
    crop
}

impl<B: Backend> Deck<B> {
    fn picture_made(&mut self, result: jade_pptx_writer::Result<u32>) -> Result<u32, String> {
        let id = result.map_err(|error| error.to_string())?;
        self.reload()?;
        Ok(id)
    }

    /// What the 図の形式 tab shows for picture `id`, or `None` when it is
    /// no picture.
    pub fn picture_format(&self, slide: usize, id: u32) -> Option<PictureFormatInfo> {
        jade_pptx_writer::picture_format(self.session.bytes(), slide, id).ok()
    }

    /// The picture's crop, thousandths of a percent per side.
    pub fn picture_crop(&self, slide: usize, id: u32) -> Option<SourceRect> {
        jade_pptx_writer::picture_crop(self.session.bytes(), slide, id)
            .ok()
            .flatten()
            .map(|info| info.crop)
    }

    /// Whether 背景の削除 has been applied to picture `id`.
    pub fn picture_has_background_removed(&self, slide: usize, id: u32) -> bool {
        fn find(
            items: &[jade_pptx_reader::ShapeItem],
            id: u32,
        ) -> Option<&jade_pptx_reader::Picture> {
            items.iter().find_map(|item| match item {
                jade_pptx_reader::ShapeItem::Picture(picture) if picture.id == Some(id) => {
                    Some(picture)
                }
                jade_pptx_reader::ShapeItem::Group(group) => find(&group.children, id),
                _ => None,
            })
        }
        self.deck
            .slides
            .get(slide)
            .and_then(|s| find(&s.content.shapes, id))
            .is_some_and(|picture| {
                picture
                    .blip_effects
                    .iter()
                    .any(|e| matches!(e, jade_pptx_reader::BlipEffect::BackgroundRemoval(_)))
            })
    }

    /// One 図の形式 command.
    pub fn format_picture(
        &mut self,
        slide: usize,
        id: u32,
        edit: &PictureEdit,
    ) -> Result<bool, String> {
        let result = self.session.format_picture(slide, id, edit);
        self.after(result)
    }

    /// 図の変更: another picture in its place, its size and format kept.
    pub fn change_picture(&mut self, slide: usize, id: u32, image: &[u8]) -> Result<bool, String> {
        let info = jade_pptx_writer::picture_replacement(self.session.bytes(), slide, id)
            .map_err(|error| error.to_string())?
            .ok_or("not a picture")?;
        if !info.can_replace {
            return Err("this picture cannot be changed".into());
        }
        let result = self
            .session
            .replace_picture(slide, id, image, &info.fingerprint)
            .map(|replaced| replaced.changed);
        self.after(result)
    }

    /// 回転: turns shape `id` by `degrees` (右へ 90 度 = 90).
    pub fn rotate_picture(&mut self, slide: usize, id: u32, degrees: f64) -> Result<bool, String> {
        let transform = self
            .shape(slide, id)
            .and_then(|shape| shape.transform)
            .ok_or("no such shape")?;
        let current = f64::from(transform.rotation_60000ths) / 60_000.0;
        let turned = (current + degrees).rem_euclid(360.0);
        let next = jade_pptx_reader::Transform {
            rotation_60000ths: (turned * 60_000.0).round() as i32 % 21_600_000,
            ..transform
        };
        let result = self
            .session
            .set_shape_transform(slide, id, next)
            .map(|_| true);
        self.after(result)
    }

    /// 上下反転 / 左右反転.
    pub fn flip_picture(
        &mut self,
        slide: usize,
        id: u32,
        horizontal: bool,
    ) -> Result<bool, String> {
        let transform = self
            .shape(slide, id)
            .and_then(|shape| shape.transform)
            .ok_or("no such shape")?;
        let next = if horizontal {
            jade_pptx_reader::Transform {
                flip_h: !transform.flip_h,
                ..transform
            }
        } else {
            jade_pptx_reader::Transform {
                flip_v: !transform.flip_v,
                ..transform
            }
        };
        let result = self
            .session
            .set_shape_transform(slide, id, next)
            .map(|_| true);
        self.after(result)
    }

    /// サイズ: 高さ and 幅 in slide px; with 縦横比を固定 the other follows.
    pub fn set_picture_size(
        &mut self,
        slide: usize,
        id: u32,
        width: Option<f64>,
        height: Option<f64>,
    ) -> Result<bool, String> {
        let rect = self.shape_rect(slide, id).ok_or("no such shape")?;
        let locked = self
            .picture_format(slide, id)
            .is_some_and(|info| info.lock_aspect);
        let next = sized(rect, width, height, locked);
        let transform = self
            .shape(slide, id)
            .and_then(|shape| shape.transform)
            .ok_or("no such shape")?;
        let result = self
            .session
            .set_shape_transform(slide, id, next.apply_to(transform))
            .map(|_| true);
        self.after(result)
    }

    /// トリミング with the handles: the crop (thousandths of a percent per
    /// side, negative for an outset); the picture keeps its scale.
    pub fn crop_picture(
        &mut self,
        slide: usize,
        id: u32,
        crop: SourceRect,
    ) -> Result<bool, String> {
        let result = self.session.set_picture_crop(slide, id, crop, false);
        self.after(result)
    }

    /// トリミング › 縦横比: the largest centred part of the picture with
    /// that aspect, the frame following.
    pub fn crop_picture_to_aspect(
        &mut self,
        slide: usize,
        id: u32,
        width: f64,
        height: f64,
    ) -> Result<bool, String> {
        // The whole picture as the frame stretches it, before any crop.
        let rect = self.shape_rect(slide, id).ok_or("not a picture")?;
        let now = self.picture_crop(slide, id).ok_or("not a picture")?;
        let shown = |near: i32, far: i32| (1.0 - f64::from(near + far) / 100_000.0).max(0.01);
        let crop = aspect_crop(
            rect.width / shown(now.left, now.right),
            rect.height / shown(now.top, now.bottom),
            width,
            height,
        );
        self.crop_picture(slide, id, crop)
    }

    /// 図の圧縮: every picture, or only `only`, at `ppi`. How many got
    /// smaller.
    pub fn compress_pictures(
        &mut self,
        only: Option<(usize, u32)>,
        ppi: u32,
        delete_cropped: bool,
    ) -> Result<usize, String> {
        let count = self
            .session
            .compress_pictures(only, ppi, delete_cropped)
            .map_err(|error| error.to_string())?;
        if count > 0 {
            self.reload()?;
        }
        Ok(count)
    }

    /// 図形に変換 on an SVG picture: the group of freeforms it becomes.
    pub fn convert_svg_to_shapes(&mut self, slide: usize, id: u32) -> Result<u32, String> {
        let group = self
            .session
            .convert_svg_to_shapes(slide, id)
            .map_err(|error| error.to_string())?;
        self.reload()?;
        Ok(group)
    }

    /// 挿入 › 図 with an SVG: in the middle of the slide at its own size,
    /// shrunk to fit.
    pub fn insert_svg(&mut self, slide: usize, svg: &[u8]) -> Result<u32, String> {
        let (width, height) = jade_picture_effects::svg_size(svg).ok_or("not an SVG picture")?;
        self.insert_svg_sized(slide, svg, f64::from(width), f64::from(height))
    }

    fn insert_svg_sized(
        &mut self,
        slide: usize,
        svg: &[u8],
        width: f64,
        height: f64,
    ) -> Result<u32, String> {
        let (slide_width, slide_height) = self.slide_size();
        let (width, height) = (width.max(1.0), height.max(1.0));
        let fit = (slide_width * 0.9 / width)
            .min(slide_height * 0.9 / height)
            .min(1.0);
        let (width, height) = (width * fit, height * fit);
        let rect = ShapeRect {
            x: (slide_width - width) / 2.0,
            y: (slide_height - height) / 2.0,
            width,
            height,
        };
        let result = self.session.insert_svg_picture(slide, transform(rect), svg);
        self.picture_made(result)
    }

    /// 挿入 › アイコン: icon `index` of [`ICONS`], an inch square as
    /// PowerPoint places one.
    pub fn insert_icon(&mut self, slide: usize, index: usize) -> Result<u32, String> {
        let (_, _, svg) = ICONS.get(index).ok_or("no such icon")?;
        self.insert_svg_sized(slide, svg.as_bytes(), 96.0, 96.0)
    }

    /// A picture file of any kind PowerPoint takes: an SVG, or a bitmap
    /// (PNG, JPEG, GIF).
    pub fn insert_picture_file(&mut self, slide: usize, bytes: &[u8]) -> Result<u32, String> {
        if jade_picture_effects::is_svg(bytes) {
            self.insert_svg(slide, bytes)
        } else {
            self.insert_picture(slide, bytes)
        }
    }

    /// フォト アルバム: a new slide after `after` for each `per_slide`
    /// pictures, laid out in a grid. The first new slide's index.
    pub fn insert_photo_album(
        &mut self,
        after: usize,
        pictures: &[Vec<u8>],
        per_slide: usize,
    ) -> Result<usize, String> {
        let per_slide = per_slide.clamp(1, 4);
        let first = after + 1;
        let (slide_width, slide_height) = self.slide_size();
        for (page, chunk) in pictures.chunks(per_slide).enumerate() {
            let index = first + page;
            self.insert_slide(index)?;
            let columns = if chunk.len() > 1 { 2 } else { 1 };
            let rows = chunk.len().div_ceil(columns);
            let cell_width = slide_width / columns as f64;
            let cell_height = slide_height / rows as f64;
            for (n, image) in chunk.iter().enumerate() {
                let info = jade_pptx_writer::picture_info(image).map_err(|e| e.to_string())?;
                let (w, h) = (f64::from(info.width.max(1)), f64::from(info.height.max(1)));
                let fit = (cell_width * 0.9 / w).min(cell_height * 0.9 / h);
                let (w, h) = (w * fit, h * fit);
                let (column, row) = (n % columns, n / columns);
                let rect = ShapeRect {
                    x: cell_width * column as f64 + (cell_width - w) / 2.0,
                    y: cell_height * row as f64 + (cell_height - h) / 2.0,
                    width: w,
                    height: h,
                };
                let result = self
                    .session
                    .insert_picture(index, transform(rect), image)
                    .map(|inserted| inserted.id);
                self.picture_made(result)?;
            }
        }
        Ok(first)
    }

    /// Whether slide `index` shows an animated GIF, which a view redraws
    /// as it plays.
    pub fn has_animated_pictures(&mut self, index: usize) -> bool {
        let Some(layout) = self.layout(index) else {
            return false;
        };
        layout.shapes.iter().any(|shape| {
            shape.image_part.as_deref().is_some_and(|part| {
                part.ends_with(".gif")
                    && self
                        .parts
                        .get(part)
                        .and_then(|bytes| jade_picture_effects::gif_frames(bytes))
                        .is_some_and(|frames| frames.len() > 1)
            })
        })
    }
}

fn transform(rect: ShapeRect) -> jade_pptx_reader::Transform {
    rect.apply_to(jade_pptx_reader::Transform {
        offset_emu: (0, 0),
        extent_emu: (0, 0),
        rotation_60000ths: 0,
        flip_h: false,
        flip_v: false,
    })
}

thread_local! {
    static CLOCK_MS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static DELAYS: std::cell::RefCell<std::collections::HashMap<String, Vec<u32>>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Where animated GIFs are in their play, in ms: a view sets it before it
/// paints a slide.
pub fn set_picture_clock(ms: u64) {
    CLOCK_MS.with(|clock| clock.set(ms));
}

/// The key that draws the frame an animated GIF shows now; other keys as
/// they are.
pub(super) fn playing<'k>(key: &'k str, bytes: Option<&[u8]>) -> std::borrow::Cow<'k, str> {
    use std::borrow::Cow;
    if key.contains("#jade-fx=") || !key.to_ascii_lowercase().ends_with(".gif") {
        return Cow::Borrowed(key);
    }
    let delays = DELAYS.with(|cache| {
        if let Some(delays) = cache.borrow().get(key) {
            return delays.clone();
        }
        let delays: Vec<u32> = bytes
            .and_then(jade_picture_effects::gif_frames)
            .map(|frames| frames.into_iter().map(|(_, delay)| delay).collect())
            .unwrap_or_default();
        cache.borrow_mut().insert(key.to_string(), delays.clone());
        delays
    });
    let total: u64 = delays.iter().map(|d| u64::from(*d)).sum();
    if delays.len() < 2 || total == 0 {
        return Cow::Borrowed(key);
    }
    let mut at = CLOCK_MS.with(std::cell::Cell::get) % total;
    let mut frame = 0;
    for (index, delay) in delays.iter().enumerate() {
        if at < u64::from(*delay) {
            frame = index;
            break;
        }
        at -= u64::from(*delay);
    }
    Cow::Owned(jade_picture_effects::key(
        key,
        &[jade_picture_effects::Effect::Frame(frame)],
    ))
}

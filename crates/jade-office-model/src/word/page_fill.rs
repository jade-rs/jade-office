//! 塗りつぶし効果: the gradient, texture, pattern or picture Word puts
//! behind every page (デザイン › ページの色 › 塗りつぶし効果…), the dialog
//! that picks one and the command that puts it there.

use jade_docx_headless_api::{HeadlessCommand, HeadlessPageFill};

use super::WordSession;

/// The dialog's tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FillTab {
    /// グラデーション
    #[default]
    Gradient,
    /// テクスチャ
    Texture,
    /// パターン
    Pattern,
    /// 図
    Picture,
}

impl FillTab {
    pub const ALL: [FillTab; 4] = [
        FillTab::Gradient,
        FillTab::Texture,
        FillTab::Pattern,
        FillTab::Picture,
    ];

    pub fn label_key(self) -> &'static str {
        match self {
            FillTab::Gradient => "setup.fill.tab.gradient",
            FillTab::Texture => "setup.fill.tab.texture",
            FillTab::Pattern => "setup.fill.tab.pattern",
            FillTab::Picture => "setup.fill.tab.picture",
        }
    }
}

/// グラデーションの種類.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GradientStyle {
    /// 横: the colour changes from top to bottom.
    #[default]
    Horizontal,
    /// 縦: from left to right.
    Vertical,
    /// 斜め (右上がり).
    DiagonalUp,
    /// 斜め (右下がり).
    DiagonalDown,
}

impl GradientStyle {
    pub const ALL: [GradientStyle; 4] = [
        GradientStyle::Horizontal,
        GradientStyle::Vertical,
        GradientStyle::DiagonalUp,
        GradientStyle::DiagonalDown,
    ];

    /// VML's `angle`.
    pub fn angle(self) -> f64 {
        match self {
            GradientStyle::Horizontal => 0.0,
            GradientStyle::Vertical => -90.0,
            GradientStyle::DiagonalUp => 45.0,
            GradientStyle::DiagonalDown => -45.0,
        }
    }

    fn from_angle(angle: f64) -> Self {
        Self::ALL
            .into_iter()
            .find(|style| (style.angle() - angle).abs() < 0.5)
            .unwrap_or_default()
    }

    pub fn label_key(self) -> &'static str {
        match self {
            GradientStyle::Horizontal => "setup.fill.style.horizontal",
            GradientStyle::Vertical => "setup.fill.style.vertical",
            GradientStyle::DiagonalUp => "setup.fill.style.diagonal_up",
            GradientStyle::DiagonalDown => "setup.fill.style.diagonal_down",
        }
    }
}

/// The colours the dialog offers: black, then ページの色's.
pub fn fill_colors() -> impl Iterator<Item = (&'static str, &'static str)> {
    std::iter::once(("000000", "setup.fill.black")).chain(super::page_setup::PAGE_COLORS)
}

/// バリエーション: VML's `focus` — from the first colour, from the second,
/// the second in the middle, the first in the middle.
pub const VARIANTS: [f64; 4] = [0.0, 100.0, 50.0, -50.0];

/// テクスチャ: tiles made here, each with Word's name for its look.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Texture {
    Newsprint,
    RecycledPaper,
    PaperBag,
    Canvas,
    Denim,
    Cork,
    Sand,
    Granite,
}

impl Texture {
    pub const ALL: [Texture; 8] = [
        Texture::Newsprint,
        Texture::RecycledPaper,
        Texture::PaperBag,
        Texture::Canvas,
        Texture::Denim,
        Texture::Cork,
        Texture::Sand,
        Texture::Granite,
    ];

    pub fn label_key(self) -> &'static str {
        match self {
            Texture::Newsprint => "setup.fill.texture.newsprint",
            Texture::RecycledPaper => "setup.fill.texture.recycled_paper",
            Texture::PaperBag => "setup.fill.texture.paper_bag",
            Texture::Canvas => "setup.fill.texture.canvas",
            Texture::Denim => "setup.fill.texture.denim",
            Texture::Cork => "setup.fill.texture.cork",
            Texture::Sand => "setup.fill.texture.sand",
            Texture::Granite => "setup.fill.texture.granite",
        }
    }

    /// `o:title`, as Word names its textures.
    pub fn title(self) -> &'static str {
        match self {
            Texture::Newsprint => "Newsprint",
            Texture::RecycledPaper => "Recycled paper",
            Texture::PaperBag => "Paper bag",
            Texture::Canvas => "Canvas",
            Texture::Denim => "Denim",
            Texture::Cork => "Cork",
            Texture::Sand => "Sand",
            Texture::Granite => "Granite",
        }
    }

    /// The colour it is mostly, `RRGGBB`, for a swatch.
    pub fn swatch(self) -> u32 {
        let [r, g, b] = self.base();
        (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
    }

    fn base(self) -> [u8; 3] {
        match self {
            Texture::Newsprint => [0xE6, 0xE4, 0xDE],
            Texture::RecycledPaper => [0xE8, 0xDE, 0xC6],
            Texture::PaperBag => [0xC6, 0xA2, 0x72],
            Texture::Canvas => [0xDA, 0xD0, 0xB4],
            Texture::Denim => [0x4C, 0x6A, 0x98],
            Texture::Cork => [0xB2, 0x84, 0x50],
            Texture::Sand => [0xE2, 0xD2, 0xA6],
            Texture::Granite => [0x9C, 0x9A, 0x98],
        }
    }

    /// The tile, 64 × 64, as a PNG.
    pub fn png(self) -> Vec<u8> {
        const SIZE: u32 = 64;
        let base = self.base();
        let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
        for y in 0..SIZE {
            for x in 0..SIZE {
                // Noise that repeats with the tile.
                let grain = noise(x, y, self as u32) as i32 - 128;
                let blot = noise(x / 4, y / 4, 17 + self as u32) as i32 - 128;
                let shade = match self {
                    Texture::Newsprint => grain / 12,
                    Texture::RecycledPaper => grain / 10 + if grain > 120 { -40 } else { 0 },
                    Texture::PaperBag => grain / 10 + (noise(0, y, 5) as i32 - 128) / 8,
                    Texture::Canvas => grain / 14 + if x % 3 == 0 || y % 3 == 0 { -14 } else { 0 },
                    Texture::Denim => grain / 8 + if (x + y) % 4 < 2 { -18 } else { 10 },
                    Texture::Cork => grain / 6 + blot / 4,
                    Texture::Sand => grain / 6,
                    Texture::Granite => {
                        grain / 4
                            + if grain > 110 {
                                50
                            } else if grain < -115 {
                                -50
                            } else {
                                0
                            }
                    }
                };
                for channel in base {
                    rgba.push((i32::from(channel) + shade).clamp(0, 255) as u8);
                }
                rgba.push(255);
            }
        }
        super::signatures::rgba_png(SIZE, SIZE, &rgba).unwrap_or_default()
    }
}

/// A value 0–255 for a point, the same each time.
fn noise(x: u32, y: u32, seed: u32) -> u8 {
    let mut h = x
        .wrapping_mul(0x27d4_eb2d)
        .wrapping_add(y.wrapping_mul(0x1656_67b1))
        .wrapping_add(seed.wrapping_mul(0x9e37_79b9));
    h ^= h >> 15;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    (h >> 24) as u8
}

/// パターン: 8 × 8 bits, a set bit drawn in the foreground.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pattern {
    #[default]
    Percent5,
    Percent10,
    Percent25,
    Percent50,
    Percent75,
    Horizontal,
    Vertical,
    DownwardDiagonal,
    UpwardDiagonal,
    Cross,
    DiagonalCross,
    Checker,
}

impl Pattern {
    pub const ALL: [Pattern; 12] = [
        Pattern::Percent5,
        Pattern::Percent10,
        Pattern::Percent25,
        Pattern::Percent50,
        Pattern::Percent75,
        Pattern::Horizontal,
        Pattern::Vertical,
        Pattern::DownwardDiagonal,
        Pattern::UpwardDiagonal,
        Pattern::Cross,
        Pattern::DiagonalCross,
        Pattern::Checker,
    ];

    pub fn label_key(self) -> &'static str {
        match self {
            Pattern::Percent5 => "setup.fill.pattern.pct5",
            Pattern::Percent10 => "setup.fill.pattern.pct10",
            Pattern::Percent25 => "setup.fill.pattern.pct25",
            Pattern::Percent50 => "setup.fill.pattern.pct50",
            Pattern::Percent75 => "setup.fill.pattern.pct75",
            Pattern::Horizontal => "setup.fill.pattern.horizontal",
            Pattern::Vertical => "setup.fill.pattern.vertical",
            Pattern::DownwardDiagonal => "setup.fill.pattern.down_diagonal",
            Pattern::UpwardDiagonal => "setup.fill.pattern.up_diagonal",
            Pattern::Cross => "setup.fill.pattern.cross",
            Pattern::DiagonalCross => "setup.fill.pattern.diagonal_cross",
            Pattern::Checker => "setup.fill.pattern.checker",
        }
    }

    /// `o:title`.
    pub fn title(self) -> &'static str {
        match self {
            Pattern::Percent5 => "5%",
            Pattern::Percent10 => "10%",
            Pattern::Percent25 => "25%",
            Pattern::Percent50 => "50%",
            Pattern::Percent75 => "75%",
            Pattern::Horizontal => "Horizontal",
            Pattern::Vertical => "Vertical",
            Pattern::DownwardDiagonal => "Downward diagonal",
            Pattern::UpwardDiagonal => "Upward diagonal",
            Pattern::Cross => "Cross",
            Pattern::DiagonalCross => "Diagonal cross",
            Pattern::Checker => "Small checker board",
        }
    }

    /// Row by row, the high bit leftmost.
    pub fn bits(self) -> [u8; 8] {
        match self {
            Pattern::Percent5 => [0x80, 0, 0, 0, 0x08, 0, 0, 0],
            Pattern::Percent10 => [0x80, 0, 0x08, 0, 0x80, 0, 0x08, 0],
            Pattern::Percent25 => [0x88, 0, 0x22, 0, 0x88, 0, 0x22, 0],
            Pattern::Percent50 => [0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55],
            Pattern::Percent75 => [0x77, 0xFF, 0xDD, 0xFF, 0x77, 0xFF, 0xDD, 0xFF],
            Pattern::Horizontal => [0xFF, 0, 0, 0, 0xFF, 0, 0, 0],
            Pattern::Vertical => [0x88; 8],
            Pattern::DownwardDiagonal => [0x80, 0x40, 0x20, 0x10, 0x08, 0x04, 0x02, 0x01],
            Pattern::UpwardDiagonal => [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80],
            Pattern::Cross => [0xFF, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80],
            Pattern::DiagonalCross => [0x81, 0x42, 0x24, 0x18, 0x18, 0x24, 0x42, 0x81],
            Pattern::Checker => [0xCC, 0xCC, 0x33, 0x33, 0xCC, 0xCC, 0x33, 0x33],
        }
    }

    /// The bits black on white, 8 × 8, as a PNG: what Word stores, to be
    /// drawn in the foreground and background.
    pub fn png(self) -> Vec<u8> {
        let mut rgba = Vec::with_capacity(8 * 8 * 4);
        for row in self.bits() {
            for column in 0..8 {
                let value = if row & (0x80 >> column) != 0 { 0 } else { 255 };
                rgba.extend_from_slice(&[value, value, value, 255]);
            }
        }
        super::signatures::rgba_png(8, 8, &rgba).unwrap_or_default()
    }
}

/// A file brought for a texture or a picture: its name and bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FillFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// 塗りつぶし効果 as the dialog holds it.
#[derive(Clone, Debug, PartialEq)]
pub struct PageFillDialog {
    pub tab: FillTab,
    /// グラデーション: 色 1 and 色 2, `RRGGBB`.
    pub color: String,
    pub color2: String,
    pub style: GradientStyle,
    /// An index into [`VARIANTS`].
    pub variant: usize,
    /// テクスチャ: one of the made ones, or その他のテクスチャ… brought from a
    /// file.
    pub texture: Option<Texture>,
    pub texture_file: Option<FillFile>,
    /// パターン, 前景 and 背景.
    pub pattern: Pattern,
    pub foreground: String,
    pub background: String,
    /// 図: 図の選択….
    pub picture: Option<FillFile>,
}

impl PageFillDialog {
    /// The dialog over the effect there is, or a gradient from the page
    /// colour when there is none.
    pub fn open(
        current: Option<&jade_docx_reader::document::PageBackgroundFill>,
        page_color: Option<&str>,
    ) -> Self {
        let page = page_color
            .map(|color| color.trim_start_matches('#').to_ascii_uppercase())
            .unwrap_or_else(|| "FFFFFF".to_string());
        let mut dialog = Self {
            tab: FillTab::Gradient,
            color: page.clone(),
            color2: "DBE5F1".to_string(),
            style: GradientStyle::Horizontal,
            variant: 0,
            texture: None,
            texture_file: None,
            pattern: Pattern::Percent5,
            foreground: "000000".to_string(),
            background: if page == "FFFFFF" {
                "FFFFFF".to_string()
            } else {
                page
            },
            picture: None,
        };
        let Some(fill) = current else {
            return dialog;
        };
        let hex = |color: &Option<String>| {
            color
                .as_deref()
                .map(|color| color.trim_start_matches('#').to_ascii_uppercase())
        };
        match fill.kind.as_str() {
            "gradient" | "gradientRadial" => {
                dialog.tab = FillTab::Gradient;
                if let Some(color) = hex(&fill.color) {
                    dialog.color = color;
                }
                if let Some(color2) = hex(&fill.color2) {
                    dialog.color2 = color2;
                }
                dialog.style = GradientStyle::from_angle(fill.angle);
                dialog.variant = VARIANTS
                    .iter()
                    .position(|focus| (focus - fill.focus).abs() < 0.5)
                    .unwrap_or(0);
            }
            "tile" => {
                dialog.tab = FillTab::Texture;
                dialog.texture = Texture::ALL
                    .into_iter()
                    .find(|texture| fill.title.as_deref() == Some(texture.title()));
            }
            "pattern" => {
                dialog.tab = FillTab::Pattern;
                if let Some(color) = hex(&fill.color) {
                    dialog.foreground = color;
                }
                if let Some(color2) = hex(&fill.color2) {
                    dialog.background = color2;
                }
                if let Some(pattern) = Pattern::ALL
                    .into_iter()
                    .find(|pattern| fill.title.as_deref() == Some(pattern.title()))
                {
                    dialog.pattern = pattern;
                }
            }
            _ => dialog.tab = FillTab::Picture,
        }
        dialog
    }

    /// The effect the dialog's tab makes, or `None` when the tab has
    /// nothing chosen yet (a texture or picture not picked).
    pub fn fill(&self) -> Option<HeadlessPageFill> {
        let hash = |color: &str| Some(format!("#{color}"));
        let blank = HeadlessPageFill {
            kind: String::new(),
            color: None,
            color2: None,
            angle: 0.0,
            focus: 0.0,
            image_base64: None,
            title: None,
        };
        match self.tab {
            FillTab::Gradient => Some(HeadlessPageFill {
                kind: "gradient".to_string(),
                color: hash(&self.color),
                color2: hash(&self.color2),
                angle: self.style.angle(),
                focus: VARIANTS[self.variant.min(VARIANTS.len() - 1)],
                ..blank
            }),
            FillTab::Texture => {
                let (title, bytes) = match (&self.texture_file, self.texture) {
                    (Some(file), _) => (file_title(&file.name), file.bytes.clone()),
                    (None, Some(texture)) => (texture.title().to_string(), texture.png()),
                    (None, None) => return None,
                };
                Some(HeadlessPageFill {
                    kind: "texture".to_string(),
                    image_base64: Some(base64(&bytes)),
                    title: Some(title),
                    ..blank
                })
            }
            FillTab::Pattern => Some(HeadlessPageFill {
                kind: "pattern".to_string(),
                color: hash(&self.foreground),
                color2: hash(&self.background),
                image_base64: Some(base64(&self.pattern.png())),
                title: Some(self.pattern.title().to_string()),
                ..blank
            }),
            FillTab::Picture => self.picture.as_ref().map(|file| HeadlessPageFill {
                kind: "picture".to_string(),
                image_base64: Some(base64(&file.bytes)),
                title: Some(file_title(&file.name)),
                ..blank
            }),
        }
    }

    /// その他のテクスチャ…: `name`'s picture as the tile.
    pub fn set_texture_file(&mut self, name: &str, bytes: Vec<u8>) {
        self.tab = FillTab::Texture;
        self.texture = None;
        self.texture_file = Some(FillFile {
            name: name.to_string(),
            bytes,
        });
    }

    /// 図の選択…: `name`'s picture over the page.
    pub fn set_picture(&mut self, name: &str, bytes: Vec<u8>) {
        self.tab = FillTab::Picture;
        self.picture = Some(FillFile {
            name: name.to_string(),
            bytes,
        });
    }

    /// One of the made textures.
    pub fn set_texture(&mut self, texture: Texture) {
        self.texture = Some(texture);
        self.texture_file = None;
    }
}

/// A file's name without its folder or extension, as Word titles it.
fn file_title(name: &str) -> String {
    let name = name.rsplit(['/', '\\']).next().unwrap_or(name);
    name.rsplit_once('.')
        .map_or(name, |(stem, _)| stem)
        .to_string()
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for (index, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if index <= chunk.len() {
                out.push(ALPHABET[((n >> shift) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// 塗りつぶし効果 on a document.
pub trait WordPageFill: WordSession {
    /// The page's fill effect, as drawn.
    fn page_fill(&self) -> Option<jade_docx_reader::document::PageBackgroundFill> {
        self.word_document().page_fill().cloned()
    }

    /// The dialog over the effect there is.
    fn page_fill_dialog(&self) -> PageFillDialog {
        let document = self.word_document();
        PageFillDialog::open(
            document.page_fill(),
            document
                .document
                .as_deref()
                .and_then(|document| document.background_color.as_deref()),
        )
    }

    /// OK on the dialog: its effect behind every page. A tab with nothing
    /// chosen leaves the page as it is.
    fn apply_page_fill(&mut self, dialog: &PageFillDialog) -> Result<(), String> {
        match dialog.fill() {
            Some(fill) => {
                self.apply_command(HeadlessCommand::SetPageFill { fill: Some(fill) }, true)
            }
            None => Ok(()),
        }
    }
}

impl<T: WordSession + ?Sized> WordPageFill for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_standard_alphabet() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn a_gradient_fill_carries_its_style_and_variant() {
        let mut dialog = PageFillDialog::open(None, Some("#FFF9E6"));
        assert_eq!(dialog.color, "FFF9E6");
        dialog.style = GradientStyle::Vertical;
        dialog.variant = 1;
        let fill = dialog.fill().unwrap();
        assert_eq!(fill.kind, "gradient");
        assert_eq!(fill.color.as_deref(), Some("#FFF9E6"));
        assert_eq!(fill.angle, -90.0);
        assert_eq!(fill.focus, 100.0);
    }

    #[test]
    fn a_texture_or_picture_needs_choosing_first() {
        let mut dialog = PageFillDialog::open(None, None);
        dialog.tab = FillTab::Texture;
        assert!(dialog.fill().is_none());
        dialog.set_texture(Texture::Denim);
        let fill = dialog.fill().unwrap();
        assert_eq!(fill.kind, "texture");
        assert_eq!(fill.title.as_deref(), Some("Denim"));
        assert!(fill.image_base64.unwrap().starts_with("iVBORw0KGgo"));

        dialog.tab = FillTab::Picture;
        assert!(dialog.fill().is_none());
        dialog.set_picture("/photos/海.png", Texture::Sand.png());
        assert_eq!(dialog.fill().unwrap().title.as_deref(), Some("海"));
    }

    #[test]
    fn a_pattern_is_black_on_white_with_its_colours_beside() {
        let mut dialog = PageFillDialog::open(None, None);
        dialog.tab = FillTab::Pattern;
        dialog.pattern = Pattern::Checker;
        dialog.foreground = "C00000".to_string();
        let fill = dialog.fill().unwrap();
        assert_eq!(fill.color.as_deref(), Some("#C00000"));
        assert_eq!(fill.color2.as_deref(), Some("#FFFFFF"));
        let decoder = png::Decoder::new(std::io::Cursor::new(Pattern::Checker.png()));
        let mut reader = decoder.read_info().unwrap();
        let mut pixels = vec![0; reader.output_buffer_size()];
        reader.next_frame(&mut pixels).unwrap();
        assert_eq!(&pixels[0..4], &[0, 0, 0, 255]);
        assert_eq!(&pixels[8..12], &[255, 255, 255, 255]);
    }

    #[test]
    fn the_dialog_reopens_on_the_fill_there_is() {
        let fill = jade_docx_reader::document::PageBackgroundFill {
            kind: "gradient".to_string(),
            color: Some("#dbe5f1".to_string()),
            color2: Some("#0070C0".to_string()),
            angle: -45.0,
            focus: 50.0,
            ..Default::default()
        };
        let dialog = PageFillDialog::open(Some(&fill), Some("#DBE5F1"));
        assert_eq!(dialog.tab, FillTab::Gradient);
        assert_eq!(dialog.color, "DBE5F1");
        assert_eq!(dialog.color2, "0070C0");
        assert_eq!(dialog.style, GradientStyle::DiagonalDown);
        assert_eq!(dialog.variant, 2);

        let pattern = jade_docx_reader::document::PageBackgroundFill {
            kind: "pattern".to_string(),
            color: Some("#FF0000".to_string()),
            color2: Some("#FFFFFF".to_string()),
            title: Some("Cross".to_string()),
            ..Default::default()
        };
        let dialog = PageFillDialog::open(Some(&pattern), None);
        assert_eq!(dialog.tab, FillTab::Pattern);
        assert_eq!(dialog.pattern, Pattern::Cross);
        assert_eq!(dialog.foreground, "FF0000");
    }
}

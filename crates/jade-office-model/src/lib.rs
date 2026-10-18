//! The editing models the native app and the browser build share.
//!
//! - [`sheet::Workbook`]: a workbook over jade-core's `HeadlessWorkbook` —
//!   writes patch the bytes, recalculate and keep undo history — with the
//!   selected sheet laid out, hit-tested and painted.
//! - [`deck::Deck`]: a presentation over the pptx `EditSession` — shape
//!   text, moves, resizes and deletes as undoable changes — read back from
//!   the session's bytes to lay out and paint.
//!
//! Neither knows about gpui, a window or a browser. What differs between the
//! shells is only the painter: Skia natively, tiny-skia in wasm. That is the
//! [`Backend`]: a surface with the fonts on it, and the pixels back.

use std::sync::Arc;

use jade_painter::Painter;

pub mod deck;
pub mod sheet;
pub mod typing;

#[cfg(feature = "skia")]
pub mod skia;
#[cfg(feature = "tiny-skia")]
pub mod tiny;

/// A painter backend: surfaces to lay out against and paint on.
pub trait Backend: Clone + 'static {
    type Painter: Painter + Clone + 'static;

    /// A `width` × `height` device-px raster surface filled white, with the
    /// backend's fonts registered.
    fn surface(&self, width: u32, height: u32) -> Option<Self::Painter>;

    /// The surface's pixels as BGRA, premultiplied — what gpui's
    /// `RenderImage` holds — with its size.
    fn bgra(painter: &Self::Painter) -> Option<(Vec<u8>, u32, u32)>;

    /// The surface as PNG bytes.
    fn png(painter: &Self::Painter) -> Option<Vec<u8>>;
}

/// The families documents ask for. A machine outside Office has none of
/// them; pointing them all at the bundled face beats drawing no text.
pub const ALIASED_FAMILIES: &[&str] = &[
    "Noto Sans JP",
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
    "Yu Mincho",
    "游明朝",
    "MS Mincho",
    "Times New Roman",
];

/// A font file registered under a family and weight. The same bytes appear
/// under many families; backends share them rather than copy.
#[derive(Clone)]
pub struct Face {
    pub family: String,
    pub weight: u16,
    pub data: Arc<Vec<u8>>,
}

/// The regular and bold faces under every family in [`ALIASED_FAMILIES`].
pub fn aliased_faces(regular: Arc<Vec<u8>>, bold: Option<Arc<Vec<u8>>>) -> Vec<Face> {
    ALIASED_FAMILIES
        .iter()
        .flat_map(|family| {
            let regular = Face {
                family: family.to_string(),
                weight: 400,
                data: Arc::clone(&regular),
            };
            let bold = bold.as_ref().map(|bold| Face {
                family: family.to_string(),
                weight: 700,
                data: Arc::clone(bold),
            });
            std::iter::once(regular).chain(bold)
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::PathBuf;
    use std::sync::Arc;

    pub fn core() -> PathBuf {
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"))
    }

    pub fn faces() -> Vec<super::Face> {
        let directory = core().join("fonts/Noto_Sans_JP/static");
        super::aliased_faces(
            Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
            Some(Arc::new(
                std::fs::read(directory.join("NotoSansJP-Bold.ttf")).unwrap(),
            )),
        )
    }

    pub fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(core().join("public/fixtures").join(name)).unwrap()
    }
}

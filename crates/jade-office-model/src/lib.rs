//! The editing models the native app and the browser build share.
//!
//! - [`sheet::Workbook`]: a workbook over jade-core's `HeadlessWorkbook` —
//!   writes patch the bytes, recalculate and keep undo history — with the
//!   selected sheet laid out, hit-tested and painted.
//! - [`word::WordFormatting`]: a Word ribbon's formatting, style and table
//!   commands over any [`word::WordSession`] — the native `NativeDocument`
//!   or the browser's tiny-skia document.
//! - [`deck::Deck`]: a presentation over the pptx `EditSession` — shape
//!   text, moves, resizes and deletes as undoable changes — read back from
//!   the session's bytes to lay out and paint.
//!
//! Neither knows about gpui, a window or a browser. What differs between the
//! shells is only the painter: Skia natively, tiny-skia in wasm. That is the
//! [`Backend`]: a surface with the fonts on it, and the pixels back.

use std::sync::Arc;

use jade_painter::Painter;

pub mod charts;
pub mod clipboard;
pub mod conditional_formatting;
pub mod csv;
pub mod custom_filter;
pub mod deck;
pub mod duplicates;
pub mod editable_ranges;
pub mod find;
pub mod format_cells;
pub mod formula_complete;
pub mod formula_tips;
pub mod grid;
pub mod insert_function;
pub mod links;
pub mod more_colors;
pub mod names;
pub mod number_formats;
pub mod page_setup_dialog;
pub mod paste_special;
#[cfg(feature = "tiny-skia")]
pub mod pdf;
pub mod pivots;
pub mod print;
pub mod protection;
pub use sheet::protected_edits;
pub mod sheet;
pub mod sheet_properties;
pub mod sort_dialog;
pub mod subtotal;
pub mod tables;
pub mod text;
pub mod text_columns;
pub mod top_ten;
pub mod typing;
pub mod validation;
pub mod word;

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

    /// A `width` × `height` px page (96 to the inch) to print on, with the
    /// backend's fonts: Skia records its drawing for a vector PDF; tiny-skia
    /// paints pixels at twice the size.
    fn page(&self, width: f64, height: f64) -> Option<Self::Painter>;

    /// Pages from [`Backend::page`], each `size` px, as a PDF.
    fn pdf(pages: &[Self::Painter], size: (f64, f64)) -> Result<Vec<u8>, String>;

    /// Makes the faces of `package`'s fonts — an Office file's bytes — ready
    /// for the surfaces made after, where the backend can find them
    /// installed. Backends that only ever draw their own faces do nothing.
    fn prepare_fonts(&self, _package: &[u8]) {}
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

/// The bundled Noto Sans Thai (jade-core
/// `fonts/Noto_Sans_Thai/NotoSansThai-VariableFont_wdth,wght.ttf`), in the
/// path relative to the fonts directory.
pub const THAI_FONT: &str = "Noto_Sans_Thai/NotoSansThai-VariableFont_wdth,wght.ttf";

/// Noto Sans Thai as a face of its own: registered beside the Japanese
/// faces, it is what Thai text falls back to, as a browser falls back, so it
/// draws in Thai glyphs rather than the Japanese face's missing-glyph boxes.
pub fn thai_face(data: Arc<Vec<u8>>) -> Face {
    Face {
        family: "Noto Sans Thai".to_string(),
        weight: 400,
        data,
    }
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

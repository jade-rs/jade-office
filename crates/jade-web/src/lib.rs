//! Slides and sheets rendered with the pure-Rust painter, natively or in the
//! browser.
//!
//! The same reader → layout → renderer chain the native app runs, with
//! `jade-tiny-skia-painter` underneath instead of Skia. Nothing in it needs a
//! thread, the filesystem or C++, so it compiles to `wasm32-unknown-unknown`
//! unchanged; `web.rs` is the thin wasm-bindgen layer over it.

use std::rc::Rc;
use std::sync::Arc;

use jade_tiny_skia_painter::FontBook;
#[cfg(test)]
use jade_tiny_skia_painter::TinySkiaPainter;

pub mod document;
#[cfg(target_arch = "wasm32")]
mod web;

/// The families documents ask for, all pointed at the one bundled face — the
/// native app's aliases.
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
    "Yu Mincho",
    "游明朝",
    "MS Mincho",
    "Times New Roman",
];

/// A font book with the regular and bold faces registered under every alias.
pub fn font_book(regular: Vec<u8>, bold: Option<Vec<u8>>) -> Result<Rc<FontBook>, String> {
    let regular = Arc::new(regular);
    let bold = bold.map(Arc::new);
    let mut book = FontBook::new();
    for family in ALIASED_FAMILIES {
        book.register(*family, 400, Arc::clone(&regular))?;
        if let Some(bold) = &bold {
            book.register(*family, 700, Arc::clone(bold))?;
        }
    }
    Ok(Rc::new(book))
}

pub mod deck;
pub mod sheet;

pub use deck::Deck;
pub use sheet::Workbook;

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::path::PathBuf;

    fn core() -> PathBuf {
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"))
    }

    pub(crate) fn fonts() -> Rc<FontBook> {
        let directory = core().join("fonts/Noto_Sans_JP/static");
        font_book(
            std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap(),
            Some(std::fs::read(directory.join("NotoSansJP-Bold.ttf")).unwrap()),
        )
        .unwrap()
    }

    fn ink(painter: &TinySkiaPainter) -> usize {
        painter
            .rgba_premultiplied()
            .chunks_exact(4)
            .filter(|pixel| pixel[0] < 200 || pixel[1] < 200 || pixel[2] < 200)
            .count()
    }

    #[test]
    fn every_fixture_slide_paints() {
        let fonts = fonts();
        for name in [
            "sample-deck.pptx",
            "grouped-deck.pptx",
            "inherited-deck.pptx",
            "themed-deck.pptx",
        ] {
            let bytes = std::fs::read(core().join("public/fixtures").join(name)).unwrap();
            let mut deck = Deck::open(&bytes, Rc::clone(&fonts)).unwrap();
            for index in 0..deck.slide_count() {
                let painter = deck.render_slide(index, 480).unwrap();
                assert_eq!(painter.width(), 480);
                assert!(ink(&painter) > 100, "{name} slide {} is blank", index + 1);
            }
        }
    }

    #[test]
    fn a_sheet_paints_its_grid_and_values() {
        let bytes =
            std::fs::read(core().join("public/fixtures/grid-operations-workbook.xlsx")).unwrap();
        let workbook = Workbook::open(&bytes, "UTC", fonts()).unwrap();
        let painter = workbook
            .render_viewport((0.0, 0.0), 640, 400, 1.0, None)
            .unwrap();
        assert!(ink(&painter) > 1000);
        let (width, height) = workbook.sheet_size();
        assert!(width > 600.0 && height > 400.0);
    }
}

//! Slides and sheets rendered with the pure-Rust painter, natively or in the
//! browser.
//!
//! The same reader → layout → renderer chain the native app runs, with
//! `jade-tiny-skia-painter` underneath instead of Skia. Nothing in it needs a
//! thread, the filesystem or C++, so it compiles to `wasm32-unknown-unknown`
//! unchanged; `web.rs` is the thin wasm-bindgen layer over it.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use jade_painter::{ImageRect, ImageSource, Painter};
use jade_pptx_layout::{layout_deck_slide, PlacedShape, SlideLayout};
use jade_pptx_render::{render_slide_with, RenderOptions as SlideOptions};
use jade_tiny_skia_painter::{FontBook, TinySkiaPainter};
use jade_xlsx_layout::{layout_workbook_sheet, LayoutOptions, Rect, SheetLayout};
use jade_xlsx_render::{render_grid, RenderOptions as SheetOptions};

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

// ---- slides -----------------------------------------------------------------

/// An open presentation: the parsed deck, its pictures, and laid-out slides.
pub struct Deck {
    deck: jade_pptx_reader::Deck,
    parts: HashMap<String, Vec<u8>>,
    fonts: Rc<FontBook>,
    options: SlideOptions,
    layouts: HashMap<usize, Rc<SlideLayout>>,
}

impl Deck {
    pub fn open(bytes: &[u8], fonts: Rc<FontBook>) -> Result<Self, String> {
        let deck = jade_pptx_reader::read_deck(bytes)
            .map_err(|error| format!("cannot read the presentation: {error}"))?;
        let package = jade_opc::OpcPackage::open(bytes)
            .map_err(|error| format!("cannot open the package: {error}"))?;
        // Every media part, keyed by name; a slide asks by the name the
        // reader resolved.
        let parts = package
            .part_names()
            .iter()
            .filter(|name| name.contains("/media/"))
            .filter_map(|name| {
                let bytes = package.part(name)?.to_vec();
                Some((name.clone(), bytes))
            })
            .collect();
        Ok(Self {
            deck,
            parts,
            fonts,
            options: SlideOptions::default(),
            layouts: HashMap::new(),
        })
    }

    pub fn slide_count(&self) -> usize {
        self.deck.slides.len()
    }

    pub fn slide_size(&self) -> (f64, f64) {
        (
            self.deck.slide_size.width_px(),
            self.deck.slide_size.height_px(),
        )
    }

    pub fn notes(&self, index: usize) -> Option<&str> {
        self.deck.slides.get(index)?.notes.as_deref()
    }

    fn layout(&mut self, index: usize) -> Option<Rc<SlideLayout>> {
        if let Some(layout) = self.layouts.get(&index) {
            return Some(Rc::clone(layout));
        }
        let measuring = TinySkiaPainter::new(1, 1, Rc::clone(&self.fonts))?;
        let measure =
            jade_pptx_render::PainterMeasure::new(measuring, self.options.default_font.clone());
        let layout = Rc::new(layout_deck_slide(&self.deck, index, &measure)?);
        self.layouts.insert(index, Rc::clone(&layout));
        Some(layout)
    }

    /// Paints slide `index` `width` px wide.
    pub fn render_slide(&mut self, index: usize, width: u32) -> Option<TinySkiaPainter> {
        let layout = self.layout(index)?;
        let (slide_width, slide_height) = self.slide_size();
        let scale = f64::from(width.max(1)) / slide_width.max(1.0);
        let height = (slide_height * scale).round().max(1.0) as u32;
        let painter = TinySkiaPainter::new(width.max(1), height, Rc::clone(&self.fonts))?;
        painter.save();
        painter.scale(scale, scale).ok()?;
        let parts = &self.parts;
        let mut draw_image = |shape: &PlacedShape| -> bool {
            let Some((part, bytes)) = shape.image_part.as_deref().and_then(|part| {
                parts
                    .get(part.trim_start_matches('/'))
                    .or_else(|| parts.get(part))
                    .map(|bytes| (part, bytes))
            }) else {
                return false;
            };
            let Some((source, destination)) = shape.image_rectangles(1.0, 1.0) else {
                return false;
            };
            painter
                .draw_image(
                    ImageSource { key: part, bytes },
                    Some([source.x, source.y, source.width, source.height]),
                    ImageRect {
                        x: destination.x,
                        y: destination.y,
                        width: destination.width,
                        height: destination.height,
                    },
                )
                .unwrap_or(false)
        };
        render_slide_with(&painter, &layout, &self.options, &mut draw_image).ok()?;
        painter.restore();
        Some(painter)
    }
}

// ---- sheets -----------------------------------------------------------------

/// An open workbook with one sheet laid out.
pub struct Workbook {
    workbook: jade_xlsx_reader::Workbook,
    fonts: Rc<FontBook>,
    index: usize,
    layout: SheetLayout,
}

impl Workbook {
    pub fn open(bytes: &[u8], fonts: Rc<FontBook>) -> Result<Self, String> {
        let workbook = jade_xlsx_reader::read_xlsx(bytes)
            .map_err(|error| format!("cannot read the workbook: {error}"))?;
        if workbook.sheets.is_empty() {
            return Err("the workbook has no sheets".to_string());
        }
        let layout = lay_out(&workbook, 0, &fonts)?;
        Ok(Self {
            workbook,
            fonts,
            index: 0,
            layout,
        })
    }

    pub fn sheet_names(&self) -> Vec<String> {
        self.workbook
            .sheets
            .iter()
            .map(|sheet| sheet.name.clone())
            .collect()
    }

    pub fn select_sheet(&mut self, index: usize) -> Result<(), String> {
        if index >= self.workbook.sheets.len() {
            return Err(format!("no sheet at index {index}"));
        }
        self.layout = lay_out(&self.workbook, index, &self.fonts)?;
        self.index = index;
        Ok(())
    }

    /// The laid-out sheet's size in px, headers included.
    pub fn sheet_size(&self) -> (f64, f64) {
        let metrics = self.layout.header_metrics();
        (
            self.layout.content_width + metrics.row_width,
            self.layout.content_height + metrics.column_height,
        )
    }

    /// Paints the part of the sheet under `scroll` (sheet px) onto a
    /// `width` × `height` surface, at `scale` device px per sheet px.
    pub fn render_viewport(
        &self,
        scroll: (f64, f64),
        width: u32,
        height: u32,
        scale: f64,
    ) -> Option<TinySkiaPainter> {
        let painter = TinySkiaPainter::new(width.max(1), height.max(1), Rc::clone(&self.fonts))?;
        let metrics = self.layout.header_metrics();
        let viewport = Rect {
            x: scroll.0.max(0.0),
            y: scroll.1.max(0.0),
            width: (f64::from(width) / scale - metrics.row_width).max(0.0),
            height: (f64::from(height) / scale - metrics.column_height).max(0.0),
        };
        painter.save();
        painter.scale(scale, scale).ok()?;
        render_grid(
            &painter,
            &self.layout,
            viewport,
            None,
            &SheetOptions::default(),
        )
        .ok()?;
        painter.restore();
        Some(painter)
    }
}

fn lay_out(
    workbook: &jade_xlsx_reader::Workbook,
    index: usize,
    fonts: &Rc<FontBook>,
) -> Result<SheetLayout, String> {
    let measuring = TinySkiaPainter::new(1, 1, Rc::clone(fonts))
        .ok_or_else(|| "cannot create a measuring surface".to_string())?;
    layout_workbook_sheet(
        workbook,
        index,
        &LayoutOptions {
            min_column_count: 12,
            min_row_count: 30,
            date_system: workbook.date_system,
            measure: Rc::new(jade_xlsx_render::PainterMeasure::new(measuring)),
            ..LayoutOptions::default()
        }
        .with_normal_font_metrics(&workbook.styles),
    )
    .ok_or_else(|| "the sheet does not exist".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn core() -> PathBuf {
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"))
    }

    fn fonts() -> Rc<FontBook> {
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
        let workbook = Workbook::open(&bytes, fonts()).unwrap();
        let painter = workbook.render_viewport((0.0, 0.0), 640, 400, 1.0).unwrap();
        assert!(ink(&painter) > 1000);
        let (width, height) = workbook.sheet_size();
        assert!(width > 600.0 && height > 400.0);
    }
}

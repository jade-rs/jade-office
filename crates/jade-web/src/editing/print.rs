//! Printing a Word document in the browser: the same settings as the
//! native app's 印刷 panel (`jade_office_model::word::printing`) — which
//! pages, the markup, pages per sheet, scaling to a paper, the background
//! and hidden text — made into a PDF of the sheets for the browser to print.
//! The browser's own print dialog then takes copies and two-sided printing.

use std::rc::Rc;

use jade_office_model::word::printing::{
    self, pages_to_print, PageLabel, PrintRange, PrintSettings, Sheet,
};

use super::EditableDocument;
use crate::pdf::PdfPage;

/// Device pixels per CSS pixel the pages are drawn at for printing: 144
/// to the inch.
const PRINT_SCALE: f64 = 1.5;

/// A page's pixels for printing.
struct PagePixels {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl EditableDocument {
    /// The document as `settings` print it: a PDF of the sheets. `current`
    /// is the page the caret is on.
    pub fn print_pdf(
        &mut self,
        settings: &PrintSettings,
        current: usize,
    ) -> Result<Vec<u8>, String> {
        let docx = self
            .word_document
            .export_docx()
            .map_err(|error| format!("cannot print: {error:?}"))?;
        let mut bytes = printing::print_docx(&docx, settings)?;
        if settings.range == PrintRange::Selection {
            if let Some(range) = self.selection.filter(|range| !range.is_collapsed()) {
                let (start, end) = range.normalized();
                bytes = printing::selection_docx(
                    &bytes,
                    (start.paragraph, start.offset),
                    (end.paragraph, end.offset),
                )?;
            }
        }
        let mut printed = EditableDocument::open_resident(
            &bytes,
            PRINT_SCALE,
            Rc::clone(&self.fonts),
            Rc::clone(&self.measurer),
            0..0,
        )?;
        if !settings.markup {
            printed.options.render_changes = false;
            printed.options.render_comments = false;
            printed.options.render_original = false;
            printed.repaint_all()?;
        }
        let labels: Vec<PageLabel> = printed
            .page_labels()
            .into_iter()
            .map(|(section, number)| PageLabel {
                section: section + 1,
                number,
            })
            .collect();
        let sizes: Vec<(f64, f64)> = (0..printed.page_count())
            .map(|index| {
                let (width, height) = printed
                    .page_sizes
                    .borrow()
                    .get(index)
                    .copied()
                    .unwrap_or_else(|| printed.page_size());
                (width * 0.75, height * 0.75)
            })
            .collect();
        let pages =
            pages_to_print(settings, &labels, current).map_err(|error| error.to_string())?;
        let sheets = printing::sheets(settings, &pages, &sizes);
        let mut pixels: std::collections::HashMap<usize, PagePixels> = Default::default();
        for page in &pages {
            if pixels.contains_key(page) {
                continue;
            }
            printed.set_resident_pages(*page..*page + 1)?;
            let surface = printed
                .page(*page)
                .ok_or_else(|| format!("page {} did not paint", page + 1))?;
            pixels.insert(
                *page,
                PagePixels {
                    width: surface.width(),
                    height: surface.height(),
                    rgba: surface.rgba_premultiplied(),
                },
            );
        }
        let pages: Vec<PdfPage> = sheets
            .iter()
            .map(|sheet| sheet_pixels(sheet, &sizes, &pixels))
            .collect();
        crate::pdf::pdf_of_pages(&pages)
    }
}

/// A sheet as pixels: white, each page on it resampled into its place.
fn sheet_pixels(
    sheet: &Sheet,
    sizes: &[(f64, f64)],
    pixels: &std::collections::HashMap<usize, PagePixels>,
) -> PdfPage {
    // A page alone at its own size is its pixels as they are.
    if let [placement] = sheet.placements.as_slice() {
        if placement.scale == 1.0 && placement.x == 0.0 && placement.y == 0.0 {
            if let Some(page) = pixels.get(&placement.page) {
                return PdfPage {
                    width_pt: sheet.width,
                    height_pt: sheet.height,
                    pixel_width: page.width,
                    pixel_height: page.height,
                    rgba: page.rgba.clone(),
                };
            }
        }
    }
    let per_point = PRINT_SCALE / 0.75;
    let width = (sheet.width * per_point).ceil().max(1.0) as u32;
    let height = (sheet.height * per_point).ceil().max(1.0) as u32;
    let mut rgba = vec![255u8; (width * height * 4) as usize];
    for placement in &sheet.placements {
        let Some(page) = pixels.get(&placement.page) else {
            continue;
        };
        let (page_width_pt, page_height_pt) =
            sizes.get(placement.page).copied().unwrap_or((612.0, 792.0));
        let left = placement.x * per_point;
        let top = placement.y * per_point;
        let target_width = page_width_pt * placement.scale * per_point;
        let target_height = page_height_pt * placement.scale * per_point;
        let x0 = left.floor().max(0.0) as u32;
        let y0 = top.floor().max(0.0) as u32;
        let x1 = ((left + target_width).ceil() as u32).min(width);
        let y1 = ((top + target_height).ceil() as u32).min(height);
        for y in y0..y1 {
            let v = ((y as f64 + 0.5 - top) / target_height).clamp(0.0, 1.0);
            let source_y = ((v * page.height as f64) as u32).min(page.height.saturating_sub(1));
            for x in x0..x1 {
                let u = ((x as f64 + 0.5 - left) / target_width).clamp(0.0, 1.0);
                let source_x = ((u * page.width as f64) as u32).min(page.width.saturating_sub(1));
                let from = ((source_y * page.width + source_x) * 4) as usize;
                let to = ((y * width + x) * 4) as usize;
                if let (Some(source), Some(target)) =
                    (page.rgba.get(from..from + 4), rgba.get_mut(to..to + 4))
                {
                    target.copy_from_slice(source);
                }
            }
        }
    }
    PdfPage {
        width_pt: sheet.width,
        height_pt: sheet.height,
        pixel_width: width,
        pixel_height: height,
        rgba,
    }
}

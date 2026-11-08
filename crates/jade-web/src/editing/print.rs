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

/// Paper layout used by the print panel and the generated PDF, in points.
#[derive(Clone, Debug, PartialEq)]
pub struct PrintPreview {
    pub sheets: Vec<Sheet>,
    pub page_sizes: Vec<(f64, f64)>,
}

struct PrintLayout {
    document: EditableDocument,
    pages: Vec<usize>,
    preview: PrintPreview,
}

/// A print preview snapshot. Only the requested sheet is painted; navigation
/// reuses pagination and does not allocate images for the whole document.
pub struct PrintPreviewSession {
    layout: PrintLayout,
}

impl PrintPreviewSession {
    pub fn layout(&self) -> &PrintPreview {
        &self.layout.preview
    }

    /// Render the same sheet pixels used by PDF printing (RGBA, premultiplied).
    pub fn render_sheet(&mut self, index: usize) -> Result<PdfPage, String> {
        let sheet = self
            .layout
            .preview
            .sheets
            .get(index)
            .ok_or_else(|| format!("print sheet {} does not exist", index + 1))?;
        let mut pixels = std::collections::HashMap::new();
        for placement in &sheet.placements {
            let page = placement.page;
            if pixels.contains_key(&page) {
                continue;
            }
            self.layout.document.set_resident_pages(page..page + 1)?;
            let surface = self
                .layout
                .document
                .page(page)
                .ok_or_else(|| format!("page {} did not paint", page + 1))?;
            pixels.insert(
                page,
                PagePixels {
                    width: surface.width(),
                    height: surface.height(),
                    rgba: surface.rgba_premultiplied(),
                },
            );
        }
        let image = sheet_pixels(sheet, &self.layout.preview.page_sizes, &pixels);
        self.layout.document.set_resident_pages(0..0)?;
        Ok(image)
    }
}

impl EditableDocument {
    /// A fresh paper layout shared by PDF export and printing. Screen-only
    /// decorations must never appear in either output, even though the editor
    /// enables formatting and crop marks by default.
    pub(super) fn open_for_output(&self, bytes: &[u8]) -> Result<Self, String> {
        let mut printed = Self::open_resident_with_datetime(
            bytes,
            PRINT_SCALE,
            Rc::clone(&self.fonts),
            Rc::clone(&self.measurer),
            0..0,
            self.field_datetime,
        )?;
        printed.options.show_formatting_marks = false;
        printed.options.show_crop_marks = false;
        printed.repaint_all()?;
        Ok(printed)
    }

    /// The document as `settings` print it: a PDF of the sheets. `current`
    /// is the page the caret is on.
    pub fn print_pdf(
        &mut self,
        settings: &PrintSettings,
        current: usize,
    ) -> Result<Vec<u8>, String> {
        self.print_pdf_from_docx(&self.word_docx()?, settings, current)
    }

    pub(super) fn print_pdf_from_docx(
        &self,
        docx: &[u8],
        settings: &PrintSettings,
        current: usize,
    ) -> Result<Vec<u8>, String> {
        let PrintLayout {
            mut document,
            pages,
            preview,
        } = self.print_layout_from_docx(docx, settings, current)?;
        let printed = &mut document;
        let sizes = &preview.page_sizes;
        let sheets = &preview.sheets;
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
            .map(|sheet| sheet_pixels(sheet, sizes, &pixels))
            .collect();
        crate::pdf::pdf_of_pages(&pages)
    }
    /// Prepare pagination without painting page images or changing editor state.
    pub fn print_preview(
        &self,
        settings: &PrintSettings,
        current: usize,
    ) -> Result<PrintPreview, String> {
        Ok(self
            .prepare_print_preview(settings, current)?
            .layout()
            .clone())
    }

    pub fn prepare_print_preview(
        &self,
        settings: &PrintSettings,
        current: usize,
    ) -> Result<PrintPreviewSession, String> {
        self.prepare_print_preview_from_docx(&self.word_docx()?, settings, current)
    }

    pub(super) fn prepare_print_preview_from_docx(
        &self,
        docx: &[u8],
        settings: &PrintSettings,
        current: usize,
    ) -> Result<PrintPreviewSession, String> {
        if settings.update_fields {
            // The print action updates the real document (and its undo stack).
            // Preview that same update on an isolated snapshot so opening or
            // cancelling the panel never edits the user's document.
            let mut updated = self.open_for_output(docx)?;
            updated.selection = self.selection;
            printing::update_fields(&mut updated)?;
            return Ok(PrintPreviewSession {
                layout: updated.print_layout_from_docx(&updated.word_docx()?, settings, current)?,
            });
        }
        Ok(PrintPreviewSession {
            layout: self.print_layout_from_docx(docx, settings, current)?,
        })
    }

    fn print_layout_from_docx(
        &self,
        docx: &[u8],
        settings: &PrintSettings,
        current: usize,
    ) -> Result<PrintLayout, String> {
        let mut bytes = printing::print_docx(docx, settings)?;
        if settings.range == PrintRange::Selection {
            let range = self
                .selection
                .filter(|range| !range.is_collapsed())
                .ok_or("Select content before printing the selection.")?;
            let (start, end) = range.normalized();
            bytes = printing::selection_docx(
                &bytes,
                (start.paragraph, start.offset),
                (end.paragraph, end.offset),
            )?;
        }
        let mut printed = self.open_for_output(&bytes)?;
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
        Ok(PrintLayout {
            document: printed,
            pages,
            preview: PrintPreview {
                sheets,
                page_sizes: sizes,
            },
        })
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

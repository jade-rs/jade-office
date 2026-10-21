//! The native backend: Skia surfaces, for the desktop app.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::Arc;

use jade_docx_native_render::document::NativeDocument;
use jade_skia_painter::SkiaPainter;

use crate::{Backend, Face};

/// Skia surfaces sharing one set of parsed faces.
///
/// Registering a face from bytes makes Skia parse and copy the file, and the
/// aliases put the same two files under twenty families. So each distinct
/// file is parsed once, onto a 1×1 template, and a new surface is handed the
/// template's refcounted typefaces.
#[derive(Clone)]
pub struct SkiaBackend {
    template: SkiaPainter,
    /// Whether a file's own fonts are looked for among the installed ones,
    /// and the families already looked for.
    installed_fonts: Option<Rc<RefCell<BTreeSet<String>>>>,
}

impl SkiaBackend {
    pub fn new(faces: &[Face]) -> Result<Self, String> {
        let template =
            SkiaPainter::new(1, 1).ok_or_else(|| "cannot create a font surface".to_string())?;
        // The index of the face already parsed from each distinct file.
        let mut parsed: Vec<(Arc<Vec<u8>>, usize)> = Vec::new();
        for face in faces {
            match parsed
                .iter()
                .find(|(data, _)| Arc::ptr_eq(data, &face.data))
            {
                Some((_, index)) => {
                    let (_, _, typeface) = template.registered_faces().swap_remove(*index);
                    template.register_face(face.family.as_str(), face.weight, typeface);
                }
                None => {
                    let index = template.registered_faces().len();
                    template
                        .register_font_bytes(face.family.as_str(), face.weight, &face.data)
                        .ok_or_else(|| format!("{}: not a font Skia can read", face.family))?;
                    parsed.push((Arc::clone(&face.data), index));
                }
            }
        }
        Ok(Self {
            template,
            installed_fonts: None,
        })
    }

    /// The backend drawing a file's own fonts where they are installed
    /// (Office's own faces first, then the system's), over the faces it was
    /// made with: as Excel and PowerPoint draw them, so text is as wide as
    /// there and cells and text boxes break it where they do.
    pub fn with_installed_fonts(mut self) -> Self {
        self.installed_fonts = Some(Rc::new(RefCell::new(BTreeSet::new())));
        self
    }

    /// Gives an existing surface — a PDF page, say — the backend's faces.
    pub fn register_on(&self, painter: &SkiaPainter) {
        for (family, weight, italic, typeface) in self.template.registered_faces_styled() {
            painter.register_face_styled(family, weight, italic, typeface);
        }
    }
}

impl Backend for SkiaBackend {
    type Painter = SkiaPainter;

    fn surface(&self, width: u32, height: u32) -> Option<SkiaPainter> {
        let painter = SkiaPainter::new(width.max(1) as i32, height.max(1) as i32)?;
        self.register_on(&painter);
        Some(painter)
    }

    fn bgra(painter: &SkiaPainter) -> Option<(Vec<u8>, u32, u32)> {
        let (pixels, width, height) = painter.read_bgra_premultiplied()?;
        Some((pixels, width as u32, height as u32))
    }

    fn png(painter: &SkiaPainter) -> Option<Vec<u8>> {
        painter.encode_png()
    }

    fn page(&self, width: f64, height: f64) -> Option<SkiaPainter> {
        let page = SkiaPainter::new_recording(width, height)?;
        self.register_on(&page);
        Some(page)
    }

    fn pdf(pages: &[SkiaPainter], _size: (f64, f64)) -> Result<Vec<u8>, String> {
        // Each recording knows its size.
        SkiaPainter::encode_pdf(pages)
    }

    fn prepare_fonts(&self, package: &[u8]) {
        use jade_docx_native_render::system_fonts::{families_named_in, FontLibrary};
        let Some(looked_for) = &self.installed_fonts else {
            return;
        };
        let families: Vec<String> = families_named_in(package)
            .into_iter()
            .filter(|family| looked_for.borrow_mut().insert(family.to_lowercase()))
            .collect();
        if families.is_empty() {
            return;
        }
        let (faces, _missing) =
            FontLibrary::system().faces_for(families.iter().map(String::as_str));
        // Registered after the stand-ins, so the painter picks these.
        for face in faces {
            let registered = std::fs::read(&face.path).ok().and_then(|bytes| {
                self.template.register_font_bytes_styled(
                    face.family.as_str(),
                    face.weight,
                    face.italic,
                    &bytes,
                    face.index as usize,
                )
            });
            if registered.is_none() {
                eprintln!("font {} skipped: {}", face.family, face.path.display());
            }
        }
    }
}

/// The native app's Word document drives the shared ribbon commands.
impl crate::word::WordSession for jade_docx_native_render::document::NativeDocument {
    fn word_document(&self) -> &jade_docx_editor::word_document::WordDocument {
        NativeDocument::word_document(self)
    }

    fn selection_range(&self) -> Option<jade_selection::SelectionRange> {
        NativeDocument::selection_range(self)
    }

    fn set_selection_range(&mut self, range: Option<jade_selection::SelectionRange>) {
        NativeDocument::set_selection_range(self, range)
    }

    fn apply_command(
        &mut self,
        command: jade_docx_headless_api::HeadlessCommand,
        keep_selection: bool,
    ) -> Result<(), String> {
        NativeDocument::apply_command(self, command, keep_selection)
    }

    fn apply_commands(
        &mut self,
        commands: Vec<jade_docx_headless_api::HeadlessCommand>,
        keep_selection: bool,
    ) -> Result<(), String> {
        NativeDocument::apply_commands(self, commands, keep_selection)
    }

    fn execute_command(
        &mut self,
        command: Box<dyn jade_docx_editor::commands::Command>,
        keep_selection: bool,
    ) -> Result<(), String> {
        NativeDocument::execute_command(self, command, keep_selection)
    }

    fn cell_id_for_paragraph(&self, paragraph: usize) -> Option<usize> {
        self.renderer().cell_id_for_paragraph(paragraph)
    }

    fn cell_layout(&self, cell_id: usize) -> Option<&crate::word::CellLayout> {
        self.renderer().cell_layout(cell_id)
    }

    fn cell_layouts(&self) -> &[crate::word::CellLayout] {
        self.renderer().cell_layouts_debug_ref()
    }

    fn table_order_for(&self, table_id: usize) -> Option<usize> {
        self.renderer().table_order_for(table_id)
    }

    fn hyperlink_regions(&self) -> Vec<jade_docx_editor::canvas_renderer::HyperlinkRegion> {
        self.renderer()
            .derive_hyperlink_regions_from_document(NativeDocument::word_document(self))
    }

    fn select_cells(&mut self, anchor_cell: usize, focus_cell: usize) -> Result<(), String> {
        NativeDocument::select_cells(self, anchor_cell, focus_cell)
    }

    fn page_count(&self) -> Option<usize> {
        Some(NativeDocument::page_count(self))
    }

    fn position_metrics(
        &self,
        (paragraph, offset): crate::word::Position,
    ) -> Option<(f64, f64, f64)> {
        self.renderer()
            .caret_metrics_at(jade_selection::TextPosition::new(paragraph, offset))
            .map(|metrics| (metrics.x, metrics.absolute_top, metrics.height))
    }

    fn paragraph_page(&self, paragraph: usize) -> Option<usize> {
        self.renderer()
            .caret_metrics_at(jade_selection::TextPosition::new(paragraph, 0))
            .map(|metrics| self.page_and_offset(metrics.absolute_top).0 + 1)
            .or_else(|| self.renderer().cached_paragraph_start_page(paragraph))
    }

    fn text_regions(
        &self,
        start: crate::word::Position,
        end: crate::word::Position,
    ) -> Vec<jade_docx_editor::canvas_renderer::TextRegion> {
        use jade_selection::TextPosition;
        self.renderer().text_range_regions(
            TextPosition::new(start.0, start.1),
            TextPosition::new(end.0, end.1),
        )
    }
}

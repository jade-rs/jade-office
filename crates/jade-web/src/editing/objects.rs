//! 挿入 › オブジェクト in the browser: a file embedded as Word embeds one
//! (`jade_office_model::word::ole_objects`), shown as its icon or a Word
//! document's first page, drawn here with the page's own fonts.

use std::rc::Rc;

use jade_office_model::word::ole_objects::{object_picture, EmbeddedFile, WordOleObjects};

use super::EditableDocument;

impl EditableDocument {
    /// The first page of `docx` as a PNG, at this document's scale, and
    /// its size in CSS px.
    pub fn first_page_png(&self, docx: &[u8]) -> Result<(Vec<u8>, f64, f64), String> {
        let page = EditableDocument::open_resident(
            docx,
            self.scale,
            Rc::clone(&self.fonts),
            Rc::clone(&self.measurer),
            0..1,
        )?;
        let (width, height) = page.page_size();
        let png = page
            .page(0)
            .and_then(|painter| painter.encode_png())
            .ok_or("the object could not be drawn")?;
        Ok((png, width, height))
    }

    /// `name`'s `bytes` embedded at the caret: as its icon when `icon`,
    /// else a Word document as its first page.
    pub fn insert_object_file(
        &mut self,
        name: &str,
        bytes: Vec<u8>,
        icon: bool,
    ) -> Result<(), String> {
        let picture = object_picture(name, &bytes, icon, |docx| self.first_page_png(docx))?;
        self.insert_ole_object(name, bytes, picture.png, picture.size, picture.icon)
    }

    /// The file the picture at `paragraph`, `run` embeds, which a double
    /// click hands to the browser to save or open.
    pub fn object_file(&self, paragraph: usize, run: usize) -> Option<EmbeddedFile> {
        self.embedded_file_at(paragraph, run)
    }
}

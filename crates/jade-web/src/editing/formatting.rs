//! The ribbon's commands on the browser build's Word document: it is a
//! [`WordSession`], so it gets `jade-office-model`'s [`WordFormatting`] — the
//! same formatting, style and table commands the native app runs.

use jade_docx_editor::headless_api::apply_headless_commands_with_executor_to_document;
use jade_docx_editor::word_document::WordDocument;
use jade_docx_headless_api::{HeadlessCommand, HeadlessEditRequest};
pub use jade_office_model::word::{
    style_gallery, CellLayout, CharacterFormat, FormatState, GalleryStyle, ListKind,
    ParagraphBorders, ParagraphStyle, TableBorders, TableCell, TableCommand, WordFormatting,
    WordSession,
};
use jade_selection::SelectionRange;

use super::EditableDocument;

impl WordSession for EditableDocument {
    fn word_document(&self) -> &WordDocument {
        &self.word_document
    }

    fn selection_range(&self) -> Option<SelectionRange> {
        self.selection
    }

    fn set_selection_range(&mut self, range: Option<SelectionRange>) {
        self.selection = range;
        self.renderer.set_selection_range(range);
    }

    fn apply_command(
        &mut self,
        command: HeadlessCommand,
        keep_selection: bool,
    ) -> Result<(), String> {
        let before = self.selection;
        let after = apply_headless_commands_with_executor_to_document(
            &mut self.word_document,
            &mut self.executor,
            self.selection.take(),
            &HeadlessEditRequest {
                commands: vec![command],
            },
        );
        let after = match after {
            Ok(after) => after,
            Err(error) => {
                self.selection = before;
                return Err(format!("edit failed: {error:?}"));
            }
        };
        self.composition = None;
        self.dirty = true;
        self.selection = if keep_selection {
            before
        } else {
            after.or(before)
        };
        self.renderer.set_selection_range(self.selection);
        // A format can move every line after it, and a table edit every
        // paragraph: the whole document, as undo repaints it.
        self.repaint_all()
    }

    fn cell_id_for_paragraph(&self, paragraph: usize) -> Option<usize> {
        self.renderer.cell_id_for_paragraph(paragraph)
    }

    fn cell_layout(&self, cell_id: usize) -> Option<&CellLayout> {
        self.renderer.cell_layout(cell_id)
    }

    fn cell_layouts(&self) -> &[CellLayout] {
        self.renderer.cell_layouts_debug_ref()
    }

    fn table_order_for(&self, table_id: usize) -> Option<usize> {
        self.renderer.table_order_for(table_id)
    }
}

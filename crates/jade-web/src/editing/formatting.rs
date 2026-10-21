//! The ribbon's commands on the browser build's Word document: it is a
//! [`WordSession`], so it gets `jade-office-model`'s [`WordFormatting`] — the
//! same formatting, style and table commands the native app runs.

use jade_docx_editor::commands::{Command, CommandContext};
use jade_docx_editor::headless_api::apply_headless_commands_with_executor_to_document;
use jade_docx_editor::word_document::WordDocument;
use jade_docx_headless_api::{HeadlessCommand, HeadlessEditRequest};
pub use jade_office_model::word::{
    style_gallery, CellLayout, CellStep, CharacterFormat, FindOptions, FormatState, Found,
    GalleryStyle, Hyperlink, LinkDraft, LinkTarget, ListKind, ParagraphBorders, ParagraphStyle,
    TableBorders, TableCell, TableCommand, WordFind, WordFormatting, WordLinks, WordSession,
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
        self.finish_command(before, after, keep_selection)
    }

    fn apply_commands(
        &mut self,
        commands: Vec<HeadlessCommand>,
        keep_selection: bool,
    ) -> Result<(), String> {
        if commands.is_empty() {
            return Ok(());
        }
        let before = self.selection;
        self.executor.begin_group();
        let after = apply_headless_commands_with_executor_to_document(
            &mut self.word_document,
            &mut self.executor,
            self.selection.take(),
            &HeadlessEditRequest { commands },
        );
        let after = match after {
            Ok(after) => {
                self.executor.end_group();
                after
            }
            Err(error) => {
                // The ones that ran come back off: the batch is one action.
                let mut selection = before;
                let mut context = CommandContext::new(&mut self.word_document, &mut selection);
                self.executor.abort_group(&mut context);
                self.selection = before;
                self.repaint_all()?;
                return Err(format!("edit failed: {error:?}"));
            }
        };
        self.finish_command(before, after, keep_selection)
    }

    fn execute_command(
        &mut self,
        command: Box<dyn Command>,
        keep_selection: bool,
    ) -> Result<(), String> {
        let before = self.selection;
        let mut selection = self.selection;
        let mut context = CommandContext::new(&mut self.word_document, &mut selection);
        self.executor
            .execute(command, &mut context)
            .map_err(|error| format!("edit failed: {error:?}"))?;
        self.finish_command(before, selection, keep_selection)
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

    fn hyperlink_regions(&self) -> Vec<jade_docx_editor::canvas_renderer::HyperlinkRegion> {
        self.renderer
            .derive_hyperlink_regions_from_document(&self.word_document)
    }

    fn select_cells(&mut self, anchor_cell: usize, focus_cell: usize) -> Result<(), String> {
        let range = self
            .renderer
            .cell_selection_range(anchor_cell, focus_cell)
            .ok_or_else(|| "the cells are not in one table".to_string())?;
        self.composition = None;
        self.selection = Some(range);
        self.renderer.set_selection_range(Some(range));
        self.renderer.set_cell_selection(anchor_cell, focus_cell);
        Ok(())
    }

    fn page_count(&self) -> Option<usize> {
        Some(EditableDocument::page_count(self))
    }

    fn paragraph_page(&self, paragraph: usize) -> Option<usize> {
        self.renderer.cached_paragraph_start_page(paragraph)
    }
}

impl EditableDocument {
    /// The tail of a ribbon command: the selection it leaves, and the whole
    /// document repainted — a format can move every line after it, and a
    /// table edit every paragraph, as undo repaints it.
    fn finish_command(
        &mut self,
        before: Option<SelectionRange>,
        after: Option<SelectionRange>,
        keep_selection: bool,
    ) -> Result<(), String> {
        self.composition = None;
        self.dirty = true;
        self.selection = if keep_selection {
            before
        } else {
            after.or(before)
        };
        self.renderer.set_selection_range(self.selection);
        self.repaint_all()
    }
}

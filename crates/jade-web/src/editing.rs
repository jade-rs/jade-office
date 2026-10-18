//! A Word document open for editing in the browser build.
//!
//! The same sequence the native app's `NativeDocument` runs, on the pure-Rust
//! host: edits go through the editor's command executor so they can be
//! undone, then the renderer repaints from the edited paragraph down. A
//! click is hit-tested against the last paint; the caret is reported, not
//! painted, so it can move without a repaint. An IME's composition is shown
//! in the text as it changes but kept off the undo history, so one undo
//! takes back the word it committed.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use jade_docx_editor::canvas_renderer::CanvasRenderer;
use jade_docx_editor::commands::{CommandContext, CommandExecutor};
use jade_docx_editor::headless_api::{
    apply_headless_commands_to_document, apply_headless_commands_with_executor_to_document,
};
use jade_docx_editor::render_host::PAGE_GAP_PX;
use jade_docx_editor::word_document::WordDocument;
use jade_docx_editor::Options;
use jade_docx_headless_api::{HeadlessCommand, HeadlessDeleteDirection, HeadlessEditRequest};
use jade_docx_layout::shaping_measurer::ShapingMeasurer;
use jade_selection::{SelectionRange, TextPosition};
use jade_tiny_skia_painter::{FontBook, TinySkiaPainter};

use crate::document::{TinySkiaRenderHost, INITIAL_PAGE};

/// Where the caret is drawn: in the document's coordinate space, which
/// stacks the pages with a gap between them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaretRect {
    pub x: f64,
    pub document_y: f64,
    pub width: f64,
    pub height: f64,
}

pub struct EditableDocument {
    word_document: WordDocument,
    executor: CommandExecutor,
    /// How many executor entries each user action left, newest last, and
    /// the same for the actions undone.
    history: Vec<usize>,
    redo_history: Vec<usize>,
    selection: Option<SelectionRange>,
    /// The composition in the text: its paragraph and byte range.
    composition: Option<(usize, usize, usize)>,
    renderer: CanvasRenderer<TinySkiaPainter>,
    pages: Rc<RefCell<Vec<TinySkiaPainter>>>,
    rendered_pages: Rc<Cell<usize>>,
    pass_covers_whole_document: Rc<Cell<bool>>,
    options: Options,
    page_height: f64,
    dirty: bool,
}

impl EditableDocument {
    /// Reads `.docx` bytes and paints every page at `scale` device px per
    /// CSS px.
    pub fn open(
        bytes: &[u8],
        scale: f64,
        fonts: Rc<FontBook>,
        measurer: Rc<ShapingMeasurer>,
    ) -> Result<Self, String> {
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        let mut options = Options::default();
        options.show_formatting_marks = false;
        let word_document = futures::executor::block_on(WordDocument::load(bytes, &options))
            .map_err(|error| format!("cannot read the document: {error}"))?;
        if word_document.document.is_none() {
            return Err("the document has no main part".to_string());
        }
        // The section properties decide the page box: a first pass finds it,
        // and the renderer kept is the one built to it.
        let mut opened = Self::render(
            &word_document,
            &options,
            INITIAL_PAGE,
            scale,
            &fonts,
            &measurer,
        )?;
        let size = opened.0.layout_page_size();
        if size != INITIAL_PAGE && size.0 > 1.0 && size.1 > 1.0 {
            opened = Self::render(&word_document, &options, size, scale, &fonts, &measurer)?;
        }
        let (renderer, pages, rendered_pages, pass_covers_whole_document) = opened;
        let page_height = renderer.layout_page_size().1;
        Ok(Self {
            word_document,
            executor: CommandExecutor::default(),
            history: Vec::new(),
            redo_history: Vec::new(),
            selection: None,
            composition: None,
            renderer,
            pages,
            rendered_pages,
            pass_covers_whole_document,
            options,
            page_height,
            dirty: false,
        })
    }

    #[allow(clippy::type_complexity)]
    fn render(
        word_document: &WordDocument,
        options: &Options,
        (width, height): (f64, f64),
        scale: f64,
        fonts: &Rc<FontBook>,
        measurer: &Rc<ShapingMeasurer>,
    ) -> Result<
        (
            CanvasRenderer<TinySkiaPainter>,
            Rc<RefCell<Vec<TinySkiaPainter>>>,
            Rc<Cell<usize>>,
            Rc<Cell<bool>>,
        ),
        String,
    > {
        let host =
            TinySkiaRenderHost::new(width, height, scale, Rc::clone(fonts), Rc::clone(measurer))
                .ok_or_else(|| "cannot allocate a page surface".to_string())?;
        let pages = Rc::clone(&host.pages);
        let rendered = Rc::clone(&host.rendered_pages);
        let whole = Rc::clone(&host.pass_covers_whole_document);
        let mut renderer = CanvasRenderer::<TinySkiaPainter>::with_host(Box::new(host), false)
            .map_err(|_| "renderer setup failed".to_string())?;
        renderer.sync_document_header_footer_stories(word_document);
        renderer.set_use_selection_overlay(false);
        renderer.mark_full_repaint();
        let document = word_document
            .document
            .as_ref()
            .ok_or_else(|| "the document has no main part".to_string())?
            .clone();
        renderer
            .render_with_numbering(
                &document,
                options,
                word_document.numbering_part.as_ref(),
                word_document.header_lines.as_deref(),
                word_document.footer_lines.as_deref(),
                Some(&word_document.comments),
            )
            .map_err(|_| "render failed".to_string())?;
        if let Some(error) = renderer.take_render_error() {
            return Err(error);
        }
        Ok((renderer, pages, rendered, whole))
    }

    // ---- pages --------------------------------------------------------------

    pub fn page_count(&self) -> usize {
        self.rendered_pages.get().min(self.pages.borrow().len())
    }

    /// Page `index`'s surface, as last painted.
    pub fn page(&self, index: usize) -> Option<TinySkiaPainter> {
        if index >= self.page_count() {
            return None;
        }
        self.pages.borrow().get(index).cloned()
    }

    /// The page box, in CSS px.
    pub fn page_size(&self) -> (f64, f64) {
        self.renderer.layout_page_size()
    }

    /// From the top of one page to the top of the next.
    pub fn page_pitch(&self) -> f64 {
        self.page_height + PAGE_GAP_PX
    }

    /// Which page a document coordinate is on, and where on it.
    pub fn page_and_offset(&self, document_y: f64) -> (usize, f64) {
        let pitch = self.page_pitch().max(1.0);
        let page = ((document_y / pitch).floor().max(0.0) as usize)
            .min(self.page_count().saturating_sub(1));
        (page, document_y - page as f64 * pitch)
    }

    /// The document coordinate of a point on a page.
    pub fn document_y(&self, page: usize, y_on_page: f64) -> f64 {
        y_on_page + page as f64 * self.page_pitch()
    }

    // ---- text ---------------------------------------------------------------

    pub fn paragraph_count(&self) -> usize {
        self.word_document.paragraph_count()
    }

    pub fn paragraph_text(&self, paragraph: usize) -> Option<String> {
        self.word_document
            .paragraph(paragraph)
            .map(|paragraph| paragraph.runs.iter().map(|run| run.text.as_str()).collect())
    }

    // ---- caret --------------------------------------------------------------

    /// The text position under a point in document coordinates.
    pub fn position_at(&self, x: f64, document_y: f64) -> Option<(usize, usize)> {
        self.renderer
            .hit_test_position(x, document_y)
            .map(|position| (position.paragraph, position.offset))
    }

    /// Moves the caret, reporting where it is drawn.
    pub fn place_caret(&mut self, paragraph: usize, offset: usize) -> Option<CaretRect> {
        self.composition = None;
        self.set_caret(paragraph, offset);
        self.caret_rect()
    }

    fn set_caret(&mut self, paragraph: usize, offset: usize) {
        let range = SelectionRange::collapsed(TextPosition::new(paragraph, offset));
        self.selection = Some(range);
        self.renderer.set_selection_range(Some(range));
    }

    /// Where the caret is drawn now.
    pub fn caret_rect(&self) -> Option<CaretRect> {
        self.renderer.caret_metrics().map(|metrics| CaretRect {
            x: metrics.x,
            document_y: metrics.absolute_top,
            width: metrics.width.max(1.0),
            height: metrics.height,
        })
    }

    /// The caret's paragraph and byte offset.
    pub fn caret(&self) -> Option<(usize, usize)> {
        self.selection
            .map(|range| (range.focus.paragraph, range.focus.offset))
    }

    // ---- edits --------------------------------------------------------------

    /// Types `text` at the caret.
    pub fn insert_text(&mut self, text: &str) -> Result<(), String> {
        let (paragraph, offset) = self.caret().ok_or("no caret")?;
        self.apply(
            paragraph,
            HeadlessCommand::InsertText {
                paragraph,
                offset,
                text: text.to_string(),
            },
            true,
        )?;
        self.set_caret(paragraph, offset + text.len());
        Ok(())
    }

    /// Backspace (`backwards`) or Delete: one character, joining paragraphs
    /// at their edges.
    pub fn delete(&mut self, backwards: bool) -> Result<(), String> {
        let (paragraph, offset) = self.caret().ok_or("no caret")?;
        let text = self.paragraph_text(paragraph).unwrap_or_default();
        let caret = if backwards {
            if offset == 0 {
                if paragraph == 0 {
                    return Ok(());
                }
                let before = self.paragraph_text(paragraph - 1).unwrap_or_default();
                (paragraph - 1, before.len())
            } else {
                let previous = text[..offset.min(text.len())]
                    .chars()
                    .next_back()
                    .map_or(1, char::len_utf8);
                (paragraph, offset - previous)
            }
        } else {
            if offset >= text.len() && paragraph + 1 >= self.paragraph_count() {
                return Ok(());
            }
            (paragraph, offset)
        };
        self.apply(
            paragraph,
            HeadlessCommand::DeleteText {
                paragraph,
                offset,
                direction: if backwards {
                    HeadlessDeleteDirection::Backward
                } else {
                    HeadlessDeleteDirection::Forward
                },
                count: 1,
            },
            true,
        )?;
        self.set_caret(caret.0, caret.1);
        Ok(())
    }

    /// Enter: splits the paragraph at the caret.
    pub fn insert_paragraph_break(&mut self) -> Result<(), String> {
        let (paragraph, offset) = self.caret().ok_or("no caret")?;
        self.apply(
            paragraph,
            HeadlessCommand::InsertParagraphBreak {
                paragraph,
                offset,
                count: 1,
            },
            true,
        )?;
        self.set_caret(paragraph + 1, 0);
        Ok(())
    }

    /// Shows an IME's composition at the caret in place of the previous one,
    /// off the undo history.
    pub fn set_composition(&mut self, text: &str) -> Result<(), String> {
        let (paragraph, start, end) = match self.composition {
            Some(composition) => composition,
            None => {
                let (paragraph, offset) = self.caret().ok_or("no caret")?;
                (paragraph, offset, offset)
            }
        };
        self.apply(
            paragraph,
            HeadlessCommand::ReplaceRange {
                start_paragraph: paragraph,
                start_offset: start,
                end_paragraph: paragraph,
                end_offset: end,
                text: text.to_string(),
            },
            false,
        )?;
        self.composition = Some((paragraph, start, start + text.len()));
        self.set_caret(paragraph, start + text.len());
        Ok(())
    }

    /// Replaces the composition with the committed `text`, as one undoable
    /// edit.
    pub fn commit_composition(&mut self, text: &str) -> Result<(), String> {
        if let Some((paragraph, start, end)) = self.composition.take() {
            self.apply(
                paragraph,
                HeadlessCommand::ReplaceRange {
                    start_paragraph: paragraph,
                    start_offset: start,
                    end_paragraph: paragraph,
                    end_offset: end,
                    text: String::new(),
                },
                false,
            )?;
            self.set_caret(paragraph, start);
        }
        if text.is_empty() {
            return Ok(());
        }
        self.insert_text(text)
    }

    /// The composition in the text, if one is open: paragraph and byte
    /// range.
    pub fn composition(&self) -> Option<(usize, usize, usize)> {
        self.composition
    }

    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_history.is_empty()
    }

    pub fn undo(&mut self) -> Result<bool, String> {
        self.step_history(true)
    }

    pub fn redo(&mut self) -> Result<bool, String> {
        self.step_history(false)
    }

    fn step_history(&mut self, backwards: bool) -> Result<bool, String> {
        let Some(steps) = (if backwards {
            self.history.pop()
        } else {
            self.redo_history.pop()
        }) else {
            return Ok(false);
        };
        self.composition = None;
        let mut selection = self.selection.take();
        {
            let mut context = CommandContext::new(&mut self.word_document, &mut selection);
            for _ in 0..steps {
                let stepped = if backwards {
                    self.executor.undo(&mut context)
                } else {
                    self.executor.redo(&mut context)
                }
                .map_err(|error| format!("history failed: {error:?}"))?;
                if !stepped {
                    break;
                }
            }
        }
        self.selection = selection;
        self.renderer.set_selection_range(self.selection);
        if backwards {
            self.redo_history.push(steps);
        } else {
            self.history.push(steps);
        }
        self.dirty = true;
        self.repaint_all()?;
        Ok(true)
    }

    /// Applies `command` — recorded for undo when `recorded` — and repaints
    /// from `paragraph` down.
    fn apply(
        &mut self,
        paragraph: usize,
        command: HeadlessCommand,
        recorded: bool,
    ) -> Result<(), String> {
        let request = HeadlessEditRequest {
            commands: vec![command],
        };
        if recorded {
            self.selection = apply_headless_commands_with_executor_to_document(
                &mut self.word_document,
                &mut self.executor,
                self.selection.take(),
                &request,
            )
            .map_err(|error| format!("edit failed: {error:?}"))?;
            self.history.push(1);
            self.redo_history.clear();
        } else {
            apply_headless_commands_to_document(&mut self.word_document, &request)
                .map_err(|error| format!("edit failed: {error:?}"))?;
        }
        self.dirty = true;
        self.repaint_from(paragraph)
    }

    /// The renderer's incremental sequence, through every page: an edit can
    /// move content onto any page after it.
    fn repaint_from(&mut self, paragraph: usize) -> Result<(), String> {
        let document = self
            .word_document
            .document
            .as_ref()
            .ok_or_else(|| "document went missing".to_string())?
            .clone();
        let last_page = self.page_count().max(1) + 1;
        self.pass_covers_whole_document.set(true);
        self.renderer
            .prepare_incremental_repaint(paragraph, last_page);
        let result = self.renderer.render_bounded_to_page_with_parts(
            document,
            &self.options,
            self.word_document.numbering_part.as_ref(),
            self.word_document.header_lines.as_deref(),
            self.word_document.footer_lines.as_deref(),
            last_page,
        );
        result.map_err(|_| "repaint failed".to_string())?;
        if let Some(error) = self.renderer.take_render_error() {
            return Err(error);
        }
        Ok(())
    }

    /// Every page from scratch: undo can change content anywhere.
    fn repaint_all(&mut self) -> Result<(), String> {
        let document = self
            .word_document
            .document
            .as_ref()
            .ok_or_else(|| "document went missing".to_string())?
            .clone();
        self.renderer
            .sync_document_header_footer_stories(&self.word_document);
        self.renderer.invalidate_paragraph_content_hash_cache();
        self.renderer.mark_full_repaint();
        self.pass_covers_whole_document.set(true);
        self.renderer
            .render_with_numbering(
                &document,
                &self.options,
                self.word_document.numbering_part.as_ref(),
                self.word_document.header_lines.as_deref(),
                self.word_document.footer_lines.as_deref(),
                Some(&self.word_document.comments),
            )
            .map_err(|_| "repaint failed".to_string())?;
        if let Some(error) = self.renderer.take_render_error() {
            return Err(error);
        }
        Ok(())
    }

    // ---- saving -------------------------------------------------------------

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// The document as `.docx` bytes, every edit in it; marks it saved.
    pub fn export_docx(&mut self) -> Result<Vec<u8>, String> {
        let bytes = self
            .word_document
            .export_docx()
            .map_err(|error| format!("cannot save: {error:?}"))?;
        self.dirty = false;
        Ok(bytes)
    }
}

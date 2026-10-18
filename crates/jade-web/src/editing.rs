//! A Word document open for editing in the browser build.
//!
//! The same sequence the native app's `NativeDocument` runs, on the pure-Rust
//! host: edits go through the editor's command executor so they can be
//! undone, then the renderer repaints from the edited paragraph down. A
//! click is hit-tested against the last paint; the caret is reported, not
//! painted, so it can move without a repaint. An IME's composition is shown
//! in the text as it changes but kept off the undo history, so one undo
//! takes back the word it committed. A selection is painted by the
//! renderer, as in the native app; typing, deleting and pasting replace it.

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

/// Where an arrow, Home or End takes the caret.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    Left,
    Right,
    Up,
    Down,
    LineStart,
    LineEnd,
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
    /// See [`Self::set_dark_paper`].
    dark_paper: Rc<Cell<bool>>,
    options: Options,
    page_height: f64,
    dirty: bool,
    /// The x a run of Up and Down keeps to, so passing a short line does
    /// not pull the caret to its end for good.
    goal_x: Option<f64>,
    /// The line the caret is on when that is not where it is drawn: a
    /// wrapped line's first position is drawn at the end of the line above,
    /// and Home there means this line.
    line_y: Option<f64>,
    /// The first page repainted since the shell last asked.
    repainted_from: Option<usize>,
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
        let (renderer, pages, rendered_pages, pass_covers_whole_document, dark_paper) = opened;
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
            dark_paper,
            options,
            page_height,
            dirty: false,
            goal_x: None,
            line_y: None,
            repainted_from: None,
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
        let dark = Rc::clone(&host.dark_paper);
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
        Ok((renderer, pages, rendered, whole, dark))
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

    /// Moves the caret, reporting where it is drawn. A selection it
    /// collapses is repainted without its highlight.
    pub fn place_caret(&mut self, paragraph: usize, offset: usize) -> Option<CaretRect> {
        self.composition = None;
        self.goal_x = None;
        self.line_y = None;
        let highlighted = self.highlighted_span();
        self.set_caret(paragraph, offset);
        if let Some((from, to)) = highlighted {
            // The text is as it was; a failed repaint leaves only a stale
            // highlight, which the next paint clears.
            let _ = self.repaint_highlight(from, to);
        }
        self.caret_rect()
    }

    /// Selects from `anchor` to `focus` and repaints so the highlight shows.
    pub fn select(&mut self, anchor: (usize, usize), focus: (usize, usize)) -> Result<(), String> {
        if anchor == focus {
            self.place_caret(focus.0, focus.1);
            return Ok(());
        }
        self.composition = None;
        self.goal_x = None;
        self.line_y = None;
        let highlighted = self.highlighted_span();
        let range = SelectionRange::new(
            TextPosition::new(anchor.0, anchor.1),
            TextPosition::new(focus.0, focus.1),
        );
        self.selection = Some(range);
        self.renderer.set_selection_range(Some(range));
        let (start, end) = range.normalized();
        let (from, to) = highlighted.map_or((start.paragraph, end.paragraph), |(from, to)| {
            (from.min(start.paragraph), to.max(end.paragraph))
        });
        self.repaint_highlight(from, to)
    }

    /// The first page repainted since the last call, for a shell that keeps
    /// images of the pages: the ones before it are as they were.
    pub fn take_repainted_from(&mut self) -> Option<usize> {
        self.repainted_from.take()
    }

    /// Everything, from the first paragraph's start to the last one's end.
    pub fn select_all(&mut self) -> Result<(), String> {
        let last = self.paragraph_count().saturating_sub(1);
        let end = self.paragraph_text(last).map_or(0, |text| text.len());
        self.select((0, 0), (last, end))
    }

    /// The selection as `(anchor, focus)`, or `None` when it is a caret.
    pub fn selection(&self) -> Option<((usize, usize), (usize, usize))> {
        let range = self.selection.filter(|range| !range.is_collapsed())?;
        Some((
            (range.anchor.paragraph, range.anchor.offset),
            (range.focus.paragraph, range.focus.offset),
        ))
    }

    /// The selected text for the clipboard, paragraphs joined by newlines —
    /// which a paste turns back into paragraph breaks.
    pub fn selected_text(&self) -> Option<String> {
        let range = self.selection.filter(|range| !range.is_collapsed())?;
        let (start, end) = range.normalized();
        if start.paragraph == end.paragraph {
            let text = self.paragraph_text(start.paragraph)?;
            return Some(text.get(start.offset..end.offset)?.to_string());
        }
        let mut parts = vec![self
            .paragraph_text(start.paragraph)?
            .get(start.offset..)?
            .to_string()];
        for paragraph in (start.paragraph + 1)..end.paragraph {
            parts.push(self.paragraph_text(paragraph).unwrap_or_default());
        }
        parts.push(
            self.paragraph_text(end.paragraph)?
                .get(..end.offset)?
                .to_string(),
        );
        Some(parts.join("\n"))
    }

    /// An arrow, Home or End; with `extend` (Shift) the selection's far end
    /// moves and its anchor stays.
    pub fn move_caret(&mut self, motion: Motion, extend: bool) -> Result<(), String> {
        let range = self.selection.ok_or("no caret")?;
        let anchor = (range.anchor.paragraph, range.anchor.offset);
        let focus = (range.focus.paragraph, range.focus.offset);
        let (start, end) = range.normalized();
        let goal_x = self.goal_x;
        let target = match motion {
            // Without Shift, an arrow collapses a selection to its edge.
            Motion::Left if !extend && !range.is_collapsed() => (start.paragraph, start.offset),
            Motion::Right if !extend && !range.is_collapsed() => (end.paragraph, end.offset),
            Motion::Left | Motion::Right => self.step_character(focus, motion == Motion::Right),
            Motion::Up | Motion::Down | Motion::LineStart | Motion::LineEnd => {
                let rect = self.rect_at(focus).ok_or("the caret has no position")?;
                let line = self.line_y.unwrap_or(rect.document_y + rect.height / 2.0);
                let (x, y) = match motion {
                    Motion::Up => (goal_x.unwrap_or(rect.x), line - rect.height),
                    Motion::Down => (goal_x.unwrap_or(rect.x), line + rect.height),
                    Motion::LineStart => (f64::MIN / 4.0, line),
                    _ => (f64::MAX / 4.0, line),
                };
                let target = self.position_at(x, y).unwrap_or(focus);
                if extend {
                    self.select(anchor, target)?;
                } else {
                    self.place_caret(target.0, target.1);
                }
                self.goal_x = Some(x);
                self.line_y = self.line_after_wrap(target, y);
                return Ok(());
            }
        };
        if extend {
            self.select(anchor, target)
        } else {
            self.place_caret(target.0, target.1);
            Ok(())
        }
    }

    /// One character along from `position`, across paragraph ends.
    fn step_character(
        &self,
        (paragraph, offset): (usize, usize),
        forwards: bool,
    ) -> (usize, usize) {
        let text = self.paragraph_text(paragraph).unwrap_or_default();
        if forwards {
            match text.get(offset..).and_then(|rest| rest.chars().next()) {
                Some(next) => (paragraph, offset + next.len_utf8()),
                None if paragraph + 1 < self.paragraph_count() => (paragraph + 1, 0),
                None => (paragraph, offset),
            }
        } else {
            match text
                .get(..offset)
                .and_then(|before| before.chars().next_back())
            {
                Some(previous) => (paragraph, offset - previous.len_utf8()),
                None if paragraph > 0 => (
                    paragraph - 1,
                    self.paragraph_text(paragraph - 1)
                        .map_or(0, |text| text.len()),
                ),
                None => (paragraph, offset),
            }
        }
    }

    /// Where a caret at `position` would be drawn, leaving the selection as
    /// it is.
    fn rect_at(&mut self, (paragraph, offset): (usize, usize)) -> Option<CaretRect> {
        let selection = self.selection;
        self.renderer
            .set_selection_range(Some(SelectionRange::collapsed(TextPosition::new(
                paragraph, offset,
            ))));
        let rect = self.caret_rect();
        self.renderer.set_selection_range(selection);
        rect
    }

    /// The middle of the line `position` starts, when it is a wrapped
    /// line's first position — drawn at the end of the line above — and
    /// `aimed_at` was nearer that line than the one above.
    fn line_after_wrap(&mut self, position: (usize, usize), aimed_at: f64) -> Option<f64> {
        let next = self.step_character(position, true);
        if next.0 != position.0 || next == position {
            return None;
        }
        let here = self.rect_at(position)?;
        let after = self.rect_at(next)?;
        let (above, below) = (
            here.document_y + here.height / 2.0,
            after.document_y + after.height / 2.0,
        );
        (below > above + here.height / 2.0 && (below - aimed_at).abs() < (above - aimed_at).abs())
            .then_some(below)
    }

    /// The paragraphs a painted highlight covers, if there is one.
    fn highlighted_span(&self) -> Option<(usize, usize)> {
        self.selection
            .filter(|range| !range.is_collapsed())
            .map(|range| {
                let (start, end) = range.normalized();
                (start.paragraph, end.paragraph)
            })
    }

    /// The page `paragraph` starts on, as last painted.
    fn paragraph_page(&mut self, paragraph: usize) -> usize {
        self.rect_at((paragraph, 0))
            .map_or(0, |rect| self.page_and_offset(rect.document_y).0)
    }

    /// Repaints the pages paragraphs `from` to `to` are on — a highlight
    /// came or went, the text did not change, so nothing after them moves.
    fn repaint_highlight(&mut self, from: usize, to: usize) -> Result<(), String> {
        let end = self.paragraph_text(to).map_or(0, |text| text.len());
        let last_page = self.rect_at((to, end)).map_or(self.page_count(), |rect| {
            self.page_and_offset(rect.document_y + rect.height).0 + 1
        });
        self.repaint_through(from, last_page)
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

    /// Types `text` at the caret, or in place of the selection.
    pub fn insert_text(&mut self, text: &str) -> Result<(), String> {
        self.goal_x = None;
        if self.selection().is_some() {
            return self.replace_selection(text);
        }
        if text.is_empty() {
            return Ok(());
        }
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
        self.goal_x = None;
        if self.selection().is_some() {
            return self.replace_selection("");
        }
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

    /// Enter: splits the paragraph at the caret, after removing the
    /// selection — as one undoable step.
    pub fn insert_paragraph_break(&mut self) -> Result<(), String> {
        self.goal_x = None;
        if self.selection().is_some() {
            let since = self.history.len();
            self.replace_selection("")?;
            self.split_at_caret()?;
            self.group_history(since);
            return Ok(());
        }
        self.split_at_caret()
    }

    fn split_at_caret(&mut self) -> Result<(), String> {
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

    /// Pastes `text` as typing would, its lines becoming paragraphs; one
    /// undo takes it all back.
    pub fn paste(&mut self, text: &str) -> Result<(), String> {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        let since = self.history.len();
        let mut lines = text.split('\n');
        let result = (|| {
            self.insert_text(lines.next().unwrap_or_default())?;
            for line in lines {
                self.split_at_caret()?;
                self.insert_text(line)?;
            }
            Ok(())
        })();
        self.group_history(since);
        result
    }

    /// Replaces the selection with `text`; the caret follows the text.
    fn replace_selection(&mut self, text: &str) -> Result<(), String> {
        let range = self
            .selection
            .filter(|range| !range.is_collapsed())
            .ok_or("no selection")?;
        let (start, end) = range.normalized();
        self.apply(
            start.paragraph,
            HeadlessCommand::ReplaceRange {
                start_paragraph: start.paragraph,
                start_offset: start.offset,
                end_paragraph: end.paragraph,
                end_offset: end.offset,
                text: text.to_string(),
            },
            true,
        )?;
        self.set_caret(start.paragraph, start.offset + text.len());
        Ok(())
    }

    /// Folds the history entries added since `since` into one user action.
    fn group_history(&mut self, since: usize) {
        if self.history.len() > since + 1 {
            let steps = self.history.drain(since..).sum();
            self.history.push(steps);
        }
    }

    /// Shows an IME's composition at the caret in place of the previous one,
    /// off the undo history. A selection goes first, as typing over it would.
    pub fn set_composition(&mut self, text: &str) -> Result<(), String> {
        if self.composition.is_none() && self.selection().is_some() {
            self.replace_selection("")?;
        }
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
        self.line_y = None;
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
        let last_page = self.page_count().max(1) + 1;
        self.repaint_through(paragraph, last_page)
    }

    /// The incremental sequence from `paragraph` through page `last_page`
    /// (1-based); only a pass through every page may change the page count.
    fn repaint_through(&mut self, paragraph: usize, last_page: usize) -> Result<(), String> {
        let document = self
            .word_document
            .document
            .as_ref()
            .ok_or_else(|| "document went missing".to_string())?
            .clone();
        let first_page = self.paragraph_page(paragraph);
        self.repainted_from = Some(
            self.repainted_from
                .map_or(first_page, |page| page.min(first_page)),
        );
        self.pass_covers_whole_document
            .set(last_page > self.page_count());
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

    /// Shows the pages on dark paper, or on the document's own, and repaints
    /// them if that changed anything. Returns whether they were repainted.
    ///
    /// Only the pages on screen change: the document keeps its colours, and
    /// [`Self::export_docx`] writes them.
    pub fn set_dark_paper(&mut self, on: bool) -> Result<bool, String> {
        if self.dark_paper.replace(on) == on {
            return Ok(false);
        }
        // The whole pool: a surface reused when the document grows must not
        // come back on the other paper.
        for page in self.pages.borrow().iter() {
            page.set_dark_paper(on);
        }
        self.repaint_all()?;
        Ok(true)
    }

    /// Every page from scratch: undo can change content anywhere.
    fn repaint_all(&mut self) -> Result<(), String> {
        self.repainted_from = Some(0);
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

//! A Word document open for editing in the browser build.
//!
//! The same sequence the native app's `NativeDocument` runs, on the pure-Rust
//! host: edits go through the editor's command executor so they can be
//! undone, then the renderer repaints from the edited paragraph down. A
//! click is hit-tested against the last paint; the caret is reported, not
//! painted, so it can move without a repaint. An IME's composition is shown
//! in the text as it changes but kept off the undo history, so one undo
//! takes back the word it committed. A selection is not painted either: the
//! renderer works out its highlight from the last paint
//! ([`EditableDocument::selection_rects`]) and the shell draws it over the
//! pages, so selecting — a drag above all — repaints nothing. Typing,
//! deleting and pasting replace it.

use std::cell::{Cell, RefCell};
use std::ops::{Range, RangeInclusive};
use std::rc::Rc;

use jade_docx_editor::canvas_renderer::{CanvasRenderer, CaretAffinity};
use jade_docx_editor::commands::{CommandContext, CommandExecutor};
use jade_docx_editor::headless_api::{
    apply_headless_commands_to_document, apply_headless_commands_with_executor_to_document,
};
use jade_docx_editor::render_host::PAGE_GAP_PX;
use jade_docx_editor::word_document::{FloatingObjectTarget, WordDocument};
use jade_docx_editor::Options;
use jade_docx_headless_api::{HeadlessCommand, HeadlessDeleteDirection, HeadlessEditRequest};
use jade_docx_layout::shaping_measurer::ShapingMeasurer;
use jade_office_model::word::picture::PlacedObject;
use jade_selection::{SelectionRange, TextPosition};
use jade_tiny_skia_painter::{FontBook, TinySkiaPainter};

use crate::document::{is_placeholder, placeholder, surface, TinySkiaRenderHost, INITIAL_PAGE};

mod autocorrect;
mod formatting;
pub use formatting::{
    style_gallery, CellStep, CharacterFormat, FindOptions, FormatState, Found, GalleryStyle,
    Hyperlink, LinkDraft, LinkTarget, ListKind, ParagraphBorders, ParagraphStyle, TableBorders,
    TableCell, TableCommand, WordFind, WordFormatting, WordLinks, WordSession,
};

/// Where the caret is drawn: in the document's coordinate space, which
/// stacks the pages with a gap between them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaretRect {
    pub x: f64,
    pub document_y: f64,
    pub width: f64,
    pub height: f64,
}

/// One rectangle of the selection's highlight, in CSS px of page `page`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HighlightRect {
    pub page: usize,
    pub x: f64,
    pub y: f64,
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
    /// To the start of the word before the caret (⌥← on a Mac, Ctrl+←
    /// elsewhere), across a paragraph's start.
    WordLeft,
    /// To the end of the word after the caret, across a paragraph's end.
    WordRight,
    /// The start and the end of the document's body (⌘↑ and ⌘↓, or
    /// Ctrl+Home and Ctrl+End).
    DocumentStart,
    DocumentEnd,
}

pub struct EditableDocument {
    word_document: WordDocument,
    /// The undo history: one entry per user action, an action of several
    /// commands (a paste, Enter over a selection) recorded as a group. It
    /// keeps the last 200 actions.
    executor: CommandExecutor,
    selection: Option<SelectionRange>,
    /// The composition in the text: its paragraph and byte range.
    composition: Option<(usize, usize, usize)>,
    renderer: CanvasRenderer<TinySkiaPainter>,
    pages: Rc<RefCell<Vec<TinySkiaPainter>>>,
    rendered_pages: Rc<Cell<usize>>,
    pass_covers_whole_document: Rc<Cell<bool>>,
    /// See [`TinySkiaRenderHost::last_paintable_page`].
    last_paintable_page: Rc<Cell<usize>>,
    /// See [`Self::set_dark_paper`].
    dark_paper: Rc<Cell<bool>>,
    /// The pages kept as pixels: see [`Self::set_resident_pages`].
    resident: Rc<RefCell<Range<usize>>>,
    /// Each page's box, for giving a page its surface back.
    page_sizes: Rc<RefCell<Vec<(f64, f64)>>>,
    fonts: Rc<FontBook>,
    measurer: Rc<ShapingMeasurer>,
    /// The page box the renderer was built to: see [`Self::follow_page_box`].
    built_for: (f64, f64),
    scale: f64,
    options: Options,
    dirty: bool,
    /// The x a run of Up and Down keeps to, so passing a short line does
    /// not pull the caret to its end for good.
    goal_x: Option<f64>,
    /// Which line a caret on a soft wrap is drawn on — the offset that ends
    /// one line starts the next — as the point it was placed at says.
    affinity: CaretAffinity,
    /// The first page repainted since the shell last asked.
    repainted_from: Option<usize>,
    /// The pages repainted since the shell last asked, from the renderer's
    /// dirty regions.
    repainted_pages: std::collections::BTreeSet<usize>,
    /// While edits are batched (see [`Self::paste`]): the first paragraph
    /// any of them touched, repainted from once they are all in.
    deferred_repaint: Option<usize>,
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
        Self::open_resident(bytes, scale, fonts, measurer, 0..usize::MAX)
    }

    /// [`Self::open`], keeping only the pages in `resident` (0-based) as
    /// pixels: what a shell showing a long document wants, which then moves
    /// them with [`Self::set_resident_pages`] as it scrolls.
    pub fn open_resident(
        bytes: &[u8],
        scale: f64,
        fonts: Rc<FontBook>,
        measurer: Rc<ShapingMeasurer>,
        resident: Range<usize>,
    ) -> Result<Self, String> {
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        // Formatting marks on, as in Word and jade. The bundled face lacks the
        // paragraph- and page-break arrows; the painter draws those itself.
        let mut options = Options::default();
        let word_document = futures::executor::block_on(WordDocument::load(bytes, &options))
            .map_err(|error| format!("cannot read the document: {error}"))?;
        if word_document.document.is_none() {
            return Err("the document has no main part".to_string());
        }
        crate::document::show_changes_as_saved(&mut options, &word_document);
        // The section properties decide the page box: a first pass finds it,
        // and the renderer kept is the one built to it. The first pass only
        // lays the document out — painting every page of it was most of what
        // opening a document that is not Letter cost, thrown away — and when
        // the box is Letter after all, its layout is painted as it stands.
        // The first pass keeps no page as pixels: it lays out, and draws on
        // nothing.
        let mut opened = Self::render(
            &word_document,
            &options,
            INITIAL_PAGE,
            scale,
            &fonts,
            &measurer,
            0..0,
        )?;
        let size = opened.renderer.layout_page_size();
        let mut built_for = INITIAL_PAGE;
        if size != INITIAL_PAGE && size.0 > 1.0 && size.1 > 1.0 {
            built_for = size;
            opened = Self::render(
                &word_document,
                &options,
                size,
                scale,
                &fonts,
                &measurer,
                resident,
            )?;
        } else {
            // Letter after all: its layout, painted on the resident pages.
            *opened.host.resident.borrow_mut() = resident;
            Self::paint_every_page(
                &mut opened.renderer,
                &word_document,
                &options,
                &opened.host.resident.borrow(),
            )?;
        }
        let Opened { renderer, host } = opened;
        Ok(Self {
            word_document,
            executor: CommandExecutor::default(),
            selection: None,
            composition: None,
            renderer,
            pages: host.pages,
            rendered_pages: host.rendered,
            pass_covers_whole_document: host.whole,
            last_paintable_page: host.limit,
            dark_paper: host.dark,
            resident: host.resident,
            page_sizes: host.sizes,
            fonts,
            measurer,
            built_for,
            scale,
            options,
            dirty: false,
            goal_x: None,
            affinity: CaretAffinity::Upstream,
            repainted_from: None,
            deferred_repaint: None,
            repainted_pages: std::collections::BTreeSet::new(),
        })
    }

    /// Lays `word_document` out on pages of `width` × `height` and paints
    /// the `resident` ones.
    fn render(
        word_document: &WordDocument,
        options: &Options,
        (width, height): (f64, f64),
        scale: f64,
        fonts: &Rc<FontBook>,
        measurer: &Rc<ShapingMeasurer>,
        resident: Range<usize>,
    ) -> Result<Opened, String> {
        let host = TinySkiaRenderHost::with_resident(
            width,
            height,
            scale,
            Rc::clone(fonts),
            Rc::clone(measurer),
            resident,
        )
        .ok_or_else(|| "cannot allocate a page surface".to_string())?;
        let handles = HostHandles {
            pages: Rc::clone(&host.pages),
            rendered: Rc::clone(&host.rendered_pages),
            whole: Rc::clone(&host.pass_covers_whole_document),
            limit: Rc::clone(&host.last_paintable_page),
            dark: Rc::clone(&host.dark_paper),
            resident: Rc::clone(&host.resident),
            sizes: Rc::clone(&host.page_sizes),
        };
        let mut renderer = CanvasRenderer::<TinySkiaPainter>::with_host(Box::new(host), false)
            .map_err(|_| "renderer setup failed".to_string())?;
        renderer.sync_word_document(word_document);
        // The shell draws the selection over the pages.
        renderer.set_use_selection_overlay(true);
        Self::paint_every_page(
            &mut renderer,
            word_document,
            options,
            &handles.resident.borrow(),
        )?;
        Ok(Opened {
            renderer,
            host: handles,
        })
    }

    /// A whole-document pass: every page laid out, the `resident` ones
    /// painted.
    fn paint_every_page(
        renderer: &mut CanvasRenderer<TinySkiaPainter>,
        word_document: &WordDocument,
        options: &Options,
        resident: &Range<usize>,
    ) -> Result<(), String> {
        renderer.mark_full_repaint();
        let document = word_document
            .document
            .as_ref()
            .ok_or_else(|| "the document has no main part".to_string())?
            .clone();
        renderer.set_visible_page_range(drawn_pages(resident, None));
        let rendered = renderer.render_with_numbering(
            &document,
            options,
            word_document.numbering_part.as_ref(),
            word_document.header_lines.as_deref(),
            word_document.footer_lines.as_deref(),
            Some(&word_document.comments),
        );
        renderer.clear_visible_page_range();
        rendered.map_err(|_| "render failed".to_string())?;
        if let Some(error) = renderer.take_render_error() {
            return Err(error);
        }
        renderer.clear_dirty_regions();
        Ok(())
    }

    /// Keeps the pages in `pages` (0-based) as pixels and lets go of the
    /// rest: a page leaving gives its surface back, one coming in is given a
    /// surface and painted — from the layout as it stands, so it costs the
    /// drawing of those pages alone. Returns whether any page was painted,
    /// for a shell to re-read ([`Self::take_repainted_pages`] says which).
    pub fn set_resident_pages(&mut self, pages: Range<usize>) -> Result<bool, String> {
        if *self.resident.borrow() == pages {
            return Ok(false);
        }
        *self.resident.borrow_mut() = pages.clone();
        let count = self.page_count();
        let mut arriving = Vec::new();
        {
            let mut surfaces = self.pages.borrow_mut();
            let sizes = self.page_sizes.borrow();
            for (index, page) in surfaces.iter_mut().enumerate() {
                let keep = pages.contains(&index) && index < count;
                if keep != is_placeholder(page) {
                    continue;
                }
                let (width, height) = sizes.get(index).copied().unwrap_or(INITIAL_PAGE);
                let replacement = if keep {
                    arriving.push(index);
                    surface(&self.fonts, &self.measurer, width, height, self.scale)
                } else {
                    placeholder(&self.fonts, &self.measurer, self.scale)
                }
                .ok_or("cannot allocate a page surface")?;
                replacement.set_dark_paper(self.dark_paper.get());
                *page = replacement;
            }
        }
        let (Some(&first), Some(&last)) = (arriving.first(), arriving.last()) else {
            return Ok(false);
        };
        // Those pages only, from the layout the last pass left.
        let document = self
            .word_document
            .document
            .as_ref()
            .ok_or_else(|| "document went missing".to_string())?
            .clone();
        self.renderer.mark_full_repaint();
        self.pass_covers_whole_document.set(true);
        self.renderer
            .set_visible_page_range(Some((first + 1)..=(last + 1)));
        let rendered = self.renderer.render_with_numbering(
            &document,
            &self.options,
            self.word_document.numbering_part.as_ref(),
            self.word_document.header_lines.as_deref(),
            self.word_document.footer_lines.as_deref(),
            Some(&self.word_document.comments),
        );
        self.renderer.clear_visible_page_range();
        self.renderer.clear_dirty_regions();
        rendered.map_err(|_| "repaint failed".to_string())?;
        self.repainted_pages.extend(arriving);
        Ok(true)
    }

    // ---- pages --------------------------------------------------------------

    /// The renderer's band-redraw counters: eligible, used, and why not.
    pub fn band_redraw_stats(&self) -> (u64, u64, Vec<(&'static str, u64)>) {
        self.renderer.band_redraw_stats()
    }

    pub fn page_count(&self) -> usize {
        self.rendered_pages.get().min(self.pages.borrow().len())
    }

    /// Page `index`'s surface, as last painted.
    pub fn page(&self, index: usize) -> Option<TinySkiaPainter> {
        if index >= self.page_count() {
            return None;
        }
        self.pages
            .borrow()
            .get(index)
            .filter(|page| !is_placeholder(page))
            .cloned()
    }

    /// The page box, in CSS px.
    pub fn page_size(&self) -> (f64, f64) {
        self.renderer.layout_page_size()
    }

    /// From the top of one page to the top of the next: the page box as
    /// laid out now, which a page turned or another paper changes.
    pub fn page_pitch(&self) -> f64 {
        self.page_size().1 + PAGE_GAP_PX
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

    /// [`Self::position_at`] in the body only: where a drag that started in
    /// the body goes, rather than into a header, a footer, a note or a text
    /// box it passes over.
    pub fn body_position_at(&self, x: f64, document_y: f64) -> Option<(usize, usize)> {
        self.renderer
            .hit_test_body_position(x, document_y, &self.word_document)
            .map(|position| (position.paragraph, position.offset))
    }

    /// Whether paragraph `index` is in the body.
    pub fn is_body_paragraph(&self, index: usize) -> bool {
        self.word_document.is_body_paragraph_surface(index)
    }

    /// Moves the caret, reporting where it is drawn.
    pub fn place_caret(&mut self, paragraph: usize, offset: usize) -> Option<CaretRect> {
        self.composition = None;
        self.goal_x = None;
        self.affinity = CaretAffinity::Upstream;
        self.renderer.set_caret_affinity(self.affinity);
        self.set_caret(paragraph, offset);
        self.caret_rect()
    }

    /// Moves the caret to the text under a point, drawn on the line the
    /// point is on: a wrapped line's first position is also the last one of
    /// the line above, so a click at a line's start and one past the end of
    /// the line above land on the same offset, drawn where each was.
    pub fn place_caret_at(&mut self, x: f64, document_y: f64) -> Option<CaretRect> {
        let (paragraph, offset) = self.position_at(x, document_y)?;
        self.place_caret(paragraph, offset);
        self.lean_towards(document_y);
        self.caret_rect()
    }

    /// Draws the caret on whichever of its possible lines is nearer `y`.
    fn lean_towards(&mut self, y: f64) {
        let mut nearest = (CaretAffinity::Upstream, f64::INFINITY);
        for affinity in [CaretAffinity::Upstream, CaretAffinity::Downstream] {
            self.renderer.set_caret_affinity(affinity);
            if let Some(rect) = self.caret_rect() {
                let distance = (rect.document_y + rect.height / 2.0 - y).abs();
                if distance < nearest.1 {
                    nearest = (affinity, distance);
                }
            }
        }
        self.affinity = nearest.0;
        self.renderer.set_caret_affinity(self.affinity);
    }

    /// Selects from `anchor` to `focus`. Nothing is repainted: the shell
    /// draws [`Self::selection_rects`] over the pages.
    pub fn select(&mut self, anchor: (usize, usize), focus: (usize, usize)) -> Result<(), String> {
        if anchor == focus {
            self.place_caret(focus.0, focus.1);
            return Ok(());
        }
        self.composition = None;
        self.goal_x = None;
        let range = SelectionRange::new(
            TextPosition::new(anchor.0, anchor.1),
            TextPosition::new(focus.0, focus.1),
        );
        self.selection = Some(range);
        self.renderer.set_selection_range(Some(range));
        Ok(())
    }

    /// The run of letters and digits around `position` — Japanese runs as
    /// a whole, having no spaces to break on — selected, as a double click
    /// selects in Word. Elsewhere, the caret goes to `position`.
    pub fn select_word(&mut self, (paragraph, offset): (usize, usize)) -> Result<(), String> {
        let text = self.paragraph_text(paragraph).unwrap_or_default();
        // A run of one script in Japanese, a word and the spaces after it in
        // English, as Word's double-click takes them (the native app's too).
        let (start, end) = jade_office_model::word::selection::word_at(&text, offset);
        self.select((paragraph, start), (paragraph, end))
    }

    /// Tab and ⇧Tab in a table, as in Word: to the next or the previous
    /// cell, its text selected so typing replaces it; Tab in the last cell
    /// adds a row below and goes into it. `Ok(false)` when the caret is not
    /// in a table, where Tab types a tab.
    pub fn step_cell(&mut self, forwards: bool) -> Result<bool, String> {
        use formatting::{CellStep, WordFormatting};
        match WordFormatting::step_cell(self, forwards)? {
            CellStep::NotInTable => Ok(false),
            CellStep::Stay => Ok(true),
            CellStep::Select(from, to) => {
                self.select(from, to)?;
                Ok(true)
            }
        }
    }

    /// The whole of `paragraph` selected, as a triple click selects in Word.
    pub fn select_paragraph(&mut self, paragraph: usize) -> Result<(), String> {
        let end = self.paragraph_text(paragraph).map_or(0, |text| text.len());
        self.select((paragraph, 0), (paragraph, end))
    }

    /// The selection's highlight, page by page, as the last paint laid the
    /// text out; empty for a caret.
    pub fn selection_rects(&self) -> Vec<HighlightRect> {
        if self.selection().is_none() {
            return Vec::new();
        }
        let pitch = self.page_pitch();
        self.renderer
            .get_selection_rects()
            .into_iter()
            .map(|rect| {
                // Text rectangles are in page px; whole-cell ones in
                // document px, gaps included. A page-px top is never as far
                // down as the top of its page in document px.
                let page_top = rect.page as f64 * pitch;
                let y = if rect.page > 0 && rect.y >= page_top {
                    rect.y - page_top
                } else {
                    rect.y
                };
                HighlightRect {
                    page: rect.page,
                    x: rect.x,
                    y,
                    width: rect.width,
                    height: rect.height,
                }
            })
            .collect()
    }

    /// The first page repainted since the last call, for a shell that keeps
    /// images of the pages: the ones before it are as they were.
    pub fn take_repainted_from(&mut self) -> Option<usize> {
        self.repainted_from.take()
    }

    /// The pages repainted since the last call, in order: what a shell that
    /// keeps images of the pages has to re-read. A keystroke usually repaints
    /// one; re-reading every page after it was most of what a keystroke cost
    /// the browser build.
    pub fn take_repainted_pages(&mut self) -> Vec<usize> {
        self.repainted_from = None;
        std::mem::take(&mut self.repainted_pages)
            .into_iter()
            .filter(|&page| page < self.page_count())
            .collect()
    }

    /// Notes the pages the pass just painted and consumes its dirty regions,
    /// as the browser editor does after each pass. Left in place, the full
    /// repaint the document opened with stayed pending forever, and every
    /// keystroke repainted every page from nothing.
    ///
    /// Pages past `last_page` (0-based) were not painted, whatever the
    /// regions say: see [`TinySkiaRenderHost::last_paintable_page`].
    fn consume_dirty_regions(&mut self, last_page: usize) {
        if self.renderer.needs_full_repaint() {
            self.repainted_pages
                .extend(0..self.page_count().min(last_page.saturating_add(1)));
        } else {
            self.repainted_pages.extend(
                self.renderer
                    .dirty_regions()
                    .iter()
                    .map(|region| region.page)
                    .filter(|&page| page <= last_page),
            );
        }
        self.renderer.clear_dirty_regions();
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
            Motion::WordLeft | Motion::WordRight => {
                self.step_word(focus, motion == Motion::WordRight)
            }
            Motion::DocumentStart => {
                let first = (0..self.paragraph_count())
                    .find(|&index| self.is_body_paragraph(index))
                    .unwrap_or(0);
                (first, 0)
            }
            Motion::DocumentEnd => {
                let last = (0..self.paragraph_count())
                    .rev()
                    .find(|&index| self.is_body_paragraph(index))
                    .unwrap_or(0);
                (last, self.paragraph_text(last).map_or(0, |text| text.len()))
            }
            Motion::Up | Motion::Down | Motion::LineStart | Motion::LineEnd => {
                // The line the caret is drawn on, wrap or not.
                let rect = self.rect_at(focus).ok_or("the caret has no position")?;
                let line = rect.document_y + rect.height / 2.0;
                let (x, y) = match motion {
                    Motion::Up => (goal_x.unwrap_or(rect.x), line - rect.height),
                    Motion::Down => (goal_x.unwrap_or(rect.x), line + rect.height),
                    Motion::LineStart => (f64::MIN / 4.0, line),
                    _ => (f64::MAX / 4.0, line),
                };
                let (target, y) = match motion {
                    // Along the caret's own line: in a table row the far side
                    // of the page is another cell's text.
                    Motion::LineStart | Motion::LineEnd => (
                        self.renderer
                            .selection_manager()
                            .hit_test_matching(x, y, |paragraph| paragraph == focus.0)
                            .map(|position| (position.paragraph, position.offset)),
                        y,
                    ),
                    _ => self
                        .line_beyond(focus, x, line, rect.height, motion == Motion::Down)
                        .map_or((None, y), |(target, y)| (Some(target), y)),
                };
                let target = target.unwrap_or(focus);
                if extend {
                    self.select(anchor, target)?;
                } else {
                    self.place_caret(target.0, target.1);
                    self.lean_towards(y);
                }
                self.goal_x = Some(x);
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

    /// The position at `x` on the line above or below the one through
    /// `line` — a caret at `from`, `height` tall — and the height it was
    /// found at; `None` at the top or the bottom of the document.
    ///
    /// A probe just past the caret's line can miss the next one: below a
    /// page's last line are its bottom margin, the gap and the next page's
    /// top margin, and a hit-test there still lands on the line it is
    /// nearest. So the probe walks on until it finds a line other than the
    /// caret's. Body text stays in the body, as a drag does, rather than
    /// stepping into a footer or a header on the way.
    fn line_beyond(
        &self,
        from: (usize, usize),
        x: f64,
        line: f64,
        height: f64,
        down: bool,
    ) -> Option<((usize, usize), f64)> {
        let body = self.is_body_paragraph(from.0);
        let hit = |y: f64| {
            if body {
                self.body_position_at(x, y)
            } else {
                self.position_at(x, y)
            }
        };
        // What the caret's own line has at `x`: the same answer further on
        // is still that line.
        let here = hit(line);
        let direction = if down { 1.0 } else { -1.0 };
        let step = (height / 2.0).clamp(2.0, 8.0);
        let end = self.page_count() as f64 * self.page_pitch();
        let mut y = line + height * direction;
        while (0.0..end).contains(&y) {
            if let Some(position) = hit(y).filter(|&position| Some(position) != here) {
                return Some((position, y));
            }
            y += step * direction;
        }
        None
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

    /// One word along from `position`, as ⌥← and ⌥→ go: past any spaces and
    /// punctuation, then over a run of letters and digits (Japanese runs as
    /// a whole). At a paragraph's edge, into the next paragraph.
    fn step_word(&self, (paragraph, offset): (usize, usize), forwards: bool) -> (usize, usize) {
        use jade_office_model::text::{word_end_after, word_start_before};
        let text = self.paragraph_text(paragraph).unwrap_or_default();
        let offset = offset.min(text.len());
        let at_edge = if forwards {
            offset == text.len()
        } else {
            offset == 0
        };
        if at_edge {
            return self.step_character((paragraph, offset), forwards);
        }
        if forwards {
            (paragraph, word_end_after(&text, offset))
        } else {
            (paragraph, word_start_before(&text, offset))
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

    /// The page `paragraph` starts on, as last painted.
    fn paragraph_page(&mut self, paragraph: usize) -> usize {
        self.paragraph_start_page(paragraph).unwrap_or(0)
    }

    /// The page (0-based) `paragraph` starts on: from its lines as last
    /// laid out, or — for a paragraph before the resident pages, whose lines
    /// a pass that does not draw them leaves out — from the layout cache.
    pub fn paragraph_start_page(&mut self, paragraph: usize) -> Option<usize> {
        match self.rect_at((paragraph, 0)) {
            Some(rect) => Some(self.page_and_offset(rect.document_y).0),
            None => self
                .renderer
                .cached_paragraph_start_page(paragraph)
                .map(|page| page.saturating_sub(1)),
        }
    }

    fn set_caret(&mut self, paragraph: usize, offset: usize) {
        let range = SelectionRange::collapsed(TextPosition::new(paragraph, offset));
        self.selection = Some(range);
        self.renderer.set_selection_range(Some(range));
    }

    /// Where the selection's focus is drawn as a caret would be: the
    /// renderer reports a caret only while nothing is selected.
    pub fn focus_rect(&mut self) -> Option<CaretRect> {
        let focus = self.caret()?;
        self.rect_at(focus)
    }

    /// Where the caret is drawn now; `None` while a range is selected (see
    /// [`Self::focus_rect`]).
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
            self.executor.begin_group();
            let result = self
                .replace_selection("")
                .and_then(|()| self.split_at_caret());
            self.executor.end_group();
            return result;
        }
        self.split_at_caret()
    }

    /// Shift+Enter: a line break (`w:br`) at the caret, the line ending
    /// inside the paragraph — after removing the selection, as one undoable
    /// step. The break is one character of the paragraph's text, `\n`.
    pub fn insert_line_break(&mut self) -> Result<(), String> {
        self.goal_x = None;
        if self.selection().is_some() {
            self.executor.begin_group();
            let result = self
                .replace_selection("")
                .and_then(|()| self.insert_text("\n"));
            self.executor.end_group();
            return result;
        }
        self.insert_text("\n")
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
        let mut lines = text.split('\n');
        // Every line is a split and an insert, and each of those repainted
        // the document from its paragraph down: pasting n lines cost n
        // repaints of the rest of the document. They are all made first and
        // the document is repainted from the first paragraph they touched,
        // once.
        self.deferred_repaint = Some(usize::MAX);
        // One undo step, however many lines: counted per command, a paste of
        // more than a hundred lines outran the history's 200 entries and
        // could only be undone in part.
        self.executor.begin_group();
        let result = (|| {
            self.insert_text(lines.next().unwrap_or_default())?;
            for line in lines {
                self.split_at_caret()?;
                self.insert_text(line)?;
            }
            Ok(())
        })();
        self.executor.end_group();
        let from = self
            .deferred_repaint
            .take()
            .filter(|&from| from != usize::MAX);
        let repainted = match from {
            Some(from) => self.repaint_from(from),
            None => Ok(()),
        };
        result.and(repainted)
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
        self.executor.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.executor.can_redo()
    }

    pub fn undo(&mut self) -> Result<bool, String> {
        self.step_history(true)
    }

    pub fn redo(&mut self) -> Result<bool, String> {
        self.step_history(false)
    }

    fn step_history(&mut self, backwards: bool) -> Result<bool, String> {
        self.composition = None;
        let mut selection = self.selection.take();
        let stepped = self.edit_in_place(|this| {
            let mut context = CommandContext::new(&mut this.word_document, &mut selection);
            if backwards {
                this.executor.undo(&mut context)
            } else {
                this.executor.redo(&mut context)
            }
            .map_err(|error| format!("history failed: {error:?}"))
        });
        self.selection = selection;
        if !stepped? {
            return Ok(false);
        }
        self.renderer.set_selection_range(self.selection);
        self.dirty = true;
        self.repaint_all()?;
        Ok(true)
    }

    /// Runs `edit` on the document with the renderer's hold on it let go,
    /// then points the renderer at the result.
    ///
    /// The renderer keeps the document it last laid out, so changing a
    /// paragraph through the shared `Rc` copied the whole document first —
    /// every paragraph, on every keystroke, a tenth of what typing cost. Let
    /// go of, the document is changed where it is. Letting go also forgets
    /// the paragraph hashes the renderer keys by address, which an edit in
    /// place would otherwise leave stale; the copy forgot them too.
    fn edit_in_place<R>(&mut self, edit: impl FnOnce(&mut Self) -> R) -> R {
        self.renderer.drop_document_cache();
        let result = edit(self);
        if let Some(document) = &self.word_document.document {
            self.renderer.restore_document_cache(Rc::clone(document));
        }
        result
    }

    /// Applies `command` — recorded for undo when `recorded` — and repaints
    /// from `paragraph` down.
    fn apply(
        &mut self,
        paragraph: usize,
        command: HeadlessCommand,
        recorded: bool,
    ) -> Result<(), String> {
        let paragraphs = self.paragraph_count();
        let request = HeadlessEditRequest {
            commands: vec![command],
        };
        self.edit_in_place(|this| {
            if recorded {
                this.selection = apply_headless_commands_with_executor_to_document(
                    &mut this.word_document,
                    &mut this.executor,
                    this.selection.take(),
                    &request,
                )
                .map_err(|error| format!("edit failed: {error:?}"))?;
            } else {
                apply_headless_commands_to_document(&mut this.word_document, &request)
                    .map_err(|error| format!("edit failed: {error:?}"))?;
            }
            Ok::<(), String>(())
        })?;
        self.dirty = true;
        if let Some(from) = &mut self.deferred_repaint {
            *from = (*from).min(paragraph);
            return Ok(());
        }
        // Most edits leave their paragraph's place and height alone, and then
        // nothing after it moves: the page it is on is all that changes, and
        // of that page only the paragraph's band — every other pixel stays on
        // the surfaces, which this host keeps between passes. So the first
        // pass stops at the paragraph's page; if the paragraph came out of it
        // where and as tall as it went in, that is all. Otherwise the rest of
        // the document is repainted after it.
        let geometry = |renderer: &CanvasRenderer<TinySkiaPainter>| {
            (
                renderer.cached_paragraph_start_page(paragraph),
                renderer.cached_paragraph_height(paragraph),
                renderer.paragraph_layout_origin(paragraph),
            )
        };
        let before = geometry(&self.renderer);
        let pages = self.page_count();
        let page = self.paragraph_page(paragraph) + 1;
        self.renderer.set_band_redraw_enabled(true);
        let first = self.repaint_through(paragraph, Some(page - 1), page);
        self.renderer.set_band_redraw_enabled(false);
        first?;
        let held_still = before.1.is_some() && geometry(&self.renderer) == before;
        // A split or a join leaves its paragraph where it was but moves every
        // paragraph after it, so a changed count is never "held still".
        if held_still && self.page_count() == pages && self.paragraph_count() == paragraphs {
            return Ok(());
        }
        // The rest, from the paragraph after it: the first pass cached the
        // paragraph as it now is, so a pass from it would find it unmoved and
        // stop at its page, leaving the pages below as they were.
        if paragraph + 1 < self.paragraph_count() {
            self.repaint_from(paragraph + 1)
        } else {
            self.repaint_from(paragraph)
        }
    }

    /// The renderer's incremental sequence, through every page: an edit can
    /// move content onto any page after it, and add pages — as many as it
    /// takes, so the bound is the document's new end, not one page past its
    /// old one (a paste of many lines, or one long line, adds several).
    fn repaint_from(&mut self, paragraph: usize) -> Result<(), String> {
        self.repaint_through(paragraph, None, usize::MAX)
    }

    /// The incremental sequence from `paragraph` through page `last_page`
    /// (1-based); only a pass through every page may change the page count.
    ///
    /// With `draw_from` (a 0-based page), the pages before it are laid out
    /// but not drawn, and keep their pixels. The renderer otherwise repaints
    /// them whole on every pass, so a keystroke cost more the further down
    /// the document it was typed; a caller passes it when nothing before
    /// that page can have changed.
    fn repaint_through(
        &mut self,
        paragraph: usize,
        draw_from: Option<usize>,
        last_page: usize,
    ) -> Result<(), String> {
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
        // Pages past the bound keep their pixels: see `last_paintable_page`.
        self.last_paintable_page.set(last_page.saturating_sub(1));
        let drawn = drawn_pages(&self.resident.borrow(), draw_from);
        self.renderer.set_visible_page_range(drawn);
        let result = self.renderer.render_bounded_to_page_with_parts(
            document,
            &self.options,
            self.word_document.numbering_part.as_ref(),
            self.word_document.header_lines.as_deref(),
            self.word_document.footer_lines.as_deref(),
            last_page,
        );
        self.last_paintable_page.set(usize::MAX);
        self.renderer.clear_visible_page_range();
        self.consume_dirty_regions(last_page.saturating_sub(1));
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

    /// Hides the top-level body elements under collapsed headings (Word's
    /// 見出しの折りたたみ) and repaints; empty shows them all again.
    pub fn set_collapsed_blocks(&mut self, blocks: Vec<usize>) -> Result<(), String> {
        self.options.set_collapsed_blocks(blocks);
        self.repaint_all()
    }

    /// Shows or hides Word's formatting marks (表示 → 編集記号) and
    /// repaints.
    pub fn set_formatting_marks(&mut self, show: bool) -> Result<(), String> {
        if self.options.show_formatting_marks == show {
            return Ok(());
        }
        self.options.show_formatting_marks = show;
        self.repaint_all()
    }

    /// Whether the formatting marks show.
    pub fn formatting_marks(&self) -> bool {
        self.options.show_formatting_marks
    }

    /// The top-level body elements collapsed headings hide.
    pub fn collapsed_blocks(&self) -> &[usize] {
        self.options.collapsed_blocks()
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
        self.renderer.sync_word_document(&self.word_document);
        self.renderer.invalidate_paragraph_content_hash_cache();
        self.renderer.mark_full_repaint();
        self.pass_covers_whole_document.set(true);
        let drawn = drawn_pages(&self.resident.borrow(), None);
        self.renderer.set_visible_page_range(drawn);
        let rendered = self.renderer.render_with_numbering(
            &document,
            &self.options,
            self.word_document.numbering_part.as_ref(),
            self.word_document.header_lines.as_deref(),
            self.word_document.footer_lines.as_deref(),
            Some(&self.word_document.comments),
        );
        self.renderer.clear_visible_page_range();
        rendered.map_err(|_| "repaint failed".to_string())?;
        self.consume_dirty_regions(usize::MAX);
        if let Some(error) = self.renderer.take_render_error() {
            return Err(error);
        }
        self.follow_page_box()
    }

    /// When an edit has changed the page box — the page turned, another
    /// paper — the renderer is built again to it, as opening the document
    /// builds it. One built to another box went on laying the text out to
    /// that box's width: turned to landscape, the lines kept their portrait
    /// width and ran over each other on the wider pages.
    fn follow_page_box(&mut self) -> Result<(), String> {
        let size = self.renderer.layout_page_size();
        if size == self.built_for || size.0 <= 1.0 || size.1 <= 1.0 {
            return Ok(());
        }
        let resident = self.resident.borrow().clone();
        let Opened { renderer, host } = Self::render(
            &self.word_document,
            &self.options,
            size,
            self.scale,
            &self.fonts,
            &self.measurer,
            resident,
        )?;
        let dark = self.dark_paper.get();
        self.renderer = renderer;
        self.pages = host.pages;
        self.rendered_pages = host.rendered;
        self.pass_covers_whole_document = host.whole;
        self.last_paintable_page = host.limit;
        self.dark_paper = host.dark;
        self.resident = host.resident;
        self.page_sizes = host.sizes;
        self.built_for = size;
        self.renderer.set_caret_affinity(self.affinity);
        self.renderer.set_selection_range(self.selection);
        self.repainted_from = Some(0);
        self.repainted_pages.extend(0..self.page_count());
        // Painted on the document's own paper: back on the paper on show.
        if dark {
            self.set_dark_paper(true)?;
        }
        Ok(())
    }

    // ---- pictures and shapes ------------------------------------------------

    /// The picture or shape under a point in document coordinates, as the
    /// last paint placed it: a press there takes hold of it rather than
    /// putting the caret behind it.
    pub fn object_at(&self, x: f64, document_y: f64) -> Option<PlacedObject> {
        self.renderer.hit_test_object(x, document_y).map(placed)
    }

    /// Where a picture or shape is on its page, as the last paint placed it.
    pub fn object_region(&self, target: FloatingObjectTarget) -> Option<PlacedObject> {
        self.renderer.object_region(target).map(placed)
    }

    // ---- saving -------------------------------------------------------------

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// The document as a PDF of its pages as the editor paints them (see
    /// [`crate::pdf`]), on its own paper, at the view's scale: each page
    /// painted in turn, then the pages kept before put back.
    pub fn export_pdf(&mut self) -> Result<Vec<u8>, String> {
        let resident = self.resident.borrow().clone();
        let dark = self.dark_paper.get();
        // The document's own paper, whatever the view shows.
        self.set_dark_paper(false)?;
        let mut pages = Vec::with_capacity(self.page_count());
        let mut result = Ok(());
        for index in 0..self.page_count() {
            if let Err(error) = self.set_resident_pages(index..index + 1) {
                result = Err(error);
                break;
            }
            let Some(page) = self.page(index) else {
                result = Err(format!("page {} did not paint", index + 1));
                break;
            };
            let (width, height) = self
                .page_sizes
                .borrow()
                .get(index)
                .copied()
                .unwrap_or_else(|| self.page_size());
            pages.push(crate::pdf::PdfPage {
                // CSS px to points.
                width_pt: width * 0.75,
                height_pt: height * 0.75,
                pixel_width: page.width(),
                pixel_height: page.height(),
                rgba: page.rgba_premultiplied(),
            });
        }
        self.set_resident_pages(resident)?;
        self.set_dark_paper(dark)?;
        result?;
        crate::pdf::pdf_of_pages(&pages)
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

/// A picture or shape as the renderer placed it.
fn placed(info: jade_docx_editor::canvas_renderer::ObjectDebugInfo) -> PlacedObject {
    PlacedObject {
        target: FloatingObjectTarget {
            paragraph_index: info.paragraph_index,
            run_index: info.run_index,
            kind: info.kind,
        },
        page: info.page.saturating_sub(1),
        x: info.x,
        y: info.y,
        width: info.width,
        height: info.height,
    }
}

/// A renderer just built, and the handles on its host's state.
struct Opened {
    renderer: CanvasRenderer<TinySkiaPainter>,
    host: HostHandles,
}

/// The host's state the document reads and sets after the renderer has
/// taken the host.
struct HostHandles {
    pages: Rc<RefCell<Vec<TinySkiaPainter>>>,
    rendered: Rc<Cell<usize>>,
    whole: Rc<Cell<bool>>,
    limit: Rc<Cell<usize>>,
    dark: Rc<Cell<bool>>,
    resident: Rc<RefCell<Range<usize>>>,
    sizes: Rc<RefCell<Vec<(f64, f64)>>>,
}

/// The pages a pass may draw on, 1-based as the renderer counts them: the
/// resident ones, from `draw_from` (0-based) on. `None` for every page.
fn drawn_pages(resident: &Range<usize>, draw_from: Option<usize>) -> Option<RangeInclusive<usize>> {
    let start = resident.start.max(draw_from.unwrap_or(0));
    if start == 0 && resident.end == usize::MAX {
        return None;
    }
    if start >= resident.end {
        // Nothing to draw: a page past any document's end.
        return Some(usize::MAX..=usize::MAX);
    }
    Some((start + 1)..=resident.end)
}

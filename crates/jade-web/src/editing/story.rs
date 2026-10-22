//! Headers and footers edited where they are, as Word edits them: the
//! browser's copy of the native app's `word_story`.
//!
//! The story is opened as a document of its own (the engine's
//! `story_docx`), laid out on a page as wide as its section's, and drawn
//! over the page it was opened on, where its header or footer is; the rest
//! of that page is washed out behind it. [`WordDoc`] is what the shell
//! edits: the story while one is open and the main document otherwise —
//! every editing command reaches the open one through `Deref` and
//! [`WordSession`] — while the pages, and where a click lands or the caret
//! is drawn on them, stay the main document's. Closing writes the story back
//! into its part as one undo step of the main document.

use std::ops::{Deref, DerefMut};

use jade_docx_editor::commands::snapshot_edit::SnapshotEditCommand;
use jade_docx_editor::commands::Command;
use jade_docx_editor::word_document::{FloatingObjectTarget, WordDocument};
use jade_docx_headless_api::{
    HeadlessCommand, HeadlessFieldKind, HeadlessSectionLayoutPatch, HeadlessSectionScope,
};
use jade_office_model::word::page::Story;
use jade_office_model::word::picture::PlacedObject;
use jade_office_model::word::{CellLayout, Position, WordSession};
use jade_selection::SelectionRange;
use jade_tiny_skia_painter::TinySkiaPainter;

use super::{CaretRect, EditableDocument, HighlightRect};

/// Points as CSS pixels.
fn css(points: f32) -> f64 {
    f64::from(points) * 96.0 / 72.0
}

fn role_name(role: Story) -> &'static str {
    match role {
        Story::Header => "header",
        Story::Footer => "footer",
    }
}

/// A header or footer open for editing over page `page` of the main
/// document.
pub struct StoryEdit {
    pub document: EditableDocument,
    /// The main document's page (0-based) it is drawn on.
    pub page: usize,
    pub section: usize,
    pub role: Story,
    /// `default`, `first` or `even`.
    pub variant: &'static str,
    /// Whether the section shows the previous section's story.
    pub linked: bool,
    /// The body's top and bottom edges on the page, in CSS px: the band the
    /// story is drawn in reaches at least to them.
    body_top: f64,
    body_bottom: f64,
    /// Where the story's content starts on its own page.
    content_top: f64,
    /// The main document's caret, put back when the story closes.
    main_caret: Option<(usize, usize)>,
}

impl StoryEdit {
    /// The bottom of the story's content on its own page.
    fn content_bottom(&self) -> f64 {
        self.document
            .renderer
            .selection_manager()
            .paragraphs()
            .iter()
            .flat_map(|paragraph| paragraph.lines.iter())
            .map(|line| line.absolute_top + line.height)
            .fold(self.content_top, f64::max)
            .max(self.content_top + 14.0)
    }

    /// Story y + offset = page y. A header starts where it does on the
    /// page; a footer ends at its distance from the bottom edge.
    pub fn offset(&self, page_height: f64) -> f64 {
        match self.role {
            Story::Header => 0.0,
            Story::Footer => page_height - self.content_top - self.content_bottom(),
        }
    }

    /// The rows of the page the story is drawn over, in CSS px.
    pub fn band(&self, page_height: f64) -> (f64, f64) {
        let offset = self.offset(page_height);
        match self.role {
            Story::Header => (0.0, self.body_top.max(self.content_bottom() + offset + 6.0)),
            Story::Footer => (
                self.body_bottom
                    .min(self.content_top + offset - 6.0)
                    .max(0.0),
                page_height,
            ),
        }
    }
}

/// A change to the open story's section that the ヘッダーとフッター tab
/// makes: it changes the main document, so the story is written back
/// first and opened again over the same page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StoryChange {
    /// 前と同じヘッダー/フッター.
    LinkToPrevious(bool),
    /// 先頭ページのみ別指定.
    DifferentFirst(bool),
    /// 奇数/偶数ページ別指定.
    DifferentOddEven(bool),
    /// ヘッダーの位置 / フッターの位置, a millimetre further from the edge or
    /// nearer.
    Distance(bool),
}

/// The document the shell edits: a story while one is open, the main
/// document otherwise.
pub struct WordDoc {
    main: EditableDocument,
    story: Option<Box<StoryEdit>>,
}

impl Deref for WordDoc {
    type Target = EditableDocument;

    fn deref(&self) -> &EditableDocument {
        match &self.story {
            Some(story) => &story.document,
            None => &self.main,
        }
    }
}

impl DerefMut for WordDoc {
    fn deref_mut(&mut self) -> &mut EditableDocument {
        match &mut self.story {
            Some(story) => &mut story.document,
            None => &mut self.main,
        }
    }
}

impl AsRef<EditableDocument> for WordDoc {
    fn as_ref(&self) -> &EditableDocument {
        self
    }
}

impl AsMut<EditableDocument> for WordDoc {
    fn as_mut(&mut self) -> &mut EditableDocument {
        self
    }
}

impl From<EditableDocument> for WordDoc {
    fn from(main: EditableDocument) -> Self {
        Self::new(main)
    }
}

/// The shared ribbon commands act on whichever document is open: the story
/// while one is, so formatting, tables, pictures and fields go into it.
impl WordSession for WordDoc {
    fn word_document(&self) -> &WordDocument {
        WordSession::word_document(&**self)
    }

    fn selection_range(&self) -> Option<SelectionRange> {
        WordSession::selection_range(&**self)
    }

    fn set_selection_range(&mut self, range: Option<SelectionRange>) {
        WordSession::set_selection_range(&mut **self, range)
    }

    fn apply_command(
        &mut self,
        command: HeadlessCommand,
        keep_selection: bool,
    ) -> Result<(), String> {
        WordSession::apply_command(&mut **self, command, keep_selection)
    }

    fn apply_commands(
        &mut self,
        commands: Vec<HeadlessCommand>,
        keep_selection: bool,
    ) -> Result<(), String> {
        WordSession::apply_commands(&mut **self, commands, keep_selection)
    }

    fn execute_command(
        &mut self,
        command: Box<dyn Command>,
        keep_selection: bool,
    ) -> Result<(), String> {
        WordSession::execute_command(&mut **self, command, keep_selection)
    }

    fn cell_id_for_paragraph(&self, paragraph: usize) -> Option<usize> {
        WordSession::cell_id_for_paragraph(&**self, paragraph)
    }

    fn cell_layout(&self, cell_id: usize) -> Option<&CellLayout> {
        WordSession::cell_layout(&**self, cell_id)
    }

    fn cell_layouts(&self) -> &[CellLayout] {
        WordSession::cell_layouts(&**self)
    }

    fn table_order_for(&self, table_id: usize) -> Option<usize> {
        WordSession::table_order_for(&**self, table_id)
    }

    fn hyperlink_regions(&self) -> Vec<jade_docx_editor::canvas_renderer::HyperlinkRegion> {
        WordSession::hyperlink_regions(&**self)
    }

    fn text_regions(
        &self,
        start: Position,
        end: Position,
    ) -> Vec<jade_docx_editor::canvas_renderer::TextRegion> {
        WordSession::text_regions(&**self, start, end)
    }

    fn select_cells(&mut self, anchor_cell: usize, focus_cell: usize) -> Result<(), String> {
        WordSession::select_cells(&mut **self, anchor_cell, focus_cell)
    }

    fn page_count(&self) -> Option<usize> {
        WordSession::page_count(&**self)
    }

    fn paragraph_page(&self, paragraph: usize) -> Option<usize> {
        match &self.story {
            // A story's text is on the page it is open over.
            Some(story) => Some(story.page + 1),
            None => WordSession::paragraph_page(&self.main, paragraph),
        }
    }

    fn position_metrics(&self, at: Position) -> Option<(f64, f64, f64)> {
        WordSession::position_metrics(&**self, at)
    }

    fn math_zones(&self) -> Vec<jade_docx_editor::canvas_renderer::MathZone> {
        WordSession::math_zones(&**self)
    }
}

impl WordDoc {
    pub fn new(main: EditableDocument) -> Self {
        Self { main, story: None }
    }

    /// The main document, whichever is being edited.
    pub fn main(&self) -> &EditableDocument {
        &self.main
    }

    pub fn main_mut(&mut self) -> &mut EditableDocument {
        &mut self.main
    }

    /// The header or footer open, if one is.
    pub fn story(&self) -> Option<&StoryEdit> {
        self.story.as_deref()
    }

    fn page_height(&self) -> f64 {
        self.main.page_size().1
    }

    // ---- the pages: always the main document's ------------------------------

    pub fn page_count(&self) -> usize {
        self.main.page_count()
    }

    pub fn page_size(&self) -> (f64, f64) {
        self.main.page_size()
    }

    pub fn page_pitch(&self) -> f64 {
        self.main.page_pitch()
    }

    pub fn page_and_offset(&self, document_y: f64) -> (usize, f64) {
        self.main.page_and_offset(document_y)
    }

    pub fn document_y(&self, page: usize, y_on_page: f64) -> f64 {
        self.main.document_y(page, y_on_page)
    }

    pub fn set_resident_pages(&mut self, pages: std::ops::Range<usize>) -> Result<bool, String> {
        self.main.set_resident_pages(pages)
    }

    /// The first page repainted since the last call: the story's page when
    /// the story repainted.
    pub fn take_repainted_from(&mut self) -> Option<usize> {
        let main = self.main.take_repainted_from();
        let story = self
            .story
            .as_mut()
            .and_then(|story| story.document.take_repainted_from().map(|_| story.page));
        match (main, story) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// The pages repainted since the last call, the story's among them when
    /// it repainted.
    pub fn take_repainted_pages(&mut self) -> Vec<usize> {
        let mut pages = self.main.take_repainted_pages();
        if let Some(story) = self.story.as_mut() {
            if !story.document.take_repainted_pages().is_empty() && !pages.contains(&story.page) {
                pages.push(story.page);
                pages.sort_unstable();
            }
        }
        pages
    }

    /// Page `index` as the view shows it: the story's page over its band,
    /// the rest of that page washed out, a dashed line where the story
    /// ends.
    pub fn page(&self, index: usize) -> Option<TinySkiaPainter> {
        let page = self.main.page(index)?;
        let Some(story) = self.story.as_deref().filter(|story| story.page == index) else {
            return Some(page);
        };
        let Some(story_page) = story.document.page(0) else {
            return Some(page);
        };
        let (width, height) = (page.width() as usize, page.height() as usize);
        let mut pixels = page.rgba_premultiplied();
        let story_pixels = story_page.rgba_premultiplied();
        let story_width = story_page.width() as usize;
        let story_height = story_page.height() as i64;
        let page_height = self.page_height();
        let scale = height as f64 / page_height.max(1.0);
        let (top, bottom) = story.band(page_height);
        let (top, bottom) = (
            (top * scale).floor().max(0.0) as usize,
            ((bottom * scale).ceil().max(0.0) as usize).min(height),
        );
        let offset = (story.offset(page_height) * scale).round() as i64;
        let stride = width * 4;
        let story_stride = story_width * 4;
        let paper = [pixels[0], pixels[1], pixels[2], pixels[3]];
        for (y, row) in pixels.chunks_exact_mut(stride).enumerate() {
            if y >= top && y < bottom {
                let source = y as i64 - offset;
                if source >= 0 && source < story_height && story_width >= width {
                    let start = source as usize * story_stride;
                    row.copy_from_slice(&story_pixels[start..start + stride]);
                } else {
                    for pixel in row.chunks_exact_mut(4) {
                        pixel.copy_from_slice(&paper);
                    }
                }
            } else {
                // The body behind the story, washed out as Word shows it.
                for pixel in row.chunks_exact_mut(4) {
                    for channel in 0..3 {
                        pixel[channel] =
                            ((u16::from(pixel[channel]) + u16::from(paper[channel])) / 2) as u8;
                    }
                }
            }
        }
        // The dashed line at the story's edge, in Word's blue (RGBA).
        let edge = match story.role {
            Story::Header => bottom.min(height.saturating_sub(1)),
            Story::Footer => top,
        };
        let thickness = (scale.round() as usize).max(1);
        for y in edge..(edge + thickness).min(height) {
            let row = &mut pixels[y * stride..(y + 1) * stride];
            for (x, pixel) in row.chunks_exact_mut(4).enumerate() {
                if (x / (6 * thickness)).is_multiple_of(2) {
                    pixel.copy_from_slice(&[0x0F, 0x6C, 0xD6, 0xFF]);
                }
            }
        }
        TinySkiaPainter::from_rgba_premultiplied(
            width as u32,
            height as u32,
            &pixels,
            std::rc::Rc::clone(&self.main.fonts),
        )
    }

    // ---- where things are, in the main document's coordinates --------------

    /// A main-document y on the story's page, as the story's own y, when it
    /// falls in the band the story is drawn in.
    fn story_y(&self, document_y: f64) -> Option<f64> {
        let story = self.story.as_deref()?;
        let (page, y) = self.main.page_and_offset(document_y);
        let height = self.page_height();
        let (top, bottom) = story.band(height);
        (page == story.page && y >= top && y <= bottom).then(|| y - story.offset(height))
    }

    /// Whether a point on page `page` is in the open story's band.
    pub fn in_story(&self, page: usize, y_on_page: f64) -> bool {
        self.story.as_deref().is_some_and(|story| {
            let (top, bottom) = story.band(self.page_height());
            story.page == page && y_on_page >= top && y_on_page <= bottom
        })
    }

    /// Which story's margin a point on page `page` is in: above the body's
    /// top margin the header, below its bottom one the footer.
    pub fn margin_story(&self, page: usize, y_on_page: f64) -> Option<Story> {
        let section = self.main.renderer.section_index_for_zero_based_page(page);
        let layout = self.main.word_document.section_layout(Some(section));
        let height = self.page_height();
        let gutter_top = if layout.gutter_at_top {
            layout.gutter
        } else {
            0.0
        };
        if y_on_page < css(layout.margin_top + gutter_top) {
            Some(Story::Header)
        } else if y_on_page > height - css(layout.margin_bottom) {
            Some(Story::Footer)
        } else {
            None
        }
    }

    fn to_main(&self, rect: CaretRect) -> CaretRect {
        match self.story.as_deref() {
            None => rect,
            Some(story) => CaretRect {
                document_y: self.main.document_y(
                    story.page,
                    rect.document_y + story.offset(self.page_height()),
                ),
                ..rect
            },
        }
    }

    pub fn position_at(&self, x: f64, document_y: f64) -> Option<(usize, usize)> {
        match self.story.as_deref() {
            None => self.main.position_at(x, document_y),
            Some(story) => story.document.position_at(x, self.story_y(document_y)?),
        }
    }

    pub fn body_position_at(&self, x: f64, document_y: f64) -> Option<(usize, usize)> {
        match self.story.as_deref() {
            None => self.main.body_position_at(x, document_y),
            Some(story) => story.document.position_at(x, self.story_y(document_y)?),
        }
    }

    pub fn place_caret(&mut self, paragraph: usize, offset: usize) -> Option<CaretRect> {
        let rect = (**self).place_caret(paragraph, offset)?;
        Some(self.to_main(rect))
    }

    pub fn place_caret_at(&mut self, x: f64, document_y: f64) -> Option<CaretRect> {
        let rect = match self.story.is_some() {
            false => self.main.place_caret_at(x, document_y)?,
            true => {
                let y = self.story_y(document_y)?;
                (**self).place_caret_at(x, y)?
            }
        };
        Some(self.to_main(rect))
    }

    pub fn caret_rect(&self) -> Option<CaretRect> {
        Some(self.to_main((**self).caret_rect()?))
    }

    pub fn focus_rect(&mut self) -> Option<CaretRect> {
        let rect = (**self).focus_rect()?;
        Some(self.to_main(rect))
    }

    /// The page (0-based) `paragraph` starts on: the story's page for a
    /// story paragraph.
    pub fn paragraph_start_page(&mut self, paragraph: usize) -> Option<usize> {
        match self.story.as_deref() {
            Some(story) => Some(story.page),
            None => self.main.paragraph_start_page(paragraph),
        }
    }

    /// Tab in a table: the editor's own step, not the shared model's
    /// (`WordFormatting::step_cell`), which the open document would
    /// otherwise answer for.
    pub fn step_cell(&mut self, forwards: bool) -> Result<bool, String> {
        EditableDocument::step_cell(&mut **self, forwards)
    }

    pub fn selection_rects(&self) -> Vec<HighlightRect> {
        let rects = (**self).selection_rects();
        match self.story.as_deref() {
            None => rects,
            Some(story) => {
                let offset = story.offset(self.page_height());
                rects
                    .into_iter()
                    .map(|rect| HighlightRect {
                        page: story.page,
                        y: rect.y + offset,
                        ..rect
                    })
                    .collect()
            }
        }
    }

    /// A story object as the main page shows it.
    fn object_to_main(&self, object: PlacedObject) -> PlacedObject {
        match self.story.as_deref() {
            None => object,
            Some(story) => PlacedObject {
                page: story.page,
                y: object.y + story.offset(self.page_height()),
                ..object
            },
        }
    }

    pub fn object_at(&self, x: f64, document_y: f64) -> Option<PlacedObject> {
        match self.story.as_deref() {
            None => self.main.object_at(x, document_y),
            Some(story) => {
                let found = story.document.object_at(x, self.story_y(document_y)?)?;
                Some(self.object_to_main(found))
            }
        }
    }

    pub fn object_region(&self, target: FloatingObjectTarget) -> Option<PlacedObject> {
        let found = (**self).object_region(target)?;
        Some(self.object_to_main(found))
    }

    /// Moves or resizes a picture or shape to `rect` on its page, the
    /// story's own page for one in the open story. One undo step.
    pub fn place_object(
        &mut self,
        object: PlacedObject,
        (x, y, width, height): (f64, f64, f64, f64),
    ) -> Result<(), String> {
        use jade_office_model::word::picture::WordPicture;
        match self.story.as_deref() {
            None => self.main.place_object(object, (x, y, width, height)),
            Some(story) => {
                let offset = story.offset(self.page_height());
                let object = PlacedObject {
                    page: 0,
                    y: object.y - offset,
                    ..object
                };
                WordPicture::place_object(&mut **self, object, (x, y - offset, width, height))
            }
        }
    }

    // ---- saving --------------------------------------------------------------

    pub fn is_dirty(&self) -> bool {
        self.main.is_dirty()
            || self
                .story
                .as_deref()
                .is_some_and(|story| story.document.can_undo())
    }

    pub fn set_dark_paper(&mut self, on: bool) -> Result<bool, String> {
        if let Some(story) = self.story.as_mut() {
            story.document.set_dark_paper(on)?;
        }
        self.main.set_dark_paper(on)
    }

    /// The main document as it would be saved: with the open story's edits
    /// in it.
    pub fn export_docx(&mut self) -> Result<Vec<u8>, String> {
        let Some(story) = self
            .story
            .as_deref_mut()
            .filter(|story| story.document.can_undo())
        else {
            return self.main.export_docx();
        };
        let bytes = story.document.export_docx()?;
        let mut document = self.main.word_document.clone();
        document.replace_story_from_docx(
            story.section,
            role_name(story.role),
            story.variant,
            &bytes,
        )?;
        document
            .export_docx()
            .map_err(|error| format!("cannot save: {error:?}"))
    }

    pub fn export_pdf(&mut self) -> Result<Vec<u8>, String> {
        self.main.export_pdf()
    }

    // ---- opening and closing -------------------------------------------------

    /// Opens `role` of page `page` (0-based) for editing in place, closing
    /// one already open first. The story's caret starts at its beginning.
    pub fn open_story(&mut self, page: usize, role: Story) -> Result<(), String> {
        let main_caret = match self.story.as_deref() {
            Some(story) => story.main_caret,
            None => self.main.caret(),
        };
        self.close_story()?;
        let renderer = &self.main.renderer;
        let page = page.min(self.main.page_count().saturating_sub(1));
        let section = renderer.section_index_for_zero_based_page(page);
        let first_of_section =
            page == 0 || renderer.section_index_for_zero_based_page(page - 1) != section;
        let word = &self.main.word_document;
        let layout = word.section_layout(Some(section));
        let variant = if first_of_section && layout.different_first_page {
            "first"
        } else if layout.different_odd_even && (page + 1).is_multiple_of(2) {
            "even"
        } else {
            "default"
        };
        let linked = word.header_footer_linked(section, role_name(role), variant)?;
        let bytes = word.story_docx(section, role_name(role), variant, page + 1)?;
        let mut document = EditableDocument::open_resident(
            &bytes,
            self.main.scale,
            std::rc::Rc::clone(&self.main.fonts),
            std::rc::Rc::clone(&self.main.measurer),
            0..1,
        )?;
        document.set_dark_paper(self.main.dark_paper.get())?;
        let gutter_top = if layout.gutter_at_top {
            layout.gutter
        } else {
            0.0
        };
        let page_height = self.page_height();
        document.place_caret(0, 0);
        self.story = Some(Box::new(StoryEdit {
            document,
            page,
            section,
            role,
            variant,
            linked,
            body_top: css(layout.margin_top + gutter_top),
            body_bottom: page_height - css(layout.margin_bottom),
            content_top: css(match role {
                Story::Header => layout.header_distance,
                Story::Footer => layout.footer_distance,
            }),
            main_caret,
        }));
        self.redraw_page(page);
        Ok(())
    }

    /// Closes the story, writing it back into its part as one undo step of
    /// the main document when it was changed. The main document's caret
    /// goes back to where it was before the story opened.
    pub fn close_story(&mut self) -> Result<bool, String> {
        let Some(mut story) = self.story.take() else {
            return Ok(false);
        };
        if story.document.can_undo() {
            let bytes = story.document.export_docx()?;
            let (section, role, variant) = (story.section, role_name(story.role), story.variant);
            WordSession::execute_command(
                &mut self.main,
                Box::new(SnapshotEditCommand::new(
                    "edit_header_footer",
                    move |document| {
                        document
                            .replace_story_from_docx(section, role, variant, &bytes)
                            .map(|_| ())
                    },
                )),
                true,
            )?;
        }
        if let Some((paragraph, offset)) = story.main_caret {
            self.main.place_caret(paragraph, offset);
        }
        self.redraw_page(story.page);
        Ok(true)
    }

    /// Has the shell read page `page` again whole: what it showed there —
    /// the story over it, or not — changed without the page being drawn.
    fn redraw_page(&mut self, page: usize) {
        if let Some(painter) = self.main.page(page) {
            painter.mark_drawn();
        }
        self.main.repainted_pages.insert(page);
        self.main.repainted_from =
            Some(self.main.repainted_from.map_or(page, |from| from.min(page)));
    }

    /// The page to open the same story on in the previous or the next
    /// section — the first page of it — if there is one.
    pub fn story_section_page(&self, next: bool) -> Option<usize> {
        let story = self.story.as_deref()?;
        let renderer = &self.main.renderer;
        let pages = self.main.page_count();
        let section_of = |page: usize| renderer.section_index_for_zero_based_page(page);
        if next {
            (story.page + 1..pages).find(|&candidate| section_of(candidate) != story.section)
        } else {
            let candidate = (0..story.page)
                .rev()
                .find(|&candidate| section_of(candidate) != story.section)?;
            let previous = section_of(candidate);
            Some(
                (0..=candidate)
                    .find(|&first| section_of(first) == previous)
                    .unwrap_or(candidate),
            )
        }
    }

    /// The open story's section, as its layout stands.
    pub fn story_layout(&self) -> Option<jade_docx_headless_api::HeadlessSectionLayout> {
        let story = self.story.as_deref()?;
        Some(self.main.word_document.section_layout(Some(story.section)))
    }

    /// A field at the caret in the open story, showing `cached` until the
    /// page lays it out — Word writes the result it computed when the field
    /// went in.
    pub fn insert_story_field(
        &mut self,
        kind: HeadlessFieldKind,
        cached: String,
    ) -> Result<(), String> {
        let range = WordSession::selection_range(&**self).ok_or("no caret")?;
        let (start, end) = range.normalized();
        WordSession::apply_command(
            &mut **self,
            HeadlessCommand::InsertField {
                start_paragraph: start.paragraph,
                start_offset: start.offset,
                end_paragraph: end.paragraph,
                end_offset: end.offset,
                kind,
                cached_text: cached,
            },
            false,
        )
    }

    /// What a field of `kind` shows when it goes into the open story,
    /// before its page lays it out: `file_name` for FILENAME and `today`
    /// for DATE and TIME; `heading_missing` when no paragraph is in 見出し 1.
    pub fn story_field_text(
        &self,
        kind: HeadlessFieldKind,
        file_name: &str,
        today: &str,
        heading_missing: &str,
    ) -> String {
        let page = self.story.as_deref().map_or(0, |story| story.page);
        match kind {
            HeadlessFieldKind::Page => (page + 1).to_string(),
            HeadlessFieldKind::NumPages => self.main.page_count().to_string(),
            HeadlessFieldKind::Date | HeadlessFieldKind::Time => today.to_string(),
            HeadlessFieldKind::FileName => file_name.to_string(),
            HeadlessFieldKind::StyleRef => self
                .main
                .word_document
                .first_paragraph_text_in_style("heading 1")
                .filter(|text| !text.trim().is_empty())
                .unwrap_or_else(|| heading_missing.to_string()),
        }
    }

    /// A change to the open story's section: the story is written back,
    /// the main document changed (one undo step each), and the story opened
    /// again over the same page.
    pub fn change_story(&mut self, change: StoryChange) -> Result<(), String> {
        let Some(story) = self.story.as_deref() else {
            return Ok(());
        };
        let (page, role, section, variant) = (story.page, story.role, story.section, story.variant);
        self.close_story()?;
        let layout = self.main.word_document.section_layout(Some(section));
        let command = match change {
            StoryChange::LinkToPrevious(linked) => HeadlessCommand::SetHeaderFooterLink {
                section,
                role: role_name(role).to_string(),
                variant: variant.to_string(),
                linked,
            },
            StoryChange::DifferentFirst(on) => HeadlessCommand::SetHeaderFooterVariants {
                different_first_page: on,
                different_odd_even_pages: layout.different_odd_even,
                section: Some(section),
            },
            StoryChange::DifferentOddEven(on) => HeadlessCommand::SetHeaderFooterVariants {
                different_first_page: layout.different_first_page,
                different_odd_even_pages: on,
                section: Some(section),
            },
            StoryChange::Distance(further) => {
                let step: f32 = if further { 72.0 / 25.4 } else { -72.0 / 25.4 };
                let patch = match role {
                    Story::Header => HeadlessSectionLayoutPatch {
                        header_distance: Some((layout.header_distance + step).max(0.0)),
                        ..Default::default()
                    },
                    Story::Footer => HeadlessSectionLayoutPatch {
                        footer_distance: Some((layout.footer_distance + step).max(0.0)),
                        ..Default::default()
                    },
                };
                HeadlessCommand::SetSectionLayout {
                    scope: HeadlessSectionScope::Index { index: section },
                    patch: Box::new(patch),
                }
            }
        };
        let result = WordSession::apply_command(&mut self.main, command, true);
        self.open_story(page, role)?;
        result
    }
}

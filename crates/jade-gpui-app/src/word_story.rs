//! Headers and footers edited as Word edits them: double-click into the top
//! or the bottom of a page and the header or footer opens in place, the
//! body dimmed behind it, with every tool the body has — formatting, several
//! paragraphs, tables, pictures, fields — and the ヘッダーとフッター tab
//! over the ribbon. Double-click the body, or 閉じる, to go back.
//!
//! The story is edited as a document of its own (the engine's
//! `story_docx`), laid out on a page as wide as the section's, and drawn
//! over the page it was opened on, where its header or footer is. The view
//! keeps talking to one document: [`WordDoc`] is the story while one is
//! open and the main document otherwise, and maps the page geometry — the
//! pages, where a click lands, where the caret is drawn — to the main
//! document's pages. Closing writes the story back into its part as one
//! undo step of the main document.

use std::ops::{Deref, DerefMut};

use gpui::{div, prelude::*, px, AnyElement, Context, SharedString};
use jade_docx_editor::commands::snapshot_edit::SnapshotEditCommand;
use jade_docx_native_render::{CaretAffinity, CaretRect, NativeDocument, ObjectHit};
use jade_office_model::word::headless::{HeadlessCommand, HeadlessFieldKind};
use jade_office_model::word::page::Story;
use jade_office_model::word::WordSession;

use crate::ribbon::{group, listener, row, Action};
use crate::tooltip::Tooltip;
use crate::{i18n, theme, DocumentView};

/// Points as CSS pixels.
fn css(points: f32) -> f64 {
    f64::from(points) * 96.0 / 72.0
}

/// A header or footer open for editing over page `page` of the main
/// document.
pub struct StoryEdit {
    pub document: NativeDocument,
    /// The main document's page it is drawn on.
    pub page: usize,
    pub section: usize,
    pub role: Story,
    /// `default`, `first` or `even`.
    pub variant: &'static str,
    /// Whether the section shows the previous section's story.
    pub linked: bool,
    /// The body's top and bottom edges on the page, in CSS pixels: the
    /// band the story is drawn in reaches at least to them.
    body_top: f64,
    body_bottom: f64,
    /// Where the story's content starts on its own page.
    content_top: f64,
    /// The main document's caret, put back when the story closes.
    main_caret: (usize, usize),
}

/// The document the view edits: a story while one is open, the main
/// document otherwise.
pub struct WordDoc {
    main: NativeDocument,
    story: Option<StoryEdit>,
}

impl Deref for WordDoc {
    type Target = NativeDocument;

    fn deref(&self) -> &NativeDocument {
        match &self.story {
            Some(story) => &story.document,
            None => &self.main,
        }
    }
}

impl DerefMut for WordDoc {
    fn deref_mut(&mut self) -> &mut NativeDocument {
        match &mut self.story {
            Some(story) => &mut story.document,
            None => &mut self.main,
        }
    }
}

/// The shared ribbon commands act on whichever document is open: the story
/// while one is, so formatting, tables, pictures and fields go into it.
impl WordSession for WordDoc {
    fn word_document(&self) -> &jade_docx_editor::word_document::WordDocument {
        WordSession::word_document(&**self)
    }

    fn selection_range(&self) -> Option<jade_office_model::word::SelectionRangeOf> {
        WordSession::selection_range(&**self)
    }

    fn set_selection_range(&mut self, range: Option<jade_office_model::word::SelectionRangeOf>) {
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
        command: Box<dyn jade_docx_editor::commands::Command>,
        keep_selection: bool,
    ) -> Result<(), String> {
        WordSession::execute_command(&mut **self, command, keep_selection)
    }

    fn cell_id_for_paragraph(&self, paragraph: usize) -> Option<usize> {
        WordSession::cell_id_for_paragraph(&**self, paragraph)
    }

    fn cell_layout(&self, cell_id: usize) -> Option<&jade_office_model::word::CellLayout> {
        WordSession::cell_layout(&**self, cell_id)
    }

    fn cell_layouts(&self) -> &[jade_office_model::word::CellLayout] {
        WordSession::cell_layouts(&**self)
    }

    fn table_order_for(&self, table_id: usize) -> Option<usize> {
        WordSession::table_order_for(&**self, table_id)
    }

    fn hyperlink_regions(&self) -> Vec<jade_docx_editor::canvas_renderer::HyperlinkRegion> {
        WordSession::hyperlink_regions(&**self)
    }

    fn select_cells(&mut self, anchor_cell: usize, focus_cell: usize) -> Result<(), String> {
        WordSession::select_cells(&mut **self, anchor_cell, focus_cell)
    }

    fn math_zones(&self) -> Vec<jade_docx_editor::canvas_renderer::MathZone> {
        WordSession::math_zones(&**self)
    }
}

fn role_name(role: Story) -> &'static str {
    match role {
        Story::Header => "header",
        Story::Footer => "footer",
    }
}

impl StoryEdit {
    /// The bottom of the story's content on its own page.
    fn content_bottom(&self) -> f64 {
        self.document
            .renderer()
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
    fn offset(&self, page_height: f64) -> f64 {
        match self.role {
            Story::Header => 0.0,
            Story::Footer => page_height - self.content_top - self.content_bottom(),
        }
    }

    /// The rows of the page the story is drawn over, in CSS pixels.
    fn band(&self, page_height: f64) -> (f64, f64) {
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

impl WordDoc {
    pub fn new(main: NativeDocument) -> Self {
        Self { main, story: None }
    }

    pub fn main(&self) -> &NativeDocument {
        &self.main
    }

    pub fn story(&self) -> Option<&StoryEdit> {
        self.story.as_ref()
    }

    fn page_height(&self) -> f64 {
        self.main.layout_page_size().1
    }

    pub fn page_count(&self) -> usize {
        self.main.page_count()
    }

    pub fn page_pitch(&self) -> f64 {
        self.main.page_pitch()
    }

    pub fn layout_page_size(&self) -> (f64, f64) {
        self.main.layout_page_size()
    }

    pub fn document_y(&self, page_index: usize, y_on_page: f64) -> f64 {
        self.main.document_y(page_index, y_on_page)
    }

    pub fn page_and_offset(&self, document_y: f64) -> (usize, f64) {
        self.main.page_and_offset(document_y)
    }

    /// A main-document y on the story's page, as the story's own y, when it
    /// falls in the band the story is drawn in.
    fn story_y(&self, document_y: f64) -> Option<f64> {
        let story = self.story.as_ref()?;
        let (page, y) = self.main.page_and_offset(document_y);
        let height = self.page_height();
        let (top, bottom) = story.band(height);
        (page == story.page && y >= top && y <= bottom).then(|| y - story.offset(height))
    }

    /// Whether a point on `page` is in the open story's band.
    pub fn in_story(&self, page: usize, y_on_page: f64) -> bool {
        self.story.as_ref().is_some_and(|story| {
            let (top, bottom) = story.band(self.page_height());
            story.page == page && y_on_page >= top && y_on_page <= bottom
        })
    }

    pub fn position_at(&self, x: f64, y: f64) -> Option<(usize, usize)> {
        match &self.story {
            None => self.main.position_at(x, y),
            Some(story) => story.document.position_at(x, self.story_y(y)?),
        }
    }

    fn to_main(&self, rect: CaretRect) -> CaretRect {
        match &self.story {
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

    pub fn place_caret(&mut self, paragraph: usize, offset: usize) -> Option<CaretRect> {
        let rect = (**self).place_caret(paragraph, offset)?;
        Some(self.to_main(rect))
    }

    pub fn place_caret_with_affinity(
        &mut self,
        paragraph: usize,
        offset: usize,
        affinity: CaretAffinity,
    ) -> Option<CaretRect> {
        let rect = (**self).place_caret_with_affinity(paragraph, offset, affinity)?;
        Some(self.to_main(rect))
    }

    pub fn object_at(&self, x: f64, y: f64) -> Option<ObjectHit> {
        match &self.story {
            None => self.main.object_at(x, y),
            Some(story) => {
                let mut hit = story.document.object_at(x, self.story_y(y)?)?;
                hit.y += story.offset(self.page_height());
                hit.page = story.page;
                Some(hit)
            }
        }
    }

    fn story_object(&self, object: ObjectHit, y: f64) -> (ObjectHit, f64) {
        match &self.story {
            None => (object, y),
            Some(story) => {
                let offset = story.offset(self.page_height());
                (
                    ObjectHit {
                        page: 0,
                        y: object.y - offset,
                        ..object
                    },
                    y - offset,
                )
            }
        }
    }

    pub fn transform_object(
        &mut self,
        object: ObjectHit,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Result<(), String> {
        let (object, y) = self.story_object(object, y);
        (**self).transform_object(object, x, y, width, height)
    }

    pub fn retransform_object(
        &mut self,
        object: ObjectHit,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Result<(), String> {
        let (object, y) = self.story_object(object, y);
        (**self).retransform_object(object, x, y, width, height)
    }

    pub fn set_raster_scale(&mut self, scale: f64) -> Result<bool, String> {
        if let Some(story) = self.story.as_mut() {
            story.document.set_raster_scale(scale)?;
        }
        self.main.set_raster_scale(scale)
    }

    pub fn set_dark_paper(&mut self, on: bool) -> Result<bool, String> {
        if let Some(story) = self.story.as_mut() {
            story.document.set_dark_paper(on)?;
        }
        self.main.set_dark_paper(on)
    }

    /// The main document as it would be saved: with the open story's edits
    /// in it.
    pub fn export_docx(&self) -> Result<Vec<u8>, String> {
        let Some(story) = self
            .story
            .as_ref()
            .filter(|story| story.document.can_undo())
        else {
            return self.main.export_docx();
        };
        let bytes = story.document.export_docx()?;
        let mut document = self.main.word_document().clone();
        document.replace_story_from_docx(
            story.section,
            role_name(story.role),
            story.variant,
            &bytes,
        )?;
        document
            .export_docx()
            .map_err(|err| format!("export failed: {err:?}"))
    }

    pub fn export_pdf(&self) -> Result<Vec<u8>, String> {
        self.main.export_pdf()
    }

    /// A page as the view shows it: the story's page over its band, the rest
    /// of the page dimmed, a line where the story ends.
    pub fn page_pixels(&self, page_index: usize) -> Option<(Vec<u8>, i32, i32)> {
        let (mut pixels, width, height) = self.main.page_pixels(page_index)?;
        let Some(story) = self.story.as_ref().filter(|story| story.page == page_index) else {
            return Some((pixels, width, height));
        };
        let (story_pixels, story_width, story_height) = story.document.page_pixels(0)?;
        let page_height = self.page_height();
        let scale = f64::from(height) / page_height.max(1.0);
        let (top, bottom) = story.band(page_height);
        let (top, bottom) = (
            (top * scale).floor().max(0.0) as i32,
            ((bottom * scale).ceil() as i32).min(height),
        );
        let offset = (story.offset(page_height) * scale).round() as i32;
        let stride = width as usize * 4;
        let story_stride = story_width as usize * 4;
        let paper = [pixels[0], pixels[1], pixels[2], pixels[3]];
        for y in 0..height {
            let row = &mut pixels[y as usize * stride..(y as usize + 1) * stride];
            if y >= top && y < bottom {
                let source = y - offset;
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
        // The dashed line at the story's edge, in Word's blue (BGRA).
        let edge = match story.role {
            Story::Header => bottom.min(height - 1),
            Story::Footer => top.max(0),
        };
        let thickness = (scale.round() as i32).max(1);
        for y in edge..(edge + thickness).min(height) {
            let row = &mut pixels[y as usize * stride..(y as usize + 1) * stride];
            for (x, pixel) in row.chunks_exact_mut(4).enumerate() {
                if (x / (6 * thickness as usize)).is_multiple_of(2) {
                    pixel.copy_from_slice(&[0xD6, 0x6C, 0x0F, 0xFF]);
                }
            }
        }
        Some((pixels, width, height))
    }

    /// Opens `role` of page `page` for editing. `raster_scale` and `dark`
    /// are what the main document is painted with.
    fn open_story(
        &mut self,
        page: usize,
        role: Story,
        raster_scale: f64,
        dark: bool,
        main_caret: (usize, usize),
    ) -> Result<(), String> {
        let renderer = self.main.renderer();
        let section = renderer.section_index_for_zero_based_page(page);
        let first_of_section =
            page == 0 || renderer.section_index_for_zero_based_page(page - 1) != section;
        let word = self.main.word_document();
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
        let mut document = crate::open_document_bytes(&bytes)?;
        document.set_raster_scale(raster_scale)?;
        document.set_dark_paper(dark)?;
        let gutter_top = if layout.gutter_at_top {
            layout.gutter
        } else {
            0.0
        };
        let (_, page_height) = self.main.layout_page_size();
        self.story = Some(StoryEdit {
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
        });
        Ok(())
    }

    /// Closes the story, writing it back into its part as one undo step of
    /// the main document when it was changed. Returns the main document's
    /// caret from before it opened.
    fn close_story(&mut self) -> Result<Option<(usize, usize)>, String> {
        let Some(story) = self.story.take() else {
            return Ok(None);
        };
        if story.document.can_undo() {
            let bytes = story.document.export_docx()?;
            let (section, role, variant) = (story.section, role_name(story.role), story.variant);
            self.main.execute_command(
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
        Ok(Some(story.main_caret))
    }
}

/// Today, as a DATE field shows it before it is laid out (`2026/10/21`).
fn today() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as libc::time_t)
        .unwrap_or(0);
    // SAFETY: `localtime_r` writes only into the `tm` it is given, which is
    // plain data that all zeroes is a valid value of.
    let local = unsafe {
        let mut local: libc::tm = std::mem::zeroed();
        libc::localtime_r(&seconds, &mut local);
        local
    };
    format!(
        "{}/{}/{}",
        local.tm_year + 1900,
        local.tm_mon + 1,
        local.tm_mday
    )
}

/// The ヘッダーとフッター tab's commands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StoryAction {
    /// ヘッダーに移動 / フッターに移動.
    GoTo(Story),
    /// 前へ / 次へ: the previous or the next section's.
    Section(bool),
    /// 前と同じヘッダー/フッター.
    LinkToPrevious(bool),
    /// 先頭ページのみ別指定.
    DifferentFirst(bool),
    /// 奇数/偶数ページ別指定.
    DifferentOddEven(bool),
    /// ヘッダーの位置 / フッターの位置, a millimetre further or nearer.
    Distance(bool),
    /// A field at the caret.
    Field(HeadlessFieldKind),
    Close,
}

impl DocumentView {
    /// Whether a header or footer is open.
    pub(crate) fn editing_story(&self) -> bool {
        self.document.story().is_some()
    }

    /// Opens `role` on `page` in place.
    pub(crate) fn open_story(&mut self, page: usize, role: Story, cx: &mut Context<Self>) {
        if let Err(error) = self.close_story_editing() {
            self.table_status = Some(SharedString::from(error));
        }
        let caret = self.insert_at;
        match self.document.open_story(
            page,
            role,
            f64::from(self.raster_scale),
            self.dark_pages,
            caret,
        ) {
            Ok(()) => {
                self.ribbon_tab = crate::RibbonTab::HeaderFooter;
                self.selection_anchor = None;
                self.drag_anchor = None;
                self.selected_object = None;
                self.insert_at = (0, 0);
                self.caret = self.document.place_caret(0, 0);
                self.rebuild_all_images();
            }
            Err(error) => self.table_status = Some(SharedString::from(error)),
        }
        cx.notify();
    }

    /// Closes the open story, writing it back. The caret goes back to where
    /// it was in the body.
    pub(crate) fn close_story_editing(&mut self) -> Result<(), String> {
        let was_open = self.editing_story();
        let caret = self.document.close_story()?;
        if !was_open {
            return Ok(());
        }
        if self.ribbon_tab == crate::RibbonTab::HeaderFooter {
            self.ribbon_tab = crate::RibbonTab::Insert;
        }
        self.dirty |= self.document.can_undo();
        self.selected_object = None;
        if let Some(caret) = caret {
            self.insert_at = caret;
            self.caret = self.document.place_caret(caret.0, caret.1);
        }
        self.rebuild_all_images();
        Ok(())
    }

    fn close_story(&mut self, cx: &mut Context<Self>) {
        if let Err(error) = self.close_story_editing() {
            self.table_status = Some(SharedString::from(error));
        }
        cx.notify();
    }

    /// A click on a page while deciding about stories: a double-click in the
    /// top or bottom margin opens the header or the footer there, one in
    /// the body closes an open story, and a click outside an open story's
    /// band does nothing else. Returns whether the click was taken.
    pub(crate) fn story_click(
        &mut self,
        page: usize,
        y_on_page: f64,
        click_count: usize,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.editing_story() {
            if self.document.in_story(page, y_on_page) {
                return false;
            }
            if click_count >= 2 {
                if let Some(role) = self.margin_story(page, y_on_page) {
                    self.open_story(page, role, cx);
                } else {
                    self.close_story(cx);
                }
            }
            return true;
        }
        if click_count < 2 {
            return false;
        }
        match self.margin_story(page, y_on_page) {
            Some(role) => {
                self.open_story(page, role, cx);
                true
            }
            None => false,
        }
    }

    /// Which story's margin a point on `page` is in.
    fn margin_story(&self, page: usize, y_on_page: f64) -> Option<Story> {
        let main = self.document.main();
        let section = main.renderer().section_index_for_zero_based_page(page);
        let layout = main.word_document().section_layout(Some(section));
        let (_, height) = main.layout_page_size();
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

    /// Insert's ヘッダー / フッター: the story of the page the caret is on.
    pub(crate) fn edit_story_in_place(&mut self, role: Story, cx: &mut Context<Self>) {
        let page = self.caret_page_1idx().saturating_sub(1);
        self.open_story(page, role, cx);
    }

    /// The label on the open story's edge: which story, which section, and
    /// whether it follows the previous section's.
    pub(crate) fn story_label(&self, page: usize) -> Option<AnyElement> {
        let story = self.document.story()?;
        if story.page != page {
            return None;
        }
        let (_, height) = self.document.layout_page_size();
        let (top, bottom) = story.band(height);
        let zoom = self.zoom;
        let mut label = String::from(i18n::text(match (story.role, story.variant) {
            (Story::Header, "first") => "story.header.first",
            (Story::Header, "even") => "story.header.even",
            (Story::Header, _) => "story.header",
            (Story::Footer, "first") => "story.footer.first",
            (Story::Footer, "even") => "story.footer.even",
            (Story::Footer, _) => "story.footer",
        }));
        if self.document.main().word_document().section_count() > 1 {
            label.push_str(&format!(
                " - {}",
                i18n::format("story.section", &[("n", &(story.section + 1))])
            ));
        }
        let y = match story.role {
            Story::Header => bottom,
            Story::Footer => top - 16.0,
        };
        Some(
            div()
                .absolute()
                .left(px(8.0))
                .top(px(y as f32 * zoom))
                .flex()
                .gap_1()
                .child(
                    div()
                        .px_1()
                        .rounded_sm()
                        .bg(gpui::rgb(0x0F6CBD))
                        .text_color(gpui::rgb(0xFFFFFF))
                        .text_xs()
                        .child(label),
                )
                .when(story.linked, |labels| {
                    labels.child(
                        div()
                            .px_1()
                            .rounded_sm()
                            .bg(gpui::rgb(0x0F6CBD))
                            .text_color(gpui::rgb(0xFFFFFF))
                            .text_xs()
                            .child(i18n::text("story.linked")),
                    )
                })
                .into_any_element(),
        )
    }

    /// Runs a ヘッダーとフッター tab command.
    pub(crate) fn run_story_action(
        &mut self,
        action: StoryAction,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let Some(story) = self.document.story() else {
            return Ok(());
        };
        let (page, role, section, variant) = (story.page, story.role, story.section, story.variant);
        match action {
            StoryAction::Close => {
                self.close_story(cx);
                Ok(())
            }
            StoryAction::GoTo(target) => {
                self.open_story(page, target, cx);
                Ok(())
            }
            StoryAction::Section(next) => {
                let pages = self.document.page_count();
                let renderer = self.document.main().renderer();
                let target = if next {
                    (page + 1..pages).find(|&candidate| {
                        renderer.section_index_for_zero_based_page(candidate) != section
                    })
                } else {
                    (0..page)
                        .rev()
                        .find(|&candidate| {
                            renderer.section_index_for_zero_based_page(candidate) != section
                        })
                        .map(|candidate| {
                            let previous = renderer.section_index_for_zero_based_page(candidate);
                            (0..=candidate)
                                .find(|&first| {
                                    renderer.section_index_for_zero_based_page(first) == previous
                                })
                                .unwrap_or(candidate)
                        })
                };
                if let Some(target) = target {
                    self.open_story(target, role, cx);
                }
                Ok(())
            }
            StoryAction::Field(kind) => {
                let range = self.document.selection_range().ok_or("no caret")?;
                let (start, end) = range.normalized();
                // What the field shows until the page lays it out: Word
                // writes the result it computed when it was inserted.
                let cached = match kind {
                    HeadlessFieldKind::Page => (page + 1).to_string(),
                    HeadlessFieldKind::NumPages => self.document.page_count().to_string(),
                    HeadlessFieldKind::Date | HeadlessFieldKind::Time => today(),
                    HeadlessFieldKind::FileName => std::path::Path::new(self.path.as_ref())
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .filter(|name| !name.is_empty())
                        .unwrap_or_else(|| "document.docx".to_string()),
                    HeadlessFieldKind::StyleRef => self
                        .document
                        .main()
                        .word_document()
                        .first_paragraph_text_in_style("heading 1")
                        .filter(|text| !text.trim().is_empty())
                        .unwrap_or_else(|| i18n::text("story.field.heading_missing").to_string()),
                };
                self.document.apply_command(
                    HeadlessCommand::InsertField {
                        start_paragraph: start.paragraph,
                        start_offset: start.offset,
                        end_paragraph: end.paragraph,
                        end_offset: end.offset,
                        kind,
                        cached_text: cached,
                    },
                    false,
                )?;
                if let Some(range) = self.document.selection_range() {
                    self.insert_at = (range.focus.paragraph, range.focus.offset);
                }
                Ok(())
            }
            StoryAction::LinkToPrevious(_)
            | StoryAction::DifferentFirst(_)
            | StoryAction::DifferentOddEven(_)
            | StoryAction::Distance(_) => {
                // These change the main document: the story is written back
                // first, then opened again over the same page.
                self.close_story_editing()?;
                let layout = self.document.word_document().section_layout(Some(section));
                let command = match action {
                    StoryAction::LinkToPrevious(linked) => HeadlessCommand::SetHeaderFooterLink {
                        section,
                        role: role_name(role).to_string(),
                        variant: variant.to_string(),
                        linked,
                    },
                    StoryAction::DifferentFirst(on) => HeadlessCommand::SetHeaderFooterVariants {
                        different_first_page: on,
                        different_odd_even_pages: layout.different_odd_even,
                        section: Some(section),
                    },
                    StoryAction::DifferentOddEven(on) => HeadlessCommand::SetHeaderFooterVariants {
                        different_first_page: layout.different_first_page,
                        different_odd_even_pages: on,
                        section: Some(section),
                    },
                    _ => {
                        let further = matches!(action, StoryAction::Distance(true));
                        let step: f32 = if further { 72.0 / 25.4 } else { -72.0 / 25.4 };
                        let patch = match role {
                            Story::Header => {
                                jade_office_model::word::headless::HeadlessSectionLayoutPatch {
                                    header_distance: Some((layout.header_distance + step).max(0.0)),
                                    ..Default::default()
                                }
                            }
                            Story::Footer => {
                                jade_office_model::word::headless::HeadlessSectionLayoutPatch {
                                    footer_distance: Some((layout.footer_distance + step).max(0.0)),
                                    ..Default::default()
                                }
                            }
                        };
                        HeadlessCommand::SetSectionLayout {
                            scope: jade_office_model::word::headless::HeadlessSectionScope::Index {
                                index: section,
                            },
                            patch: Box::new(patch),
                        }
                    }
                };
                let result = self.document.apply_command(command, true);
                self.dirty = true;
                self.open_story(page, role, cx);
                result
            }
        }
    }

    /// The ヘッダーとフッター tab, while a story is open.
    pub(crate) fn story_groups(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(story) = self.document.story() else {
            return Vec::new();
        };
        let layout = self
            .document
            .main()
            .word_document()
            .section_layout(Some(story.section));
        let (role, linked, section) = (story.role, story.linked, story.section);
        let distance = match role {
            Story::Header => layout.header_distance,
            Story::Footer => layout.footer_distance,
        };
        let button = |id: &'static str,
                      label: &'static str,
                      tooltip: &'static str,
                      active: bool,
                      enabled: bool,
                      action: StoryAction,
                      cx: &mut Context<Self>| {
            div()
                .id(id)
                .flex()
                .flex_none()
                .items_center()
                .h(px(26.0))
                .px_2()
                .rounded_md()
                .text_xs()
                .whitespace_nowrap()
                .text_color(if enabled {
                    theme::text()
                } else {
                    theme::text_faint()
                })
                .when(active, |button| button.bg(theme::selected()))
                .when(enabled, |button| {
                    button
                        .cursor_pointer()
                        .hover(|button| button.bg(theme::hovered()))
                        .on_click(listener(Action::Story(action), cx))
                })
                .tooltip(Tooltip::text(tooltip))
                .child(label)
                .into_any_element()
        };
        let t = i18n::text;
        vec![
            group(
                t("story.group.insert"),
                vec![
                    row(vec![
                        button(
                            "story-page",
                            t("story.field.page"),
                            t("story.field.page_title"),
                            false,
                            true,
                            StoryAction::Field(HeadlessFieldKind::Page),
                            cx,
                        ),
                        button(
                            "story-pages",
                            t("story.field.pages"),
                            t("story.field.pages_title"),
                            false,
                            true,
                            StoryAction::Field(HeadlessFieldKind::NumPages),
                            cx,
                        ),
                    ]),
                    row(vec![
                        button(
                            "story-date",
                            t("story.field.date"),
                            t("story.field.date_title"),
                            false,
                            true,
                            StoryAction::Field(HeadlessFieldKind::Date),
                            cx,
                        ),
                        button(
                            "story-heading",
                            t("story.field.heading"),
                            t("story.field.heading_title"),
                            false,
                            true,
                            StoryAction::Field(HeadlessFieldKind::StyleRef),
                            cx,
                        ),
                        button(
                            "story-file",
                            t("story.field.file"),
                            t("story.field.file_title"),
                            false,
                            true,
                            StoryAction::Field(HeadlessFieldKind::FileName),
                            cx,
                        ),
                    ]),
                ],
            ),
            group(
                t("story.group.navigation"),
                vec![
                    row(vec![
                        button(
                            "story-to-header",
                            t("story.goto.header"),
                            t("story.goto.header"),
                            role == Story::Header,
                            role != Story::Header,
                            StoryAction::GoTo(Story::Header),
                            cx,
                        ),
                        button(
                            "story-to-footer",
                            t("story.goto.footer"),
                            t("story.goto.footer"),
                            role == Story::Footer,
                            role != Story::Footer,
                            StoryAction::GoTo(Story::Footer),
                            cx,
                        ),
                    ]),
                    row(vec![
                        button(
                            "story-previous",
                            t("story.previous"),
                            t("story.previous_title"),
                            false,
                            section > 0,
                            StoryAction::Section(false),
                            cx,
                        ),
                        button(
                            "story-next",
                            t("story.next"),
                            t("story.next_title"),
                            false,
                            section + 1 < layout.section_count,
                            StoryAction::Section(true),
                            cx,
                        ),
                        button(
                            "story-link",
                            t("story.link"),
                            t("story.link_title"),
                            linked,
                            section > 0,
                            StoryAction::LinkToPrevious(!linked),
                            cx,
                        ),
                    ]),
                ],
            ),
            group(
                t("story.group.options"),
                vec![
                    row(vec![button(
                        "story-first",
                        t("setup.header_footer.first_page"),
                        t("setup.header_footer.first_page"),
                        layout.different_first_page,
                        true,
                        StoryAction::DifferentFirst(!layout.different_first_page),
                        cx,
                    )]),
                    row(vec![button(
                        "story-odd-even",
                        t("setup.header_footer.odd_even"),
                        t("setup.header_footer.odd_even"),
                        layout.different_odd_even,
                        true,
                        StoryAction::DifferentOddEven(!layout.different_odd_even),
                        cx,
                    )]),
                ],
            ),
            group(
                t("story.group.position"),
                vec![row(vec![
                    button(
                        "story-nearer",
                        "−",
                        t("story.distance_title"),
                        false,
                        distance > 0.0,
                        StoryAction::Distance(false),
                        cx,
                    ),
                    div()
                        .text_xs()
                        .px_1()
                        .child(i18n::format(
                            match role {
                                Story::Header => "story.distance.header",
                                Story::Footer => "story.distance.footer",
                            },
                            &[("mm", &format!("{:.1}", distance * 25.4 / 72.0))],
                        ))
                        .into_any_element(),
                    button(
                        "story-further",
                        "+",
                        t("story.distance_title"),
                        false,
                        true,
                        StoryAction::Distance(true),
                        cx,
                    ),
                ])],
            ),
            group(
                t("story.group.close"),
                vec![row(vec![button(
                    "story-close",
                    t("story.close"),
                    t("story.close_title"),
                    false,
                    true,
                    StoryAction::Close,
                    cx,
                )])],
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::StoryAction;
    use crate::ribbon::Action;
    use jade_office_model::word::headless::HeadlessFieldKind;
    use jade_office_model::word::page::{Story, WordPage};
    use jade_office_model::word::{CharacterFormat, WordFormatting};

    fn open<'a>(
        cx: &'a mut gpui::TestAppContext,
        file: &str,
    ) -> (
        gpui::Entity<crate::DocumentView>,
        &'a mut gpui::VisualTestContext,
    ) {
        use gpui::Focusable;
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../jade-core")
            .join(file);
        let (view, cx) =
            cx.add_window_view(|_window, cx| crate::open_document_view(&path, cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        (view, cx)
    }

    fn part(bytes: &[u8], name: &str) -> String {
        let package = jade_opc::OpcPackage::open(bytes).unwrap();
        package.part_str(name).unwrap().unwrap_or_default()
    }

    /// A double-click in the top margin opens the header in place; what is
    /// typed — two paragraphs, one bold — goes into it, the page shows it
    /// over the dimmed body, and a double-click in the body writes it back
    /// as one undo step.
    #[gpui::test]
    fn the_header_opens_in_place_and_takes_paragraphs(cx: &mut gpui::TestAppContext) {
        use crate::Undo;
        let (view, cx) = open(cx, "public/fixtures/header.docx");
        let before = view.read_with(cx, |view, _| view.document.page_pixels(0).unwrap().0);
        view.update(cx, |view, cx| {
            assert!(
                !view.story_click(0, 400.0, 2, cx),
                "the body is not a margin"
            );
            assert!(view.story_click(0, 10.0, 2, cx));
            assert!(view.editing_story());
            assert!(view.ribbon_tab == crate::RibbonTab::HeaderFooter);
            let story = view.document.story().unwrap();
            assert_eq!(
                (story.role, story.variant, story.page),
                (Story::Header, "default", 0)
            );
            // The header's own paragraph is the story's first.
            assert!(view.document.paragraph_text(0).unwrap().contains("Hello"));
        });
        let shown = view.read_with(cx, |view, _| view.document.page_pixels(0).unwrap().0);
        assert_ne!(
            shown, before,
            "the page shows the story over the dimmed body"
        );

        view.update(cx, |view, _| {
            let end = view.document.paragraph_text(0).unwrap().len();
            view.document.insert_paragraph_break(0, end).unwrap();
            view.insert_at = (1, 0);
            view.caret = view.document.place_caret(1, 0);
        });
        cx.simulate_input("second line");
        view.update(cx, |view, cx| {
            let paragraph = view.insert_at.0;
            assert_eq!(paragraph, 1);
            view.document
                .select((paragraph, 0), (paragraph, "second".len()), 1)
                .unwrap();
            view.document.toggle_format(CharacterFormat::Bold).unwrap();
            // A double-click in the body closes the header.
            assert!(view.story_click(0, 400.0, 2, cx));
            assert!(!view.editing_story());
        });
        let saved = view.read_with(cx, |view, _| view.document.export_docx().unwrap());
        let header = part(&saved, "word/header1.xml");
        assert!(
            header.contains("second") && header.contains(" line"),
            "{header}"
        );
        assert_eq!(header.matches("<w:p>").count(), 2, "{header}");
        assert!(header.contains("<w:b/>"), "{header}");
        assert!(header.contains("Hello"));

        cx.dispatch_action(Undo);
        let undone = view.read_with(cx, |view, _| view.document.export_docx().unwrap());
        assert!(!part(&undone, "word/header1.xml").contains("second"));
    }

    /// Saving while a footer is open saves what was typed into it; fields
    /// go in at the caret.
    #[gpui::test]
    fn the_footer_takes_fields_and_is_saved_while_open(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open(cx, "public/fixtures/header.docx");
        view.update(cx, |view, cx| {
            let (_, height) = view.document.layout_page_size();
            assert!(view.story_click(0, height - 5.0, 2, cx));
            assert_eq!(view.document.story().unwrap().role, Story::Footer);
            view.document.place_caret(0, 0);
            view.run_ribbon(
                Action::Story(StoryAction::Field(HeadlessFieldKind::Page)),
                cx,
            );
            view.run_ribbon(
                Action::Story(StoryAction::Field(HeadlessFieldKind::NumPages)),
                cx,
            );
            for kind in [
                HeadlessFieldKind::Date,
                HeadlessFieldKind::FileName,
                HeadlessFieldKind::StyleRef,
            ] {
                view.run_ribbon(Action::Story(StoryAction::Field(kind)), cx);
                assert!(
                    view.table_status.is_none(),
                    "{kind:?}: {:?}",
                    view.table_status
                );
            }
        });
        let saved = view.read_with(cx, |view, _| {
            assert!(view.editing_story());
            view.document.export_docx().unwrap()
        });
        let footer = part(&saved, "word/footer1.xml");
        assert!(footer.contains("NUMPAGES"), "{footer}");
        assert!(footer.contains("PAGE"));
        for code in ["DATE", "FILENAME", "STYLEREF"] {
            assert!(footer.contains(code), "{code} missing from {footer}");
        }
    }

    /// 先頭ページのみ別指定 from the tab gives the first page a header of
    /// its own, open for typing, and the body's pages keep theirs.
    #[gpui::test]
    fn a_different_first_page_gets_its_own_header(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open(cx, "public/fixtures/header.docx");
        view.update(cx, |view, cx| {
            view.run_ribbon(Action::EditStory(Story::Header), cx);
            view.run_ribbon(Action::Story(StoryAction::DifferentFirst(true)), cx);
            let story = view.document.story().unwrap();
            assert_eq!(story.variant, "first");
        });
        cx.simulate_input("cover");
        view.update(cx, |view, cx| {
            view.run_ribbon(Action::Story(StoryAction::Close), cx);
            assert!(view.document.page_setup().margins.top > 0.0);
        });
        let saved = view.read_with(cx, |view, _| view.document.export_docx().unwrap());
        let document = part(&saved, "word/document.xml");
        assert!(document.contains("<w:titlePg/>"), "{document}");
        assert!(document.contains(r#"w:type="first""#));
    }

    /// 前と同じヘッダー: the second section's header follows the first's,
    /// then gets one of its own again.
    #[gpui::test]
    fn a_section_links_its_header_to_the_previous(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open(cx, "public/fixtures/header-footer-sections.docx");
        view.update(cx, |view, cx| {
            let pages = view.document.page_count();
            let second = (0..pages)
                .find(|&page| {
                    view.document
                        .main()
                        .renderer()
                        .section_index_for_zero_based_page(page)
                        == 1
                })
                .unwrap();
            view.open_story(second, Story::Header, cx);
            let story = view.document.story().unwrap();
            assert_eq!(story.section, 1);
            assert!(!story.linked);
            view.run_ribbon(Action::Story(StoryAction::LinkToPrevious(true)), cx);
            assert!(view.document.story().unwrap().linked);
            view.run_ribbon(Action::Story(StoryAction::Section(false)), cx);
            assert_eq!(view.document.story().unwrap().section, 0);
            view.run_ribbon(Action::Story(StoryAction::Close), cx);
        });
    }

    /// A picture and a table go into the header as into the body, and come
    /// back into its part with the picture's media beside it.
    #[gpui::test]
    fn the_header_takes_a_picture_and_a_table(cx: &mut gpui::TestAppContext) {
        use jade_office_model::word::picture::WordPicture;
        let (view, cx) = open(cx, "public/fixtures/header.docx");
        let mut png = Vec::new();
        image::RgbaImage::from_pixel(8, 8, image::Rgba([200, 30, 30, 255]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        view.update(cx, |view, cx| {
            view.run_ribbon(Action::EditStory(Story::Header), cx);
            view.document.place_caret(0, 0);
            WordPicture::insert_picture(&mut view.document, png.clone()).unwrap();
            view.document.place_caret(0, 0);
            WordFormatting::insert_table(&mut view.document, 2, 2).unwrap();
            view.run_ribbon(Action::Story(StoryAction::Close), cx);
        });
        let saved = view.read_with(cx, |view, _| view.document.export_docx().unwrap());
        let header = part(&saved, "word/header1.xml");
        assert!(header.contains("<w:drawing>"), "{header}");
        assert!(header.contains("<w:tbl>"), "{header}");
        let rels = part(&saved, "word/_rels/header1.xml.rels");
        let package = jade_opc::OpcPackage::open(&saved).unwrap();
        let media = package
            .part_names()
            .iter()
            .filter(|name| name.starts_with("word/media/"))
            .count();
        assert!(rels.contains("relationships/image"), "{rels}");
        assert!(media > 0);
    }

    /// ページ罫線 from デザイン: the dialog's box goes around the page.
    #[gpui::test]
    fn the_page_border_dialog_draws_a_frame(cx: &mut gpui::TestAppContext) {
        use crate::word_page_setup::{PageDialog, SetupAction};
        use jade_office_model::word::page_setup::{BorderSetting, WordPageSetup};
        let (view, cx) = open(cx, "public/fixtures/header.docx");
        view.update(cx, |view, cx| {
            view.run_ribbon(Action::Setup(SetupAction::OpenBorders), cx);
            let state = view.page_dialog.as_mut().unwrap();
            let PageDialog::Borders(dialog) = &mut state.dialog else {
                panic!("the ページ罫線 dialog");
            };
            dialog.set_setting(BorderSetting::Box);
        });
        cx.simulate_keystrokes("enter");
        view.read_with(cx, |view, _| {
            assert!(view.page_dialog.is_none());
            assert!(view.document.section_layout().page_borders.top.is_some());
        });
    }

    /// ヘッダーの位置 moves the header nearer the edge, a millimetre a step.
    #[gpui::test]
    fn the_header_distance_steps(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open(cx, "public/fixtures/header.docx");
        view.update(cx, |view, cx| {
            let before = view
                .document
                .word_document()
                .section_layout(None)
                .header_distance;
            view.run_ribbon(Action::EditStory(Story::Header), cx);
            view.run_ribbon(Action::Story(StoryAction::Distance(true)), cx);
            assert!(view.editing_story());
            let after = view
                .document
                .main()
                .word_document()
                .section_layout(None)
                .header_distance;
            assert!(
                (after - before - 72.0 / 25.4).abs() < 0.1,
                "{before} → {after}"
            );
            let _ = WordFormatting::format_state(&view.document);
        });
    }
}

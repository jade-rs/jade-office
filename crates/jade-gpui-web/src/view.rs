//! One open file: a Word document's pages, a deck or a workbook, laid out as
//! the native app lays out its views (`jade-gpui-app`'s `DocumentView`,
//! `DeckView` and `WorkbookView`) — the page or slide list on the left, the
//! deck's toolbar and notes, the formula bar and the sheet tabs.
//!
//! Sheets and slides can be edited, through the same editing cores the
//! native app uses (`jade-web`'s `Workbook` and `Deck`): select a cell and
//! type, an IME's composition included; select a shape and type into it,
//! move it by dragging or with the arrow keys, resize it from its corners,
//! delete it; undo and redo; and save, which hands the bytes to the page to
//! download. Word documents are viewed.

use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    canvas, div, img, point, prelude::*, px, rgb, rgba, size, svg, App, Bounds, Context,
    DispatchPhase, ElementInputHandler, EntityInputHandler, FocusHandle, Global, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, RenderImage, ScrollHandle,
    SharedString, UTF16Selection, Window,
};
use jade_tiny_skia_painter::TinySkiaPainter;
use jade_web::deck::{DragMode, ShapeDrag, TableChange, TextTarget};
use jade_web::sheet::CellRef;
use jade_web::text::{byte_to_utf16, utf16_to_byte, TextEdit};

use crate::ribbon::{self, Action};
use crate::{i18n, palette, Fonts, Host, UI_FONT};
use jade_web::editing::{CharacterFormat, FormatState, HighlightRect, WordFormatting};

mod sheet;
mod word_find;
mod word_story;

/// The deck's slide list, as in the native app.
const DECK_SIDEBAR_WIDTH: f32 = 200.0;
const SLIDE_THUMBNAIL_WIDTH: f32 = 152.0;
const TOOLBAR_HEIGHT: f32 = 40.0;
const NOTES_HEIGHT: f32 = 110.0;
const SLIDE_MARGIN: f32 = 28.0;
/// The document's page list.
const PAGES_SIDEBAR_WIDTH: f32 = 168.0;
const PAGE_THUMBNAIL_WIDTH: f32 = 112.0;
/// The pages' padding and the gap between them (`p_8`, `gap_6`).
const PAGES_PADDING: f32 = 32.0;
const PAGE_GAP: f32 = 24.0;
const FORMULA_BAR_HEIGHT: f32 = 32.0;
const SHEET_TABS_HEIGHT: f32 = 30.0;
/// How far an arrow key moves a shape, in slide px; ⌥ moves one.
const NUDGE_PX: f64 = 6.0;
/// The size of a selection's corner handles, in window px.
const HANDLE_SIZE: f32 = 9.0;

pub const ZOOM_MIN: f32 = 0.25;
pub const ZOOM_MAX: f32 = 4.0;
/// The stops − and + step through, Office's own set.
const ZOOM_STOPS: &[f32] = &[
    0.25, 0.33, 0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 4.0,
];

/// The next stop above (`up`) or below `current`.
pub fn zoom_step(current: f32, up: bool) -> f32 {
    const EPSILON: f32 = 0.005;
    if up {
        ZOOM_STOPS
            .iter()
            .copied()
            .find(|stop| *stop > current + EPSILON)
            .unwrap_or(ZOOM_MAX)
    } else {
        ZOOM_STOPS
            .iter()
            .rev()
            .copied()
            .find(|stop| *stop < current - EPSILON)
            .unwrap_or(ZOOM_MIN)
    }
}

fn clamp_zoom(zoom: f32) -> f32 {
    if zoom.is_finite() {
        zoom.clamp(ZOOM_MIN, ZOOM_MAX)
    } else {
        1.0
    }
}

/// The space the active view is given, set by the workspace each frame
/// before its views render, so a resize paints at the right size on the
/// frame it happens.
#[derive(Clone, Copy)]
pub struct EditorArea(pub f32, pub f32);

impl Global for EditorArea {}

fn editor_area(window: &Window, cx: &App) -> (f32, f32) {
    match cx.try_global::<EditorArea>() {
        Some(area) => (area.0, area.1),
        None => {
            let size = window.viewport_size();
            (f32::from(size.width), f32::from(size.height))
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Document,
    Workbook,
    Deck,
}

impl Kind {
    /// By extension: the Open XML packages, macro-enabled and template
    /// forms included, which open and save the same way.
    pub fn of(name: &str) -> Option<Kind> {
        let extension = name.rsplit_once('.')?.1.to_lowercase();
        match extension.as_str() {
            "docx" | "docm" | "dotx" | "dotm" => Some(Kind::Document),
            "xlsx" | "xlsm" | "xltx" | "xltm" => Some(Kind::Workbook),
            "pptx" | "pptm" | "potx" | "potm" | "ppsx" | "ppsm" => Some(Kind::Deck),
            _ => None,
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Kind::Document => "icons/file-text.svg",
            Kind::Workbook => "icons/sheet.svg",
            Kind::Deck => "icons/presentation.svg",
        }
    }

    /// A colour per format, so a tab or a row says what it is at a glance:
    /// the 400 shades on the dark chrome, a step deeper on the light one.
    pub fn color(self) -> gpui::Rgba {
        let (dark, light) = match self {
            Kind::Document => (0x60a5fa, 0x3b82f6),
            Kind::Workbook => (0x34d399, 0x059669),
            Kind::Deck => (0xfb923c, 0xea580c),
        };
        rgb(if palette::is_dark() { dark } else { light })
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Document => i18n::text("kind.document"),
            Kind::Workbook => i18n::text("kind.workbook"),
            Kind::Deck => i18n::text("kind.deck"),
        }
    }
}

/// Rows of device px a page image is cut into. An edit repaints a band of
/// its page — or, where the renderer cannot band it, the whole page — and
/// only the tiles whose pixels came out different are read back and uploaded
/// again: a whole page at 2x is megabytes, a tile a few hundred kilobytes.
const TILE_ROWS: u32 = 128;

/// How many of a Word document's pages are kept as pixels at least: the ones
/// on screen and those round them, up to this many — every page of a short
/// document, so its page list stays drawn. About 30 MB a page at 2x, with
/// its tiles.
const RESIDENT_PAGES: usize = 8;

/// A Word page's pixels, as a column of tiles.
struct PageImage {
    tiles: Vec<Arc<RenderImage>>,
    /// Each tile's [`TinySkiaPainter::rows_fingerprint`] when it was read.
    fingerprints: Vec<u64>,
    size: (u32, u32),
    /// The page as it was when it stopped being kept as pixels, an eighth
    /// its size: what the page list shows of a page off screen.
    thumbnail: Option<Arc<RenderImage>>,
}

/// How many times smaller, across and down, a kept thumbnail is than its
/// page: an A4 page at 2x keeps 200 × 280 px, about 220 KB.
const THUMBNAIL_STEP: usize = 8;

impl PageImage {
    /// A page kept as no pixels — off screen in a long document: the paper
    /// is drawn round it, and nothing else.
    fn blank() -> Self {
        Self {
            tiles: Vec::new(),
            fingerprints: Vec::new(),
            size: (0, 0),
            thumbnail: None,
        }
    }

    /// Lets the page's pixels go, keeping a thumbnail of them.
    fn release(&mut self, retired: &mut Vec<Arc<RenderImage>>) {
        if self.tiles.is_empty() {
            return;
        }
        let thumbnail = self.shrink();
        retired.append(&mut self.tiles);
        retired.extend(self.thumbnail.take());
        *self = Self {
            thumbnail,
            ..Self::blank()
        };
    }

    /// Drops a kept thumbnail that no longer shows the page.
    fn forget_thumbnail(&mut self, retired: &mut Vec<Arc<RenderImage>>) {
        retired.extend(self.thumbnail.take());
    }

    /// The tiles averaged down by [`THUMBNAIL_STEP`] into one picture.
    fn shrink(&self) -> Option<Arc<RenderImage>> {
        let (width, height) = (self.size.0 as usize, self.size.1 as usize);
        let (across, down) = (width / THUMBNAIL_STEP, height / THUMBNAIL_STEP);
        if across == 0 || down == 0 {
            return None;
        }
        let mut sums = vec![0u32; across * down * 4];
        for (index, tile) in self.tiles.iter().enumerate() {
            let bytes = tile.as_bytes(0)?;
            let first = index * TILE_ROWS as usize;
            for (row, line) in bytes.chunks_exact(width * 4).enumerate() {
                let y = (first + row) / THUMBNAIL_STEP;
                if y >= down {
                    break;
                }
                let (pixels, _) = line.as_chunks::<4>();
                for (x, pixel) in pixels.iter().take(across * THUMBNAIL_STEP).enumerate() {
                    let at = (y * across + x / THUMBNAIL_STEP) * 4;
                    for (sum, value) in sums[at..at + 4].iter_mut().zip(pixel) {
                        *sum += u32::from(*value);
                    }
                }
            }
        }
        let cells = (THUMBNAIL_STEP * THUMBNAIL_STEP) as u32;
        let pixels = sums.into_iter().map(|sum| (sum / cells) as u8).collect();
        let buffer = image::ImageBuffer::from_raw(across as u32, down as u32, pixels)?;
        Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
    }

    fn rows(painter: &TinySkiaPainter, index: u32) -> Range<u32> {
        index * TILE_ROWS..((index + 1) * TILE_ROWS).min(painter.height())
    }

    fn tile(painter: &TinySkiaPainter, index: u32) -> Option<Arc<RenderImage>> {
        let rows = Self::rows(painter, index);
        let height = rows.len() as u32;
        let buffer = image::ImageBuffer::from_raw(
            painter.width(),
            height,
            painter.bgra_premultiplied_rows(rows),
        )?;
        Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
    }

    /// Every tile of `painter`, whose drawing so far is then taken as read.
    fn new(painter: &TinySkiaPainter) -> Option<Self> {
        painter.take_damaged_rows();
        let size = (painter.width(), painter.height());
        let count = size.1.div_ceil(TILE_ROWS);
        let tiles = (0..count)
            .map(|index| Self::tile(painter, index))
            .collect::<Option<Vec<_>>>()?;
        let fingerprints = (0..count)
            .map(|index| painter.rows_fingerprint(Self::rows(painter, index)))
            .collect();
        Some(Self {
            tiles,
            fingerprints,
            size,
            thumbnail: None,
        })
    }

    /// Re-reads the tiles drawing has changed since the last read: of the
    /// rows it touched, those whose pixels differ from the tile's. A page of
    /// another size is read whole.
    fn refresh(&mut self, painter: &TinySkiaPainter, retired: &mut Vec<Arc<RenderImage>>) {
        if (painter.width(), painter.height()) != self.size {
            if let Some(fresh) = Self::new(painter) {
                retired.append(&mut self.tiles);
                retired.extend(self.thumbnail.take());
                *self = fresh;
            }
            return;
        }
        let Some(rows) = painter.take_damaged_rows() else {
            return;
        };
        for index in rows.start / TILE_ROWS..rows.end.div_ceil(TILE_ROWS) {
            let fingerprint = painter.rows_fingerprint(Self::rows(painter, index));
            let Some(known) = self.fingerprints.get_mut(index as usize) else {
                continue;
            };
            if *known == fingerprint {
                continue;
            }
            if let (Some(slot), Some(fresh)) = (
                self.tiles.get_mut(index as usize),
                Self::tile(painter, index),
            ) {
                retired.push(std::mem::replace(slot, fresh));
                *known = fingerprint;
            }
        }
    }

    /// The tiles stacked to fill their parent.
    fn element(&self) -> gpui::Div {
        if let (true, Some(thumbnail)) = (self.tiles.is_empty(), &self.thumbnail) {
            return div().size_full().child(img(thumbnail.clone()).size_full());
        }
        let height = self.size.1.max(1) as f32;
        div()
            .size_full()
            .flex()
            .flex_col()
            .children(self.tiles.iter().enumerate().map(|(index, tile)| {
                let rows = (self.size.1 - index as u32 * TILE_ROWS).min(TILE_ROWS);
                img(tile.clone())
                    .w_full()
                    .h(gpui::relative(rows as f32 / height))
                    .flex_none()
            }))
    }
}

fn image(painter: TinySkiaPainter) -> Option<Arc<RenderImage>> {
    let (width, height) = (painter.width(), painter.height());
    let buffer = image::ImageBuffer::from_raw(width, height, painter.bgra_premultiplied())?;
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
}

/// A slide's pixels and what they were painted for: slide, device width,
/// edit generation.
type PaintedSlide = (usize, u32, u64, Arc<RenderImage>);

/// The caret, the selection and the edit the ribbon's formatting was read
/// for.
type FormatKey = (
    Option<(usize, usize)>,
    Option<((usize, usize), (usize, usize))>,
    u64,
);

// One per open file, so the variants' sizes do not add up to anything.
#[allow(clippy::large_enum_variant)]
enum Document {
    /// Word: the document open for editing, its pages as images, shown as
    /// a column of pages.
    Pages {
        document: Box<jade_web::editing::EditableDocument>,
        pages: Vec<PageImage>,
        page_size: (f32, f32),
        scroll: ScrollHandle,
        current: usize,
        /// Where each page sits in the window, recorded at paint: clicks
        /// arrive in window coordinates.
        page_bounds: Rc<RefCell<Vec<Option<Bounds<Pixels>>>>>,
        /// The ribbon over the pages: its tab, its open list, the gallery.
        ribbon: Box<ribbon::Ribbon>,
        /// The formatting at the caret and the caret, selection and edit it
        /// was read for: reading it runs queries, so not on every frame.
        format: Option<(FormatKey, FormatState)>,
        /// The selection's highlight and the caret, selection and edit it
        /// was read for.
        highlight: Option<(FormatKey, Vec<HighlightRect>)>,
        /// Whether a press on a page is being dragged, extending the
        /// selection wherever the pointer goes.
        dragging: bool,
        /// Whether the caret is to be scrolled to once its page is painted:
        /// see `reveal_caret`.
        reveal_pending: bool,
        /// The find bar (⌘F), while it is open: typing goes into it.
        find: Option<Box<word_find::FindBar>>,
        /// A header or footer being typed, in its bar over the pages.
        story: Option<Box<crate::word_page::StoryBar>>,
    },
    Deck {
        deck: jade_web::Deck,
        current: usize,
        thumbnails: Vec<Option<Arc<RenderImage>>>,
        thumbnail_scroll: ScrollHandle,
        slide: Option<PaintedSlide>,
        selected: Option<u32>,
        /// The selected shape's name, for the status bar.
        selected_name: Option<SharedString>,
        /// Text being typed into a shape, or a cell of a table.
        edit: Option<(TextTarget, TextEdit)>,
        /// Cells selected across a table, as by Shift+click.
        cells: Option<CellSelection>,
        /// A shape being moved or resized with the mouse.
        drag: Option<ShapeDrag>,
        /// Window px per slide px of the slide on show.
        scale: f32,
    },
    Workbook(Box<sheet::Sheet>),
}

/// Cells selected across a table: what Merge Cells merges.
#[derive(Clone, Copy)]
struct CellSelection {
    id: u32,
    from: (usize, usize),
    to: (usize, usize),
    /// The cells' box, in slide px.
    rect: jade_web::deck::ShapeRect,
}

pub struct DocView {
    pub name: SharedString,
    pub kind: Kind,
    document: Document,
    retired: Vec<Arc<RenderImage>>,
    /// Bumped by every edit, so pictures painted before it are repainted.
    generation: u64,
    status: SharedString,
    /// Where the editable surface — the grid or the slide — sits in the
    /// window, recorded at paint: clicks arrive in window coordinates.
    surface: Rc<Cell<Option<Bounds<Pixels>>>>,
    focus: FocusHandle,
    /// A chosen zoom, or `None` to fit (the deck's default).
    zoom: Option<f32>,
    /// The zoom on show, a fitted one included.
    shown_zoom: f32,
    show_notes: bool,
    slideshow: bool,
    /// Whether the browser's paste event came after the last ⌘V.
    paste_arrived: bool,
    host: Host,
}

impl DocView {
    /// Opens `bytes` as the file `name` names.
    pub fn open(
        name: SharedString,
        bytes: &[u8],
        fonts: &Fonts,
        host: Host,
        focus: FocusHandle,
    ) -> Result<Self, String> {
        let kind = Kind::of(&name)
            .ok_or_else(|| i18n::format("workspace.unsupported", &[("name", &name)]))?;
        // An OLE compound file, not a package: password-protected, or one
        // of the old binary formats.
        if bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]) {
            return Err(i18n::format("workspace.protected", &[("name", &name)]));
        }
        let document = match kind {
            Kind::Document => {
                // The first pages only, as pixels: the view keeps the ones on
                // screen as it scrolls (see `follow_scroll`).
                let opened = jade_web::editing::EditableDocument::open_resident(
                    bytes,
                    fonts.scale,
                    fonts.backend.fonts(),
                    Rc::clone(&fonts.measurer),
                    0..RESIDENT_PAGES,
                )?;
                let (width, height) = opened.page_size();
                let mut ribbon = Box::<ribbon::Ribbon>::default();
                ribbon.read_styles(&opened);
                Document::Pages {
                    ribbon,
                    format: None,
                    highlight: None,
                    dragging: false,
                    reveal_pending: false,
                    find: None,
                    story: None,
                    pages: (0..opened.page_count())
                        .map(|index| {
                            opened
                                .page(index)
                                .as_ref()
                                .and_then(PageImage::new)
                                .unwrap_or_else(PageImage::blank)
                        })
                        .collect(),
                    document: Box::new(opened),
                    page_size: (width as f32, height as f32),
                    scroll: ScrollHandle::new(),
                    current: 0,
                    page_bounds: Rc::new(RefCell::new(Vec::new())),
                }
            }
            Kind::Deck => {
                let deck = jade_web::Deck::open(bytes, fonts.backend.clone())?;
                if deck.slide_count() == 0 {
                    return Err(i18n::format("deck.empty", &[("name", &name)]));
                }
                Document::Deck {
                    thumbnails: vec![None; deck.slide_count()],
                    thumbnail_scroll: ScrollHandle::new(),
                    deck,
                    current: 0,
                    slide: None,
                    selected: None,
                    selected_name: None,
                    edit: None,
                    cells: None,
                    drag: None,
                    scale: 1.0,
                }
            }
            Kind::Workbook => Document::Workbook(Box::new(sheet::Sheet::open(
                jade_web::Workbook::open(bytes, &fonts.time_zone, fonts.backend.clone())?,
            ))),
        };
        Ok(Self {
            name,
            kind,
            document,
            retired: Vec::new(),
            generation: 0,
            status: SharedString::default(),
            surface: Rc::new(Cell::new(None)),
            focus,
            zoom: (kind != Kind::Deck).then_some(1.0),
            shown_zoom: 1.0,
            show_notes: true,
            slideshow: false,
            paste_arrived: false,
            host,
        })
    }

    // ---- what the workspace asks ------------------------------------------

    /// What the status bar shows for this view, as the native views do.
    pub fn status_items(&self) -> Vec<SharedString> {
        let mut items: Vec<SharedString> = Vec::new();
        match &self.document {
            Document::Pages { pages, current, .. } => items.push(
                i18n::format(
                    "doc.position",
                    &[("current", &(current + 1)), ("count", &pages.len())],
                )
                .into(),
            ),
            Document::Deck {
                deck,
                current,
                selected_name,
                ..
            } => {
                items.push(
                    i18n::format(
                        "deck.position",
                        &[("current", &(current + 1)), ("count", &deck.slide_count())],
                    )
                    .into(),
                );
                if let Some(name) = selected_name {
                    items.push(i18n::format("deck.selected_shape", &[("name", name)]).into());
                }
            }
            Document::Workbook(sheet) => sheet.status_items(&mut items),
        }
        if !self.status.is_empty() {
            items.push(self.status.clone());
        }
        items
    }

    /// The chosen zoom, or the fitted one last shown: the workspace's status
    /// bar renders before this view does, so a fresh choice must not wait for
    /// the next paint to show.
    pub fn zoom(&self) -> f32 {
        self.zoom.unwrap_or(self.shown_zoom)
    }

    pub fn set_zoom(&mut self, zoom: f32, cx: &mut Context<Self>) {
        let zoom = clamp_zoom(zoom);
        if let Document::Workbook(sheet) = &self.document {
            let scroll = &sheet.scroll;
            // Keep the same cell at the top left: the offset is in screen
            // px, which a zoom rescales.
            let ratio = zoom / self.zoom().max(0.01);
            let offset = scroll.offset();
            scroll.set_offset(point(offset.x * ratio, offset.y * ratio));
        }
        self.zoom = Some(zoom);
        cx.notify();
    }

    /// Fits the slide, or the page's width; a sheet goes back to 100%.
    pub fn zoom_to_fit(&mut self, window: &Window, cx: &mut Context<Self>) {
        match &self.document {
            Document::Deck { deck, .. } => {
                let (area_width, area_height) = editor_area(window, cx);
                let notes_height = if self.show_notes { NOTES_HEIGHT } else { 0.0 };
                self.shown_zoom =
                    deck_fit(deck.slide_size(), (area_width, area_height - notes_height));
                self.zoom = None;
                cx.notify();
            }
            Document::Pages { page_size, .. } => {
                let (area_width, _) = editor_area(window, cx);
                let fit =
                    (area_width - PAGES_SIDEBAR_WIDTH - PAGES_PADDING * 2.0) / page_size.0.max(1.0);
                self.set_zoom(fit, cx);
            }
            Document::Workbook(_) => self.set_zoom(1.0, cx),
        }
    }

    pub fn is_dirty(&self) -> bool {
        match &self.document {
            Document::Pages { document, .. } => document.is_dirty(),
            Document::Deck { deck, .. } => deck.is_dirty(),
            Document::Workbook(sheet) => sheet.book.is_dirty(),
        }
    }

    pub fn is_editable(&self) -> bool {
        true
    }

    pub fn in_slideshow(&self) -> bool {
        self.slideshow
    }

    // ---- editing ----------------------------------------------------------

    /// After an edit: repaint what showed the old content.
    fn edited(&mut self, result: Result<bool, String>, cx: &mut Context<Self>) {
        match result {
            Ok(true) => {
                self.generation += 1;
                self.status = SharedString::default();
                // Undo can touch any slide; thumbnails are cheap to repaint.
                if let Document::Deck { thumbnails, .. } = &mut self.document {
                    for thumbnail in thumbnails.iter_mut() {
                        if let Some(old) = thumbnail.take() {
                            self.retired.push(old);
                        }
                    }
                }
            }
            Ok(false) => {}
            // A formula Excel would refuse: its words, not a write error.
            Err(error) if error == jade_web::typing::FORMULA_PROBLEM => {
                self.status = i18n::text("sheet.formula_problem").into();
            }
            // An entry a data validation refuses: the rule's words, or
            // Excel's.
            Err(error) if jade_web::sheet::validation_refusal(&error).is_some() => {
                let message = jade_web::sheet::validation_refusal(&error).unwrap_or_default();
                self.status = if message.is_empty() {
                    i18n::text("sheet.validation_refused").into()
                } else {
                    message.to_string().into()
                };
            }
            Err(error) => {
                let key = if self.kind == Kind::Deck {
                    "deck.edit_failed"
                } else {
                    "sheet.edit_failed"
                };
                self.status = i18n::format(key, &[("error", &error)]).into();
            }
        }
        cx.notify();
    }

    /// Commits any edit in progress and hands the file's bytes to the page.
    pub fn save(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        self.commit_slide_text(cx);
        let bytes = match &mut self.document {
            Document::Pages { document, .. } => document.export_docx(),
            Document::Deck { deck, .. } => Ok(deck.save_bytes()),
            Document::Workbook(sheet) => sheet.book.save_bytes(),
        };
        self.status = match bytes {
            Ok(bytes) => {
                self.host.save(&self.name, &bytes);
                i18n::format("web.downloaded", &[("name", &self.name)]).into()
            }
            Err(error) => i18n::format("sheet.save_failed", &[("error", &error)]).into(),
        };
        cx.notify();
    }

    /// PDF として書き出す: the Word document's pages, as the page shows
    /// them, handed to the page to download.
    pub fn export_pdf(&mut self, cx: &mut Context<Self>) {
        let Document::Pages { document, .. } = &mut self.document else {
            return;
        };
        let pdf = document.export_pdf();
        self.refresh_repainted();
        let name = format!(
            "{}.pdf",
            self.name
                .rsplit_once('.')
                .map_or(self.name.as_ref(), |(stem, _)| stem)
        );
        self.status = match pdf {
            Ok(bytes) => {
                self.host.save(&name, &bytes);
                i18n::format("web.downloaded", &[("name", &name)]).into()
            }
            Err(error) => i18n::format("sheet.save_failed", &[("error", &error)]).into(),
        };
        cx.notify();
    }

    pub fn step_history(&mut self, undo: bool, cx: &mut Context<Self>) {
        let keep;
        let result = match &mut self.document {
            Document::Pages { document, .. } => {
                let result = if undo {
                    document.undo()
                } else {
                    document.redo()
                };
                self.refresh_repainted();
                // To where the step put the caret, as Word scrolls to what
                // it undid.
                self.reveal_caret();
                self.word_result(result.map(|_| ()), cx);
                return;
            }
            Document::Deck {
                deck,
                edit,
                selected,
                current,
                ..
            } => {
                *edit = None;
                let result = if undo { deck.undo() } else { deck.redo() };
                // The selection stays on a shape the step kept, as in
                // PowerPoint; one it took away is deselected.
                keep = (*selected).filter(|&id| deck.shape(*current, id).is_some());
                result
            }
            Document::Workbook(sheet) => {
                sheet.edit = None;
                let result = if undo {
                    sheet.book.undo()
                } else {
                    sheet.book.redo()
                };
                // Undo can bring a sheet back or take one away.
                self.sheets_changed(result, cx);
                return;
            }
        };
        self.edited(result, cx);
        self.select_shape(keep);
    }

    // ---- the deck ---------------------------------------------------------

    fn select_shape(&mut self, id: Option<u32>) {
        if let Document::Deck {
            deck,
            current,
            selected,
            selected_name,
            ..
        } = &mut self.document
        {
            *selected = id;
            *selected_name = id
                .and_then(|id| deck.shape(*current, id))
                .map(|shape| SharedString::from(shape.name));
        }
    }

    fn go_to_slide(&mut self, index: usize, cx: &mut Context<Self>) {
        self.commit_slide_text(cx);
        self.select_shape(None);
        if let Document::Deck {
            deck,
            current,
            thumbnail_scroll,
            drag,
            ..
        } = &mut self.document
        {
            *drag = None;
            *current = index.min(deck.slide_count().saturating_sub(1));
            thumbnail_scroll.scroll_to_item(*current);
        }
        cx.notify();
    }

    fn step_slide(&mut self, delta: isize, cx: &mut Context<Self>) {
        if let Document::Deck { current, .. } = &self.document {
            let to = current.saturating_add_signed(delta);
            self.go_to_slide(to, cx);
        }
    }

    fn start_slideshow(&mut self, from: Option<usize>, cx: &mut Context<Self>) {
        if let Some(from) = from {
            self.go_to_slide(from, cx);
        } else {
            self.commit_slide_text(cx);
            self.select_shape(None);
        }
        self.slideshow = true;
        cx.notify();
    }

    /// Writes the text being typed into a shape, if any.
    fn commit_slide_text(&mut self, cx: &mut Context<Self>) {
        let Document::Deck {
            deck,
            edit,
            current,
            ..
        } = &mut self.document
        else {
            return;
        };
        let Some((target, text)) = edit.take() else {
            return;
        };
        let result = deck.set_text(*current, target, &text.text);
        self.edited(result, cx);
    }

    /// Starts typing into the selected shape — in a table, into the cell at
    /// `point`, or the first cell without one.
    fn begin_slide_text(&mut self, point: Option<(f64, f64)>, cx: &mut Context<Self>) {
        let Document::Deck {
            deck,
            edit,
            current,
            selected,
            ..
        } = &mut self.document
        else {
            return;
        };
        let Some(id) = *selected else {
            return;
        };
        let target = deck.text_target(*current, id, point);
        match deck.text(*current, target) {
            Some(text) => *edit = Some((target, TextEdit::new(text))),
            None => self.status = i18n::text("deck.text_locked").into(),
        }
        cx.notify();
    }

    /// Starts typing into `target` from the text it has — all of it
    /// selected, when `whole`, so typing replaces it.
    fn edit_slide_text(&mut self, target: TextTarget, whole: bool, cx: &mut Context<Self>) {
        let Document::Deck {
            deck,
            edit,
            current,
            ..
        } = &mut self.document
        else {
            return;
        };
        if let Some(text) = deck.text(*current, target) {
            let text = if whole {
                TextEdit::selected(text)
            } else {
                TextEdit::new(text)
            };
            *edit = Some((target, text));
        }
        cx.notify();
    }

    /// The cell being typed into, if a table's.
    fn edited_cell(&self) -> Option<TextTarget> {
        match &self.document {
            Document::Deck {
                edit: Some((target @ TextTarget::Cell { .. }, _)),
                ..
            } => Some(*target),
            _ => None,
        }
    }

    /// Tab in a table: the text written, then the next cell (⇧Tab: the one
    /// before). Tab in the last cell adds a row and goes to its first, as
    /// in PowerPoint.
    fn tab_in_table(&mut self, backwards: bool, cx: &mut Context<Self>) {
        let Some(from @ TextTarget::Cell { id, row, .. }) = self.edited_cell() else {
            return;
        };
        self.commit_slide_text(cx);
        let Document::Deck { deck, current, .. } = &mut self.document else {
            return;
        };
        let slide = *current;
        match deck.next_cell(slide, from, backwards) {
            // The next cell's text selected, as PowerPoint's Tab does.
            Some(next) => self.edit_slide_text(next, true, cx),
            None if !backwards => {
                let result = deck.change_table(slide, id, TableChange::InsertRow(row + 1));
                let added = matches!(result, Ok(true));
                self.edited(result, cx);
                if !added {
                    return;
                }
                self.edit_slide_text(
                    TextTarget::Cell {
                        id,
                        row: row + 1,
                        column: 0,
                    },
                    false,
                    cx,
                );
            }
            None => self.edit_slide_text(from, true, cx),
        }
    }

    /// Extends a cell selection — from the cell being typed into, or the
    /// selection's first — to the cell at (`x`, `y`) of the same table.
    fn select_slide_cells_to(&mut self, x: f64, y: f64, cx: &mut Context<Self>) -> bool {
        let Document::Deck {
            deck,
            current,
            edit,
            cells,
            ..
        } = &mut self.document
        else {
            return false;
        };
        let (id, from) = match (*cells, edit.as_ref().map(|(target, _)| *target)) {
            (Some(cells), _) => (cells.id, cells.from),
            (None, Some(TextTarget::Cell { id, row, column })) => (id, (row, column)),
            _ => return false,
        };
        let TextTarget::Cell { row, column, .. } = deck.text_target(*current, id, Some((x, y)))
        else {
            return false;
        };
        let Some(rect) = deck.cell_range_rect(*current, id, from, (row, column)) else {
            return false;
        };
        self.commit_slide_text(cx);
        if let Document::Deck { cells, .. } = &mut self.document {
            *cells = Some(CellSelection {
                id,
                from,
                to: (row, column),
                rect,
            });
        }
        cx.notify();
        true
    }

    /// Merge Cells: the selected cells become one, typed into next.
    fn merge_slide_cells(&mut self, cx: &mut Context<Self>) {
        let Document::Deck {
            deck,
            current,
            cells,
            ..
        } = &mut self.document
        else {
            return;
        };
        let Some(selection) = cells.take() else {
            return;
        };
        let change = jade_web::deck::merge_cells(selection.from, selection.to);
        let result = deck.change_table(*current, selection.id, change);
        let merged = matches!(result, Ok(true));
        self.edited(result, cx);
        if let (true, TableChange::MergeCells { row, column, .. }) = (merged, change) {
            let target = TextTarget::Cell {
                id: selection.id,
                row,
                column,
            };
            self.edit_slide_text(target, false, cx);
        }
        cx.notify();
    }

    /// Split Cells: the merged cell being typed into becomes its cells.
    fn split_slide_cell(&mut self, cx: &mut Context<Self>) {
        let Some(target @ TextTarget::Cell { id, row, column }) = self.edited_cell() else {
            return;
        };
        self.commit_slide_text(cx);
        let Document::Deck { deck, current, .. } = &mut self.document else {
            return;
        };
        let result = deck.change_table(*current, id, TableChange::SplitCell { row, column });
        let split = matches!(result, Ok(true));
        self.edited(result, cx);
        if split {
            self.edit_slide_text(target, false, cx);
        }
    }

    /// A row or column added or removed beside the cell being typed into,
    /// its text written first.
    fn change_slide_table(
        &mut self,
        change: impl FnOnce(usize, usize) -> TableChange,
        cx: &mut Context<Self>,
    ) {
        let Some(TextTarget::Cell { id, row, column }) = self.edited_cell() else {
            return;
        };
        self.commit_slide_text(cx);
        let Document::Deck { deck, current, .. } = &mut self.document else {
            return;
        };
        let result = deck.change_table(*current, id, change(row, column));
        self.edited(result, cx);
        cx.notify();
    }

    fn deck_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let modifiers = event.keystroke.modifiers;
        let key = event.keystroke.key.as_str();
        if self.slideshow {
            match key {
                "right" | "down" | "space" | "enter" | "pagedown" => self.step_slide(1, cx),
                "left" | "up" | "backspace" | "pageup" => self.step_slide(-1, cx),
                "escape" => {
                    self.slideshow = false;
                    cx.notify();
                }
                _ => return false,
            }
            return true;
        }
        if let Document::Deck {
            cells: cells @ Some(_),
            ..
        } = &mut self.document
        {
            if key == "escape" {
                *cells = None;
                cx.notify();
                return true;
            }
        }
        let Document::Deck {
            deck,
            current,
            selected,
            edit,
            ..
        } = &mut self.document
        else {
            return false;
        };
        if let Some((target, text)) = edit.as_mut() {
            if text.marked.is_some() {
                return false;
            }
            let in_table = matches!(target, TextTarget::Cell { .. });
            match key {
                // As in PowerPoint, Escape leaves the text, keeping it.
                "escape" => self.commit_slide_text(cx),
                "tab" if in_table => self.tab_in_table(modifiers.shift, cx),
                "enter" => text.insert("\n"),
                "backspace" => text.backspace(),
                "delete" => text.delete(),
                "left" => text.step(false),
                "right" => text.step(true),
                "home" => text.to_end(false),
                "end" => text.to_end(true),
                _ => return false,
            }
            cx.notify();
            return true;
        }
        let slide = *current;
        if let Some(id) = *selected {
            let step = if modifiers.alt { 1.0 } else { NUDGE_PX };
            let result = match key {
                "left" => deck.move_shape(slide, id, -step, 0.0),
                "right" => deck.move_shape(slide, id, step, 0.0),
                "up" => deck.move_shape(slide, id, 0.0, -step),
                "down" => deck.move_shape(slide, id, 0.0, step),
                "delete" | "backspace" => {
                    let result = deck.delete_shape(slide, id);
                    self.select_shape(None);
                    result
                }
                "enter" | "f2" => {
                    self.begin_slide_text(None, cx);
                    return true;
                }
                "escape" => {
                    self.select_shape(None);
                    cx.notify();
                    return true;
                }
                "tab" => {
                    let next = deck.next_shape(slide, Some(id), modifiers.shift);
                    self.select_shape(next);
                    cx.notify();
                    return true;
                }
                _ => return false,
            };
            self.edited(result, cx);
            return true;
        }
        match key {
            "down" | "right" | "pagedown" => self.step_slide(1, cx),
            "up" | "left" | "pageup" => self.step_slide(-1, cx),
            "tab" => {
                let next = deck.next_shape(slide, None, modifiers.shift);
                self.select_shape(next);
                cx.notify();
            }
            _ => return false,
        }
        true
    }

    fn slide_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        cx.stop_propagation();
        let Some((x, y)) = self.slide_point(event.position) else {
            return;
        };
        // Shift+click in a table whose cell is being typed into, or whose
        // cells are selected, selects the cells between, as in PowerPoint.
        if event.modifiers.shift && self.select_slide_cells_to(x, y, cx) {
            return;
        }
        let Document::Deck {
            deck,
            current,
            edit,
            cells,
            ..
        } = &mut self.document
        else {
            return;
        };
        *cells = None;
        // A click inside the shape or cell being typed into stays in the
        // text.
        if let Some((target, _)) = edit {
            let inside = deck
                .text_rect(*current, *target)
                .is_some_and(|rect| rect.contains(x, y));
            if inside {
                return;
            }
        }
        let hit = deck.shape_at(*current, x, y);
        let movable = hit
            .and_then(|id| deck.shape(*current, id))
            .is_some_and(|shape| shape.can_move);
        self.commit_slide_text(cx);
        self.select_shape(hit);
        match hit {
            // In a table, the cell under the click.
            Some(_) if event.click_count >= 2 => self.begin_slide_text(Some((x, y)), cx),
            // Dragging the shape moves it.
            Some(id) if movable => {
                if let Document::Deck { drag, .. } = &mut self.document {
                    *drag = Some(ShapeDrag::new(id, DragMode::Move, (x, y)));
                }
            }
            _ => {}
        }
        cx.notify();
    }

    /// A press on a corner handle of the selected shape: dragging it resizes
    /// the shape from the opposite corner.
    fn start_slide_resize(
        &mut self,
        left: bool,
        top: bool,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        cx.stop_propagation();
        let Some(from) = self.slide_point(event.position) else {
            return;
        };
        let Document::Deck {
            deck,
            current,
            selected,
            ..
        } = &mut self.document
        else {
            return;
        };
        let Some(id) = *selected else {
            return;
        };
        if !deck
            .shape(*current, id)
            .is_some_and(|shape| shape.can_resize)
        {
            return;
        }
        self.commit_slide_text(cx);
        if let Document::Deck { drag, .. } = &mut self.document {
            *drag = Some(ShapeDrag::new(id, DragMode::Resize { left, top }, from));
        }
        cx.notify();
    }

    /// The pointer moving with the button down drags the shape along.
    fn slide_mouse_move(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if !event.dragging() {
            return;
        }
        let Some((x, y)) = self.slide_point(event.position) else {
            return;
        };
        if let Document::Deck {
            drag: Some(drag), ..
        } = &mut self.document
        {
            drag.drag_to(x, y);
            cx.notify();
        }
    }

    /// Letting go writes the drag, as one undoable change.
    fn slide_mouse_up(&mut self, cx: &mut Context<Self>) {
        let Document::Deck {
            deck,
            current,
            drag,
            ..
        } = &mut self.document
        else {
            return;
        };
        let Some(drag) = drag.take() else {
            return;
        };
        let result = deck.drag_shape(*current, &drag);
        self.edited(result, cx);
    }

    /// A window point as a point on the slide on show, in slide px.
    fn slide_point(&self, position: gpui::Point<Pixels>) -> Option<(f64, f64)> {
        let bounds = self.surface.get()?;
        let Document::Deck { scale, .. } = &self.document else {
            return None;
        };
        let to_slide = |value: Pixels| f64::from(value) / f64::from(*scale);
        Some((
            to_slide(position.x - bounds.origin.x),
            to_slide(position.y - bounds.origin.y),
        ))
    }

    /// Paints the current slide at `width` window px, unless it already is.
    fn paint_slide(&mut self, width: f32, scale: f32) {
        let generation = self.generation;
        let Document::Deck {
            deck,
            current,
            slide,
            ..
        } = &mut self.document
        else {
            return;
        };
        let device_width = (width * scale).round() as u32;
        let fresh = matches!(slide, Some((index, w, g, _))
            if *index == *current && *w == device_width && *g == generation);
        if fresh {
            return;
        }
        match deck.render_slide(*current, device_width).and_then(image) {
            Some(painted) => {
                if let Some((_, _, _, old)) =
                    slide.replace((*current, device_width, generation, painted))
                {
                    self.retired.push(old);
                }
            }
            None => {
                self.status = i18n::format("deck.paint_failed", &[("n", &(*current + 1))]).into()
            }
        }
    }

    /// The slide alone on black, filling the window, as a slideshow.
    fn slideshow_body(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui::Div {
        let (area_width, area_height) = editor_area(window, cx);
        let Document::Deck { deck, .. } = &self.document else {
            unreachable!()
        };
        let (slide_width, slide_height) = deck.slide_size();
        let fit = (area_width / slide_width as f32).min(area_height / slide_height as f32);
        let (width, height) = (slide_width as f32 * fit, slide_height as f32 * fit);
        self.paint_slide(width, window.scale_factor());
        let Document::Deck { slide, .. } = &self.document else {
            unreachable!()
        };
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::black())
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _event, _window, cx| view.step_slide(1, cx)),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|view, _event, _window, cx| view.step_slide(-1, cx)),
            )
            .children(
                slide
                    .as_ref()
                    .map(|(_, _, _, image)| img(image.clone()).w(px(width)).h(px(height))),
            )
    }

    fn deck_body(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui::Div {
        let scale = window.scale_factor();
        let editing_cell = self.edited_cell().is_some();
        let editing_merged = match (&self.document, self.edited_cell()) {
            (Document::Deck { deck, current, .. }, Some(target)) => deck
                .cell_span(*current, target)
                .is_some_and(|span| span != (1, 1)),
            _ => false,
        };
        let selected_cells = match &self.document {
            Document::Deck { cells, .. } => cells.map(|cells| cells.rect),
            _ => None,
        };
        let (area_width, area_height) = editor_area(window, cx);
        let notes_height = if self.show_notes { NOTES_HEIGHT } else { 0.0 };
        let Document::Deck {
            deck, thumbnails, ..
        } = &mut self.document
        else {
            unreachable!()
        };
        for (index, thumbnail) in thumbnails.iter_mut().enumerate() {
            if thumbnail.is_none() {
                *thumbnail = deck
                    .render_slide(index, (SLIDE_THUMBNAIL_WIDTH * scale) as u32)
                    .and_then(image);
            }
        }
        let (slide_width, slide_height) = deck.slide_size();
        let zoom = self.zoom.unwrap_or_else(|| {
            deck_fit(deck.slide_size(), (area_width, area_height - notes_height))
        });
        self.shown_zoom = zoom;
        let (width, height) = (slide_width as f32 * zoom, slide_height as f32 * zoom);
        self.paint_slide(width, scale);

        let Document::Deck {
            deck,
            current,
            thumbnails,
            thumbnail_scroll,
            slide,
            selected,
            edit,
            drag,
            scale: shown_scale,
            ..
        } = &mut self.document
        else {
            unreachable!()
        };
        *shown_scale = zoom;
        let current_index = *current;
        let count = deck.slide_count();
        let thumbnail_height = SLIDE_THUMBNAIL_WIDTH * (slide_height / slide_width) as f32;
        let rows: Vec<_> = thumbnails
            .iter()
            .enumerate()
            .map(|(index, thumbnail)| {
                let selected = index == current_index;
                div()
                    .id(("slide-thumbnail", index))
                    .flex()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .cursor_pointer()
                    .on_click(
                        cx.listener(move |view, _event, _window, cx| view.go_to_slide(index, cx)),
                    )
                    .child(
                        div()
                            .w(px(16.0))
                            .text_xs()
                            .text_color(if selected {
                                palette::accent()
                            } else {
                                palette::faint()
                            })
                            .child(format!("{}", index + 1)),
                    )
                    .child(
                        div()
                            .w(px(SLIDE_THUMBNAIL_WIDTH))
                            .h(px(thumbnail_height))
                            .flex_none()
                            .bg(gpui::white())
                            .border_2()
                            .rounded_sm()
                            .overflow_hidden()
                            .border_color(if selected {
                                palette::accent()
                            } else {
                                palette::border()
                            })
                            .hover(|thumbnail| thumbnail.border_color(palette::faint()))
                            .children(thumbnail.clone().map(|image| img(image).size_full())),
                    )
            })
            .collect();
        let slide_image = slide.as_ref().map(|(_, _, _, image)| image.clone());
        let notes = deck
            .notes(current_index)
            .map(|notes| SharedString::from(notes.to_string()));
        let show_notes = self.show_notes;

        // The selection outline and its corner handles, or the text editor,
        // over the slide; a shape being dragged where the drag has taken it.
        let resizable = selected
            .and_then(|id| deck.shape(current_index, id))
            .is_some_and(|shape| shape.can_resize);
        let overlay = selected.and_then(|id| {
            let editing = edit.as_ref().filter(|(target, _)| target.id() == id);
            let rect = match editing {
                Some((target, _)) => deck.text_rect(current_index, *target)?,
                None => {
                    let rect = deck.shape_rect(current_index, id)?;
                    match drag.as_ref().filter(|drag| drag.id == id) {
                        Some(drag) => drag.rect(rect),
                        None => rect,
                    }
                }
            };
            let (x, y, w, h) = (
                rect.x as f32 * zoom,
                rect.y as f32 * zoom,
                rect.width as f32 * zoom,
                rect.height as f32 * zoom,
            );
            Some(match editing {
                Some((_, text)) => div()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .min_w(px(w))
                    .min_h(px(h))
                    .p(px(6.0))
                    .bg(gpui::rgba(0xffffffee))
                    .border_1()
                    .border_color(palette::accent())
                    .text_color(gpui::black())
                    .text_size(px((18.0 * zoom).max(11.0)))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_end()
                            .child(if text.whole {
                                div()
                                    .bg(palette::selected())
                                    .child(text.split().0.to_string())
                            } else {
                                div().child(text.split().0.to_string())
                            })
                            .child(
                                div()
                                    .w(px(1.5))
                                    .h(px((20.0 * zoom).max(12.0)))
                                    .bg(gpui::black()),
                            )
                            .child(text.split().1.to_string()),
                    ),
                None => div()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(w))
                    .h(px(h))
                    .border_1()
                    .border_color(palette::accent())
                    .when(resizable, |outline| {
                        outline.children(
                            [(true, true), (false, true), (true, false), (false, false)]
                                .into_iter()
                                .enumerate()
                                .map(|(index, (is_left, is_top))| {
                                    let at = |near: bool, length: f32| {
                                        px(if near { 0.0 } else { length } - HANDLE_SIZE / 2.0)
                                    };
                                    div()
                                        .id(("resize-handle", index))
                                        .absolute()
                                        .left(at(is_left, w))
                                        .top(at(is_top, h))
                                        .size(px(HANDLE_SIZE))
                                        .rounded_full()
                                        .bg(gpui::white())
                                        .border_1()
                                        .border_color(palette::accent())
                                        .cursor(if is_left == is_top {
                                            gpui::CursorStyle::ResizeUpLeftDownRight
                                        } else {
                                            gpui::CursorStyle::ResizeUpRightDownLeft
                                        })
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |view, event, window, cx| {
                                                view.start_slide_resize(
                                                    is_left, is_top, event, window, cx,
                                                )
                                            }),
                                        )
                                }),
                        )
                    }),
            })
        });

        let store = Rc::clone(&self.surface);
        let entity = cx.entity();
        div()
            .size_full()
            .flex()
            .bg(palette::window())
            // A drag follows the pointer anywhere over the view, and ends
            // wherever the button comes up.
            .on_mouse_move(cx.listener(|view, event, _window, cx| view.slide_mouse_move(event, cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _event, _window, cx| view.slide_mouse_up(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|view, _event, _window, cx| view.slide_mouse_up(cx)),
            )
            .child(
                div()
                    .w(px(DECK_SIDEBAR_WIDTH))
                    .h_full()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .bg(palette::sidebar())
                    .border_r_1()
                    .border_color(palette::border())
                    .child(
                        div()
                            .px_3()
                            .pt_3()
                            .pb_2()
                            .text_xs()
                            .text_color(palette::faint())
                            .child(i18n::format("deck.slides", &[("n", &count)])),
                    )
                    .child(
                        div()
                            .id("slide-thumbnails")
                            .flex_1()
                            .overflow_y_scroll()
                            .track_scroll(thumbnail_scroll)
                            .flex()
                            .flex_col()
                            .gap_1()
                            .pb_3()
                            .children(rows),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(
                        toolbar(i18n::format(
                            "deck.position",
                            &[("current", &(current_index + 1)), ("count", &count)],
                        ))
                        // A table's layout, while one of its cells is typed
                        // into: PowerPoint's Layout tab.
                        .when(editing_cell, |bar| {
                            bar.child(
                                toolbar_button(
                                    "row-below",
                                    "icons/arrow-down-to-line.svg",
                                    i18n::text("deck.row_below"),
                                )
                                .on_click(cx.listener(
                                    |view, _event, _window, cx| {
                                        view.change_slide_table(
                                            |row, _| TableChange::InsertRow(row + 1),
                                            cx,
                                        )
                                    },
                                )),
                            )
                            .child(
                                toolbar_button(
                                    "column-right",
                                    "icons/arrow-right-to-line.svg",
                                    i18n::text("deck.column_right"),
                                )
                                .on_click(cx.listener(
                                    |view, _event, _window, cx| {
                                        view.change_slide_table(
                                            |_, column| TableChange::InsertColumn(column + 1),
                                            cx,
                                        )
                                    },
                                )),
                            )
                            .child(
                                toolbar_button(
                                    "delete-row",
                                    "icons/trash-2.svg",
                                    i18n::text("deck.delete_row"),
                                )
                                .on_click(cx.listener(
                                    |view, _event, _window, cx| {
                                        view.change_slide_table(
                                            |row, _| TableChange::DeleteRow(row),
                                            cx,
                                        )
                                    },
                                )),
                            )
                            .child(
                                toolbar_button(
                                    "delete-column",
                                    "icons/trash-2.svg",
                                    i18n::text("deck.delete_column"),
                                )
                                .on_click(cx.listener(
                                    |view, _event, _window, cx| {
                                        view.change_slide_table(
                                            |_, column| TableChange::DeleteColumn(column),
                                            cx,
                                        )
                                    },
                                )),
                            )
                        })
                        .when(selected_cells.is_some(), |bar| {
                            bar.child(
                                toolbar_button(
                                    "merge-cells",
                                    "icons/merge.svg",
                                    i18n::text("deck.merge_cells"),
                                )
                                .on_click(cx.listener(
                                    |view, _event, _window, cx| view.merge_slide_cells(cx),
                                )),
                            )
                        })
                        .when(editing_merged, |bar| {
                            bar.child(
                                toolbar_button(
                                    "split-cell",
                                    "icons/split.svg",
                                    i18n::text("deck.split_cell"),
                                )
                                .on_click(cx.listener(
                                    |view, _event, _window, cx| view.split_slide_cell(cx),
                                )),
                            )
                        })
                        .child(
                            toolbar_button(
                                "notes",
                                "icons/file-text.svg",
                                i18n::text("deck.notes"),
                            )
                            .when(show_notes, |button| button.bg(palette::selected()))
                            .on_click(cx.listener(
                                |view, _event, _window, cx| {
                                    view.show_notes = !view.show_notes;
                                    cx.notify();
                                },
                            )),
                        )
                        .child(
                            toolbar_button(
                                "from-current",
                                "icons/play.svg",
                                i18n::text("deck.from_current"),
                            )
                            .on_click(cx.listener(
                                |view, _event, _window, cx| view.start_slideshow(None, cx),
                            )),
                        )
                        .child(
                            div()
                                .id("slideshow")
                                .flex()
                                .items_center()
                                .gap_1p5()
                                .ml_1()
                                .px_3()
                                .py_1()
                                .rounded_md()
                                .text_xs()
                                .text_color(gpui::white())
                                .bg(palette::accent())
                                .cursor_pointer()
                                .hover(|button| button.opacity(0.9))
                                .child(
                                    svg()
                                        .path("icons/play.svg")
                                        .size_3()
                                        .text_color(gpui::white()),
                                )
                                .child(i18n::text("deck.slideshow"))
                                .on_click(cx.listener(|view, _event, _window, cx| {
                                    view.start_slideshow(Some(0), cx)
                                })),
                        ),
                    )
                    .child(
                        // Scrolls once a chosen zoom makes the slide bigger
                        // than the space. A click off the slide clears the
                        // selection.
                        div()
                            .id("slide-canvas")
                            .flex_1()
                            .min_h_0()
                            .overflow_scroll()
                            .bg(palette::canvas())
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|view, _event, window, cx| {
                                    window.focus(&view.focus, cx);
                                    view.commit_slide_text(cx);
                                    view.select_shape(None);
                                    cx.notify();
                                }),
                            )
                            .child(
                                div()
                                    .min_w_full()
                                    .min_h_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .p(px(SLIDE_MARGIN))
                                    .child(
                                        div()
                                            .relative()
                                            .flex_none()
                                            .w(px(width))
                                            .h(px(height))
                                            .bg(gpui::white())
                                            .shadow_lg()
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(Self::slide_mouse_down),
                                            )
                                            .children(
                                                slide_image.map(|image| {
                                                    img(image).w(px(width)).h(px(height))
                                                }),
                                            )
                                            .children(overlay)
                                            // The selected cells, shaded as
                                            // PowerPoint shades them.
                                            .children(selected_cells.map(|rect| {
                                                div()
                                                    .absolute()
                                                    .left(px(rect.x as f32 * zoom))
                                                    .top(px(rect.y as f32 * zoom))
                                                    .w(px(rect.width as f32 * zoom))
                                                    .h(px(rect.height as f32 * zoom))
                                                    .bg(gpui::rgba(0x3b82f633))
                                                    .border_1()
                                                    .border_color(palette::accent())
                                            }))
                                            .child(
                                                canvas(
                                                    move |bounds, _window, _cx| {
                                                        store.set(Some(bounds));
                                                        bounds
                                                    },
                                                    move |_bounds, slide, window, cx| {
                                                        let focus = entity.read(cx).focus.clone();
                                                        window.handle_input(
                                                            &focus,
                                                            ElementInputHandler::new(
                                                                slide,
                                                                entity.clone(),
                                                            ),
                                                            cx,
                                                        );
                                                    },
                                                )
                                                .absolute()
                                                .top_0()
                                                .left_0()
                                                .size_full(),
                                            ),
                                    ),
                            ),
                    )
                    .when(show_notes, |column| {
                        column.child(
                            div()
                                .id("slide-notes")
                                .h(px(NOTES_HEIGHT))
                                .flex_none()
                                .overflow_y_scroll()
                                .border_t_1()
                                .border_color(palette::border())
                                .bg(palette::panel())
                                .px_4()
                                .py_2()
                                .text_sm()
                                .child(match notes {
                                    Some(notes) => div().child(notes),
                                    None => div()
                                        .text_color(palette::faint())
                                        .child(i18n::text("deck.no_notes")),
                                }),
                        )
                    }),
            )
    }

    // ---- pages ------------------------------------------------------------

    /// Re-images what the document repainted since it was last asked: the
    /// tiles of each page its drawing touched, read from the page itself, so
    /// nothing drawn is missed and nothing undrawn is read.
    fn refresh_repainted(&mut self) {
        let Document::Pages {
            document, pages, ..
        } = &mut self.document
        else {
            return;
        };
        // An edit from this page on can have moved what any later page
        // shows: a thumbnail kept of one is out of date.
        let changed_from = document.take_repainted_from();
        document.take_repainted_pages();
        let count = document.page_count();
        for index in 0..count {
            let Some(painter) = document.page(index) else {
                // Not kept as pixels: its tiles go, a thumbnail stays.
                match pages.get_mut(index) {
                    Some(page) => {
                        page.release(&mut self.retired);
                        if changed_from.is_some_and(|from| index >= from) {
                            page.forget_thumbnail(&mut self.retired);
                        }
                    }
                    None => pages.push(PageImage::blank()),
                }
                continue;
            };
            if let Some(page) = pages.get_mut(index) {
                page.refresh(&painter, &mut self.retired);
            } else if let Some(page) = PageImage::new(&painter) {
                pages.push(page);
            }
        }
        // A page the document no longer has goes.
        while pages.len() > count {
            if let Some(mut old) = pages.pop() {
                self.retired.append(&mut old.tiles);
                self.retired.extend(old.thumbnail.take());
            }
        }
    }

    /// Runs an edit on the Word document and shows its result.
    fn word_edit(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut jade_web::editing::EditableDocument) -> Result<(), String>,
    ) {
        let Document::Pages { document, .. } = &mut self.document else {
            return;
        };
        let result = edit(document);
        self.refresh_repainted();
        self.reveal_caret();
        self.word_result(result, cx);
    }

    /// Scrolls the pages so the caret — a selection's moving end — is in
    /// view, a little clear of the edges, as typing or an arrow key moves
    /// it: left alone, Down at the bottom of the view or typing past it
    /// carried the caret off screen. Worked out from the layout the pages
    /// are stacked in, so it holds for a page not yet painted.
    fn reveal_caret(&mut self) {
        const MARGIN: f32 = 24.0;
        let zoom = self.zoom.unwrap_or(1.0);
        let Document::Pages {
            document,
            page_size,
            scroll,
            reveal_pending,
            ..
        } = &mut self.document
        else {
            return;
        };
        let pitch = page_size.1 * zoom + PAGE_GAP;
        let Some(caret) = document.focus_rect() else {
            // A caret on a page not kept as pixels, before the ones that
            // are, has no lines to place it by: to the top of its page,
            // and once that page is painted, to the caret itself.
            let page = document
                .caret()
                .and_then(|(paragraph, _)| document.paragraph_start_page(paragraph));
            if let Some(page) = page {
                let target = (PAGES_PADDING + page as f32 * pitch - MARGIN)
                    .clamp(0.0, f32::from(scroll.max_offset().y).max(0.0));
                scroll.set_offset(point(scroll.offset().x, px(-target)));
                *reveal_pending = true;
            }
            return;
        };
        let (page, y) = document.page_and_offset(caret.document_y);
        let top = PAGES_PADDING + page as f32 * pitch + y as f32 * zoom;
        let bottom = top + caret.height as f32 * zoom;
        let view = f32::from(scroll.bounds().size.height);
        if view <= MARGIN * 2.0 {
            return;
        }
        let offset = scroll.offset();
        let shown = -f32::from(offset.y);
        let target = if top < shown + MARGIN {
            top - MARGIN
        } else if bottom > shown + view - MARGIN {
            bottom + MARGIN - view
        } else {
            return;
        };
        let target = target.clamp(0.0, f32::from(scroll.max_offset().y).max(0.0));
        scroll.set_offset(point(offset.x, px(-target)));
        // The current page, for the status bar, which is drawn before the
        // pages are; and the pages now on screen, painted.
        self.follow_scroll(gpui::Size::default());
    }

    /// Moves the caret or the selection without changing the text: no
    /// status, nothing unsaved.
    fn word_move(
        &mut self,
        cx: &mut Context<Self>,
        motion: impl FnOnce(&mut jade_web::editing::EditableDocument) -> Result<(), String>,
    ) {
        let Document::Pages { document, .. } = &mut self.document else {
            return;
        };
        if let Err(error) = motion(document) {
            self.status = error.into();
        }
        self.refresh_repainted();
        cx.notify();
    }

    /// The selected text onto the clipboard; with `cut`, out of the
    /// document too.
    fn word_copy(&mut self, cut: bool, cx: &mut Context<Self>) {
        let Document::Pages { document, .. } = &self.document else {
            return;
        };
        let Some(text) = document.selected_text() else {
            return;
        };
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        if cut {
            self.word_edit(cx, |document| document.delete(true));
        }
    }

    fn word_result(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        self.generation += 1;
        self.status = match result {
            Ok(()) => SharedString::default(),
            Err(error) => error.into(),
        };
        cx.notify();
    }

    /// A point on page `index` in the document's coordinates.
    fn page_point(&self, index: usize, position: gpui::Point<Pixels>) -> Option<(f64, f64)> {
        let Document::Pages {
            document,
            page_bounds,
            ..
        } = &self.document
        else {
            return None;
        };
        let bounds = page_bounds.borrow().get(index).copied().flatten()?;
        let zoom = self.shown_zoom.max(0.01);
        let x = f64::from((position.x - bounds.origin.x) / zoom);
        let y = f64::from((position.y - bounds.origin.y) / zoom);
        Some((x, document.document_y(index, y)))
    }

    /// The text position under a point on page `index`.
    fn page_position(&self, index: usize, position: gpui::Point<Pixels>) -> Option<(usize, usize)> {
        let (x, y) = self.page_point(index, position)?;
        let Document::Pages { document, .. } = &self.document else {
            return None;
        };
        document.position_at(x, y)
    }

    /// A press on page `index`: the caret goes to the text under it, or with
    /// Shift the selection reaches there.
    fn page_mouse_down(
        &mut self,
        index: usize,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        if let Some(bar) = self.find_bar_mut() {
            bar.active = false;
        }
        let (Some((x, y)), Some(target)) = (
            self.page_point(index, event.position),
            self.page_position(index, event.position),
        ) else {
            return;
        };
        let extend = event.modifiers.shift;
        let clicks = event.click_count;
        if let Document::Pages { dragging, .. } = &mut self.document {
            // A drag selects from a single press; a double or triple one
            // selects a word or a paragraph and is done.
            *dragging = extend || clicks <= 1;
        }
        self.word_move(cx, |document| {
            // Mid-composition the text under the pointer is the IME's.
            if document.composition().is_some() {
                return Ok(());
            }
            match (extend, document.selection(), document.caret()) {
                (true, Some((anchor, _)), _) | (true, None, Some(anchor)) => {
                    document.select(anchor, target)
                }
                _ if clicks == 2 => document.select_word(target),
                _ if clicks >= 3 => document.select_paragraph(target.0),
                // On the line clicked, at a wrap too.
                _ => {
                    document.place_caret_at(x, y);
                    Ok(())
                }
            }
        });
    }

    /// The text position nearest a window point while dragging from
    /// `anchor`: over the page under the point, or else the nearest page, the
    /// point pulled onto it — so the gaps between pages and the space round
    /// them still extend the selection, as in Word. A drag that starts in the
    /// body stays in it rather than jumping into a header or a footer.
    fn drag_position(
        &self,
        anchor: (usize, usize),
        position: gpui::Point<Pixels>,
    ) -> Option<(usize, usize)> {
        let Document::Pages {
            document,
            page_bounds,
            pages,
            ..
        } = &self.document
        else {
            return None;
        };
        let (index, bounds) = {
            let store = page_bounds.borrow();
            let distance = |bounds: &Bounds<Pixels>| {
                let y = f32::from(position.y);
                let (top, bottom) = (f32::from(bounds.top()), f32::from(bounds.bottom()));
                (top - y).max(y - bottom).max(0.0)
            };
            store
                .iter()
                .take(pages.len())
                .enumerate()
                .filter_map(|(index, bounds)| Some((index, (*bounds)?)))
                .min_by(|(_, a), (_, b)| distance(a).total_cmp(&distance(b)))?
        };
        let pull = |at: Pixels, from: Pixels, to: Pixels| {
            px(f32::from(at).clamp(f32::from(from), f32::from(to) - 1.0))
        };
        let inside = point(
            pull(position.x, bounds.left(), bounds.right()),
            pull(position.y, bounds.top(), bounds.bottom()),
        );
        let (x, y) = self.page_point(index, inside)?;
        if document.is_body_paragraph(anchor.0) {
            document.body_position_at(x, y)
        } else {
            document.position_at(x, y)
        }
    }

    /// A drag from a press on a page selects from where it was pressed. Seen
    /// through the whole window, not only over a page, and past the top or
    /// the bottom of the view the pages scroll to meet the pointer.
    fn page_drag(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Document::Pages {
            document,
            dragging,
            scroll,
            ..
        } = &mut self.document
        else {
            return;
        };
        if !*dragging {
            return;
        }
        // A release outside the window never arrived.
        if event.pressed_button != Some(MouseButton::Left) {
            *dragging = false;
            return;
        }
        if document.composition().is_some() {
            return;
        }
        let (anchor, focus) = match (document.selection(), document.caret()) {
            (Some(selection), _) => selection,
            (None, Some(caret)) => (caret, caret),
            _ => return,
        };
        let view = scroll.bounds();
        let overshoot = if event.position.y < view.top() {
            event.position.y - view.top()
        } else if event.position.y > view.bottom() {
            event.position.y - view.bottom()
        } else {
            px(0.0)
        };
        if overshoot != px(0.0) {
            let offset = scroll.offset();
            let y = f32::from(offset.y - overshoot).clamp(-f32::from(scroll.max_offset().y), 0.0);
            scroll.set_offset(point(offset.x, px(y)));
            cx.notify();
        }
        let Some(target) = self.drag_position(anchor, event.position) else {
            return;
        };
        if focus != target {
            self.word_move(cx, |document| document.select(anchor, target));
        }
    }

    /// A release ends a drag; the ribbon, left alone through it, catches up.
    fn page_release(&mut self, cx: &mut Context<Self>) {
        if let Document::Pages { dragging, .. } = &mut self.document {
            if std::mem::take(dragging) {
                cx.notify();
            }
        }
    }

    /// The long moves of ⌘ (a Mac) and Ctrl (elsewhere) with an arrow, Home
    /// or End: to the document's start or end, a line's ends, or a word
    /// along — Shift extending the selection, as the plain arrows do.
    fn pages_jump(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        use jade_web::editing::Motion;
        let modifiers = event.keystroke.modifiers;
        let motion = match (event.keystroke.key.as_str(), modifiers.platform) {
            ("up" | "home", _) => Motion::DocumentStart,
            ("down" | "end", _) => Motion::DocumentEnd,
            ("left", true) => Motion::LineStart,
            ("right", true) => Motion::LineEnd,
            ("left", false) => Motion::WordLeft,
            ("right", false) => Motion::WordRight,
            _ => return false,
        };
        self.pages_motion(motion, modifiers.shift, cx)
    }

    /// Moves the caret, or with `extend` the selection's moving end, and
    /// keeps it in view. Mid-composition the keys are the IME's.
    fn pages_motion(
        &mut self,
        motion: jade_web::editing::Motion,
        extend: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        let Document::Pages { document, .. } = &self.document else {
            return false;
        };
        if document.composition().is_some() || document.caret().is_none() {
            return false;
        }
        self.word_move(cx, |document| document.move_caret(motion, extend));
        self.reveal_caret();
        true
    }

    /// Keys the Word view answers: deleting, splitting, and moving the caret
    /// — Shift extending the selection. Mid-composition they are the IME's.
    fn pages_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        use jade_web::editing::Motion;
        let key = event.keystroke.key.as_str();
        let extend = event.keystroke.modifiers.shift;
        let alt = event.keystroke.modifiers.alt;
        let Document::Pages { document, .. } = &mut self.document else {
            return false;
        };
        if document.composition().is_some() || document.caret().is_none() {
            return false;
        }
        let motion = match key {
            // ⌥⌫ and ⌥⌦ delete a word, as on a Mac: the word is selected,
            // then deleted, in one edit.
            "backspace" | "delete" if alt && document.selection().is_none() => {
                let forwards = key == "delete";
                self.word_edit(cx, |document| {
                    let motion = if forwards {
                        Motion::WordRight
                    } else {
                        Motion::WordLeft
                    };
                    document.move_caret(motion, true)?;
                    document.delete(!forwards)
                });
                return true;
            }
            "left" if alt => Motion::WordLeft,
            "right" if alt => Motion::WordRight,
            "backspace" => {
                self.word_edit(cx, |document| document.delete(true));
                return true;
            }
            "delete" => {
                self.word_edit(cx, |document| document.delete(false));
                return true;
            }
            "enter" => {
                self.word_edit(cx, |document| document.insert_paragraph_break());
                return true;
            }
            // In a table, to the next or the previous cell; elsewhere a tab
            // typed, as in Word.
            "tab" => {
                let backwards = extend;
                self.word_edit(cx, |document| {
                    if document.step_cell(!backwards)? || backwards {
                        Ok(())
                    } else {
                        document.insert_text("\t")
                    }
                });
                return true;
            }
            "left" => Motion::Left,
            "right" => Motion::Right,
            "up" => Motion::Up,
            "down" => Motion::Down,
            "home" => Motion::LineStart,
            "end" => Motion::LineEnd,
            _ => return false,
        };
        self.word_move(cx, |document| document.move_caret(motion, extend));
        self.reveal_caret();
        true
    }

    /// Runs a ribbon control against the Word document: a formatting command
    /// keeps the selection, so the next one acts on the same text; a table
    /// command that adds or removes cells puts the caret where it left it.
    pub(crate) fn ribbon_action(
        &mut self,
        action: Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Document::Pages {
            ribbon, document, ..
        } = &mut self.document
        else {
            return;
        };
        if action.is_chrome() {
            match action {
                Action::Tab(tab) => {
                    ribbon.tab = tab;
                    ribbon.popup = None;
                }
                Action::Popup(popup) => ribbon.toggle(popup),
                Action::ClosePopup => ribbon.popup = None,
                Action::Grid(columns, rows) => ribbon.grid = (columns, rows),
                _ => {}
            }
            return cx.notify();
        }
        ribbon.popup = None;
        window.focus(&self.focus, cx);
        if let Action::EditStory(story) = action {
            return self.open_story(story, cx);
        }
        if let Action::Style(style_id) = action {
            let result = document.set_paragraph_style(style_id.as_deref());
            ribbon.read_styles(document);
            self.refresh_repainted();
            return self.word_result(result, cx);
        }
        self.word_edit(cx, move |document| match action {
            Action::Toggle(format) => document.toggle_format(format),
            Action::ClearFormatting => document.clear_formatting(),
            Action::FontFamily(family) => document.set_font_family(family),
            Action::FontSize(points) => document.set_font_size(points),
            Action::StepFontSize(grow) => document.step_font_size(grow),
            Action::FontColor(color) => document.set_font_color(&color),
            Action::Highlight(color) => document.set_highlight(color),
            Action::List(bullets) => document.toggle_list(bullets),
            Action::Indent(increase) => document.step_indent(increase),
            Action::Align(alignment) => document.set_alignment(alignment),
            Action::LineSpacing(multiple) => document.set_line_spacing(multiple),
            Action::Shading(color) => document.set_paragraph_shading(color.as_deref()),
            Action::ParagraphBorders(borders) => document.set_paragraph_borders(borders),
            Action::InsertTable(rows, columns) => {
                let caret = document.insert_table(rows, columns)?;
                document.place_caret(caret.0, caret.1);
                Ok(())
            }
            Action::Table(command) if command.is_structural() => {
                let caret = document.table_command(command)?;
                document.place_caret(caret.0, caret.1);
                Ok(())
            }
            Action::Table(command) => document.table_command(command).map(|_| ()),
            Action::Undo => document.undo().map(|_| ()),
            Action::Redo => document.redo().map(|_| ()),
            Action::Page(command) => {
                jade_web::word::page::WordPage::page_command(document, command, !i18n::is_english())
            }
            Action::EditStory(_)
            | Action::Style(_)
            | Action::Tab(_)
            | Action::Popup(_)
            | Action::ClosePopup
            | Action::Grid(..) => Ok(()),
        });
    }

    /// The ribbon over the pages, with what it shows as current read again
    /// only when the caret, the selection or the document changed.
    fn word_ribbon(
        &mut self,
        viewport: gpui::Size<Pixels>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let generation = self.generation;
        let Document::Pages {
            document,
            ribbon,
            format,
            pages,
            current,
            dragging,
            ..
        } = &mut self.document
        else {
            unreachable!()
        };
        let key = (document.caret(), document.selection(), generation);
        let state = match format {
            // Not while a drag is still moving the selection: reading it runs
            // queries over the whole selection, on every move.
            Some((cached, state)) if *cached == key || *dragging => state.clone(),
            _ => {
                let state = document.format_state();
                *format = Some((key, state.clone()));
                state
            }
        };
        let position = i18n::format(
            "doc.position",
            &[("current", &(*current + 1)), ("count", &pages.len())],
        );
        // Read only for the tab that shows it: a query of the section.
        let page = (ribbon.tab == ribbon::Tab::Layout).then(|| {
            use jade_web::word::page::WordPage;
            let japanese = !i18n::is_english();
            ribbon::PageState {
                setup: document.page_setup(),
                preset: document.margin_preset(japanese),
                japanese,
            }
        });
        ribbon::ribbon(
            ribbon,
            &state,
            page.as_ref(),
            position,
            document.is_dirty(),
            viewport,
            cx,
        )
    }

    /// Keeps the pages on screen, one either side and as many more round
    /// them as make [`RESIDENT_PAGES`], as pixels, and lets the rest go: a
    /// long document's pages at 2x ran the browser out of memory when all of
    /// them were kept.
    fn follow_scroll(&mut self, viewport: gpui::Size<Pixels>) {
        let zoom = self.zoom.unwrap_or(1.0);
        let Document::Pages {
            document,
            page_size,
            scroll,
            current,
            ..
        } = &mut self.document
        else {
            return;
        };
        let pitch = (page_size.1 * zoom + PAGE_GAP).max(1.0);
        let top = (-f32::from(scroll.offset().y) - PAGES_PADDING).max(0.0);
        // The page at the top third of the view is the current one: worked
        // out here, before the ribbon shows it, so a jump shows its page on
        // the frame it lands.
        *current =
            (((top + pitch / 3.0) / pitch) as usize).min(document.page_count().saturating_sub(1));
        let height = match f32::from(scroll.bounds().size.height) {
            shown if shown > 0.0 => shown,
            _ => f32::from(viewport.height),
        };
        let count = document.page_count();
        let first = ((top / pitch) as usize).saturating_sub(1);
        let last = ((top + height) / pitch) as usize + 2;
        // Widened evenly to the budget, within the document.
        let spare = RESIDENT_PAGES.saturating_sub(last - first);
        let start = first.saturating_sub(spare / 2);
        let end = (last + spare - (first - start)).min(count.max(last));
        let start = start.min(end.saturating_sub(RESIDENT_PAGES.max(last - first)));
        let resident = start..end;
        match document.set_resident_pages(resident) {
            Ok(true) => {
                self.refresh_repainted();
                if let Document::Pages { reveal_pending, .. } = &mut self.document {
                    if std::mem::take(reveal_pending) {
                        self.reveal_caret();
                    }
                }
            }
            Ok(false) => {}
            Err(error) => self.status = error.into(),
        }
    }

    fn pages_body(&mut self, viewport: gpui::Size<Pixels>, cx: &mut Context<Self>) -> gpui::Div {
        let zoom = self.zoom.unwrap_or(1.0);
        self.shown_zoom = zoom;
        self.follow_scroll(viewport);
        let word_ribbon = self.word_ribbon(viewport, cx);
        let story_bar = self.story_bar_element();
        let find_bar = self.find_bar_element(cx);
        let Document::Pages {
            document,
            pages,
            page_size,
            scroll,
            current,
            page_bounds,
            highlight,
            ..
        } = &mut self.document
        else {
            unreachable!()
        };
        let (width, height) = (page_size.0 * zoom, page_size.1 * zoom);
        // The selection's highlight, read again only when the selection or
        // the text changed.
        let key = (document.caret(), document.selection(), self.generation);
        if highlight.as_ref().is_none_or(|(cached, _)| *cached != key) {
            *highlight = Some((key, document.selection_rects()));
        }
        let highlight = highlight
            .as_ref()
            .map(|(_, rects)| rects.as_slice())
            .unwrap_or(&[]);
        // The caret, as a page and a rectangle on it.
        let caret = document.caret_rect().map(|caret| {
            let (page, y) = document.page_and_offset(caret.document_y);
            (
                page,
                caret.x as f32 * zoom,
                y as f32 * zoom,
                caret.height as f32 * zoom,
            )
        });
        page_bounds.borrow_mut().resize(pages.len(), None);
        let bounds_store = Rc::clone(page_bounds);
        let current_index = *current;
        let count = pages.len();
        let thumbnail_height = PAGE_THUMBNAIL_WIDTH * page_size.1 / page_size.0.max(1.0);
        let rows: Vec<_> = pages
            .iter()
            .enumerate()
            .map(|(index, page)| {
                let selected = index == current_index;
                div()
                    .id(("page", index))
                    .flex()
                    .gap_2()
                    .px_1()
                    .py_1()
                    .rounded_md()
                    .hover(|row| row.bg(palette::hovered()))
                    .cursor_pointer()
                    .child(
                        div()
                            .w(px(16.0))
                            .text_xs()
                            .text_color(if selected {
                                palette::accent()
                            } else {
                                palette::faint()
                            })
                            .child(format!("{}", index + 1)),
                    )
                    .child(
                        div()
                            .w(px(PAGE_THUMBNAIL_WIDTH))
                            .h(px(thumbnail_height))
                            .flex_none()
                            .overflow_hidden()
                            .rounded_sm()
                            .bg(palette::paper())
                            .border_2()
                            .border_color(if selected {
                                palette::accent()
                            } else {
                                palette::border()
                            })
                            .child(page.element()),
                    )
                    .on_click(cx.listener(move |view, _event, _window, cx| {
                        if let Document::Pages {
                            scroll, current, ..
                        } = &mut view.document
                        {
                            *current = index;
                            scroll.scroll_to_item(index);
                        }
                        cx.notify();
                    }))
            })
            .collect();
        let entity = cx.entity();
        let surfaces: Vec<_> = pages
            .iter()
            .enumerate()
            .map(|(index, page)| {
                let store = Rc::clone(&bounds_store);
                let entity = entity.clone();
                div()
                    .relative()
                    .flex_none()
                    .w(px(width))
                    .h(px(height))
                    .bg(palette::paper())
                    .shadow_lg()
                    .cursor_text()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, event, window, cx| {
                            view.page_mouse_down(index, event, window, cx)
                        }),
                    )
                    .child(div().w(px(width)).h(px(height)).child(page.element()))
                    // The selection, over the picture, in the renderer's own
                    // fill and edges.
                    .children(
                        highlight
                            .iter()
                            .filter(|rect| rect.page == index)
                            .map(|rect| {
                                div()
                                    .absolute()
                                    .left(px(rect.x as f32 * zoom))
                                    .top(px(rect.y as f32 * zoom))
                                    .w(px(rect.width as f32 * zoom))
                                    .h(px(rect.height as f32 * zoom))
                                    .bg(rgba(0x584f_e833))
                                    .border_l_1()
                                    .border_r_1()
                                    .border_color(rgba(0x584f_e8c7))
                            }),
                    )
                    .children(caret.filter(|(page, ..)| *page == index).map(
                        |(_, x, y, caret_height)| {
                            div()
                                .absolute()
                                .left(px(x))
                                .top(px(y))
                                .w(px(1.5))
                                .h(px(caret_height.max(8.0)))
                                .bg(palette::ink())
                        },
                    ))
                    // Records where the page is; registers the input handler
                    // so typing — an IME's included — reaches the document.
                    .child(
                        canvas(
                            move |bounds, _window, _cx| {
                                if let Some(slot) = store.borrow_mut().get_mut(index) {
                                    *slot = Some(bounds);
                                }
                                bounds
                            },
                            move |_bounds, page, window, cx| {
                                if index == 0 {
                                    let focus = entity.read(cx).focus.clone();
                                    window.handle_input(
                                        &focus,
                                        ElementInputHandler::new(page, entity.clone()),
                                        cx,
                                    );
                                    // A drag is followed wherever the pointer
                                    // goes, not only over a page.
                                    let dragged = entity.clone();
                                    window.on_mouse_event(
                                        move |event: &MouseMoveEvent, phase, _window, cx| {
                                            if phase == DispatchPhase::Bubble {
                                                dragged.update(cx, |view, cx| {
                                                    view.page_drag(event, cx)
                                                });
                                            }
                                        },
                                    );
                                    let released = entity.clone();
                                    window.on_mouse_event(
                                        move |event: &MouseUpEvent, phase, _window, cx| {
                                            if phase == DispatchPhase::Bubble
                                                && event.button == MouseButton::Left
                                            {
                                                released
                                                    .update(cx, |view, cx| view.page_release(cx));
                                            }
                                        },
                                    );
                                }
                            },
                        )
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full(),
                    )
            })
            .collect();
        div()
            .size_full()
            .flex()
            .bg(palette::window())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .w(px(PAGES_SIDEBAR_WIDTH))
                    .h_full()
                    .bg(palette::sidebar())
                    .border_r_1()
                    .border_color(palette::border())
                    .child(
                        div()
                            .px_3()
                            .pt_3()
                            .pb_2()
                            .text_xs()
                            .text_color(palette::faint())
                            .child(i18n::format("doc.pages", &[("n", &count)])),
                    )
                    .child(
                        div()
                            .id("page-list")
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .px_2()
                            .flex_1()
                            .overflow_y_scroll()
                            .children(rows),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(word_ribbon)
                    .children(story_bar)
                    .children(find_bar)
                    .child(
                        div()
                            .id("pages")
                            .flex_1()
                            .min_h_0()
                            .overflow_scroll()
                            .track_scroll(scroll)
                            .bg(palette::canvas())
                            .on_scroll_wheel(cx.listener(|_view, _event, _window, cx| cx.notify()))
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(PAGE_GAP))
                            .p(px(PAGES_PADDING))
                            .children(surfaces),
                    ),
            )
    }

    fn handle_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
        let key = event.keystroke.key.as_str();
        // ⌘V is left to the browser, whose paste event brings the clipboard
        // here. A browser that kept the copy off the clipboard (an embedding
        // page without clipboard access) sends no paste event for an empty
        // one: then a sheet pastes its own copy, as its menu's Paste does.
        if (modifiers.platform || modifiers.control)
            && key == "v"
            && matches!(&self.document, Document::Workbook(sheet) if sheet.typed().is_none())
        {
            self.paste_arrived = false;
            let view = cx.entity().downgrade();
            window.on_next_frame(move |_window, cx| {
                view.update(cx, |view, cx| {
                    if !view.paste_arrived {
                        view.sheet_paste(None, cx);
                    }
                })
                .ok();
            });
        }
        // The find bar over a Word document takes the keys it has.
        if self.story_editing() && self.story_key(event, cx) {
            cx.stop_propagation();
            return;
        }
        if self.finding() && self.find_key(event, cx) {
            cx.stop_propagation();
            return;
        }
        // A sheet takes its own keys first: Excel's shortcuts, ⌘-arrows,
        // and the keys of a cell being typed into.
        if matches!(self.document, Document::Workbook(_))
            && !self.slideshow
            && self.sheet_key(event, cx)
        {
            cx.stop_propagation();
            return;
        }
        // ⌘ on a Mac, Ctrl elsewhere.
        if modifiers.platform || modifiers.control {
            let pages = matches!(self.document, Document::Pages { .. });
            if pages && self.pages_jump(event, cx) {
                cx.stop_propagation();
                return;
            }
            match key {
                "z" => self.step_history(!modifiers.shift, cx),
                "y" => self.step_history(false, cx),
                "s" => self.save(cx),
                "a" if pages => self.word_move(cx, |document| document.select_all()),
                // Find, and Word for Mac's Find and Replace.
                "f" if pages => self.open_find(false, cx),
                "h" if pages && modifiers.shift => self.open_find(true, cx),
                "c" if pages => self.word_copy(false, cx),
                "x" if pages => self.word_copy(true, cx),
                // Word's own shortcuts for the ribbon's three toggles.
                "b" | "i" | "u" if pages => {
                    let format = match key {
                        "b" => CharacterFormat::Bold,
                        "i" => CharacterFormat::Italic,
                        _ => CharacterFormat::Underline,
                    };
                    self.ribbon_action(Action::Toggle(format), window, cx)
                }
                // ⌘V is left to the browser, whose paste event brings the
                // clipboard to the input handler.
                _ => return,
            }
            cx.stop_propagation();
            return;
        }
        let handled = match self.document {
            Document::Workbook(_) => false,
            Document::Deck { .. } => self.deck_key(event, cx),
            Document::Pages { .. } => self.pages_key(event, cx),
        };
        if handled {
            cx.stop_propagation();
        }
    }

    /// The text being typed into, starting an edit when typing begins one:
    /// into the selected cell (replacing it, as in Excel) or shape.
    fn typing_target(&mut self, cx: &mut Context<Self>) -> Option<&mut TextEdit> {
        if self.slideshow {
            return None;
        }
        match &self.document {
            // Typing into a selected cell replaces it, as in Excel.
            Document::Workbook(sheet) if sheet.typed().is_none() => {
                self.begin_cell_edit(Some(String::new()), false, cx)
            }
            Document::Deck {
                edit: None,
                selected: Some(_),
                ..
            } => self.begin_slide_text(None, cx),
            _ => {}
        }
        match &mut self.document {
            Document::Workbook(sheet) => sheet.typing(),
            Document::Deck { edit, .. } => edit.as_mut().map(|(_, text)| text),
            Document::Pages {
                story: Some(bar), ..
            } => Some(&mut bar.text),
            Document::Pages { find, .. } => find
                .as_deref_mut()
                .filter(|bar| bar.active)
                .map(|bar| bar.field_mut()),
        }
    }

    fn current_text(&self) -> Option<&TextEdit> {
        match &self.document {
            Document::Workbook(sheet) => sheet.typed(),
            Document::Deck { edit, .. } => edit.as_ref().map(|(_, text)| text),
            Document::Pages {
                story: Some(bar), ..
            } => Some(&bar.text),
            Document::Pages { find, .. } => find
                .as_deref()
                .filter(|bar| bar.active)
                .map(|bar| bar.field()),
        }
    }
}

/// The zoom that fits a slide of `slide` px in the deck view's `area`
/// (notes excluded), beside the slide list and under the toolbar.
fn deck_fit(slide: (f64, f64), area: (f32, f32)) -> f32 {
    let available = (
        (area.0 - DECK_SIDEBAR_WIDTH - SLIDE_MARGIN * 2.0).max(40.0),
        (area.1 - TOOLBAR_HEIGHT - SLIDE_MARGIN * 2.0).max(40.0),
    );
    (available.0 / slide.0.max(1.0) as f32).min(available.1 / slide.1.max(1.0) as f32)
}

/// The strip over the slide or the pages: where you are on the left, the
/// view's commands on the right.
fn toolbar(position: String) -> gpui::Div {
    div()
        .h(px(TOOLBAR_HEIGHT))
        .flex_none()
        .flex()
        .items_center()
        .gap_1()
        .px_3()
        .bg(palette::panel())
        .border_b_1()
        .border_color(palette::border())
        .child(
            div()
                .px_1()
                .text_xs()
                .text_color(palette::dim())
                .child(position),
        )
        .child(div().flex_1())
}

fn toolbar_button(
    id: &'static str,
    icon: &'static str,
    label: &'static str,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_1p5()
        .px_2p5()
        .py_1()
        .rounded_md()
        .text_xs()
        .text_color(palette::dim())
        .cursor_pointer()
        .hover(|button| button.bg(palette::hovered()).text_color(palette::text()))
        .child(svg().path(icon).size_3p5().text_color(palette::dim()))
        .child(label)
}

impl DocView {
    /// The Word document's text around the caret — its paragraph — and
    /// where in it the caret and the composition are, for the input
    /// handler.
    fn word_input(&self) -> Option<(String, usize, Option<Range<usize>>)> {
        // While a bar has the keyboard, typing is the bar's.
        if self.finding() || self.story_editing() {
            return None;
        }
        let Document::Pages { document, .. } = &self.document else {
            return None;
        };
        let (paragraph, offset) = document.caret()?;
        let text = document.paragraph_text(paragraph)?;
        let marked = document
            .composition()
            .filter(|(composed, ..)| *composed == paragraph)
            .map(|(_, start, end)| start..end);
        Some((text, offset, marked))
    }
}

impl EntityInputHandler for DocView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        if let Some((text, _, _)) = self.word_input() {
            let start = utf16_to_byte(&text, range.start);
            let end = utf16_to_byte(&text, range.end).max(start);
            *adjusted = Some(byte_to_utf16(&text, start)..byte_to_utf16(&text, end));
            return Some(text.get(start..end)?.to_string());
        }
        let text = self
            .current_text()
            .map(|edit| edit.text.as_str())
            .unwrap_or("");
        let start = utf16_to_byte(text, range.start);
        let end = utf16_to_byte(text, range.end).max(start);
        *adjusted = Some(byte_to_utf16(text, start)..byte_to_utf16(text, end));
        Some(text.get(start..end)?.to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        if let Some((text, offset, _)) = self.word_input() {
            let caret = byte_to_utf16(&text, offset);
            return Some(UTF16Selection {
                range: caret..caret,
                reversed: false,
            });
        }
        let (text, caret, whole) = self
            .current_text()
            .map(|edit| (edit.text.as_str(), edit.caret(), edit.whole))
            .unwrap_or(("", 0, false));
        if whole {
            return Some(UTF16Selection {
                range: 0..byte_to_utf16(text, text.len()),
                reversed: false,
            });
        }
        let caret = byte_to_utf16(text, caret);
        Some(UTF16Selection {
            range: caret..caret,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        if let Some((text, _, marked)) = self.word_input() {
            let marked = marked?;
            return Some(byte_to_utf16(&text, marked.start)..byte_to_utf16(&text, marked.end));
        }
        let edit = self.current_text()?;
        let marked = edit.marked.clone()?;
        Some(byte_to_utf16(&edit.text, marked.start)..byte_to_utf16(&edit.text, marked.end))
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        // The composition stays as it reads, committed.
        if let Some((text, _, Some(marked))) = self.word_input() {
            let committed = text[marked].to_string();
            self.word_edit(cx, |document| document.commit_composition(&committed));
            return;
        }
        match &mut self.document {
            Document::Workbook(sheet) => {
                if let Some(edit) = sheet.typing() {
                    edit.marked = None;
                }
            }
            Document::Deck {
                edit: Some((_, edit)),
                ..
            } => edit.marked = None,
            Document::Pages {
                story: Some(bar), ..
            } => bar.text.marked = None,
            Document::Pages {
                find: Some(bar), ..
            } if bar.active => bar.field_mut().marked = None,
            _ => {}
        }
    }

    fn paste(&mut self, item: gpui::ClipboardItem, window: &mut Window, cx: &mut Context<Self>) {
        self.paste_arrived = true;
        let Some(text) = item.text() else {
            return;
        };
        match &mut self.document {
            // Into a bar, its first line.
            Document::Pages { story: Some(_), .. } => {
                let line = text.lines().next().unwrap_or_default().to_string();
                self.replace_text_in_range(None, &line, window, cx);
            }
            Document::Pages {
                find: Some(bar), ..
            } if bar.active => {
                let line = text.lines().next().unwrap_or_default().to_string();
                self.replace_text_in_range(None, &line, window, cx);
            }
            Document::Pages { .. } => {
                // Lines become paragraphs, as they were when copied out.
                self.word_edit(cx, move |document| document.paste(&text));
            }
            // Excel's clipboard text fills cells from the selection's
            // top-left, or tiles over it; a copy made here pastes its
            // formulas and formats.
            Document::Workbook(sheet) if sheet.typed().is_none() => {
                self.sheet_paste(Some(&text), cx);
            }
            // Into a cell being typed into, its first line, as in Excel.
            Document::Workbook(_) => {
                let line = text.lines().next().unwrap_or_default().to_string();
                self.replace_text_in_range(None, &line, window, cx);
            }
            Document::Deck { .. } => self.replace_text_in_range(None, &text, window, cx),
        }
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(self.document, Document::Pages { .. })
            && !self.finding()
            && !self.story_editing()
        {
            let text = text.to_string();
            let composing = self
                .word_input()
                .is_some_and(|(_, _, marked)| marked.is_some());
            self.word_edit(cx, move |document| {
                if composing {
                    document.commit_composition(&text)
                } else {
                    document.insert_text(&text)
                }
            });
            return;
        }
        if let Some(edit) = self.typing_target(cx) {
            edit.replace(range, text, false);
            cx.notify();
        }
        if let Document::Workbook(sheet) = &mut self.document {
            sheet.typed_into();
        }
        if self.finding() {
            self.typed_into_find(cx);
        }
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(self.document, Document::Pages { .. })
            && !self.finding()
            && !self.story_editing()
        {
            let text = new_text.to_string();
            self.word_edit(cx, move |document| document.set_composition(&text));
            return;
        }
        if let Some(edit) = self.typing_target(cx) {
            edit.replace(range, new_text, true);
            cx.notify();
        }
        if let Document::Workbook(sheet) = &mut self.document {
            sheet.typed_into();
        }
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        // In a Word document, beside the caret — a selection's moving end,
        // which the renderer reports no caret for.
        if let Document::Pages {
            document,
            page_bounds,
            ..
        } = &mut self.document
        {
            if let Some(caret) = document.focus_rect() {
                let (page, y) = document.page_and_offset(caret.document_y);
                if let Some(page) = page_bounds.borrow().get(page).copied().flatten() {
                    let zoom = self.shown_zoom;
                    return Some(Bounds {
                        origin: point(
                            page.origin.x + px(caret.x as f32 * zoom),
                            page.origin.y + px(y as f32 * zoom),
                        ),
                        size: size(px(1.0), px(caret.height as f32 * zoom)),
                    });
                }
            }
        }
        // The candidate window goes at the editing surface's top-left: never
        // off-screen, if not beside the caret yet.
        Some(Bounds {
            origin: element_bounds.origin,
            size: size(px(1.0), px(20.0)),
        })
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}

impl gpui::Focusable for DocView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for DocView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for image in self.retired.drain(..) {
            window.drop_image(image).ok();
        }
        // Word pages follow the palette onto dark paper and back, checked
        // per frame so the toggle and the system can both change it. A no-op
        // unless it changed.
        if let Document::Pages { document, .. } = &mut self.document {
            match document.set_dark_paper(palette::is_dark()) {
                Ok(true) => self.refresh_repainted(),
                Ok(false) => {}
                Err(error) => self.status = error.into(),
            }
        }
        let body = if self.slideshow {
            self.slideshow_body(window, cx)
        } else {
            match self.document {
                Document::Pages { .. } => self.pages_body(window.viewport_size(), cx),
                Document::Deck { .. } => self.deck_body(window, cx),
                Document::Workbook(_) => self.workbook_body(window, cx),
            }
        };
        div()
            .size_full()
            .text_color(palette::text())
            .font_family(UI_FONT)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::handle_key))
            .child(body)
    }
}

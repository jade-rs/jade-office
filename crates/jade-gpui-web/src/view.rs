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

mod deck;
mod sheet;
pub(crate) mod word_characters;
pub(crate) mod word_controls;
pub(crate) mod word_design;
pub(crate) mod word_dialog;
mod word_dialogs;
pub(crate) mod word_equation;
pub(crate) mod word_fields;
mod word_find;
mod word_form;
pub(crate) mod word_kit;
pub(crate) mod word_lists;
pub(crate) mod word_mailings;
pub(crate) mod word_objects_ui;
pub(crate) mod word_paragraph;
pub(crate) mod word_references;
mod word_review;
pub(crate) mod word_ruler;
pub(crate) mod word_setup;
pub(crate) mod word_story;
pub(crate) mod word_styles;
mod word_table;
pub(crate) mod word_tools_ui;

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
            // A CSV opens as a workbook of one sheet, as in Excel.
            "xlsx" | "xlsm" | "xltx" | "xltm" | "csv" => Some(Kind::Workbook),
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
        document: Box<jade_web::editing::WordDoc>,
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
        /// A comment being typed — new, a reply, an edit — in its bar.
        comment: Option<Box<word_review::CommentBar>>,
        /// A dialog — a link, a note, the paragraph's format — over the
        /// pages; it takes the keyboard as the bars do.
        dialog: Option<Box<word_dialogs::Dialog>>,
        /// Whether the comments pane is open beside the pages.
        show_comments: bool,
        /// The picture or shape last pressed on, and the drag moving or
        /// resizing it while the button is down.
        object: Option<ObjectGrab>,
        /// The table tabs' dialogs, pen and tools, and a border being
        /// dragged.
        tables: Box<jade_web::word::table_ui::TableTools>,
    },
    Deck {
        deck: jade_web::Deck,
        current: usize,
        thumbnails: Vec<Option<Arc<RenderImage>>>,
        thumbnail_scroll: ScrollHandle,
        slide: Option<PaintedSlide>,
        selected: Option<u32>,
        /// Shapes selected along with `selected`, by ⇧- or ⌘-click or ⌘A.
        more: Vec<u32>,
        /// Shapes copied with ⌘C: their slide and ids, pasted from there.
        clipboard: Option<(usize, Vec<u32>)>,
        /// The selected shape's name, for the status bar.
        selected_name: Option<SharedString>,
        /// Text being typed into a table's cell, in a box over it.
        edit: Option<(TextTarget, TextEdit)>,
        /// The speaker notes being typed into, written when they are left.
        notes_edit: Option<TextEdit>,
        /// Text being typed into a shape where it lies: its caret,
        /// selection and an IME's composition.
        typing: Option<jade_web::deck::ShapeText>,
        /// A press in the text being typed into is being dragged, selecting.
        selecting_text: bool,
        /// Cells selected across a table, as by Shift+click.
        cells: Option<CellSelection>,
        /// A shape being moved or resized with the mouse.
        drag: Option<ShapeDrag>,
        /// Window px per slide px of the slide on show.
        scale: f32,
        /// Whether Layout ▾'s list is open.
        layouts_open: bool,
        /// The format strip's list that is open, if any.
        format_popup: Option<deck::Popup>,
        /// What the next drag on the slide draws: a text box or a shape.
        inserting: Option<jade_web::deck::Insert>,
        /// That drag, from where it started to where it is, in slide px.
        drawing: Option<((f64, f64), (f64, f64))>,
        /// Keys go to the slide list — Delete deletes the slide — after a
        /// click on a thumbnail, as in PowerPoint's thumbnail pane.
        slides_focused: bool,
    },
    Workbook(Box<sheet::Sheet>),
}

/// A picture or shape taken hold of on a page: where it is, where the
/// press was, and the box a drag has given it so far — drawn as an outline
/// and put in on release, one undo step.
#[derive(Clone, Copy)]
struct ObjectGrab {
    object: jade_web::word::picture::PlacedObject,
    from: (f64, f64),
    rect: (f64, f64, f64, f64),
    resizing: bool,
    /// Whether the button is still down.
    dragging: bool,
}

thread_local! {
    /// The last copy's formatting, for a paste — in any tab — that finds the
    /// copy's text still on the clipboard.
    pub(crate) static RICH_CLIP: RefCell<Option<jade_web::word::clipboard::RichClip>> =
        const { RefCell::new(None) };
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
    /// The status bar's 文字カウント and the caret, selection and edit it
    /// was counted for: counting reads every paragraph.
    word_count: RefCell<Option<(FormatKey, SharedString)>>,
    /// The フォント dialog and the other character tools' dialogs.
    characters: word_characters::Characters,
    /// 参考資料's dialogs, the navigation pane and ブックマーク表示
    /// (`word_references`).
    references: word_references::References,
    /// The 開発 tab's dialogs and the list or calendar dropped under a
    /// content control.
    controls: word_controls::ControlsUi,
    /// The field dialogs: フィールド, ASK and FILLIN's prompts, 文書の
    /// プロパティ (`word_fields`).
    fields: word_fields::FieldsUi,
    /// The paragraph, list, style and template tools' dialogs, the rulers
    /// and the スタイル pane (`word_tools_ui`).
    tools: word_tools_ui::WordTools,
    /// トリミング, the pens, text box links, wrap points and the Selection
    /// Pane (`word_objects_ui`).
    objects_ui: word_objects_ui::ObjectsUi,
    /// The 差し込み文書 tab's merge, for a Word document.
    mailings: Option<word_mailings::Mailings>,
    host: Host,
    /// A ページ設定, 段組み, ページ番号の書式 or ページ罫線 dialog over a Word
    /// document, while one is open (`word_setup`).
    page_dialog: Option<Box<word_setup::PageDialogState>>,
    /// The equation being edited, if one is (`word_equation`).
    equation: Option<Box<jade_web::word::equation::EquationEditor>>,
    /// The 記号と特殊文字 category the 数式 tab shows.
    equation_symbols: usize,
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
        let mut mailings = None;
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
                mailings = Some(word_mailings::Mailings::new(
                    &opened,
                    Some(word_mailings::MergeFonts {
                        scale: fonts.scale,
                        fonts: fonts.backend.fonts(),
                        measurer: Rc::clone(&fonts.measurer),
                    }),
                ));
                let mut ribbon = Box::<ribbon::Ribbon>::default();
                ribbon.read_styles(&opened);
                Document::Pages {
                    ribbon,
                    format: None,
                    highlight: None,
                    dragging: false,
                    reveal_pending: false,
                    find: None,
                    comment: None,
                    dialog: None,
                    show_comments: false,
                    object: None,
                    tables: Box::default(),
                    pages: (0..opened.page_count())
                        .map(|index| {
                            opened
                                .page(index)
                                .as_ref()
                                .and_then(PageImage::new)
                                .unwrap_or_else(PageImage::blank)
                        })
                        .collect(),
                    document: Box::new(opened.into()),
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
                    more: Vec::new(),
                    clipboard: None,
                    selected_name: None,
                    edit: None,
                    notes_edit: None,
                    typing: None,
                    selecting_text: false,
                    cells: None,
                    drag: None,
                    scale: 1.0,
                    layouts_open: false,
                    format_popup: None,
                    inserting: None,
                    drawing: None,
                    slides_focused: false,
                }
            }
            Kind::Workbook => {
                let book = if jade_web::csv::is_csv(&name) {
                    jade_web::Workbook::open_csv(
                        &name,
                        bytes,
                        &fonts.time_zone,
                        fonts.backend.clone(),
                    )?
                } else {
                    jade_web::Workbook::open(bytes, &fonts.time_zone, fonts.backend.clone())?
                };
                Document::Workbook(Box::new(sheet::Sheet::open(book)))
            }
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
            word_count: RefCell::new(None),
            characters: word_characters::Characters::default(),
            references: word_references::References::default(),
            controls: word_controls::ControlsUi::default(),
            fields: word_fields::FieldsUi::default(),
            tools: word_tools_ui::WordTools::default(),
            objects_ui: word_objects_ui::ObjectsUi::default(),
            mailings,
            host,
            page_dialog: None,
            equation: None,
            equation_symbols: 0,
        })
    }

    // ---- what the workspace asks ------------------------------------------

    /// What the status bar shows for this view, as the native views do.
    pub fn status_items(&self) -> Vec<SharedString> {
        let mut items: Vec<SharedString> = Vec::new();
        match &self.document {
            Document::Pages {
                pages,
                current,
                document,
                ribbon,
                ..
            } => {
                items.push(
                    i18n::format(
                        "doc.position",
                        &[("current", &(current + 1)), ("count", &pages.len())],
                    )
                    .into(),
                );
                items.push(self.word_count_status(document));
                if ribbon.painter.is_some() {
                    items.push(i18n::text("painter.armed").into());
                }
            }
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

    /// 文字カウント for the status bar, as Word shows it: the document's
    /// words and characters, and the selection's out of them.
    fn word_count_status(&self, document: &jade_web::editing::EditableDocument) -> SharedString {
        use jade_web::word::count::grouped;
        use jade_web::word::WordCounting;
        let key = (document.caret(), document.selection(), self.generation);
        if let Some((cached, status)) = self.word_count.borrow().as_ref() {
            if *cached == key {
                return status.clone();
            }
        }
        let whole = document.document_count();
        let status: SharedString = match document.selection_count() {
            Some(selected) => i18n::format(
                "doc.word_count_selection",
                &[
                    ("selected_words", &grouped(selected.words)),
                    ("words", &grouped(whole.words)),
                    ("selected_characters", &grouped(selected.characters)),
                    ("characters", &grouped(whole.characters)),
                ],
            ),
            None => i18n::format(
                "doc.word_count",
                &[
                    ("words", &grouped(whole.words)),
                    ("characters", &grouped(whole.characters)),
                ],
            ),
        }
        .into();
        *self.word_count.borrow_mut() = Some((key, status.clone()));
        status
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
                // Undo can touch any slide, add one or take one away;
                // thumbnails are cheap to repaint.
                if let Document::Deck {
                    deck,
                    current,
                    thumbnails,
                    ..
                } = &mut self.document
                {
                    self.retired.extend(thumbnails.drain(..).flatten());
                    thumbnails.resize(deck.slide_count(), None);
                    *current = (*current).min(deck.slide_count().saturating_sub(1));
                }
            }
            Ok(false) => {}
            // A formula Excel would refuse: its words, not a write error.
            Err(error) if error == jade_web::typing::FORMULA_PROBLEM => {
                self.status = i18n::text("sheet.formula_problem").into();
            }
            // An entry a data validation refuses: the rule's words, or
            // Excel's.
            // A protected sheet: Excel's words.
            Err(error) if jade_web::sheet::protected_refusal(&error) => {
                self.status = i18n::text("sheet.protected").into();
            }
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
            // A .csv saves the active sheet as CSV, as Excel does.
            Document::Workbook(sheet) => sheet.book.save_bytes_as(&self.name),
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

    /// Whether the page's picker was opened for a mail merge's list.
    pub fn wants_data_source(&self) -> bool {
        word_mailings::wants_source(self) || self.picking_for_tools()
    }

    /// The list the page's picker brought for the mail merge, or the
    /// template or picture a tool asked for.
    pub fn take_data_source(&mut self, name: &str, bytes: &[u8], cx: &mut Context<Self>) {
        if self.picking_for_tools() {
            return self.take_picked(name, bytes, cx);
        }
        word_mailings::take_source(self, name, bytes, cx);
    }

    /// A picture the page's picker brought, put in at the Word document's
    /// caret. Whether there was a document to take it.
    pub fn insert_word_picture(&mut self, bytes: Vec<u8>, cx: &mut Context<Self>) -> bool {
        if !matches!(self.document, Document::Pages { .. }) {
            return false;
        }
        // One a picture content control asked for goes into it.
        if word_controls::take_picture(self, &bytes, cx) {
            return true;
        }
        self.word_edit(cx, move |document| {
            use jade_web::word::picture::WordPicture;
            document.insert_picture(bytes)
        });
        true
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
                self.equation_after_history();
                return;
            }
            Document::Deck {
                deck,
                edit,
                typing,
                selected,
                current,
                ..
            } => {
                *edit = None;
                let result = if undo { deck.undo() } else { deck.redo() };
                // Text typed in place: Undo takes it back and the caret
                // stays, as in PowerPoint.
                if let Some(typing) = typing.as_mut() {
                    typing.composition = None;
                    typing.clamp(deck);
                }
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

    // ---- pages ------------------------------------------------------------

    /// Re-images what the document repainted since it was last asked: the
    /// tiles of each page its drawing touched, read from the page itself, so
    /// nothing drawn is missed and nothing undrawn is read.
    fn refresh_repainted(&mut self) {
        // Headings collapsed in the body follow edits that move them.
        word_references::refresh_collapsed(self);
        let Document::Pages {
            document,
            pages,
            page_size,
            ..
        } = &mut self.document
        else {
            return;
        };
        // The box the pages are laid out in, which a page turned or another
        // paper changes: the slots they are shown in, and their thumbnails,
        // follow it.
        let (width, height) = document.page_size();
        *page_size = (width as f32, height as f32);
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
        edit: impl FnOnce(&mut jade_web::editing::WordDoc) -> Result<(), String>,
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
        motion: impl FnOnce(&mut jade_web::editing::WordDoc) -> Result<(), String>,
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
        let clip = jade_web::word::clipboard::WordClipboard::copy_rich(document.as_ref());
        RICH_CLIP.with(|kept| *kept.borrow_mut() = clip);
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
        // A click in the pages takes the keyboard back from the pane's
        // search box.
        self.references.searching = false;
        window.focus(&self.focus, cx);
        if let Some(bar) = self.find_bar_mut() {
            bar.active = false;
        }
        // A double-click in the top or bottom margin opens the header or the
        // footer in place; one in the body closes it.
        if let (Some((_, y)), Document::Pages { document, .. }) =
            (self.page_point(index, event.position), &self.document)
        {
            let page_y = y - document.document_y(index, 0.0);
            if self.story_click(index, page_y, event.click_count, cx) {
                return;
            }
        }
        let (Some((x, y)), Some(target)) = (
            self.page_point(index, event.position),
            self.page_position(index, event.position),
        ) else {
            return;
        };
        // The pen, a crop handle, a wrap point, a text box to link to.
        if let Document::Pages { document, .. } = &self.document {
            let page_y = y - document.document_y(index, 0.0);
            if self.object_ui_press(index, (x, page_y), y, cx) {
                return;
            }
        }
        // A picture is over the text: a press on one takes hold of it rather
        // than putting the caret behind it. Its bottom-right corner resizes.
        if let Document::Pages {
            document, object, ..
        } = &mut self.document
        {
            let page_y = y - document.document_y(index, 0.0);
            let grabbed = document.object_at(x, y).filter(|found| found.page == index);
            *object = grabbed.map(|found| {
                const HANDLE: f64 = 12.0;
                ObjectGrab {
                    object: found,
                    from: (x, page_y),
                    rect: (found.x, found.y, found.width, found.height),
                    resizing: x >= found.x + found.width - HANDLE
                        && page_y >= found.y + found.height - HANDLE,
                    dragging: true,
                }
            });
            if object.is_some() {
                return cx.notify();
            }
        }
        // ⌘-click follows a link, as Word does.
        if (event.modifiers.platform || event.modifiers.control) && self.follow_link_at(x, y, cx) {
            return;
        }
        let page_y = match &self.document {
            Document::Pages { document, .. } => y - document.document_y(index, 0.0),
            _ => y,
        };
        if self.equation_click(index, x, page_y, cx) {
            return;
        }
        if self.table_mouse_down(index, (x, y), event.click_count, cx) {
            return;
        }
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
        // A press on a content control or a form field: a box ticked, a
        // list or a calendar dropped, a placeholder selected.
        if !extend && clicks == 1 {
            word_controls::click(self, index, cx);
        }
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
        if self.object_ui_drag(event, cx) {
            return;
        }
        if self.drag_object(event, cx) {
            return;
        }
        if self.table_mouse_move(event, cx) {
            return;
        }
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

    /// Moves or resizes the picture held, as an outline: the pages are not
    /// repainted until the release puts it there. Whether one was held.
    fn drag_object(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) -> bool {
        let zoom = self.shown_zoom.max(0.01);
        let Document::Pages {
            object: Some(grab),
            page_bounds,
            ..
        } = &mut self.document
        else {
            return false;
        };
        if !grab.dragging {
            return false;
        }
        if event.pressed_button != Some(MouseButton::Left) {
            grab.dragging = false;
            return true;
        }
        let Some(bounds) = page_bounds
            .borrow()
            .get(grab.object.page)
            .copied()
            .flatten()
        else {
            return true;
        };
        let x = f64::from((event.position.x - bounds.origin.x) / zoom);
        let y = f64::from((event.position.y - bounds.origin.y) / zoom);
        let (dx, dy) = (x - grab.from.0, y - grab.from.1);
        let start = grab.object;
        grab.rect = if grab.resizing {
            (
                start.x,
                start.y,
                (start.width + dx).max(8.0),
                (start.height + dy).max(8.0),
            )
        } else {
            (start.x + dx, start.y + dy, start.width, start.height)
        };
        cx.notify();
        true
    }

    /// A release puts the picture held where the drag left it.
    fn release_object(&mut self, cx: &mut Context<Self>) {
        let Document::Pages {
            object: Some(grab), ..
        } = &mut self.document
        else {
            return;
        };
        if !std::mem::take(&mut grab.dragging) {
            return;
        }
        let start = grab.object;
        let rect = grab.rect;
        if rect == (start.x, start.y, start.width, start.height) {
            return;
        }
        self.word_edit(cx, move |document| document.place_object(start, rect));
        // Where the picture is now, for the outline and the next drag.
        if let Document::Pages {
            document, object, ..
        } = &mut self.document
        {
            *object = object.and_then(|grab| {
                let moved = document.object_region(grab.object.target)?;
                Some(ObjectGrab {
                    object: moved,
                    rect: (moved.x, moved.y, moved.width, moved.height),
                    ..grab
                })
            });
        }
    }

    /// A release ends a drag; the ribbon, left alone through it, catches up.
    fn page_release(&mut self, cx: &mut Context<Self>) {
        if self.object_ui_release(cx) {
            return;
        }
        self.release_object(cx);
        // The brush paints what was just clicked or dragged over.
        let painting = matches!(
            &self.document,
            Document::Pages { ribbon, object: None, .. } if ribbon.painter.is_some()
        );
        if painting {
            self.paint_format(false, cx);
        }
        self.table_mouse_up(cx);
        if let Document::Pages { dragging, .. } = &mut self.document {
            if std::mem::take(dragging) {
                cx.notify();
            }
        }
    }

    /// 書式のコピー: takes up the format at the caret or the selection, to
    /// paint onto what is selected next; `sticky` (a double-click on the
    /// brush) keeps it painting until Escape. A second click puts it down.
    fn pick_up_format(&mut self, sticky: bool, cx: &mut Context<Self>) {
        use jade_web::word::WordFormatPainter;
        let Document::Pages {
            document, ribbon, ..
        } = &mut self.document
        else {
            return;
        };
        let copied = document.copy_format();
        ribbon.copied_format = Some(copied.clone());
        ribbon.painter = match &ribbon.painter {
            // A double-click arrives as a single click first: it turns the
            // brush sticky rather than off.
            Some(_) if !sticky => None,
            _ => Some((copied, sticky)),
        };
        cx.notify();
    }

    /// Paints the format the brush holds — or, from ⌘⇧V, the one ⌘⇧C took
    /// up — onto the selection, or the word at the caret.
    fn paint_format(&mut self, from_shortcut: bool, cx: &mut Context<Self>) {
        use jade_web::word::WordFormatPainter;
        let Document::Pages { ribbon, .. } = &mut self.document else {
            return;
        };
        let copied = if from_shortcut {
            ribbon.copied_format.clone()
        } else {
            ribbon.painter.as_ref().map(|(copied, _)| copied.clone())
        };
        let Some(copied) = copied else {
            return;
        };
        if !from_shortcut && !ribbon.painter.as_ref().is_some_and(|(_, sticky)| *sticky) {
            ribbon.painter = None;
        }
        self.word_edit(cx, move |document| document.paste_format(&copied));
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
            "escape" => {
                if let Document::Pages { ribbon, .. } = &mut self.document {
                    if ribbon.painter.take().is_some() {
                        cx.notify();
                        return true;
                    }
                }
                return false;
            }
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
            // ⇧Enter breaks the line inside the paragraph (`w:br`), as in
            // Word; Enter splits the paragraph.
            "enter" if extend => {
                self.word_edit(cx, |document| document.insert_line_break());
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
                    // A document protected for forms goes from field to field.
                    if jade_web::word::controls::WordControls::tab_to_form_field(
                        document, backwards,
                    ) {
                        return Ok(());
                    }
                    // At a list item's start, a level down (or up).
                    if let Some(result) = crate::word_tools::tab_in_list(document, backwards) {
                        return result;
                    }
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
        // ホーム's size, colour and bold format the equation being edited.
        if self.equation_home_format(&action, cx) {
            if let Document::Pages { ribbon, .. } = &mut self.document {
                ribbon.popup = None;
            }
            return;
        }
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
        if let Action::Story(action) = action {
            return self.story_action(action, cx);
        }
        if let Action::Review(review) = action {
            return self.review_action(review, cx);
        }
        if action == Action::InsertLink {
            return self.open_link_dialog(cx);
        }
        if let Action::FormatPainter(sticky) = action {
            return self.pick_up_format(sticky, cx);
        }
        if action == Action::ParagraphDialog {
            return self.open_paragraph_dialog(cx);
        }
        if let Action::InsertNote(kind) = action {
            return self.open_note_dialog(kind, cx);
        }
        if let Action::Setup(setup) = action {
            return self.setup_action(setup, cx);
        }
        if let Action::Characters(action) = action {
            return word_characters::run(self, action, cx);
        }
        if let Action::References(action) = action {
            return word_references::run(self, action, cx);
        }
        if let Action::Controls(action) = action {
            return word_controls::run(self, action, cx);
        }
        if let Action::Equation(action) = action {
            return self.equation_action(action, cx);
        }

        if matches!(action, Action::Undo | Action::Redo) && self.equation.is_some() {
            // A paragraph whose style updates itself takes the format into the
            // style (自動的に更新する).
            let formats = matches!(
                action,
                Action::Toggle(_)
                    | Action::FontFamilyName(_)
                    | Action::FontSize(_)
                    | Action::StepFontSize(_)
                    | Action::FontColor(_)
                    | Action::Align(_)
                    | Action::LineSpacing(_)
                    | Action::Indent(_)
                    | Action::Shading(_)
                    | Action::ParagraphBorders(_)
            );
            if formats {
                let view = cx.weak_entity();
                cx.defer(move |cx| {
                    view.update(cx, |view, cx| {
                        view.auto_update_style();
                        cx.notify();
                    })
                    .ok();
                });
            }
            self.word_edit(cx, move |document| match action {
                Action::Undo => document.undo().map(|_| ()),
                _ => document.redo().map(|_| ()),
            });
            return self.equation_after_history();
        }
        if action == Action::InsertPicture {
            // The page's picker answers through `open_file`, which hands a
            // picture to the Word document in front.
            return self.host.pick();
        }
        if let Action::Wrap(wrap) = action {
            let selected = match &self.document {
                Document::Pages { object, .. } => object.map(|grab| grab.object),
                _ => None,
            };
            if let Some(selected) = selected {
                self.word_edit(cx, move |document| {
                    use jade_web::word::picture::WordPicture;
                    document.set_picture_wrap(selected, wrap)
                });
                // Where the picture now is: floating moves it.
                self.refresh_selected_object();
            }
            return;
        }
        if let Action::Tables(action) = action {
            return self.table_action(action, cx);
        }
        if let Action::Word(command) = action {
            return self.run_tool(command, cx);
        }
        if let Action::Object(crate::word_objects::ObjectAction::Ui(ui)) = action {
            return self.object_ui_action(ui, cx);
        }
        if let Action::Object(object_action) = action {
            let selected = match &self.document {
                Document::Pages { object, .. } => object.map(|grab| grab.object),
                _ => None,
            };
            self.word_edit(cx, move |document| {
                crate::word_objects::run(document, selected, object_action)
            });
            // The object where it is now: moved, rotated or restyled.
            if let Document::Pages {
                document, object, ..
            } = &mut self.document
            {
                *object = object.and_then(|grab| {
                    let moved = document.object_region(grab.object.target)?;
                    Some(ObjectGrab {
                        object: moved,
                        rect: (moved.x, moved.y, moved.width, moved.height),
                        ..grab
                    })
                });
            }
            return;
        }
        if let Action::Style(style_id) = action {
            let result = match style_id.as_deref() {
                Some(id) => crate::word_tools::apply_style(document, id),
                None => document.set_paragraph_style(None),
            };
            ribbon.read_styles(document);
            self.refresh_repainted();
            return self.word_result(result, cx);
        }
        self.word_edit(cx, move |document| match action {
            Action::Toggle(format) => document.toggle_format(format),
            Action::ClearFormatting => document.clear_formatting(),
            Action::FontFamilyName(family) => document.set_font_family(&family),
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
            | Action::Story(_)
            | Action::Setup(_)
            | Action::Characters(_)
            | Action::References(_)
            | Action::Controls(_)
            | Action::Equation(_)
            | Action::Review(_)
            | Action::InsertPicture
            | Action::InsertLink
            | Action::InsertNote(_)
            | Action::ParagraphDialog
            | Action::FormatPainter(_)
            | Action::Wrap(_)
            | Action::Object(_)
            | Action::Style(_)
            | Action::Tables(_)
            | Action::Word(_)
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
        let table = self.table_tab_state();
        let developer = matches!(&self.document,
            Document::Pages { ribbon, .. } if ribbon.tab == ribbon::Tab::Developer)
        .then(|| word_controls::state(self));
        let mailings = matches!(&self.document,
            Document::Pages { ribbon, .. } if ribbon.tab == ribbon::Tab::Mailings)
        .then(|| word_mailings::groups(self))
        .flatten();
        let equation = self.equation_tab_state();
        let story = self.story_tab_state();
        let object_modes = self.object_modes();
        let Document::Pages {
            document,
            ribbon,
            format,
            pages,
            current,
            dragging,
            show_comments,
            object,
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
        let tabs = {
            use jade_web::word::review::WordReview;
            ribbon::TabState {
                page,
                tools: crate::word_tools::ToolState::read(
                    document,
                    match ribbon.popup {
                        Some(ribbon::Popup::Word(menu)) => Some(menu),
                        _ => None,
                    },
                ),
                tracking: document.is_tracking_changes(),
                comments: *show_comments,
                picture: object.is_some_and(|grab| grab.object.is_picture()),
                table,
                layout: (ribbon.tab == ribbon::Tab::Layout)
                    .then(|| word_setup::LayoutState::read(document)),
                references: (ribbon.tab == ribbon::Tab::References)
                    .then(|| word_references::ribbon_state(document, &self.references)),
                object: object.map(|grab| grab.object),
                developer,
                mailings,
                equation,
                story,
                field_codes: document.field_code_view().all,
                styles_pane: self.tools.styles_pane,
                ruler: (self.tools.ruler, self.tools.unit),
                object_modes,
            }
        };
        ribbon::ribbon(
            ribbon,
            &state,
            &tabs,
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
        let dialog_bar = self.dialog_element(cx);
        let comment_bar = self.comment_bar_element();
        let comments_pane = self.comments_pane(cx);
        let styles_pane = self.styles_pane(cx);
        let selection_pane = self.selection_pane(cx);
        let mut object_overlays = match &self.document {
            Document::Pages { pages, .. } => (0..pages.len())
                .map(|index| self.object_ui_overlay(index, zoom))
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        }
        .into_iter();
        let horizontal_ruler = self.horizontal_ruler(cx);
        let mut vertical_rulers = match &self.document {
            Document::Pages { pages, .. } => (0..pages.len())
                .map(|index| self.vertical_ruler(index))
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        }
        .into_iter();
        let find_bar = self.find_bar_element(cx);
        let table_cursor = self.table_cursor();
        let table_dialog = self.table_dialog_element(cx);
        let mut table_overlays = match &self.document {
            Document::Pages { pages, .. } => (0..pages.len())
                .map(|index| self.table_overlay(index, zoom))
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        }
        .into_iter();
        let page_dialog = self.page_dialog_element(cx);
        let navigation = word_references::navigation_pane(self, cx);
        let marks = word_references::bookmark_marks(self);
        let mut control_overlays = match &self.document {
            Document::Pages { pages, .. } => (0..pages.len())
                .map(|index| {
                    let mut overlay = word_controls::overlay(self, index, zoom, cx);
                    overlay.extend(word_mailings::overlay(self, index, zoom));
                    overlay
                })
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        }
        .into_iter();
        let equation_overlay = self.equation_overlay(zoom);
        let mut story_labels = match &self.document {
            Document::Pages { pages, .. } => (0..pages.len())
                .map(|index| self.story_label(index, zoom))
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        }
        .into_iter();
        let editing_equation = self.equation.is_some();
        let Document::Pages {
            document,
            pages,
            page_size,
            scroll,
            current,
            page_bounds,
            highlight,
            object,
            ..
        } = &mut self.document
        else {
            unreachable!()
        };
        let object = *object;
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
        // The picture held, as a page and its box on it, zoomed.
        let object_outline = object.map(|grab| {
            let (x, y, width, height) = grab.rect;
            (
                grab.object.page,
                x as f32 * zoom,
                y as f32 * zoom,
                width as f32 * zoom,
                height as f32 * zoom,
            )
        });
        // The caret, as a page and a rectangle on it.
        // Inside an equation its own caret is drawn (`word_equation`).
        let caret = document
            .caret_rect()
            .filter(|_| !editing_equation)
            .map(|caret| {
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
                let table_overlay = table_overlays.next().flatten();
                let control_overlay = control_overlays.next().unwrap_or_default();
                let story_label = story_labels.next().flatten();
                let vertical_ruler = vertical_rulers.next().flatten();
                let object_overlay = object_overlays.next().unwrap_or_default();
                div()
                    .relative()
                    .flex_none()
                    .w(px(width))
                    .h(px(height))
                    .bg(palette::paper())
                    .shadow_lg()
                    .cursor(table_cursor)
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
                    .children(object_outline.filter(|(page, ..)| *page == index).map(
                        |(_, x, y, width, height)| {
                            div()
                                .absolute()
                                .left(px(x))
                                .top(px(y))
                                .w(px(width))
                                .h(px(height))
                                .border_1()
                                .border_color(palette::accent())
                                .child(
                                    div()
                                        .absolute()
                                        .right(px(-4.0))
                                        .bottom(px(-4.0))
                                        .size(px(8.0))
                                        .bg(palette::accent()),
                                )
                        },
                    ))
                    .children(table_overlay)
                    .children(story_label)
                    .children(vertical_ruler)
                    .children(object_overlay)
                    .children(word_references::page_marks(&marks, index, zoom, cx))
                    .children(control_overlay)
                    .children(
                        equation_overlay
                            .as_ref()
                            .filter(|overlay| overlay.page == index)
                            .map(|overlay| word_equation::overlay_elements(overlay, zoom))
                            .unwrap_or_default(),
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
            .children(navigation)
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
                    .children(dialog_bar)
                    .children(comment_bar)
                    .children(find_bar)
                    .children(horizontal_ruler)
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
            .children(comments_pane)
            .children(styles_pane)
            .children(selection_pane)
            .children(table_dialog.map(|dialog| gpui::deferred(dialog).with_priority(3)))
            .children(page_dialog)
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
        // An open table dialog takes the keys it has.
        if self.table_dialog_key(event, cx) {
            cx.stop_propagation();
            return;
        }
        // A dialog over a Word document is modal: it takes every key.
        if self.page_dialog.is_some() && self.page_dialog_key(event, cx) {
            cx.stop_propagation();
            return;
        }
        // A mail-merge or content control dialog takes the keyboard the
        // same way.
        if word_mailings::is_open(self) {
            if word_mailings::handle_key(self, key, event.keystroke.modifiers.shift, cx) {
                cx.stop_propagation();
            }
            return;
        }
        if self.controls.is_open() {
            if word_controls::handle_key(self, key, event.keystroke.modifiers.shift, cx) {
                cx.stop_propagation();
            }
            return;
        }
        if self.fields.is_open() {
            if self.handle_fields_key(event, cx) {
                cx.stop_propagation();
            }
            return;
        }
        if self.tools.is_open() {
            if self.handle_tool_dialog_key(event, cx) {
                cx.stop_propagation();
            }
            return;
        }
        if self.object_ui_key(key, cx) {
            cx.stop_propagation();
            return;
        }
        // A dialog takes the keyboard while it is open: its own keys stop
        // here, and typing goes on to the input handler, into its field.
        if self.characters.is_open() {
            if word_characters::handle_key(self, key, cx) {
                cx.stop_propagation();
            }
            return;
        }
        if self.references.is_open() || self.references.searching {
            if word_references::handle_key(self, key, modifiers.shift, cx) {
                cx.stop_propagation();
            }
            return;
        }
        // The find bar over a Word document takes the keys it has.
        if self.dialog_open() && self.dialog_key(event, cx) {
            cx.stop_propagation();
            return;
        }
        if self.commenting() && self.comment_key(event, cx) {
            cx.stop_propagation();
            return;
        }
        if self.equation_key(event, cx) {
            cx.stop_propagation();
            return;
        }
        if self.story_key(event, cx) {
            cx.stop_propagation();
            return;
        }
        if self.finding() && self.find_key(event, cx) {
            cx.stop_propagation();
            return;
        }
        // Word's field keys: F9, ⌘F11/⌘⇧F11, ⌘⇧F9.
        if self.field_key(event, cx) {
            cx.stop_propagation();
            return;
        }
        // ⌃⌘V 形式を選択して貼り付け, ⌥⇧⌘V 書式を結合して貼り付け.
        if matches!(self.document, Document::Pages { .. }) && modifiers.platform && key == "v" {
            if modifiers.control {
                word_characters::open_paste_special(self, cx);
                cx.stop_propagation();
                return;
            }
            if modifiers.alt && modifiers.shift {
                word_characters::paste_with(
                    self,
                    jade_web::word::clipboard::PasteMode::MergeFormatting,
                    cx,
                );
                cx.stop_propagation();
                return;
            }
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
            let deck = matches!(self.document, Document::Deck { .. });
            if deck && !self.slideshow && self.deck_shortcut(event, cx) {
                cx.stop_propagation();
                return;
            }
            match key {
                "z" => self.step_history(!modifiers.shift, cx),
                "y" => self.step_history(false, cx),
                "s" => self.save(cx),
                "a" if pages => self.word_move(cx, |document| document.select_all()),
                // Find, and Word for Mac's Find and Replace.
                "f" if pages && !modifiers.alt => self.open_find(false, cx),
                "h" if pages && modifiers.shift => self.open_find(true, cx),
                // Word for Mac's 書式のコピー / 書式の貼り付け.
                "c" if pages && modifiers.shift => self.pick_up_format(false, cx),
                "v" if pages && modifiers.shift => self.paint_format(true, cx),
                // ⌘↩ is Word's page break (改ページ).
                "enter" if pages => self.setup_action(
                    word_setup::SetupAction::Break(jade_web::word::page_setup::PageBreak::Page),
                    cx,
                ),
                "c" if pages => self.word_copy(false, cx),
                // ハイパーリンクの挿入.
                "k" if pages => self.open_link_dialog(cx),
                // Word for Mac's 書式 → 段落 (⌥⌘M).
                "m" | "µ" if pages && modifiers.alt => self.open_paragraph_dialog(cx),
                // Word for Mac's ⌥⌘F and ⌥⌘D: 脚注の挿入, 文末脚注の挿入.
                "f" | "ƒ" if pages && modifiers.alt => {
                    self.open_note_dialog(jade_web::word::NoteKind::Footnote, cx)
                }
                "d" | "∂" if pages && modifiers.alt => {
                    self.open_note_dialog(jade_web::word::NoteKind::Endnote, cx)
                }
                // Word for Mac's フォント… (書式 menu).
                "d" if pages => word_characters::open_font_dialog(self, cx),
                // Word for Mac's ジャンプ (⌥⌘G), 索引登録 (⌥⇧⌘X), 引用文の
                // 登録 (⌥⇧⌘I) and the navigation pane (⌃⌘N).
                "g" | "©" if pages && modifiers.alt => {
                    word_references::run(self, word_references::RefAction::OpenGoTo, cx)
                }
                "x" | "˛" | "X" if pages && modifiers.alt && modifiers.shift => {
                    word_references::run(self, word_references::RefAction::OpenMarkEntry, cx)
                }
                "i" | "ˆ" | "I" if pages && modifiers.alt && modifiers.shift => {
                    word_references::run(self, word_references::RefAction::OpenMarkCitation, cx)
                }
                "n" if pages && modifiers.platform && modifiers.control => {
                    word_references::run(self, word_references::RefAction::ToggleNavigation, cx)
                }
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
        if word_mailings::is_open(self) {
            return self
                .mailings
                .as_mut()
                .and_then(|mailings| mailings.merge.field_mut());
        }
        if self.controls.is_open() {
            return self.controls.field_mut();
        }
        if self.fields.is_open() {
            return self.fields.text_field_mut();
        }
        if self.tools.is_open() {
            return self.dialog_field_mut();
        }
        if self.objects_ui.text_field().is_some() {
            return self.objects_ui.text_field_mut();
        }
        if self.characters.is_open() {
            return self.characters.text_field_mut();
        }
        if self.references.is_open() || self.references.searching {
            return self.references.text_field_mut();
        }
        match &self.document {
            // Typing into a selected cell replaces it, as in Excel.
            // A dialog over the grid takes no typing into the cells.
            Document::Workbook(sheet)
                if sheet.asking.is_some() || sheet.managing || sheet.pasting.is_some() => {}
            Document::Workbook(sheet) if sheet.typed().is_none() => {
                self.begin_cell_edit(Some(String::new()), false, cx)
            }
            Document::Deck {
                edit: None,
                notes_edit: None,
                typing: None,
                selected: Some(_),
                ..
            } => self.begin_slide_text(None, cx),
            _ => {}
        }
        match &mut self.document {
            Document::Workbook(sheet) => sheet.typing(),
            Document::Deck {
                notes_edit: Some(notes),
                ..
            } => Some(notes),
            Document::Deck { edit, .. } => edit.as_mut().map(|(_, text)| text),
            Document::Pages {
                dialog: Some(dialog),
                ..
            } => dialog.field_mut(),
            Document::Pages {
                comment: Some(bar), ..
            } => Some(&mut bar.text),
            Document::Pages { find, .. } => find
                .as_deref_mut()
                .filter(|bar| bar.active)
                .map(|bar| bar.field_mut()),
        }
    }

    /// The field of an open Word dialog that typing goes into.
    fn word_dialog_field_mut(&mut self) -> Option<&mut TextEdit> {
        if self.fields.is_open() {
            return self.fields.text_field_mut();
        }
        if self.tools.is_open() {
            return self.dialog_field_mut();
        }
        if self.objects_ui.text_field().is_some() {
            return self.objects_ui.text_field_mut();
        }
        if self.characters.is_open() {
            return self.characters.text_field_mut();
        }
        None
    }

    fn current_text(&self) -> Option<&TextEdit> {
        if let Some(field) = self
            .mailings
            .as_ref()
            .and_then(|mailings| mailings.merge.field())
        {
            return Some(field);
        }
        if let Some(field) = self.controls.field() {
            return Some(field);
        }
        if let Some(field) = self.fields.text_field() {
            return Some(field);
        }
        if let Some(field) = self.dialog_field() {
            return Some(field);
        }
        if let Some(field) = self.objects_ui.text_field() {
            return Some(field);
        }
        if let Some(field) = self.characters.text_field() {
            return Some(field);
        }
        if let Some(field) = self.references.text_field() {
            return Some(field);
        }
        match &self.document {
            Document::Workbook(sheet) => sheet.typed(),
            Document::Deck {
                notes_edit: Some(notes),
                ..
            } => Some(notes),
            Document::Deck { edit, .. } => edit.as_ref().map(|(_, text)| text),
            Document::Pages {
                dialog: Some(dialog),
                ..
            } => dialog.field(),
            Document::Pages {
                comment: Some(bar), ..
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
        (area.1 - TOOLBAR_HEIGHT - deck::FORMAT_BAR_HEIGHT - SLIDE_MARGIN * 2.0).max(40.0),
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
        if self.finding() || self.commenting() {
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
        if let Some((text, _, _)) = self.slide_input() {
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
        if let Some((_, range, marked)) = self.slide_input() {
            let reversed = marked.is_none() && self.slide_input_reversed();
            return Some(UTF16Selection { range, reversed });
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
        if let Some((_, _, marked)) = self.slide_input() {
            return marked;
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
        if self.slide_input().is_some() {
            self.edit_typing(cx, |typing, deck| typing.unmark(deck));
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
                comment: Some(bar), ..
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
            Document::Pages {
                comment: Some(_), ..
            } => {
                let line = text.lines().next().unwrap_or_default().to_string();
                self.replace_text_in_range(None, &line, window, cx);
            }
            Document::Pages {
                find: Some(bar), ..
            } if bar.active => {
                let line = text.lines().next().unwrap_or_default().to_string();
                self.replace_text_in_range(None, &line, window, cx);
            }
            // Copied here, and still what the clipboard holds: with its
            // formatting, as one undo step.
            Document::Pages { .. }
                if RICH_CLIP.with(|kept| {
                    kept.borrow()
                        .as_ref()
                        .is_some_and(|clip| clip.matches(&text))
                }) =>
            {
                let clip = RICH_CLIP.with(|kept| kept.borrow().clone());
                self.word_edit(cx, move |document| {
                    use jade_web::word::clipboard::WordClipboard;
                    let Some(clip) = clip else {
                        return Ok(());
                    };
                    let caret = document.paste_rich(&clip)?;
                    document.place_caret(caret.0, caret.1);
                    Ok(())
                });
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
            // Into text being typed; with shapes selected, ⌘V pasted the
            // shapes copied in the app.
            Document::Deck {
                edit: None,
                typing: None,
                notes_edit: None,
                ..
            } => {}
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
        if word_mailings::is_open(self) {
            if let Some(field) = self
                .mailings
                .as_mut()
                .and_then(|mailings| mailings.merge.field_mut())
            {
                field.replace(range, text, false);
            }
            return cx.notify();
        }
        if self.controls.is_open() {
            if let Some(field) = self.controls.field_mut() {
                field.replace(range, text, false);
            }
            return cx.notify();
        }
        // A Word dialog's field (`word_fields`, `word_characters`, the
        // paragraph, list and style dialogs).
        if let Some(field) = self.word_dialog_field_mut() {
            // A field shown wholly selected is replaced by what is typed,
            // wherever the browser's mirror thinks the caret is.
            let range = range.filter(|range| !(field.whole && range.is_empty()));
            field.replace(range, text, false);
            return cx.notify();
        }
        if self.table_dialog_typed(text, cx) {
            return;
        }
        if self.page_dialog.is_some() {
            return self.page_dialog_input(text, cx);
        }
        // A 参考資料 dialog's field, or the navigation pane's search box.
        if let Some(field) = self.references.text_field_mut() {
            field.replace(range, text, false);
            return cx.notify();
        }
        if self.equation.is_some() {
            return self.equation_type(text, cx);
        }
        if matches!(self.document, Document::Pages { .. }) && !self.finding() && !self.commenting()
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
        if self.type_into_slide(range.clone(), text, false, cx) {
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
        if word_mailings::is_open(self) {
            if let Some(field) = self
                .mailings
                .as_mut()
                .and_then(|mailings| mailings.merge.field_mut())
            {
                field.replace(range, new_text, true);
            }
            return cx.notify();
        }
        if self.controls.is_open() {
            if let Some(field) = self.controls.field_mut() {
                field.replace(range, new_text, true);
            }
            return cx.notify();
        }
        if let Some(field) = self.word_dialog_field_mut() {
            let range = range.filter(|range| !(field.whole && range.is_empty()));
            field.replace(range, new_text, true);
            return cx.notify();
        }
        if self.table_dialog_open() || self.page_dialog.is_some() {
            return;
        }
        if let Some(field) = self.references.text_field_mut() {
            field.replace(range, new_text, true);
            return cx.notify();
        }
        if matches!(self.document, Document::Pages { .. }) && !self.finding() && !self.commenting()
        {
            let text = new_text.to_string();
            self.word_edit(cx, move |document| document.set_composition(&text));
            return;
        }
        if self.type_into_slide(range.clone(), new_text, true, cx) {
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
        if let Some(caret) = self.slide_caret_bounds() {
            return Some(caret);
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
        let dialog = word_characters::overlay(self, window, cx);
        let reference_dialog = word_references::overlay(self, window, cx);
        let controls_dialog = word_controls::dialog(self, window, cx);
        let mailings_dialog = word_mailings::dialog(self, window, cx);
        let fields_dialog = self.fields_dialog_element(window, cx);
        let tool_dialog = self.tool_dialog_element(window, cx);
        div()
            .size_full()
            .text_color(palette::text())
            .font_family(UI_FONT)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::handle_key))
            .child(body)
            .children(dialog)
            .children(reference_dialog)
            .children(controls_dialog)
            .children(mailings_dialog)
            .children(fields_dialog)
            .children(tool_dialog)
    }
}

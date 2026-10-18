//! One open file: a Word document's pages, a deck or a workbook, laid out as
//! the native app lays out its views (`jade-gpui-app`'s `DocumentView`,
//! `DeckView` and `WorkbookView`) — the page or slide list on the left, the
//! deck's toolbar and notes, the formula bar and the sheet tabs.
//!
//! Sheets and slides can be edited, through the same editing cores the
//! native app uses (`jade-web`'s `Workbook` and `Deck`): select a cell and
//! type, an IME's composition included; select a shape and type into it,
//! move it with the arrow keys, delete it; undo and redo; and save, which
//! hands the bytes to the page to download. Word documents are viewed.

use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    canvas, div, img, point, prelude::*, px, rgb, size, svg, App, Bounds, Context,
    ElementInputHandler, EntityInputHandler, FocusHandle, Global, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, Pixels, RenderImage, ScrollHandle, SharedString,
    UTF16Selection, Window,
};
use jade_tiny_skia_painter::TinySkiaPainter;
use jade_web::sheet::{format_number, reference_text, span, CellRef, MAX_COLUMN, MAX_ROW};

use crate::{i18n, palette, Fonts, Host, UI_FONT};

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

/// A Word page's pixels, as a column of tiles.
struct PageImage {
    tiles: Vec<Arc<RenderImage>>,
    /// Each tile's [`TinySkiaPainter::rows_fingerprint`] when it was read.
    fingerprints: Vec<u64>,
    size: (u32, u32),
}

impl PageImage {
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
        })
    }

    /// Re-reads the tiles drawing has changed since the last read: of the
    /// rows it touched, those whose pixels differ from the tile's. A page of
    /// another size is read whole.
    fn refresh(&mut self, painter: &TinySkiaPainter, retired: &mut Vec<Arc<RenderImage>>) {
        if (painter.width(), painter.height()) != self.size {
            if let Some(fresh) = Self::new(painter) {
                retired.append(&mut self.tiles);
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

/// UTF-16 offset → byte offset in `text`, clamped to its end.
fn utf16_to_byte(text: &str, offset: usize) -> usize {
    let mut units = 0;
    for (byte, ch) in text.char_indices() {
        if units >= offset {
            return byte;
        }
        units += ch.len_utf16();
    }
    text.len()
}

fn byte_to_utf16(text: &str, byte: usize) -> usize {
    text[..byte.min(text.len())].encode_utf16().count()
}

/// Text being typed: into a cell or into a shape.
struct TextEdit {
    text: String,
    /// The IME's composition, as a byte range of `text`.
    marked: Option<Range<usize>>,
}

impl TextEdit {
    fn new(text: String) -> Self {
        Self { text, marked: None }
    }

    /// Replaces a UTF-16 range — or the composition, or nothing at the end —
    /// with `text`, marking it as the composition when `mark` is set.
    fn replace(&mut self, range: Option<Range<usize>>, text: &str, mark: bool) {
        let bytes = match range {
            Some(range) => {
                utf16_to_byte(&self.text, range.start)..utf16_to_byte(&self.text, range.end)
            }
            None => self
                .marked
                .clone()
                .unwrap_or(self.text.len()..self.text.len()),
        };
        let start = bytes.start;
        self.text.replace_range(bytes, text);
        self.marked = (mark && !text.is_empty()).then(|| start..start + text.len());
    }
}

/// A sheet viewport's pixels and what they were painted for.
struct PaintedSheet {
    scroll: (f32, f32),
    device: (u32, u32),
    zoom: f32,
    selection: (CellRef, CellRef),
    generation: u64,
    image: Arc<RenderImage>,
}

/// A slide's pixels and what they were painted for: slide, device width,
/// edit generation.
type PaintedSlide = (usize, u32, u64, Arc<RenderImage>);

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
        edit: Option<(u32, TextEdit)>,
        /// Window px per slide px of the slide on show.
        scale: f32,
    },
    Workbook {
        book: jade_web::Workbook,
        sheets: Vec<SharedString>,
        anchor: CellRef,
        focus: CellRef,
        edit: Option<TextEdit>,
        scroll: ScrollHandle,
        painted: Option<PaintedSheet>,
        viewport: (f32, f32),
    },
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
                let opened = jade_web::editing::EditableDocument::open(
                    bytes,
                    fonts.scale,
                    fonts.backend.fonts(),
                    Rc::clone(&fonts.measurer),
                )?;
                let (width, height) = opened.page_size();
                Document::Pages {
                    pages: (0..opened.page_count())
                        .filter_map(|index| opened.page(index).as_ref().and_then(PageImage::new))
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
                    scale: 1.0,
                }
            }
            Kind::Workbook => {
                let book =
                    jade_web::Workbook::open(bytes, &fonts.time_zone, fonts.backend.clone())?;
                Document::Workbook {
                    sheets: book
                        .sheet_names()
                        .into_iter()
                        .map(SharedString::from)
                        .collect(),
                    book,
                    anchor: CellRef::new(0, 0),
                    focus: CellRef::new(0, 0),
                    edit: None,
                    scroll: ScrollHandle::new(),
                    painted: None,
                    viewport: (1.0, 1.0),
                }
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
            Document::Workbook {
                book,
                anchor,
                focus,
                ..
            } => {
                items.push(i18n::format("sheet.position", &[("name", &book.sheet_name())]).into());
                items.push(selection_label(*anchor, *focus).into());
                // Excel's summary of a multi-cell selection.
                let range = span(*anchor, *focus);
                if range.start != range.end {
                    let summary = book.summary(range);
                    if let (Some(sum), Some(average)) = (summary.sum, summary.average) {
                        items.push(
                            i18n::format("sheet.average", &[("value", &format_number(average))])
                                .into(),
                        );
                        items.push(
                            i18n::format("sheet.cell_count", &[("n", &summary.count)]).into(),
                        );
                        items.push(
                            i18n::format("sheet.sum", &[("value", &format_number(sum))]).into(),
                        );
                    } else if summary.count > 0 {
                        items.push(
                            i18n::format("sheet.cell_count", &[("n", &summary.count)]).into(),
                        );
                    }
                }
            }
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
        if let Document::Workbook { scroll, .. } = &self.document {
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
            Document::Workbook { .. } => self.set_zoom(1.0, cx),
        }
    }

    pub fn is_dirty(&self) -> bool {
        match &self.document {
            Document::Pages { document, .. } => document.is_dirty(),
            Document::Deck { deck, .. } => deck.is_dirty(),
            Document::Workbook { book, .. } => book.is_dirty(),
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
            Document::Workbook { book, .. } => book.save_bytes(),
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

    pub fn step_history(&mut self, undo: bool, cx: &mut Context<Self>) {
        let result = match &mut self.document {
            Document::Pages { document, .. } => {
                let result = if undo {
                    document.undo()
                } else {
                    document.redo()
                };
                self.refresh_repainted();
                self.word_result(result.map(|_| ()), cx);
                return;
            }
            Document::Deck {
                deck,
                edit,
                selected,
                selected_name,
                ..
            } => {
                *edit = None;
                *selected = None;
                *selected_name = None;
                if undo {
                    deck.undo()
                } else {
                    deck.redo()
                }
            }
            Document::Workbook { book, edit, .. } => {
                *edit = None;
                if undo {
                    book.undo()
                } else {
                    book.redo()
                }
            }
        };
        self.edited(result, cx);
    }

    // ---- the workbook -----------------------------------------------------

    /// The scroll position in sheet px (the offset is in screen px).
    fn sheet_scroll(scroll: &ScrollHandle, zoom: f32) -> (f64, f64) {
        let offset = scroll.offset();
        let zoom = f64::from(zoom);
        (
            (-f64::from(offset.x) / zoom).max(0.0),
            (-f64::from(offset.y) / zoom).max(0.0),
        )
    }

    fn select_cell(&mut self, to: CellRef, extend: bool, cx: &mut Context<Self>) {
        let zoom = self.shown_zoom;
        let Document::Workbook {
            book,
            anchor,
            focus,
            scroll,
            viewport,
            ..
        } = &mut self.document
        else {
            return;
        };
        if extend {
            *focus = to;
        } else {
            *anchor = to;
            *focus = to;
        }
        book.reach(to);
        let at = Self::sheet_scroll(scroll, zoom);
        let zoom = f64::from(zoom);
        let (x, y) = book.reveal(
            to,
            at,
            (f64::from(viewport.0) / zoom, f64::from(viewport.1) / zoom),
        );
        if (x, y) != at {
            scroll.set_offset(point(px((-x * zoom) as f32), px((-y * zoom) as f32)));
        }
        cx.notify();
    }

    fn step_cell(&mut self, columns: i64, rows: i64, extend: bool, cx: &mut Context<Self>) {
        let Document::Workbook { anchor, focus, .. } = &self.document else {
            return;
        };
        let from = if extend { *focus } else { *anchor };
        let to = CellRef::new(
            (i64::from(from.column) + columns).clamp(0, i64::from(MAX_COLUMN)) as u32,
            (i64::from(from.row) + rows).clamp(0, i64::from(MAX_ROW)) as u32,
        );
        self.select_cell(to, extend, cx);
    }

    /// Writes the cell being typed into, if any; the write recalculates.
    ///
    /// Returns whether the edit is done. A formula Excel would refuse keeps
    /// its editor open with what was typed, as Excel does, so the caller
    /// stays where it is rather than moving on without it.
    fn commit_cell(&mut self, cx: &mut Context<Self>) -> bool {
        let Document::Workbook {
            book, edit, anchor, ..
        } = &mut self.document
        else {
            return true;
        };
        let Some(text) = edit.take() else {
            return true;
        };
        let result = book.set_cell(*anchor, &text.text);
        let refused = matches!(&result, Err(error) if error == jade_web::typing::FORMULA_PROBLEM);
        if refused {
            *edit = Some(text);
        }
        self.edited(result, cx);
        !refused
    }

    fn begin_cell_edit(&mut self, text: Option<String>, cx: &mut Context<Self>) {
        if let Document::Workbook {
            book, edit, anchor, ..
        } = &mut self.document
        {
            let text = text.unwrap_or_else(|| book.edit_text(*anchor));
            *edit = Some(TextEdit::new(text));
            cx.notify();
        }
    }

    fn select_sheet(&mut self, index: usize, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        if let Document::Workbook {
            book,
            anchor,
            focus,
            scroll,
            painted,
            ..
        } = &mut self.document
        {
            if book.select_sheet(index).is_ok() {
                *anchor = CellRef::new(0, 0);
                *focus = *anchor;
                if let Some(old) = painted.take() {
                    self.retired.push(old.image);
                }
                scroll.set_offset(point(px(0.0), px(0.0)));
            }
        }
        cx.notify();
    }

    fn sheet_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let shift = event.keystroke.modifiers.shift;
        let key = event.keystroke.key.as_str();
        let Document::Workbook {
            book,
            edit,
            anchor,
            focus,
            ..
        } = &mut self.document
        else {
            return false;
        };
        if let Some(text) = edit.as_mut() {
            // Mid-composition the keys are the IME's.
            if text.marked.is_some() {
                return false;
            }
            match key {
                "enter" => {
                    if self.commit_cell(cx) {
                        self.step_cell(0, if shift { -1 } else { 1 }, false, cx);
                    }
                }
                "tab" => {
                    if self.commit_cell(cx) {
                        self.step_cell(if shift { -1 } else { 1 }, 0, false, cx);
                    }
                }
                "escape" => {
                    *edit = None;
                    cx.notify();
                }
                "backspace" => {
                    text.text.pop();
                    cx.notify();
                }
                _ => return false,
            }
            return true;
        }
        match key {
            "up" => self.step_cell(0, -1, shift, cx),
            "down" => self.step_cell(0, 1, shift, cx),
            "left" => self.step_cell(-1, 0, shift, cx),
            "right" => self.step_cell(1, 0, shift, cx),
            "enter" => self.step_cell(0, if shift { -1 } else { 1 }, false, cx),
            "tab" => self.step_cell(if shift { -1 } else { 1 }, 0, false, cx),
            "f2" => self.begin_cell_edit(None, cx),
            "delete" | "backspace" => {
                let result = book.clear(span(*anchor, *focus));
                self.edited(result, cx);
            }
            // Anything else is text, which arrives through the input handler
            // and starts an edit there.
            _ => return false,
        }
        true
    }

    fn sheet_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        if !self.commit_cell(cx) {
            return;
        }
        let Some(bounds) = self.surface.get() else {
            return;
        };
        let zoom = self.shown_zoom;
        let Document::Workbook {
            book,
            anchor,
            focus,
            scroll,
            ..
        } = &mut self.document
        else {
            return;
        };
        let x = f64::from(event.position.x - bounds.origin.x) / f64::from(zoom);
        let y = f64::from(event.position.y - bounds.origin.y) / f64::from(zoom);
        let Some((start, end)) = book.hit(x, y, Self::sheet_scroll(scroll, zoom)) else {
            return;
        };
        if event.modifiers.shift {
            *focus = end;
        } else {
            *anchor = start;
            *focus = end;
        }
        if event.click_count >= 2 && start == end {
            self.begin_cell_edit(None, cx);
        }
        cx.notify();
    }

    fn workbook_body(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui::Div {
        let generation = self.generation;
        let zoom = self.zoom.unwrap_or(1.0);
        self.shown_zoom = zoom;
        let (area_width, area_height) = editor_area(window, cx);
        let Document::Workbook {
            book,
            sheets,
            anchor,
            focus,
            edit,
            scroll,
            painted,
            viewport,
        } = &mut self.document
        else {
            unreachable!()
        };
        let scale = window.scale_factor();
        let size = (
            area_width.max(1.0),
            (area_height - FORMULA_BAR_HEIGHT - SHEET_TABS_HEIGHT).max(1.0),
        );
        *viewport = size;
        let offset = scroll.offset();
        let at = (-f32::from(offset.x), -f32::from(offset.y));
        let device = (
            (size.0 * scale).round() as u32,
            (size.1 * scale).round() as u32,
        );
        let selection = (*anchor, *focus);
        let fresh = painted.as_ref().is_some_and(|p| {
            p.scroll == at
                && p.device == device
                && p.zoom == zoom
                && p.selection == selection
                && p.generation == generation
        });
        if !fresh {
            let painter = book.render_viewport(
                (f64::from(at.0 / zoom), f64::from(at.1 / zoom)),
                device.0,
                device.1,
                f64::from(scale * zoom),
                Some(selection),
            );
            match painter.and_then(image) {
                Some(image) => {
                    if let Some(old) = painted.replace(PaintedSheet {
                        scroll: at,
                        device,
                        zoom,
                        selection,
                        generation,
                        image,
                    }) {
                        self.retired.push(old.image);
                    }
                }
                None => self.status = i18n::text("sheet.paint_failed").into(),
            }
        }
        let (sheet_width, sheet_height) = book.sheet_size();
        let picture = painted.as_ref().map(|p| p.image.clone());
        let current = book.sheet_index();

        let content = match edit {
            Some(text) => text.text.clone(),
            None => book.edit_text(*anchor),
        };
        let editing = edit.is_some();
        let name_box = selection_label(*anchor, *focus);

        // The editor over the active cell, white like the cell under it.
        let cell_editor = edit.as_ref().and_then(|text| {
            let (x, y, w, h) =
                book.cell_rect(*anchor, (f64::from(at.0 / zoom), f64::from(at.1 / zoom)))?;
            Some(
                div()
                    .absolute()
                    .left(px(x as f32 * zoom))
                    .top(px(y as f32 * zoom))
                    .min_w(px(w as f32 * zoom))
                    .h(px(h as f32 * zoom))
                    .flex()
                    .items_center()
                    .px(px(3.0 * zoom))
                    .bg(gpui::white())
                    .border_2()
                    .border_color(rgb(0x217346))
                    .text_color(gpui::black())
                    .text_size(px(14.0 * zoom))
                    .whitespace_nowrap()
                    .child(text.text.clone())
                    .child(div().w(px(1.0)).h(px(15.0 * zoom)).bg(gpui::black())),
            )
        });

        let tabs: Vec<_> = sheets
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let selected = index == current;
                div()
                    .id(("sheet-tab", index))
                    .relative()
                    .h_full()
                    .flex()
                    .items_center()
                    .px_4()
                    .text_xs()
                    .cursor_pointer()
                    .border_r_1()
                    .border_color(palette::border())
                    .text_color(if selected {
                        palette::accent()
                    } else {
                        palette::dim()
                    })
                    .when(selected, |tab| {
                        tab.bg(palette::window()).child(
                            div()
                                .absolute()
                                .bottom_0()
                                .left_1()
                                .right_1()
                                .h(px(2.0))
                                .rounded_t_sm()
                                .bg(palette::accent()),
                        )
                    })
                    .hover(|tab| tab.bg(palette::hovered()))
                    .child(name.clone())
                    .on_click(
                        cx.listener(move |view, _event, _window, cx| view.select_sheet(index, cx)),
                    )
            })
            .collect();

        let store = Rc::clone(&self.surface);
        let entity = cx.entity();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(palette::canvas())
            // The name box and the formula bar.
            .child(
                div()
                    .h(px(FORMULA_BAR_HEIGHT))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .bg(palette::panel())
                    .border_b_1()
                    .border_color(palette::border())
                    .text_xs()
                    .child(
                        div()
                            .w(px(96.0))
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .border_1()
                            .border_color(palette::border())
                            .bg(palette::window())
                            .text_color(palette::text())
                            .child(name_box),
                    )
                    .child(
                        div()
                            .px_1()
                            .italic()
                            .text_color(palette::faint())
                            .child("fx"),
                    )
                    .child(
                        div()
                            .id("formula-bar")
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .flex()
                            .items_center()
                            .px_2()
                            .cursor_text()
                            .text_color(palette::text())
                            .truncate()
                            .child(content)
                            .when(editing, |bar| {
                                bar.child(div().w(px(1.0)).h(px(14.0)).bg(palette::accent()))
                            })
                            .on_click(cx.listener(|view, _event, window, cx| {
                                window.focus(&view.focus, cx);
                                if !matches!(
                                    view.document,
                                    Document::Workbook { edit: Some(_), .. }
                                ) {
                                    view.begin_cell_edit(None, cx);
                                }
                            })),
                    ),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .overflow_hidden()
                    .child(
                        // The painted window sits under the scroller, which
                        // is transparent and owns the scrollbars and the
                        // mouse.
                        div()
                            .absolute()
                            .inset_0()
                            .children(picture.map(|image| img(image).w(px(size.0)).h(px(size.1)))),
                    )
                    .child(
                        div()
                            .id("sheet-scroll")
                            .absolute()
                            .inset_0()
                            .overflow_scroll()
                            .track_scroll(scroll)
                            .cursor(gpui::CursorStyle::Crosshair)
                            .on_mouse_down(MouseButton::Left, cx.listener(Self::sheet_mouse_down))
                            .child(
                                div()
                                    .w(px(sheet_width as f32 * zoom))
                                    .h(px(sheet_height as f32 * zoom))
                                    .flex_none(),
                            ),
                    )
                    .children(cell_editor)
                    // Records where the grid is, and registers the input
                    // handler so typing — an IME's included — reaches the
                    // cell.
                    .child(
                        canvas(
                            move |bounds, _window, _cx| {
                                store.set(Some(bounds));
                                bounds
                            },
                            move |_bounds, grid, window, cx| {
                                let focus = entity.read(cx).focus.clone();
                                window.handle_input(
                                    &focus,
                                    ElementInputHandler::new(grid, entity.clone()),
                                    cx,
                                );
                            },
                        )
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full(),
                    ),
            )
            // Excel keeps its sheet tabs along the bottom edge.
            .child(
                div()
                    .h(px(SHEET_TABS_HEIGHT))
                    .flex_none()
                    .flex()
                    .items_center()
                    .bg(palette::panel())
                    .border_t_1()
                    .border_color(palette::border())
                    .children(tabs),
            )
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
            ..
        } = &mut self.document
        {
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
        let Some((id, text)) = edit.take() else {
            return;
        };
        let result = deck.set_shape_text(*current, id, &text.text);
        self.edited(result, cx);
    }

    fn begin_slide_text(&mut self, cx: &mut Context<Self>) {
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
        match deck.shape_text(*current, id) {
            Some(text) => *edit = Some((id, TextEdit::new(text))),
            None => self.status = i18n::text("deck.text_locked").into(),
        }
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
        if let Some((_, text)) = edit.as_mut() {
            if text.marked.is_some() {
                return false;
            }
            match key {
                // As in PowerPoint, Escape leaves the text, keeping it.
                "escape" => self.commit_slide_text(cx),
                "enter" => {
                    text.text.push('\n');
                    cx.notify();
                }
                "backspace" => {
                    text.text.pop();
                    cx.notify();
                }
                _ => return false,
            }
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
                    self.begin_slide_text(cx);
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
        let Some(bounds) = self.surface.get() else {
            return;
        };
        let Document::Deck {
            deck,
            current,
            edit,
            scale,
            ..
        } = &mut self.document
        else {
            return;
        };
        let to_slide = |value: Pixels| f64::from(value) / f64::from(*scale);
        let x = to_slide(event.position.x - bounds.origin.x);
        let y = to_slide(event.position.y - bounds.origin.y);
        // A click inside the shape being typed into stays in the text.
        if let Some((id, _)) = edit {
            let inside = deck
                .shape_rect(*current, *id)
                .is_some_and(|rect| rect.contains(x, y));
            if inside {
                return;
            }
        }
        let hit = deck.shape_at(*current, x, y);
        self.commit_slide_text(cx);
        self.select_shape(hit);
        if event.click_count >= 2 && hit.is_some() {
            self.begin_slide_text(cx);
        }
        cx.notify();
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

        // The selection outline, or the text editor, over the slide.
        let overlay = selected.and_then(|id| {
            let rect = deck.shape_rect(current_index, id)?;
            let (x, y, w, h) = (
                rect.x as f32 * zoom,
                rect.y as f32 * zoom,
                rect.width as f32 * zoom,
                rect.height as f32 * zoom,
            );
            Some(match edit.as_ref().filter(|(edited, _)| *edited == id) {
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
                            .child(text.text.clone())
                            .child(
                                div()
                                    .w(px(1.5))
                                    .h(px((20.0 * zoom).max(12.0)))
                                    .bg(gpui::black()),
                            ),
                    ),
                None => div()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(w))
                    .h(px(h))
                    .border_1()
                    .border_color(palette::accent()),
            })
        });

        let store = Rc::clone(&self.surface);
        let entity = cx.entity();
        div()
            .size_full()
            .flex()
            .bg(palette::window())
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
        document.take_repainted_pages();
        let count = document.page_count();
        for index in 0..count {
            let Some(painter) = document.page(index) else {
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
        self.word_result(result, cx);
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

    /// The selected cells onto the clipboard as Excel's text; with `cut`,
    /// cleared too.
    fn sheet_copy(&mut self, cut: bool, cx: &mut Context<Self>) {
        let Document::Workbook {
            book,
            anchor,
            focus,
            ..
        } = &mut self.document
        else {
            return;
        };
        let range = span(*anchor, *focus);
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(book.range_text(range)));
        if cut {
            let result = book.clear(range);
            self.edited(result, cx);
        }
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
        let (Some((x, y)), Some(target)) = (
            self.page_point(index, event.position),
            self.page_position(index, event.position),
        ) else {
            return;
        };
        let extend = event.modifiers.shift;
        self.word_move(cx, |document| {
            // Mid-composition the text under the pointer is the IME's.
            if document.composition().is_some() {
                return Ok(());
            }
            match (extend, document.selection(), document.caret()) {
                (true, Some((anchor, _)), _) | (true, None, Some(anchor)) => {
                    document.select(anchor, target)
                }
                // On the line clicked, at a wrap too.
                _ => {
                    document.place_caret_at(x, y);
                    Ok(())
                }
            }
        });
    }

    /// A drag on page `index` selects from where it was pressed.
    fn page_mouse_move(&mut self, index: usize, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if event.pressed_button != Some(MouseButton::Left) {
            return;
        }
        let Some(target) = self.page_position(index, event.position) else {
            return;
        };
        let Document::Pages { document, .. } = &self.document else {
            return;
        };
        if document.composition().is_some() {
            return;
        }
        let (anchor, focus) = match (document.selection(), document.caret()) {
            (Some(selection), _) => selection,
            (None, Some(caret)) => (caret, caret),
            _ => return,
        };
        if focus != target {
            self.word_move(cx, |document| document.select(anchor, target));
        }
    }

    /// Keys the Word view answers: deleting, splitting, and moving the caret
    /// — Shift extending the selection. Mid-composition they are the IME's.
    fn pages_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        use jade_web::editing::Motion;
        let key = event.keystroke.key.as_str();
        let extend = event.keystroke.modifiers.shift;
        let Document::Pages { document, .. } = &mut self.document else {
            return false;
        };
        if document.composition().is_some() || document.caret().is_none() {
            return false;
        }
        let motion = match key {
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
            "left" => Motion::Left,
            "right" => Motion::Right,
            "up" => Motion::Up,
            "down" => Motion::Down,
            "home" => Motion::LineStart,
            "end" => Motion::LineEnd,
            _ => return false,
        };
        self.word_move(cx, |document| document.move_caret(motion, extend));
        true
    }

    fn pages_body(&mut self, cx: &mut Context<Self>) -> gpui::Div {
        let zoom = self.zoom.unwrap_or(1.0);
        self.shown_zoom = zoom;
        let Document::Pages {
            document,
            pages,
            page_size,
            scroll,
            current,
            page_bounds,
        } = &mut self.document
        else {
            unreachable!()
        };
        let (width, height) = (page_size.0 * zoom, page_size.1 * zoom);
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
        // The page at the top third of the view is the current one.
        let offset = -f32::from(scroll.offset().y);
        let pitch = height + PAGE_GAP;
        *current = (((offset - PAGES_PADDING + pitch / 3.0) / pitch).max(0.0) as usize)
            .min(pages.len().saturating_sub(1));
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
                    .on_mouse_move(cx.listener(move |view, event, _window, cx| {
                        view.page_mouse_move(index, event, cx)
                    }))
                    .child(div().w(px(width)).h(px(height)).child(page.element()))
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
                    .child(toolbar(i18n::format(
                        "doc.position",
                        &[("current", &(current_index + 1)), ("count", &count)],
                    )))
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

    fn handle_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
        let key = event.keystroke.key.as_str();
        // ⌘ on a Mac, Ctrl elsewhere.
        if modifiers.platform || modifiers.control {
            let pages = matches!(self.document, Document::Pages { .. });
            // A sheet copies its selected cells, not the text of a cell
            // being typed into.
            let cells = matches!(self.document, Document::Workbook { edit: None, .. });
            match key {
                "z" => self.step_history(!modifiers.shift, cx),
                "y" => self.step_history(false, cx),
                "s" => self.save(cx),
                "a" if pages => self.word_move(cx, |document| document.select_all()),
                "c" if pages => self.word_copy(false, cx),
                "x" if pages => self.word_copy(true, cx),
                "c" if cells => self.sheet_copy(false, cx),
                "x" if cells => self.sheet_copy(true, cx),
                // ⌘V is left to the browser, whose paste event brings the
                // clipboard to the input handler.
                _ => return,
            }
            cx.stop_propagation();
            return;
        }
        let handled = match self.document {
            Document::Workbook { .. } => self.sheet_key(event, cx),
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
            Document::Workbook { edit: None, .. } => self.begin_cell_edit(Some(String::new()), cx),
            Document::Deck {
                edit: None,
                selected: Some(_),
                ..
            } => self.begin_slide_text(cx),
            _ => {}
        }
        match &mut self.document {
            Document::Workbook { edit, .. } => edit.as_mut(),
            Document::Deck { edit, .. } => edit.as_mut().map(|(_, text)| text),
            Document::Pages { .. } => None,
        }
    }

    fn current_text(&self) -> Option<&TextEdit> {
        match &self.document {
            Document::Workbook { edit, .. } => edit.as_ref(),
            Document::Deck { edit, .. } => edit.as_ref().map(|(_, text)| text),
            Document::Pages { .. } => None,
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

/// The name box's text: the active cell, or the selected range.
fn selection_label(anchor: CellRef, focus: CellRef) -> String {
    let range = span(anchor, focus);
    if range.start == range.end {
        reference_text(range.start)
    } else {
        format!(
            "{}:{}",
            reference_text(range.start),
            reference_text(range.end)
        )
    }
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
        // Typing appends: the caret is the end of the text.
        let text = self
            .current_text()
            .map(|edit| edit.text.as_str())
            .unwrap_or("");
        let caret = byte_to_utf16(text, text.len());
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
            Document::Workbook {
                edit: Some(edit), ..
            } => edit.marked = None,
            Document::Deck {
                edit: Some((_, edit)),
                ..
            } => edit.marked = None,
            _ => {}
        }
    }

    fn paste(&mut self, item: gpui::ClipboardItem, window: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = item.text() else {
            return;
        };
        match &mut self.document {
            Document::Pages { .. } => {
                // Lines become paragraphs, as they were when copied out.
                self.word_edit(cx, move |document| document.paste(&text));
            }
            Document::Workbook {
                book,
                edit: None,
                anchor,
                focus,
                ..
            } => {
                // Excel's clipboard text fills cells from the selection's
                // top-left, and what it filled is selected.
                let origin = span(*anchor, *focus).start;
                let result = book.paste(origin, &text);
                if let Ok(Some(far)) = result {
                    *anchor = origin;
                    *focus = far;
                }
                self.edited(result.map(|far| far.is_some()), cx);
            }
            // Into a cell being typed into, its first line, as in Excel.
            Document::Workbook { .. } => {
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
        if matches!(self.document, Document::Pages { .. }) {
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
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(self.document, Document::Pages { .. }) {
            let text = new_text.to_string();
            self.word_edit(cx, move |document| document.set_composition(&text));
            return;
        }
        if let Some(edit) = self.typing_target(cx) {
            edit.replace(range, new_text, true);
            cx.notify();
        }
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        // In a Word document, beside the caret.
        if let Document::Pages {
            document,
            page_bounds,
            ..
        } = &self.document
        {
            if let Some(caret) = document.caret_rect() {
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
                Document::Pages { .. } => self.pages_body(cx),
                Document::Deck { .. } => self.deck_body(window, cx),
                Document::Workbook { .. } => self.workbook_body(window, cx),
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

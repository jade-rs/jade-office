//! A native jade window: DOCX in, pages on screen, no browser anywhere.
//!
//! Usage: `jade-gpui-app <input.docx>`
//!
//! The pages come from `jade-docx-editor`'s own renderer via `jade-docx-native-render`,
//! so what is on screen is the same layout the browser produces. gpui's only
//! job here is to own the window and put the pixels on it.
//!
//! ## Why the pages are rasterised up front
//!
//! Rendering happens once, before the window opens, and gpui then scrolls
//! finished images. That is deliberate for a first app: it separates "can the
//! renderer drive a native window at all" from "can it repaint at interactive
//! rates", and only the first question is being answered here. Live repaint —
//! and with it editing and IME — needs the renderer driven from gpui's own
//! paint cycle, which is the next step, not this one.

mod native_print;
mod workbook;

use gpui::{
    actions, canvas, div, img, point, prelude::*, px, size, AnyElement, App, Application, Bounds,
    ClipboardItem, Context, ElementInputHandler, EntityInputHandler, FocusHandle, Focusable,
    KeyBinding, KeyDownEvent, Menu, MenuItem, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, RenderImage, ScrollHandle, SharedString, TitlebarOptions, UTF16Selection,
    Window, WindowBackgroundAppearance, WindowBounds, WindowOptions,
};
use jade_docx_native_render::{
    CaretRect, CharacterFormat, FaceSpec, NativeDocument, ObjectHit, SearchHit,
};
use std::cell::RefCell;
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

// Every command the menu offers, and every shortcut that is not plain typing.
//
// These are actions rather than arms of the key handler because a menu item and
// a shortcut have to be the same thing: gpui reads the shortcut to display from
// the keymap, so a menu built on anything else would show the wrong key, or
// none. Keys that only make sense while typing — arrows, backspace, the find
// bar's own keys — stay in the key handler, where there is nothing to show.
actions!(
    jade,
    [
        Quit,
        Open,
        Save,
        SaveAs,
        Print,
        Undo,
        Redo,
        Cut,
        Copy,
        Paste,
        SelectAll,
        SelectParagraph,
        NextPage,
        PreviousPage,
        FindAndReplace,
        FindSelection,
        FindNext,
        Replace,
        ReplaceAll,
        Bold,
        Italic,
        Underline,
        AlignLeft,
        AlignCenter,
        AlignRight,
        IndentMore,
        IndentLess,
        InsertTableRow,
        DeleteTableRow,
        AddComment,
        ToggleChangeTracking,
        ZoomIn,
        ZoomOut,
        ZoomActual,
    ]
);

/// The shortcuts, in one place, so the menu and the keyboard cannot disagree.
///
/// The context is the document view's, so a shortcut does nothing while the
/// window has no document focused rather than firing into an empty app.
fn key_bindings() -> Vec<KeyBinding> {
    const CONTEXT: Option<&str> = Some("Document");
    vec![
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-o", Open, CONTEXT),
        KeyBinding::new("cmd-s", Save, CONTEXT),
        KeyBinding::new("cmd-shift-s", SaveAs, CONTEXT),
        KeyBinding::new("cmd-p", Print, CONTEXT),
        KeyBinding::new("cmd-z", Undo, CONTEXT),
        KeyBinding::new("cmd-shift-z", Redo, CONTEXT),
        KeyBinding::new("cmd-x", Cut, CONTEXT),
        KeyBinding::new("cmd-c", Copy, CONTEXT),
        KeyBinding::new("cmd-v", Paste, CONTEXT),
        KeyBinding::new("cmd-a", SelectAll, CONTEXT),
        KeyBinding::new("cmd-alt-a", SelectParagraph, CONTEXT),
        // Page Down / Page Up rather than ⌘↓ / ⌘↑: those two mean "end of
        // document" and "start of document" on macOS, and taking them for
        // paging would be taking them from where the caret goes.
        KeyBinding::new("pagedown", NextPage, CONTEXT),
        KeyBinding::new("pageup", PreviousPage, CONTEXT),
        KeyBinding::new("cmd-f", FindAndReplace, CONTEXT),
        KeyBinding::new("cmd-e", FindSelection, CONTEXT),
        KeyBinding::new("cmd-g", FindNext, CONTEXT),
        KeyBinding::new("cmd-alt-f", Replace, CONTEXT),
        KeyBinding::new("cmd-alt-shift-f", ReplaceAll, CONTEXT),
        KeyBinding::new("cmd-b", Bold, CONTEXT),
        KeyBinding::new("cmd-i", Italic, CONTEXT),
        KeyBinding::new("cmd-u", Underline, CONTEXT),
        KeyBinding::new("cmd-shift-l", AlignLeft, CONTEXT),
        KeyBinding::new("cmd-shift-e", AlignCenter, CONTEXT),
        KeyBinding::new("cmd-shift-r", AlignRight, CONTEXT),
        KeyBinding::new("cmd-]", IndentMore, CONTEXT),
        KeyBinding::new("cmd-[", IndentLess, CONTEXT),
        KeyBinding::new("cmd-shift-enter", InsertTableRow, CONTEXT),
        KeyBinding::new("cmd-shift-backspace", DeleteTableRow, CONTEXT),
        KeyBinding::new("cmd-m", AddComment, CONTEXT),
        KeyBinding::new("cmd-shift-t", ToggleChangeTracking, CONTEXT),
        KeyBinding::new("cmd-=", ZoomIn, CONTEXT),
        KeyBinding::new("cmd--", ZoomOut, CONTEXT),
        KeyBinding::new("cmd-0", ZoomActual, CONTEXT),
    ]
}

/// The menu bar.
///
/// Everything the shell can do is here. It was all keyboard-only, which meant a
/// feature nobody had been told about did not exist.
fn menus() -> Vec<Menu> {
    vec![
        Menu {
            name: "jade".into(),
            items: vec![MenuItem::action("Quit jade", Quit)],
        },
        Menu {
            name: "File".into(),
            items: vec![
                MenuItem::action("Open…", Open),
                MenuItem::separator(),
                MenuItem::action("Save", Save),
                MenuItem::action("Save As…", SaveAs),
                MenuItem::separator(),
                MenuItem::action("Print…", Print),
            ],
        },
        Menu {
            name: "Edit".into(),
            items: vec![
                MenuItem::action("Undo", Undo),
                MenuItem::action("Redo", Redo),
                MenuItem::separator(),
                MenuItem::action("Cut", Cut),
                MenuItem::action("Copy", Copy),
                MenuItem::action("Paste", Paste),
                MenuItem::action("Select All", SelectAll),
                MenuItem::action("Select Paragraph", SelectParagraph),
                MenuItem::separator(),
                MenuItem::action("Find and Replace…", FindAndReplace),
                MenuItem::action("Find Selection", FindSelection),
                MenuItem::action("Find Next", FindNext),
                MenuItem::action("Replace…", Replace),
                MenuItem::action("Replace All", ReplaceAll),
            ],
        },
        Menu {
            name: "Format".into(),
            items: vec![
                MenuItem::action("Bold", Bold),
                MenuItem::action("Italic", Italic),
                MenuItem::action("Underline", Underline),
                MenuItem::separator(),
                MenuItem::action("Align Left", AlignLeft),
                MenuItem::action("Center", AlignCenter),
                MenuItem::action("Align Right", AlignRight),
                MenuItem::separator(),
                MenuItem::action("Increase Indent", IndentMore),
                MenuItem::action("Decrease Indent", IndentLess),
            ],
        },
        Menu {
            name: "Table".into(),
            items: vec![
                MenuItem::action("Insert Row Below", InsertTableRow),
                MenuItem::action("Delete Row", DeleteTableRow),
            ],
        },
        Menu {
            name: "Review".into(),
            items: vec![
                MenuItem::action("Add Comment", AddComment),
                MenuItem::action("Track Changes", ToggleChangeTracking),
            ],
        },
        Menu {
            name: "View".into(),
            items: vec![
                MenuItem::action("Next Page", NextPage),
                MenuItem::action("Previous Page", PreviousPage),
                MenuItem::separator(),
                MenuItem::action("Zoom In", ZoomIn),
                MenuItem::action("Zoom Out", ZoomOut),
                MenuItem::action("Actual Size", ZoomActual),
            ],
        },
    ]
}

/// The font the shell draws its own labels in.
///
/// gpui's default is `.SystemUIFont`, which carries no Japanese: every label
/// here would be blank. `JADE_FONT_PROBE=1` reports what a family is missing.
const UI_FONT_FAMILY: &str = "Hiragino Sans";

/// The page box to open with. The document's own section properties win — this
/// is only what `open` starts from before it finds out what the document wants,
/// so nothing else in the shell should use it.
const INITIAL_PAGE_WIDTH_PX: f64 = 816.0;
const INITIAL_PAGE_HEIGHT_PX: f64 = 1056.0;

/// Families a document is likely to name, aliased onto the bundled face.
///
/// Real font matching is its own problem; without this the first render is
/// blank, which would look like a renderer failure rather than a missing font.
/// Where typed text lands until there is a caret.
///
/// It has to be on the page the repaint is bounded to. Paragraph 30 is on page
/// 2 of the sample contract, and typing there while bounding layout to page 1
/// produced a full, correct-looking repaint of page 1 that never changed —
/// timings and all.
const INSERT_PARAGRAPH: usize = 6;

/// A face carrying the arrows Word uses for formatting marks.
const SYMBOL_FALLBACK: &str = "/System/Library/Fonts/Apple Symbols.ttf";

const ALIASED_FAMILIES: &[&str] = &[
    "Noto Sans JP",
    "Times New Roman",
    "Yu Mincho",
    "MS Mincho",
    "MS Gothic",
    "serif",
];

fn bundled_font(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../jade-core/fonts/Noto_Sans_JP/static")
        .join(name)
}

/// Opens `path` and paints it, ready to be edited.
fn open_document(path: &str) -> Result<NativeDocument, String> {
    let bytes = std::fs::read(path).map_err(|err| format!("{path}: {err}"))?;
    let regular = bundled_font("NotoSansJP-Regular.ttf");
    let bold = bundled_font("NotoSansJP-Bold.ttf");
    let mut faces = Vec::new();
    for family in ALIASED_FAMILIES {
        faces.push(FaceSpec {
            family,
            weight: 400,
            path: &regular,
        });
        faces.push(FaceSpec {
            family,
            weight: 700,
            path: &bold,
        });
    }
    // Word's line- and page-break marks are U+21B5 and U+21D3, which a Japanese
    // text face does not carry. Registered so the painter's per-character
    // fallback has somewhere to go; without it those marks draw as tofu, which
    // is why they used to be switched off entirely.
    let symbols = PathBuf::from(SYMBOL_FALLBACK);
    if symbols.exists() {
        faces.push(FaceSpec {
            family: "Symbol Fallback",
            weight: 400,
            path: &symbols,
        });
    }
    NativeDocument::open(
        &bytes,
        INITIAL_PAGE_WIDTH_PX,
        INITIAL_PAGE_HEIGHT_PX,
        &faces,
    )
}

/// Reports what the text system resolves the shell's labels to.
///
/// Text that does not appear is either a font that did not resolve or glyphs
/// the resolved font does not carry, and neither is visible from the outside:
/// gpui logs nothing and draws nothing.
fn probe_fonts(cx: &mut App) {
    use gpui::{Font, FontFeatures, FontStyle, FontWeight};

    /// Every character the shell puts on screen itself. The document's own text
    /// is drawn by jade, not by gpui, so this is the whole set that has to have
    /// glyphs in the UI font.
    const LABELS: &str = "貼付切取複写大小下上文字色蛍光書式解除箇条書き段落番号\
        字下げ－＋左中央右両端戻すやり直し検索表空白ページ改行コメント目次記録\
        すべて反映取消編集記号縮拡等倍前へ次ホーム挿入レイアウト参照設定校閲表示\
        クリップボードスタイル游明朝ゴシック置換見出し標準検索置換フィールド\
        Word 出力 0123456789×%.:¶";

    let names = cx.text_system().all_font_names();
    println!("system knows {} font names", names.len());

    for family in [".SystemUIFont", UI_FONT_FAMILY] {
        let font = Font {
            family: family.into(),
            features: FontFeatures::default(),
            fallbacks: None,
            weight: FontWeight::default(),
            style: FontStyle::default(),
        };
        let font_id = cx.text_system().resolve_font(&font);
        let resolved = cx
            .text_system()
            .get_font_for_id(font_id)
            .map(|font| font.family.to_string())
            .unwrap_or_else(|| "?".to_string());
        // A font that resolves is not a font that has the glyph, and a missing
        // glyph is exactly what an empty label looks like from the outside.
        let missing: String = LABELS
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .filter(|ch| cx.text_system().advance(font_id, px(14.0), *ch).is_err())
            .collect();
        println!(
            "{family:>16} -> {resolved:<20} missing: {}",
            if missing.is_empty() {
                "none".to_string()
            } else {
                missing
            },
        );
    }
}

/// Turns a page's pixels into something gpui can draw.
///
/// `RenderImage` holds BGRA, which is why the surface is read in that order
/// rather than swapped here.
fn page_image(document: &NativeDocument, page_index: usize) -> Option<Arc<RenderImage>> {
    let (bgra, width, height) = document.page_pixels(page_index)?;
    let buffer = image::ImageBuffer::from_raw(width as u32, height as u32, bgra)?;
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
}

/// One dark palette in one place, so a colour is named by its job rather than
/// repeated as a literal at each use.
pub(crate) mod theme {
    use gpui::{rgb, rgba, Rgba};

    /// The window body. Slightly translucent so the desktop tints through,
    /// which is what makes the rounded corners read as a floating panel rather
    /// than a rectangle with clipped edges.
    pub fn window() -> Rgba {
        rgba(0x191320f2)
    }
    pub fn sidebar() -> Rgba {
        rgba(0x1f1729f2)
    }
    /// Darker than the chrome so the white pages sit in a well.
    pub fn canvas() -> Rgba {
        rgba(0x120e18f7)
    }
    pub fn panel() -> Rgba {
        rgba(0x1f1729f2)
    }
    pub fn border() -> Rgba {
        rgba(0xffffff12)
    }
    pub fn selected() -> Rgba {
        rgba(0xffffff16)
    }
    pub fn hovered() -> Rgba {
        rgba(0xffffff0b)
    }
    pub fn text() -> Rgba {
        rgb(0xece8f2)
    }
    pub fn text_dim() -> Rgba {
        rgb(0xa79fb4)
    }
    pub fn text_faint() -> Rgba {
        rgb(0x6f6879)
    }
    pub fn accent() -> Rgba {
        rgb(0xc9a6f5)
    }
    /// Solid and dark: it sits on the white page, not on the app chrome.
    pub fn caret() -> Rgba {
        rgb(0x1b1b1f)
    }
}

/// The ribbon's tabs, in Word's order.
///
/// The browser shell shows these seven, and a tab that is present but empty is
/// worse than one that is not: each of these has something under it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RibbonTab {
    Home,
    Insert,
    Design,
    Layout,
    References,
    Review,
    View,
}

impl RibbonTab {
    const ALL: [RibbonTab; 7] = [
        RibbonTab::Home,
        RibbonTab::Insert,
        RibbonTab::Design,
        RibbonTab::Layout,
        RibbonTab::References,
        RibbonTab::Review,
        RibbonTab::View,
    ];

    fn label(self) -> &'static str {
        match self {
            RibbonTab::Home => "ホーム",
            RibbonTab::Insert => "挿入",
            RibbonTab::Design => "デザイン",
            RibbonTab::Layout => "レイアウト",
            RibbonTab::References => "参照設定",
            RibbonTab::Review => "校閲",
            RibbonTab::View => "表示",
        }
    }
}

/// The lists the ribbon can drop under itself.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RibbonPopup {
    FontFamily,
    FontColor,
    Highlight,
    LineSpacing,
    Shading,
}

/// The find bar's two fields.
///
/// The bar is a real text field rather than "search for the selection": the
/// query can be something that is not in the document yet, which is exactly
/// what replace needs.
#[derive(Default)]
struct FindBar {
    query: String,
    replacement: String,
    /// Which field typing lands in. Tab moves between them.
    editing_replacement: bool,
    /// The composition in progress inside the focused field, as a byte range.
    /// The IME works here the same as it does in the document.
    marked: Option<Range<usize>>,
}

impl FindBar {
    /// The field being typed into.
    fn field(&self) -> &String {
        if self.editing_replacement {
            &self.replacement
        } else {
            &self.query
        }
    }

    fn field_mut(&mut self) -> &mut String {
        if self.editing_replacement {
            &mut self.replacement
        } else {
            &mut self.query
        }
    }
}

/// A picture or shape being dragged, and what the drag is doing to it.
struct ObjectDrag {
    object: ObjectHit,
    /// The rectangle the object had when the drag started. Deltas are measured
    /// from here rather than accumulated, so a drag that crosses a repaint does
    /// not drift.
    start: (f64, f64, f64, f64),
    /// Where the pointer went down, in page coordinates.
    from: (f64, f64),
    /// A grab near the bottom-right corner resizes; anywhere else moves.
    resizing: bool,
    /// Whether a transform from this drag is already on the undo stack. The
    /// next move replaces it rather than stacking on it, so the whole drag is
    /// one undo.
    applied: bool,
}

struct DocumentView {
    title: SharedString,
    path: SharedString,
    file_size: u64,
    document: NativeDocument,
    /// One image per page, rebuilt only for the page an edit touched.
    images: Vec<Arc<RenderImage>>,
    /// Where to write the session when `JADE_DUMP_PAGE` names a directory,
    /// resolved once at startup rather than looked up on every repaint.
    ///
    /// It stays in the release binary deliberately: the window cannot be
    /// captured from outside it, so when something is drawn wrong this is the
    /// only way to see what the shell drew — and that is never the moment to
    /// discover the diagnostic was compiled out.
    dump_directory: Option<PathBuf>,
    /// What the shell has asked the document to do, most recent last, while
    /// dumping. A page shows what went wrong; this shows which call did it, and
    /// with the exported document it is enough to replay the session in a test.
    call_log: Vec<String>,
    /// Images that have been replaced and are waiting to be dropped from gpui's
    /// texture atlas. Dropping needs the window, which an edit does not have,
    /// so they are collected here and released at the next render.
    retired_images: Vec<Arc<RenderImage>>,
    /// Where typed text goes, as a paragraph and a byte offset. Set by
    /// clicking; starts at a paragraph on the first page so typing works before
    /// the first click.
    insert_at: (usize, usize),
    /// The file the last save went to, shown so a save is visibly a save.
    saved_to: Option<SharedString>,
    /// Whether edits are being recorded as revisions.
    tracking_changes: bool,
    /// Why the last table command did nothing, if it did nothing.
    table_status: Option<SharedString>,
    /// The last search query and where it got to, shown in the panel.
    find_query: Option<String>,
    find_status: Option<SharedString>,
    /// Which ribbon tab is showing.
    ribbon_tab: RibbonTab,
    /// Which ribbon popup is open, if any. A list under the ribbon rather than
    /// a native menu, because gpui has no menu to drop down inside a window.
    popup: Option<RibbonPopup>,
    /// The font size the size buttons last set, so + and − are relative to what
    /// was asked for rather than to whatever the run happens to carry.
    font_size_pt: f32,
    /// The family the picker last applied, shown in the ribbon.
    font_family: SharedString,
    /// The find bar, while it is open. Open, it takes the keyboard: typing goes
    /// into its field rather than into the document.
    find_bar: Option<FindBar>,
    /// Left indent in points. The command sets an absolute value, so the shell
    /// tracks what it last asked for to make ⌘] and ⌘[ relative.
    indent_points: f32,
    /// Display scale. The pages are rasterised at their own size and drawn
    /// scaled, so zooming costs a layout pass in gpui rather than a re-render —
    /// at the price of resampling rather than re-rasterising crisply.
    zoom: f32,
    /// Where a shift-extended selection is anchored. Separate from
    /// `drag_anchor`: a keyboard selection outlives the mouse being down.
    selection_anchor: Option<(usize, usize)>,
    /// Where a drag started, while the mouse is down. Selection is anchored
    /// here and extended to wherever the pointer is.
    drag_anchor: Option<(usize, usize)>,
    /// The picture or shape being dragged, if the mouse went down on one.
    /// Pictures are not in the text flow, so dragging one is a move, not a
    /// selection.
    object_drag: Option<ObjectDrag>,
    /// The byte range of the composition in the current paragraph, while one is
    /// in progress. The IME replaces it in place as the reading changes.
    marked: Option<Range<usize>>,
    /// Where to draw the caret, in document coordinates, or `None` if the
    /// insertion point has no geometry (an empty or unlaid paragraph).
    caret: Option<CaretRect>,
    /// Each page's on-screen rectangle, recorded during paint so a click can be
    /// turned into a point on the page. gpui reports a click in window
    /// coordinates and the renderer hit-tests in document ones.
    page_bounds: Rc<RefCell<Vec<Option<Bounds<Pixels>>>>>,
    /// How long the last repaint took, shown so the cost of a keystroke is
    /// visible rather than claimed.
    last_repaint: Option<Duration>,
    typed: usize,
    current_page: usize,
    scroll: ScrollHandle,
    focus: FocusHandle,
}

impl DocumentView {
    /// The page most of the viewport is showing, from the scroll offset.
    ///
    /// The sidebar highlight followed clicks only, so scrolling left it
    /// pointing at wherever you last clicked.
    fn page_from_scroll(&self) -> usize {
        // Scroll is in screen pixels and the pitch is in document pixels.
        let offset = -f64::from(self.scroll.offset().y) / f64::from(self.zoom);
        let pitch = self.document.page_pitch();
        // Half a page in, so the highlight changes when a page takes over the
        // view rather than the moment its top edge appears.
        (((offset + self.document.layout_page_size().1 / 2.0) / pitch)
            .floor()
            .max(0.0) as usize)
            .min(self.images.len().saturating_sub(1))
    }

    /// The page the caret is on, 1-based.
    ///
    /// A repaint is bounded to this, so it has to follow the caret: bounding to
    /// page 1 while editing page 2 gives a repaint that runs, reports a time,
    /// and draws an unchanged page 1.
    fn caret_page_1idx(&self) -> usize {
        self.caret
            .map(|caret| self.document.page_and_offset(caret.document_y).0 + 1)
            .unwrap_or(1)
    }

    /// Refreshes the image for the page the caret is on. The others are
    /// untouched by an edit bounded to it.
    /// Re-reads the page the caret is on, and the one after it.
    ///
    /// An edit that makes a line wrap — a table cell growing a second line, a
    /// paragraph gaining a line — moves everything below it, and the bottom of
    /// that movement is on the next page. Refreshing only the caret's page
    /// leaves the top of the next one showing what used to be there.
    /// `set_page_image` compares before it replaces, so a page that did not
    /// change costs a read and no upload.
    fn refresh_caret_page_image(&mut self) {
        self.dump_page_if_asked();
        let page = self.caret_page_1idx().saturating_sub(1);
        self.set_page_image(page);
        if page + 1 < self.images.len() {
            self.set_page_image(page + 1);
        }
    }

    /// Writes the page the caret is on to `$JADE_DUMP_PAGE/app-page.png`.
    ///
    /// The window cannot be captured from outside it, so this is how what the
    /// app actually draws gets looked at: set the variable, reproduce whatever
    /// is wrong, and the page is on disk.
    /// Records one document call for the dump. Free when the dump is off.
    fn log_call(&mut self, call: String) {
        if self.dump_directory.is_none() {
            return;
        }
        if self.call_log.len() >= 200 {
            self.call_log.remove(0);
        }
        self.call_log.push(call);
    }

    fn dump_page_if_asked(&self) {
        let Some(directory) = self.dump_directory.as_ref().map(|path| path.display()) else {
            return;
        };
        let page = self.caret_page_1idx().saturating_sub(1);
        let Some((bgra, width, height)) = self.document.page_pixels(page) else {
            return;
        };
        let mut rgba = Vec::with_capacity(bgra.len());
        for pixel in bgra.chunks_exact(4) {
            rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
        }
        let Some(buffer) = image::RgbaImage::from_raw(width as u32, height as u32, rgba) else {
            return;
        };
        let path = format!("{directory}/app-page.png");
        if let Err(error) = buffer.save(&path) {
            eprintln!("dump failed: {error}");
        }

        // The page alone says what is wrong; this says what produced it. The
        // same document API a test drives, so the two can be compared call for
        // call.
        let state = format!(
            "caret paragraph {:?}\ncaret cell {:?}\ncaret rect {:?}\n             paragraph text {:?}\npages {} bound {}\nzoom {}\n             marks {}\ntable {:#?}\n",
            self.insert_at,
            self.document.caret_table_cell(),
            self.caret,
            self.document.paragraph_text(self.insert_at.0),
            self.document.page_count(),
            self.caret_page_1idx(),
            self.zoom,
            self.document.formatting_marks(),
            self.document.table_snapshot(0),
        );
        if let Err(error) = std::fs::write(
            format!("{directory}/app-state.txt"),
            format!("{state}\ncalls:\n{}\n", self.call_log.join("\n")),
        ) {
            eprintln!("state dump failed: {error}");
        }
        // The document itself, so what the shell is looking at can be opened
        // and rendered somewhere the window is not in the way.
        if let Ok(bytes) = self.document.export_docx() {
            let _ = std::fs::write(format!("{directory}/app-document.docx"), bytes);
        }
    }

    /// Re-reads one page, unless it is unchanged.
    ///
    /// Every replacement is a texture for gpui to upload and an old one to
    /// retire, and most repaints leave most pages exactly as they were —
    /// comparing the pixels costs a fraction of what uploading them does.
    fn set_page_image(&mut self, index: usize) {
        let Some((bgra, width, height)) = self.document.page_pixels(index) else {
            return;
        };
        if let Some(current) = self.images.get(index) {
            if current.as_bytes(0) == Some(bgra.as_slice()) {
                return;
            }
        }
        let Some(buffer) = image::ImageBuffer::from_raw(width as u32, height as u32, bgra) else {
            return;
        };
        let image = Arc::new(RenderImage::new(vec![image::Frame::new(buffer)]));
        if let Some(slot) = self.images.get_mut(index) {
            // The replaced texture stays in gpui's atlas until it is dropped,
            // and a page-sized one is megabytes. Left to accumulate — which is
            // what a keystroke used to do — the atlas grows without bound.
            let retired = std::mem::replace(slot, image);
            self.retired_images.push(retired);
        } else if index == self.images.len() {
            self.images.push(image);
        }
    }

    /// Keys that are not text.
    ///
    /// Typed characters arrive through the input handler instead — that is what
    /// lets a composition exist — so this only handles the keys an IME never
    /// sees.
    fn handle_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        // A composition owns the keyboard while it is running; letting these
        // through would edit around the preedit.
        if self.marked.is_some() {
            return;
        }
        // The bar owns the keyboard while it is open, so Enter means "next
        // match" and Backspace edits the query rather than the document.
        if self.find_bar.is_some() && self.handle_find_bar_key(event, cx) {
            return;
        }
        // Every command with a menu item is an action, bound in `key_bindings`
        // and handled on the root element. gpui dispatches a matched binding
        // before the key handler and stops there, so an arm for one of them
        // here would be dead — and, if it ever ran, would run it twice.

        // A selection is what backspace and delete act on when there is one:
        // removing a character next to it instead would leave the selection
        // behind and surprise anyone who just dragged over a phrase.
        if self.document.selection().is_some()
            && matches!(event.keystroke.key.as_str(), "backspace" | "delete")
        {
            let page = self.caret_page_1idx();
            match self.document.replace_selection("", page) {
                Ok(caret) => {
                    self.insert_at = caret;
                    self.caret = self.document.place_caret(caret.0, caret.1);
                    self.rebuild_all_images();
                    cx.notify();
                }
                Err(error) => eprintln!("delete failed: {error}"),
            }
            return;
        }

        let (paragraph, offset) = self.insert_at;
        match event.keystroke.key.as_str() {
            "backspace" if offset > 0 => {
                let text = self.current_paragraph_text();
                let removed = text[..offset].chars().next_back().map(char::len_utf8);
                if let Some(removed) = removed {
                    let page = self.caret_page_1idx();
                    let result = self.document.delete_text(paragraph, offset, true, 1, page);
                    self.after_edit(result, (paragraph, offset - removed), cx);
                }
            }
            "delete" => {
                let page = self.caret_page_1idx();
                let result = self.document.delete_text(paragraph, offset, false, 1, page);
                self.after_edit(result, (paragraph, offset), cx);
            }
            "enter" => {
                // The split re-flows everything below, so the whole document is
                // repainted rather than the visible page.
                let result = self.document.insert_paragraph_break(paragraph, offset);
                self.rebuild_all_images();
                self.after_edit(result, (paragraph + 1, 0), cx);
            }
            "left" | "right" | "up" | "down" => {
                let extending = event.keystroke.modifiers.shift;
                // Anchored where the caret was before this keystroke, unless a
                // shift-selection is already running.
                let anchor = if extending {
                    self.selection_anchor.unwrap_or(self.insert_at)
                } else {
                    self.selection_anchor = None;
                    self.insert_at
                };
                match event.keystroke.key.as_str() {
                    "left" => self.step_caret(-1, cx),
                    "right" => self.step_caret(1, cx),
                    "up" => self.step_line(-1.0, cx),
                    _ => self.step_line(1.0, cx),
                }
                if extending {
                    self.extend_selection_to_caret(anchor, cx);
                }
            }
            _ => {}
        }
    }

    // ---- the ribbon ------------------------------------------------------

    /// One ribbon button: a glyph over nothing, with the command as its title.
    ///
    /// Text rather than icons: the shell has no asset pipeline, and a labelled
    /// glyph says what the button does without one.
    fn ribbon_button(
        &self,
        id: &'static str,
        glyph: &str,
        _title: &str,
        active: bool,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        div()
            .id(id)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .min_w(px(30.0))
            .h(px(28.0))
            .px_2()
            .whitespace_nowrap()
            .rounded_md()
            .when(active, |button| button.bg(theme::selected()))
            .hover(|button| button.bg(theme::hovered()))
            .cursor_pointer()
            .text_sm()
            .text_color(theme::text())
            .child(glyph.to_string())
            .on_click(cx.listener(move |view, _event, _window, cx| action(view, cx)))
    }

    /// A group of buttons with Word's caption under it.
    fn ribbon_group(label: &str, rows: Vec<AnyElement>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .flex_none()
            .items_center()
            .gap_1()
            .px_3()
            .border_r_1()
            .border_color(theme::border())
            .child(div().flex().flex_col().gap_1().children(rows))
            .child(
                div()
                    .text_xs()
                    .text_color(theme::text_faint())
                    .child(label.to_string()),
            )
    }

    /// A row inside a group.
    fn ribbon_row(children: Vec<AnyElement>) -> AnyElement {
        div()
            .flex()
            .items_center()
            .gap_1()
            .children(children)
            .into_any_element()
    }

    /// The tab strip, and the tab's own groups under it.
    fn ribbon(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.ribbon_tab;
        div()
            .flex()
            .flex_col()
            .flex_none()
            .bg(theme::panel())
            .border_b_1()
            .border_color(theme::border())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_4()
                    .pt(px(8.0))
                    .children(RibbonTab::ALL.map(|tab| {
                        div()
                            .id(tab.label())
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .cursor_pointer()
                            .text_sm()
                            .when(tab == active, |element| {
                                element.bg(theme::selected()).text_color(theme::text())
                            })
                            .when(tab != active, |element| {
                                element.text_color(theme::text_dim())
                            })
                            .hover(|element| element.bg(theme::hovered()))
                            .child(tab.label())
                            .on_click(cx.listener(move |view, _event, _window, cx| {
                                view.ribbon_tab = tab;
                                view.popup = None;
                                cx.notify();
                            }))
                    }))
                    // Word 出力 sits at the right of the strip, as it does in
                    // the browser shell.
                    .child(
                        div().flex_1().child(
                            div()
                                .id("export")
                                .ml_auto()
                                .w(px(104.0))
                                .px_3()
                                .py_1()
                                .rounded_md()
                                .border_1()
                                .border_color(theme::border())
                                .cursor_pointer()
                                .hover(|button| button.bg(theme::hovered()))
                                .text_sm()
                                .text_color(theme::text())
                                .child("Word 出力")
                                .on_click(cx.listener(|view, _event, _window, cx| view.save(cx))),
                        ),
                    ),
            )
            .child(
                div()
                    .id("ribbon-groups")
                    .flex()
                    .items_start()
                    .gap_1()
                    .px_4()
                    .py_2()
                    // Scrolls rather than being cut off: the groups are wider
                    // than a narrow window, and a group that is off the edge is
                    // a command nobody can reach.
                    .overflow_x_scroll()
                    .children(self.ribbon_groups(cx)),
            )
            .children(self.ribbon_popup(cx))
    }

    /// The groups under the selected tab.
    fn ribbon_groups(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        match self.ribbon_tab {
            RibbonTab::Home => self.home_groups(cx),
            RibbonTab::Insert => vec![
                Self::ribbon_group(
                    "表",
                    vec![Self::ribbon_row(vec![
                        self.ribbon_button(
                            "table-3x3",
                            "表 3×3",
                            "3×3 の表",
                            false,
                            cx,
                            |view, cx| view.insert_table(3, 3, cx),
                        )
                        .into_any_element(),
                        self.ribbon_button(
                            "table-2x2",
                            "表 2×2",
                            "2×2 の表",
                            false,
                            cx,
                            |view, cx| view.insert_table(2, 2, cx),
                        )
                        .into_any_element(),
                    ])],
                )
                .into_any_element(),
                Self::ribbon_group(
                    "ページ",
                    vec![Self::ribbon_row(vec![self
                        .ribbon_button(
                            "blank-page",
                            "空白ページ",
                            "空白ページ",
                            false,
                            cx,
                            |view, cx| view.insert_blank_page(cx),
                        )
                        .into_any_element()])],
                )
                .into_any_element(),
                Self::ribbon_group(
                    "コメント",
                    vec![Self::ribbon_row(vec![self
                        .ribbon_button(
                            "comment",
                            "コメント",
                            "コメント",
                            false,
                            cx,
                            |view, cx| view.add_comment(cx),
                        )
                        .into_any_element()])],
                )
                .into_any_element(),
            ],
            RibbonTab::Design => vec![
                Self::ribbon_group(
                    "ページの色",
                    vec![Self::ribbon_row(vec![
                        self.ribbon_button(
                            "page-cream",
                            "クリーム",
                            "ページの色",
                            false,
                            cx,
                            |view, cx| view.set_page_background(Some("FFF9E6"), cx),
                        )
                        .into_any_element(),
                        self.ribbon_button(
                            "page-grey",
                            "薄灰",
                            "ページの色",
                            false,
                            cx,
                            |view, cx| view.set_page_background(Some("F2F2F2"), cx),
                        )
                        .into_any_element(),
                        self.ribbon_button(
                            "page-white",
                            "なし",
                            "ページの色を消す",
                            false,
                            cx,
                            |view, cx| view.set_page_background(None, cx),
                        )
                        .into_any_element(),
                    ])],
                )
                .into_any_element(),
                Self::ribbon_group(
                    "ページ罫線",
                    vec![Self::ribbon_row(vec![
                        self.ribbon_button(
                            "page-border",
                            "囲む",
                            "ページを罫線で囲む",
                            false,
                            cx,
                            |view, cx| view.set_page_borders(true, cx),
                        )
                        .into_any_element(),
                        self.ribbon_button(
                            "page-no-border",
                            "なし",
                            "ページの罫線を消す",
                            false,
                            cx,
                            |view, cx| view.set_page_borders(false, cx),
                        )
                        .into_any_element(),
                    ])],
                )
                .into_any_element(),
            ],
            RibbonTab::Layout => vec![
                Self::ribbon_group(
                    "段落",
                    vec![Self::ribbon_row(vec![
                        self.ribbon_button(
                            "layout-indent-less",
                            "字下げ－",
                            "インデントを減らす",
                            false,
                            cx,
                            |view, cx| view.indent(-36.0, cx),
                        )
                        .into_any_element(),
                        self.ribbon_button(
                            "layout-indent-more",
                            "字下げ＋",
                            "インデントを増やす",
                            false,
                            cx,
                            |view, cx| view.indent(36.0, cx),
                        )
                        .into_any_element(),
                    ])],
                )
                .into_any_element(),
                Self::ribbon_group(
                    "改ページ",
                    vec![Self::ribbon_row(vec![self
                        .ribbon_button(
                            "layout-break",
                            "改ページ",
                            "改ページ",
                            false,
                            cx,
                            |view, cx| view.insert_blank_page(cx),
                        )
                        .into_any_element()])],
                )
                .into_any_element(),
            ],
            RibbonTab::References => vec![Self::ribbon_group(
                "目次",
                vec![Self::ribbon_row(vec![self
                    .ribbon_button("toc", "目次", "目次を挿入", false, cx, |view, cx| {
                        view.insert_table_of_contents(cx)
                    })
                    .into_any_element()])],
            )
            .into_any_element()],
            RibbonTab::Review => {
                let tracking = self.tracking_changes;
                vec![
                    Self::ribbon_group(
                        "コメント",
                        vec![Self::ribbon_row(vec![self
                            .ribbon_button(
                                "review-comment",
                                "コメント",
                                "コメント",
                                false,
                                cx,
                                |view, cx| view.add_comment(cx),
                            )
                            .into_any_element()])],
                    )
                    .into_any_element(),
                    Self::ribbon_group(
                        "変更履歴",
                        vec![Self::ribbon_row(vec![
                            self.ribbon_button(
                                "track",
                                "記録",
                                "変更履歴の記録",
                                tracking,
                                cx,
                                |view, cx| view.toggle_tracking(cx),
                            )
                            .into_any_element(),
                            self.ribbon_button(
                                "accept",
                                "すべて反映",
                                "すべて反映",
                                false,
                                cx,
                                |view, cx| view.resolve_revisions(true, cx),
                            )
                            .into_any_element(),
                            self.ribbon_button(
                                "reject",
                                "すべて取消",
                                "すべて元に戻す",
                                false,
                                cx,
                                |view, cx| view.resolve_revisions(false, cx),
                            )
                            .into_any_element(),
                        ])],
                    )
                    .into_any_element(),
                ]
            }
            RibbonTab::View => {
                let marks = self.document.formatting_marks();
                vec![
                    Self::ribbon_group(
                        "表示",
                        vec![Self::ribbon_row(vec![self
                            .ribbon_button(
                                "marks",
                                "編集記号",
                                "編集記号の表示",
                                marks,
                                cx,
                                |view, cx| view.toggle_formatting_marks(cx),
                            )
                            .into_any_element()])],
                    )
                    .into_any_element(),
                    Self::ribbon_group(
                        "ズーム",
                        vec![Self::ribbon_row(vec![
                            self.ribbon_button(
                                "zoom-out",
                                "縮小",
                                "縮小",
                                false,
                                cx,
                                |view, cx| {
                                    let zoom = view.zoom / 1.1;
                                    view.set_zoom(zoom, cx)
                                },
                            )
                            .into_any_element(),
                            self.ribbon_button(
                                "zoom-reset",
                                "等倍",
                                "等倍",
                                false,
                                cx,
                                |view, cx| view.set_zoom(1.0, cx),
                            )
                            .into_any_element(),
                            self.ribbon_button(
                                "zoom-in",
                                "拡大",
                                "拡大",
                                false,
                                cx,
                                |view, cx| {
                                    let zoom = view.zoom * 1.1;
                                    view.set_zoom(zoom, cx)
                                },
                            )
                            .into_any_element(),
                        ])],
                    )
                    .into_any_element(),
                    Self::ribbon_group(
                        "ページ",
                        vec![Self::ribbon_row(vec![
                            self.ribbon_button(
                                "prev-page",
                                "前へ",
                                "前のページ",
                                false,
                                cx,
                                |view, cx| view.go_to_page(-1, cx),
                            )
                            .into_any_element(),
                            self.ribbon_button(
                                "next-page",
                                "次へ",
                                "次のページ",
                                false,
                                cx,
                                |view, cx| view.go_to_page(1, cx),
                            )
                            .into_any_element(),
                        ])],
                    )
                    .into_any_element(),
                ]
            }
        }
    }

    /// The ホーム tab: what the browser shell puts there.
    fn home_groups(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let family = self.font_family.clone();
        let size = self.font_size_pt;
        vec![
            Self::ribbon_group(
                "クリップボード",
                vec![Self::ribbon_row(vec![
                    self.ribbon_button("paste", "貼付", "ペースト", false, cx, |view, cx| {
                        view.paste(cx)
                    })
                    .into_any_element(),
                    self.ribbon_button("cut", "切取", "切り取り", false, cx, |view, cx| {
                        view.cut(cx)
                    })
                    .into_any_element(),
                    self.ribbon_button("copy", "複写", "コピー", false, cx, |view, cx| {
                        view.copy(cx)
                    })
                    .into_any_element(),
                ])],
            )
            .into_any_element(),
            Self::ribbon_group(
                "フォント",
                vec![
                    Self::ribbon_row(vec![
                        // The family picker: a button showing the family, and a
                        // list that opens under the ribbon.
                        div()
                            .id("family")
                            .flex()
                            .items_center()
                            .w(px(120.0))
                            .h(px(24.0))
                            .px_2()
                            .rounded_md()
                            .border_1()
                            .border_color(theme::border())
                            .cursor_pointer()
                            .hover(|button| button.bg(theme::hovered()))
                            .text_sm()
                            .text_color(theme::text())
                            .child(family)
                            .on_click(cx.listener(|view, _event, _window, cx| {
                                view.popup = (view.popup != Some(RibbonPopup::FontFamily))
                                    .then_some(RibbonPopup::FontFamily);
                                cx.notify();
                            }))
                            .into_any_element(),
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(44.0))
                            .h(px(24.0))
                            .rounded_md()
                            .border_1()
                            .border_color(theme::border())
                            .text_sm()
                            .text_color(theme::text_dim())
                            .child(format!("{size:.1}"))
                            .into_any_element(),
                        self.ribbon_button(
                            "size-up",
                            "大",
                            "フォントサイズの拡大",
                            false,
                            cx,
                            |view, cx| {
                                let size = view.font_size_pt + 1.0;
                                view.set_font_size(size, cx)
                            },
                        )
                        .into_any_element(),
                        self.ribbon_button(
                            "size-down",
                            "小",
                            "フォントサイズの縮小",
                            false,
                            cx,
                            |view, cx| {
                                let size = view.font_size_pt - 1.0;
                                view.set_font_size(size, cx)
                            },
                        )
                        .into_any_element(),
                    ]),
                    Self::ribbon_row(vec![
                        self.ribbon_button("bold", "B", "太字", false, cx, |view, cx| {
                            view.format(CharacterFormat::Bold, cx)
                        })
                        .into_any_element(),
                        self.ribbon_button("italic", "I", "斜体", false, cx, |view, cx| {
                            view.format(CharacterFormat::Italic, cx)
                        })
                        .into_any_element(),
                        self.ribbon_button("underline", "U", "下線", false, cx, |view, cx| {
                            view.format(CharacterFormat::Underline, cx)
                        })
                        .into_any_element(),
                        self.ribbon_button(
                            "strike",
                            "S",
                            "取り消し線",
                            false,
                            cx,
                            |view, cx| view.format(CharacterFormat::Strikethrough, cx),
                        )
                        .into_any_element(),
                        self.ribbon_button("sub", "下付", "下付き", false, cx, |view, cx| {
                            view.format(CharacterFormat::Subscript, cx)
                        })
                        .into_any_element(),
                        self.ribbon_button("sup", "上付", "上付き", false, cx, |view, cx| {
                            view.format(CharacterFormat::Superscript, cx)
                        })
                        .into_any_element(),
                        self.ribbon_button(
                            "colour",
                            "文字色",
                            "フォントの色",
                            self.popup == Some(RibbonPopup::FontColor),
                            cx,
                            |view, cx| view.toggle_popup(RibbonPopup::FontColor, cx),
                        )
                        .into_any_element(),
                        self.ribbon_button(
                            "highlight",
                            "蛍光",
                            "蛍光ペン",
                            self.popup == Some(RibbonPopup::Highlight),
                            cx,
                            |view, cx| view.toggle_popup(RibbonPopup::Highlight, cx),
                        )
                        .into_any_element(),
                        self.ribbon_button(
                            "clear",
                            "書式解除",
                            "書式のクリア",
                            false,
                            cx,
                            |view, cx| view.format(CharacterFormat::Clear, cx),
                        )
                        .into_any_element(),
                    ]),
                ],
            )
            .into_any_element(),
            Self::ribbon_group(
                "段落",
                vec![
                    Self::ribbon_row(vec![
                        self.ribbon_button(
                            "bullets",
                            "箇条書き",
                            "箇条書き",
                            false,
                            cx,
                            |view, cx| view.set_numbering(Some("1"), cx),
                        )
                        .into_any_element(),
                        self.ribbon_button(
                            "numbering",
                            "段落番号",
                            "段落番号",
                            false,
                            cx,
                            |view, cx| view.set_numbering(Some("2"), cx),
                        )
                        .into_any_element(),
                        self.ribbon_button(
                            "no-list",
                            "解除",
                            "リストの解除",
                            false,
                            cx,
                            |view, cx| view.set_numbering(None, cx),
                        )
                        .into_any_element(),
                        self.ribbon_button(
                            "indent-less",
                            "字下げ－",
                            "インデントを減らす",
                            false,
                            cx,
                            |view, cx| view.indent(-36.0, cx),
                        )
                        .into_any_element(),
                        self.ribbon_button(
                            "indent-more",
                            "字下げ＋",
                            "インデントを増やす",
                            false,
                            cx,
                            |view, cx| view.indent(36.0, cx),
                        )
                        .into_any_element(),
                    ]),
                    Self::ribbon_row(vec![
                        self.ribbon_button(
                            "line-spacing",
                            "行間",
                            "行と段落の間隔",
                            self.popup == Some(RibbonPopup::LineSpacing),
                            cx,
                            |view, cx| view.toggle_popup(RibbonPopup::LineSpacing, cx),
                        )
                        .into_any_element(),
                        self.ribbon_button(
                            "space-after",
                            "段落後",
                            "段落の後に間隔を空ける",
                            false,
                            cx,
                            |view, cx| view.set_paragraph_spacing(12.0, cx),
                        )
                        .into_any_element(),
                        self.ribbon_button(
                            "shading",
                            "網かけ",
                            "段落の網かけ",
                            self.popup == Some(RibbonPopup::Shading),
                            cx,
                            |view, cx| view.toggle_popup(RibbonPopup::Shading, cx),
                        )
                        .into_any_element(),
                        self.ribbon_button(
                            "borders",
                            "罫線",
                            "段落を罫線で囲む",
                            false,
                            cx,
                            |view, cx| view.set_paragraph_borders(true, cx),
                        )
                        .into_any_element(),
                        self.ribbon_button(
                            "no-borders",
                            "罫線なし",
                            "段落の罫線を消す",
                            false,
                            cx,
                            |view, cx| view.set_paragraph_borders(false, cx),
                        )
                        .into_any_element(),
                    ]),
                    Self::ribbon_row(vec![
                        self.ribbon_button(
                            "align-left",
                            "左",
                            "左揃え",
                            false,
                            cx,
                            |view, cx| view.align("left", cx),
                        )
                        .into_any_element(),
                        self.ribbon_button(
                            "align-center",
                            "中央",
                            "中央揃え",
                            false,
                            cx,
                            |view, cx| view.align("center", cx),
                        )
                        .into_any_element(),
                        self.ribbon_button(
                            "align-right",
                            "右",
                            "右揃え",
                            false,
                            cx,
                            |view, cx| view.align("right", cx),
                        )
                        .into_any_element(),
                        self.ribbon_button(
                            "align-both",
                            "両端",
                            "両端揃え",
                            false,
                            cx,
                            |view, cx| view.align("both", cx),
                        )
                        .into_any_element(),
                    ]),
                ],
            )
            .into_any_element(),
            Self::ribbon_group(
                "スタイル",
                vec![Self::ribbon_row(vec![
                    self.style_button("style-normal", "標準", "Normal", cx)
                        .into_any_element(),
                    self.style_button("style-h1", "見出し 1", "Heading1", cx)
                        .into_any_element(),
                    self.style_button("style-h2", "見出し 2", "Heading2", cx)
                        .into_any_element(),
                    self.style_button("style-h3", "見出し 3", "Heading3", cx)
                        .into_any_element(),
                ])],
            )
            .into_any_element(),
            Self::ribbon_group(
                "編集",
                vec![Self::ribbon_row(vec![
                    self.ribbon_button("undo", "戻す", "元に戻す", false, cx, |view, cx| {
                        view.step_history(true, cx)
                    })
                    .into_any_element(),
                    self.ribbon_button(
                        "redo",
                        "やり直し",
                        "やり直し",
                        false,
                        cx,
                        |view, cx| view.step_history(false, cx),
                    )
                    .into_any_element(),
                    self.ribbon_button(
                        "find",
                        "検索",
                        "検索と置換",
                        false,
                        cx,
                        |view, cx| view.open_find_bar(cx),
                    )
                    .into_any_element(),
                ])],
            )
            .into_any_element(),
        ]
    }

    /// A style gallery entry, wider than a glyph button because the name is the
    /// point of it.
    fn style_button(
        &self,
        id: &'static str,
        label: &'static str,
        style_id: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(id)
            .flex()
            .items_center()
            .justify_center()
            .w(px(72.0))
            .h(px(28.0))
            .rounded_md()
            .border_1()
            .border_color(theme::border())
            .cursor_pointer()
            .hover(|button| button.bg(theme::hovered()))
            .text_sm()
            .text_color(theme::text())
            .child(label)
            .on_click(
                cx.listener(move |view, _event, _window, cx| {
                    view.set_paragraph_style(style_id, cx)
                }),
            )
    }

    /// The list under the ribbon, while one is open.
    ///
    /// Word puts these behind a swatch or a chevron; here they are rows, which
    /// is what can be built without an icon set.
    fn ribbon_popup(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let popup = self.popup?;
        /// A family, or a colour as a name and its `RRGGBB`.
        const FAMILIES: [&str; 6] = [
            "游明朝",
            "游ゴシック",
            "MS 明朝",
            "MS ゴシック",
            "Times New Roman",
            "Arial",
        ];
        const COLORS: [(&str, &str); 7] = [
            ("自動", "000000"),
            ("赤", "C00000"),
            ("オレンジ", "E36C0A"),
            ("緑", "00B050"),
            ("青", "0070C0"),
            ("紫", "7030A0"),
            ("灰", "808080"),
        ];
        const HIGHLIGHTS: [(&str, &str); 5] = [
            ("なし", "none"),
            ("黄", "yellow"),
            ("緑", "green"),
            ("水色", "cyan"),
            ("桃", "magenta"),
        ];
        const SPACINGS: [(&str, f32); 4] =
            [("1.0", 1.0), ("1.15", 1.15), ("1.5", 1.5), ("2.0", 2.0)];
        const SHADINGS: [(&str, &str); 5] = [
            ("なし", ""),
            ("薄黄", "FFF2CC"),
            ("薄青", "DEEAF6"),
            ("薄緑", "E2EFDA"),
            ("薄灰", "EDEDED"),
        ];

        let rows: Vec<AnyElement> = match popup {
            RibbonPopup::FontFamily => FAMILIES
                .iter()
                .map(|family| {
                    self.popup_row(family, family, cx, move |view, cx| {
                        view.set_font_family(family, cx)
                    })
                    .into_any_element()
                })
                .collect(),
            RibbonPopup::FontColor => COLORS
                .iter()
                .map(|(label, color)| {
                    self.popup_row(color, label, cx, move |view, cx| {
                        view.set_font_color(color, cx)
                    })
                    .into_any_element()
                })
                .collect(),
            RibbonPopup::Highlight => HIGHLIGHTS
                .iter()
                .map(|(label, color)| {
                    self.popup_row(color, label, cx, move |view, cx| {
                        view.set_highlight(color, cx)
                    })
                    .into_any_element()
                })
                .collect(),
            RibbonPopup::LineSpacing => SPACINGS
                .iter()
                .map(|(label, multiple)| {
                    let multiple = *multiple;
                    self.popup_row(label, label, cx, move |view, cx| {
                        view.set_line_spacing(multiple, cx)
                    })
                    .into_any_element()
                })
                .collect(),
            RibbonPopup::Shading => SHADINGS
                .iter()
                .map(|(label, color)| {
                    self.popup_row(label, label, cx, move |view, cx| {
                        view.set_shading((!color.is_empty()).then_some(color), cx)
                    })
                    .into_any_element()
                })
                .collect(),
        };

        Some(
            div()
                .flex()
                .flex_col()
                .w(px(180.0))
                .ml(px(180.0))
                .mb_2()
                .p_1()
                .rounded_md()
                .bg(theme::panel())
                .border_1()
                .border_color(theme::border())
                .children(rows),
        )
    }

    /// One row of a ribbon popup.
    fn popup_row(
        &self,
        id: &'static str,
        label: &'static str,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        div()
            .id(id)
            .px_2()
            .py_1()
            .rounded_md()
            .cursor_pointer()
            .text_sm()
            .text_color(theme::text())
            .hover(|row| row.bg(theme::hovered()))
            .child(label)
            .on_click(cx.listener(move |view, _event, _window, cx| {
                action(view, cx);
                view.popup = None;
                cx.notify();
            }))
    }

    /// The find bar, drawn above the pages while it is open.
    ///
    /// A caret bar after the focused field, because the field has no cursor of
    /// its own: without it there is no way to see which of the two fields the
    /// next keystroke goes into.
    fn find_bar_element(&self) -> Option<impl IntoElement> {
        let bar = self.find_bar.as_ref()?;
        let field = |label: &str, value: &str, focused: bool| {
            div()
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_1()
                .rounded_md()
                .bg(if focused {
                    theme::canvas()
                } else {
                    theme::panel()
                })
                .border_1()
                .border_color(if focused {
                    theme::accent()
                } else {
                    theme::border()
                })
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_faint())
                        .child(label.to_string()),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(theme::text())
                        .min_w(px(140.0))
                        .child(format!("{value}{}", if focused { "|" } else { "" })),
                )
        };

        Some(
            div()
                .flex()
                .items_center()
                .gap_3()
                .flex_none()
                .px_4()
                .py_2()
                .bg(theme::panel())
                .border_b_1()
                .border_color(theme::border())
                .child(field("Find", &bar.query, !bar.editing_replacement))
                .child(field("Replace", &bar.replacement, bar.editing_replacement))
                .child(
                    div().text_xs().text_color(theme::text_faint()).child(
                        self.find_status
                            .as_ref()
                            .map(|status| status.to_string())
                            .unwrap_or_else(|| "—".to_string()),
                    ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_faint())
                        .child("⇥ field · ⏎ next · ⇧⏎ replace · ⌘⏎ all · esc close"),
                ),
        )
    }

    /// The text the platform's input handler is editing: the find bar's field
    /// while it is open, otherwise the paragraph the caret is in.
    fn focused_text(&self) -> String {
        match self.find_bar.as_ref() {
            Some(bar) => bar.field().clone(),
            None => self.current_paragraph_text(),
        }
    }

    /// Applies a keystroke — or a composition — to the find bar's field.
    ///
    /// The same two entry points the document uses, so Japanese input works in
    /// the query as well: `mark` distinguishes a preedit from a commit.
    fn replace_in_find_bar(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        mark: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(bar) = self.find_bar.as_mut() else {
            return;
        };
        let field = bar.field().clone();
        let bytes = match range {
            Some(range) => utf16_to_byte(&field, range.start)..utf16_to_byte(&field, range.end),
            None => bar.marked.clone().unwrap_or(field.len()..field.len()),
        };
        let start = bytes.start.min(field.len());
        let end = bytes.end.clamp(start, field.len());
        let mut updated = String::with_capacity(field.len() + text.len());
        updated.push_str(&field[..start]);
        updated.push_str(text);
        updated.push_str(&field[end..]);
        *bar.field_mut() = updated;
        bar.marked = mark.then(|| start..start + text.len());
        self.sync_find_query(cx);
    }

    /// Opens the find bar, seeded with the selection if there is one.
    fn open_find_bar(&mut self, cx: &mut Context<Self>) {
        let mut bar = FindBar::default();
        if let Some(selected) = self.document.selected_text() {
            if !selected.trim().is_empty() {
                bar.query = selected;
            }
        }
        if bar.query.is_empty() {
            if let Some(query) = self.find_query.clone() {
                bar.query = query;
            }
        }
        self.find_query = (!bar.query.is_empty()).then(|| bar.query.clone());
        self.find_bar = Some(bar);
        self.update_find_count();
        cx.notify();
    }

    fn close_find_bar(&mut self, cx: &mut Context<Self>) {
        self.find_bar = None;
        cx.notify();
    }

    /// Keys the bar handles itself. Returns whether it took the keystroke.
    fn handle_find_bar_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let platform = event.keystroke.modifiers.platform;
        let shift = event.keystroke.modifiers.shift;
        match event.keystroke.key.as_str() {
            "escape" => {
                self.close_find_bar(cx);
                true
            }
            "tab" => {
                if let Some(bar) = self.find_bar.as_mut() {
                    bar.editing_replacement = !bar.editing_replacement;
                    bar.marked = None;
                }
                cx.notify();
                true
            }
            // ⌘⏎ replaces every match, ⇧⏎ replaces the one that is selected,
            // and ⏎ on its own just walks to the next.
            "enter" | "return" if platform => {
                self.replace_all_matches(cx);
                true
            }
            "enter" | "return" if shift => {
                self.replace_current_match(cx);
                true
            }
            "enter" | "return" => {
                self.find_query = self.find_bar.as_ref().map(|bar| bar.query.clone());
                self.find_next(cx);
                true
            }
            "backspace" => {
                if let Some(bar) = self.find_bar.as_mut() {
                    bar.marked = None;
                    let field = bar.field_mut();
                    field.pop();
                }
                self.sync_find_query(cx);
                true
            }
            // Everything else — including ⌘Z and ⌘S — still belongs to the
            // document.
            _ => false,
        }
    }

    /// Puts what has been typed into the bar's query into effect.
    fn sync_find_query(&mut self, cx: &mut Context<Self>) {
        let Some(bar) = self.find_bar.as_ref() else {
            return;
        };
        self.find_query = (!bar.query.is_empty()).then(|| bar.query.clone());
        self.update_find_count();
        cx.notify();
    }

    /// Reports how many matches the current query has, without moving to one.
    ///
    /// Typing a query should say what it would find before Enter is pressed;
    /// jumping on every keystroke would drag the view around mid-word.
    fn update_find_count(&mut self) {
        let Some(query) = self.find_query.clone() else {
            self.find_status = None;
            return;
        };
        let count = self.document.find(&query, false).len();
        self.find_status = Some(SharedString::from(if count == 0 {
            format!("{query}: no matches")
        } else {
            format!("{query}: {count} matches")
        }));
    }

    /// Replaces the match the find has landed on, then moves to the next.
    ///
    /// From the menu with no bar open there is nothing to replace yet, so this
    /// opens it — on the replacement field, which is the one the command is
    /// about.
    fn replace_current_match(&mut self, cx: &mut Context<Self>) {
        if self.find_bar.is_none() {
            self.open_find_bar(cx);
            if let Some(bar) = self.find_bar.as_mut() {
                bar.editing_replacement = true;
            }
            return cx.notify();
        }
        let Some(bar) = self.find_bar.as_ref() else {
            return;
        };
        let (query, replacement) = (bar.query.clone(), bar.replacement.clone());
        if query.is_empty() {
            return;
        }
        // The selection is the match the last find jumped to. Without one there
        // is nothing to replace yet, so find first.
        let Some(((paragraph, start), (_, end))) = self.document.selection() else {
            self.find_query = Some(query);
            return self.find_next(cx);
        };
        let hit = SearchHit {
            paragraph,
            start_offset: start.min(end),
            end_offset: start.max(end),
        };
        let page = self.caret_page_1idx();
        match self.document.replace_one(hit, &replacement, page) {
            Ok(()) => {
                self.insert_at = (paragraph, hit.start_offset + replacement.len());
                self.caret = self
                    .document
                    .place_caret(self.insert_at.0, self.insert_at.1);
                self.rebuild_all_images();
                self.find_query = Some(query);
                self.find_next(cx);
            }
            Err(error) => eprintln!("replace failed: {error}"),
        }
    }

    /// Replaces every match, in one undo step.
    fn replace_all_matches(&mut self, cx: &mut Context<Self>) {
        if self.find_bar.is_none() {
            self.open_find_bar(cx);
            if let Some(bar) = self.find_bar.as_mut() {
                bar.editing_replacement = true;
            }
            return cx.notify();
        }
        let Some(bar) = self.find_bar.as_ref() else {
            return;
        };
        let (query, replacement) = (bar.query.clone(), bar.replacement.clone());
        if query.is_empty() {
            return;
        }
        let page = self.caret_page_1idx();
        match self.document.replace_all(&query, &replacement, page) {
            Ok(replaced) => {
                // Replacing re-flows text onto other pages, so every page is
                // rebuilt rather than the one the caret is on.
                self.rebuild_all_images();
                self.caret = self
                    .document
                    .place_caret(self.insert_at.0, self.insert_at.1);
                self.find_status = Some(SharedString::from(format!(
                    "{query} → {replacement}: {replaced} replaced"
                )));
                cx.notify();
            }
            Err(error) => eprintln!("replace all failed: {error}"),
        }
    }

    /// Takes the selection as the search query and jumps to the next match.
    fn start_find(&mut self, cx: &mut Context<Self>) {
        if let Some(query) = self.document.selected_text() {
            if !query.trim().is_empty() {
                self.find_query = Some(query);
            }
        }
        self.find_next(cx);
    }

    /// Selects the first match after the caret, wrapping to the top.
    fn find_next(&mut self, cx: &mut Context<Self>) {
        let Some(query) = self.find_query.clone() else {
            return;
        };
        let hits = self.document.find(&query, false);
        if hits.is_empty() {
            self.find_status = Some(SharedString::from(format!("{query}: no matches")));
            return cx.notify();
        }
        let (paragraph, offset) = self.insert_at;
        // Wrapping: the first hit strictly after the caret, else the first of
        // all. Strictly, so repeating does not land on the same match forever.
        let next = hits
            .iter()
            .find(|hit| {
                hit.paragraph > paragraph
                    || (hit.paragraph == paragraph && hit.start_offset > offset)
            })
            .copied()
            .unwrap_or(hits[0]);
        let position = hits.iter().position(|hit| *hit == next).unwrap_or(0);
        self.find_status = Some(SharedString::from(format!(
            "{query}: {} of {}",
            position + 1,
            hits.len()
        )));

        let page = self.caret_page_1idx();
        if self
            .document
            .select(
                (next.paragraph, next.start_offset),
                (next.paragraph, next.end_offset),
                page,
            )
            .is_ok()
        {
            self.insert_at = (next.paragraph, next.end_offset);
            self.caret = self.document.place_caret(next.paragraph, next.end_offset);
            if let Some(caret) = self.caret {
                let (page_of_hit, _) = self.document.page_and_offset(caret.document_y);
                self.scroll.scroll_to_item(page_of_hit);
                self.current_page = page_of_hit;
            }
            self.rebuild_all_images();
            cx.notify();
        }
    }

    /// Inserts a row below the caret's, or deletes the caret's row.
    ///
    /// Silently does nothing outside a table: these share ⌘⇧Return and
    /// ⌘⇧Backspace with nothing else, and a beep would be noise.
    fn edit_table_row(&mut self, insert: bool, cx: &mut Context<Self>) {
        match self.document.edit_table_row(insert) {
            Ok(()) => {
                self.caret = self
                    .document
                    .place_caret(self.insert_at.0, self.insert_at.1);
                self.rebuild_all_images();
                cx.notify();
            }
            Err(error) => self.table_status = Some(SharedString::from(error)),
        }
    }

    /// Leaves a comment on the selection, or on the caret's paragraph.
    ///
    /// The text is fixed until there is somewhere to type it. That is honest
    /// about what this does — a placeholder comment is still a real comment in
    /// the file, and the panel says how many there are.
    fn add_comment(&mut self, cx: &mut Context<Self>) {
        match self.document.add_comment("Comment", "jade") {
            Ok(()) => {
                self.rebuild_all_images();
                cx.notify();
            }
            Err(error) => eprintln!("comment failed: {error}"),
        }
    }

    fn toggle_tracking(&mut self, cx: &mut Context<Self>) {
        let next = !self.tracking_changes;
        match self.document.set_change_tracking(next, "jade") {
            Ok(()) => {
                self.tracking_changes = next;
                self.rebuild_all_images();
                cx.notify();
            }
            Err(error) => eprintln!("change tracking failed: {error}"),
        }
    }

    fn align(&mut self, alignment: &str, cx: &mut Context<Self>) {
        let page = self.caret_page_1idx();
        if self.document.set_alignment(alignment, page).is_ok() {
            self.rebuild_all_images();
            cx.notify();
        }
    }

    fn indent(&mut self, points: f32, cx: &mut Context<Self>) {
        self.indent_points = (self.indent_points + points).max(0.0);
        let page = self.caret_page_1idx();
        let indent = self.indent_points;
        if self.document.shift_indent(indent, page).is_ok() {
            self.rebuild_all_images();
            cx.notify();
        }
    }

    /// Clamped so a stray repeat cannot leave the document unreadably small or
    /// so large that a page no longer fits any sensible window.
    fn set_zoom(&mut self, zoom: f32, cx: &mut Context<Self>) {
        self.zoom = zoom.clamp(0.4, 3.0);
        cx.notify();
    }

    fn format(&mut self, format: CharacterFormat, cx: &mut Context<Self>) {
        let page = self.caret_page_1idx();
        // An error here means there was no selection, which is the ordinary
        // case for a caret rather than a failure.
        if self.document.format_selection(format, page).is_ok() {
            self.refresh_caret_page_image();
            cx.notify();
        }
    }

    /// Selects the paragraph the caret is in.
    ///
    /// Not the whole document: selecting every paragraph would repaint all of
    /// them, and the paragraph is the unit everything else here works on.
    /// Applies a run-level command and refreshes what it touched.
    fn after_run_format(&mut self, applied: Result<(), String>, cx: &mut Context<Self>) {
        // An error is "there was no selection", which is the ordinary case for
        // a caret rather than a failure. The selection survives a command that
        // did apply, so the next ribbon button acts on the same run.
        if applied.is_ok() {
            self.refresh_caret_page_image();
            cx.notify();
        }
    }

    fn set_font_size(&mut self, points: f32, cx: &mut Context<Self>) {
        self.font_size_pt = points.clamp(6.0, 96.0);
        let page = self.caret_page_1idx();
        let applied = self.document.set_font_size(self.font_size_pt, page);
        self.after_run_format(applied, cx);
        cx.notify();
    }

    fn set_font_family(&mut self, family: &str, cx: &mut Context<Self>) {
        self.font_family = SharedString::from(family.to_string());
        self.popup = None;
        let page = self.caret_page_1idx();
        let applied = self.document.set_font_family(family, page);
        self.after_run_format(applied, cx);
        cx.notify();
    }

    fn set_font_color(&mut self, color: &str, cx: &mut Context<Self>) {
        let page = self.caret_page_1idx();
        let applied = self.document.set_font_color(color, page);
        self.after_run_format(applied, cx);
    }

    fn set_highlight(&mut self, color: &str, cx: &mut Context<Self>) {
        let page = self.caret_page_1idx();
        let applied = self.document.set_highlight_color(color, page);
        self.after_run_format(applied, cx);
    }

    /// Opens a ribbon list, or closes it if it is the one already open.
    fn toggle_popup(&mut self, popup: RibbonPopup, cx: &mut Context<Self>) {
        self.popup = (self.popup != Some(popup)).then_some(popup);
        cx.notify();
    }

    fn set_line_spacing(&mut self, multiple: f32, cx: &mut Context<Self>) {
        let page = self.caret_page_1idx();
        if self.document.set_line_spacing(multiple, page).is_ok() {
            // Line height moves everything below it.
            self.rebuild_all_images();
            cx.notify();
        }
    }

    fn set_paragraph_spacing(&mut self, after: f32, cx: &mut Context<Self>) {
        let page = self.caret_page_1idx();
        if self
            .document
            .set_paragraph_spacing(None, Some(after), page)
            .is_ok()
        {
            self.rebuild_all_images();
            cx.notify();
        }
    }

    fn set_shading(&mut self, color: Option<&str>, cx: &mut Context<Self>) {
        let page = self.caret_page_1idx();
        if self.document.set_paragraph_shading(color, page).is_ok() {
            self.refresh_caret_page_image();
            cx.notify();
        }
    }

    fn set_paragraph_borders(&mut self, boxed: bool, cx: &mut Context<Self>) {
        let page = self.caret_page_1idx();
        if self.document.set_paragraph_borders(boxed, page).is_ok() {
            self.refresh_caret_page_image();
            cx.notify();
        }
    }

    fn set_page_background(&mut self, color: Option<&str>, cx: &mut Context<Self>) {
        if self.document.set_page_background(color).is_ok() {
            self.rebuild_all_images();
            cx.notify();
        }
    }

    fn set_page_borders(&mut self, boxed: bool, cx: &mut Context<Self>) {
        if self.document.set_page_borders(boxed).is_ok() {
            self.rebuild_all_images();
            cx.notify();
        }
    }

    fn set_paragraph_style(&mut self, style_id: &str, cx: &mut Context<Self>) {
        let page = self.caret_page_1idx();
        if self.document.set_paragraph_style(style_id, page).is_ok() {
            // A style changes line heights, so everything below can move.
            self.rebuild_all_images();
            cx.notify();
        }
    }

    fn set_numbering(&mut self, num_id: Option<&str>, cx: &mut Context<Self>) {
        let page = self.caret_page_1idx();
        if self.document.set_numbering(num_id, page).is_ok() {
            self.rebuild_all_images();
            cx.notify();
        }
    }

    fn insert_table(&mut self, rows: usize, columns: usize, cx: &mut Context<Self>) {
        self.log_call(format!(
            "insert_table {rows}x{columns} at paragraph {} selection {:?}",
            self.insert_at.0,
            self.document.selection(),
        ));
        if self.document.insert_table(rows, columns).is_ok() {
            self.rebuild_all_images();
            cx.notify();
        }
    }

    fn insert_blank_page(&mut self, cx: &mut Context<Self>) {
        if self.document.insert_blank_page().is_ok() {
            self.rebuild_all_images();
            cx.notify();
        }
    }

    fn insert_table_of_contents(&mut self, cx: &mut Context<Self>) {
        if self.document.insert_table_of_contents().is_ok() {
            self.rebuild_all_images();
            cx.notify();
        }
    }

    fn resolve_revisions(&mut self, accept: bool, cx: &mut Context<Self>) {
        if self.document.resolve_revisions(accept).is_ok() {
            self.rebuild_all_images();
            cx.notify();
        }
    }

    fn toggle_formatting_marks(&mut self, cx: &mut Context<Self>) {
        let show = !self.document.formatting_marks();
        if self.document.set_formatting_marks(show).is_ok() {
            self.rebuild_all_images();
            cx.notify();
        }
    }

    /// Selects the whole document, the way ⌘A does everywhere else.
    fn select_document(&mut self, cx: &mut Context<Self>) {
        let paragraphs = self.document.paragraph_count();
        let Some(last) = paragraphs.checked_sub(1) else {
            return;
        };
        let end = self
            .document
            .paragraph_text(last)
            .map(|text| text.len())
            .unwrap_or(0);
        let page = self.caret_page_1idx();
        if self.document.select((0, 0), (last, end), page).is_ok() {
            self.insert_at = (last, end);
            self.caret = self.document.place_caret(last, end);
            self.selection_anchor = None;
            // The highlight is painted by the renderer, and it now covers every
            // page rather than the one the caret is on.
            self.rebuild_all_images();
            cx.notify();
        }
    }

    /// Scrolls a page forwards or backwards.
    fn go_to_page(&mut self, delta: isize, cx: &mut Context<Self>) {
        let last = self.images.len().saturating_sub(1);
        let target = (self.current_page as isize + delta).clamp(0, last as isize) as usize;
        self.current_page = target;
        self.scroll.scroll_to_item(target);
        cx.notify();
    }

    fn select_paragraph(&mut self, cx: &mut Context<Self>) {
        let paragraph = self.insert_at.0;
        let Some(text) = self.document.paragraph_text(paragraph) else {
            return;
        };
        let page = self.caret_page_1idx();
        if self
            .document
            .select((paragraph, 0), (paragraph, text.len()), page)
            .is_ok()
        {
            self.insert_at = (paragraph, text.len());
            self.caret = self.document.place_caret(paragraph, text.len());
            self.refresh_caret_page_image();
            cx.notify();
        }
    }

    /// Selects the run of word characters around the caret.
    ///
    /// Word boundaries here are "letters and digits versus everything else",
    /// which is right for Latin and wrong for Japanese, where a run has no
    /// spaces to break on. Selecting the whole run is still more useful than
    /// selecting nothing, and it is honest about what it does.
    fn select_word_at_caret(&mut self, cx: &mut Context<Self>) {
        let (paragraph, offset) = self.insert_at;
        let Some(text) = self.document.paragraph_text(paragraph) else {
            return;
        };
        let is_word = |ch: char| ch.is_alphanumeric();
        let start = text[..offset.min(text.len())]
            .char_indices()
            .rev()
            .take_while(|(_, ch)| is_word(*ch))
            .last()
            .map(|(index, _)| index)
            .unwrap_or(offset);
        let end = text[offset.min(text.len())..]
            .char_indices()
            .take_while(|(_, ch)| is_word(*ch))
            .last()
            .map(|(index, ch)| offset + index + ch.len_utf8())
            .unwrap_or(offset);
        if start == end {
            return;
        }
        let page = self.caret_page_1idx();
        if self
            .document
            .select((paragraph, start), (paragraph, end), page)
            .is_ok()
        {
            self.insert_at = (paragraph, end);
            self.caret = self.document.place_caret(paragraph, end);
            self.refresh_caret_page_image();
            cx.notify();
        }
    }

    fn copy(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = self.document.selected_text() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    fn cut(&mut self, cx: &mut Context<Self>) {
        self.copy(cx);
        let page = self.caret_page_1idx();
        if let Ok(caret) = self.document.replace_selection("", page) {
            self.insert_at = caret;
            self.caret = self.document.place_caret(caret.0, caret.1);
            self.rebuild_all_images();
            cx.notify();
        }
    }

    fn paste(&mut self, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        // Newlines become paragraph breaks, which is what they were when the
        // text was copied out of a document.
        let mut lines = text.split('\n');
        let Some(first) = lines.next() else { return };
        let page = self.caret_page_1idx();

        // A paste replaces the selection, exactly as typing does.
        if self.document.selection().is_some() {
            match self.document.replace_selection(first, page) {
                Ok(caret) => self.insert_at = caret,
                Err(error) => return eprintln!("paste failed: {error}"),
            }
        } else {
            let (paragraph, offset) = self.insert_at;
            match self.document.insert_text(paragraph, offset, first, page) {
                Ok(_) => self.insert_at = (paragraph, offset + first.len()),
                Err(error) => return eprintln!("paste failed: {error}"),
            }
        }

        for line in lines {
            let (paragraph, offset) = self.insert_at;
            if self
                .document
                .insert_paragraph_break(paragraph, offset)
                .is_err()
            {
                break;
            }
            self.insert_at = (paragraph + 1, 0);
            if !line.is_empty() {
                let (paragraph, offset) = self.insert_at;
                if self
                    .document
                    .insert_text(paragraph, offset, line, page)
                    .is_ok()
                {
                    self.insert_at = (paragraph, offset + line.len());
                }
            }
        }

        self.caret = self
            .document
            .place_caret(self.insert_at.0, self.insert_at.1);
        self.rebuild_all_images();
        cx.notify();
    }

    /// Undo or redo, then repaint everything: history can move content onto
    /// pages this window is not looking at.
    fn step_history(&mut self, backwards: bool, cx: &mut Context<Self>) {
        let result = if backwards {
            self.document.undo()
        } else {
            self.document.redo()
        };
        match result {
            Ok(Some(caret)) => {
                self.insert_at = caret;
                self.caret = self.document.place_caret(caret.0, caret.1);
                self.rebuild_all_images();
                cx.notify();
            }
            // Nothing on the stack: not an error, just nothing to do.
            Ok(None) => {}
            Err(error) => eprintln!("history failed: {error}"),
        }
    }

    /// Writes the document back to a .docx beside the one that was opened.
    fn save(&mut self, cx: &mut Context<Self>) {
        // Beside the original rather than over it: this shell is not yet the
        // thing to trust with the only copy of a document.
        let target = PathBuf::from(self.path.to_string()).with_extension("edited.docx");
        self.write_to(target, cx);
    }

    /// Asks where to save, then saves there.
    fn save_as(&mut self, cx: &mut Context<Self>) {
        let source = PathBuf::from(self.path.to_string());
        let directory = source
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let suggested = source
            .file_stem()
            .map(|stem| format!("{}.docx", stem.to_string_lossy()))
            .unwrap_or_else(|| "document.docx".to_string());
        let prompt = cx.prompt_for_new_path(&directory, Some(&suggested));
        cx.spawn(async move |view, cx| {
            let Ok(Ok(Some(path))) = prompt.await else {
                // Cancelled, or the platform had nothing to say.
                return;
            };
            view.update(cx, |view, cx| view.write_to(path, cx)).ok();
        })
        .detach();
    }

    fn write_to(&mut self, target: PathBuf, cx: &mut Context<Self>) {
        match self.document.export_docx() {
            Ok(bytes) => match std::fs::write(&target, &bytes) {
                Ok(()) => {
                    self.saved_to = Some(SharedString::from(
                        target
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                    ));
                    cx.notify();
                }
                Err(error) => eprintln!("save failed: {error}"),
            },
            Err(error) => eprintln!("export failed: {error}"),
        }
    }

    /// Prints the same vector PDF bytes produced by the export renderer.
    fn print(&mut self) {
        let result = self
            .document
            .export_docx()
            .map_err(|error| format!("export failed: {error}"))
            .and_then(|docx| {
                jade_docx_pdf_renderer::render_docx_to_pdf(
                    &docx,
                    &jade_docx_pdf_renderer::RenderOptions::default(),
                )
                .map_err(|error| format!("PDF render failed: {error:?}"))
            })
            .and_then(|pdf| native_print::show_pdf_print_dialog(&pdf));
        if let Err(error) = result {
            eprintln!("print failed: {error}");
        }
    }

    /// Asks for a .docx and opens it in this window.
    ///
    /// The app took its document from argv and could not be pointed at another
    /// one without being restarted.
    fn open_file(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |view, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            view.update(cx, |view, cx| view.load(path, cx)).ok();
        })
        .detach();
    }

    /// Replaces the document this window is showing.
    fn load(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let document = match open_document(&path.to_string_lossy()) {
            Ok(document) => document,
            Err(error) => {
                // The window keeps the document it has: failing to open a
                // second file is not a reason to lose the first.
                eprintln!("{error}");
                return;
            }
        };
        self.document = document;
        self.title = SharedString::from(
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.to_string_lossy().into_owned()),
        );
        self.file_size = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
        self.path = SharedString::from(path.to_string_lossy().into_owned());
        // Everything below is about the document that is no longer open.
        self.insert_at = (INSERT_PARAGRAPH, 0);
        self.caret = self.document.place_caret(INSERT_PARAGRAPH, 0);
        self.selection_anchor = None;
        self.drag_anchor = None;
        self.object_drag = None;
        self.marked = None;
        self.find_bar = None;
        self.find_query = None;
        self.find_status = None;
        self.table_status = None;
        self.saved_to = None;
        self.indent_points = 0.0;
        self.typed = 0;
        self.current_page = 0;
        self.last_repaint = None;
        self.page_bounds.borrow_mut().clear();
        self.rebuild_all_images();
        cx.notify();
    }

    /// Extends the selection to the caret, anchored where it was before the
    /// last extension.
    ///
    /// Shift-arrow keeps the anchor still and moves the focus, which is why the
    /// anchor is remembered rather than recomputed from the selection each
    /// time — recomputing it would let the anchor drift as the focus moves.
    fn extend_selection_to_caret(&mut self, anchor: (usize, usize), cx: &mut Context<Self>) {
        let page = self.caret_page_1idx();
        if anchor == self.insert_at {
            return;
        }
        if self.document.select(anchor, self.insert_at, page).is_ok() {
            self.selection_anchor = Some(anchor);
            self.refresh_caret_page_image();
            cx.notify();
        }
    }

    /// Moves the caret one character along the current paragraph.
    fn step_caret(&mut self, direction: i32, cx: &mut Context<Self>) {
        let (paragraph, offset) = self.insert_at;
        let text = self.current_paragraph_text();
        let next = if direction < 0 {
            text[..offset.min(text.len())]
                .chars()
                .next_back()
                .map(|ch| offset - ch.len_utf8())
        } else {
            text[offset.min(text.len())..]
                .chars()
                .next()
                .map(|ch| offset + ch.len_utf8())
        };
        if let Some(next) = next {
            self.insert_at = (paragraph, next);
            self.caret = self.document.place_caret(paragraph, next);
            cx.notify();
        }
    }

    /// Moves the caret a line up or down, by asking what is at the caret's x on
    /// the neighbouring line. Layout owns where lines are; this does not guess.
    fn step_line(&mut self, direction: f64, cx: &mut Context<Self>) {
        let Some(caret) = self.caret else { return };
        let probe_y = caret.document_y + caret.height * (0.5 + direction);
        if let Some(position) = self.document.position_at(caret.x, probe_y) {
            self.insert_at = position;
            self.caret = self.document.place_caret(position.0, position.1);
            cx.notify();
        }
    }

    /// Shared tail of an edit: record the timing, move the caret, refresh the
    /// page image.
    fn after_edit(
        &mut self,
        result: Result<Duration, String>,
        caret_at: (usize, usize),
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(elapsed) => {
                self.last_repaint = Some(elapsed);
                self.insert_at = caret_at;
                self.caret = self.document.place_caret(caret_at.0, caret_at.1);
                self.refresh_caret_page_image();
                cx.notify();
            }
            Err(error) => eprintln!("edit failed: {error}"),
        }
    }

    /// Re-reads every page. Needed after a re-flow, where a later page's
    /// content may have moved even though the edit was on an earlier one.
    fn rebuild_all_images(&mut self) {
        self.dump_page_if_asked();
        let pages = self.document.page_count();
        for index in 0..pages {
            self.set_page_image(index);
        }
        // A document that has shrunk leaves images for pages that are no longer
        // there; they still hold their place in the atlas.
        if self.images.len() > pages {
            let dropped: Vec<_> = self.images.drain(pages..).collect();
            self.retired_images.extend(dropped);
        }
    }

    /// The text position under a window-coordinate point on `page_index`.
    fn position_at_window_point(
        &self,
        page_index: usize,
        position: gpui::Point<Pixels>,
    ) -> Option<(usize, usize)> {
        let bounds = self.page_bounds.borrow().get(page_index).copied()??;
        // Divided by the zoom: the page is drawn scaled, so a point on screen
        // is that many document pixels in. Without this, clicks land further
        // and further from the pointer the more you zoom.
        let zoom = f64::from(self.zoom);
        let local_x = f64::from(position.x - bounds.origin.x) / zoom;
        let local_y = f64::from(position.y - bounds.origin.y) / zoom;
        // Through the document's own mapping rather than page height times
        // index: the renderer leaves a gap between pages, so the height alone
        // is right on page 1 and drifts by one gap per page after it.
        let document_y = self.document.document_y(page_index, local_y);
        self.document.position_at(local_x, document_y)
    }

    /// Extends the selection from the drag anchor to the pointer.
    fn drag_to(&mut self, page_index: usize, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if self.object_drag.is_some() {
            return self.drag_object_to(page_index, event, cx);
        }
        let Some(anchor) = self.drag_anchor else {
            return;
        };
        let Some(focus) = self.position_at_window_point(page_index, event.position) else {
            return;
        };
        if focus == anchor {
            // Collapsed: a click that has not moved yet is a caret, not an
            // empty selection, and painting a highlight for it flickers.
            return;
        }
        let page = self.caret_page_1idx();
        if self.document.select(anchor, focus, page).is_ok() {
            self.insert_at = focus;
            self.caret = self.document.place_caret(focus.0, focus.1);
            self.refresh_caret_page_image();
            cx.notify();
        }
    }

    /// Moves or resizes the object the pointer grabbed.
    ///
    /// Measured from where the drag started rather than from the last position,
    /// so the object tracks the pointer exactly however many moves arrive.
    fn drag_object_to(
        &mut self,
        page_index: usize,
        event: &MouseMoveEvent,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = self.object_drag.as_ref() else {
            return;
        };
        let Some(Some(bounds)) = self.page_bounds.borrow().get(page_index).copied() else {
            return;
        };
        let zoom = f64::from(self.zoom);
        let local_x = f64::from(event.position.x - bounds.origin.x) / zoom;
        let local_y = f64::from(event.position.y - bounds.origin.y) / zoom;
        let (dx, dy) = (local_x - drag.from.0, local_y - drag.from.1);
        let (start_x, start_y, start_width, start_height) = drag.start;

        let rect = if drag.resizing {
            // A minimum, or a drag past the top-left corner would invert the
            // picture and then be impossible to grab again.
            (
                start_x,
                start_y,
                (start_width + dx).max(8.0),
                (start_height + dy).max(8.0),
            )
        } else {
            (start_x + dx, start_y + dy, start_width, start_height)
        };
        let object = drag.object;
        // The first move of a drag records; the rest replace what it recorded,
        // so releasing the mouse leaves one undo step rather than thirty.
        let moved = if drag.applied {
            self.document
                .retransform_object(object, rect.0, rect.1, rect.2, rect.3)
        } else {
            self.document
                .transform_object(object, rect.0, rect.1, rect.2, rect.3)
        };
        if let Err(error) = moved {
            eprintln!("transform failed: {error}");
            return;
        }
        if let Some(drag) = self.object_drag.as_mut() {
            drag.applied = true;
        }
        // The object is not bound to one page's bitmap: moving it can uncover
        // text on the page it came from, so every page is rebuilt.
        self.rebuild_all_images();
        cx.notify();
    }

    /// Turns a click into an insertion point.
    fn click_page(&mut self, page_index: usize, event: &MouseDownEvent, cx: &mut Context<Self>) {
        self.log_call(format!("click page {page_index}"));
        let Some(Some(bounds)) = self.page_bounds.borrow().get(page_index).copied() else {
            return;
        };
        let zoom = f64::from(self.zoom);
        let local_x = f64::from(event.position.x - bounds.origin.x) / zoom;
        let local_y = f64::from(event.position.y - bounds.origin.y) / zoom;

        // Through the document's own mapping rather than page height times
        // index: the renderer leaves a gap between pages, so the height alone
        // is right on page 1 and drifts by one gap per page after it.
        let document_y = self.document.document_y(page_index, local_y);

        // A picture is on top of the text, so a click inside one is a grab on
        // the picture. Falling through would put the caret behind it and let a
        // drag select the text underneath instead.
        if let Some(object) = self.document.object_at(local_x, document_y) {
            // The bottom-right corner resizes, the rest moves — the handle Word
            // and the browser shell both put there.
            const HANDLE: f64 = 12.0;
            let resizing = local_x >= object.x + object.width - HANDLE
                && local_y >= object.y + object.height - HANDLE;
            self.object_drag = Some(ObjectDrag {
                object,
                start: (object.x, object.y, object.width, object.height),
                from: (local_x, local_y),
                resizing,
                applied: false,
            });
            self.drag_anchor = None;
            cx.notify();
            return;
        }

        if let Some(position) = self.document.position_at(local_x, document_y) {
            self.insert_at = position;
            self.current_page = page_index;
            self.drag_anchor = Some(position);
            // A click starts a new selection; the keyboard anchor from any
            // earlier shift-arrow run is no longer where the user means.
            self.selection_anchor = None;
            self.caret = self.document.place_caret(position.0, position.1);
            self.refresh_caret_page_image();
            cx.notify();
        }
    }

    fn sidebar_row(&self, index: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = index == self.current_page;
        div()
            .id(("page", index))
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .py_2()
            .rounded_md()
            .when(selected, |row| row.bg(theme::selected()))
            .hover(|row| row.bg(theme::hovered()))
            .cursor_pointer()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::text_faint())
                            .child(self.title.clone()),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(if selected {
                                theme::text()
                            } else {
                                theme::text_dim()
                            })
                            .child(format!("Page {}", index + 1)),
                    ),
            )
            .when(selected, |row| {
                row.child(div().size_1p5().rounded_full().bg(theme::accent()))
            })
            .on_click(cx.listener(move |view, _event, _window, cx| {
                view.current_page = index;
                view.scroll.scroll_to_item(index);
                cx.notify();
            }))
    }

    /// The caret, as an overlay pinned over the page it falls on.
    ///
    /// Drawn here rather than painted into the page so moving it costs nothing:
    /// the document bitmap is untouched, which is also what will let it blink.
    fn caret_overlay(&self, page_index: usize) -> Option<impl IntoElement> {
        let caret = self.caret?;
        let zoom = self.zoom;
        let (page_of_caret, local_y) = self.document.page_and_offset(caret.document_y);
        if page_of_caret != page_index {
            return None;
        }
        Some(
            div()
                .absolute()
                .left(px(caret.x as f32 * zoom))
                .top(px(local_y as f32 * zoom))
                .w(px((caret.width.max(1.5) as f32 * zoom).max(1.0)))
                .h(px(caret.height as f32 * zoom))
                .bg(theme::caret()),
        )
    }

    fn info_row(label: &str, value: String) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .text_color(theme::text_faint())
                    .child(label.to_string()),
            )
            .child(div().text_sm().text_color(theme::text_dim()).child(value))
    }
}

/// UTF-16 offsets are what the platform IME speaks; the document is bytes.
///
/// Both are offsets into the *same* paragraph text, so the conversion is a
/// scan rather than anything structural — but it has to happen at every
/// boundary, because a Japanese character is one UTF-16 unit and three bytes.
fn utf16_to_byte(text: &str, utf16_offset: usize) -> usize {
    let mut utf16 = 0;
    for (byte, ch) in text.char_indices() {
        if utf16 >= utf16_offset {
            return byte;
        }
        utf16 += ch.len_utf16();
    }
    text.len()
}

fn byte_to_utf16(text: &str, byte_offset: usize) -> usize {
    // Clamped to a character boundary rather than trusting the caller. A byte
    // offset that came from a different paragraph lands mid-character here, and
    // slicing on it panics inside a callback the platform cannot unwind
    // through — the process dies rather than the input being ignored.
    let mut end = byte_offset.min(text.len());
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].chars().map(char::len_utf16).sum()
}

/// The text the IME is editing: the paragraph the caret is in.
///
/// A composition never spans a paragraph, and a paragraph is what an edit
/// command addresses, so it is the natural window to expose.
impl DocumentView {
    fn current_paragraph_text(&self) -> String {
        self.document
            .paragraph_text(self.insert_at.0)
            .unwrap_or_default()
    }

    /// Replaces `range` (bytes, in the current paragraph) with `text`, leaving
    /// the caret after it.
    fn replace_bytes(&mut self, range: Range<usize>, text: &str, cx: &mut Context<Self>) {
        self.log_call(format!(
            "replace_bytes paragraph {} range {:?} text {text:?} bound {} marked {:?} selection {:?}",
            self.insert_at.0,
            range,
            self.caret_page_1idx(),
            self.marked,
            self.document.selection(),
        ));
        // A selection is what the next character replaces — the same rule
        // backspace already followed. Inserting at the caret instead left the
        // selected text on the page with the new character beside it.
        //
        // Not while a composition is running: there the marked range is what
        // the IME is replacing, and the selection it started from is already
        // gone.
        if self.marked.is_none() && self.document.selection().is_some() {
            let page = self.caret_page_1idx();
            match self.document.replace_selection(text, page) {
                Ok(caret) => {
                    self.insert_at = caret;
                    self.caret = self.document.place_caret(caret.0, caret.1);
                    self.selection_anchor = None;
                    self.typed += text.chars().count();
                    // A selection can span paragraphs, and replacing one pulls
                    // the tail up: every page below can move.
                    self.rebuild_all_images();
                    cx.notify();
                }
                Err(error) => eprintln!("edit failed: {error}"),
            }
            return;
        }

        let paragraph = self.insert_at.0;
        let page = self.caret_page_1idx();
        let result = if range.is_empty() {
            self.document
                .insert_text(paragraph, range.start, text, page)
        } else {
            self.document
                .replace_range(paragraph, range.start, range.end, text, page)
        };
        match result {
            Ok(elapsed) => {
                self.last_repaint = Some(elapsed);
                self.typed += text.chars().count();
                self.insert_at = (paragraph, range.start + text.len());
                self.caret = self
                    .document
                    .place_caret(self.insert_at.0, self.insert_at.1);
                self.refresh_caret_page_image();
                cx.notify();
            }
            Err(error) => eprintln!("edit failed: {error}"),
        }
    }
}

impl EntityInputHandler for DocumentView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let text = self.focused_text();
        let start = utf16_to_byte(&text, range.start);
        let end = utf16_to_byte(&text, range.end).max(start);
        *adjusted = Some(byte_to_utf16(&text, start)..byte_to_utf16(&text, end));
        Some(text.get(start..end)?.to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let text = self.focused_text();
        // The bar has no caret model: typing appends, so the insertion point is
        // the end of the field.
        let caret = match self.find_bar.as_ref() {
            Some(_) => byte_to_utf16(&text, text.len()),
            None => byte_to_utf16(&text, self.insert_at.1),
        };
        // Collapsed: there is no selection model yet, only a caret.
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
        let text = self.focused_text();
        let marked = match self.find_bar.as_ref() {
            Some(bar) => bar.marked.clone()?,
            None => self.marked.clone()?,
        };
        Some(byte_to_utf16(&text, marked.start)..byte_to_utf16(&text, marked.end))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        if let Some(bar) = self.find_bar.as_mut() {
            bar.marked = None;
            return;
        }
        // Only the mark is dropped. The text stays: the platform sends this
        // when a composition ends, and what is on the page is the committed
        // result.
        self.marked = None;
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.find_bar.is_some() {
            self.replace_in_find_bar(range, text, false, cx);
            return;
        }
        let paragraph_text = self.current_paragraph_text();
        // No range means "replace the composition", and no composition means
        // "insert at the caret".
        let bytes = match range {
            Some(range) => {
                utf16_to_byte(&paragraph_text, range.start)
                    ..utf16_to_byte(&paragraph_text, range.end)
            }
            None => self
                .marked
                .clone()
                .unwrap_or(self.insert_at.1..self.insert_at.1),
        };
        self.marked = None;
        self.replace_bytes(bytes, text, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.find_bar.is_some() {
            self.replace_in_find_bar(range, new_text, true, cx);
            return;
        }
        let paragraph_text = self.current_paragraph_text();
        let bytes = match range {
            Some(range) => {
                utf16_to_byte(&paragraph_text, range.start)
                    ..utf16_to_byte(&paragraph_text, range.end)
            }
            None => self
                .marked
                .clone()
                .unwrap_or(self.insert_at.1..self.insert_at.1),
        };
        let start = bytes.start;
        self.replace_bytes(bytes, new_text, cx);
        // The preedit is now in the document, and this is the range the next
        // keystroke of the same composition will replace.
        self.marked = Some(start..start + new_text.len());
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        // Where the candidate window goes. The caret rectangle is already the
        // right answer; it only has to be moved into window coordinates.
        let caret = self.caret?;
        let (page, local_y) = self.document.page_and_offset(caret.document_y);
        let origin = self
            .page_bounds
            .borrow()
            .get(page)
            .copied()
            .flatten()?
            .origin;
        let _ = element_bounds;
        // Scaled like the caret overlay, or the candidate window drifts from
        // the text it belongs to as soon as the view is zoomed.
        let zoom = self.zoom;
        Some(Bounds {
            origin: point(
                origin.x + px(caret.x as f32 * zoom),
                origin.y + px(local_y as f32 * zoom),
            ),
            size: size(
                px(caret.width as f32 * zoom),
                px(caret.height as f32 * zoom),
            ),
        })
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let bounds = self.page_bounds.borrow();
        let (page, origin) = bounds
            .iter()
            .enumerate()
            .find_map(|(index, b)| b.map(|b| (index, b)))
            .map(|(index, b)| (index, b.origin))?;
        drop(bounds);
        let zoom = f64::from(self.zoom);
        let local_x = f64::from(point.x - origin.x) / zoom;
        let document_y = self
            .document
            .document_y(page, f64::from(point.y - origin.y) / zoom);
        let (paragraph, offset) = self.document.position_at(local_x, document_y)?;
        // The IME's offsets are into the paragraph it is editing. A point in a
        // different paragraph has no index in that text, and answering with one
        // anyway is how a byte offset from elsewhere ends up mid-character.
        if paragraph != self.insert_at.0 {
            return None;
        }
        let text = self.current_paragraph_text();
        Some(byte_to_utf16(&text, offset))
    }
}

impl Focusable for DocumentView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for DocumentView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The one place with a window to hand. Textures replaced since the last
        // frame are released here rather than at the edit that replaced them.
        for image in self.retired_images.drain(..) {
            window.drop_image(image).ok();
        }

        let page_count = self.images.len();
        // Recomputed each frame: gpui does not notify on scroll, and this is
        // cheap next to laying the frame out.
        self.current_page = self.page_from_scroll();
        // The rendered page box, which is the document's rather than whatever
        // the shell asked for at open.
        let (page_width, page_height) = {
            let (w, h) = self.document.layout_page_size();
            (w as f32, h as f32)
        };
        let zoom = self.zoom;
        div()
            .id("root")
            // Rounded and clipped so the window reads as a floating panel. The
            // titlebar is transparent, so without this the corners would be
            // square regardless of the shape the platform draws.
            .rounded_xl()
            .overflow_hidden()
            .border_1()
            .border_color(theme::border())
            .track_focus(&self.focus)
            // What the shortcuts in `key_bindings` are scoped to, so they fire
            // only while a document has the keyboard.
            .key_context("Document")
            .on_action(cx.listener(|view, _: &Open, _window, cx| view.open_file(cx)))
            .on_action(cx.listener(|view, _: &Save, _window, cx| view.save(cx)))
            .on_action(cx.listener(|view, _: &SaveAs, _window, cx| view.save_as(cx)))
            .on_action(cx.listener(|view, _: &Print, _window, _cx| view.print()))
            .on_action(cx.listener(|view, _: &Undo, _window, cx| view.step_history(true, cx)))
            .on_action(cx.listener(|view, _: &Redo, _window, cx| view.step_history(false, cx)))
            .on_action(cx.listener(|view, _: &Cut, _window, cx| view.cut(cx)))
            .on_action(cx.listener(|view, _: &Copy, _window, cx| view.copy(cx)))
            .on_action(cx.listener(|view, _: &Paste, _window, cx| view.paste(cx)))
            .on_action(cx.listener(|view, _: &SelectAll, _window, cx| view.select_document(cx)))
            .on_action(
                cx.listener(|view, _: &SelectParagraph, _window, cx| view.select_paragraph(cx)),
            )
            .on_action(cx.listener(|view, _: &NextPage, _window, cx| view.go_to_page(1, cx)))
            .on_action(cx.listener(|view, _: &PreviousPage, _window, cx| view.go_to_page(-1, cx)))
            .on_action(cx.listener(|view, _: &FindAndReplace, _window, cx| view.open_find_bar(cx)))
            .on_action(cx.listener(|view, _: &FindSelection, _window, cx| view.start_find(cx)))
            .on_action(cx.listener(|view, _: &FindNext, _window, cx| view.find_next(cx)))
            .on_action(cx.listener(|view, _: &Replace, _window, cx| view.replace_current_match(cx)))
            .on_action(
                cx.listener(|view, _: &ReplaceAll, _window, cx| view.replace_all_matches(cx)),
            )
            .on_action(
                cx.listener(|view, _: &Bold, _window, cx| view.format(CharacterFormat::Bold, cx)),
            )
            .on_action(
                cx.listener(|view, _: &Italic, _window, cx| {
                    view.format(CharacterFormat::Italic, cx)
                }),
            )
            .on_action(cx.listener(|view, _: &Underline, _window, cx| {
                view.format(CharacterFormat::Underline, cx)
            }))
            .on_action(cx.listener(|view, _: &AlignLeft, _window, cx| view.align("left", cx)))
            .on_action(cx.listener(|view, _: &AlignCenter, _window, cx| view.align("center", cx)))
            .on_action(cx.listener(|view, _: &AlignRight, _window, cx| view.align("right", cx)))
            .on_action(cx.listener(|view, _: &IndentMore, _window, cx| view.indent(36.0, cx)))
            .on_action(cx.listener(|view, _: &IndentLess, _window, cx| view.indent(-36.0, cx)))
            .on_action(
                cx.listener(|view, _: &InsertTableRow, _window, cx| view.edit_table_row(true, cx)),
            )
            .on_action(
                cx.listener(|view, _: &DeleteTableRow, _window, cx| view.edit_table_row(false, cx)),
            )
            .on_action(cx.listener(|view, _: &AddComment, _window, cx| view.add_comment(cx)))
            .on_action(
                cx.listener(|view, _: &ToggleChangeTracking, _window, cx| view.toggle_tracking(cx)),
            )
            .on_action(
                cx.listener(|view, _: &ZoomIn, _window, cx| view.set_zoom(view.zoom * 1.1, cx)),
            )
            .on_action(
                cx.listener(|view, _: &ZoomOut, _window, cx| view.set_zoom(view.zoom / 1.1, cx)),
            )
            .on_action(cx.listener(|view, _: &ZoomActual, _window, cx| view.set_zoom(1.0, cx)))
            .on_key_down(cx.listener(|view, event: &KeyDownEvent, _window, cx| {
                view.handle_key(event, cx);
            }))
            .flex()
            .size_full()
            .bg(theme::window())
            // gpui's default is `.SystemUIFont`, which carries no Japanese at
            // all, so every label in this shell would be blank. Found with
            // `JADE_FONT_PROBE=1`, which reports each character's advance in a
            // family — the only way to see this without a screen.
            .font_family(UI_FONT_FAMILY)
            .text_color(theme::text())
            // ---- sidebar: the document's pages ----------------------------
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .w(px(240.0))
                    .h_full()
                    .bg(theme::sidebar())
                    .border_r_1()
                    .border_color(theme::border())
                    .child(
                        // Left clear of the traffic lights, which are drawn
                        // over the window rather than in a system titlebar.
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .px_4()
                            .pt(px(44.0))
                            .pb_3()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme::text())
                                    .child(self.title.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme::text_faint())
                                    .child(format!("{page_count} pages")),
                            ),
                    )
                    .child(
                        div()
                            .id("page-list")
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .px_2()
                            .size_full()
                            .overflow_scroll()
                            .children((0..page_count).map(|index| self.sidebar_row(index, cx))),
                    )
                    // Anchored at the bottom, like the account chip in a chat
                    // shell: what this window is looking at, always in view.
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .gap_3()
                            .px_4()
                            .py_3()
                            .border_t_1()
                            .border_color(theme::border())
                            .child(
                                div()
                                    .flex()
                                    .flex_none()
                                    .size_7()
                                    .items_center()
                                    .justify_center()
                                    .rounded_md()
                                    .bg(theme::selected())
                                    .text_xs()
                                    .text_color(theme::accent())
                                    .child("W"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(theme::text())
                                            .child(self.title.clone()),
                                    )
                                    .child(div().text_xs().text_color(theme::text_faint()).child(
                                        format!("{:.0} KB", self.file_size as f64 / 1024.0),
                                    )),
                            ),
                    ),
            )
            // ---- canvas: the ribbon, the find bar, then the pages ----------
            .child(
                div()
                    .flex()
                    .flex_col()
                    .size_full()
                    // The document's own tab, above the ribbon, the way the
                    // browser shell shows which file is open.
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .flex_none()
                            // Tall enough that the traffic lights, which the
                            // platform draws over a transparent titlebar at
                            // (16, 16), sit inside the strip rather than over
                            // the tab.
                            .h(px(44.0))
                            .pt(px(8.0))
                            .px_4()
                            .gap_2()
                            .bg(theme::panel())
                            .border_b_1()
                            .border_color(theme::border())
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .bg(theme::selected())
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(theme::text())
                                            .child(self.title.clone()),
                                    ),
                            ),
                    )
                    .child(self.ribbon(cx))
                    .children(self.find_bar_element())
                    .child(
                        div()
                            .id("pages")
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_6()
                            .p_8()
                            .size_full()
                            .bg(theme::canvas())
                            .overflow_scroll()
                            // Tracked so the sidebar can jump to a page.
                            .track_scroll(&self.scroll)
                            .children(self.images.iter().enumerate().map(|(index, page)| {
                                let bounds_store = Rc::clone(&self.page_bounds);
                                let entity = cx.entity();
                                div()
                                    .id(("page-surface", index))
                                    .relative()
                                    .flex_none()
                                    .shadow_lg()
                                    .cursor_pointer()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(
                                            move |view, event: &MouseDownEvent, _window, cx| {
                                                view.click_page(index, event, cx);
                                                if event.click_count >= 2 {
                                                    view.select_word_at_caret(cx);
                                                }
                                            },
                                        ),
                                    )
                                    .on_mouse_move(cx.listener(
                                        move |view, event: &MouseMoveEvent, _window, cx| {
                                            if event.dragging() {
                                                view.drag_to(index, event, cx);
                                            }
                                        },
                                    ))
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(
                                            move |view, _event: &MouseUpEvent, _window, _cx| {
                                                view.drag_anchor = None;
                                            },
                                        ),
                                    )
                                    .child(
                                        img(page.clone())
                                            .w(px(page_width * zoom))
                                            .h(px(page_height * zoom)),
                                    )
                                    .children(self.caret_overlay(index))
                                    // Records where this page ended up. A click arrives
                                    // in window coordinates and has to be made local
                                    // before the renderer can hit-test it.
                                    .child(
                                        canvas(
                                            move |bounds, _window, _cx| {
                                                let mut store = bounds_store.borrow_mut();
                                                if store.len() <= index {
                                                    store.resize(index + 1, None);
                                                }
                                                store[index] = Some(bounds);
                                                bounds
                                            },
                                            move |_bounds, page_bounds, window, cx| {
                                                // Registered during paint, which is what
                                                // `handle_input` requires, and only for the
                                                // page the caret is on — the handler is
                                                // per-focus, not per-element, so registering
                                                // it three times would just be the last one.
                                                if index == 0 {
                                                    window.handle_input(
                                                        &entity.focus_handle(cx),
                                                        ElementInputHandler::new(
                                                            page_bounds,
                                                            entity.clone(),
                                                        ),
                                                        cx,
                                                    );
                                                }
                                            },
                                        )
                                        // `absolute` without an offset keeps the element's
                                        // static position, which here is the slot after the
                                        // image — a whole page too low. `top_0().left_0()`
                                        // is what actually pins it over the page.
                                        .absolute()
                                        .top_0()
                                        .left_0()
                                        .size_full(),
                                    )
                            })),
                    ),
            )
            // ---- right panel: what is actually known about the document ----
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .w(px(240.0))
                    .h_full()
                    .gap_5()
                    .px_4()
                    .pt(px(44.0))
                    .bg(theme::panel())
                    .border_l_1()
                    .border_color(theme::border())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_sm().text_color(theme::text()).child("Document"))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme::text_faint())
                                    .child("Rendered without a browser"),
                            ),
                    )
                    .child(Self::info_row("Pages", page_count.to_string()))
                    .child(Self::info_row(
                        "Page size",
                        format!("{} x {} px", page_width as i32, page_height as i32),
                    ))
                    .child(Self::info_row(
                        "File size",
                        format!("{:.1} KB", self.file_size as f64 / 1024.0),
                    ))
                    .child(Self::info_row("Renderer", "jade + Skia".to_string()))
                    .child(Self::info_row(
                        "Last repaint",
                        match self.last_repaint {
                            Some(elapsed) => format!("{:.2} ms", elapsed.as_secs_f64() * 1000.0),
                            None => "—".to_string(),
                        },
                    ))
                    .child(Self::info_row("Typed", format!("{} chars", self.typed)))
                    .child(Self::info_row(
                        "Insertion point",
                        match self.document.selection() {
                            Some((anchor, focus)) => {
                                format!("¶{}:{} – ¶{}:{}", anchor.0, anchor.1, focus.0, focus.1)
                            }
                            None => format!("¶{} · byte {}", self.insert_at.0, self.insert_at.1),
                        },
                    ))
                    .child(Self::info_row(
                        "History",
                        format!(
                            "{} undo · {} redo",
                            if self.document.can_undo() {
                                "can"
                            } else {
                                "no"
                            },
                            if self.document.can_redo() {
                                "can"
                            } else {
                                "no"
                            },
                        ),
                    ))
                    .child(Self::info_row(
                        "Review",
                        format!(
                            "{} comments · tracking {}",
                            self.document.comment_count(),
                            if self.tracking_changes { "on" } else { "off" }
                        ),
                    ))
                    .child(Self::info_row(
                        "Table",
                        match self.document.caret_table_cell() {
                            Some(cell) => format!(
                                "row {} of {} · column {} of {}",
                                cell.row + 1,
                                cell.rows,
                                cell.column + 1,
                                cell.columns
                            ),
                            None => self
                                .table_status
                                .as_ref()
                                .map(|status| status.to_string())
                                .unwrap_or_else(|| "—".to_string()),
                        },
                    ))
                    .child(Self::info_row(
                        "Find",
                        self.find_status
                            .as_ref()
                            .map(|status| status.to_string())
                            .unwrap_or_else(|| "—".to_string()),
                    ))
                    .child(Self::info_row(
                        "Saved",
                        self.saved_to
                            .as_ref()
                            .map(|name| name.to_string())
                            .unwrap_or_else(|| "—".to_string()),
                    ))
                    .child(div().text_xs().text_color(theme::text_faint()).child(
                        // The commands are in the menu bar now, with their
                        // shortcuts on them. What is left here is what no menu
                        // item can show: the mouse and the find bar's own keys.
                        "Drag to select · double-click a word · ⇧arrows extend \
                                 · type over a selection to replace it \
                                 · drag a picture to move it, its corner to size \
                                 · in the find bar: ⇥ field · ⏎ next \
                                 · ⇧⏎ replace · ⌘⏎ all · esc close.",
                    ))
                    .child(
                        div()
                            .mt_auto()
                            .pb_4()
                            .text_xs()
                            .text_color(theme::text_faint())
                            .child(self.path.clone()),
                    ),
            )
    }
}

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: jade-gpui-app <input.docx|input.xlsx>");
        std::process::exit(2);
    };

    // A workbook is a different window: no pages, no Word ribbon, and a sheet
    // too tall to rasterise up front. Dispatching here keeps the document view
    // from growing a second mode it would have to branch on everywhere.
    if path.to_lowercase().ends_with(".xlsx") {
        workbook::run(path);
        return;
    }

    // Opened before the window exists: a failure here is a plain message on
    // stderr rather than an empty window with no explanation.
    let mut document = match open_document(&path) {
        Ok(document) => document,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    };
    // Placed before the window opens so the caret is visible from the first
    // frame rather than only after the first click.
    let caret = document.place_caret(INSERT_PARAGRAPH, 0);
    let images: Vec<_> = (0..document.page_count())
        .filter_map(|index| page_image(&document, index))
        .collect();
    let file_size = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
    let full_path = SharedString::from(path.clone());
    let title = SharedString::from(
        PathBuf::from(&path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or(path.clone()),
    );

    Application::new().run(move |cx: &mut App| {
        // A way to ask the text system what it makes of the shell's own labels
        // without a window, which is the only part of this app that can be
        // checked without looking at a screen.
        if std::env::var("JADE_FONT_PROBE").is_ok() {
            probe_fonts(cx);
            cx.quit();
            return;
        }
        cx.bind_keys(key_bindings());
        cx.set_menus(menus());
        // The only action the window does not own: quitting is the app's.
        cx.on_action(|_: &Quit, cx: &mut App| cx.quit());

        let bounds = Bounds::centered(None, size(px(1320.0), px(900.0)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some(SharedString::from("jade")),
                        // Drawn over by the app so the sidebar reaches the top
                        // edge; the traffic lights are nudged down to match.
                        appears_transparent: true,
                        traffic_light_position: Some(point(px(16.0), px(16.0))),
                    }),
                    // Transparent rather than blurred: the body paints its own
                    // translucent colour, and rounded corners need whatever is
                    // behind the window to show through them.
                    window_background: WindowBackgroundAppearance::Transparent,
                    ..Default::default()
                },
                |_window, cx| {
                    cx.new(|cx| DocumentView {
                        title,
                        path: full_path,
                        file_size,
                        document,
                        images,
                        retired_images: Vec::new(),
                        dump_directory: std::env::var_os("JADE_DUMP_PAGE").map(PathBuf::from),
                        call_log: Vec::new(),
                        insert_at: (INSERT_PARAGRAPH, 0),
                        caret,
                        drag_anchor: None,
                        object_drag: None,
                        selection_anchor: None,
                        zoom: 1.0,
                        tracking_changes: false,
                        table_status: None,
                        find_query: None,
                        find_status: None,
                        find_bar: None,
                        ribbon_tab: RibbonTab::Home,
                        popup: None,
                        font_size_pt: 10.5,
                        font_family: SharedString::from("游明朝"),
                        indent_points: 0.0,
                        saved_to: None,
                        marked: None,
                        page_bounds: Rc::new(RefCell::new(Vec::new())),
                        last_repaint: None,
                        typed: 0,
                        current_page: 0,
                        scroll: ScrollHandle::new(),
                        focus: cx.focus_handle(),
                    })
                },
            )
            .expect("open window");

        // Focused up front, or the first keystroke goes nowhere and the app
        // looks broken rather than unfocused.
        window
            .update(cx, |view, window, cx| {
                window.focus(&view.focus_handle(cx));
            })
            .expect("focus the document");
        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    use super::{key_bindings, menus, Menu, MenuItem};
    use std::collections::{HashMap, HashSet};

    /// Every action named in a menu, in order.
    fn menu_actions(menus: &[Menu]) -> Vec<&'static str> {
        fn walk(items: &[MenuItem], out: &mut Vec<&'static str>) {
            for item in items {
                match item {
                    MenuItem::Action { action, .. } => out.push(action.name()),
                    MenuItem::Submenu(menu) => walk(&menu.items, out),
                    _ => {}
                }
            }
        }
        let mut out = Vec::new();
        for menu in menus {
            walk(&menu.items, &mut out);
        }
        out
    }

    /// A menu item with no binding shows no shortcut, which is how a command
    /// ends up looking keyboard-inaccessible even when it is not.
    #[test]
    fn every_menu_item_has_a_shortcut() {
        let bound: HashSet<&str> = key_bindings()
            .iter()
            .map(|binding| binding.action().name())
            .collect();

        let missing: Vec<_> = menu_actions(&menus())
            .into_iter()
            .filter(|name| !bound.contains(name))
            .collect();

        assert!(
            missing.is_empty(),
            "menu items with no shortcut: {missing:?}"
        );
    }

    /// Two commands on one key means one of them silently never runs. The key
    /// handler used to resolve this by ordering its match arms — ⇧⌘E before ⌘E
    /// — and an action table has no such ordering to lean on.
    #[test]
    fn no_keystroke_is_bound_twice() {
        let mut seen: HashMap<String, &'static str> = HashMap::new();
        for binding in key_bindings() {
            let keystrokes = binding
                .keystrokes()
                .iter()
                .map(|keystroke| format!("{keystroke:?}"))
                .collect::<Vec<_>>()
                .join(" ");
            let action = binding.action().name();
            if let Some(previous) = seen.insert(keystrokes.clone(), action) {
                panic!("{keystrokes} is bound to both {previous} and {action}");
            }
        }
    }

    /// The shortcuts the key handler still owns must not be bound to an action:
    /// a binding is dispatched first and stops there, so binding one of these
    /// would take it away from the find bar or from caret movement.
    #[test]
    fn the_keys_the_handler_owns_are_not_bound() {
        let bound: HashSet<String> = key_bindings()
            .iter()
            .map(|binding| {
                binding
                    .keystrokes()
                    .iter()
                    .map(|keystroke| format!("{keystroke:?}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect();

        for reserved in [
            "escape",
            "tab",
            "enter",
            "shift-enter",
            "cmd-enter",
            "backspace",
        ] {
            let keystrokes = gpui::KeyBinding::new(reserved, super::Save, None)
                .keystrokes()
                .iter()
                .map(|keystroke| format!("{keystroke:?}"))
                .collect::<Vec<_>>()
                .join(" ");
            assert!(
                !bound.contains(&keystrokes),
                "{reserved} is bound to an action, so the key handler will never see it",
            );
        }
    }
}

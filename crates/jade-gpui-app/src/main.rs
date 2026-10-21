//! jade office: Word, Excel and PowerPoint files in one native app, no
//! browser anywhere.
//!
//! Usage: `jade-gpui-app [file.docx|file.xlsx|file.pptx ...]`
//!
//! Each file opens in the window its format wants (see `shell`); with no
//! arguments the app shows its start window. The rest of this file is the
//! document (DOCX) window.
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

mod deck;
mod editor;
mod i18n;
mod native_print;
mod ribbon;
mod shell;
mod tooltip;
mod word_design;
mod word_dialog;
mod word_lists;
mod word_page;
mod word_paragraph;
mod word_review;
mod word_ruler;
mod word_styles;
mod word_tools;
mod workbook;
mod workspace;

use editor::Editor;
use i18n::text as t;
use tooltip::Tooltip;

/// "yes"/"no" in the live language, for the details panel.
fn yes_no(value: bool) -> &'static str {
    t(if value { "doc.info.yes" } else { "doc.info.no" })
}

use gpui::{
    actions, canvas, div, img, point, prelude::*, px, size, AnyElement, App, Application, Bounds,
    ClipboardItem, Context, ElementInputHandler, EntityInputHandler, FocusHandle, Focusable,
    KeyBinding, KeyDownEvent, Menu, MenuItem, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, RenderImage, ScrollHandle, SharedString, UTF16Selection, Window,
};
use jade_docx_native_render::{CaretAffinity, CaretRect, FaceSpec, NativeDocument, ObjectHit};
use jade_office_model::word::clipboard::{RichClip, WordClipboard};
use jade_office_model::word::page::{Story, WordPage};
use jade_office_model::word::picture::{ObjectTarget, PlacedObject, WordPicture, Wrap};
use jade_office_model::word::review::WordReview;
use jade_office_model::word::{
    CharacterFormat, FindOptions, FormatState, Found, TableCommand, WordFind, WordFormatting,
};
use ribbon::Action;
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
        NewDocument,
        NewFromTemplate,
        Open,
        CloseWindow,
        StartSlideshow,
        ToggleInspector,
        ToggleExplorer,
        ToggleTheme,
        ThemeSystem,
        ThemeLight,
        ThemeDark,
        ToggleDarkPages,
        OpenFolder,
        Save,
        SaveAs,
        Print,
        ExportPdf,
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
        ZoomFit,
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
        // Opening and closing belong to the app, not to one kind of window:
        // they work from the start window and from every view.
        KeyBinding::new("cmd-n", NewDocument, None),
        // Word for Mac's 新規テンプレートから.
        KeyBinding::new("cmd-shift-p", NewFromTemplate, None),
        KeyBinding::new("cmd-o", Open, None),
        KeyBinding::new("cmd-w", CloseWindow, None),
        KeyBinding::new("cmd-shift-o", OpenFolder, None),
        KeyBinding::new("cmd-alt-e", ToggleExplorer, None),
        KeyBinding::new("cmd-alt-i", ToggleInspector, None),
        KeyBinding::new("cmd-alt-t", ToggleTheme, None),
        KeyBinding::new("cmd-ctrl-alt-s", ThemeSystem, None),
        KeyBinding::new("cmd-ctrl-alt-l", ThemeLight, None),
        KeyBinding::new("cmd-ctrl-alt-d", ThemeDark, None),
        KeyBinding::new("cmd-ctrl-alt-p", ToggleDarkPages, None),
        // Saving, history and the clipboard mean something in more than one
        // kind of tab; each view that has them answers with its own handler.
        KeyBinding::new("cmd-s", Save, None),
        KeyBinding::new("cmd-shift-s", SaveAs, None),
        // Unscoped so each view that can print answers it with its own
        // handler; a view without one leaves it alone.
        KeyBinding::new("cmd-p", Print, None),
        KeyBinding::new("cmd-alt-p", ExportPdf, CONTEXT),
        KeyBinding::new("f5", StartSlideshow, Some("Deck")),
        KeyBinding::new("cmd-z", Undo, None),
        KeyBinding::new("cmd-shift-z", Redo, None),
        KeyBinding::new("cmd-x", Cut, None),
        KeyBinding::new("cmd-c", Copy, None),
        KeyBinding::new("cmd-v", Paste, None),
        KeyBinding::new("cmd-a", SelectAll, None),
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
        // Zoom belongs to the workspace, which steps whichever editor is in
        // front, so these work in every kind of tab.
        KeyBinding::new("cmd-=", ZoomIn, None),
        KeyBinding::new("cmd--", ZoomOut, None),
        KeyBinding::new("cmd-0", ZoomActual, None),
        KeyBinding::new("cmd-9", ZoomFit, None),
    ]
}

/// The menu bar.
///
/// Everything the shell can do is here. It was all keyboard-only, which meant a
/// feature nobody had been told about did not exist.
/// A menu label with a check in front when `mode` is the one in use: gpui's
/// menus have no checked state of their own, so the menus are rebuilt when
/// the mode changes.
fn checked(mode: theme::Mode, key: &'static str) -> String {
    let mark = if theme::mode() == mode {
        "✓ "
    } else {
        "    "
    };
    format!("{mark}{}", t(key))
}

/// Switches to `mode`, remembers it for next time, and repaints.
fn apply_theme(mode: theme::Mode, cx: &mut App) {
    theme::set_mode(mode);
    theme::save(mode);
    cx.set_menus(menus());
    cx.refresh_windows();
}

fn menus() -> Vec<Menu> {
    vec![
        Menu {
            name: "jade".into(),
            items: vec![MenuItem::action(t("menu.app.quit"), Quit)],
        },
        Menu {
            name: t("menu.file").into(),
            items: vec![
                MenuItem::action(t("menu.file.new"), NewDocument),
                MenuItem::action(t("menu.file.new_from_template"), NewFromTemplate),
                MenuItem::separator(),
                MenuItem::action(t("menu.file.open"), Open),
                MenuItem::action(t("menu.file.open_folder"), OpenFolder),
                MenuItem::action(t("menu.file.close_tab"), CloseWindow),
                MenuItem::separator(),
                MenuItem::action(t("menu.file.save"), Save),
                MenuItem::action(t("menu.file.save_as"), SaveAs),
                MenuItem::separator(),
                MenuItem::action(t("menu.file.print"), Print),
                MenuItem::action(t("menu.file.export_pdf"), ExportPdf),
            ],
        },
        Menu {
            name: t("menu.edit").into(),
            items: vec![
                MenuItem::action(t("menu.edit.undo"), Undo),
                MenuItem::action(t("menu.edit.redo"), Redo),
                MenuItem::separator(),
                MenuItem::action(t("menu.edit.cut"), Cut),
                MenuItem::action(t("menu.edit.copy"), Copy),
                MenuItem::action(t("menu.edit.paste"), Paste),
                MenuItem::action(t("menu.edit.select_all"), SelectAll),
                MenuItem::action(t("menu.edit.select_paragraph"), SelectParagraph),
                MenuItem::separator(),
                MenuItem::action(t("menu.edit.find_replace"), FindAndReplace),
                MenuItem::action(t("menu.edit.find_selection"), FindSelection),
                MenuItem::action(t("menu.edit.find_next"), FindNext),
                MenuItem::action(t("menu.edit.replace"), Replace),
                MenuItem::action(t("menu.edit.replace_all"), ReplaceAll),
            ],
        },
        Menu {
            name: t("menu.format").into(),
            items: vec![
                MenuItem::action(t("menu.format.bold"), Bold),
                MenuItem::action(t("menu.format.italic"), Italic),
                MenuItem::action(t("menu.format.underline"), Underline),
                MenuItem::separator(),
                MenuItem::action(t("menu.format.align_left"), AlignLeft),
                MenuItem::action(t("menu.format.center"), AlignCenter),
                MenuItem::action(t("menu.format.align_right"), AlignRight),
                MenuItem::separator(),
                MenuItem::action(t("menu.format.indent_more"), IndentMore),
                MenuItem::action(t("menu.format.indent_less"), IndentLess),
            ],
        },
        Menu {
            name: t("menu.table").into(),
            items: vec![
                MenuItem::action(t("menu.table.insert_row"), InsertTableRow),
                MenuItem::action(t("menu.table.delete_row"), DeleteTableRow),
            ],
        },
        Menu {
            name: t("menu.review").into(),
            items: vec![
                MenuItem::action(t("menu.review.add_comment"), AddComment),
                MenuItem::action(t("menu.review.track_changes"), ToggleChangeTracking),
            ],
        },
        Menu {
            name: t("menu.view").into(),
            items: vec![
                MenuItem::action(t("menu.view.explorer"), ToggleExplorer),
                MenuItem::action(t("menu.view.details"), ToggleInspector),
                MenuItem::submenu(Menu {
                    name: t("menu.view.appearance").into(),
                    items: vec![
                        MenuItem::action(
                            checked(theme::Mode::System, "menu.view.appearance.system"),
                            ThemeSystem,
                        ),
                        MenuItem::action(
                            checked(theme::Mode::Light, "menu.view.appearance.light"),
                            ThemeLight,
                        ),
                        MenuItem::action(
                            checked(theme::Mode::Dark, "menu.view.appearance.dark"),
                            ThemeDark,
                        ),
                        MenuItem::separator(),
                        MenuItem::action(
                            format!(
                                "{}{}",
                                if theme::dark_pages_wanted() {
                                    "✓ "
                                } else {
                                    "    "
                                },
                                t("menu.view.appearance.dark_pages")
                            ),
                            ToggleDarkPages,
                        ),
                        MenuItem::separator(),
                        MenuItem::action(t("menu.view.theme"), ToggleTheme),
                    ],
                }),
                MenuItem::separator(),
                MenuItem::action(t("menu.view.next_page"), NextPage),
                MenuItem::action(t("menu.view.previous_page"), PreviousPage),
                MenuItem::separator(),
                MenuItem::action(t("menu.view.zoom_in"), ZoomIn),
                MenuItem::action(t("menu.view.zoom_out"), ZoomOut),
                MenuItem::action(t("menu.view.zoom_actual"), ZoomActual),
                MenuItem::action(t("menu.view.zoom_fit"), ZoomFit),
            ],
        },
        Menu {
            name: t("menu.slideshow").into(),
            items: vec![MenuItem::action(
                t("menu.slideshow.from_beginning"),
                StartSlideshow,
            )],
        },
    ]
}

/// The font the shell draws its own labels in.
///
/// gpui's default is `.SystemUIFont`, which carries no Japanese: every label
/// here would be blank. `JADE_FONT_PROBE=1` reports what a family is missing.
pub(crate) const UI_FONT_FAMILY: &str = "Hiragino Sans";

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

/// The bundled Noto Sans Thai, which Thai text falls back to.
fn thai_font() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../jade-core/fonts")
        .join(jade_office_model::THAI_FONT)
}

/// The Skia backend the shared editing models (`jade-office-model`) paint
/// with: the bundled faces under every alias, parsed once per process and
/// shared by every workbook and deck view.
pub(crate) fn skia_backend() -> Result<jade_office_model::skia::SkiaBackend, String> {
    thread_local! {
        static BACKEND: RefCell<Option<jade_office_model::skia::SkiaBackend>> =
            const { RefCell::new(None) };
    }
    BACKEND.with(|backend| {
        if let Some(backend) = backend.borrow().as_ref() {
            return Ok(backend.clone());
        }
        let read = |name: &str| {
            std::fs::read(bundled_font(name))
                .map(Arc::new)
                .map_err(|error| format!("{name}: {error}"))
        };
        let mut faces = jade_office_model::aliased_faces(
            read("NotoSansJP-Regular.ttf")?,
            Some(read("NotoSansJP-Bold.ttf")?),
        );
        // Thai — BAHTTEXT's words among it — falls back to Noto Sans Thai.
        let thai = thai_font();
        faces.push(jade_office_model::thai_face(Arc::new(
            std::fs::read(&thai).map_err(|error| format!("{}: {error}", thai.display()))?,
        )));
        let made = jade_office_model::skia::SkiaBackend::new(&faces)?;
        *backend.borrow_mut() = Some(made.clone());
        Ok(made)
    })
}

/// Opens `path` and paints it, ready to be edited.
fn open_document(path: &str) -> Result<NativeDocument, String> {
    let bytes = std::fs::read(path).map_err(|err| format!("{path}: {err}"))?;
    open_document_bytes(&bytes)
}

/// `.docx` bytes, painted and ready to be edited.
fn open_document_bytes(bytes: &[u8]) -> Result<NativeDocument, String> {
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
    let thai = thai_font();
    faces.push(FaceSpec {
        family: "Noto Sans Thai",
        weight: 400,
        path: &thai,
    });
    NativeDocument::open(bytes, INITIAL_PAGE_WIDTH_PX, INITIAL_PAGE_HEIGHT_PX, &faces)
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

thread_local! {
    /// The last copy's formatting, for a paste in any document view that
    /// finds the copy's text still on the clipboard.
    static RICH_CLIP: RefCell<Option<RichClip>> = const { RefCell::new(None) };
}

/// A picture or shape a click found, as the shared commands address it.
fn placed_object(object: &ObjectHit) -> PlacedObject {
    PlacedObject {
        target: ObjectTarget {
            paragraph_index: object.paragraph,
            run_index: object.run,
            kind: object.kind,
        },
        page: object.page.saturating_sub(1),
        x: object.x,
        y: object.y,
        width: object.width,
        height: object.height,
    }
}

const WRAP_IDS: [&str; 7] = [
    "wrap-inline",
    "wrap-square",
    "wrap-tight",
    "wrap-through",
    "wrap-top-bottom",
    "wrap-behind",
    "wrap-front",
];

fn wrap_label(wrap: Wrap) -> &'static str {
    t(match wrap {
        Wrap::Inline => "picture.wrap.inline",
        Wrap::Square => "picture.wrap.square",
        Wrap::Tight => "picture.wrap.tight",
        Wrap::Through => "picture.wrap.through",
        Wrap::TopAndBottom => "picture.wrap.top_bottom",
        Wrap::BehindText => "picture.wrap.behind",
        Wrap::InFrontOfText => "picture.wrap.front",
    })
}

/// The palette, in one place, so a colour is named by its job rather than
/// repeated as a literal at each use.
///
/// The values are jade-ide's (`packages/jade-ide/src/styles/theme.css` in
/// jlsi/jade): zinc neutrals with an indigo primary, in a dark and a light
/// set. Which set is live is a process-wide switch; flipping it and
/// refreshing the windows repaints every view, since each reads its colours
/// per frame.
pub(crate) mod theme {
    use gpui::{rgb, Rgba, WindowAppearance};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

    /// What the user picked: follow macOS, or keep one palette regardless.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Mode {
        System,
        Light,
        Dark,
    }

    impl Mode {
        fn name(self) -> &'static str {
            match self {
                Mode::System => "system",
                Mode::Light => "light",
                Mode::Dark => "dark",
            }
        }

        pub fn parse(text: &str) -> Option<Mode> {
            match text.trim() {
                "system" => Some(Mode::System),
                "light" => Some(Mode::Light),
                "dark" => Some(Mode::Dark),
                _ => None,
            }
        }

        fn to_u8(self) -> u8 {
            self as u8
        }

        fn from_u8(value: u8) -> Mode {
            match value {
                1 => Mode::Light,
                2 => Mode::Dark,
                _ => Mode::System,
            }
        }
    }

    static MODE: AtomicU8 = AtomicU8::new(0);
    /// The palette in use: the mode's, or the system's while following it.
    static DARK: AtomicBool = AtomicBool::new(true);
    /// What macOS last said, kept so that going back to System needs no
    /// window to ask.
    static SYSTEM_DARK: AtomicBool = AtomicBool::new(true);

    pub fn is_dark() -> bool {
        DARK.load(Ordering::Relaxed)
    }

    pub fn mode() -> Mode {
        Mode::from_u8(MODE.load(Ordering::Relaxed))
    }

    pub fn set_mode(mode: Mode) {
        MODE.store(mode.to_u8(), Ordering::Relaxed);
        resolve();
    }

    /// Records the system appearance; the palette follows it in System mode.
    pub fn set_system(appearance: WindowAppearance) {
        let dark = matches!(
            appearance,
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        );
        SYSTEM_DARK.store(dark, Ordering::Relaxed);
        resolve();
    }

    fn resolve() {
        let dark = match mode() {
            Mode::System => SYSTEM_DARK.load(Ordering::Relaxed),
            Mode::Light => false,
            Mode::Dark => true,
        };
        DARK.store(dark, Ordering::Relaxed);
    }

    /// Whether the dark palette should show documents on dark paper too.
    /// Wanted by default, as in Word; the light palette always shows paper.
    static DARK_PAGES: AtomicBool = AtomicBool::new(true);

    /// Whether the pages are shown on dark paper right now.
    pub fn dark_pages() -> bool {
        is_dark() && dark_pages_wanted()
    }

    pub fn dark_pages_wanted() -> bool {
        DARK_PAGES.load(Ordering::Relaxed)
    }

    pub fn set_dark_pages_wanted(wanted: bool) {
        DARK_PAGES.store(wanted, Ordering::Relaxed);
    }

    fn store() -> Option<PathBuf> {
        settings_file("theme.txt")
    }

    fn settings_file(name: &str) -> Option<PathBuf> {
        // Tests switch themes too, and must not rewrite the user's choice.
        if cfg!(test) {
            return None;
        }
        let home = std::env::var_os("HOME")?;
        Some(
            PathBuf::from(home)
                .join("Library/Application Support/jade-office")
                .join(name),
        )
    }

    /// The dark-pages choice saved last time, or on.
    pub fn load_dark_pages() -> bool {
        settings_file("dark-pages.txt")
            .and_then(|store| std::fs::read_to_string(store).ok())
            .map(|text| text.trim() != "off")
            .unwrap_or(true)
    }

    pub fn save_dark_pages(wanted: bool) {
        let Some(store) = settings_file("dark-pages.txt") else {
            return;
        };
        if let Some(directory) = store.parent() {
            std::fs::create_dir_all(directory).ok();
        }
        std::fs::write(store, if wanted { "on" } else { "off" }).ok();
    }

    /// The mode saved last time, or System.
    pub fn load() -> Mode {
        store()
            .and_then(|store| std::fs::read_to_string(store).ok())
            .and_then(|text| Mode::parse(&text))
            .unwrap_or(Mode::System)
    }

    pub fn save(mode: Mode) {
        let Some(store) = store() else {
            return;
        };
        if let Some(directory) = store.parent() {
            std::fs::create_dir_all(directory).ok();
        }
        std::fs::write(store, mode.name()).ok();
    }

    fn pick(dark: u32, light: u32) -> Rgba {
        rgb(if is_dark() { dark } else { light })
    }

    /// `--background`: the window body and the title bar.
    pub fn window() -> Rgba {
        pick(0x09090b, 0xffffff)
    }
    /// `--sidebar`: the activity bar and the explorer.
    pub fn sidebar() -> Rgba {
        pick(0x0a0a0c, 0xf8fafc)
    }
    /// The well the white pages and slides sit in.
    pub fn canvas() -> Rgba {
        pick(0x141416, 0xeef0f4)
    }
    /// `--card`: tab strips, panels, the status bar.
    pub fn panel() -> Rgba {
        pick(0x0d0d0f, 0xffffff)
    }
    /// `--border`.
    pub fn border() -> Rgba {
        pick(0x27272a, 0xe2e8f0)
    }
    /// `--accent`: what is selected.
    pub fn selected() -> Rgba {
        pick(0x1f1f23, 0xede9fe)
    }
    /// `--muted`: what the pointer is over.
    pub fn hovered() -> Rgba {
        pick(0x18181b, 0xf1f5f9)
    }
    /// `--foreground`.
    pub fn text() -> Rgba {
        pick(0xe4e4e7, 0x1a1a2e)
    }
    /// `--muted-foreground`.
    pub fn text_dim() -> Rgba {
        pick(0xa1a1aa, 0x64748b)
    }
    pub fn text_faint() -> Rgba {
        pick(0x71717a, 0x94a3b8)
    }
    /// `--primary`.
    pub fn accent() -> Rgba {
        pick(0x818cf8, 0x6366f1)
    }
    /// The second stop of the IDE's gradients (`purple-400` / `purple-500`).
    pub fn accent_alt() -> Rgba {
        pick(0xc084fc, 0xa855f7)
    }
    /// The "Ready" dot in the status bar (`emerald-400` / `emerald-500`).
    pub fn ready() -> Rgba {
        pick(0x34d399, 0x10b981)
    }
    /// Solid, and the opposite of the paper it sits on rather than of the
    /// app chrome: dark on white pages, light on dark ones.
    pub fn caret() -> Rgba {
        rgb(if dark_pages() { 0xe4e4e7 } else { 0x1b1b1f })
    }
    /// The page behind a thumbnail, before its image is in.
    pub fn paper() -> Rgba {
        if dark_pages() {
            let [r, g, b] = jade_painter::paper::to_dark_paper_u8([0xff, 0xff, 0xff]);
            rgb(u32::from_be_bytes([0, r, g, b]))
        } else {
            rgb(0xffffff)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn modes_round_trip_through_their_names() {
            for mode in [Mode::System, Mode::Light, Mode::Dark] {
                assert_eq!(Mode::parse(mode.name()), Some(mode));
                assert_eq!(Mode::from_u8(mode.to_u8()), mode);
            }
            assert_eq!(Mode::parse(" dark\n"), Some(Mode::Dark));
            assert_eq!(Mode::parse("sepia"), None);
        }

        #[test]
        fn system_mode_follows_the_appearance_and_the_others_do_not() {
            set_mode(Mode::System);
            set_system(WindowAppearance::Light);
            assert!(!is_dark());
            set_system(WindowAppearance::VibrantDark);
            assert!(is_dark());
            set_mode(Mode::Light);
            assert!(!is_dark());
            set_system(WindowAppearance::Dark);
            assert!(!is_dark(), "an explicit choice outlasts the system's");
            set_mode(Mode::System);
            assert!(is_dark(), "going back to System picks up what it said");
            set_mode(Mode::Dark);
        }

        #[test]
        fn dark_pages_need_the_wish_as_well_as_the_dark_palette() {
            // Only the wish is switched here: the palette is shared with the
            // test above, which runs alongside.
            set_dark_pages_wanted(false);
            assert!(!dark_pages(), "the paper stays white when not wanted");
            set_dark_pages_wanted(true);
        }
    }
}

/// The shell's icons, compiled in. Each is a 24×24 stroke drawing in the
/// style of the IDE's icon set, drawn for this app; gpui tints them with the
/// element's text colour.
pub(crate) struct Assets;

impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        macro_rules! icons {
            ($($name:literal),* $(,)?) => {
                match path {
                    $(concat!("icons/", $name, ".svg") => Some(include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"), "/assets/icons/", $name, ".svg"
                    )).as_slice()),)*
                    _ => None,
                }
            };
        }
        let bytes = icons!(
            "a-arrow-down",
            "a-arrow-up",
            "align-center",
            "align-justify",
            "align-left",
            "align-right",
            "arrow-down-to-line",
            "arrow-left-to-line",
            "arrow-right-to-line",
            "arrow-up-to-line",
            "baseline",
            "bold",
            "borders",
            "chart-column",
            "check",
            "chevron-down",
            "chevron-right",
            "chevrons-up-down",
            "clock",
            "eraser",
            "eye-off",
            "file",
            "file-plus",
            "file-text",
            "files",
            "filter",
            "folder",
            "folder-open",
            "folder-plus",
            "highlighter",
            "image",
            "indent-decrease",
            "indent-increase",
            "italic",
            "list",
            "list-ordered",
            "merge",
            "moon",
            "paint-bucket",
            "paintbrush",
            "panels-top-left",
            "panel-right",
            "play",
            "presentation",
            "printer",
            "redo",
            "settings-2",
            "shapes",
            "sheet",
            "split",
            "square",
            "strikethrough",
            "subscript",
            "sun",
            "superscript",
            "table",
            "trash-2",
            "type",
            "underline",
            "undo",
            "wrap-text",
            "x",
            "zap",
            // The Word view's paragraph, list and design tools.
            "align-distribute",
            "blocks",
            "list-tree",
            "palette",
            "ruler",
            "sparkles",
            "type-plus",
        );
        Ok(bytes.map(std::borrow::Cow::Borrowed))
    }

    fn list(&self, _path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(Vec::new())
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
            RibbonTab::Home => t("doc.tab.home"),
            RibbonTab::Insert => t("doc.tab.insert"),
            RibbonTab::Design => t("doc.tab.design"),
            RibbonTab::Layout => t("doc.tab.layout"),
            RibbonTab::References => t("doc.tab.references"),
            RibbonTab::Review => t("doc.tab.review"),
            RibbonTab::View => t("doc.tab.view"),
        }
    }
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
    /// Word's two options: 大文字と小文字を区別する, 完全に一致する単語だけ.
    options: FindOptions,
    /// The match the last search landed on, which 置換 replaces.
    current: Option<Found>,
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

/// The caret, the selection and the edit count the ribbon's formatting was
/// read for.
type FormatKey = (
    (usize, usize),
    Option<((usize, usize), (usize, usize))>,
    u64,
);

struct DocumentView {
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
    /// A new document, not saved anywhere yet: `path` is only its name,
    /// and ⌘S asks where to save it.
    untitled: bool,
    /// Why the last table command did nothing, if it did nothing.
    table_status: Option<SharedString>,
    /// The last search query and where it got to, shown in the panel.
    find_query: Option<String>,
    find_status: Option<SharedString>,
    /// Which ribbon tab is showing.
    ribbon_tab: RibbonTab,
    /// The ribbon's open list, table picker and style gallery.
    ribbon: ribbon::Ribbon,
    /// Bumped whenever the pages are repainted, so the formatting the ribbon
    /// shows is read again after an edit.
    edits: u64,
    /// The formatting at the caret, and the caret, selection and edit it was
    /// read for: reading it runs queries, so not on every frame.
    format: Option<(FormatKey, FormatState)>,
    /// The find bar, while it is open. Open, it takes the keyboard: typing goes
    /// into its field rather than into the document.
    find_bar: Option<FindBar>,
    /// A header or footer being typed, in its bar over the pages; it takes
    /// the keyboard as the find bar does.
    story_bar: Option<word_page::StoryBar>,
    /// A comment being typed — new, a reply, an edit — in its bar.
    comment_bar: Option<word_review::CommentBar>,
    /// Whether the comments pane is open beside the pages.
    show_comments: bool,
    /// The paragraph, list, style and design tools' dialog, rulers and pane.
    tools: word_tools::WordTools,
    /// Display scale. The pages are rasterised at their own size and drawn
    /// scaled, so zooming costs a layout pass in gpui rather than a re-render —
    /// at the price of resampling rather than re-rasterising crisply.
    zoom: f32,
    /// The window's backing scale the pages were last rasterised at. A page
    /// painted at 1x and shown on a 2x screen is stretched, and its text goes
    /// soft; following the window keeps one page pixel per screen pixel.
    raster_scale: f32,
    /// Whether the pages were last painted on dark paper. Followed per frame,
    /// like `raster_scale`, so a theme change from anywhere repaints them.
    dark_pages: bool,
    /// Where a shift-extended selection is anchored. Separate from
    /// `drag_anchor`: a keyboard selection outlives the mouse being down.
    selection_anchor: Option<(usize, usize)>,
    /// Where a drag started, while the mouse is down. Selection is anchored
    /// here and extended to wherever the pointer is.
    drag_anchor: Option<(usize, usize)>,
    /// The picture or shape last clicked, which 文字列の折り返し acts on and
    /// an outline shows.
    selected_object: Option<ObjectHit>,
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
    /// Whether the details panel on the right is open.
    show_inspector: bool,
    /// Whether there are edits since the document was opened or saved.
    dirty: bool,
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
        self.edits += 1;
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
        // A dialog over the pages has every key while it is open.
        if self.tools.dialog.is_some() {
            if self.handle_dialog_key(event, cx) {
                cx.stop_propagation();
            }
            // Nothing reaches the document while a dialog is open.
            return;
        }
        // The bar owns the keyboard while it is open, so Enter means "next
        // match" and Backspace edits the query rather than the document.
        if self.comment_bar.is_some() && self.handle_comment_bar_key(event, cx) {
            cx.stop_propagation();
            return;
        }
        if self.story_bar.is_some() && self.handle_story_bar_key(event, cx) {
            cx.stop_propagation();
            return;
        }
        if self.find_bar.is_some() && self.handle_find_bar_key(event, cx) {
            // Taken: the platform must not also type it (Tab, Enter) into
            // the field through the input handler.
            cx.stop_propagation();
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
                    self.rebuild_after_edit();
                    cx.notify();
                }
                Err(error) => eprintln!("delete failed: {error}"),
            }
            return;
        }

        let (paragraph, offset) = self.insert_at;
        let modifiers = event.keystroke.modifiers;
        let key = event.keystroke.key.as_str();
        match key {
            // ⌥⌫ and ⌥⌦ take a word, ⌘⌫ the line up to the caret, as on a
            // Mac: what goes is selected, then deleted, in one edit.
            "backspace" | "delete"
                if modifiers.alt || (modifiers.platform && key == "backspace") =>
            {
                let to = if modifiers.platform {
                    self.line_edge(false)
                } else {
                    self.word_step((paragraph, offset), key == "delete")
                };
                if to != self.insert_at {
                    let page = self.caret_page_1idx();
                    let edited = self
                        .document
                        .select(self.insert_at, to, page)
                        .and_then(|()| self.document.replace_selection("", page));
                    match edited {
                        Ok(caret) => {
                            self.selection_anchor = None;
                            self.insert_at = caret;
                            self.caret = self.document.place_caret(caret.0, caret.1);
                            self.rebuild_after_edit();
                            cx.notify();
                        }
                        Err(error) => eprintln!("delete failed: {error}"),
                    }
                }
            }
            "backspace" if offset > 0 => {
                let text = self.current_paragraph_text();
                let removed = text[..offset].chars().next_back().map(char::len_utf8);
                if let Some(removed) = removed {
                    let page = self.caret_page_1idx();
                    let result = self.document.delete_text(paragraph, offset, true, 1, page);
                    self.after_edit(result, (paragraph, offset - removed), cx);
                }
            }
            // At a paragraph's start, Backspace joins it to the one before.
            "backspace" if paragraph > 0 => {
                let joined_at = self
                    .document
                    .paragraph_text(paragraph - 1)
                    .map_or(0, |text| text.len());
                let page = self.caret_page_1idx();
                let result = self.document.delete_text(paragraph, 0, true, 1, page);
                self.rebuild_all_images();
                self.after_edit(result, (paragraph - 1, joined_at), cx);
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
                self.rebuild_after_edit();
                self.after_edit(result, (paragraph + 1, 0), cx);
            }
            // In a table, to the next or the previous cell; elsewhere a tab
            // typed, as in Word. Taken here, so the platform does not type
            // the tab a second time through the input handler.
            "tab" => {
                cx.stop_propagation();
                self.tab(modifiers.shift, cx);
            }
            "left" | "right" | "up" | "down" | "home" | "end" => {
                let extending = modifiers.shift;
                // Anchored where the caret was before this keystroke, unless a
                // shift-selection is already running.
                let anchor = if extending {
                    self.selection_anchor.unwrap_or(self.insert_at)
                } else {
                    self.selection_anchor = None;
                    self.insert_at
                };
                // ⌘ with an arrow (Ctrl with Home or End elsewhere) goes to a
                // line's or the document's ends, ⌥ a word along.
                let long = modifiers.platform || modifiers.control;
                match (key, long, modifiers.alt) {
                    ("up", true, _) | ("home", true, _) => {
                        self.jump_to(self.document_edge(false), cx)
                    }
                    ("down", true, _) | ("end", true, _) => {
                        self.jump_to(self.document_edge(true), cx)
                    }
                    ("left", true, _) | ("home", false, _) => {
                        self.jump_to(self.line_edge(false), cx)
                    }
                    ("right", true, _) | ("end", false, _) => {
                        self.jump_to(self.line_edge(true), cx)
                    }
                    ("left", false, true) => {
                        self.jump_to(self.word_step(self.insert_at, false), cx)
                    }
                    ("right", false, true) => {
                        self.jump_to(self.word_step(self.insert_at, true), cx)
                    }
                    ("left", ..) => self.step_caret(-1, cx),
                    ("right", ..) => self.step_caret(1, cx),
                    ("up", ..) => self.step_line(-1.0, cx),
                    _ => self.step_line(1.0, cx),
                }
                if extending {
                    self.extend_selection_to_caret(anchor, cx);
                }
            }
            _ => {}
        }
    }

    /// Tab: in a table, the next cell (⇧ the previous one) with its text
    /// selected, a row added past the last; elsewhere a tab typed.
    fn tab(&mut self, backwards: bool, cx: &mut Context<Self>) {
        use jade_office_model::word::CellStep;
        let (paragraph, offset) = self.insert_at;
        self.document.place_caret(paragraph, offset);
        let step = WordFormatting::step_cell(&mut self.document, !backwards);
        match step {
            Ok(CellStep::Select(from, to)) => {
                // A row added is a re-flow: every page is re-read.
                self.rebuild_all_images();
                self.selection_anchor = Some(from);
                self.insert_at = to;
                self.caret = self.document.place_caret(to.0, to.1);
                if from != to {
                    let page = self.caret_page_1idx();
                    if let Err(error) = self.document.select(from, to, page) {
                        eprintln!("tab failed: {error}");
                    }
                }
                self.refresh_caret_page_image();
                cx.notify();
            }
            Ok(CellStep::Stay) => {}
            // At a list item's start, a level down (or up), as in Word.
            Ok(CellStep::NotInTable) if self.tab_in_list(backwards, cx) => {}
            Ok(CellStep::NotInTable) if !backwards => {
                let page = self.caret_page_1idx();
                let result = self.document.insert_text(paragraph, offset, "\t", page);
                self.after_edit(result, (paragraph, offset + 1), cx);
            }
            Ok(CellStep::NotInTable) => {}
            Err(error) => eprintln!("tab failed: {error}"),
        }
    }

    /// The caret to `position`, drawn there.
    fn jump_to(&mut self, position: (usize, usize), cx: &mut Context<Self>) {
        self.insert_at = position;
        self.caret = self.document.place_caret(position.0, position.1);
        cx.notify();
    }

    /// One word along from `position`, as ⌥← and ⌥→ go; at a paragraph's
    /// edge, into the paragraph beside it.
    fn word_step(&self, (paragraph, offset): (usize, usize), forwards: bool) -> (usize, usize) {
        use jade_office_model::text::{word_end_after, word_start_before};
        let text = self.document.paragraph_text(paragraph).unwrap_or_default();
        let offset = offset.min(text.len());
        if forwards && offset < text.len() {
            (paragraph, word_end_after(&text, offset))
        } else if !forwards && offset > 0 {
            (paragraph, word_start_before(&text, offset))
        } else {
            self.character_step((paragraph, offset), forwards)
        }
    }

    /// One character along from `position`, across paragraph ends.
    fn character_step(
        &self,
        (paragraph, offset): (usize, usize),
        forwards: bool,
    ) -> (usize, usize) {
        let text = self.document.paragraph_text(paragraph).unwrap_or_default();
        let offset = offset.min(text.len());
        if forwards {
            match text[offset..].chars().next() {
                Some(next) => (paragraph, offset + next.len_utf8()),
                None if paragraph + 1 < self.document.paragraph_count() => (paragraph + 1, 0),
                None => (paragraph, offset),
            }
        } else {
            match text[..offset].chars().next_back() {
                Some(previous) => (paragraph, offset - previous.len_utf8()),
                None if paragraph > 0 => (
                    paragraph - 1,
                    self.document
                        .paragraph_text(paragraph - 1)
                        .map_or(0, |text| text.len()),
                ),
                None => (paragraph, offset),
            }
        }
    }

    /// The start or the end of the line the caret is drawn on, along the
    /// caret's own paragraph: in a table row the far side of the page is
    /// another cell's text.
    fn line_edge(&self, end: bool) -> (usize, usize) {
        let Some(caret) = self.caret else {
            return self.insert_at;
        };
        let paragraph = self.insert_at.0;
        let x = if end { f64::MAX / 4.0 } else { f64::MIN / 4.0 };
        self.document
            .renderer()
            .selection_manager()
            .hit_test_matching(x, caret.document_y + caret.height / 2.0, |index| {
                index == paragraph
            })
            .map_or(self.insert_at, |position| {
                (position.paragraph, position.offset)
            })
    }

    /// The start or the end of the document's body (⌘↑ and ⌘↓).
    fn document_edge(&self, end: bool) -> (usize, usize) {
        let word = self.document.word_document();
        let count = self.document.paragraph_count();
        let body = |index: &usize| word.is_body_paragraph_surface(*index);
        let paragraph = if end {
            (0..count).rev().find(body)
        } else {
            (0..count).find(body)
        }
        .unwrap_or(0);
        let offset = if end {
            self.document
                .paragraph_text(paragraph)
                .map_or(0, |text| text.len())
        } else {
            0
        };
        (paragraph, offset)
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
        title: &str,
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
            // The glyph is often an abbreviation (A+, x₂); the title says
            // what the button does.
            .when(title != glyph, |button| {
                button.tooltip(Tooltip::text(title.to_string()))
            })
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
    fn ribbon(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                                view.ribbon.popup = None;
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
                                .child(t("doc.export_word"))
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
            .children(
                self.ribbon
                    .popup
                    .map(|_| ribbon::backdrop(window.viewport_size(), cx)),
            )
    }

    /// The groups under the selected tab.
    fn ribbon_groups(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let state = self.format_state();
        match self.ribbon_tab {
            RibbonTab::Home => {
                let mut groups = vec![self.clipboard_group(cx)];
                groups.extend(ribbon::home(&self.ribbon, &state, cx));
                // Lists and the paragraph's settings after 段落, the style
                // tools after the gallery.
                let lists = self.list_group(cx);
                groups.insert(3.min(groups.len()), lists);
                let styles = self.style_tools_group(cx);
                groups.insert(5.min(groups.len()), styles);
                groups
            }
            RibbonTab::Insert => vec![
                ribbon::insert_table(&self.ribbon, cx).remove(0),
                self.quick_parts_group(cx),
                self.picture_group(cx),
                word_page::header_footer(&self.ribbon, cx),
                Self::ribbon_group(
                    t("doc.group.pages"),
                    vec![Self::ribbon_row(vec![self
                        .ribbon_button(
                            "blank-page",
                            t("doc.blank_page"),
                            t("doc.blank_page"),
                            false,
                            cx,
                            |view, cx| view.insert_blank_page(cx),
                        )
                        .into_any_element()])],
                )
                .into_any_element(),
                Self::ribbon_group(
                    t("doc.comment"),
                    vec![Self::ribbon_row(vec![self
                        .ribbon_button(
                            "comment",
                            t("doc.comment"),
                            t("doc.comment"),
                            false,
                            cx,
                            |view, cx| view.add_comment(cx),
                        )
                        .into_any_element()])],
                )
                .into_any_element(),
            ],
            RibbonTab::Design => self
                .design_groups(cx)
                .into_iter()
                .chain([
                    Self::ribbon_group(
                        t("doc.page_color"),
                        vec![Self::ribbon_row(vec![
                            self.ribbon_button(
                                "page-cream",
                                t("doc.color.cream"),
                                t("doc.page_color"),
                                false,
                                cx,
                                |view, cx| view.set_page_background(Some("FFF9E6"), cx),
                            )
                            .into_any_element(),
                            self.ribbon_button(
                                "page-grey",
                                t("doc.color.light_gray"),
                                t("doc.page_color"),
                                false,
                                cx,
                                |view, cx| view.set_page_background(Some("F2F2F2"), cx),
                            )
                            .into_any_element(),
                            self.ribbon_button(
                                "page-white",
                                t("doc.none"),
                                t("doc.page_color_none"),
                                false,
                                cx,
                                |view, cx| view.set_page_background(None, cx),
                            )
                            .into_any_element(),
                        ])],
                    )
                    .into_any_element(),
                    Self::ribbon_group(
                        t("doc.page_borders"),
                        vec![Self::ribbon_row(vec![
                            self.ribbon_button(
                                "page-border",
                                t("doc.box"),
                                t("doc.page_borders_box"),
                                false,
                                cx,
                                |view, cx| view.set_page_borders(true, cx),
                            )
                            .into_any_element(),
                            self.ribbon_button(
                                "page-no-border",
                                t("doc.none"),
                                t("doc.page_borders_none"),
                                false,
                                cx,
                                |view, cx| view.set_page_borders(false, cx),
                            )
                            .into_any_element(),
                        ])],
                    )
                    .into_any_element(),
                ])
                .collect(),
            RibbonTab::Layout => vec![
                {
                    let japanese = i18n::language() == i18n::Language::Japanese;
                    let setup = self.document.page_setup();
                    let preset = self.document.margin_preset(japanese);
                    word_page::page_setup(&self.ribbon, &setup, preset, japanese, cx)
                },
                Self::ribbon_group(
                    t("doc.group.paragraph"),
                    vec![Self::ribbon_row(vec![
                        self.ribbon_button(
                            "layout-indent-less",
                            t("doc.indent_less"),
                            t("doc.indent_less_title"),
                            false,
                            cx,
                            |view, cx| view.run_ribbon(Action::Indent(false), cx),
                        )
                        .into_any_element(),
                        self.ribbon_button(
                            "layout-indent-more",
                            t("doc.indent_more"),
                            t("doc.indent_more_title"),
                            false,
                            cx,
                            |view, cx| view.run_ribbon(Action::Indent(true), cx),
                        )
                        .into_any_element(),
                    ])],
                )
                .into_any_element(),
                Self::ribbon_group(
                    t("doc.page_break"),
                    vec![Self::ribbon_row(vec![self
                        .ribbon_button(
                            "layout-break",
                            t("doc.page_break"),
                            t("doc.page_break"),
                            false,
                            cx,
                            |view, cx| view.insert_blank_page(cx),
                        )
                        .into_any_element()])],
                )
                .into_any_element(),
            ]
            .into_iter()
            .chain(ribbon::table_tools(&self.ribbon, &state, cx))
            .collect(),
            RibbonTab::References => vec![Self::ribbon_group(
                t("doc.toc"),
                vec![Self::ribbon_row(vec![self
                    .ribbon_button(
                        "toc",
                        t("doc.toc"),
                        t("doc.toc_title"),
                        false,
                        cx,
                        |view, cx| view.insert_table_of_contents(cx),
                    )
                    .into_any_element()])],
            )
            .into_any_element()],
            RibbonTab::Review => self.review_groups(cx),
            RibbonTab::View => {
                let marks = self.document.formatting_marks();
                vec![
                    self.ruler_group(cx),
                    Self::ribbon_group(
                        t("doc.group.show"),
                        vec![Self::ribbon_row(vec![self
                            .ribbon_button(
                                "marks",
                                t("doc.marks"),
                                t("doc.marks_title"),
                                marks,
                                cx,
                                |view, cx| view.toggle_formatting_marks(cx),
                            )
                            .into_any_element()])],
                    )
                    .into_any_element(),
                    Self::ribbon_group(
                        t("doc.group.zoom"),
                        vec![Self::ribbon_row(vec![
                            self.ribbon_button(
                                "zoom-out",
                                t("doc.zoom_out"),
                                t("doc.zoom_out"),
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
                                t("doc.zoom_actual"),
                                t("doc.zoom_actual"),
                                false,
                                cx,
                                |view, cx| view.set_zoom(1.0, cx),
                            )
                            .into_any_element(),
                            self.ribbon_button(
                                "zoom-in",
                                t("doc.zoom_in"),
                                t("doc.zoom_in"),
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
                        t("doc.group.pages"),
                        vec![Self::ribbon_row(vec![
                            self.ribbon_button(
                                "prev-page",
                                t("doc.previous"),
                                t("doc.previous_page"),
                                false,
                                cx,
                                |view, cx| view.go_to_page(-1, cx),
                            )
                            .into_any_element(),
                            self.ribbon_button(
                                "next-page",
                                t("doc.next"),
                                t("doc.next_page"),
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

    /// 挿入's 図: a picture from a file, and the wrap of the one last
    /// clicked.
    fn picture_group(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let selected = self
            .selected_object
            .is_some_and(|object| placed_object(&object).is_picture());
        let mut wraps = vec![self
            .ribbon_button(
                "insert-picture",
                t("picture.insert"),
                t("picture.insert_title"),
                false,
                cx,
                |view, cx| view.prompt_for_picture(cx),
            )
            .into_any_element()];
        if selected {
            for (index, wrap) in jade_office_model::word::picture::WRAPS
                .into_iter()
                .enumerate()
            {
                wraps.push(
                    self.ribbon_button(
                        WRAP_IDS[index],
                        wrap_label(wrap),
                        t("picture.wrap"),
                        false,
                        cx,
                        move |view, cx| view.wrap_picture(wrap, cx),
                    )
                    .into_any_element(),
                );
            }
        }
        Self::ribbon_group(t("picture.group"), vec![Self::ribbon_row(wraps)]).into_any_element()
    }

    /// The ホーム tab's clipboard, which only the native app can paste into
    /// without asking.
    fn clipboard_group(&mut self, cx: &mut Context<Self>) -> AnyElement {
        Self::ribbon_group(
            t("doc.group.clipboard"),
            vec![Self::ribbon_row(vec![
                self.ribbon_button(
                    "paste",
                    t("doc.paste"),
                    t("doc.paste_title"),
                    false,
                    cx,
                    |view, cx| view.paste(cx),
                )
                .into_any_element(),
                self.ribbon_button(
                    "cut",
                    t("doc.cut"),
                    t("doc.cut_title"),
                    false,
                    cx,
                    |view, cx| view.cut(cx),
                )
                .into_any_element(),
                self.ribbon_button(
                    "copy",
                    t("doc.copy"),
                    t("doc.copy_title"),
                    false,
                    cx,
                    |view, cx| view.copy(cx),
                )
                .into_any_element(),
            ])],
        )
        .into_any_element()
    }

    /// The find bar, drawn above the pages while it is open.
    ///
    /// A caret bar after the focused field, because the field has no cursor of
    /// its own: without it there is no way to see which of the two fields the
    /// next keystroke goes into.
    fn find_bar_element(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let bar = self.find_bar.as_ref()?;
        let option = |id: &'static str, glyph: &'static str, title: &'static str, on: bool| {
            div()
                .id(id)
                .flex()
                .items_center()
                .justify_center()
                .min_w(px(28.0))
                .h(px(24.0))
                .px_1()
                .rounded_md()
                .cursor_pointer()
                .text_sm()
                .when(on, |button| {
                    button.bg(theme::selected()).text_color(theme::accent())
                })
                .when(!on, |button| button.text_color(theme::text_dim()))
                .hover(|button| button.bg(theme::hovered()))
                .tooltip(Tooltip::text(title))
                .child(glyph)
        };
        let match_case = option(
            "find-match-case",
            "Aa",
            t("doc.find.match_case"),
            bar.options.match_case,
        )
        .on_click(cx.listener(|view, _event, _window, cx| view.toggle_find_option(false, cx)));
        let whole_word = option(
            "find-whole-word",
            "ab|",
            t("doc.find.whole_word"),
            bar.options.whole_word,
        )
        .on_click(cx.listener(|view, _event, _window, cx| view.toggle_find_option(true, cx)));
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
                .child(field(
                    t("doc.find.query"),
                    &bar.query,
                    !bar.editing_replacement,
                ))
                .child(match_case)
                .child(whole_word)
                .child(field(
                    t("doc.find.replacement"),
                    &bar.replacement,
                    bar.editing_replacement,
                ))
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
                        .child(t("doc.find.keys")),
                ),
        )
    }

    /// The field a bar over the pages is taking typing into — a header's or
    /// a footer's — while one is open.
    fn text_bar(&self) -> Option<&jade_office_model::text::TextEdit> {
        if self.tools.dialog.is_some() {
            return self.dialog_field();
        }
        match (&self.comment_bar, &self.story_bar) {
            (Some(bar), _) => Some(&bar.text),
            (None, Some(bar)) => Some(&bar.text),
            (None, None) => None,
        }
    }

    fn text_bar_mut(&mut self) -> Option<&mut jade_office_model::text::TextEdit> {
        if self.tools.dialog.is_some() {
            return self.dialog_field_mut();
        }
        match (&mut self.comment_bar, &mut self.story_bar) {
            (Some(bar), _) => Some(&mut bar.text),
            (None, Some(bar)) => Some(&mut bar.text),
            (None, None) => None,
        }
    }

    /// ヘッダーの編集 / フッターの編集: the bar over the pages, holding the
    /// story's first line (its tab-separated left, centre and right).
    fn open_story_bar(&mut self, story: Story, cx: &mut Context<Self>) {
        self.find_bar = None;
        self.comment_bar = None;
        let text = self.document.story_text(story);
        self.story_bar = Some(word_page::StoryBar {
            story,
            text: jade_office_model::text::TextEdit::new(text),
        });
        cx.notify();
    }

    /// Keys the header and footer bar takes: Enter puts the text in, Escape
    /// leaves it as it was, Tab types the tab between Word's zones.
    fn handle_story_bar_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let Some(bar) = self.story_bar.as_mut() else {
            return false;
        };
        if bar.text.marked.is_some() {
            return false;
        }
        match event.keystroke.key.as_str() {
            "escape" => self.story_bar = None,
            "enter" | "return" => {
                let Some(bar) = self.story_bar.take() else {
                    return false;
                };
                match self.document.set_story_text(bar.story, &bar.text.text) {
                    Ok(()) => {
                        self.dirty = true;
                        self.caret = self
                            .document
                            .place_caret(self.insert_at.0, self.insert_at.1);
                        self.rebuild_all_images();
                    }
                    Err(error) => self.table_status = Some(SharedString::from(error)),
                }
            }
            "tab" => bar.text.insert("\t"),
            "backspace" => bar.text.backspace(),
            "delete" => bar.text.delete(),
            "left" => bar.text.step(false),
            "right" => bar.text.step(true),
            "home" | "up" => bar.text.to_end(false),
            "end" | "down" => bar.text.to_end(true),
            _ => return false,
        }
        cx.notify();
        true
    }

    /// The header or footer bar, over the pages while it is open.
    fn story_bar_element(&self) -> Option<impl IntoElement> {
        let bar = self.story_bar.as_ref()?;
        let (before, after) = bar.text.split();
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
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_faint())
                        .child(bar.label()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .min_w(px(320.0))
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .bg(theme::canvas())
                        .border_1()
                        .border_color(theme::accent())
                        .text_sm()
                        .text_color(theme::text())
                        // A tab shows as Word's zone mark.
                        .child(before.replace('\t', " ⇥ "))
                        .child(div().w(px(1.0)).h(px(16.0)).bg(theme::accent()))
                        .child(after.replace('\t', " ⇥ ")),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_faint())
                        .child(t("page.story_keys")),
                ),
        )
    }

    /// The text the platform's input handler is editing: the find bar's field
    /// while it is open, otherwise the paragraph the caret is in.
    fn focused_text(&self) -> String {
        if let Some(text) = self.text_bar() {
            return text.text.clone();
        }
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
        let alt = event.keystroke.modifiers.alt;
        match event.keystroke.key.as_str() {
            "escape" => {
                self.close_find_bar(cx);
                true
            }
            // ⌥⌘C and ⌥⌘W: the bar's two options.
            "c" if platform && alt => {
                self.toggle_find_option(false, cx);
                true
            }
            "w" if platform && alt => {
                self.toggle_find_option(true, cx);
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

    /// Turns 完全に一致する単語だけ (`whole_word`) or 大文字と小文字を区別する on
    /// or off; what the query matches is counted again.
    fn toggle_find_option(&mut self, whole_word: bool, cx: &mut Context<Self>) {
        if let Some(bar) = self.find_bar.as_mut() {
            if whole_word {
                bar.options.whole_word = !bar.options.whole_word;
            } else {
                bar.options.match_case = !bar.options.match_case;
            }
            bar.current = None;
        }
        self.update_find_count();
        cx.notify();
    }

    /// The bar's options, or Word's defaults with the bar closed.
    fn find_options(&self) -> FindOptions {
        self.find_bar
            .as_ref()
            .map(|bar| bar.options)
            .unwrap_or_default()
    }

    /// Puts what has been typed into the bar's query into effect.
    fn sync_find_query(&mut self, cx: &mut Context<Self>) {
        let Some(bar) = self.find_bar.as_mut() else {
            return;
        };
        bar.current = None;
        let bar = &*bar;
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
        let count = self.document.find_all(&query, self.find_options()).len();
        self.find_status = Some(SharedString::from(if count == 0 {
            i18n::format("doc.find.none", &[("query", &query)])
        } else {
            i18n::format("doc.find.count", &[("query", &query), ("n", &count)])
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
        // The match the last find landed on. Without one there is nothing to
        // replace yet, so find first.
        let Some(hit) = bar.current else {
            self.find_query = Some(query);
            return self.find_next(cx);
        };
        match self.document.replace_found(hit, &replacement) {
            Ok(after) => {
                self.insert_at = after;
                self.caret = self
                    .document
                    .place_caret(self.insert_at.0, self.insert_at.1);
                self.rebuild_after_edit();
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
        let options = bar.options;
        match WordFind::replace_all(&mut self.document, &query, &replacement, options) {
            Ok(replaced) => {
                if let Some(bar) = self.find_bar.as_mut() {
                    bar.current = None;
                }
                // Replacing re-flows text onto other pages, so every page is
                // rebuilt rather than the one the caret is on.
                self.rebuild_after_edit();
                self.caret = self
                    .document
                    .place_caret(self.insert_at.0, self.insert_at.1);
                self.find_status = Some(SharedString::from(i18n::format(
                    "doc.find.replaced",
                    &[
                        ("query", &query),
                        ("replacement", &replacement),
                        ("n", &replaced),
                    ],
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
        let hits = self.document.find_all(&query, self.find_options());
        if hits.is_empty() {
            self.find_status = Some(SharedString::from(i18n::format(
                "doc.find.none",
                &[("query", &query)],
            )));
            return cx.notify();
        }
        let (paragraph, offset) = self.insert_at;
        // Wrapping: the first hit strictly after the caret, else the first of
        // all. Strictly, so repeating does not land on the same match forever.
        let next = hits
            .iter()
            .find(|hit| hit.start() > (paragraph, offset))
            .copied()
            .unwrap_or(hits[0]);
        if let Some(bar) = self.find_bar.as_mut() {
            bar.current = Some(next);
        }
        let position = hits.iter().position(|hit| *hit == next).unwrap_or(0);
        self.find_status = Some(SharedString::from(i18n::format(
            "doc.find.position",
            &[
                ("query", &query),
                ("n", &(position + 1)),
                ("count", &hits.len()),
            ],
        )));

        let page = self.caret_page_1idx();
        if self.document.select(next.start(), next.end(), page).is_ok() {
            self.insert_at = next.end();
            // Where the caret is drawn, the selection kept: the replace acts
            // on the match, and a typed key on the selection.
            self.caret = self.document.place_caret(next.paragraph, next.end);
            let _ = self.document.select(next.start(), next.end(), page);
            if let Some(caret) = self.caret {
                let (page_of_hit, _) = self.document.page_and_offset(caret.document_y);
                self.scroll.scroll_to_item(page_of_hit);
                self.current_page = page_of_hit;
            }
            self.rebuild_all_images();
            cx.notify();
        }
    }

    fn set_page_background(&mut self, color: Option<&str>, cx: &mut Context<Self>) {
        if self.document.set_page_background(color).is_ok() {
            self.rebuild_after_edit();
            cx.notify();
        }
    }

    fn set_page_borders(&mut self, boxed: bool, cx: &mut Context<Self>) {
        if self.document.set_page_borders(boxed).is_ok() {
            self.rebuild_after_edit();
            cx.notify();
        }
    }

    // ---- the ribbon's commands ----------------------------------------------

    /// The formatting at the caret, read again only when the caret, the
    /// selection or the pages changed.
    fn format_state(&mut self) -> FormatState {
        let key = (self.insert_at, self.document.selection(), self.edits);
        match &self.format {
            Some((cached, state)) if *cached == key => state.clone(),
            _ => {
                let state = WordFormatting::format_state(&self.document);
                self.format = Some((key, state.clone()));
                state
            }
        }
    }

    /// A ribbon control was clicked.
    fn ribbon_action(&mut self, action: Action, _window: &mut Window, cx: &mut Context<Self>) {
        self.run_ribbon(action, cx);
    }

    /// Runs a ribbon command — from a button, a list or a shortcut — through
    /// `jade-office-model`'s shared commands: formatting keeps the selection,
    /// so the next command acts on the same text; a table command that adds
    /// or removes cells moves the caret to where the command left it.
    fn run_ribbon(&mut self, action: Action, cx: &mut Context<Self>) {
        if action.is_chrome() {
            match action {
                Action::Popup(popup) => self.ribbon.toggle(popup),
                Action::ClosePopup => self.ribbon.popup = None,
                Action::Grid(columns, rows) => self.ribbon.grid = (columns, rows),
                _ => {}
            }
            return cx.notify();
        }
        self.ribbon.popup = None;
        // Direct formatting a self-updating style takes in (自動的に更新する).
        let formats = matches!(
            action,
            Action::Toggle(_)
                | Action::FontFamily(_)
                | Action::FontSize(_)
                | Action::StepFontSize(_)
                | Action::FontColor(_)
                | Action::Align(_)
                | Action::LineSpacing(_)
                | Action::Indent(_)
                | Action::Shading(_)
                | Action::ParagraphBorders(_)
        );
        if let Action::Word(command) = action {
            return self.run_tool(command, cx);
        }
        let document = &mut self.document;
        let done = match action {
            Action::Undo => return self.step_history(true, cx),
            Action::Redo => return self.step_history(false, cx),
            Action::InsertTable(rows, columns) => {
                let moved = WordFormatting::insert_table(document, rows, columns);
                return self.ribbon_moved(moved, cx);
            }
            Action::Table(command) if command.is_structural() => {
                let moved = document.table_command(command);
                return self.ribbon_moved(moved, cx);
            }
            Action::Table(command) => document.table_command(command).map(|_| ()),
            Action::Toggle(format) => document.toggle_format(format),
            Action::ClearFormatting => document.clear_formatting(),
            Action::FontFamily(family) => WordFormatting::set_font_family(document, family),
            Action::FontSize(points) => WordFormatting::set_font_size(document, points),
            Action::StepFontSize(grow) => document.step_font_size(grow),
            Action::FontColor(color) => WordFormatting::set_font_color(document, &color),
            Action::Highlight(color) => document.set_highlight(color),
            Action::List(bullets) => document.toggle_list(bullets),
            Action::Indent(increase) => document.step_indent(increase),
            Action::Align(alignment) => WordFormatting::set_alignment(document, alignment),
            Action::LineSpacing(multiple) => WordFormatting::set_line_spacing(document, multiple),
            Action::Shading(color) => {
                WordFormatting::set_paragraph_shading(document, color.as_deref())
            }
            Action::ParagraphBorders(borders) => {
                WordFormatting::set_paragraph_borders(document, borders)
            }
            Action::Style(style_id) => {
                // A character or linked style from the gallery goes on as
                // Word puts it (word_styles).
                let done = match style_id.as_deref() {
                    Some(id) => {
                        jade_office_model::word::styles::WordStyles::apply_style(document, id)
                    }
                    None => WordFormatting::set_paragraph_style(document, None),
                };
                self.ribbon.read_styles(&self.document);
                done
            }
            Action::Page(command) => {
                let japanese = i18n::language() == i18n::Language::Japanese;
                WordPage::page_command(document, command, japanese)
            }
            Action::EditStory(story) => return self.open_story_bar(story, cx),
            Action::Popup(_) | Action::ClosePopup | Action::Grid(..) | Action::Word(_) => Ok(()),
        };
        match done {
            Ok(()) => {
                self.dirty = true;
                self.table_status = None;
                if formats {
                    self.auto_update_style();
                }
                // A bare caret stays put in its paragraph, but the line it is
                // on may have moved; a selection hides the caret anyway.
                if self.document.selection().is_none() {
                    self.caret = self
                        .document
                        .place_caret(self.insert_at.0, self.insert_at.1);
                }
                self.rebuild_all_images();
            }
            Err(error) => self.table_status = Some(SharedString::from(error)),
        }
        cx.notify();
    }

    /// After a command that moved the caret: put it there, with no
    /// selection.
    fn ribbon_moved(&mut self, moved: Result<(usize, usize), String>, cx: &mut Context<Self>) {
        match moved {
            Ok(caret) => {
                self.table_status = None;
                self.selection_anchor = None;
                self.insert_at = caret;
                self.caret = self.document.place_caret(caret.0, caret.1);
                self.rebuild_after_edit();
            }
            Err(error) => self.table_status = Some(SharedString::from(error)),
        }
        cx.notify();
    }

    fn insert_blank_page(&mut self, cx: &mut Context<Self>) {
        if self.document.insert_blank_page().is_ok() {
            self.rebuild_after_edit();
            cx.notify();
        }
    }

    fn insert_table_of_contents(&mut self, cx: &mut Context<Self>) {
        if self.document.insert_table_of_contents().is_ok() {
            self.rebuild_after_edit();
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

    /// The selection's text onto the clipboard, and its formatting beside it
    /// for a paste in this app that finds that text still there.
    fn copy(&mut self, cx: &mut Context<Self>) {
        let Some(text) = self.document.selected_text() else {
            return;
        };
        let clip = WordClipboard::copy_rich(&self.document);
        RICH_CLIP.with(|kept| *kept.borrow_mut() = clip);
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }

    fn cut(&mut self, cx: &mut Context<Self>) {
        self.copy(cx);
        let page = self.caret_page_1idx();
        if let Ok(caret) = self.document.replace_selection("", page) {
            self.insert_at = caret;
            self.caret = self.document.place_caret(caret.0, caret.1);
            self.rebuild_after_edit();
            cx.notify();
        }
    }

    fn paste(&mut self, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        // Copied here, and still what the clipboard holds: with its formatting.
        let clip = RICH_CLIP.with(|kept| kept.borrow().clone());
        if let Some(clip) = clip.filter(|clip| clip.matches(&text)) {
            return self.paste_rich(&clip, cx);
        }
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
        self.rebuild_after_edit();
        cx.notify();
    }

    /// A paste that keeps the formatting, as one undo step.
    fn paste_rich(&mut self, clip: &RichClip, cx: &mut Context<Self>) {
        if self.document.selection().is_none() {
            self.document
                .place_caret(self.insert_at.0, self.insert_at.1);
        }
        match WordClipboard::paste_rich(&mut self.document, clip) {
            Ok(caret) => {
                self.selection_anchor = None;
                self.insert_at = caret;
                self.caret = self.document.place_caret(caret.0, caret.1);
                self.rebuild_after_edit();
                cx.notify();
            }
            Err(error) => eprintln!("paste failed: {error}"),
        }
    }

    /// 挿入 → 画像: asks for a picture and puts it in at the caret.
    fn prompt_for_picture(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(SharedString::from(t("picture.insert"))),
        });
        cx.spawn(async move |view, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.first() else {
                return;
            };
            match std::fs::read(path) {
                Ok(bytes) => {
                    view.update(cx, |view, cx| view.insert_picture(bytes, cx))
                        .ok();
                }
                Err(error) => eprintln!("{}: {error}", path.display()),
            }
        })
        .detach();
    }

    fn insert_picture(&mut self, bytes: Vec<u8>, cx: &mut Context<Self>) {
        self.document
            .place_caret(self.insert_at.0, self.insert_at.1);
        match WordPicture::insert_picture(&mut self.document, bytes) {
            Ok(()) => {
                self.table_status = None;
                self.caret = self
                    .document
                    .place_caret(self.insert_at.0, self.insert_at.1);
                self.rebuild_after_edit();
            }
            Err(error) => self.table_status = Some(SharedString::from(error)),
        }
        cx.notify();
    }

    /// 文字列の折り返し for the picture last clicked.
    fn wrap_picture(&mut self, wrap: Wrap, cx: &mut Context<Self>) {
        let Some(object) = self.selected_object else {
            return;
        };
        let placed = placed_object(&object);
        match WordPicture::set_picture_wrap(&mut self.document, placed, wrap) {
            Ok(()) => {
                self.table_status = None;
                self.rebuild_after_edit();
            }
            Err(error) => self.table_status = Some(SharedString::from(error)),
        }
        cx.notify();
    }

    /// The outline of the picture last clicked, on its page.
    fn object_outline(&self, page_index: usize) -> Option<impl IntoElement> {
        let object = self.selected_object?;
        if object.page.saturating_sub(1) != page_index {
            return None;
        }
        let zoom = self.zoom;
        Some(
            div()
                .absolute()
                .left(px(object.x as f32 * zoom))
                .top(px(object.y as f32 * zoom))
                .w(px(object.width as f32 * zoom))
                .h(px(object.height as f32 * zoom))
                .border_1()
                .border_color(theme::accent())
                .child(
                    div()
                        .absolute()
                        .right(px(-4.0))
                        .bottom(px(-4.0))
                        .size(px(8.0))
                        .bg(theme::accent()),
                ),
        )
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
                self.rebuild_after_edit();
                cx.notify();
            }
            // Nothing on the stack: not an error, just nothing to do.
            Ok(None) => {}
            Err(error) => eprintln!("history failed: {error}"),
        }
    }

    /// Saves over the document that was opened, as Word does; a new one
    /// asks where to go first.
    fn save(&mut self, cx: &mut Context<Self>) {
        if self.untitled {
            return self.save_as(cx);
        }
        let target = PathBuf::from(self.path.to_string());
        self.write_to(target, cx);
    }

    /// Asks where to save, then saves there.
    fn save_as(&mut self, cx: &mut Context<Self>) {
        let source = PathBuf::from(self.path.to_string());
        let directory = self.save_directory();
        // The name it has, extension and all: a .docm stays macro-enabled.
        let suggested = source
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
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

    /// Where a save prompt starts: the document's folder, or for a new
    /// one, Documents.
    fn save_directory(&self) -> PathBuf {
        let source = PathBuf::from(self.path.to_string());
        match source.parent() {
            Some(parent) if !self.untitled && !parent.as_os_str().is_empty() => parent.into(),
            _ => std::env::var_os("HOME")
                .map(|home| PathBuf::from(home).join("Documents"))
                .filter(|documents| documents.is_dir())
                .unwrap_or_else(|| PathBuf::from(".")),
        }
    }

    /// ファイル → PDF として書き出す: asks where, then writes the pages as
    /// the editor paints them, their text searchable.
    fn export_pdf(&mut self, cx: &mut Context<Self>) {
        let source = PathBuf::from(self.path.to_string());
        let suggested = format!(
            "{}.pdf",
            source
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_else(|| "document".to_string())
        );
        let prompt = cx.prompt_for_new_path(&self.save_directory(), Some(&suggested));
        cx.spawn(async move |view, cx| {
            let Ok(Ok(Some(path))) = prompt.await else {
                return;
            };
            view.update(cx, |view, cx| view.write_pdf_to(&path, cx))
                .ok();
        })
        .detach();
    }

    fn write_pdf_to(&mut self, target: &std::path::Path, cx: &mut Context<Self>) {
        let written = self
            .document
            .export_pdf()
            .and_then(|pdf| std::fs::write(target, pdf).map_err(|error| error.to_string()));
        match written {
            Ok(()) => {
                self.saved_to = Some(SharedString::from(crate::shell::file_name(target)));
                cx.notify();
            }
            Err(error) => eprintln!("PDF export failed: {error}"),
        }
    }

    fn write_to(&mut self, target: PathBuf, cx: &mut Context<Self>) {
        match self.document.export_docx() {
            Ok(bytes) => match crate::shell::write_in_place(&target, &bytes) {
                Ok(()) => {
                    self.dirty = false;
                    // Saved as, the document is that file from now on.
                    if self.untitled {
                        self.untitled = false;
                        self.path = SharedString::from(target.to_string_lossy().into_owned());
                    }
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

    /// Moves the caret one character along, across paragraph ends.
    fn step_caret(&mut self, direction: i32, cx: &mut Context<Self>) {
        let next = self.character_step(self.insert_at, direction > 0);
        if next != self.insert_at {
            self.jump_to(next, cx);
        }
    }

    /// Moves the caret a line up or down, by asking what is at the caret's x on
    /// the neighbouring line. Layout owns where lines are; this does not guess.
    fn step_line(&mut self, direction: f64, cx: &mut Context<Self>) {
        let Some(caret) = self.caret else { return };
        let probe_y = caret.document_y + caret.height * (0.5 + direction);
        if let Some(position) = self.document.position_at(caret.x, probe_y) {
            self.insert_at = position;
            self.caret = self.place_caret_near(position, probe_y);
            cx.notify();
        }
    }

    /// Places the caret at `position`, drawn on whichever of its lines is
    /// nearer `document_y`: a wrapped line's first position is also the last
    /// of the line above, and the point it came from says which is meant.
    fn place_caret_near(&mut self, position: (usize, usize), document_y: f64) -> Option<CaretRect> {
        let mut nearest: Option<(CaretAffinity, f64)> = None;
        for affinity in [CaretAffinity::Upstream, CaretAffinity::Downstream] {
            if let Some(rect) = self
                .document
                .place_caret_with_affinity(position.0, position.1, affinity)
            {
                let distance = (rect.document_y + rect.height / 2.0 - document_y).abs();
                if nearest.is_none_or(|(_, best)| distance < best) {
                    nearest = Some((affinity, distance));
                }
            }
        }
        let affinity = nearest.map_or(CaretAffinity::Upstream, |(affinity, _)| affinity);
        self.document
            .place_caret_with_affinity(position.0, position.1, affinity)
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
                self.dirty = true;
                self.last_repaint = Some(elapsed);
                self.insert_at = caret_at;
                self.caret = self.document.place_caret(caret_at.0, caret_at.1);
                self.refresh_caret_page_image();
                cx.notify();
            }
            Err(error) => eprintln!("edit failed: {error}"),
        }
    }

    /// [`Self::rebuild_all_images`] after a change to the document's content,
    /// which leaves it unsaved.
    fn rebuild_after_edit(&mut self) {
        self.dirty = true;
        self.rebuild_all_images();
    }

    /// Re-reads every page. Needed after a re-flow, where a later page's
    /// content may have moved even though the edit was on an earlier one.
    fn rebuild_all_images(&mut self) {
        self.edits += 1;
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
        self.rebuild_after_edit();
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
            self.selected_object = Some(object);
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

        self.selected_object = None;
        if let Some(position) = self.document.position_at(local_x, document_y) {
            self.insert_at = position;
            self.current_page = page_index;
            self.drag_anchor = Some(position);
            // A click starts a new selection; the keyboard anchor from any
            // earlier shift-arrow run is no longer where the user means.
            self.selection_anchor = None;
            self.caret = self.place_caret_near(position, document_y);
            self.refresh_caret_page_image();
            cx.notify();
        }
    }

    /// One page in the page list: a thumbnail of the page itself, the way
    /// Word's navigation pane shows pages, with its number beside it.
    fn sidebar_row(&self, index: usize, cx: &mut Context<Self>) -> impl IntoElement {
        const THUMBNAIL_WIDTH: f32 = 112.0;
        let selected = index == self.current_page;
        let (page_width, page_height) = self.document.layout_page_size();
        let height = THUMBNAIL_WIDTH * (page_height / page_width.max(1.0)) as f32;
        div()
            .id(("page", index))
            .flex()
            .gap_2()
            .px_1()
            .py_1()
            .rounded_md()
            .hover(|row| row.bg(theme::hovered()))
            .cursor_pointer()
            .child(
                div()
                    .w(px(16.0))
                    .text_xs()
                    .text_color(if selected {
                        theme::accent()
                    } else {
                        theme::text_faint()
                    })
                    .child(format!("{}", index + 1)),
            )
            .child(
                div()
                    .w(px(THUMBNAIL_WIDTH))
                    .h(px(height))
                    .flex_none()
                    .overflow_hidden()
                    .rounded_sm()
                    .bg(theme::paper())
                    .border_2()
                    .border_color(if selected {
                        theme::accent()
                    } else {
                        theme::border()
                    })
                    .children(
                        self.images
                            .get(index)
                            .cloned()
                            .map(|image| img(image).size_full()),
                    ),
            )
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
pub(crate) fn utf16_to_byte(text: &str, utf16_offset: usize) -> usize {
    let mut utf16 = 0;
    for (byte, ch) in text.char_indices() {
        if utf16 >= utf16_offset {
            return byte;
        }
        utf16 += ch.len_utf16();
    }
    text.len()
}

pub(crate) fn byte_to_utf16(text: &str, byte_offset: usize) -> usize {
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
                    self.rebuild_after_edit();
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
                self.dirty = true;
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
        if let Some(field) = self.text_bar() {
            let caret = byte_to_utf16(&text, field.caret());
            return Some(UTF16Selection {
                range: caret..caret,
                reversed: false,
            });
        }
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
        if let Some(field) = self.text_bar() {
            let marked = field.marked.clone()?;
            return Some(byte_to_utf16(&text, marked.start)..byte_to_utf16(&text, marked.end));
        }
        let marked = match self.find_bar.as_ref() {
            Some(bar) => bar.marked.clone()?,
            None => self.marked.clone()?,
        };
        Some(byte_to_utf16(&text, marked.start)..byte_to_utf16(&text, marked.end))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        if let Some(field) = self.text_bar_mut() {
            field.marked = None;
            return;
        }
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
        if let Some(field) = self.text_bar_mut() {
            field.replace(range, text, false);
            return cx.notify();
        }
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
        if let Some(field) = self.text_bar_mut() {
            field.replace(range, new_text, true);
            return cx.notify();
        }
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

impl editor::Editor for DocumentView {
    fn status_items(&self) -> Vec<SharedString> {
        let mut items = vec![SharedString::from(i18n::format(
            "doc.position",
            &[
                ("current", &(self.current_page + 1)),
                ("count", &self.images.len()),
            ],
        ))];
        let comments = self.document.comment_count();
        if comments > 0 {
            items.push(i18n::format("doc.comments", &[("n", &comments)]).into());
        }
        if self.document.is_tracking_changes() {
            items.push(i18n::text("doc.tracking").into());
        }
        items
    }

    fn zoom(&self) -> f32 {
        self.zoom
    }

    fn set_zoom(&mut self, zoom: f32, cx: &mut Context<Self>) {
        self.zoom = editor::clamp_zoom(zoom);
        cx.notify();
    }

    fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// The page's width fitted to the space beside the page list.
    fn zoom_to_fit(&mut self, cx: &mut Context<Self>) {
        const PAGE_LIST_WIDTH: f32 = 168.0;
        const PAGE_GUTTER: f32 = 64.0;
        let available = cx
            .try_global::<workspace::EditorArea>()
            .map(|area| area.0)
            .unwrap_or(1000.0)
            - PAGE_LIST_WIDTH
            - PAGE_GUTTER
            - if self.show_inspector { 260.0 } else { 0.0 };
        let (page_width, _) = self.document.layout_page_size();
        self.set_zoom(available / page_width.max(1.0) as f32, cx);
    }
}

impl Render for DocumentView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The one place with a window to hand. Textures replaced since the last
        // frame are released here rather than at the edit that replaced them.
        for image in self.retired_images.drain(..) {
            window.drop_image(image).ok();
        }
        // Checked per frame because a window can move to a screen of another
        // density at any time. Only a change costs anything: a full repaint at
        // the new density, whose old textures are retired on the next frame.
        let scale_factor = window.scale_factor();
        if scale_factor != self.raster_scale {
            self.raster_scale = scale_factor;
            match self.document.set_raster_scale(f64::from(scale_factor)) {
                Ok(true) => self.rebuild_all_images(),
                Ok(false) => {}
                Err(err) => eprintln!("raster scale {scale_factor}: {err}"),
            }
        }
        let dark_pages = theme::dark_pages();
        if dark_pages != self.dark_pages {
            self.dark_pages = dark_pages;
            match self.document.set_dark_paper(dark_pages) {
                Ok(true) => self.rebuild_all_images(),
                Ok(false) => {}
                Err(err) => eprintln!("dark pages {dark_pages}: {err}"),
            }
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
        let comments_pane = self.show_comments.then(|| self.comments_pane(cx));
        let styles_pane = self.styles_pane(cx);
        let ruler = self.horizontal_ruler(cx);
        let dialog = self.dialog_element(cx);
        let vertical_rulers: Vec<Option<AnyElement>> = (0..self.images.len())
            .map(|index| self.vertical_ruler(index))
            .collect();
        let mut vertical_rulers = vertical_rulers.into_iter();
        div()
            .id("root")
            .relative()
            .track_focus(&self.focus)
            // What the shortcuts in `key_bindings` are scoped to, so they fire
            // only while a document has the keyboard.
            .key_context("Document")
            .on_action(cx.listener(|view, _: &Save, _window, cx| view.save(cx)))
            .on_action(cx.listener(|view, _: &SaveAs, _window, cx| view.save_as(cx)))
            .on_action(cx.listener(|view, _: &Print, _window, _cx| view.print()))
            .on_action(cx.listener(|view, _: &ExportPdf, _window, cx| view.export_pdf(cx)))
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
            .on_action(cx.listener(|view, _: &Bold, _window, cx| {
                view.run_ribbon(Action::Toggle(CharacterFormat::Bold), cx)
            }))
            .on_action(cx.listener(|view, _: &Italic, _window, cx| {
                view.run_ribbon(Action::Toggle(CharacterFormat::Italic), cx)
            }))
            .on_action(cx.listener(|view, _: &Underline, _window, cx| {
                view.run_ribbon(Action::Toggle(CharacterFormat::Underline), cx)
            }))
            .on_action(cx.listener(|view, _: &AlignLeft, _window, cx| {
                view.run_ribbon(Action::Align("left"), cx)
            }))
            .on_action(cx.listener(|view, _: &AlignCenter, _window, cx| {
                view.run_ribbon(Action::Align("center"), cx)
            }))
            .on_action(cx.listener(|view, _: &AlignRight, _window, cx| {
                view.run_ribbon(Action::Align("right"), cx)
            }))
            .on_action(cx.listener(|view, _: &IndentMore, _window, cx| {
                view.run_ribbon(Action::Indent(true), cx)
            }))
            .on_action(cx.listener(|view, _: &IndentLess, _window, cx| {
                view.run_ribbon(Action::Indent(false), cx)
            }))
            .on_action(cx.listener(|view, _: &InsertTableRow, _window, cx| {
                view.run_ribbon(Action::Table(TableCommand::InsertRowBelow), cx)
            }))
            .on_action(cx.listener(|view, _: &DeleteTableRow, _window, cx| {
                view.run_ribbon(Action::Table(TableCommand::DeleteRow), cx)
            }))
            .on_action(cx.listener(|view, _: &AddComment, _window, cx| view.add_comment(cx)))
            .on_action(
                cx.listener(|view, _: &ToggleChangeTracking, _window, cx| view.toggle_tracking(cx)),
            )
            .on_action(cx.listener(|view, _: &ToggleInspector, _window, cx| {
                view.show_inspector = !view.show_inspector;
                cx.notify();
            }))
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
                    .w(px(168.0))
                    .h_full()
                    .bg(theme::sidebar())
                    .border_r_1()
                    .border_color(theme::border())
                    .child(
                        div()
                            .px_3()
                            .pt_3()
                            .pb_2()
                            .text_xs()
                            .text_color(theme::text_faint())
                            .child(i18n::format("doc.pages", &[("n", &page_count)])),
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
                    ),
            )
            // ---- canvas: the ribbon, the find bar, then the pages ----------
            .child(
                div()
                    .flex()
                    .flex_col()
                    .size_full()
                    .child(self.ribbon(window, cx))
                    .children(self.comment_bar_element())
                    .children(self.story_bar_element())
                    .children(self.find_bar_element(cx))
                    .children(ruler)
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
                                    .children(vertical_rulers.next().flatten())
                                    .children(self.object_outline(index))
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
            // ---- the comments pane, beside the pages while it is open ----
            .children(comments_pane)
            .children(styles_pane)
            .children(dialog)
            // ---- right panel: what is actually known about the document ----
            // Hidden until asked for (the activity bar's panel button, ⌥⌘I),
            // as the IDE's right panel is.
            .when(self.show_inspector, |root| {
                root.child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_none()
                        .w(px(260.0))
                        .h_full()
                        .gap_5()
                        .px_4()
                        .pt_4()
                        .bg(theme::panel())
                        .border_l_1()
                        .border_color(theme::border())
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(theme::text())
                                        .child(t("doc.info.title")),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme::text_faint())
                                        .child(t("doc.info.subtitle")),
                                ),
                        )
                        .child(Self::info_row(t("doc.info.pages"), page_count.to_string()))
                        .child(Self::info_row(
                            t("doc.info.page_size"),
                            format!("{} x {} px", page_width as i32, page_height as i32),
                        ))
                        .child(Self::info_row(
                            t("doc.info.file_size"),
                            format!("{:.1} KB", self.file_size as f64 / 1024.0),
                        ))
                        .child(Self::info_row(
                            t("doc.info.renderer"),
                            "jade + Skia".to_string(),
                        ))
                        .child(Self::info_row(
                            t("doc.info.last_repaint"),
                            match self.last_repaint {
                                Some(elapsed) => {
                                    format!("{:.2} ms", elapsed.as_secs_f64() * 1000.0)
                                }
                                None => "—".to_string(),
                            },
                        ))
                        .child(Self::info_row(
                            t("doc.info.typed"),
                            i18n::format("doc.info.typed_value", &[("n", &self.typed)]),
                        ))
                        .child(Self::info_row(
                            t("doc.info.insertion"),
                            match self.document.selection() {
                                Some((anchor, focus)) => {
                                    format!("¶{}:{} – ¶{}:{}", anchor.0, anchor.1, focus.0, focus.1)
                                }
                                None => {
                                    format!("¶{} · byte {}", self.insert_at.0, self.insert_at.1)
                                }
                            },
                        ))
                        .child(Self::info_row(
                            t("doc.info.history"),
                            i18n::format(
                                "doc.info.history_value",
                                &[
                                    ("undo", &yes_no(self.document.can_undo())),
                                    ("redo", &yes_no(self.document.can_redo())),
                                ],
                            ),
                        ))
                        .child(Self::info_row(
                            t("doc.info.review"),
                            i18n::format(
                                "doc.info.review_value",
                                &[
                                    ("n", &self.document.comment_count()),
                                    (
                                        "tracking",
                                        &t(if self.document.is_tracking_changes() {
                                            "doc.info.on"
                                        } else {
                                            "doc.info.off"
                                        }),
                                    ),
                                ],
                            ),
                        ))
                        .child(Self::info_row(
                            t("doc.info.table"),
                            match self.document.caret_table_cell() {
                                Some(cell) => i18n::format(
                                    "doc.info.table_value",
                                    &[
                                        ("row", &(cell.row + 1)),
                                        ("rows", &cell.rows),
                                        ("column", &(cell.column + 1)),
                                        ("columns", &cell.columns),
                                    ],
                                ),
                                None => self
                                    .table_status
                                    .as_ref()
                                    .map(|status| status.to_string())
                                    .unwrap_or_else(|| "—".to_string()),
                            },
                        ))
                        .child(Self::info_row(
                            t("doc.info.find"),
                            self.find_status
                                .as_ref()
                                .map(|status| status.to_string())
                                .unwrap_or_else(|| "—".to_string()),
                        ))
                        .child(Self::info_row(
                            t("doc.info.saved"),
                            self.saved_to
                                .as_ref()
                                .map(|name| name.to_string())
                                .unwrap_or_else(|| "—".to_string()),
                        ))
                        .child(div().text_xs().text_color(theme::text_faint()).child(
                            // The commands are in the menu bar now, with their
                            // shortcuts on them. What is left here is what no menu
                            // item can show: the mouse and the find bar's own keys.
                            t("doc.info.help"),
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
            })
    }
}

/// Opens `path` as a document (DOCX) view, for the workspace to show in a tab.
pub(crate) fn new_document_view(
    path: &std::path::Path,
    cx: &mut App,
) -> Result<gpui::Entity<DocumentView>, String> {
    let view = open_document_view(path, cx)?;
    Ok(cx.new(|_| view))
}

/// A new document — blank, or made from a template — in a view of its own,
/// `name` its title until it is saved somewhere.
pub(crate) fn untitled_document_view(
    bytes: &[u8],
    name: &str,
    cx: &mut App,
) -> Result<gpui::Entity<DocumentView>, String> {
    let view = untitled_view(bytes, name, cx)?;
    Ok(cx.new(|_| view))
}

fn untitled_view(bytes: &[u8], name: &str, cx: &mut App) -> Result<DocumentView, String> {
    let document = open_document_bytes(bytes)?;
    let mut view = document_view(document, name.to_string(), bytes.len() as u64, cx);
    view.untitled = true;
    Ok(view)
}

/// Reads `path` and paints its pages, ready to be shown.
fn open_document_view(path: &std::path::Path, cx: &mut App) -> Result<DocumentView, String> {
    let path_text = path.to_string_lossy().into_owned();
    let document = open_document(&path_text)?;
    let file_size = std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
    Ok(document_view(document, path_text, file_size, cx))
}

/// The view over a document just opened.
fn document_view(
    mut document: NativeDocument,
    path_text: String,
    file_size: u64,
    cx: &mut App,
) -> DocumentView {
    // Placed before the window opens so the caret is visible from the first
    // frame rather than only after the first click.
    let caret = document.place_caret(INSERT_PARAGRAPH, 0);
    let images: Vec<_> = (0..document.page_count())
        .filter_map(|index| page_image(&document, index))
        .collect();
    let mut ribbon = ribbon::Ribbon::default();
    ribbon.read_styles(&document);

    DocumentView {
        path: SharedString::from(path_text),
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
        selected_object: None,
        selection_anchor: None,
        zoom: 1.0,
        raster_scale: 1.0,
        dark_pages: false,
        table_status: None,
        find_query: None,
        find_status: None,
        find_bar: None,
        story_bar: None,
        comment_bar: None,
        show_comments: false,
        tools: word_tools::WordTools::default(),
        ribbon_tab: RibbonTab::Home,
        ribbon,
        edits: 0,
        format: None,
        saved_to: None,
        untitled: false,
        marked: None,
        page_bounds: Rc::new(RefCell::new(Vec::new())),
        last_repaint: None,
        typed: 0,
        current_page: 0,
        show_inspector: false,
        dirty: false,
        scroll: ScrollHandle::new(),
        focus: cx.focus_handle(),
    }
}

fn main() {
    let paths: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    // Documents are laid out like the Word of the machine the app runs on —
    // Word for Mac here. The layout reads it per thread, and every document
    // is opened and laid out on this one, gpui's main thread.
    {
        use jade_office_model::word::platform;
        platform::set_word_layout_platform(platform::native_word_layout_platform());
    }

    let application = Application::new().with_assets(Assets);
    // Files handed over by the platform — Finder's "Open With", a drop on the
    // Dock icon — arrive here, outside any window, and are opened on the main
    // thread like any other.
    let (opened_urls, mut url_queue) = futures::channel::mpsc::unbounded::<Vec<String>>();
    application.on_open_urls(move |urls| {
        opened_urls.unbounded_send(urls).ok();
    });

    // Clicking the Dock icon with every window closed brings the start window
    // back rather than doing nothing.
    application.on_reopen(|cx| {
        if cx.windows().is_empty() {
            shell::workspace(cx).ok();
        }
    });

    i18n::init();
    application.run(move |cx: &mut App| {
        // A way to ask the text system what it makes of the shell's own labels
        // without a window, which is the only part of this app that can be
        // checked without looking at a screen.
        if std::env::var("JADE_FONT_PROBE").is_ok() {
            probe_fonts(cx);
            cx.quit();
            return;
        }
        // The saved appearance, or `JADE_THEME=light|dark|system` for this
        // run only; the system's own is read now and followed from each
        // window (see `shell::follow_appearance`).
        theme::set_system(cx.window_appearance());
        theme::set_mode(
            std::env::var("JADE_THEME")
                .ok()
                .and_then(|theme| theme::Mode::parse(&theme))
                .unwrap_or_else(theme::load),
        );
        theme::set_dark_pages_wanted(theme::load_dark_pages());
        cx.bind_keys(key_bindings());
        cx.set_menus(menus());
        // The actions no single window owns.
        cx.on_action(|_: &Quit, cx: &mut App| cx.quit());
        cx.on_action(|_: &Open, cx: &mut App| shell::prompt_and_open(cx));
        cx.on_action(|_: &NewDocument, cx: &mut App| shell::open_blank(cx));
        cx.on_action(|_: &NewFromTemplate, cx: &mut App| shell::prompt_for_template(cx));
        cx.on_action(|_: &ToggleTheme, cx: &mut App| {
            let mode = if theme::is_dark() {
                theme::Mode::Light
            } else {
                theme::Mode::Dark
            };
            apply_theme(mode, cx);
        });
        cx.on_action(|_: &ThemeSystem, cx: &mut App| apply_theme(theme::Mode::System, cx));
        cx.on_action(|_: &ThemeLight, cx: &mut App| apply_theme(theme::Mode::Light, cx));
        cx.on_action(|_: &ThemeDark, cx: &mut App| apply_theme(theme::Mode::Dark, cx));
        cx.on_action(|_: &ToggleDarkPages, cx: &mut App| {
            let wanted = !theme::dark_pages_wanted();
            theme::set_dark_pages_wanted(wanted);
            theme::save_dark_pages(wanted);
            cx.set_menus(menus());
            cx.refresh_windows();
        });
        cx.on_action(|_: &CloseWindow, cx: &mut App| {
            if let Some(window) = cx.active_window() {
                window
                    .update(cx, |_, window, _| window.remove_window())
                    .ok();
            }
        });

        cx.spawn(async move |cx| {
            use futures::StreamExt;
            while let Some(urls) = url_queue.next().await {
                let paths: Vec<PathBuf> = urls
                    .iter()
                    .filter_map(|url| url.strip_prefix("file://"))
                    .map(|path| PathBuf::from(percent_decode(path)))
                    .collect();
                if cx.update(|cx| shell::open_paths(&paths, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();

        if paths.is_empty() {
            shell::workspace(cx).ok();
        } else {
            shell::open_paths(&paths, cx);
        }
        cx.activate(true);
    });
}

/// `%20`-style escapes in a `file://` URL's path, decoded as UTF-8.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&text[index + 1..index + 3], 16) {
                decoded.push(byte);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

#[cfg(test)]
mod tests {
    use super::{key_bindings, menus, percent_decode, Menu, MenuItem};
    use std::collections::{HashMap, HashSet};

    /// Typing into a document edits it and marks its tab unsaved; Undo
    /// takes the typing back, a character at a time — each insert is its
    /// own step in the document's history.
    #[gpui::test]
    fn typing_into_a_document_marks_it_unsaved(cx: &mut gpui::TestAppContext) {
        use super::{open_document_view, Editor, Undo};
        use gpui::Focusable;
        let path = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../jade-core/contract.docx"
        ));
        let (view, cx) = cx.add_window_view(|_window, cx| open_document_view(&path, cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        let before = view.read_with(cx, |view, _| {
            assert!(!view.is_dirty());
            view.current_paragraph_text()
        });
        cx.simulate_input("jade");
        view.read_with(cx, |view, _| {
            assert!(view.is_dirty());
            assert!(view.current_paragraph_text().contains("jade"));
        });
        for _ in 0.."jade".len() {
            cx.dispatch_action(Undo);
        }
        view.read_with(cx, |view, _| {
            assert_eq!(view.current_paragraph_text(), before);
        });
    }

    /// ⌘S saves over the document that was opened, as Word does, leaving
    /// nothing else beside it.
    #[gpui::test]
    fn save_writes_the_document_that_was_opened(cx: &mut gpui::TestAppContext) {
        use super::{open_document_view, Editor, Save};
        use gpui::Focusable;
        let source = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../jade-core/contract.docx"
        ));
        let path = crate::shell::scratch_copy(&source, "contract.docm");
        let (view, cx) = cx.add_window_view(|_window, cx| open_document_view(&path, cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        cx.simulate_input("jadesaved");
        cx.dispatch_action(Save);
        view.read_with(cx, |view, _| assert!(!view.is_dirty()));
        assert_eq!(crate::shell::siblings(&path), ["contract.docm"]);
        let saved = jade_opc::OpcPackage::open(&std::fs::read(&path).unwrap()).unwrap();
        let xml = saved.part_str("word/document.xml").unwrap().unwrap();
        assert!(xml.contains("jadesaved"));
    }

    /// The contract fixture, open in a window with the keyboard on it.
    fn open_contract(
        cx: &mut gpui::TestAppContext,
    ) -> (
        gpui::Entity<super::DocumentView>,
        &mut gpui::VisualTestContext,
    ) {
        use super::open_document_view;
        use gpui::Focusable;
        let path = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../jade-core/contract.docx"
        ));
        let (view, cx) = cx.add_window_view(|_window, cx| open_document_view(&path, cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        (view, cx)
    }

    /// Selects the first few characters of the caret's paragraph, as a drag
    /// would.
    fn select_start(view: &mut super::DocumentView) {
        let paragraph = view.insert_at.0;
        let text = view.current_paragraph_text();
        let end = text.char_indices().nth(3).map(|(index, _)| index).unwrap();
        view.document
            .select((paragraph, 0), (paragraph, end), 1)
            .unwrap();
        view.insert_at = (paragraph, end);
    }

    /// ⌘B turns bold on for the selection and the ribbon shows it pressed;
    /// the same shortcut turns it off, and Undo steps back through both.
    #[gpui::test]
    fn bold_toggles_from_the_shortcut(cx: &mut gpui::TestAppContext) {
        use super::{Bold, Editor, Undo};
        let (view, cx) = open_contract(cx);
        let was_bold = view.update(cx, |view, _| {
            select_start(view);
            view.format_state().bold
        });
        cx.dispatch_action(Bold);
        view.update(cx, |view, _| {
            assert_eq!(view.format_state().bold, !was_bold);
            assert!(view.is_dirty());
            assert!(
                view.document.selection().is_some(),
                "the selection survives"
            );
        });
        cx.dispatch_action(Bold);
        view.update(cx, |view, _| assert_eq!(view.format_state().bold, was_bold));
        cx.dispatch_action(Undo);
        view.update(cx, |view, _| {
            assert_eq!(view.format_state().bold, !was_bold)
        });
    }

    /// A heading from the gallery shows as the active entry; 標準 takes it
    /// off again.
    #[gpui::test]
    fn the_gallery_applies_a_heading(cx: &mut gpui::TestAppContext) {
        use super::ribbon::Action;
        let (view, cx) = open_contract(cx);
        view.update(cx, |view, cx| {
            view.run_ribbon(Action::Style(Some("Heading1".to_string())), cx);
            let state = view.format_state();
            assert!(
                view.ribbon.is_active(1, state.style_id.as_deref()),
                "{state:?}"
            );
            assert!(state.font_size.is_some());
            view.run_ribbon(Action::Style(None), cx);
            let state = view.format_state();
            assert!(
                view.ribbon.is_active(0, state.style_id.as_deref()),
                "{state:?}"
            );
        });
    }

    /// A table picked on the grid goes in with the caret in its first cell;
    /// a row goes in below, two cells merge and unmerge, and Undo takes the
    /// steps back.
    #[gpui::test]
    fn table_tools_edit_the_caret_table(cx: &mut gpui::TestAppContext) {
        use super::ribbon::Action;
        use super::{TableCommand, Undo};
        let (view, cx) = open_contract(cx);
        view.update(cx, |view, cx| {
            view.run_ribbon(Action::InsertTable(2, 3), cx);
            let cell = view
                .format_state()
                .table
                .expect("the caret is in the table");
            assert_eq!(
                (cell.row, cell.column, cell.rows, cell.columns),
                (0, 0, 2, 3)
            );

            view.run_ribbon(Action::Table(TableCommand::InsertRowBelow), cx);
            assert_eq!(view.format_state().table.unwrap().rows, 3);

            // The caret's cell and the one to its right.
            let next = view.insert_at.0 + 1;
            let from = view.insert_at;
            view.document.select(from, (next, 0), 1).unwrap();
            view.insert_at = (next, 0);
            assert!(view.format_state().table.unwrap().can_merge);
            view.run_ribbon(Action::Table(TableCommand::MergeCells), cx);
            assert!(view.format_state().table.unwrap().can_unmerge);
            assert!(view.table_status.is_none(), "{:?}", view.table_status);
        });
        cx.dispatch_action(Undo);
        view.update(cx, |view, _| {
            assert!(
                !view.format_state().table.unwrap().can_unmerge,
                "the merge is undone"
            );
        });
    }

    /// Puts the caret at the start of the caret's paragraph and types
    /// `text` there; returns the paragraph.
    fn type_at_start(
        view: &gpui::Entity<super::DocumentView>,
        cx: &mut gpui::VisualTestContext,
        text: &str,
    ) -> usize {
        let paragraph = view.update(cx, |view, _| {
            let paragraph = view.insert_at.0;
            view.insert_at = (paragraph, 0);
            view.caret = view.document.place_caret(paragraph, 0);
            paragraph
        });
        cx.simulate_input(text);
        paragraph
    }

    /// ⌥← and ⌥→ go a word along, ⇧ extending; ⌘← and End go to the line's
    /// ends, ⌘↑ and ⌘↓ to the document's — as on a Mac.
    #[gpui::test]
    fn word_line_and_document_keys_move_the_caret(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_contract(cx);
        let paragraph = type_at_start(&view, cx, "one two, three ");
        view.update(cx, |view, _| {
            view.insert_at = (paragraph, 0);
            view.caret = view.document.place_caret(paragraph, 0);
        });
        cx.simulate_keystrokes("alt-right");
        view.read_with(cx, |view, _| assert_eq!(view.insert_at, (paragraph, 3)));
        cx.simulate_keystrokes("alt-right");
        view.read_with(cx, |view, _| assert_eq!(view.insert_at, (paragraph, 7)));
        cx.simulate_keystrokes("alt-shift-left");
        view.read_with(cx, |view, _| {
            assert_eq!(
                view.document.selection(),
                Some(((paragraph, 7), (paragraph, 4)))
            )
        });
        cx.simulate_keystrokes("end");
        let line_end = view.read_with(cx, |view, _| {
            assert!(view.document.selection().is_none());
            view.insert_at
        });
        assert_eq!(line_end.0, paragraph);
        assert!(line_end.1 > "one two, three ".len(), "{line_end:?}");
        cx.simulate_keystrokes("cmd-left");
        view.read_with(cx, |view, _| assert_eq!(view.insert_at, (paragraph, 0)));
        cx.simulate_keystrokes("cmd-down");
        let end = view.read_with(cx, |view, _| view.insert_at);
        assert!(end.0 > paragraph, "{end:?}");
        cx.simulate_keystrokes("cmd-up");
        view.read_with(cx, |view, _| {
            assert!(view.insert_at.0 < paragraph);
            assert_eq!(view.insert_at.1, 0);
        });
    }

    /// ⌥⌫ takes the word before the caret in one edit; Backspace at a
    /// paragraph's start joins it to the one before.
    #[gpui::test]
    fn option_backspace_deletes_a_word_and_backspace_joins(cx: &mut gpui::TestAppContext) {
        use super::Undo;
        let (view, cx) = open_contract(cx);
        let paragraph = type_at_start(&view, cx, "alpha beta ");
        let before = view.read_with(cx, |view, _| view.current_paragraph_text());
        cx.simulate_keystrokes("alt-backspace");
        view.read_with(cx, |view, _| {
            assert_eq!(view.insert_at, (paragraph, "alpha ".len()));
            assert_eq!(
                view.current_paragraph_text(),
                before.replacen("beta ", "", 1)
            );
        });
        cx.dispatch_action(Undo);
        view.read_with(cx, |view, _| {
            assert_eq!(view.current_paragraph_text(), before)
        });

        view.update(cx, |view, _| {
            view.insert_at = (paragraph, 0);
            view.caret = view.document.place_caret(paragraph, 0);
        });
        let above = view.read_with(cx, |view, _| {
            view.document.paragraph_text(paragraph - 1).unwrap()
        });
        cx.simulate_keystrokes("backspace");
        view.read_with(cx, |view, _| {
            assert_eq!(view.insert_at, (paragraph - 1, above.len()));
            assert_eq!(view.current_paragraph_text(), format!("{above}{before}"));
        });
    }

    /// Tab goes from cell to cell with the cell's text selected, ⇧Tab
    /// back, and Tab in the last cell adds a row; outside a table it types
    /// a tab.
    #[gpui::test]
    fn tab_goes_from_cell_to_cell(cx: &mut gpui::TestAppContext) {
        use super::ribbon::Action;
        let (view, cx) = open_contract(cx);
        // A paragraph that is not a list item: at an item's start Tab takes
        // the item a level down instead, as in Word (word_tools).
        view.update(cx, |view, _| {
            let plain = (view.insert_at.0..view.document.paragraph_count())
                .find(|&index| {
                    view.document
                        .word_document()
                        .paragraph(index)
                        .and_then(|paragraph| paragraph.properties.as_ref())
                        .and_then(|props| props.numbering.as_ref())
                        .is_none()
                        && view
                            .document
                            .paragraph_text(index)
                            .is_some_and(|text| text.chars().count() > 3)
                })
                .unwrap();
            view.insert_at = (plain, 0);
        });
        let paragraph = type_at_start(&view, cx, "");
        cx.simulate_keystrokes("tab");
        view.read_with(cx, |view, _| {
            assert!(view.current_paragraph_text().starts_with('\t'));
            assert_eq!(view.insert_at, (paragraph, 1));
        });

        view.update(cx, |view, cx| {
            view.run_ribbon(Action::InsertTable(1, 2), cx)
        });
        let first = view.read_with(cx, |view, _| view.insert_at);
        cx.simulate_input("one");
        cx.simulate_keystrokes("tab");
        let second = view.read_with(cx, |view, _| {
            assert_ne!(view.insert_at.0, first.0);
            view.insert_at.0
        });
        cx.simulate_input("two");
        cx.simulate_keystrokes("shift-tab");
        view.read_with(cx, |view, _| {
            assert_eq!(
                view.document.selection(),
                Some(((first.0, 0), (first.0, 3)))
            )
        });
        view.update(cx, |view, _| {
            view.insert_at = (second, 3);
            view.caret = view.document.place_caret(second, 3);
        });
        cx.simulate_keystrokes("tab");
        view.update(cx, |view, _| {
            let cell = view.format_state().table.expect("in the table");
            assert_eq!((cell.rows, cell.row, cell.column), (2, 1, 0));
        });
    }

    /// ⌥⌘W and ⌥⌘C turn the bar's options on, which narrow what is found;
    /// ⌘⏎ replaces every match left, and one Undo takes them all back.
    #[gpui::test]
    fn find_options_narrow_and_replace_all_undoes_once(cx: &mut gpui::TestAppContext) {
        use super::{FindAndReplace, Undo, WordFind};
        let (view, cx) = open_contract(cx);
        let paragraph = type_at_start(&view, cx, "Jade jade JADE jaded. ");
        let before = view.read_with(cx, |view, _| view.current_paragraph_text());
        cx.dispatch_action(FindAndReplace);
        cx.simulate_input("jade");
        let count = |view: &super::DocumentView| {
            view.document
                .find_all("jade", view.find_options())
                .iter()
                .filter(|found| found.paragraph == paragraph)
                .count()
        };
        view.read_with(cx, |view, _| assert_eq!(count(view), 4));
        cx.simulate_keystrokes("alt-cmd-w");
        view.read_with(cx, |view, _| {
            assert!(view.find_options().whole_word);
            assert_eq!(count(view), 3);
        });
        cx.simulate_keystrokes("alt-cmd-c");
        view.read_with(cx, |view, _| assert_eq!(count(view), 1));
        cx.simulate_keystrokes("alt-cmd-c");
        cx.simulate_keystrokes("tab");
        cx.simulate_input("Word");
        cx.simulate_keystrokes("cmd-enter");
        view.read_with(cx, |view, _| {
            let text = view.document.paragraph_text(paragraph).unwrap();
            assert!(text.starts_with("Word Word Word jaded. "), "{text}");
            assert_eq!(
                text.replacen("Word Word Word", "Jade jade JADE", 1),
                before,
                "only the whole words changed"
            );
        });
        cx.simulate_keystrokes("escape");
        cx.dispatch_action(Undo);
        view.read_with(cx, |view, _| {
            assert_eq!(view.document.paragraph_text(paragraph).unwrap(), before)
        });
    }

    /// ⌘N's blank document opens untitled, takes typing, and is written
    /// out as a PDF of its pages.
    #[gpui::test]
    fn a_new_document_opens_untitled_and_exports_a_pdf(cx: &mut gpui::TestAppContext) {
        use gpui::Focusable;
        use jade_office_model::word::new::{blank_document, Blank};
        let (view, cx) = cx.add_window_view(|_window, cx| {
            super::untitled_view(&blank_document(Blank::Japanese), "文書 1.docx", cx).unwrap()
        });
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        view.update(cx, |view, _| {
            assert!(view.untitled);
            view.insert_at = (0, 0);
            view.caret = view.document.place_caret(0, 0);
        });
        cx.simulate_input("新しい文書");
        let pdf = std::env::temp_dir().join(format!("jade-new-{}.pdf", std::process::id()));
        view.update(cx, |view, cx| {
            assert_eq!(view.current_paragraph_text(), "新しい文書");
            assert!(view.dirty);
            view.write_pdf_to(&pdf, cx);
        });
        let bytes = std::fs::read(&pdf).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
        std::fs::remove_file(&pdf).ok();
    }

    /// The page tools: 余白 from the gallery, the page turned, the header
    /// typed into its bar and put in with Enter, a page number in the
    /// footer; Undo takes each back.
    #[gpui::test]
    fn the_page_tools_set_up_the_page_and_its_header(cx: &mut gpui::TestAppContext) {
        use super::ribbon::Action;
        use super::{Undo, WordPage};
        use jade_office_model::word::page::{MarginPreset, NumberAlignment, PageCommand, Story};
        let (view, cx) = open_contract(cx);
        let before = view.read_with(cx, |view, _| view.document.page_setup());
        view.update(cx, |view, cx| {
            view.run_ribbon(Action::Page(PageCommand::Margins(MarginPreset::Narrow)), cx);
            assert_eq!(view.document.page_setup().margins.top, 36.0);
            view.run_ribbon(Action::Page(PageCommand::Landscape(!before.landscape)), cx);
            assert_eq!(view.document.page_setup().landscape, !before.landscape);
            view.run_ribbon(Action::EditStory(Story::Header), cx);
            assert!(view.story_bar.is_some());
        });
        view.update(cx, |view, _| {
            view.story_bar.as_mut().unwrap().text =
                jade_office_model::text::TextEdit::new(String::new());
        });
        cx.simulate_input("jade");
        cx.simulate_keystrokes("tab");
        cx.simulate_input("header");
        cx.simulate_keystrokes("enter");
        view.update(cx, |view, cx| {
            assert!(view.story_bar.is_none());
            assert_eq!(view.document.story_text(Story::Header), "jade\theader");
            view.run_ribbon(
                Action::Page(PageCommand::PageNumber(
                    Story::Footer,
                    NumberAlignment::Center,
                )),
                cx,
            );
        });
        for _ in 0..4 {
            cx.dispatch_action(Undo);
        }
        view.read_with(cx, |view, _| assert_eq!(view.document.page_setup(), before));
    }

    /// ⌘M opens the comment bar; Enter puts the comment on the selection,
    /// and the pane's reply and resolve work on it. With changes tracked,
    /// typing is a revision that 元に戻す takes out again.
    #[gpui::test]
    fn comments_are_written_answered_and_changes_decided(cx: &mut gpui::TestAppContext) {
        use super::word_review::CommentEntry;
        use super::AddComment;
        use jade_office_model::word::review::WordReview;
        let (view, cx) = open_contract(cx);
        let paragraph = type_at_start(&view, cx, "Clause one. ");
        view.update(cx, |view, _| {
            view.document
                .select((paragraph, 0), (paragraph, "Clause".len()), 1)
                .unwrap();
            view.insert_at = (paragraph, "Clause".len());
        });
        cx.dispatch_action(AddComment);
        cx.simulate_input("Check this.");
        cx.simulate_keystrokes("enter");
        let id = view.read_with(cx, |view, _| {
            assert!(view.comment_bar.is_none());
            assert!(view.show_comments);
            let comment = view
                .document
                .comments()
                .into_iter()
                .find(|comment| comment.text == "Check this.")
                .expect("the comment went in");
            assert_eq!(comment.anchor.map(|(start, _)| start), Some((paragraph, 0)));
            comment.id
        });
        view.update(cx, |view, cx| {
            view.open_comment_bar(CommentEntry::Reply(id.clone()), String::new(), cx)
        });
        cx.simulate_input("Done.");
        cx.simulate_keystrokes("enter");
        view.update(cx, |view, cx| {
            view.resolve_comment(&id, true, cx);
            let comment = view
                .document
                .comments()
                .into_iter()
                .find(|comment| comment.id == id)
                .unwrap();
            assert_eq!(comment.replies.len(), 1);
            assert!(comment.resolved);

            view.toggle_tracking(cx);
            assert!(view.document.is_tracking_changes());
            view.insert_at = (paragraph, 0);
            view.caret = view.document.place_caret(paragraph, 0);
        });
        let before = view.read_with(cx, |view, _| view.current_paragraph_text());
        cx.simulate_input("new ");
        view.update(cx, |view, cx| {
            assert!(!view.document.revisions().is_empty());
            view.insert_at = (paragraph, 1);
            view.caret = view.document.place_caret(paragraph, 1);
            view.decide_revision(false, cx);
            assert_eq!(view.document.paragraph_text(paragraph).unwrap(), before);
            view.toggle_tracking(cx);
            assert!(!view.document.is_tracking_changes());
        });
    }

    /// Typed with changes tracked, text is drawn as Word marks an insertion
    /// — in the revision colour, underlined — and rejected, as before.
    #[gpui::test]
    fn tracked_typing_is_drawn_as_markup(cx: &mut gpui::TestAppContext) {
        /// Pixels of the pages in the renderer's insertion colour, and the
        /// longest horizontal line of them: an underline, not a glyph. The
        /// line is matched looser, as it is drawn a keystroke's run at a time
        /// and two runs' anti-aliased ends meet in a lighter pixel.
        fn inserted(document: &jade_docx_native_render::NativeDocument) -> (usize, usize) {
            let (mut count, mut longest) = (0, 0);
            for index in 0..document.page_count() {
                let (pixels, width, _) = document.page_pixels(index).unwrap();
                for row in pixels.chunks(width as usize * 4) {
                    let mut run = 0;
                    for pixel in row.chunks(4) {
                        // BGRA, against #1b5e20.
                        let off = [pixel[2], pixel[1], pixel[0]]
                            .iter()
                            .zip([0x1b_u8, 0x5e, 0x20])
                            .map(|(&have, want)| have.abs_diff(want))
                            .max()
                            .unwrap();
                        run = if off <= 64 { run + 1 } else { 0 };
                        count += usize::from(off <= 24);
                        longest = longest.max(run);
                    }
                }
            }
            (count, longest)
        }
        let (view, cx) = open_contract(cx);
        let paragraph = type_at_start(&view, cx, "");
        view.update(cx, |view, cx| {
            // On white paper, where the colour is the renderer's own.
            view.document.set_dark_paper(false).unwrap();
            assert_eq!(inserted(&view.document).0, 0);
            view.toggle_tracking(cx);
            view.insert_at = (paragraph, 0);
            view.caret = view.document.place_caret(paragraph, 0);
        });
        let before = view.read_with(cx, |view, _| view.current_paragraph_text());
        cx.simulate_input("Inserted words ");
        view.update(cx, |view, cx| {
            let (count, longest) = inserted(&view.document);
            assert!(count > 50, "the insertion is coloured: {count} pixels");
            assert!(longest > 80, "and underlined: its longest line {longest}px");
            view.insert_at = (paragraph, 1);
            view.caret = view.document.place_caret(paragraph, 1);
            view.decide_revision(false, cx);
            assert_eq!(view.document.paragraph_text(paragraph).unwrap(), before);
            assert_eq!(inserted(&view.document).0, 0, "rejected, no markup");
        });
    }

    /// A copy keeps its formatting when it is pasted back; a picture goes
    /// in at the caret, and a click on it lets its wrap be set.
    #[gpui::test]
    fn copies_keep_formatting_and_pictures_go_in(cx: &mut gpui::TestAppContext) {
        use super::{Bold, Copy, Paste, Undo};
        let (view, cx) = open_contract(cx);
        let paragraph = type_at_start(&view, cx, "Bold plain ");
        view.update(cx, |view, _| {
            view.document
                .select((paragraph, 0), (paragraph, 4), 1)
                .unwrap();
            view.insert_at = (paragraph, 4);
        });
        cx.dispatch_action(Bold);
        cx.dispatch_action(Copy);
        let end = view.update(cx, |view, _| {
            let end = view.document.paragraph_text(paragraph).unwrap().len();
            view.insert_at = (paragraph, end);
            view.caret = view.document.place_caret(paragraph, end);
            end
        });
        cx.dispatch_action(Paste);
        view.read_with(cx, |view, _| {
            assert_eq!(view.insert_at, (paragraph, end + 4));
            let style = view.document.run_style_at(paragraph, end + 1).unwrap();
            assert_eq!(style.bold, Some(true), "the paste kept the bold");
        });
        cx.dispatch_action(Undo);
        view.read_with(cx, |view, _| {
            assert_eq!(view.document.paragraph_text(paragraph).unwrap().len(), end)
        });

        // A 2×1 PNG.
        let mut png = Vec::new();
        image::RgbaImage::from_raw(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 255])
            .unwrap()
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        view.update(cx, |view, cx| {
            view.insert_at = (paragraph, 0);
            view.insert_picture(png, cx);
            assert!(view.table_status.is_none(), "{:?}", view.table_status);
            let images = view
                .document
                .word_document()
                .paragraph(paragraph)
                .unwrap()
                .runs
                .iter()
                .filter(|run| run.image.is_some())
                .count();
            assert_eq!(images, 1);
        });
    }

    /// A page turned to landscape is laid out and painted as the same
    /// document opened landscape — its page count, its pitch and every
    /// page's pixels — and the view's page images follow.
    #[gpui::test]
    fn a_page_turned_to_landscape_paints_as_one_opened_landscape(cx: &mut gpui::TestAppContext) {
        use super::ribbon::Action;
        use jade_office_model::word::page::PageCommand;
        let (view, cx) = open_contract(cx);
        view.update(cx, |view, cx| {
            view.run_ribbon(Action::Page(PageCommand::Landscape(true)), cx);
            let mut reopened =
                super::open_document_bytes(&view.document.export_docx().unwrap()).unwrap();
            // Painted at the window's scale and on its paper, as the view's
            // document is.
            reopened
                .set_raster_scale(f64::from(view.raster_scale))
                .unwrap();
            reopened.set_dark_paper(view.document.dark_paper()).unwrap();
            let (width, height) = reopened.layout_page_size();
            assert!(width > height, "opened landscape: {width}x{height}");
            assert_eq!(view.document.layout_page_size(), (width, height));
            assert_eq!(view.document.page_pitch(), reopened.page_pitch());
            assert_eq!(view.document.page_count(), reopened.page_count());
            assert_eq!(view.images.len(), reopened.page_count());
            for index in 0..reopened.page_count() {
                let turned = view.document.page_pixels(index).unwrap();
                let opened = reopened.page_pixels(index).unwrap();
                assert_eq!((turned.1, turned.2), (opened.1, opened.2), "page {index}");
                assert!(turned.0 == opened.0, "page {index} is painted as opened");
                let image = view.images[index].size(0);
                assert_eq!(
                    (image.width.0, image.height.0),
                    (opened.1, opened.2),
                    "page {index}'s image"
                );
            }
        });
    }

    #[test]
    fn file_urls_decode_to_paths() {
        assert_eq!(
            percent_decode("/Users/me/My%20Deck%E8%B3%87%E6%96%99.pptx"),
            "/Users/me/My Deck資料.pptx"
        );
        assert_eq!(percent_decode("/a%2"), "/a%2");
    }

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

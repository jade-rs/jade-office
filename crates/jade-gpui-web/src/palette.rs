//! The native app's two palettes (`jade-gpui-app`'s `theme`), picked by the
//! page's `prefers-color-scheme` until the title bar's toggle overrides it.
//! The palette and the separate dark-pages preference survive a reload.
//! Slides and cells keep their content colours.

use crate::view::word_views::settings;
use gpui::{rgb, Rgba, WindowAppearance};
use std::cell::Cell;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    System,
    Light,
    Dark,
}

impl Mode {
    fn parse(text: &str) -> Self {
        match text.trim() {
            "light" => Self::Light,
            "dark" => Self::Dark,
            _ => Self::System,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

thread_local! {
    static SYSTEM_DARK: Cell<bool> = const { Cell::new(true) };
    static MODE: Cell<Mode> = Cell::new(Mode::parse(&settings::load("theme.txt").unwrap_or_default()));
    static DARK_PAGES: Cell<bool> = Cell::new(settings::load("dark-pages.txt").as_deref().map(str::trim) != Some("0"));
}

pub fn mode() -> Mode {
    MODE.with(Cell::get)
}

pub fn set_mode(mode: Mode) {
    MODE.with(|cell| cell.set(mode));
    settings::save("theme.txt", mode.name());
}

pub fn is_dark() -> bool {
    match mode() {
        Mode::System => SYSTEM_DARK.with(Cell::get),
        Mode::Light => false,
        Mode::Dark => true,
    }
}

pub fn dark_pages_wanted() -> bool {
    DARK_PAGES.with(Cell::get)
}

pub fn dark_pages() -> bool {
    is_dark() && dark_pages_wanted()
}

pub fn toggle_dark_pages() {
    let wanted = !dark_pages_wanted();
    DARK_PAGES.with(|cell| cell.set(wanted));
    settings::save("dark-pages.txt", if wanted { "1" } else { "0" });
}

/// Takes up the page's appearance; true when that changed the palette.
pub fn follow(appearance: WindowAppearance) -> bool {
    let before = is_dark();
    SYSTEM_DARK.with(|cell| {
        cell.set(matches!(
            appearance,
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        ))
    });
    is_dark() != before
}

/// The title bar's toggle: the other palette, from now on.
pub fn toggle() {
    set_mode(if is_dark() { Mode::Light } else { Mode::Dark });
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
pub fn border() -> Rgba {
    pick(0x27272a, 0xe2e8f0)
}
/// What is selected.
pub fn selected() -> Rgba {
    pick(0x1f1f23, 0xede9fe)
}
/// What the pointer is over.
pub fn hovered() -> Rgba {
    pick(0x18181b, 0xf1f5f9)
}
pub fn text() -> Rgba {
    pick(0xe4e4e7, 0x1a1a2e)
}
/// A Word page, behind its picture: white, or the dark paper.
pub fn paper() -> Rgba {
    let [r, g, b] = if dark_pages() {
        jade_painter::paper::to_dark_paper_u8([0xff, 0xff, 0xff])
    } else {
        [0xff, 0xff, 0xff]
    };
    rgb(u32::from_be_bytes([0, r, g, b]))
}
/// What the view draws on a Word page — the caret, a composition — in the
/// colour the page's own black ink is shown in.
pub fn ink() -> Rgba {
    let [r, g, b] = if dark_pages() {
        jade_painter::paper::to_dark_paper_u8([0, 0, 0])
    } else {
        [0, 0, 0]
    };
    rgb(u32::from_be_bytes([0, r, g, b]))
}
pub fn dim() -> Rgba {
    pick(0xa1a1aa, 0x64748b)
}
pub fn faint() -> Rgba {
    pick(0x71717a, 0x94a3b8)
}
pub fn accent() -> Rgba {
    pick(0x818cf8, 0x6366f1)
}
/// The second stop of the IDE's gradients.
pub fn accent_alt() -> Rgba {
    pick(0xc084fc, 0xa855f7)
}
/// The "Ready" dot in the status bar.
pub fn ready() -> Rgba {
    pick(0x34d399, 0x10b981)
}

/// The native app's names for [`dim`] and [`faint`], which the code ported
/// from it uses.
pub fn text_dim() -> Rgba {
    dim()
}

pub fn text_faint() -> Rgba {
    faint()
}

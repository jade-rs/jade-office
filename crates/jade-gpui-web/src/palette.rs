//! The native app's two palettes (`jade-gpui-app`'s `theme`), picked by the
//! page's `prefers-color-scheme` until the title bar's toggle overrides it.
//! A Word document's pages go dark with the dark palette, as in Word's dark
//! page mode (see `jade_painter::paper`); slides and cells keep their own
//! colours, which are the content.

use gpui::{rgb, Rgba, WindowAppearance};
use std::cell::Cell;

thread_local! {
    static SYSTEM_DARK: Cell<bool> = const { Cell::new(true) };
    /// The toggle's choice, once made; `None` follows the page.
    static CHOSEN: Cell<Option<bool>> = const { Cell::new(None) };
}

pub fn is_dark() -> bool {
    CHOSEN
        .with(Cell::get)
        .unwrap_or_else(|| SYSTEM_DARK.with(Cell::get))
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
    let dark = !is_dark();
    CHOSEN.with(|cell| cell.set(Some(dark)));
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
    let [r, g, b] = if is_dark() {
        jade_painter::paper::to_dark_paper_u8([0xff, 0xff, 0xff])
    } else {
        [0xff, 0xff, 0xff]
    };
    rgb(u32::from_be_bytes([0, r, g, b]))
}
/// What the view draws on a Word page — the caret, a composition — in the
/// colour the page's own black ink is shown in.
pub fn ink() -> Rgba {
    let [r, g, b] = if is_dark() {
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

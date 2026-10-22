//! jade in the browser, through gpui.
//!
//! The page fetches the fonts and calls [`run`]. From there it is a gpui
//! application like the native one, and looks like it: the same workspace
//! frame, panels, palette, words and icons (`jade-gpui-app`'s tables and
//! icons are compiled in here as they are). gpui-pre's web platform owns a
//! canvas and draws with WebGPU (or WebGL2); the documents' pixels come from
//! `jade-web` — the engine's renderers on the pure-Rust painter — as
//! `RenderImage`s, the way the native views take Skia's.
//!
//! The browser has no paths to open: every file — a sample from the
//! explorer, one picked or one dropped on the page — reaches the app through
//! [`open_file`] as a name and its bytes.

#![cfg(target_arch = "wasm32")]

mod assets;
mod i18n;
mod palette;
mod ribbon;
mod tooltip;
mod view;
mod word_objects;
mod word_page;
mod word_review;
mod word_table;
mod word_tools;
mod workspace;

use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures::StreamExt;
use gpui::{prelude::*, App, Application, Focusable, SharedString, WindowOptions};
use gpui_web::{CanvasFontFallback, WebBackendPreference, WebPlatform};
use wasm_bindgen::prelude::*;

use crate::workspace::Workspace;

const UI_FONT: &str = "Noto Sans JP";

/// What opening a file needs: the faces, as the painters and the docx
/// measurer take them, and the page's pixel ratio and time zone.
pub struct Fonts {
    backend: jade_web::TinySkiaBackend,
    measurer: Rc<jade_web::document::ShapingMeasurer>,
    /// Device px per CSS px, which Word pages are painted at.
    scale: f64,
    /// The browser's, which formulas calculate in.
    time_zone: String,
}

/// The page's side of the app: the hooks it passed to [`run`].
#[derive(Clone)]
pub struct Host {
    save: js_sys::Function,
    load: js_sys::Function,
    pick: js_sys::Function,
}

impl Host {
    /// Hands a saved file to the page, which offers it as a download.
    fn save(&self, name: &str, bytes: &[u8]) {
        let array = js_sys::Uint8Array::from(bytes);
        self.save
            .call2(&JsValue::NULL, &JsValue::from_str(name), &array)
            .ok();
    }

    /// Hands a PDF to the page to print: the browser's print dialog opens
    /// on it.
    fn print(&self, name: &str, bytes: &[u8]) {
        let array = js_sys::Uint8Array::from(bytes);
        self.save
            .call3(
                &JsValue::NULL,
                &JsValue::from_str(name),
                &array,
                &JsValue::TRUE,
            )
            .ok();
    }

    /// Asks the page for a sample; it answers through [`open_file`].
    fn load(&self, name: &str) {
        self.load
            .call1(&JsValue::NULL, &JsValue::from_str(name))
            .ok();
    }

    /// Asks the page to let the user pick a file; it answers through
    /// [`open_file`].
    fn pick(&self) {
        self.pick.call0(&JsValue::NULL).ok();
    }
}

type Incoming = (String, Vec<u8>);

thread_local! {
    /// Files handed over before the app is up wait here.
    static PENDING: RefCell<Vec<Incoming>> = const { RefCell::new(Vec::new()) };
    static INBOX: RefCell<Option<UnboundedSender<Incoming>>> = const { RefCell::new(None) };
}

/// Opens `bytes` as the file `name` names — `.docx`, `.xlsx` or `.pptx` — in
/// a new tab.
#[wasm_bindgen]
pub fn open_file(name: String, bytes: Vec<u8>) {
    let sent = INBOX.with(|inbox| {
        inbox
            .borrow()
            .as_ref()
            .is_some_and(|sender| sender.unbounded_send((name.clone(), bytes.clone())).is_ok())
    });
    if !sent {
        PENDING.with(|pending| pending.borrow_mut().push((name, bytes)));
    }
}

fn inbox() -> UnboundedReceiver<Incoming> {
    let (sender, receiver) = unbounded();
    for incoming in PENDING.with(|pending| std::mem::take(&mut *pending.borrow_mut())) {
        sender.unbounded_send(incoming).ok();
    }
    INBOX.with(|inbox| *inbox.borrow_mut() = Some(sender));
    receiver
}

/// Starts the app in a gpui window on the page.
///
/// `regular` and `bold` are the bundled Noto Sans JP faces, and `thai` the
/// Noto Sans Thai that Thai text falls back to. `scale` is the page's `devicePixelRatio`; `time_zone` and `language` are
/// the browser's; `samples` are the files the explorer lists. `save(name,
/// bytes)` offers a file as a download, `load(name)` fetches a sample and
/// `pick()` lets the user choose a file; the last two answer through
/// [`open_file`].
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    regular: Vec<u8>,
    bold: Vec<u8>,
    thai: Vec<u8>,
    scale: f64,
    time_zone: String,
    language: String,
    samples: Vec<String>,
    save: js_sys::Function,
    load: js_sys::Function,
    pick: js_sys::Function,
) -> Result<(), JsError> {
    console_error_panic_hook::set_once();
    // Before any document is laid out: like Word for Mac on a Mac, or as the
    // page's `?word_platform=` says.
    jade_web::set_word_layout_platform_from_page();
    i18n::init(&language);
    let backend = jade_web::backend_with(regular.clone(), Some(bold.clone()), Some(thai.clone()))
        .map_err(|e| JsError::new(&e))?;
    let measurer = jade_web::document::measurer_with(&regular, Some(&bold), Some(&thai))
        .map_err(|e| JsError::new(&e))?;
    let fonts = Rc::new(Fonts {
        backend,
        measurer,
        scale: if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        },
        time_zone,
    });
    let host = Host { save, load, pick };
    let mut incoming = inbox();

    // Single-threaded, and CJK the bundled face lacks falls back to the
    // browser's own fonts through Canvas2D.
    let platform = WebPlatform::new_with_backend_and_font_fallback(
        false,
        WebBackendPreference::Auto,
        CanvasFontFallback::EmojiAndCjk,
    );
    let handle = Application::with_platform(Rc::new(platform))
        .with_assets(assets::Assets)
        .run_embedded(move |cx: &mut App| {
            cx.text_system().add_fonts(vec![Cow::Owned(regular)]).ok();
            let window = cx.open_window(WindowOptions::default(), |_window, cx| {
                cx.new(|cx| Workspace::new(samples, fonts, host, cx))
            });
            let Ok(window) = window else {
                return;
            };
            window
                .update(cx, |workspace, window, cx| {
                    // Repaint when the page flips `prefers-color-scheme`.
                    cx.observe_window_appearance(window, |_, window, cx| {
                        if palette::follow(window.appearance()) {
                            cx.notify();
                        }
                    })
                    .detach();
                    // Every file the page hands over becomes a tab.
                    cx.spawn_in(window, async move |workspace, cx| {
                        while let Some((name, bytes)) = incoming.next().await {
                            let opened = workspace.update_in(cx, |workspace, window, cx| {
                                workspace.open(SharedString::from(name), Rc::new(bytes), window, cx)
                            });
                            if opened.is_err() {
                                break;
                            }
                        }
                    })
                    .detach();
                    window.focus(&workspace.focus_handle(cx), cx)
                })
                .ok();
        });
    // The page's event loop drives the app from here; it lives as long as
    // the page does.
    std::mem::forget(handle);
    Ok(())
}

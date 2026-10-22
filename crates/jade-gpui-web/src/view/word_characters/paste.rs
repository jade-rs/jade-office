//! 貼り付けのオプション and 形式を選択して貼り付け in the browser: a paste
//! that keeps the copy's formatting, merges it with the destination's
//! (⌥⇧⌘V), or keeps the text only; and the dialog (⌃⌘V) that lists the
//! clipboard's formats — the native app's `word_characters::paste`.
//!
//! The clipboard is read with the browser's asynchronous Clipboard API on
//! the click or the key that asked, HTML and plain text: a copy made here
//! is recognised by its text and pastes with its formatting; HTML from
//! another application pastes as `jade-office-model`'s `RichClip::from_html`
//! reads it. Where the browser refuses the read — no permission, an
//! embedding page — the app's own last copy is pasted.

use gpui::{div, prelude::*, px, AnyElement, ClickEvent, Context};
use jade_web::word::clipboard::{PasteFormat, PasteMode, RichClip, WordClipboard};
use wasm_bindgen::{JsCast, JsValue};

use super::{caption, check, list_row, Dialog};
use crate::view::DocView as DocumentView;
use crate::{i18n, palette as theme};

/// The dialog: the clipboard's formats and the one chosen.
pub struct PasteSpecial {
    pub formats: Vec<PasteFormat>,
    pub chosen: usize,
    pub clip: RichClip,
}

/// What the clipboard held when it was read.
struct Clipboard {
    clip: RichClip,
    /// Formatting came with it: the app's own copy, or HTML.
    rich: bool,
    /// The formatting is HTML from another application's copy.
    html: bool,
}

pub fn mode_label(mode: PasteMode) -> &'static str {
    i18n::text(match mode {
        PasteMode::KeepSource => "chars.paste.keep_source",
        PasteMode::MergeFormatting => "chars.paste.merge",
        PasteMode::TextOnly => "chars.paste.text_only",
    })
}

fn format_label(format: PasteFormat) -> &'static str {
    i18n::text(match format {
        PasteFormat::WordDocument => "chars.paste.format.word",
        PasteFormat::FormattedText => "chars.paste.format.rtf",
        PasteFormat::Html => "chars.paste.format.html",
        PasteFormat::UnformattedText => "chars.paste.format.text",
        PasteFormat::UnicodeText => "chars.paste.format.unicode",
    })
}

/// A property of a JS object.
fn get(object: &JsValue, name: &str) -> Option<JsValue> {
    js_sys::Reflect::get(object, &JsValue::from_str(name))
        .ok()
        .filter(|value| !value.is_undefined() && !value.is_null())
}

/// Calls `object.name(arguments…)`, for the promise it returns.
fn call(object: &JsValue, name: &str, arguments: &[JsValue]) -> Option<js_sys::Promise> {
    let method = get(object, name)?.dyn_into::<js_sys::Function>().ok()?;
    let arguments = arguments.iter().cloned().collect::<js_sys::Array>();
    method
        .apply(object, &arguments)
        .ok()?
        .dyn_into::<js_sys::Promise>()
        .ok()
}

async fn awaited(promise: Option<js_sys::Promise>) -> Option<JsValue> {
    wasm_bindgen_futures::JsFuture::from(promise?).await.ok()
}

/// The browser clipboard's HTML and plain text, as far as the browser lets
/// the page read them: `None` when it refuses outright.
async fn read_browser_clipboard() -> Option<(Option<String>, Option<String>)> {
    let navigator = get(&js_sys::global(), "navigator")?;
    let clipboard = get(&navigator, "clipboard")?;
    // `read` brings every format; a browser without it (or refusing it)
    // may still give the text.
    if let Some(items) = awaited(call(&clipboard, "read", &[])).await {
        let items = js_sys::Array::from(&items);
        let (mut html, mut text) = (None, None);
        for item in items.iter() {
            let types = get(&item, "types").map(|types| js_sys::Array::from(&types));
            let has = |kind: &str| {
                types.as_ref().is_some_and(|types| {
                    types.iter().any(|t| t.as_string().as_deref() == Some(kind))
                })
            };
            for (kind, slot) in [("text/html", &mut html), ("text/plain", &mut text)] {
                if slot.is_some() || !has(kind) {
                    continue;
                }
                let blob = awaited(call(&item, "getType", &[JsValue::from_str(kind)])).await;
                if let Some(blob) = blob {
                    *slot = awaited(call(&blob, "text", &[]))
                        .await
                        .and_then(|value| value.as_string());
                }
            }
        }
        if html.is_some() || text.is_some() {
            return Some((html, text));
        }
    }
    let text = awaited(call(&clipboard, "readText", &[]))
        .await
        .and_then(|value| value.as_string())?;
    Some((None, Some(text)))
}

/// What a paste pastes: this app's last copy when the clipboard's text is
/// still that copy's, else the HTML from elsewhere, else the text — or,
/// when the browser will not let the page read it, the app's own copy.
fn resolve(read: Option<(Option<String>, Option<String>)>) -> Option<Clipboard> {
    let kept = crate::view::RICH_CLIP.with(|kept| kept.borrow().clone());
    let Some((html, text)) = read else {
        return kept.map(|clip| Clipboard {
            clip,
            rich: true,
            html: false,
        });
    };
    if let (Some(kept), Some(text)) = (&kept, &text) {
        if kept.matches(text) {
            return Some(Clipboard {
                clip: kept.clone(),
                rich: true,
                html: false,
            });
        }
    }
    if let Some(html) = html.filter(|html| !html.trim().is_empty()) {
        let clip = RichClip::from_html(&html);
        if !clip.paragraphs.is_empty() {
            return Some(Clipboard {
                clip,
                rich: true,
                html: true,
            });
        }
    }
    let text = text.filter(|text| !text.is_empty())?;
    Some(Clipboard {
        clip: RichClip::from_text(&text),
        rich: false,
        html: false,
    })
}

/// Reads the clipboard, then hands what it held to `then`.
fn with_clipboard(
    cx: &mut Context<DocumentView>,
    then: impl FnOnce(&mut DocumentView, Clipboard, &mut Context<DocumentView>) + 'static,
) {
    cx.spawn(async move |view, cx| {
        let read = read_browser_clipboard().await;
        let Some(clipboard) = resolve(read) else {
            return;
        };
        view.update(cx, |view, cx| {
            then(view, clipboard, cx);
            // Not from an input event: nothing else asks for a frame.
            cx.refresh_windows();
        })
        .ok();
    })
    .detach();
}

/// Pastes the clipboard in `mode`, as one undo step.
pub fn paste_with(_view: &mut DocumentView, mode: PasteMode, cx: &mut Context<DocumentView>) {
    with_clipboard(cx, move |view, clipboard, cx| {
        paste_clip(view, &clipboard.clip, mode, cx)
    });
}

fn paste_clip(
    view: &mut DocumentView,
    clip: &RichClip,
    mode: PasteMode,
    cx: &mut Context<DocumentView>,
) {
    let result =
        view.word_edit_result(|document| WordClipboard::paste_as(document, clip, mode).map(|_| ()));
    view.after_character_edit(result, cx);
}

/// 形式を選択して貼り付け (⌃⌘V).
pub fn open(_view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    with_clipboard(cx, |view, clipboard, cx| {
        let formats = if clipboard.html {
            // HTML from another application: no Word or RTF copy of it.
            vec![
                PasteFormat::Html,
                PasteFormat::UnformattedText,
                PasteFormat::UnicodeText,
            ]
        } else {
            PasteFormat::available(clipboard.rich)
        };
        view.characters.open(Dialog::PasteSpecial(PasteSpecial {
            formats,
            chosen: 0,
            clip: clipboard.clip,
        }));
        cx.notify();
    });
}

/// OK: the chosen format's paste.
pub(super) fn confirm(
    view: &mut DocumentView,
    dialog: PasteSpecial,
    cx: &mut Context<DocumentView>,
) {
    let mode = dialog
        .formats
        .get(dialog.chosen)
        .map(|format| format.mode())
        .unwrap_or(PasteMode::TextOnly);
    paste_clip(view, &dialog.clip, mode, cx);
}

pub(super) fn body(view: &mut DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    let Some(Dialog::PasteSpecial(dialog)) = view.characters.dialog.as_ref() else {
        return div().into_any_element();
    };
    let chosen = dialog.chosen;
    let preview: String = dialog.clip.plain_text().chars().take(120).collect();
    let rows = dialog
        .formats
        .iter()
        .enumerate()
        .map(|(index, format)| {
            list_row(
                ("paste-format", index),
                format_label(*format),
                index == chosen,
            )
            .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                if let Some(Dialog::PasteSpecial(dialog)) = view.characters.dialog.as_mut() {
                    dialog.chosen = index;
                }
                cx.notify();
            }))
        })
        .collect::<Vec<_>>();
    div()
        .w(px(420.0))
        .flex()
        .flex_col()
        .gap_2()
        .child(caption(i18n::text("chars.paste.source")))
        .child(
            div()
                .px_1()
                .border_1()
                .border_color(theme::border())
                .bg(theme::canvas())
                .text_color(theme::text_dim())
                .child(preview),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .child(check("paste-as", i18n::text("chars.paste.paste"), true))
                .child(
                    check("paste-link", i18n::text("chars.paste.link"), false)
                        .text_color(theme::text_faint()),
                ),
        )
        .child(caption(i18n::text("chars.paste.as")))
        .child(
            div()
                .flex()
                .flex_col()
                .border_1()
                .border_color(theme::border())
                .p_1()
                .children(rows),
        )
        .into_any_element()
}

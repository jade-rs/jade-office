//! 貼り付けのオプション and 形式を選択して貼り付け: a paste that keeps the
//! copy's formatting, merges it with the destination's (⌥⇧⌘V), or keeps
//! the text only; and the dialog (⌃⌘V) that lists the clipboard's formats.

use gpui::{div, prelude::*, px, AnyElement, ClickEvent, Context};
use jade_office_model::word::clipboard::{PasteFormat, PasteMode, RichClip, WordClipboard};

use super::{caption, check, list_row, Dialog};
use crate::{i18n, theme, DocumentView, RICH_CLIP};

/// The dialog: the clipboard's formats and the one chosen.
pub struct PasteSpecial {
    pub formats: Vec<PasteFormat>,
    pub chosen: usize,
    pub clip: RichClip,
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

/// What the clipboard holds: this app's last copy when its text is still
/// there, otherwise the text alone. `None` for an empty clipboard.
fn clipboard(cx: &mut Context<DocumentView>) -> Option<(RichClip, bool)> {
    let text = cx.read_from_clipboard().and_then(|item| item.text())?;
    let kept = RICH_CLIP.with(|kept| kept.borrow().clone());
    Some(match kept.filter(|clip| clip.matches(&text)) {
        Some(clip) => (clip, true),
        None => (RichClip::from_text(&text), false),
    })
}

/// Pastes the clipboard in `mode`, as one undo step.
pub fn paste_with(view: &mut DocumentView, mode: PasteMode, cx: &mut Context<DocumentView>) {
    let Some((clip, _)) = clipboard(cx) else {
        return;
    };
    paste_clip(view, &clip, mode, cx);
}

fn paste_clip(
    view: &mut DocumentView,
    clip: &RichClip,
    mode: PasteMode,
    cx: &mut Context<DocumentView>,
) {
    if view.document.selection().is_none() {
        view.document
            .place_caret(view.insert_at.0, view.insert_at.1);
    }
    match WordClipboard::paste_as(&mut view.document, clip, mode) {
        Ok(caret) => {
            view.selection_anchor = None;
            view.insert_at = caret;
            view.caret = view.document.place_caret(caret.0, caret.1);
            view.dirty = true;
            view.rebuild_after_edit();
        }
        Err(error) => eprintln!("paste failed: {error}"),
    }
    cx.notify();
}

/// 形式を選択して貼り付け (⌃⌘V).
pub fn open(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let Some((clip, rich)) = clipboard(cx) else {
        return;
    };
    view.characters.open(Dialog::PasteSpecial(PasteSpecial {
        formats: PasteFormat::available(rich),
        chosen: 0,
        clip,
    }));
    cx.notify();
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

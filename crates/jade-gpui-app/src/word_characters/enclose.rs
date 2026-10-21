//! 囲い文字: a character in ○, □, △ or ◇, as Word's `EQ \o\ac` field —
//! the selected character, or one typed in.

use gpui::{div, prelude::*, px, AnyElement, ClickEvent, Context};
use jade_office_model::text::TextEdit;
use jade_office_model::word::asian::{WordAsian, ENCLOSURES};

use super::{caption, list_row, Dialog};
use crate::{i18n, theme, DocumentView};

/// The enclosure chosen; the character is the dialog's field.
pub struct EncloseDialog {
    pub enclosure: usize,
}

pub fn open(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    if view.document.selection().is_none() {
        view.document
            .place_caret(view.insert_at.0, view.insert_at.1);
    }
    let text = WordAsian::enclosed_character_text(&view.document);
    view.characters
        .open(Dialog::Enclose(EncloseDialog { enclosure: 0 }));
    view.characters.field = TextEdit::selected(text);
    cx.notify();
}

pub(super) fn confirm(
    view: &mut DocumentView,
    dialog: EncloseDialog,
    text: String,
    cx: &mut Context<DocumentView>,
) {
    let enclosure = ENCLOSURES[dialog.enclosure.min(ENCLOSURES.len() - 1)];
    let done = WordAsian::insert_enclosed_character(&mut view.document, &text, enclosure);
    if done.is_ok() {
        view.selection_anchor = None;
        view.rebuild_after_edit();
    }
    view.after_character_edit(done, cx);
}

pub(super) fn body(view: &mut DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    let Some(Dialog::Enclose(dialog)) = view.characters.dialog.as_ref() else {
        return div().into_any_element();
    };
    let chosen = dialog.enclosure;
    let text = view.characters.field.text.clone();
    let preview_char: String = text.chars().take(1).collect();
    div()
        .w(px(320.0))
        .flex()
        .flex_col()
        .gap_2()
        .child(caption(i18n::text("chars.enclose.text")))
        .child(
            div()
                .px_1()
                .border_1()
                .border_color(theme::accent())
                .bg(theme::canvas())
                .child(format!("{text}▏")),
        )
        .child(caption(i18n::text("chars.enclose.enclosure")))
        .child(
            div()
                .flex()
                .gap_2()
                .children(ENCLOSURES.iter().enumerate().map(|(index, enclosure)| {
                    list_row(("enclosure", index), *enclosure, index == chosen)
                        .text_size(px(20.0))
                        .px_2()
                        .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                            if let Some(Dialog::Enclose(dialog)) = view.characters.dialog.as_mut() {
                                dialog.enclosure = index;
                            }
                            cx.notify();
                        }))
                })),
        )
        .child(caption(i18n::text("chars.font.preview")))
        .child(
            div()
                .h(px(56.0))
                .flex()
                .items_center()
                .justify_center()
                .border_1()
                .border_color(theme::border())
                .bg(gpui::white())
                .child(
                    div()
                        .relative()
                        .size(px(40.0))
                        .text_color(gpui::rgb(0x000000))
                        .child(
                            div()
                                .absolute()
                                .inset_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(36.0))
                                .child(ENCLOSURES[chosen]),
                        )
                        .child(
                            div()
                                .absolute()
                                .inset_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(20.0))
                                .child(preview_char),
                        ),
                ),
        )
        .into_any_element()
}

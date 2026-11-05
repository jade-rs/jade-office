//! 書式の詳細: the pane beside the pages with the selection's フォント, 段落
//! and セクション formatting (`jade_office_model::word::reveal_formatting`),
//! each heading a link to the dialog that changes it.

use gpui::{div, prelude::*, px, AnyElement, ClickEvent, Context};
use jade_office_model::word::page_setup::SetupTab;
use jade_office_model::word::reveal_formatting::{RevealGroup, RevealValue, WordRevealFormatting};
use jade_office_model::word::styles::display_name;

use crate::ribbon::Action;
use crate::word_characters::CharAction;
use crate::word_page_setup::SetupAction;
use crate::{i18n, theme, DocumentView};

/// 表示 › 書式の詳細: the pane on or off.
pub fn toggle(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    view.reveal_formatting = !view.reveal_formatting;
    cx.notify();
}

/// The ribbon button.
pub fn button(view: &DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    view.ribbon_button(
        "reveal-formatting",
        i18n::text("reveal.title"),
        i18n::text("reveal.tooltip"),
        view.reveal_formatting,
        cx,
        toggle,
    )
    .into_any_element()
}

/// What a heading's link opens.
fn dialog(group: RevealGroup) -> Action {
    match group {
        RevealGroup::Font => Action::Characters(CharAction::OpenFontDialog),
        RevealGroup::Paragraph => Action::ParagraphDialog,
        RevealGroup::Section => Action::Setup(SetupAction::Open(SetupTab::Margins)),
    }
}

/// A value as the pane shows it.
pub fn shown(value: &RevealValue) -> String {
    match value {
        RevealValue::Text(text) => text.clone(),
        RevealValue::Key(key) => i18n::text(key).to_string(),
        RevealValue::Style(name) => {
            display_name(name, i18n::language() == i18n::Language::Japanese)
        }
    }
}

/// The pane, while it is open.
pub fn pane(view: &mut DocumentView, cx: &mut Context<DocumentView>) -> Option<AnyElement> {
    if !view.reveal_formatting {
        return None;
    }
    if view.document.selection().is_none() {
        let (paragraph, offset) = view.insert_at;
        view.document.place_caret(paragraph, offset);
    }
    let mut body = div().flex().flex_col().gap_1();
    for group in RevealGroup::ALL {
        body = body.child(
            div()
                .id(group.label())
                .mt_2()
                .text_sm()
                .text_color(theme::accent())
                .cursor_pointer()
                .hover(|link| link.underline())
                .child(i18n::text(group.label()))
                .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                    view.ribbon_action(dialog(group), window, cx)
                })),
        );
        for row in view.document.reveal(group) {
            body = body.child(
                div()
                    .flex()
                    .gap_2()
                    .pl_2()
                    .child(
                        div()
                            .w(px(96.0))
                            .flex_none()
                            .text_color(theme::text_dim())
                            .child(i18n::text(row.label)),
                    )
                    .child(div().child(shown(&row.value))),
            );
        }
    }
    Some(
        div()
            .id("reveal-pane")
            .flex()
            .flex_col()
            .flex_none()
            .w(px(260.0))
            .h_full()
            .gap_1()
            .p_3()
            .overflow_y_scroll()
            .bg(theme::panel())
            .border_l_1()
            .border_color(theme::border())
            .text_xs()
            .text_color(theme::text())
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(div().text_sm().child(i18n::text("reveal.title")))
                    .child(
                        div()
                            .id("reveal-close")
                            .px_1()
                            .rounded_md()
                            .cursor_pointer()
                            .hover(|close| close.bg(theme::hovered()))
                            .child("×")
                            .on_click(cx.listener(|view, _: &ClickEvent, _, cx| toggle(view, cx))),
                    ),
            )
            .child(body)
            .into_any_element(),
    )
}

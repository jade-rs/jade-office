//! The browser's 書式の詳細, as the native app's `word_reveal`: the pane
//! beside the pages with the selection's フォント, 段落 and セクション
//! formatting, each heading a link to its dialog.

use gpui::{div, prelude::*, px, AnyElement, ClickEvent, Context};
use jade_web::word::page_setup::SetupTab;
use jade_web::word::reveal_formatting::{RevealGroup, RevealValue, WordRevealFormatting};
use jade_web::word::styles::display_name;

use super::word_setup::SetupAction;
use super::DocView;
use crate::ribbon::Action;
use crate::{i18n, palette};

/// 表示 › 書式の詳細: the pane on or off.
fn toggle(view: &mut DocView, cx: &mut Context<DocView>) {
    view.reveal_formatting = !view.reveal_formatting;
    cx.notify();
}

/// The ribbon button.
pub fn button(on: bool, cx: &mut Context<DocView>) -> AnyElement {
    div()
        .id("reveal-formatting")
        .flex()
        .flex_none()
        .items_center()
        .h(px(24.0))
        .px_2()
        .rounded_md()
        .text_xs()
        .text_color(palette::text())
        .whitespace_nowrap()
        .cursor_pointer()
        .when(on, |button| button.bg(palette::selected()))
        .hover(|button| button.bg(palette::hovered()))
        .on_click(cx.listener(|view, _: &ClickEvent, _, cx| toggle(view, cx)))
        .child(i18n::text("reveal.title"))
        .into_any_element()
}

/// A value as the pane shows it.
fn shown(value: &RevealValue) -> String {
    match value {
        RevealValue::Text(text) => text.clone(),
        RevealValue::Key(key) => i18n::text(key).to_string(),
        RevealValue::Style(name) => display_name(name, !i18n::is_english()),
    }
}

impl DocView {
    /// The pane beside the pages, while it is open.
    pub(super) fn reveal_pane(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.reveal_formatting {
            return None;
        }
        let document = self.word_ref()?;
        let mut body = div().flex().flex_col().gap_1();
        for group in RevealGroup::ALL {
            let action = match group {
                RevealGroup::Font => None,
                RevealGroup::Paragraph => Some(Action::ParagraphDialog),
                RevealGroup::Section => Some(Action::Setup(SetupAction::Open(SetupTab::Margins))),
            };
            body = body.child(
                div()
                    .id(group.label())
                    .mt_2()
                    .text_sm()
                    .text_color(palette::accent())
                    .cursor_pointer()
                    .hover(|link| link.underline())
                    .child(i18n::text(group.label()))
                    .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                        match action.clone() {
                            Some(action) => view.ribbon_action(action, window, cx),
                            None => super::word_characters::open_font_dialog(view, cx),
                        }
                    })),
            );
            for row in document.reveal(group) {
                body = body.child(
                    div()
                        .flex()
                        .gap_2()
                        .pl_2()
                        .child(
                            div()
                                .w(px(96.0))
                                .flex_none()
                                .text_color(palette::dim())
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
                .w(px(240.0))
                .h_full()
                .gap_1()
                .px_3()
                .pt_3()
                .overflow_y_scroll()
                .bg(palette::panel())
                .border_l_1()
                .border_color(palette::border())
                .text_xs()
                .text_color(palette::text())
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
                                .hover(|close| close.bg(palette::hovered()))
                                .child("×")
                                .on_click(cx.listener(|view, _: &ClickEvent, _, cx| toggle(view, cx))),
                        ),
                )
                .child(body)
                .into_any_element(),
        )
    }
}

//! Word のオプション (Word for Mac's 環境設定, ⌘,): 表示 (which 編集記号
//! always show), 編集 (上書き入力, the Insert key, 貼り付けの既定) and
//! リボンとツール バー (which tabs show, the クイック アクセス ツール バー's
//! commands and a group of the user's own on ホーム). Each change takes
//! effect at once and is kept for the next run.

use gpui::{div, prelude::*, px, AnyElement, Context, SharedString};
use jade_office_model::word::clipboard::PasteMode;

use super::customize::{self, QuickCommand};
use super::paste::{self, Source};
use super::{dialog_frame_with, input, marks, ViewDialog};
use crate::word_dialog::{check, chip, on};
use crate::{i18n, theme, DocumentView, RibbonTab};

/// The dialog's pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionsPage {
    View,
    Edit,
    Ribbon,
}

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

pub fn open(view: &mut DocumentView, page: OptionsPage) {
    view.views.dialog = Some(ViewDialog::Options(page));
}

fn page_chip(
    id: &'static str,
    key: &'static str,
    page: OptionsPage,
    now: OptionsPage,
    cx: &Context<DocumentView>,
) -> AnyElement {
    chip(id, t(key), page == now)
        .on_click(on(cx, move |view, cx| {
            open(view, page);
            cx.notify();
        }))
        .into_any_element()
}

/// The dialog's body for `page`.
pub fn overlay(
    view: &DocumentView,
    page: OptionsPage,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    let cx: &Context<DocumentView> = cx;
    let pages = div()
        .flex()
        .gap_1()
        .child(page_chip(
            "options-view",
            "options.page.view",
            OptionsPage::View,
            page,
            cx,
        ))
        .child(page_chip(
            "options-edit",
            "options.page.edit",
            OptionsPage::Edit,
            page,
            cx,
        ))
        .child(page_chip(
            "options-ribbon",
            "options.page.ribbon",
            OptionsPage::Ribbon,
            page,
            cx,
        ));
    let body = match page {
        OptionsPage::View => view_page(cx),
        OptionsPage::Edit => edit_page(cx),
        OptionsPage::Ribbon => ribbon_page(cx),
    };
    let _ = view;
    dialog_frame_with(
        "word-options",
        t("options.title"),
        560.0,
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(pages)
            .child(body)
            .into_any_element(),
        cx,
        |view, cx| {
            view.views.dialog = None;
            cx.notify();
        },
    )
}

fn view_page(cx: &Context<DocumentView>) -> AnyElement {
    let shown = marks::kinds();
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_color(theme::text_dim())
                .child(t("options.marks")),
        )
        .children(
            marks::KINDS
                .iter()
                .enumerate()
                .map(|(index, (key, label))| {
                    let key: &'static str = key;
                    let on_now = shown.get(key);
                    check(("options-mark", index), t(label), on_now)
                        .on_click(on(cx, move |view, cx| {
                            marks::set(key, !marks::kinds().get(key));
                            marks::apply(view);
                            cx.notify();
                        }))
                        .into_any_element()
                }),
        )
        .into_any_element()
}

fn edit_page(cx: &Context<DocumentView>) -> AnyElement {
    let overtype = check("options-overtype", t("options.overtype"), input::overtype())
        .on_click(on(cx, input::toggle_overtype));
    let insert_key = check(
        "options-insert-key",
        t("options.insert_key"),
        input::insert_key_toggles(),
    )
    .on_click(on(cx, |_view, cx| {
        let on = !input::insert_key_toggles();
        super::settings::set_value("options.txt", "insert_key", if on { "1" } else { "0" });
        cx.notify();
    }));
    let modes = [
        (PasteMode::KeepSource, "context.paste_keep"),
        (PasteMode::MergeFormatting, "context.paste_merge"),
        (PasteMode::TextOnly, "context.paste_text"),
    ];
    let rows = Source::ALL.into_iter().enumerate().map(|(row, source)| {
        let current = paste::mode(source);
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(
                div()
                    .w(px(160.0))
                    .text_color(theme::text_dim())
                    .child(t(source.label_key())),
            )
            .children(modes.iter().enumerate().map(|(column, (mode, key))| {
                let mode = *mode;
                chip(("options-paste", row * 3 + column), t(key), current == mode)
                    .on_click(on(cx, move |_view, cx| {
                        paste::set_mode(source, mode);
                        cx.notify();
                    }))
                    .into_any_element()
            }))
            .into_any_element()
    });
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(overtype)
        .child(insert_key)
        .child(
            div()
                .text_color(theme::text_dim())
                .child(t("options.paste")),
        )
        .children(rows)
        .into_any_element()
}

fn ribbon_page(cx: &Context<DocumentView>) -> AnyElement {
    let tabs = RibbonTab::ALL.into_iter().enumerate().map(|(index, tab)| {
        check(
            ("options-tab", index),
            tab.label(),
            customize::tab_shown(tab),
        )
        .on_click(on(cx, move |_view, cx| {
            customize::set_tab_shown(tab, !customize::tab_shown(tab));
            cx.notify();
        }))
        .into_any_element()
    });
    let toolbar = customize::toolbar_commands();
    let group = customize::group_commands();
    let command_list = |key: &'static str, chosen: Vec<QuickCommand>, id: &'static str| {
        div()
            .id(id)
            .flex()
            .flex_wrap()
            .gap_1()
            .max_h(px(150.0))
            .overflow_y_scroll()
            .children(
                QuickCommand::ALL
                    .into_iter()
                    .enumerate()
                    .map(move |(index, command)| {
                        let chosen_now = chosen.contains(&command);
                        let chosen = chosen.clone();
                        chip(
                            (id, index),
                            SharedString::from(t(command.label_key())),
                            chosen_now,
                        )
                        .on_click(on(cx, move |_view, cx| {
                            let mut list = chosen.clone();
                            if chosen_now {
                                list.retain(|other| *other != command);
                            } else {
                                list.push(command);
                            }
                            customize::set_commands(key, &list);
                            cx.notify();
                        }))
                        .into_any_element()
                    }),
            )
    };
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_color(theme::text_dim()).child(t("options.tabs")))
        .child(div().flex().flex_wrap().gap_2().children(tabs))
        .child(div().text_color(theme::text_dim()).child(t("options.qat")))
        .child(command_list("qat", toolbar, "options-qat"))
        .child(
            div()
                .text_color(theme::text_dim())
                .child(t("options.custom_group")),
        )
        .child(command_list("custom_group", group, "options-group"))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⌘, opens Word のオプション; each page draws; a hidden tab leaves
    /// the ribbon and a toolbar command shows in the strip.
    #[gpui::test]
    fn the_options_dialog_draws_and_takes_effect(cx: &mut gpui::TestAppContext) {
        let (view, cx) = crate::word_views::tests::open_contract(cx);
        cx.dispatch_action(super::super::WordOptions);
        for page in [OptionsPage::View, OptionsPage::Edit, OptionsPage::Ribbon] {
            view.update(cx, |view, cx| {
                open(view, page);
                cx.notify();
            });
            cx.run_until_parked();
        }
        view.update(cx, |view, cx| {
            customize::set_tab_shown(RibbonTab::Design, false);
            customize::set_commands("qat", &[QuickCommand::Bold]);
            assert!(!view
                .ribbon_tabs()
                .iter()
                .any(|(tab, _)| *tab == RibbonTab::Design));
            view.views.dialog = None;
            cx.notify();
            customize::set_tab_shown(RibbonTab::Design, true);
        });
        cx.run_until_parked();
    }
}

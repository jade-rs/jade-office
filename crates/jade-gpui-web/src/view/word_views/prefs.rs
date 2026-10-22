//! Word のオプション in the browser (Word for Mac's 環境設定): 表示 (which
//! 編集記号 always show), 編集 (上書き入力, the Insert key, 貼り付けの既定),
//! リボンとツール バー (which tabs show, the クイック アクセス ツール バー's
//! commands and a group of the user's own on ホーム). Each change takes
//! effect at once and is kept in the browser for the next visit.

use std::cell::RefCell;

use gpui::{div, prelude::*, px, AnyElement, Context, SharedString, Window};
use jade_web::editing::FormattingMarkKinds;
use jade_web::word::clipboard::PasteMode;
use jade_web::word::customize::QuickCommand;

use super::customize;
use super::settings;
use super::ViewDialog;
use crate::ribbon::Tab;
use crate::view::word_kit::{button, check, chip, modal, on};
use crate::view::DocView as DocumentView;
use crate::{i18n, palette as theme};

/// The dialog's pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionsPage {
    View,
    Edit,
    Ribbon,
}

const OPTIONS: &str = "options.txt";

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

/// The marks' kinds, by their saved key, with their labels' keys.
pub const KINDS: [(&str, &str); 6] = [
    ("tabs", "options.marks.tabs"),
    ("spaces", "options.marks.spaces"),
    ("paragraph_marks", "options.marks.paragraph_marks"),
    ("hidden_text", "options.marks.hidden_text"),
    ("optional_hyphens", "options.marks.optional_hyphens"),
    ("anchors", "options.marks.anchors"),
];

fn mark(key: &str) -> bool {
    settings::flag(OPTIONS, &format!("marks.{key}"), false)
}

/// The kinds of mark the settings always show.
pub fn mark_kinds() -> FormattingMarkKinds {
    FormattingMarkKinds {
        tabs: mark("tabs"),
        spaces: mark("spaces"),
        paragraph_marks: mark("paragraph_marks"),
        hidden_text: mark("hidden_text"),
        optional_hyphens: mark("optional_hyphens"),
        anchors: mark("anchors"),
    }
}

/// Whether typing replaces the characters after the caret (上書き入力).
pub fn overtype() -> bool {
    settings::flag(OPTIONS, "overtype", false)
}

pub fn set_overtype(on: bool) {
    settings::set_flag(OPTIONS, "overtype", on);
}

/// Whether the Insert key switches 上書き入力.
pub fn insert_key_toggles() -> bool {
    settings::flag(OPTIONS, "insert_key", true)
}

/// Where what is pasted came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    SameDocument,
    OtherDocument,
    OtherProgram,
}

impl Source {
    pub const ALL: [Source; 3] = [
        Source::SameDocument,
        Source::OtherDocument,
        Source::OtherProgram,
    ];

    fn key(self) -> &'static str {
        match self {
            Source::SameDocument => "paste_same",
            Source::OtherDocument => "paste_between",
            Source::OtherProgram => "paste_other",
        }
    }

    fn label_key(self) -> &'static str {
        match self {
            Source::SameDocument => "options.paste_same",
            Source::OtherDocument => "options.paste_between",
            Source::OtherProgram => "options.paste_other",
        }
    }
}

/// The mode set for pastes from `source`; 元の書式を保持 unless changed.
pub fn paste_mode(source: Source) -> PasteMode {
    match settings::value(OPTIONS, source.key()).as_deref() {
        Some("merge") => PasteMode::MergeFormatting,
        Some("text") => PasteMode::TextOnly,
        _ => PasteMode::KeepSource,
    }
}

fn set_paste_mode(source: Source, mode: PasteMode) {
    let id = match mode {
        PasteMode::KeepSource => "keep",
        PasteMode::MergeFormatting => "merge",
        PasteMode::TextOnly => "text",
    };
    settings::set_value(OPTIONS, source.key(), id);
}

thread_local! {
    /// The document the last copy came from.
    static COPIED_FROM: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// A copy (or cut) in the document `name`.
pub fn note_copy(name: &str) {
    COPIED_FROM.with(|from| *from.borrow_mut() = Some(name.to_string()));
}

/// What a ⌘V of the app's own copy keeps in the document `name`.
pub fn default_paste(name: &str) -> PasteMode {
    let same = COPIED_FROM.with(|from| from.borrow().as_deref() == Some(name));
    paste_mode(if same {
        Source::SameDocument
    } else {
        Source::OtherDocument
    })
}

/// The dialog over the window, at `page`.
pub fn overlay(page: OptionsPage, window: &Window, cx: &mut Context<DocumentView>) -> AnyElement {
    let page_chip = |id: &'static str, key: &'static str, this: OptionsPage| {
        chip(id, t(key), page == this)
            .on_click(on(cx, move |view, cx| {
                view.views.dialog = Some(ViewDialog::Options(this));
                cx.notify();
            }))
            .into_any_element()
    };
    let pages = div()
        .flex()
        .gap_1()
        .child(page_chip(
            "options-view",
            "options.page.view",
            OptionsPage::View,
        ))
        .child(page_chip(
            "options-edit",
            "options.page.edit",
            OptionsPage::Edit,
        ))
        .child(page_chip(
            "options-ribbon",
            "options.page.ribbon",
            OptionsPage::Ribbon,
        ));
    let body = match page {
        OptionsPage::View => view_page(cx),
        OptionsPage::Edit => edit_page(cx),
        OptionsPage::Ribbon => ribbon_page(cx),
    };
    let buttons = div()
        .flex()
        .justify_end()
        .child(
            button("options-close", t("format.ok"), true).on_click(on(cx, |view, cx| {
                view.views.dialog = None;
                cx.notify();
            })),
        )
        .into_any_element();
    modal(
        "word-options",
        580.0,
        t("options.title"),
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(pages)
            .child(body)
            .into_any_element(),
        None,
        buttons,
        window,
    )
}

fn caption(key: &'static str) -> gpui::Div {
    div().text_color(theme::text_dim()).child(t(key))
}

fn view_page(cx: &Context<DocumentView>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(caption("options.marks"))
        .children(KINDS.iter().enumerate().map(|(index, (key, label))| {
            let key: &'static str = key;
            check(("options-mark", index), t(label), mark(key))
                .on_click(on(cx, move |_view, cx| {
                    settings::set_flag(OPTIONS, &format!("marks.{key}"), !mark(key));
                    cx.notify();
                }))
                .into_any_element()
        }))
        .into_any_element()
}

fn edit_page(cx: &Context<DocumentView>) -> AnyElement {
    let modes = [
        (PasteMode::KeepSource, "context.paste_keep"),
        (PasteMode::MergeFormatting, "context.paste_merge"),
        (PasteMode::TextOnly, "context.paste_text"),
    ];
    let rows = Source::ALL.into_iter().enumerate().map(|(row, source)| {
        let current = paste_mode(source);
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(
                div()
                    .w(px(170.0))
                    .text_color(theme::text_dim())
                    .child(t(source.label_key())),
            )
            .children(modes.iter().enumerate().map(|(column, (mode, key))| {
                let mode = *mode;
                chip(("options-paste", row * 3 + column), t(key), current == mode)
                    .on_click(on(cx, move |_view, cx| {
                        set_paste_mode(source, mode);
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
        .child(
            check("options-overtype", t("options.overtype"), overtype()).on_click(on(
                cx,
                |_view, cx| {
                    set_overtype(!overtype());
                    cx.notify();
                },
            )),
        )
        .child(
            check(
                "options-insert-key",
                t("options.insert_key"),
                insert_key_toggles(),
            )
            .on_click(on(cx, |_view, cx| {
                settings::set_flag(OPTIONS, "insert_key", !insert_key_toggles());
                cx.notify();
            })),
        )
        .child(caption("options.paste"))
        .children(rows)
        .into_any_element()
}

fn ribbon_page(cx: &Context<DocumentView>) -> AnyElement {
    let tabs = Tab::ALL.into_iter().enumerate().map(|(index, tab)| {
        check(
            ("options-tab", index),
            t(tab.label()),
            customize::tab_shown(tab),
        )
        .on_click(on(cx, move |_view, cx| {
            customize::set_tab_shown(tab, !customize::tab_shown(tab));
            cx.notify();
        }))
        .into_any_element()
    });
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
        .child(caption("options.tabs"))
        .child(div().flex().flex_wrap().gap_2().children(tabs))
        .child(caption("options.qat"))
        .child(command_list(
            "qat",
            customize::toolbar_commands(),
            "options-qat",
        ))
        .child(caption("options.custom_group"))
        .child(command_list(
            "custom_group",
            customize::group_commands(),
            "options-group",
        ))
        .into_any_element()
}

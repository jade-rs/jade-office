//! ツール → オートコレクト: the dialog with its two tabs, オートコレクト (the
//! capital rules and the replacement list, entries added and deleted) and
//! 入力オートフォーマット. OK keeps the settings for every document and
//! saves them in the browser for the next visit.

use gpui::{div, prelude::*, px, AnyElement, Context, SharedString, Window};
use jade_web::text::TextEdit;
use jade_web::word::autocorrect::{
    flag_mut, AutoCorrectOptions, AUTOCORRECT_KEYS, AUTOFORMAT_KEYS,
};

use super::{settings, ViewDialog};
use crate::view::word_kit::{button, check, chip, input, modal, ok_cancel, on};
use crate::view::DocView as DocumentView;
use crate::{i18n, palette as theme};

/// The オートコレクト dialog being edited.
pub struct AutoCorrectDialog {
    pub options: AutoCorrectOptions,
    /// 入力オートフォーマット's tab rather than オートコレクト's.
    pub autoformat: bool,
    /// 修正文字列 and 修正後の文字列.
    pub fields: [TextEdit; 2],
    pub focus: usize,
    /// The list entry picked, which 削除 takes out.
    pub picked: Option<usize>,
}

impl AutoCorrectDialog {
    pub fn open() -> Self {
        AutoCorrectDialog {
            options: settings::autocorrect(),
            autoformat: false,
            fields: [TextEdit::new(String::new()), TextEdit::new(String::new())],
            focus: 0,
            picked: None,
        }
    }

    /// 追加 (or 置換, when the entry is there): the two fields into the list.
    pub fn add(&mut self) {
        let from = self.fields[0].text.trim().to_string();
        let to = self.fields[1].text.clone();
        if from.is_empty() {
            return;
        }
        match self
            .options
            .entries
            .iter_mut()
            .find(|(key, _)| *key == from)
        {
            Some(entry) => entry.1 = to,
            None => {
                self.options.entries.push((from, to));
                self.options.entries.sort_by(|a, b| a.0.cmp(&b.0));
            }
        }
        self.fields = [TextEdit::new(String::new()), TextEdit::new(String::new())];
        self.focus = 0;
        self.picked = None;
    }

    /// 削除: the picked entry out of the list.
    pub fn delete(&mut self) {
        if let Some(index) = self.picked.take() {
            if index < self.options.entries.len() {
                self.options.entries.remove(index);
            }
        }
    }

    pub fn pick(&mut self, index: usize) {
        if let Some((from, to)) = self.options.entries.get(index) {
            self.fields = [TextEdit::new(from.clone()), TextEdit::new(to.clone())];
            self.picked = Some(index);
        }
    }
}

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

fn edit(
    view: &mut DocumentView,
    cx: &mut Context<DocumentView>,
    f: impl FnOnce(&mut AutoCorrectDialog),
) {
    if let Some(ViewDialog::AutoCorrect(dialog)) = view.views.dialog.as_mut() {
        f(dialog);
        cx.notify();
    }
}

/// OK: the settings kept and saved.
pub fn confirm(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    if let Some(ViewDialog::AutoCorrect(dialog)) = view.views.dialog.take() {
        settings::set_autocorrect(dialog.options);
    }
    cx.notify();
}

/// A key while the dialog is open; `true` when it was taken.
pub fn handle_key(view: &mut DocumentView, key: &str, cx: &mut Context<DocumentView>) -> bool {
    let Some(ViewDialog::AutoCorrect(dialog)) = view.views.dialog.as_mut() else {
        return false;
    };
    match key {
        "escape" => view.views.dialog = None,
        "enter" => {
            // In the replacement fields Enter adds the entry; elsewhere OK.
            if dialog.fields.iter().any(|field| !field.text.is_empty()) {
                dialog.add();
            } else {
                confirm(view, cx);
                return true;
            }
        }
        "tab" => dialog.focus = 1 - dialog.focus.min(1),
        key => {
            if !dialog.fields[dialog.focus.min(1)].edit_key(key) {
                return false;
            }
        }
    }
    cx.notify();
    true
}

/// The i18n key of a setting's label.
fn label_key(key: &str) -> &'static str {
    match key {
        "two_initial_capitals" => "autocorrect.two_initial_capitals",
        "capitalize_sentences" => "autocorrect.capitalize_sentences",
        "capitalize_days" => "autocorrect.capitalize_days",
        "replace_text" => "autocorrect.replace_text",
        "smart_quotes" => "autocorrect.smart_quotes",
        "ordinals" => "autocorrect.ordinals",
        "fractions" => "autocorrect.fractions",
        "hyphens" => "autocorrect.hyphens",
        "bulleted_lists" => "autocorrect.bulleted_lists",
        "numbered_lists" => "autocorrect.numbered_lists",
        "borders" => "autocorrect.borders",
        "headings" => "autocorrect.headings",
        "long_vowels" => "autocorrect.long_vowels",
        "brackets" => "autocorrect.brackets",
        "closing_phrases" => "autocorrect.closing_phrases",
        "cjk_spaces" => "autocorrect.cjk_spaces",
        _ => "autocorrect.kiki",
    }
}

/// The dialog over the window.
pub fn overlay(
    dialog: &AutoCorrectDialog,
    window: &Window,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    let tabs = div()
        .flex()
        .gap_1()
        .child(
            chip(
                "ac-tab-correct",
                t("autocorrect.tab.correct"),
                !dialog.autoformat,
            )
            .on_click(on(cx, |view, cx| {
                edit(view, cx, |dialog| dialog.autoformat = false)
            })),
        )
        .child(
            chip(
                "ac-tab-format",
                t("autocorrect.tab.format"),
                dialog.autoformat,
            )
            .on_click(on(cx, |view, cx| {
                edit(view, cx, |dialog| dialog.autoformat = true)
            })),
        );
    let keys: &[&'static str] = if dialog.autoformat {
        &AUTOFORMAT_KEYS
    } else {
        &AUTOCORRECT_KEYS
    };
    let mut options = dialog.options.clone();
    let checks: Vec<AnyElement> = keys
        .iter()
        .map(|key| {
            let key: &'static str = key;
            let on_now = flag_mut(&mut options, key).is_some_and(|flag| *flag);
            check(
                SharedString::from(format!("ac-{key}")),
                t(label_key(key)),
                on_now,
            )
            .on_click(on(cx, move |view, cx| {
                edit(view, cx, |dialog| {
                    if let Some(flag) = flag_mut(&mut dialog.options, key) {
                        *flag = !*flag;
                    }
                })
            }))
            .into_any_element()
        })
        .collect();
    let mut body = div().flex().flex_col().gap_2().child(tabs).children(checks);
    if !dialog.autoformat {
        let field = |index: usize, cx: &mut Context<DocumentView>| {
            input(
                ("ac-field", index),
                &dialog.fields[index],
                dialog.focus == index,
            )
            .w(px(150.0))
            .on_click(on(cx, move |view, cx| {
                edit(view, cx, |dialog| dialog.focus = index)
            }))
        };
        let entries: Vec<AnyElement> = dialog
            .options
            .entries
            .iter()
            .enumerate()
            .map(|(index, (from, to))| {
                div()
                    .id(("ac-entry", index))
                    .flex()
                    .gap_2()
                    .px_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .when(dialog.picked == Some(index), |row| {
                        row.bg(theme::selected())
                    })
                    .hover(|row| row.bg(theme::hovered()))
                    .on_click(on(cx, move |view, cx| {
                        edit(view, cx, |dialog| dialog.pick(index))
                    }))
                    .child(div().w(px(150.0)).child(from.clone()))
                    .child(div().child(to.clone()))
                    .into_any_element()
            })
            .collect();
        body = body
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .w(px(150.0))
                            .text_color(theme::text_dim())
                            .child(t("autocorrect.replace")),
                    )
                    .child(
                        div()
                            .text_color(theme::text_dim())
                            .child(t("autocorrect.with")),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(field(0, cx))
                    .child(field(1, cx))
                    .child(
                        button("ac-add", t("autocorrect.add"), false)
                            .on_click(on(cx, |view, cx| edit(view, cx, |dialog| dialog.add()))),
                    )
                    .child(
                        button("ac-delete", t("autocorrect.delete"), false)
                            .on_click(on(cx, |view, cx| edit(view, cx, |dialog| dialog.delete()))),
                    ),
            )
            .child(
                div()
                    .id("ac-entries")
                    .h(px(160.0))
                    .overflow_y_scroll()
                    .border_1()
                    .border_color(theme::border())
                    .rounded_sm()
                    .p_1()
                    .children(entries),
            );
    }
    let buttons = ok_cancel(
        "autocorrect",
        cx,
        |view, cx| {
            view.views.dialog = None;
            cx.notify();
        },
        confirm,
    );
    modal(
        "autocorrect-dialog",
        540.0,
        t("autocorrect.title"),
        body.into_any_element(),
        None,
        buttons,
        window,
    )
}

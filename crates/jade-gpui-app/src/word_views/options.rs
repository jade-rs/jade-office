//! ツール → オートコレクト: the dialog with its two tabs, オートコレクト (the
//! capital rules and the replacement list, entries added and deleted) and
//! 入力オートフォーマット. OK keeps the settings for every document and
//! saves them for the next run.

use gpui::{div, prelude::*, px, AnyElement, Context, SharedString};
use jade_office_model::text::TextEdit;
use jade_office_model::word::autocorrect::{
    flag_mut, AutoCorrectOptions, AUTOCORRECT_KEYS, AUTOFORMAT_KEYS,
};

use super::{dialog_frame_with, settings, ViewDialog};
use crate::word_dialog::{button, check, chip, on};
use crate::{i18n, theme, DocumentView};

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
        "enter" | "return" => {
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

/// The dialog's body.
pub fn body(dialog: &AutoCorrectDialog, cx: &mut Context<DocumentView>) -> AnyElement {
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
    let checks = keys.iter().map(|key| {
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
    });
    let mut body = div().flex().flex_col().gap_2().child(tabs).children(checks);
    if !dialog.autoformat {
        let field = |index: usize, cx: &mut Context<DocumentView>| {
            let text = &dialog.fields[index];
            let focused = dialog.focus == index;
            let (before, after) = text.split();
            div()
                .id(("ac-field", index))
                .flex()
                .items_center()
                .w(px(150.0))
                .h(px(24.0))
                .px_2()
                .rounded_sm()
                .border_1()
                .border_color(if focused {
                    theme::accent()
                } else {
                    theme::border()
                })
                .bg(theme::canvas())
                .overflow_hidden()
                .whitespace_nowrap()
                .on_click(on(cx, move |view, cx| {
                    edit(view, cx, |dialog| dialog.focus = index)
                }))
                .child(before.to_string())
                .when(focused, |field| {
                    field.child(div().w(px(1.0)).h(px(14.0)).bg(theme::accent()))
                })
                .child(after.to_string())
        };
        let entries = dialog
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
            })
            .collect::<Vec<_>>();
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
    body.into_any_element()
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
        _ => "autocorrect.kiki",
    }
}

/// The dialog over the window.
pub fn overlay(dialog: &AutoCorrectDialog, cx: &mut Context<DocumentView>) -> AnyElement {
    let body = body(dialog, cx);
    dialog_frame_with(
        "autocorrect-dialog",
        t("autocorrect.title"),
        520.0,
        body,
        cx,
        |view, cx| confirm(view, cx),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An entry added in the dialog corrects the next word typed, and the
    /// list is saved.
    #[gpui::test]
    fn an_entry_added_in_the_dialog_corrects(cx: &mut gpui::TestAppContext) {
        settings::set_autocorrect(AutoCorrectOptions::default());
        let (view, cx) = crate::word_views::tests::open_contract(cx);
        view.update(cx, |view, cx| {
            super::super::run(view, super::super::ViewAction::OpenAutoCorrect, cx)
        });
        cx.simulate_input("jdo");
        cx.simulate_keystrokes("tab");
        cx.simulate_input("jade-office");
        cx.simulate_keystrokes("enter");
        view.update(cx, |view, cx| {
            if let Some(ViewDialog::AutoCorrect(dialog)) = view.views.dialog.as_ref() {
                assert!(dialog
                    .options
                    .entries
                    .contains(&("jdo".to_string(), "jade-office".to_string())));
            } else {
                panic!("the dialog is open");
            }
            confirm(view, cx);
            let saved = settings::load("autocorrect.txt").unwrap();
            assert!(saved.contains("entry=jdo\tjade-office"));
            let paragraph = view.insert_at.0;
            let end = view.current_paragraph_text().len();
            view.insert_at = (paragraph, end);
            view.caret = view.document.place_caret(paragraph, end);
        });
        cx.simulate_input(" jdo ");
        view.read_with(cx, |view, _| {
            assert!(view.current_paragraph_text().ends_with(" jade-office "));
        });
    }
}

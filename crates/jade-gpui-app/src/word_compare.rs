//! 校閲 › 比較: the 文書の比較 and 組み込み dialogs — 元の文書 and 変更された
//! 文書 (this document, or one chosen from disk), 変更の表示名, and 比較の
//! 設定 (書式, 大文字/小文字の変更, 空白文字, コメント, 変更の表示単位) —
//! whose result opens as a new document, as Word opens 比較結果. What a
//! comparison is is `jade-office-model`'s `word::compare`.

use gpui::{div, prelude::*, px, AnyElement, ClickEvent, Context};
use jade_office_model::text::TextEdit;
use jade_office_model::word::compare::CompareForm;

use crate::word_characters::{button, check};
use crate::word_review_tools::{reviewer, ReviewDialog};
use crate::{i18n, theme, DocumentView};

/// The dialog: the shared form, and the label being typed.
pub struct CompareDialog {
    pub form: CompareForm,
    pub author: TextEdit,
}

/// Which document a 参照… button chooses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Original,
    Revised,
}

/// This document as it stands, to compare.
fn this_document(view: &mut DocumentView) -> Option<(String, Vec<u8>)> {
    let bytes = view.document.export_docx().ok()?;
    let name = std::path::Path::new(view.path.as_ref())
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| view.path.to_string());
    Some((name, bytes))
}

/// 比較… (or 組み込み…): this document as the original, the revised one to
/// choose.
pub fn open(view: &mut DocumentView, combine: bool, cx: &mut Context<DocumentView>) {
    let mut form = CompareForm::new(&reviewer().name, combine);
    form.original = this_document(view);
    let author = TextEdit::new(form.options.author.clone());
    view.review.dialog = Some(ReviewDialog::Compare(Box::new(CompareDialog {
        form,
        author,
    })));
    cx.notify();
}

/// 参照…: a document from disk for one side.
fn browse(side: Side, cx: &mut Context<DocumentView>) {
    let prompt = cx.prompt_for_paths(gpui::PathPromptOptions {
        files: true,
        directories: false,
        multiple: false,
        prompt: None,
    });
    cx.spawn(async move |view, cx| {
        let Ok(Ok(Some(paths))) = prompt.await else {
            return;
        };
        let Some(path) = paths.into_iter().next() else {
            return;
        };
        view.update(cx, |view, cx| {
            match std::fs::read(&path) {
                Ok(bytes) => {
                    let name = path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    choose(view, side, name, bytes);
                }
                Err(error) => {
                    view.table_status = Some(format!("{}: {error}", path.display()).into())
                }
            }
            cx.notify();
        })
        .ok();
    })
    .detach();
}

/// Puts a document on one side of the open dialog.
pub fn choose(view: &mut DocumentView, side: Side, name: String, bytes: Vec<u8>) {
    if let Some(ReviewDialog::Compare(dialog)) = view.review.dialog.as_mut() {
        match side {
            Side::Original => dialog.form.original = Some((name, bytes)),
            Side::Revised => dialog.form.revised = Some((name, bytes)),
        }
    }
}

/// The time the changes are put down to: now, as Word records it.
fn now() -> String {
    let now = crate::word_characters::local_now();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:00Z",
        now.year, now.month, now.day, now.hour, now.minute
    )
}

/// OK: the comparison, opened as a new document. Returns the result for
/// tests.
pub fn confirm(
    view: &mut DocumentView,
    dialog: CompareDialog,
    cx: &mut Context<DocumentView>,
) -> Option<Vec<u8>> {
    let mut form = dialog.form;
    form.options.author = dialog.author.text.trim().to_string();
    if form.options.author.is_empty() {
        form.options.author = reviewer().name;
    }
    match form.run(&now()) {
        Ok((bytes, summary)) => {
            let name = crate::shell::untitled_name("docx");
            let label = i18n::format(
                "compare.done",
                &[
                    ("insertions", &summary.insertions),
                    ("deletions", &summary.deletions),
                    ("formatting", &summary.formatting),
                ],
            );
            view.table_status = Some(label.into());
            if !cfg!(test) {
                crate::shell::open_untitled(name, bytes.clone(), cx);
            }
            cx.notify();
            Some(bytes)
        }
        Err(error) => {
            view.table_status = Some(error.into());
            // Left open, so the choice can be put right.
            view.review.dialog = Some(ReviewDialog::Compare(Box::new(CompareDialog {
                author: TextEdit::new(form.options.author.clone()),
                form,
            })));
            cx.notify();
            None
        }
    }
}

fn side_row(
    label: &'static str,
    chosen: Option<&str>,
    side: Side,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    let id = match side {
        Side::Original => "compare-original",
        Side::Revised => "compare-revised",
    };
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .w(px(112.0))
                .text_color(theme::text_dim())
                .child(i18n::text(label)),
        )
        .child(
            div()
                .w(px(220.0))
                .px_2()
                .py_0p5()
                .rounded_md()
                .border_1()
                .border_color(theme::border())
                .bg(theme::canvas())
                .overflow_hidden()
                .whitespace_nowrap()
                .text_color(if chosen.is_some() {
                    theme::text()
                } else {
                    theme::text_faint()
                })
                .child(
                    chosen
                        .map(str::to_string)
                        .unwrap_or_else(|| i18n::text("compare.none").to_string()),
                ),
        )
        .child(
            button(id, i18n::text("compare.browse"), false)
                .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| browse(side, cx))),
        )
        .child(
            button((id, 1usize), i18n::text("compare.this"), false).on_click(cx.listener(
                move |view, _: &ClickEvent, _, cx| {
                    if let Some((name, bytes)) = this_document(view) {
                        choose(view, side, name, bytes);
                    }
                    cx.notify();
                },
            )),
        )
        .into_any_element()
}

fn setting(
    id: &'static str,
    key: &'static str,
    on: bool,
    toggle: fn(&mut CompareDialog),
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    check(id, i18n::text(key), on)
        .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
            if let Some(ReviewDialog::Compare(dialog)) = view.review.dialog.as_mut() {
                toggle(dialog);
            }
            cx.notify();
        }))
        .into_any_element()
}

/// The dialog's body.
pub fn body(dialog: &CompareDialog, cx: &mut Context<DocumentView>) -> AnyElement {
    let options = &dialog.form.options;
    let (before, after) = dialog.author.split();
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(side_row(
            "compare.original",
            dialog.form.original.as_ref().map(|(name, _)| name.as_str()),
            Side::Original,
            cx,
        ))
        .child(side_row(
            "compare.revised",
            dialog.form.revised.as_ref().map(|(name, _)| name.as_str()),
            Side::Revised,
            cx,
        ))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .w(px(112.0))
                        .text_color(theme::text_dim())
                        .child(i18n::text("compare.label")),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .w(px(220.0))
                        .h(px(24.0))
                        .px_2()
                        .rounded_md()
                        .border_1()
                        .border_color(theme::accent())
                        .bg(theme::canvas())
                        .child(before.to_string())
                        .child(div().w(px(1.0)).h(px(16.0)).bg(theme::accent()))
                        .child(after.to_string()),
                ),
        )
        .child(
            div()
                .mt_1()
                .text_color(theme::text_dim())
                .child(i18n::text("compare.settings")),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_3()
                .child(setting(
                    "compare-formatting",
                    "compare.formatting",
                    options.formatting,
                    |dialog| dialog.form.options.formatting = !dialog.form.options.formatting,
                    cx,
                ))
                .child(setting(
                    "compare-case",
                    "compare.case",
                    options.case_changes,
                    |dialog| dialog.form.options.case_changes = !dialog.form.options.case_changes,
                    cx,
                ))
                .child(setting(
                    "compare-space",
                    "compare.space",
                    options.white_space,
                    |dialog| dialog.form.options.white_space = !dialog.form.options.white_space,
                    cx,
                ))
                .child(setting(
                    "compare-comments",
                    "compare.comments",
                    options.comments,
                    |dialog| dialog.form.options.comments = !dialog.form.options.comments,
                    cx,
                )),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .text_color(theme::text_dim())
                        .child(i18n::text("compare.level")),
                )
                .child(setting(
                    "compare-by-character",
                    "compare.by_character",
                    !options.by_word,
                    |dialog| dialog.form.options.by_word = false,
                    cx,
                ))
                .child(setting(
                    "compare-by-word",
                    "compare.by_word",
                    options.by_word,
                    |dialog| dialog.form.options.by_word = true,
                    cx,
                )),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 比較 from the tab: this document against a changed copy of it, the
    /// label typed into the dialog; the result marks the difference under
    /// that name.
    #[gpui::test]
    fn comparing_from_the_tab_labels_the_changes(cx: &mut gpui::TestAppContext) {
        use gpui::Focusable;
        let bytes = jade_office_model::word::new::blank_document(
            jade_office_model::word::new::Blank::English,
        );
        let (view, cx) = cx
            .add_window_view(|_window, cx| crate::untitled_view(&bytes, "base.docx", cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        view.update(cx, |view, _| {
            view.insert_at = (0, 0);
            view.caret = view.document.place_caret(0, 0);
        });
        cx.simulate_input("Original words.");
        let original = view.update(cx, |view, _| {
            let original = view.document.export_docx().unwrap();
            view.insert_at = (0, 0);
            view.caret = view.document.place_caret(0, 0);
            original
        });
        cx.simulate_input("New ");
        view.update(cx, |view, cx| {
            open(view, false, cx);
            // This document is the revised one; the original from before.
            let (name, revised) = this_document(view).unwrap();
            choose(view, Side::Revised, name, revised);
            choose(view, Side::Original, "original.docx".into(), original);
            if let Some(ReviewDialog::Compare(dialog)) = view.review.dialog.as_mut() {
                dialog.author = TextEdit::selected("Ann".into());
            }
        });
        cx.simulate_input("Reviewer A");
        view.update(cx, |view, cx| {
            let Some(ReviewDialog::Compare(dialog)) = view.review.dialog.take() else {
                panic!("the dialog is open");
            };
            assert_eq!(dialog.author.text, "Reviewer A");
            let result = confirm(view, *dialog, cx).expect("compared");
            let compared = jade_office_model::word::compare::reviewer_of(&result);
            assert_eq!(compared.as_deref(), Some("Reviewer A"));
        });
    }
}

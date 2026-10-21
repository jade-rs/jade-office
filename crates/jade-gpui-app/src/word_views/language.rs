//! 校閲 → 言語 → 校正言語の設定: the dialog that marks the selection's
//! language, and the language the status bar shows for the caret.

use gpui::{div, prelude::*, AnyElement, Context, SharedString};
use jade_office_model::word::language::{dominant, LanguageChoice, WordLanguage, LANGUAGES};

use super::{dialog_frame_with, ViewDialog};
use crate::ribbon::{group, row};
use crate::word_dialog::{check, chip, on};
use crate::{i18n, theme, DocumentView};

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

/// Opens the dialog on the caret's language.
pub fn open(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let language = view
        .document
        .language_at(view.insert_at)
        .or_else(|| {
            view.document
                .paragraph_text(view.insert_at.0)
                .and_then(|text| dominant(&text).map(str::to_string))
        })
        .unwrap_or_else(|| "ja-JP".to_string());
    view.views.dialog = Some(ViewDialog::Language(LanguageChoice {
        language,
        no_proof: false,
        detect: false,
    }));
    cx.notify();
}

/// OK: the choice on the selection, or on the caret's paragraph.
pub fn confirm(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let Some(ViewDialog::Language(choice)) = view.views.dialog.take() else {
        return;
    };
    let (start, end) = match view.document.selection() {
        Some((anchor, focus)) => (anchor.min(focus), anchor.max(focus)),
        None => {
            let paragraph = view.insert_at.0;
            let length = view
                .document
                .paragraph_text(paragraph)
                .map_or(0, |text| text.len());
            ((paragraph, 0), (paragraph, length))
        }
    };
    match view.document.set_language(start, end, &choice) {
        Ok(()) => {
            view.dirty = true;
            view.rebuild_all_images();
        }
        Err(error) => view.table_status = Some(SharedString::from(error)),
    }
    cx.notify();
}

fn edit(
    view: &mut DocumentView,
    cx: &mut Context<DocumentView>,
    f: impl FnOnce(&mut LanguageChoice),
) {
    if let Some(ViewDialog::Language(choice)) = view.views.dialog.as_mut() {
        f(choice);
        cx.notify();
    }
}

/// The dialog.
pub fn overlay(choice: &LanguageChoice, cx: &mut Context<DocumentView>) -> AnyElement {
    let cx: &Context<DocumentView> = cx;
    let languages = LANGUAGES.iter().enumerate().map(|(index, (tag, _, key))| {
        let tag: &'static str = tag;
        chip(
            ("language", index),
            t(key),
            choice.language == tag && !choice.detect,
        )
        .on_click(on(cx, move |view, cx| {
            edit(view, cx, |choice| {
                choice.language = tag.to_string();
                choice.detect = false;
            })
        }))
        .into_any_element()
    });
    let body = div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_color(theme::text_dim())
                .child(t("language.mark_as")),
        )
        .child(div().flex().flex_wrap().gap_1().children(languages))
        .child(
            check("language-no-proof", t("language.no_proof"), choice.no_proof)
                .on_click(on(cx, |view, cx| {
                    edit(view, cx, |choice| choice.no_proof = !choice.no_proof)
                })),
        )
        .child(
            check("language-detect", t("language.detect"), choice.detect)
                .on_click(on(cx, |view, cx| {
                    edit(view, cx, |choice| choice.detect = !choice.detect)
                })),
        );
    dialog_frame_with(
        "language-dialog",
        t("language.title"),
        460.0,
        body.into_any_element(),
        cx,
        confirm,
    )
}

/// 校閲's 言語 group.
pub fn review_group(cx: &mut Context<DocumentView>) -> AnyElement {
    group(
        t("language.group"),
        vec![row(vec![div()
            .id("review-language")
            .flex()
            .items_center()
            .h(gpui::px(26.0))
            .px_2()
            .rounded_md()
            .text_xs()
            .whitespace_nowrap()
            .cursor_pointer()
            .hover(|button| button.bg(theme::hovered()))
            .on_click(on(cx, open))
            .child(t("language.button"))
            .into_any_element()])],
    )
}

/// The status bar's language: the caret's text's.
pub fn status(view: &DocumentView) -> Option<SharedString> {
    let tag = view.document.language_at(view.insert_at).or_else(|| {
        view.document
            .paragraph_text(view.insert_at.0)
            .and_then(|text| dominant(&text).map(str::to_string))
    })?;
    let key = LANGUAGES
        .iter()
        .find(|(each, _, _)| each.eq_ignore_ascii_case(&tag))
        .map(|(_, _, key)| *key);
    Some(match key {
        Some(key) => t(key).into(),
        None => tag.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The dialog marks the selection in English, which the status bar
    /// then says.
    #[gpui::test]
    fn the_dialog_marks_the_selection(cx: &mut gpui::TestAppContext) {
        let (view, cx) = crate::word_views::tests::open_contract(cx);
        view.update(cx, |view, cx| {
            let paragraph = view.insert_at.0;
            let length = view.current_paragraph_text().len();
            view.document
                .select((paragraph, 0), (paragraph, length), 1)
                .unwrap();
            open(view, cx);
            edit(view, cx, |choice| choice.language = "en-GB".to_string());
            confirm(view, cx);
            assert!(view.views.dialog.is_none());
            let marked = view
                .document
                .word_document()
                .paragraph(paragraph)
                .unwrap()
                .runs
                .iter()
                .any(|run| {
                    run.properties
                        .as_ref()
                        .and_then(|props| props.language.as_deref())
                        == Some("en-GB")
                });
            assert!(marked, "the runs are marked in English (UK)");
            assert!(view.dirty);
        });
    }
}

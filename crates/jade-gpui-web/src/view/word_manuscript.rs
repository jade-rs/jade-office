//! レイアウト › 原稿用紙設定 in the browser build: the browser's copy of the
//! native app's `word_manuscript`, over the same model (`jade-office-model`'s
//! `word::manuscript`).

use gpui::{div, prelude::*, px, AnyElement, ClickEvent, Context};
use jade_web::word::manuscript::{
    manuscript_sizes, ManuscriptDialog, WordManuscript, MANUSCRIPT_COLORS, MANUSCRIPT_PAPERS,
    MANUSCRIPT_STYLES,
};

use super::word_dialog::{chip, labeled, on, DialogError, Inputs, WordDialog};
use super::{DocView as DocumentView, Document};
use crate::i18n;
use crate::palette as theme;
use crate::ribbon::{group, row};
use crate::tooltip::Tooltip;

/// The dialog's state: the model's dialog (it has no text fields).
pub struct ManuscriptDialogState {
    pub dialog: ManuscriptDialog,
    pub inputs: Inputs,
}

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

fn hex(color: &str) -> u32 {
    u32::from_str_radix(color.trim_start_matches('#'), 16).unwrap_or(0)
}

/// レイアウト's 原稿用紙 group: the 原稿用紙設定 button.
pub fn layout_group(cx: &mut Context<DocumentView>) -> AnyElement {
    let button = div()
        .id("manuscript-button")
        .flex()
        .flex_none()
        .items_center()
        .h(px(26.0))
        .px_2()
        .rounded_md()
        .cursor_pointer()
        .text_xs()
        .text_color(theme::text())
        .whitespace_nowrap()
        .hover(|button| button.bg(theme::hovered()))
        .tooltip(Tooltip::text(t("manuscript.title")))
        .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
            if let Document::Pages { ribbon, .. } = &mut view.document {
                ribbon.popup = None;
            }
            view.open_manuscript_dialog(cx);
        }))
        .child(t("manuscript.button"))
        .into_any_element();
    group(t("manuscript.group"), vec![row(vec![button])])
}

impl DocumentView {
    /// 原稿用紙設定…
    pub(crate) fn open_manuscript_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.word_ref().map(|document| document.manuscript_dialog()) else {
            return;
        };
        self.tools.dialog = Some(WordDialog::Manuscript(ManuscriptDialogState {
            dialog,
            inputs: Inputs::new(Vec::new()),
        }));
        cx.notify();
    }

    fn edit_manuscript(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut ManuscriptDialog),
    ) {
        self.edit_dialog(cx, |dialog| {
            if let WordDialog::Manuscript(state) = dialog {
                edit(&mut state.dialog);
            }
        })
    }

    pub(crate) fn manuscript_dialog_body(
        &self,
        state: &ManuscriptDialogState,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let dialog = &state.dialog;
        let styles = MANUSCRIPT_STYLES
            .into_iter()
            .enumerate()
            .map(|(index, (style, label))| {
                chip(("manuscript-style", index), t(label), dialog.style == style)
                    .on_click(on(cx, move |view, cx| {
                        view.edit_manuscript(cx, |dialog| dialog.set_style(style))
                    }))
            })
            .collect::<Vec<_>>();
        let sizes = manuscript_sizes(dialog.style)
            .iter()
            .copied()
            .enumerate()
            .map(|(index, (chars, lines))| {
                chip(
                    ("manuscript-size", index),
                    format!("{chars} × {lines}"),
                    (dialog.chars, dialog.lines) == (chars, lines),
                )
                .on_click(on(cx, move |view, cx| {
                    view.edit_manuscript(cx, |dialog| {
                        dialog.chars = chars;
                        dialog.lines = lines;
                    })
                }))
            })
            .collect::<Vec<_>>();
        let colors = MANUSCRIPT_COLORS
            .into_iter()
            .enumerate()
            .map(|(index, (color, label))| {
                div()
                    .id(("manuscript-color", index))
                    .size(px(18.0))
                    .rounded_sm()
                    .border_1()
                    .border_color(if dialog.color.eq_ignore_ascii_case(color) {
                        theme::accent()
                    } else {
                        theme::border()
                    })
                    .bg(gpui::rgb(hex(color)))
                    .cursor_pointer()
                    .tooltip(Tooltip::text(t(label)))
                    .on_click(on(cx, move |view, cx| {
                        view.edit_manuscript(cx, |dialog| dialog.color = color.to_string())
                    }))
            })
            .collect::<Vec<_>>();
        let papers = MANUSCRIPT_PAPERS
            .into_iter()
            .enumerate()
            .map(|(index, paper)| {
                chip(
                    ("manuscript-paper", index),
                    paper.name(),
                    dialog.paper == paper,
                )
                .on_click(on(cx, move |view, cx| {
                    view.edit_manuscript(cx, |dialog| dialog.paper = paper)
                }))
            })
            .collect::<Vec<_>>();
        let orientations = [
            (false, "manuscript.portrait"),
            (true, "manuscript.landscape"),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (landscape, label))| {
            chip(
                ("manuscript-orientation", index),
                t(label),
                dialog.landscape == landscape,
            )
            .on_click(on(cx, move |view, cx| {
                view.edit_manuscript(cx, |dialog| dialog.landscape = landscape)
            }))
        })
        .collect::<Vec<_>>();
        let enabled = dialog.style.is_some();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(labeled(t("manuscript.style")).children(styles))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .when(!enabled, |options| options.opacity(0.5))
                    .child(labeled(t("manuscript.size")).children(sizes))
                    .child(labeled(t("manuscript.color")).children(colors))
                    .child(labeled(t("manuscript.paper")).children(papers))
                    .child(labeled(t("manuscript.orientation")).children(orientations)),
            )
            .into_any_element()
    }

    pub(crate) fn apply_manuscript_dialog(
        &mut self,
        state: ManuscriptDialogState,
    ) -> Result<(), DialogError> {
        self.doc_mut()
            .apply_manuscript_dialog(&state.dialog)
            .map_err(DialogError::Failed)
    }
}

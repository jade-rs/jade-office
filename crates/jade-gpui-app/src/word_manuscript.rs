//! レイアウト › 原稿用紙 › 原稿用紙設定: the dialog over manuscript paper —
//! スタイル (なし / マス目付き / 下線付き / 外枠付き), 文字数 × 行数, 罫線の色,
//! 用紙サイズ and 印刷の向き. What it does is `jade-office-model`'s
//! `word::manuscript`, which the browser shell draws too.

use gpui::{div, prelude::*, px, AnyElement, ClickEvent, Context};
use jade_office_model::word::manuscript::{
    manuscript_sizes, ManuscriptDialog, WordManuscript, MANUSCRIPT_COLORS, MANUSCRIPT_PAPERS,
    MANUSCRIPT_STYLES,
};

use crate::tooltip::Tooltip;
use crate::word_dialog::{chip, labeled, on, DialogError, Inputs, WordDialog};
use crate::{i18n, theme, DocumentView};

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
pub(crate) fn layout_group(cx: &mut Context<DocumentView>) -> AnyElement {
    let button = div()
        .id("manuscript-button")
        .flex()
        .flex_none()
        .items_center()
        .h(px(28.0))
        .px_2()
        .rounded_md()
        .cursor_pointer()
        .text_sm()
        .text_color(theme::text())
        .hover(|button| button.bg(theme::hovered()))
        .tooltip(Tooltip::text(t("manuscript.title")))
        .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
            view.ribbon.popup = None;
            view.open_manuscript_dialog(cx);
        }))
        .child(t("manuscript.button"))
        .into_any_element();
    DocumentView::ribbon_group(
        t("manuscript.group"),
        vec![DocumentView::ribbon_row(vec![button])],
    )
    .into_any_element()
}

impl DocumentView {
    /// 原稿用紙設定…
    pub(crate) fn open_manuscript_dialog(&mut self, cx: &mut Context<Self>) {
        let dialog = self.document.manuscript_dialog();
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
        self.document
            .apply_manuscript_dialog(&state.dialog)
            .map_err(DialogError::Failed)
    }
}

#[cfg(test)]
mod tests {
    use crate::word_dialog::WordDialog;
    use jade_office_model::word::manuscript::{ManuscriptStyle, WordManuscript};
    use jade_office_model::word::WordSession;

    fn open_contract(
        cx: &mut gpui::TestAppContext,
    ) -> (
        gpui::Entity<crate::DocumentView>,
        &mut gpui::VisualTestContext,
    ) {
        use gpui::Focusable;
        let path = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../jade-core/contract.docx"
        ));
        let (view, cx) =
            cx.add_window_view(|_window, cx| crate::open_document_view(&path, cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        (view, cx)
    }

    /// レイアウト › 原稿用紙設定: マス目付き 20 × 20 lays the text on the
    /// grid as one undo step, the dialog reads it back, and なし takes it
    /// away.
    #[gpui::test]
    fn manuscript_paper_is_set_from_the_layout_tab(cx: &mut gpui::TestAppContext) {
        use crate::Undo;
        let (view, cx) = open_contract(cx);
        view.update(cx, |view, cx| {
            assert!(view.document.manuscript_grid().is_none());
            view.open_manuscript_dialog(cx);
            let Some(WordDialog::Manuscript(state)) = view.tools.dialog.as_mut() else {
                panic!("the 原稿用紙設定 dialog");
            };
            assert_eq!(state.dialog.style, None);
            state.dialog.set_style(Some(ManuscriptStyle::Squares));
            state.dialog.color = "#00B050".into();
            view.dialog_ok(cx);
            assert!(view.tools.dialog.is_none());
            let grid = view.document.manuscript_grid().expect("a grid");
            assert_eq!(grid.style, ManuscriptStyle::Squares);
            assert_eq!((grid.chars, grid.lines), (20, 20));
            assert_eq!(grid.color, "#00B050");
            let layout = view.document.word_document().section_layout(None);
            assert_eq!(layout.doc_grid.grid_type, "snapToChars");
        });
        cx.dispatch_action(Undo);
        view.update(cx, |view, cx| {
            assert!(
                view.document.manuscript_grid().is_none(),
                "undo takes it away"
            );
            let layout = view.document.word_document().section_layout(None);
            assert_ne!(layout.doc_grid.grid_type, "snapToChars");

            view.open_manuscript_dialog(cx);
            let Some(WordDialog::Manuscript(state)) = view.tools.dialog.as_mut() else {
                unreachable!()
            };
            state.dialog.set_style(Some(ManuscriptStyle::Lines));
            view.dialog_ok(cx);
            view.open_manuscript_dialog(cx);
            let Some(WordDialog::Manuscript(state)) = view.tools.dialog.as_mut() else {
                unreachable!()
            };
            assert_eq!(
                state.dialog.style,
                Some(ManuscriptStyle::Lines),
                "it reads it back"
            );
            state.dialog.set_style(None);
            view.dialog_ok(cx);
            assert!(view.document.manuscript_grid().is_none());
            let layout = view.document.word_document().section_layout(None);
            assert_eq!(layout.doc_grid.grid_type, "lines");
        });
    }
}

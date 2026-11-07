//! ホーム › 均等割り付け with text selected inside a paragraph: the
//! 文字の均等割り付け dialog — 現在の文字列の幅, 新しい文字列の幅 (字) and
//! 解除 — over `jade-office-model`'s `word::fit_text`. With no text
//! selected the button stays the paragraph's 均等割り付け.

use gpui::{div, prelude::*, AnyElement, Context};
use jade_office_model::word::fit_text::{characters_label, FitTextDialog, WordFitText};

use crate::word_dialog::{chip, labeled, on, DialogError, Inputs, WordDialog};
use crate::{i18n, theme, DocumentView};

/// The dialog's state: the model's dialog and its one field (新しい文字列の幅).
pub struct FitTextDialogState {
    pub dialog: FitTextDialog,
    pub inputs: Inputs,
}

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

impl DocumentView {
    /// 均等割り付け: the dialog when text inside one paragraph is selected.
    /// `false` leaves the button to the paragraph's alignment.
    pub(crate) fn open_fit_text_dialog(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(dialog) = self.document.fit_text_dialog() else {
            return false;
        };
        let inputs = Inputs::new(vec![dialog.new_width.clone()]);
        self.tools.dialog = Some(WordDialog::FitText(FitTextDialogState { dialog, inputs }));
        cx.notify();
        true
    }

    /// 解除: the selection back to its own width, closing the dialog.
    pub(crate) fn remove_fit_text_from_dialog(&mut self, cx: &mut Context<Self>) {
        match self.tools.dialog.take() {
            Some(WordDialog::FitText(state)) if state.dialog.fitted => {
                let result = self.document.remove_fit_text(&state.dialog);
                self.tool_done(result, cx);
            }
            other => self.tools.dialog = other,
        }
        cx.notify();
    }

    pub(crate) fn fit_text_dialog_body(
        &self,
        state: &FitTextDialogState,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let dialog = &state.dialog;
        let remove = chip("fit-text-remove", t("fit_text.remove"), false)
            .when(!dialog.fitted, |button| button.opacity(0.5))
            .on_click(on(cx, |view, cx| view.remove_fit_text_from_dialog(cx)));
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(labeled(t("fit_text.current")).child(format!(
                "{} {}",
                characters_label(dialog.current),
                t("fit_text.unit")
            )))
            .child(
                labeled(t("fit_text.new"))
                    .child(self.dialog_field_element(0, 70.0, cx))
                    .child(t("fit_text.unit")),
            )
            .child(
                div().flex().items_center().gap_2().child(remove).child(
                    div()
                        .text_color(theme::text_faint())
                        .child(dialog.text.clone()),
                ),
            )
            .into_any_element()
    }

    pub(crate) fn apply_fit_text_dialog(
        &mut self,
        mut state: FitTextDialogState,
    ) -> Result<(), DialogError> {
        state.dialog.new_width = state.inputs.text(0).to_string();
        if let Err(key) = state.dialog.command() {
            return Err(DialogError::Stay(
                Box::new(WordDialog::FitText(state)),
                t(key).to_string(),
            ));
        }
        self.document
            .apply_fit_text(&state.dialog)
            .map_err(DialogError::Failed)
    }
}

#[cfg(test)]
mod tests {
    use crate::ribbon::Action;
    use crate::word_dialog::WordDialog;
    use jade_office_model::word::fields::{SelectionRange, TextPosition};
    use jade_office_model::word::fit_text::WordFitText;
    use jade_office_model::word::WordSession;

    fn paragraph_text(
        document: &jade_docx_editor::word_document::WordDocument,
        index: usize,
    ) -> Option<String> {
        let paragraph = document.paragraph(index)?;
        Some(paragraph.runs.iter().map(|run| run.text.as_str()).collect())
    }

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

    /// ホーム › 均等割り付け: with two characters selected it opens
    /// 文字の均等割り付け, which spreads them over four 字 as one undo step,
    /// reads the width back and takes it off with 解除; with a bare caret
    /// it is the paragraph's 均等割り付け.
    #[gpui::test]
    fn fit_text_opens_on_a_selection_and_distributes_a_bare_caret(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_contract(cx);
        view.update(cx, |view, cx| {
            let paragraph = (0..view.document.word_document().paragraph_count())
                .find(|&index| {
                    paragraph_text(view.document.word_document(), index)
                        .is_some_and(|text| text.trim().chars().count() >= 4)
                })
                .expect("a paragraph with text");
            let text = paragraph_text(view.document.word_document(), paragraph).unwrap();
            let two: usize = text.chars().take(2).map(char::len_utf8).sum();
            view.document.set_selection_range(Some(SelectionRange::new(
                TextPosition::new(paragraph, 0),
                TextPosition::new(paragraph, two),
            )));
            view.run_ribbon(Action::Align("distribute"), cx);
            let Some(WordDialog::FitText(state)) = view.tools.dialog.as_mut() else {
                panic!("the 文字の均等割り付け dialog");
            };
            assert!(!state.dialog.fitted);
            state.inputs.set(0, "oops".into());
            view.dialog_ok(cx);
            let Some(WordDialog::FitText(state)) = view.tools.dialog.as_mut() else {
                panic!("a width it cannot take keeps the dialog open");
            };
            state.inputs.set(0, "4".into());
            view.dialog_ok(cx);
            assert!(view.tools.dialog.is_none());
            view.document.set_selection_range(Some(SelectionRange::new(
                TextPosition::new(paragraph, 0),
                TextPosition::new(paragraph, two),
            )));
            let dialog = view.document.fit_text_dialog().expect("still selected");
            assert!(dialog.fitted);
            assert!((dialog.current - 4.0).abs() < 0.01, "{}", dialog.current);

            // 解除.
            view.run_ribbon(Action::Align("distribute"), cx);
            view.remove_fit_text_from_dialog(cx);
            assert!(view.tools.dialog.is_none());
            view.document.set_selection_range(Some(SelectionRange::new(
                TextPosition::new(paragraph, 0),
                TextPosition::new(paragraph, two),
            )));
            assert!(!view.document.fit_text_dialog().unwrap().fitted);

            // A bare caret: the paragraph's alignment, no dialog.
            view.insert_at = (paragraph, 0);
            view.caret = view.document.place_caret(paragraph, 0);
            view.run_ribbon(Action::Align("distribute"), cx);
            assert!(view.tools.dialog.is_none());
            assert_eq!(
                jade_office_model::word::WordFormatting::format_state(&view.document)
                    .alignment
                    .as_deref(),
                Some("distribute")
            );
        });
    }
}

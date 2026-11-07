//! 文字の均等割り付け in the browser build: the browser's copy of the native
//! app's `word_fit_text`, over the same model (`jade-office-model`'s
//! `word::fit_text`).

use gpui::{div, prelude::*, AnyElement, Context};
use jade_web::word::fit_text::{characters_label, FitTextDialog, WordFitText};

use super::word_dialog::{chip, labeled, on, DialogError, Inputs, WordDialog};
use super::DocView as DocumentView;
use crate::i18n;
use crate::palette as theme;

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
        let Some(dialog) = self
            .word_ref()
            .and_then(|document| document.fit_text_dialog())
        else {
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
                let result = self.doc_mut().remove_fit_text(&state.dialog);
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
        self.doc_mut()
            .apply_fit_text(&state.dialog)
            .map_err(DialogError::Failed)
    }
}

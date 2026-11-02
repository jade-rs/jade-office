use super::WorkbookView;
use crate::{i18n, theme};
use gpui::{div, prelude::*, px, Context, MouseButton};
use jade_office_model::calculation_options::{
    CalculationField, CalculationKey, CalculationOptionsDialog,
};
type View = WorkbookView;
include!("calculation_options_ui.rs");
impl WorkbookView {
    fn calculation_options_editor(&self) -> Option<&CalculationOptionsDialog> {
        self.calculation_options.as_ref()
    }
    /// Formulas ▸ Calculation Options…, from the cell menu.
    pub(super) fn open_calculation_options(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        self.calculation_options = Some(CalculationOptionsDialog::new(
            self.book.calculation_options(),
        ));
        self.menu = None;
        cx.notify();
    }
    pub(super) fn calculation_options_action(
        &mut self,
        action: CalculationAction,
        cx: &mut Context<Self>,
    ) {
        match action {
            CalculationAction::Take => {
                let Some(dialog) = self.calculation_options.take() else {
                    return;
                };
                let result = dialog.apply(&mut self.book);
                self.after_write(result, cx);
            }
            CalculationAction::Cancel => {
                self.calculation_options = None;
                cx.notify();
            }
            change => {
                if let Some(dialog) = self.calculation_options.as_mut() {
                    change_calculation_options(dialog, change);
                }
                cx.notify();
            }
        }
    }
    pub(super) fn calculation_options_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let Some(dialog) = self.calculation_options.as_mut() else {
            return false;
        };
        match dialog.key(key) {
            CalculationKey::Take => self.calculation_options_action(CalculationAction::Take, cx),
            CalculationKey::Close => self.calculation_options_action(CalculationAction::Cancel, cx),
            CalculationKey::Handled => cx.notify(),
            CalculationKey::Unhandled => {}
        }
        // Nothing else reaches the sheet while the dialog is open.
        true
    }
}

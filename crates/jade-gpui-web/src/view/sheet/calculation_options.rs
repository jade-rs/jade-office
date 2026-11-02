use super::super::DocView;
use crate::{i18n, palette as theme};
use gpui::{div, prelude::*, px, Context, MouseButton};
use jade_web::calculation_options::{CalculationField, CalculationKey, CalculationOptionsDialog};
type View = DocView;
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../jade-gpui-app/src/workbook/calculation_options_ui.rs"
));
impl DocView {
    fn calculation_options_editor(&self) -> Option<&CalculationOptionsDialog> {
        self.sheet_ref()?.calculation_options.as_ref()
    }
    /// Formulas ▸ Calculation Options…, from the cell menu.
    pub(super) fn open_calculation_options(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        if let Some(sheet) = self.sheet_mut() {
            sheet.calculation_options = Some(CalculationOptionsDialog::new(
                sheet.book.calculation_options(),
            ));
            sheet.menu = None;
        }
        cx.notify();
    }
    pub(super) fn calculation_options_action(
        &mut self,
        action: CalculationAction,
        cx: &mut Context<Self>,
    ) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        match action {
            CalculationAction::Take => {
                let Some(dialog) = sheet.calculation_options.take() else {
                    return;
                };
                let result = dialog.apply(&mut sheet.book);
                self.edited(result, cx);
            }
            CalculationAction::Cancel => {
                sheet.calculation_options = None;
                cx.notify();
            }
            change => {
                if let Some(dialog) = sheet.calculation_options.as_mut() {
                    change_calculation_options(dialog, change);
                }
                cx.notify();
            }
        }
    }
    pub(super) fn calculation_options_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let Some(dialog) = self
            .sheet_mut()
            .and_then(|sheet| sheet.calculation_options.as_mut())
        else {
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

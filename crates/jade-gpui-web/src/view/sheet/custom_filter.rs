use super::super::DocView;
use crate::{i18n, palette as theme};
use gpui::{div, prelude::*, px, Context, Modifiers, MouseButton};
use jade_web::custom_filter::{CustomFilterDialog, CustomFilterKey, FilterTest};
type View = DocView;
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../jade-gpui-app/src/workbook/custom_filter_ui.rs"
));
impl DocView {
    fn custom_filter_editor(&self) -> Option<&CustomFilterDialog> {
        self.sheet_ref()?.custom_filter.as_ref()
    }
    /// Custom Filter… from a filter button's dropdown.
    pub(super) fn open_custom_filter(&mut self, cx: &mut Context<Self>) {
        if let Some(sheet) = self.sheet_mut() {
            if let Some(panel) = sheet.filter.take() {
                sheet.custom_filter = Some(CustomFilterDialog::new(panel.button));
            }
        }
        cx.notify();
    }
    pub(super) fn custom_filter_action(
        &mut self,
        action: CustomFilterAction,
        cx: &mut Context<Self>,
    ) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        match action {
            CustomFilterAction::Test(test) => {
                if let Some(dialog) = sheet.custom_filter.as_mut() {
                    dialog.test = test;
                }
                cx.notify();
            }
            CustomFilterAction::Take => {
                let Some(dialog) = sheet.custom_filter.take() else {
                    return;
                };
                let result =
                    sheet
                        .book
                        .filter_custom(&dialog.button, dialog.test, &dialog.value.text);
                self.edited(result, cx);
            }
            CustomFilterAction::Cancel => {
                sheet.custom_filter = None;
                cx.notify();
            }
        }
    }
    pub(super) fn custom_filter_key(
        &mut self,
        key: &str,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) -> bool {
        let command = modifiers.platform || modifiers.control;
        let Some(dialog) = self
            .sheet_mut()
            .and_then(|sheet| sheet.custom_filter.as_mut())
        else {
            return false;
        };
        match dialog.key(key, command) {
            CustomFilterKey::Take => self.custom_filter_action(CustomFilterAction::Take, cx),
            CustomFilterKey::Close => self.custom_filter_action(CustomFilterAction::Cancel, cx),
            CustomFilterKey::Handled => cx.notify(),
            CustomFilterKey::Unhandled => return false,
        }
        true
    }
}

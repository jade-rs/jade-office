use super::WorkbookView;
use crate::{i18n, theme};
use gpui::{div, prelude::*, px, Context, Modifiers, MouseButton};
use jade_office_model::custom_filter::{CustomFilterDialog, CustomFilterKey, FilterTest};
type View = WorkbookView;
include!("custom_filter_ui.rs");
impl WorkbookView {
    fn custom_filter_editor(&self) -> Option<&CustomFilterDialog> {
        self.custom_filter.as_ref()
    }
    /// Custom Filter… from a filter button's dropdown.
    pub(super) fn open_custom_filter(&mut self, cx: &mut Context<Self>) {
        if let Some(panel) = self.filter.take() {
            self.custom_filter = Some(CustomFilterDialog::new(panel.button));
        }
        cx.notify();
    }
    pub(super) fn custom_filter_action(
        &mut self,
        action: CustomFilterAction,
        cx: &mut Context<Self>,
    ) {
        match action {
            CustomFilterAction::Test(second, test) => {
                if let Some(dialog) = self.custom_filter.as_mut() {
                    if second {
                        dialog.second_test = test;
                    } else {
                        dialog.test = test;
                    }
                }
                cx.notify();
            }
            CustomFilterAction::And(and) => {
                if let Some(dialog) = self.custom_filter.as_mut() {
                    dialog.and = and;
                }
                cx.notify();
            }
            CustomFilterAction::Field(second) => {
                if let Some(dialog) = self.custom_filter.as_mut() {
                    dialog.in_second = second;
                }
                cx.notify();
            }
            CustomFilterAction::Take => {
                let Some(dialog) = self.custom_filter.take() else {
                    return;
                };
                let result = dialog.apply(&mut self.book);
                self.after_write(result, cx);
            }
            CustomFilterAction::Cancel => {
                self.custom_filter = None;
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
        let Some(dialog) = self.custom_filter.as_mut() else {
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

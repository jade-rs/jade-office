use super::WorkbookView;
use crate::{i18n, theme};
use gpui::{div, prelude::*, px, Context, MouseButton};
use jade_office_model::top_ten::{TopTenDialog, TopTenKey};
type View = WorkbookView;
include!("top_ten_ui.rs");
impl WorkbookView {
    fn top_ten_editor(&self) -> Option<&TopTenDialog> {
        self.top_ten.as_ref()
    }
    /// トップテン… from a filter button's dropdown.
    pub(super) fn open_top_ten(&mut self, cx: &mut Context<Self>) {
        if let Some(panel) = self.filter.take() {
            self.top_ten = Some(TopTenDialog::new(panel.button));
        }
        cx.notify();
    }
    pub(super) fn top_ten_action(&mut self, action: TopTenAction, cx: &mut Context<Self>) {
        match action {
            TopTenAction::Take => {
                let Some(dialog) = self.top_ten.take() else {
                    return;
                };
                let result = dialog.apply(&mut self.book);
                self.after_write(result, cx);
            }
            TopTenAction::Cancel => {
                self.top_ten = None;
                cx.notify();
            }
            change => {
                if let Some(dialog) = self.top_ten.as_mut() {
                    change_top_ten(dialog, change);
                }
                cx.notify();
            }
        }
    }
    pub(super) fn top_ten_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let Some(dialog) = self.top_ten.as_mut() else {
            return false;
        };
        match dialog.key(key) {
            TopTenKey::Take => self.top_ten_action(TopTenAction::Take, cx),
            TopTenKey::Close => self.top_ten_action(TopTenAction::Cancel, cx),
            TopTenKey::Handled => cx.notify(),
            TopTenKey::Unhandled => {}
        }
        // Nothing else reaches the sheet while the dialog is open.
        true
    }
}

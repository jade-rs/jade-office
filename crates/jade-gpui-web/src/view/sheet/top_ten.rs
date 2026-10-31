use super::super::DocView;
use crate::{i18n, palette as theme};
use gpui::{div, prelude::*, px, Context, MouseButton};
use jade_web::top_ten::{TopTenDialog, TopTenKey};
type View = DocView;
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../jade-gpui-app/src/workbook/top_ten_ui.rs"
));
impl DocView {
    fn top_ten_editor(&self) -> Option<&TopTenDialog> {
        self.sheet_ref()?.top_ten.as_ref()
    }
    /// トップテン… from a filter button's dropdown.
    pub(super) fn open_top_ten(&mut self, cx: &mut Context<Self>) {
        if let Some(sheet) = self.sheet_mut() {
            if let Some(panel) = sheet.filter.take() {
                sheet.top_ten = Some(TopTenDialog::new(panel.button));
            }
        }
        cx.notify();
    }
    pub(super) fn top_ten_action(&mut self, action: TopTenAction, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        match action {
            TopTenAction::Take => {
                let Some(dialog) = sheet.top_ten.take() else {
                    return;
                };
                let result = dialog.apply(&mut sheet.book);
                self.edited(result, cx);
            }
            TopTenAction::Cancel => {
                sheet.top_ten = None;
                cx.notify();
            }
            change => {
                if let Some(dialog) = sheet.top_ten.as_mut() {
                    change_top_ten(dialog, change);
                }
                cx.notify();
            }
        }
    }
    pub(super) fn top_ten_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let Some(dialog) = self.sheet_mut().and_then(|sheet| sheet.top_ten.as_mut()) else {
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

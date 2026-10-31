use super::WorkbookView;
use crate::{i18n, theme};
use gpui::{div, prelude::*, px, Context, MouseButton};
use jade_office_model::text_columns::TextColumnsDialog;
type View = WorkbookView;
include!("text_columns_ui.rs");
impl WorkbookView {
    fn text_columns_editor(&self) -> Option<&TextColumnsDialog> {
        self.text_columns_dialog.as_ref()
    }
    pub(super) fn open_text_columns_dialog(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        self.text_columns_dialog = Some(TextColumnsDialog::open(
            &self.book,
            self.range(),
            self.active(),
        ));
        self.menu = None;
        cx.notify();
    }
    pub(super) fn text_columns_action(
        &mut self,
        action: TextColumnsAction,
        cx: &mut Context<Self>,
    ) {
        match action {
            TextColumnsAction::Take => {
                let Some(dialog) = self.text_columns_dialog.take() else {
                    return;
                };
                let result = self.book.text_to_columns(dialog.range, &dialog.split);
                self.after_write(result, cx);
            }
            TextColumnsAction::Cancel => {
                self.text_columns_dialog = None;
                cx.notify();
            }
            change => {
                if let Some(dialog) = self.text_columns_dialog.as_mut() {
                    change_text_columns_dialog(dialog, change);
                }
                cx.notify();
            }
        }
    }
    pub(super) fn text_columns_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        if self.text_columns_dialog.is_none() {
            return false;
        }
        match key {
            "enter" => self.text_columns_action(TextColumnsAction::Take, cx),
            "escape" => self.text_columns_action(TextColumnsAction::Cancel, cx),
            _ => {}
        }
        true
    }
}

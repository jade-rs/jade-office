use super::WorkbookView;
use crate::{i18n, theme};
use gpui::{div, prelude::*, px, Context, MouseButton};
use jade_office_model::subtotal::{SubtotalDialog, SubtotalFunction};
type View = WorkbookView;
include!("subtotal_ui.rs");
impl WorkbookView {
    fn subtotal_editor(&self) -> Option<&SubtotalDialog> {
        self.subtotal_dialog.as_ref()
    }
    pub(super) fn open_subtotal_dialog(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        self.subtotal_dialog = Some(SubtotalDialog::open(
            &self.book,
            self.range(),
            self.active(),
        ));
        self.menu = None;
        cx.notify();
    }
    pub(super) fn subtotal_action(&mut self, action: SubtotalAction, cx: &mut Context<Self>) {
        match action {
            SubtotalAction::Take => {
                let Some(dialog) = self.subtotal_dialog.as_ref() else {
                    return;
                };
                let (label, grand) = subtotal_labels(dialog);
                // Nothing ticked to total: OK waits for one.
                let Some(subtotals) = dialog.subtotals(label, grand) else {
                    cx.notify();
                    return;
                };
                let range = dialog.range;
                let result = self.book.subtotal(range, &subtotals);
                if result.is_ok() {
                    self.subtotal_dialog = None;
                }
                self.after_write(result, cx);
            }
            SubtotalAction::Cancel => {
                self.subtotal_dialog = None;
                cx.notify();
            }
            change => {
                if let Some(dialog) = self.subtotal_dialog.as_mut() {
                    change_subtotal_dialog(dialog, change);
                }
                cx.notify();
            }
        }
    }
    pub(super) fn subtotal_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        if self.subtotal_dialog.is_none() {
            return false;
        }
        match key {
            "enter" => self.subtotal_action(SubtotalAction::Take, cx),
            "escape" => self.subtotal_action(SubtotalAction::Cancel, cx),
            _ => {}
        }
        true
    }
}

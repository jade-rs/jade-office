use super::super::DocView;
use crate::{i18n, palette as theme};
use gpui::{div, prelude::*, px, Context, MouseButton};
use jade_web::subtotal::{SubtotalDialog, SubtotalFunction};
type View = DocView;
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../jade-gpui-app/src/workbook/subtotal_ui.rs"
));
impl DocView {
    fn subtotal_editor(&self) -> Option<&SubtotalDialog> {
        self.sheet_ref()?.subtotal_dialog.as_ref()
    }
    pub(super) fn open_subtotal_dialog(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        if let Some(s) = self.sheet_mut() {
            s.subtotal_dialog = Some(SubtotalDialog::open(
                &s.book,
                s.selection.range(),
                s.selection.active,
            ));
            s.menu = None;
        }
        cx.notify();
    }
    pub(super) fn subtotal_action(&mut self, action: SubtotalAction, cx: &mut Context<Self>) {
        let Some(s) = self.sheet_mut() else {
            return;
        };
        match action {
            SubtotalAction::Take => {
                let Some(dialog) = s.subtotal_dialog.as_ref() else {
                    return;
                };
                let (label, grand) = subtotal_labels(dialog);
                // Nothing ticked to total: OK waits for one.
                let Some(subtotals) = dialog.subtotals(label, grand) else {
                    cx.notify();
                    return;
                };
                let range = dialog.range;
                let result = s.book.subtotal(range, &subtotals);
                if result.is_ok() {
                    s.subtotal_dialog = None;
                }
                self.edited(result, cx);
            }
            SubtotalAction::Cancel => {
                s.subtotal_dialog = None;
                cx.notify();
            }
            change => {
                if let Some(dialog) = s.subtotal_dialog.as_mut() {
                    change_subtotal_dialog(dialog, change);
                }
                cx.notify();
            }
        }
    }
    pub(super) fn subtotal_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        if self.subtotal_editor().is_none() {
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

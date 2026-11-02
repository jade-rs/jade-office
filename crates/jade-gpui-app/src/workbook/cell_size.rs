use super::WorkbookView;
use crate::{i18n, theme};
use gpui::{div, prelude::*, px, Context, Modifiers, MouseButton};
use jade_office_model::cell_size::{SizeDialog, SizeKey};
use jade_office_model::sheet::Axis;
type View = WorkbookView;
include!("cell_size_ui.rs");
impl WorkbookView {
    fn size_editor(&self) -> Option<&SizeDialog> {
        self.size_dialog.as_ref()
    }
    /// Format ▸ Row Height… (rows) or Column Width… for the selection's
    /// rows or columns.
    pub(super) fn open_size_dialog(&mut self, axis: Axis, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        let range = self.range();
        let (first, last) = match axis {
            Axis::Rows => (range.start.row, range.end.row),
            Axis::Columns => (range.start.column, range.end.column),
        };
        self.menu = None;
        self.size_dialog = Some(SizeDialog::open(&self.book, axis, first, last));
        cx.notify();
    }
    pub(super) fn size_action(&mut self, action: SizeAction, cx: &mut Context<Self>) {
        match action {
            SizeAction::Take => {
                let Some(dialog) = self.size_dialog.as_mut() else {
                    return;
                };
                let Some(result) = dialog.apply(&mut self.book) else {
                    cx.notify();
                    return;
                };
                self.size_dialog = None;
                self.after_write(result, cx);
            }
            SizeAction::Cancel => {
                self.size_dialog = None;
                cx.notify();
            }
        }
    }
    pub(super) fn size_key(
        &mut self,
        key: &str,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) -> bool {
        let command = modifiers.platform || modifiers.control;
        let Some(dialog) = self.size_dialog.as_mut() else {
            return false;
        };
        match dialog.key(key, command) {
            SizeKey::Take => self.size_action(SizeAction::Take, cx),
            SizeKey::Close => self.size_action(SizeAction::Cancel, cx),
            SizeKey::Handled => cx.notify(),
            SizeKey::Unhandled => return false,
        }
        true
    }
}

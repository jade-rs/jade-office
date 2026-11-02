use super::super::DocView;
use crate::{i18n, palette as theme};
use gpui::{div, prelude::*, px, Context, Modifiers, MouseButton};
use jade_web::cell_size::{SizeDialog, SizeKey};
use jade_web::sheet::Axis;
type View = DocView;
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../jade-gpui-app/src/workbook/cell_size_ui.rs"
));
impl DocView {
    fn size_editor(&self) -> Option<&SizeDialog> {
        self.sheet_ref()?.size_dialog.as_ref()
    }
    /// Format ▸ Row Height… (rows) or Column Width… for the selection's
    /// rows or columns.
    pub(super) fn open_size_dialog(&mut self, axis: Axis, cx: &mut Context<Self>) {
        if let Some(sheet) = self.sheet_mut() {
            let range = sheet.selection.range();
            let (first, last) = match axis {
                Axis::Rows => (range.start.row, range.end.row),
                Axis::Columns => (range.start.column, range.end.column),
            };
            sheet.size_dialog = Some(SizeDialog::open(&sheet.book, axis, first, last));
        }
        cx.notify();
    }
    pub(super) fn size_action(&mut self, action: SizeAction, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        match action {
            SizeAction::Take => {
                let Some(dialog) = sheet.size_dialog.as_mut() else {
                    return;
                };
                let Some(result) = dialog.apply(&mut sheet.book) else {
                    cx.notify();
                    return;
                };
                sheet.size_dialog = None;
                self.edited(result, cx);
            }
            SizeAction::Cancel => {
                sheet.size_dialog = None;
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
        let Some(dialog) = self
            .sheet_mut()
            .and_then(|sheet| sheet.size_dialog.as_mut())
        else {
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

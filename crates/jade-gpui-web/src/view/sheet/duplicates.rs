use super::super::DocView;
use crate::{i18n, palette as theme};
use gpui::{div, prelude::*, px, Context, MouseButton};
use jade_web::duplicates::DuplicatesDialog;
type View = DocView;
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../jade-gpui-app/src/workbook/duplicates_ui.rs"
));
impl DocView {
    fn duplicates_editor(&self) -> Option<&DuplicatesDialog> {
        self.sheet_ref()?.duplicates_dialog.as_ref()
    }
    pub(super) fn open_duplicates_dialog(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        if let Some(s) = self.sheet_mut() {
            s.duplicates_dialog = Some(DuplicatesDialog::open(
                &s.book,
                s.selection.range(),
                s.selection.active,
            ));
            s.menu = None;
        }
        cx.notify();
    }
    pub(super) fn duplicates_action(&mut self, action: DuplicatesAction, cx: &mut Context<Self>) {
        let Some(s) = self.sheet_mut() else {
            return;
        };
        match action {
            DuplicatesAction::Take => {
                // OK does nothing until a column is ticked, as in Excel.
                if s.duplicates_dialog
                    .as_ref()
                    .is_none_or(|dialog| dialog.columns().is_empty())
                {
                    return;
                }
                let Some(dialog) = s.duplicates_dialog.take() else {
                    return;
                };
                self.remove_sheet_duplicates(dialog.range, &dialog.columns(), dialog.header, cx);
            }
            DuplicatesAction::Cancel => {
                s.duplicates_dialog = None;
                cx.notify();
            }
            change => {
                if let Some(dialog) = s.duplicates_dialog.as_mut() {
                    change_duplicates_dialog(dialog, change);
                }
                cx.notify();
            }
        }
    }
    pub(super) fn duplicates_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        if self.duplicates_editor().is_none() {
            return false;
        }
        match key {
            "enter" => self.duplicates_action(DuplicatesAction::Take, cx),
            "escape" => self.duplicates_action(DuplicatesAction::Cancel, cx),
            _ => {}
        }
        true
    }
}

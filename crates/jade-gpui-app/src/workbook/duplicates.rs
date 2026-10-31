use super::WorkbookView;
use crate::{i18n, theme};
use gpui::{div, prelude::*, px, Context, MouseButton};
use jade_office_model::duplicates::DuplicatesDialog;
type View = WorkbookView;
include!("duplicates_ui.rs");
impl WorkbookView {
    fn duplicates_editor(&self) -> Option<&DuplicatesDialog> {
        self.duplicates_dialog.as_ref()
    }
    pub(super) fn open_duplicates_dialog(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        self.duplicates_dialog = Some(DuplicatesDialog::open(
            &self.book,
            self.range(),
            self.active(),
        ));
        self.menu = None;
        cx.notify();
    }
    pub(super) fn duplicates_action(&mut self, action: DuplicatesAction, cx: &mut Context<Self>) {
        match action {
            DuplicatesAction::Take => {
                // OK does nothing until a column is ticked, as in Excel.
                if self
                    .duplicates_dialog
                    .as_ref()
                    .is_none_or(|dialog| dialog.columns().is_empty())
                {
                    return;
                }
                let Some(dialog) = self.duplicates_dialog.take() else {
                    return;
                };
                self.remove_duplicates(dialog.range, &dialog.columns(), dialog.header, cx);
            }
            DuplicatesAction::Cancel => {
                self.duplicates_dialog = None;
                cx.notify();
            }
            change => {
                if let Some(dialog) = self.duplicates_dialog.as_mut() {
                    change_duplicates_dialog(dialog, change);
                }
                cx.notify();
            }
        }
    }
    pub(super) fn duplicates_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        if self.duplicates_dialog.is_none() {
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

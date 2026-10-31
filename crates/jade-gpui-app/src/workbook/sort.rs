use super::WorkbookView;
use crate::{i18n, theme};
use gpui::{div, prelude::*, px, Context, MouseButton};
use jade_office_model::sort_dialog::SortDialog;
type View = WorkbookView;
include!("sort_ui.rs");
impl WorkbookView {
    fn sort_editor(&self) -> Option<&SortDialog> {
        self.sort_dialog.as_ref()
    }
    pub(super) fn open_sort_dialog(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        self.sort_dialog = Some(SortDialog::open(&self.book, self.range(), self.active()));
        self.menu = None;
        cx.notify();
    }
    pub(super) fn sort_action(&mut self, action: SortAction, cx: &mut Context<Self>) {
        match action {
            SortAction::Take => {
                let Some(dialog) = self.sort_dialog.take() else {
                    return;
                };
                let result = self.book.sort_by(
                    dialog.range,
                    dialog.header,
                    &dialog.keys(),
                    dialog.case_sensitive,
                );
                self.after_write(result, cx);
            }
            SortAction::Cancel => {
                self.sort_dialog = None;
                cx.notify();
            }
            change => {
                if let Some(dialog) = self.sort_dialog.as_mut() {
                    change_sort_dialog(dialog, change);
                }
                cx.notify();
            }
        }
    }
    pub(super) fn sort_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        if self.sort_dialog.is_none() {
            return false;
        }
        match key {
            "enter" => self.sort_action(SortAction::Take, cx),
            "escape" => self.sort_action(SortAction::Cancel, cx),
            _ => {}
        }
        true
    }
}

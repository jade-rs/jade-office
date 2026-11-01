use super::WorkbookView;
use crate::{i18n, theme};
use gpui::{div, prelude::*, px, Context, MouseButton};
use jade_office_model::page_setup_dialog::{
    MarginPreset, PageSetupDialog, PrintOption, FITS, PAPERS,
};
type View = WorkbookView;
include!("page_setup_ui.rs");
impl WorkbookView {
    fn page_setup_editor(&self) -> Option<&PageSetupDialog> {
        self.page_setup_dialog.as_ref()
    }
    pub(super) fn open_page_setup_dialog(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        self.page_setup_dialog = Some(PageSetupDialog::open(&self.book));
        self.menu = None;
        cx.notify();
    }
    pub(super) fn page_setup_action(&mut self, action: PageSetupAction, cx: &mut Context<Self>) {
        match action {
            PageSetupAction::Take => {
                let Some(dialog) = self.page_setup_dialog.take() else {
                    return;
                };
                let result = self.book.set_page_setup(&dialog.setup);
                self.after_write(result, cx);
            }
            PageSetupAction::Cancel => {
                self.page_setup_dialog = None;
                cx.notify();
            }
            change => {
                if let Some(dialog) = self.page_setup_dialog.as_mut() {
                    change_page_setup_dialog(dialog, change);
                }
                cx.notify();
            }
        }
    }
    pub(super) fn page_setup_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        if self.page_setup_dialog.is_none() {
            return false;
        }
        match key {
            "enter" => self.page_setup_action(PageSetupAction::Take, cx),
            "escape" => self.page_setup_action(PageSetupAction::Cancel, cx),
            _ => {}
        }
        true
    }
}

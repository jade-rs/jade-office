use super::super::DocView;
use crate::{i18n, palette as theme};
use gpui::{div, prelude::*, px, Context, MouseButton};
use jade_web::page_setup_dialog::{MarginPreset, PageSetupDialog, PrintOption, FITS, PAPERS};
type View = DocView;
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../jade-gpui-app/src/workbook/page_setup_ui.rs"
));
impl DocView {
    fn page_setup_editor(&self) -> Option<&PageSetupDialog> {
        self.sheet_ref()?.page_setup_dialog.as_ref()
    }
    pub(super) fn open_page_setup_dialog(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        if let Some(s) = self.sheet_mut() {
            s.page_setup_dialog = Some(PageSetupDialog::open(&s.book));
            s.menu = None;
        }
        cx.notify();
    }
    pub(super) fn page_setup_action(&mut self, action: PageSetupAction, cx: &mut Context<Self>) {
        let Some(s) = self.sheet_mut() else {
            return;
        };
        match action {
            PageSetupAction::Take => {
                let Some(dialog) = s.page_setup_dialog.take() else {
                    return;
                };
                let result = s.book.set_page_setup(&dialog.setup);
                self.edited(result, cx);
            }
            PageSetupAction::Cancel => {
                s.page_setup_dialog = None;
                cx.notify();
            }
            change => {
                if let Some(dialog) = s.page_setup_dialog.as_mut() {
                    change_page_setup_dialog(dialog, change);
                }
                cx.notify();
            }
        }
    }
    pub(super) fn page_setup_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        if self.page_setup_editor().is_none() {
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

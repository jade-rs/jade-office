use super::super::DocView;
use crate::{i18n, palette as theme};
use gpui::{div, prelude::*, px, Context, MouseButton};
use jade_web::sort_dialog::SortDialog;
type View = DocView;
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../jade-gpui-app/src/workbook/sort_ui.rs"
));
impl DocView {
    fn sort_editor(&self) -> Option<&SortDialog> {
        self.sheet_ref()?.sort_dialog.as_ref()
    }
    pub(super) fn open_sort_dialog(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        if let Some(s) = self.sheet_mut() {
            s.sort_dialog = Some(SortDialog::open(
                &s.book,
                s.selection.range(),
                s.selection.active,
            ));
            s.menu = None;
        }
        cx.notify();
    }
    pub(super) fn sort_action(&mut self, action: SortAction, cx: &mut Context<Self>) {
        let Some(s) = self.sheet_mut() else {
            return;
        };
        match action {
            SortAction::Take => {
                let Some(dialog) = s.sort_dialog.take() else {
                    return;
                };
                let result = s.book.sort_by(
                    dialog.range,
                    dialog.header,
                    &dialog.keys(),
                    dialog.case_sensitive,
                );
                self.edited(result, cx);
            }
            SortAction::Cancel => {
                s.sort_dialog = None;
                cx.notify();
            }
            change => {
                if let Some(dialog) = s.sort_dialog.as_mut() {
                    change_sort_dialog(dialog, change);
                }
                cx.notify();
            }
        }
    }
    pub(super) fn sort_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        if self.sort_editor().is_none() {
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

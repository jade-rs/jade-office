use super::super::DocView;
use crate::{i18n, palette as theme};
use gpui::{div, prelude::*, px, Context, MouseButton};
use jade_web::text_columns::{ColumnFormat, TextColumnsDialog};
type View = DocView;
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../jade-gpui-app/src/workbook/text_columns_ui.rs"
));
impl DocView {
    fn text_columns_editor(&self) -> Option<&TextColumnsDialog> {
        self.sheet_ref()?.text_columns_dialog.as_ref()
    }
    pub(super) fn open_text_columns_dialog(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        if let Some(s) = self.sheet_mut() {
            s.text_columns_dialog = Some(TextColumnsDialog::open(
                &s.book,
                s.selection.range(),
                s.selection.active,
            ));
            s.menu = None;
        }
        cx.notify();
    }
    /// Data ▸ From Text/CSV: the page's picker opened for a text file,
    /// which comes back through the workspace to `take_text_import`.
    pub(super) fn open_text_import(&mut self, cx: &mut Context<Self>) {
        if let Some(s) = self.sheet_mut() {
            s.awaiting_text = true;
            s.menu = None;
        }
        self.host.pick();
        cx.notify();
    }
    /// Whether the page's picker was opened for From Text, and `name` is a
    /// text file it can take.
    pub fn wants_text_import(&self, name: &str) -> bool {
        let text = name.rsplit_once('.').is_some_and(|(_, extension)| {
            ["txt", "csv", "tsv", "prn"]
                .iter()
                .any(|known| extension.eq_ignore_ascii_case(known))
        });
        text && self.sheet_ref().is_some_and(|s| s.awaiting_text)
    }
    /// The text file the page's picker brought, in the Text Import Wizard.
    pub fn take_text_import(&mut self, name: &str, bytes: &[u8], cx: &mut Context<Self>) {
        if let Some(s) = self.sheet_mut() {
            s.awaiting_text = false;
            s.text_columns_dialog = Some(TextColumnsDialog::open_import(
                name,
                bytes,
                s.selection.active,
            ));
        }
        cx.notify();
    }
    pub(super) fn text_columns_action(
        &mut self,
        action: TextColumnsAction,
        cx: &mut Context<Self>,
    ) {
        let Some(s) = self.sheet_mut() else {
            return;
        };
        match action {
            TextColumnsAction::Take => {
                let Some(dialog) = s.text_columns_dialog.take() else {
                    return;
                };
                let result = dialog.apply(&mut s.book);
                self.edited(result, cx);
            }
            TextColumnsAction::Cancel => {
                s.text_columns_dialog = None;
                cx.notify();
            }
            change => {
                if let Some(dialog) = s.text_columns_dialog.as_mut() {
                    change_text_columns_dialog(dialog, change);
                }
                cx.notify();
            }
        }
    }
    pub(super) fn text_columns_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        if self.text_columns_editor().is_none() {
            return false;
        }
        match key {
            "enter" => self.text_columns_action(TextColumnsAction::Take, cx),
            "escape" => self.text_columns_action(TextColumnsAction::Cancel, cx),
            _ => {}
        }
        true
    }
}

use super::super::DocView;
use crate::{i18n, palette as theme};
use gpui::{div, prelude::*, px, Context, Modifiers, MouseButton};
use jade_web::formula_tips as jade_formula_tips;
use jade_web::function_help as jade_function_help;
use jade_web::insert_function::{insert_into, InsertFunctionDialog, InsertFunctionKey};
type View = DocView;
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../jade-gpui-app/src/workbook/insert_function_ui.rs"
));
impl DocView {
    fn insert_function_editor(&self) -> Option<&InsertFunctionDialog> {
        self.sheet_ref()?.insert_function.as_ref()
    }
    /// Insert Function (Shift+F3, or the right-click menu).
    pub(super) fn open_insert_function(&mut self, cx: &mut Context<Self>) {
        if let Some(sheet) = self.sheet_mut() {
            sheet.menu = None;
            sheet.insert_function = Some(InsertFunctionDialog::new());
        }
        cx.notify();
    }
    pub(super) fn insert_function_action(
        &mut self,
        action: InsertFunctionAction,
        cx: &mut Context<Self>,
    ) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        match action {
            InsertFunctionAction::Pick(index) => {
                if let Some(dialog) = sheet.insert_function.as_mut() {
                    dialog.picked = index;
                }
            }
            InsertFunctionAction::Take => {
                let Some(name) = sheet
                    .insert_function
                    .take()
                    .and_then(|dialog| dialog.chosen())
                else {
                    return;
                };
                // A new formula with it, or at the caret of one being typed.
                if let Some(text) =
                    insert_into(sheet.edit.as_mut().map(|edit| &mut edit.text), name)
                {
                    self.begin_cell_edit(Some(text), false, cx);
                }
            }
            InsertFunctionAction::Cancel => sheet.insert_function = None,
        }
        cx.notify();
    }
    pub(super) fn insert_function_key(
        &mut self,
        key: &str,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) -> bool {
        let command = modifiers.platform || modifiers.control;
        let Some(dialog) = self
            .sheet_mut()
            .and_then(|sheet| sheet.insert_function.as_mut())
        else {
            return false;
        };
        match dialog.key(key, command) {
            InsertFunctionKey::Take => self.insert_function_action(InsertFunctionAction::Take, cx),
            InsertFunctionKey::Close => {
                self.insert_function_action(InsertFunctionAction::Cancel, cx)
            }
            InsertFunctionKey::Handled => cx.notify(),
            InsertFunctionKey::Unhandled => return false,
        }
        true
    }
}

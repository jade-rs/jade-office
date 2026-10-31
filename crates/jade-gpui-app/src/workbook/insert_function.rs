use super::WorkbookView;
use crate::{i18n, theme};
use gpui::{div, prelude::*, px, Context, Modifiers, MouseButton};
use jade_office_model::formula_tips as jade_formula_tips;
use jade_office_model::insert_function::{insert_into, InsertFunctionDialog, InsertFunctionKey};
type View = WorkbookView;
include!("insert_function_ui.rs");
impl WorkbookView {
    fn insert_function_editor(&self) -> Option<&InsertFunctionDialog> {
        self.insert_function.as_ref()
    }
    /// Insert Function (Shift+F3, or the right-click menu).
    pub(super) fn open_insert_function(&mut self, cx: &mut Context<Self>) {
        self.menu = None;
        self.insert_function = Some(InsertFunctionDialog::new());
        cx.notify();
    }
    pub(super) fn insert_function_action(
        &mut self,
        action: InsertFunctionAction,
        cx: &mut Context<Self>,
    ) {
        match action {
            InsertFunctionAction::Pick(index) => {
                if let Some(dialog) = self.insert_function.as_mut() {
                    dialog.picked = index;
                }
            }
            InsertFunctionAction::Take => {
                let Some(name) = self
                    .insert_function
                    .take()
                    .and_then(|dialog| dialog.chosen())
                else {
                    return;
                };
                // A new formula with it, or at the caret of one being typed.
                if let Some(text) = insert_into(self.edit.as_mut().map(|edit| &mut edit.text), name)
                {
                    self.begin_edit(Some(text), false, cx);
                }
            }
            InsertFunctionAction::Cancel => self.insert_function = None,
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
        let Some(dialog) = self.insert_function.as_mut() else {
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

use super::WorkbookView;
use crate::{i18n, theme};
use gpui::{div, prelude::*, px, Context, Modifiers, MouseButton};
use jade_office_model::name_manager::{NameFilter, NameManager, NameManagerKey, NameRow, NewName};
use jade_office_model::text::TextEdit;
type View = WorkbookView;
include!("name_manager_ui.rs");
impl WorkbookView {
    fn name_manager_editor(&self) -> Option<(&NameManager, Vec<NameRow>, Vec<String>)> {
        let dialog = self.name_manager.as_ref()?;
        let sheets = self
            .sheet_names
            .iter()
            .map(|name| name.to_string())
            .collect();
        Some((dialog, dialog.listed(&self.book), sheets))
    }
    /// Formulas ▸ Name Manager.
    pub(super) fn open_name_manager(&mut self, cx: &mut Context<Self>) {
        self.name_manager = Some(NameManager::open(&self.book));
        cx.notify();
    }
    pub(super) fn name_manager_action(
        &mut self,
        action: NameManagerAction,
        cx: &mut Context<Self>,
    ) {
        let Some(mut dialog) = self.name_manager.take() else {
            return;
        };
        let result = match action {
            NameManagerAction::Select(index) => {
                dialog.select(&self.book, index);
                Ok(false)
            }
            NameManagerAction::Take => {
                let result = dialog.apply(&mut self.book);
                if let (None, Some(at)) = (dialog.new.as_ref(), dialog.selected) {
                    dialog.select(&self.book, at);
                }
                result
            }
            NameManagerAction::Filter(filter) => {
                dialog.set_filter(&self.book, filter);
                Ok(false)
            }
            NameManagerAction::New => {
                dialog.start_new(&self.book, self.selection.range());
                Ok(false)
            }
            NameManagerAction::CancelNew => {
                dialog.cancel_new(&self.book);
                Ok(false)
            }
            NameManagerAction::Scope(scope) => {
                dialog.set_scope(scope);
                Ok(false)
            }
            NameManagerAction::Focus(on_name) => {
                dialog.focus_name(on_name);
                Ok(false)
            }
            NameManagerAction::Delete => dialog.delete(&mut self.book),
            NameManagerAction::Close => {
                cx.notify();
                return;
            }
        };
        self.name_manager = Some(dialog);
        self.after_write(result, cx);
    }
    pub(super) fn name_manager_key(
        &mut self,
        key: &str,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) -> bool {
        let command = modifiers.platform || modifiers.control;
        let Some(dialog) = self.name_manager.as_mut() else {
            return false;
        };
        match dialog.key(&self.book, key, command) {
            NameManagerKey::Take => self.name_manager_action(NameManagerAction::Take, cx),
            NameManagerKey::Close => self.name_manager_action(NameManagerAction::Close, cx),
            NameManagerKey::Handled => cx.notify(),
            NameManagerKey::Unhandled => return false,
        }
        true
    }
}

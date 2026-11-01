use super::super::DocView;
use crate::{i18n, palette as theme};
use gpui::{div, prelude::*, px, Context, Modifiers, MouseButton};
use jade_web::name_manager::{NameManager, NameManagerKey, NameRow};
type View = DocView;
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../jade-gpui-app/src/workbook/name_manager_ui.rs"
));
impl DocView {
    fn name_manager_editor(&self) -> Option<(&NameManager, Vec<NameRow>, Vec<String>)> {
        let sheet = self.sheet_ref()?;
        let dialog = sheet.name_manager.as_ref()?;
        Some((
            dialog,
            NameManager::rows(&sheet.book),
            sheet.book.sheet_names(),
        ))
    }
    /// Formulas ▸ Name Manager.
    pub(super) fn open_name_manager(&mut self, cx: &mut Context<Self>) {
        if let Some(sheet) = self.sheet_mut() {
            sheet.name_manager = Some(NameManager::open(&sheet.book));
        }
        cx.notify();
    }
    pub(super) fn name_manager_action(
        &mut self,
        action: NameManagerAction,
        cx: &mut Context<Self>,
    ) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let Some(mut dialog) = sheet.name_manager.take() else {
            return;
        };
        let result = match action {
            NameManagerAction::Select(index) => {
                dialog.select(&sheet.book, index);
                Ok(false)
            }
            NameManagerAction::Take => {
                let result = dialog.apply(&mut sheet.book);
                if let Some(at) = dialog.selected {
                    dialog.select(&sheet.book, at);
                }
                result
            }
            NameManagerAction::Delete => dialog.delete(&mut sheet.book),
            NameManagerAction::Close => {
                cx.notify();
                return;
            }
        };
        sheet.name_manager = Some(dialog);
        self.edited(result, cx);
    }
    pub(super) fn name_manager_key(
        &mut self,
        key: &str,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) -> bool {
        let command = modifiers.platform || modifiers.control;
        let Some(sheet) = self.sheet_mut() else {
            return false;
        };
        let Some(dialog) = sheet.name_manager.as_mut() else {
            return false;
        };
        match dialog.key(&sheet.book, key, command) {
            NameManagerKey::Take => self.name_manager_action(NameManagerAction::Take, cx),
            NameManagerKey::Close => self.name_manager_action(NameManagerAction::Close, cx),
            NameManagerKey::Handled => cx.notify(),
            NameManagerKey::Unhandled => return false,
        }
        true
    }
}

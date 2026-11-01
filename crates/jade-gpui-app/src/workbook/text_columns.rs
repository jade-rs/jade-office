use super::WorkbookView;
use crate::{i18n, theme};
use gpui::{div, prelude::*, px, Context, MouseButton};
use jade_office_model::text_columns::{ColumnFormat, TextColumnsDialog};
type View = WorkbookView;
include!("text_columns_ui.rs");
impl WorkbookView {
    fn text_columns_editor(&self) -> Option<&TextColumnsDialog> {
        self.text_columns_dialog.as_ref()
    }
    pub(super) fn open_text_columns_dialog(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        self.text_columns_dialog = Some(TextColumnsDialog::open(
            &self.book,
            self.range(),
            self.active(),
        ));
        self.menu = None;
        cx.notify();
    }
    /// Data ▸ From Text/CSV: a text file picked, opened in the Text Import
    /// Wizard to go in at the active cell.
    pub(super) fn open_text_import(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        self.menu = None;
        let prompt = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |view, cx| {
            let Ok(Ok(Some(paths))) = prompt.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            view.update(cx, |view, cx| match std::fs::read(&path) {
                Ok(bytes) => view.import_text_file(&name, &bytes, cx),
                Err(error) => view.after_write(Err(format!("{}: {error}", path.display())), cx),
            })
            .ok();
        })
        .detach();
    }
    /// The wizard opened on the file `name` holding `bytes`.
    pub(super) fn import_text_file(&mut self, name: &str, bytes: &[u8], cx: &mut Context<Self>) {
        self.text_columns_dialog = Some(TextColumnsDialog::open_import(name, bytes, self.active()));
        cx.notify();
    }
    pub(super) fn text_columns_action(
        &mut self,
        action: TextColumnsAction,
        cx: &mut Context<Self>,
    ) {
        match action {
            TextColumnsAction::Take => {
                let Some(dialog) = self.text_columns_dialog.take() else {
                    return;
                };
                let result = dialog.apply(&mut self.book);
                self.after_write(result, cx);
            }
            TextColumnsAction::Cancel => {
                self.text_columns_dialog = None;
                cx.notify();
            }
            change => {
                if let Some(dialog) = self.text_columns_dialog.as_mut() {
                    change_text_columns_dialog(dialog, change);
                }
                cx.notify();
            }
        }
    }
    pub(super) fn text_columns_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        if self.text_columns_dialog.is_none() {
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

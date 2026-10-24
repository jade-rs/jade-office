use super::WorkbookView;
use crate::{i18n, theme};
use gpui::{div, prelude::*, px, Context, MouseButton};
use jade_office_model::tables::{TableDialog, TableField, TOTALS};
type View = WorkbookView;
include!("tables_ui.rs");
impl WorkbookView {
    fn table_body_height(&self) -> f32 {
        (self.viewport.1 - 110.).clamp(80., 560.)
    }
    fn table_editor(&self) -> Option<&TableDialog> {
        self.table_dialog.as_ref()
    }
    pub(super) fn table_editor_marked(&self) -> bool {
        self.table_editor()
            .is_some_and(|d| d.field().marked.is_some())
    }
    pub(super) fn open_table_dialog(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        self.table_dialog = Some(TableDialog::open(&self.book, self.range(), self.active()));
        self.menu = None;
        cx.notify();
    }
    pub(super) fn sheet_table_action(&mut self, action: TableAction, cx: &mut Context<Self>) {
        if matches!(action, TableAction::Take | TableAction::Convert) {
            let Some(d) = self.table_dialog.as_mut() else {
                return;
            };
            let command = match d.command() {
                Ok(command) => command,
                Err(error) => {
                    d.problem = Some(table_problem(error));
                    cx.notify();
                    return;
                }
            };
            let result = if matches!(action, TableAction::Convert) {
                self.book.convert_table_command(command)
            } else {
                self.book.apply_table_command(command)
            };
            if result.is_ok() {
                self.table_dialog = None;
            } else if let Some(d) = self.table_dialog.as_mut() {
                d.problem = result.as_ref().err().cloned();
            }
            self.after_write(result, cx);
            return;
        }
        if matches!(action, TableAction::Cancel) {
            self.table_dialog = None;
            cx.notify();
            return;
        }
        if let Some(d) = self.table_dialog.as_mut() {
            change_table_dialog(d, action);
        }
        cx.notify();
    }
    pub(super) fn table_key(
        &mut self,
        key: &str,
        shift: bool,
        command: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some(d) = self.table_dialog.as_mut() {
            match key {
                "enter" => self.sheet_table_action(TableAction::Take, cx),
                "escape" => self.sheet_table_action(TableAction::Cancel, cx),
                _ => {
                    d.key(key, shift, command);
                    cx.notify();
                }
            }
            true
        } else {
            false
        }
    }
}

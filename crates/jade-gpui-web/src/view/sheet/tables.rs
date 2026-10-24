use super::super::DocView;
use crate::{i18n, palette as theme};
use gpui::{div, prelude::*, px, Context, MouseButton};
use jade_web::tables::{TableDialog, TableField, TOTALS};
type View = DocView;
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../jade-gpui-app/src/workbook/tables_ui.rs"
));
impl DocView {
    fn table_body_height(&self) -> f32 {
        self.sheet_ref()
            .map(|s| (s.viewport.1 - 110.).clamp(80., 560.))
            .unwrap_or(300.)
    }
    fn table_editor(&self) -> Option<&TableDialog> {
        self.sheet_ref()?.table_dialog.as_ref()
    }
    pub(super) fn table_editor_marked(&self) -> bool {
        self.table_editor()
            .is_some_and(|d| d.field().marked.is_some())
    }
    pub(super) fn open_sheet_table_dialog(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        if let Some(s) = self.sheet_mut() {
            s.table_dialog = Some(TableDialog::open(
                &s.book,
                s.selection.range(),
                s.selection.active,
            ));
            s.menu = None;
        }
        cx.notify();
    }
    pub(super) fn sheet_table_action(&mut self, action: TableAction, cx: &mut Context<Self>) {
        let Some(s) = self.sheet_mut() else {
            return;
        };
        if matches!(action, TableAction::Take | TableAction::Convert) {
            let Some(d) = s.table_dialog.as_mut() else {
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
                s.book.convert_table_command(command)
            } else {
                s.book.apply_table_command(command)
            };
            if result.is_ok() {
                s.table_dialog = None;
            } else if let Some(d) = s.table_dialog.as_mut() {
                d.problem = result.as_ref().err().cloned();
            }
            self.edited(result, cx);
            return;
        }
        if matches!(action, TableAction::Cancel) {
            s.table_dialog = None;
            cx.notify();
            return;
        }
        if let Some(d) = s.table_dialog.as_mut() {
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
        if let Some(d) = self.sheet_mut().and_then(|s| s.table_dialog.as_mut()) {
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

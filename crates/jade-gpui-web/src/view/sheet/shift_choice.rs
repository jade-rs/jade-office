use super::super::DocView;
use super::MenuCommand;
use crate::{i18n, palette as theme};
use gpui::{div, prelude::*, px, Context, MouseButton};
use jade_web::sheet::Axis;
type View = DocView;
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../jade-gpui-app/src/workbook/shift_choice_ui.rs"
));
impl DocView {
    fn shift_choice_open(&self) -> Option<bool> {
        self.sheet_ref()?.shift_choice
    }
    pub(super) fn shift_choice_action(&mut self, choice: ShiftChoice, cx: &mut Context<Self>) {
        let Some(inserted) = self.sheet_mut().and_then(|sheet| sheet.shift_choice.take()) else {
            return;
        };
        match choice.command(inserted) {
            Some(command) => self.sheet_menu_command(command, false, cx),
            None => cx.notify(),
        }
    }
}

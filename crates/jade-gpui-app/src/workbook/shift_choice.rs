use super::{MenuCommand, WorkbookView};
use crate::{i18n, theme};
use gpui::{div, prelude::*, px, Context, MouseButton};
use jade_office_model::sheet::Axis;
type View = WorkbookView;
include!("shift_choice_ui.rs");
impl WorkbookView {
    fn shift_choice_open(&self) -> Option<bool> {
        self.shift_choice
    }
    pub(super) fn shift_choice_action(&mut self, choice: ShiftChoice, cx: &mut Context<Self>) {
        let Some(inserted) = self.shift_choice.take() else {
            return;
        };
        match choice.command(inserted) {
            Some(command) => self.menu_command(command, false, cx),
            None => cx.notify(),
        }
    }
}

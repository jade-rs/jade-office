use super::WorkbookView;
use crate::{i18n, theme};
use gpui::{div, prelude::*, px, Context, Modifiers, MouseButton};
use jade_office_model::form::{Control, FormKey, Label};
use jade_office_model::what_if::{Outcome, WhatIfDialog, WhatIfTool};
type View = WorkbookView;
include!("what_if_ui.rs");
impl WorkbookView {
    fn what_if_editor(&self) -> Option<&WhatIfDialog> {
        self.what_if.as_ref()
    }
    /// Goal Seek, Data Table and the rest, from the grid's menu.
    pub(super) fn open_what_if(&mut self, tool: WhatIfTool, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        let mut dialog = WhatIfDialog::open(tool, &self.book, self.range(), self.active());
        dialog.set_words(i18n::text);
        self.what_if = Some(dialog);
        self.menu = None;
        cx.notify();
    }
    pub(super) fn what_if_action(&mut self, action: WhatIfAction, cx: &mut Context<Self>) {
        let Some(dialog) = self.what_if.as_mut() else {
            return;
        };
        let result = match action {
            WhatIfAction::Control(index, option) => dialog.click(index, option, &mut self.book),
            WhatIfAction::Button(index) => {
                let Some(&(id, _)) = dialog.form().buttons.get(index) else {
                    return;
                };
                dialog.press(id, &mut self.book)
            }
        };
        self.what_if_outcome(result, cx);
    }
    fn what_if_outcome(&mut self, result: Result<Outcome, String>, cx: &mut Context<Self>) {
        match result {
            Ok(outcome) => {
                if outcome.close {
                    self.what_if = None;
                }
                if outcome.wrote {
                    // A report may have gone onto a sheet of its own.
                    self.sheets_changed(Ok(true), cx);
                } else {
                    cx.notify();
                }
            }
            Err(error) => self.after_write(Err(error), cx),
        }
    }
    pub(super) fn what_if_key(
        &mut self,
        key: &str,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) -> bool {
        let command = modifiers.platform || modifiers.control;
        let Some(dialog) = self.what_if.as_mut() else {
            return false;
        };
        match dialog.form_mut().key(key, command) {
            FormKey::Press(id) => {
                let result = dialog.press(id, &mut self.book);
                self.what_if_outcome(result, cx);
            }
            FormKey::Handled => cx.notify(),
            FormKey::Unhandled => return false,
        }
        true
    }
}

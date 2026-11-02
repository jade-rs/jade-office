use super::super::DocView;
use crate::{i18n, palette as theme};
use gpui::{div, prelude::*, px, Context, Modifiers, MouseButton};
use jade_web::form::{Control, FormKey, Label};
use jade_web::what_if::{Outcome, WhatIfDialog, WhatIfTool};
type View = DocView;
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../jade-gpui-app/src/workbook/what_if_ui.rs"
));
impl DocView {
    fn what_if_editor(&self) -> Option<&WhatIfDialog> {
        self.sheet_ref()?.what_if.as_ref()
    }
    /// Goal Seek, Data Table and the rest, from the grid's menu.
    pub(super) fn open_what_if(&mut self, tool: WhatIfTool, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        if let Some(s) = self.sheet_mut() {
            let mut dialog =
                WhatIfDialog::open(tool, &s.book, s.selection.range(), s.selection.active);
            dialog.set_words(i18n::text);
            s.what_if = Some(dialog);
            s.menu = None;
        }
        cx.notify();
    }
    pub(super) fn what_if_action(&mut self, action: WhatIfAction, cx: &mut Context<Self>) {
        let Some(s) = self.sheet_mut() else {
            return;
        };
        let Some(dialog) = s.what_if.as_mut() else {
            return;
        };
        let result = match action {
            WhatIfAction::Control(index, option) => dialog.click(index, option, &mut s.book),
            WhatIfAction::Button(index) => {
                let Some(&(id, _)) = dialog.form().buttons.get(index) else {
                    return;
                };
                dialog.press(id, &mut s.book)
            }
        };
        self.what_if_outcome(result, cx);
    }
    fn what_if_outcome(&mut self, result: Result<Outcome, String>, cx: &mut Context<Self>) {
        match result {
            Ok(outcome) => {
                if outcome.close {
                    if let Some(s) = self.sheet_mut() {
                        s.what_if = None;
                    }
                }
                if outcome.wrote {
                    // A report may have gone onto a sheet of its own.
                    self.sheets_changed(Ok(true), cx);
                } else {
                    cx.notify();
                }
            }
            Err(error) => self.edited(Err(error), cx),
        }
    }
    pub(super) fn what_if_key(
        &mut self,
        key: &str,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) -> bool {
        let command = modifiers.platform || modifiers.control;
        let Some(s) = self.sheet_mut() else {
            return false;
        };
        let Some(dialog) = s.what_if.as_mut() else {
            return false;
        };
        match dialog.form_mut().key(key, command) {
            FormKey::Press(id) => {
                let result = dialog.press(id, &mut s.book);
                self.what_if_outcome(result, cx);
            }
            FormKey::Handled => cx.notify(),
            FormKey::Unhandled => return false,
        }
        true
    }
}

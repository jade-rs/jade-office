use super::super::DocView;
use crate::{i18n, palette as theme};
use gpui::{div, prelude::*, px, Context, MouseButton};
use jade_web::editable_ranges::RangeAction;
use jade_web::protection::{ProtectionDialog, ProtectionTarget};
type View = DocView;
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../jade-gpui-app/src/workbook/protection_ui.rs"
));
impl DocView {
    fn protection_editor(&self) -> Option<&ProtectionDialog> {
        self.sheet_ref()?.protection_dialog.as_ref()
    }
    pub(super) fn open_sheet_protection(&mut self, index: usize, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        self.select_sheet(index, cx);
        let Some(s) = self.sheet_mut() else {
            return;
        };
        if s.book.sheet_index() != index {
            return;
        }
        match ProtectionDialog::open_sheet(&s.book) {
            Ok(dialog) => s.protection_dialog = Some(dialog),
            Err(_) => self.status = i18n::text("protection.unsupported").into(),
        }
        cx.notify();
    }
    pub(super) fn open_editable_ranges(&mut self, index: usize, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        self.select_sheet(index, cx);
        let Some(s) = self.sheet_mut() else {
            return;
        };
        if s.book.sheet_index() != index {
            return;
        }
        match ProtectionDialog::open_ranges(
            &s.book,
            s.selection.range(),
            i18n::text("protection.range_default"),
        ) {
            Ok(dialog) => {
                s.protection_dialog = Some(dialog);
                s.menu = None;
            }
            Err(_) => self.status = i18n::text("protection.range_unsupported").into(),
        }
        cx.notify();
    }
    pub(super) fn open_workbook_protection(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        let result = self.sheet_ref().map(|s| ProtectionDialog::open(&s.book));
        if let Some(result) = result {
            match result {
                Ok(dialog) => {
                    if let Some(s) = self.sheet_mut() {
                        s.protection_dialog = Some(dialog);
                        s.menu = None;
                    }
                }
                Err(_) => self.status = i18n::text("protection.unsupported").into(),
            }
        }
        cx.notify();
    }
    pub(super) fn workbook_protection_action(
        &mut self,
        action: ProtectionAction,
        cx: &mut Context<Self>,
    ) {
        let Some(s) = self.sheet_mut() else {
            return;
        };
        match action {
            ProtectionAction::Cancel => s.protection_dialog = None,
            ProtectionAction::Focus(i) => {
                if let Some(d) = s.protection_dialog.as_mut() {
                    d.focus(i);
                }
            }
            ProtectionAction::Toggle(i) => {
                if let Some(d) = s.protection_dialog.as_mut() {
                    d.toggle_permission(i);
                }
            }
            ProtectionAction::Range(action) => {
                if let Some(d) = s.protection_dialog.as_mut() {
                    if let Ok(true) = d.range_action(action, &mut s.book) {
                        self.edited(Ok(true), cx);
                        return;
                    }
                }
            }
            ProtectionAction::Take => {
                let entry = s
                    .edit
                    .as_ref()
                    .map(|edit| (s.selection.active, edit.text.text.clone()));
                let Some(d) = s.protection_dialog.as_mut() else {
                    return;
                };
                let operation = d.is_operation_prompt();
                let resume = d.entry_cell().is_some();
                let result = if operation {
                    d.apply_for_operation(
                        &mut s.book,
                        entry.as_ref().map(|(at, text)| (*at, text.as_str())),
                    )
                } else if resume {
                    if let Some((at, text)) = entry {
                        d.apply_for_entry(&mut s.book, at, &text)
                    } else {
                        d.problem = Some("protection.range_stale");
                        Err("protection.range_stale")
                    }
                } else {
                    d.apply(&mut s.book)
                };
                if let Ok(changed) = result {
                    if operation {
                        if let Some(outcome) = d.take_completed_operation() {
                            if let Some(range) = outcome.selection {
                                let active = outcome.active_cell(s.selection.active);
                                s.selection = super::Selection {
                                    active,
                                    anchor: range.start,
                                    focus: range.end,
                                };
                            }
                            s.edit = None;
                            s.protection_dialog = None;
                            self.edited(Ok(outcome.changed), cx);
                        } else {
                            cx.notify();
                        }
                        return;
                    }
                    if resume {
                        s.protection_dialog = None;
                        self.commit_cell(cx);
                        return;
                    }
                    let ranges = d.target == ProtectionTarget::Ranges;
                    let session_only = ranges && d.unprotect;
                    if !ranges {
                        s.protection_dialog = None;
                    }
                    if session_only {
                        cx.notify();
                    } else {
                        self.edited(Ok(changed), cx);
                    }
                    return;
                }
            }
        }
        cx.notify();
    }
    pub(super) fn protection_key(
        &mut self,
        key: &str,
        shift: bool,
        command: bool,
        cx: &mut Context<Self>,
    ) -> Option<bool> {
        let d = self.sheet_mut()?.protection_dialog.as_mut()?;
        if d.field().marked.is_some() || (command && key == "v") {
            return Some(false);
        }
        match key {
            "enter" => self.workbook_protection_action(ProtectionAction::Take, cx),
            "escape" => self.workbook_protection_action(ProtectionAction::Cancel, cx),
            _ => {
                d.key(key, shift, command);
                cx.notify();
            }
        }
        Some(true)
    }
}

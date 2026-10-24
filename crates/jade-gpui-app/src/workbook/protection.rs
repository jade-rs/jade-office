use super::WorkbookView;
use crate::{i18n, theme};
use gpui::{div, prelude::*, px, Context, MouseButton};
use jade_office_model::editable_ranges::RangeAction;
use jade_office_model::protection::{ProtectionDialog, ProtectionTarget};
type View = WorkbookView;
include!("protection_ui.rs");
impl WorkbookView {
    fn protection_editor(&self) -> Option<&ProtectionDialog> {
        self.protection_dialog.as_ref()
    }
    pub(super) fn open_sheet_protection(&mut self, index: usize, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        self.select_sheet(index, cx);
        if self.book.sheet_index() != index {
            return;
        }
        match ProtectionDialog::open_sheet(&self.book) {
            Ok(dialog) => self.protection_dialog = Some(dialog),
            Err(_) => self.status = i18n::text("protection.unsupported").into(),
        }
        self.menu = None;
        cx.notify();
    }
    pub(super) fn open_editable_ranges(&mut self, index: usize, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        self.select_sheet(index, cx);
        if self.book.sheet_index() != index {
            return;
        }
        match ProtectionDialog::open_ranges(
            &self.book,
            self.range(),
            i18n::text("protection.range_default"),
        ) {
            Ok(dialog) => self.protection_dialog = Some(dialog),
            Err(_) => self.status = i18n::text("protection.range_unsupported").into(),
        }
        self.menu = None;
        cx.notify();
    }
    pub(super) fn open_workbook_protection(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        match ProtectionDialog::open(&self.book) {
            Ok(dialog) => self.protection_dialog = Some(dialog),
            Err(_) => self.status = i18n::text("protection.unsupported").into(),
        }
        self.menu = None;
        cx.notify();
    }
    pub(super) fn workbook_protection_action(
        &mut self,
        action: ProtectionAction,
        cx: &mut Context<Self>,
    ) {
        match action {
            ProtectionAction::Cancel => self.protection_dialog = None,
            ProtectionAction::Focus(i) => {
                if let Some(d) = self.protection_dialog.as_mut() {
                    d.focus(i);
                }
            }
            ProtectionAction::Toggle(i) => {
                if let Some(d) = self.protection_dialog.as_mut() {
                    d.toggle_permission(i);
                }
            }
            ProtectionAction::Range(action) => {
                if let Some(d) = self.protection_dialog.as_mut() {
                    if let Ok(true) = d.range_action(action, &mut self.book) {
                        self.after_write(Ok(true), cx);
                        return;
                    }
                }
            }
            ProtectionAction::Take => {
                let entry = self
                    .edit
                    .as_ref()
                    .map(|edit| (self.active(), edit.text.text.clone()));
                let Some(d) = self.protection_dialog.as_mut() else {
                    return;
                };
                let operation = d.is_operation_prompt();
                let resume = d.entry_cell().is_some();
                let result = if operation {
                    d.apply_for_operation(
                        &mut self.book,
                        entry.as_ref().map(|(at, text)| (*at, text.as_str())),
                    )
                } else if resume {
                    if let Some((at, text)) = entry {
                        d.apply_for_entry(&mut self.book, at, &text)
                    } else {
                        d.problem = Some("protection.range_stale");
                        Err("protection.range_stale")
                    }
                } else {
                    d.apply(&mut self.book)
                };
                if let Ok(changed) = result {
                    if operation {
                        if let Some(outcome) = d.take_completed_operation() {
                            if let Some(range) = outcome.selection {
                                let active = outcome.active_cell(self.selection.active);
                                self.selection = super::Selection {
                                    active,
                                    anchor: range.start,
                                    focus: range.end,
                                };
                            }
                            self.edit = None;
                            self.protection_dialog = None;
                            self.after_write(Ok(outcome.changed), cx);
                        } else {
                            cx.notify();
                        }
                        return;
                    }
                    if resume {
                        self.protection_dialog = None;
                        self.commit_cell(cx);
                        return;
                    }
                    let ranges = d.target == ProtectionTarget::Ranges;
                    let session_only = ranges && d.unprotect;
                    if !ranges {
                        self.protection_dialog = None;
                    }
                    if session_only {
                        cx.notify();
                    } else {
                        self.after_write(Ok(changed), cx);
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
        let d = self.protection_dialog.as_mut()?;
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

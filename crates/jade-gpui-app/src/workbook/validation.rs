use super::WorkbookView;
use crate::{i18n, theme};
use gpui::{div, prelude::*, px, Context, MouseButton};
use jade_office_model::validation::{
    AlertStyle, PendingValidation, ValidationDialog, ValidationField, ValidationTab, IME_MODES,
    OPERATORS, STYLES, TYPES,
};
type View = WorkbookView;
include!("validation_ui.rs");
impl WorkbookView {
    fn validation_editor(&self) -> Option<&ValidationDialog> {
        self.validation_dialog.as_ref()
    }
    fn validation_pending(&self) -> Option<&PendingValidation> {
        self.validation_pending.as_ref()
    }
    fn validation_input_message(&self) -> Option<(String, String)> {
        self.book.validation_prompt(self.active())
    }
    pub(super) fn open_validation(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        self.validation_dialog = Some(ValidationDialog::open(
            &self.book,
            self.range(),
            self.active(),
        ));
        self.menu = None;
        cx.notify();
    }
    pub(super) fn validation_action(&mut self, action: ValidationAction, cx: &mut Context<Self>) {
        if matches!(action, ValidationAction::Take | ValidationAction::Clear) {
            let Some(dialog) = self.validation_dialog.as_ref() else {
                return;
            };
            let result = if matches!(action, ValidationAction::Clear) {
                match dialog.target_ranges() {
                    Ok(ranges) => self.book.clear_data_validation_ranges(&ranges),
                    Err(problem) => {
                        self.validation_dialog.as_mut().unwrap().problem = Some(problem);
                        cx.notify();
                        return;
                    }
                }
            } else {
                let rule = match dialog.rule() {
                    Ok(rule) => rule,
                    Err(problem) => {
                        self.validation_dialog.as_mut().unwrap().problem = Some(problem);
                        cx.notify();
                        return;
                    }
                };
                self.book.apply_data_validation(&rule)
            };
            if result.is_ok() {
                self.validation_dialog = None;
            }
            self.after_write(result, cx);
            return;
        }
        if matches!(action, ValidationAction::Cancel) {
            self.validation_dialog = None;
            cx.notify();
            return;
        }
        if let Some(d) = self.validation_dialog.as_mut() {
            match action {
                ValidationAction::Tab(tab) => d.select_tab(tab),
                ValidationAction::Focus(f) => d.focus(f),
                ValidationAction::Kind => {
                    d.kind = (d.kind + 1) % TYPES.len();
                    d.field = ValidationField::Ranges;
                }
                ValidationAction::Operator => d.operator = (d.operator + 1) % OPERATORS.len(),
                ValidationAction::Style => d.style = (d.style + 1) % STYLES.len(),
                ValidationAction::Ime => d.ime = (d.ime + 1) % IME_MODES.len(),
                ValidationAction::Blank => d.allow_blank = !d.allow_blank,
                ValidationAction::Same => d.apply_to_same = !d.apply_to_same,
                ValidationAction::Dropdown => d.dropdown = !d.dropdown,
                ValidationAction::Input => d.show_input = !d.show_input,
                ValidationAction::Error => d.show_error = !d.show_error,
                _ => {}
            }
        }
        cx.notify();
    }
    pub(super) fn validation_key(
        &mut self,
        key: &str,
        shift: bool,
        command: bool,
        alt: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.validation_pending.is_some() {
            match key {
                "escape" => self.answer_validation(ValidationAnswer::Cancel, cx),
                "enter" => {
                    let answer = if self.validation_pending.as_ref().unwrap().alert.style
                        == AlertStyle::Stop
                    {
                        ValidationAnswer::Retry
                    } else {
                        ValidationAnswer::Accept
                    };
                    self.answer_validation(answer, cx);
                }
                _ => {}
            }
            return true;
        }
        if let Some(dialog) = self.validation_dialog.as_mut() {
            if key == "enter"
                && alt
                && matches!(
                    dialog.field,
                    ValidationField::Prompt | ValidationField::Error
                )
            {
                dialog.field_mut().insert("\n");
                cx.notify();
                return true;
            }
            match key {
                "escape" => self.validation_action(ValidationAction::Cancel, cx),
                "enter" => self.validation_action(ValidationAction::Take, cx),
                _ => {
                    dialog.key(key, shift, command);
                    cx.notify();
                }
            }
            return true;
        }
        false
    }
    pub(super) fn answer_validation(&mut self, answer: ValidationAnswer, cx: &mut Context<Self>) {
        let Some(pending) = self.validation_pending.take() else {
            return;
        };
        match answer {
            ValidationAnswer::Accept if pending.alert.style != AlertStyle::Stop => {
                let result = self
                    .book
                    .set_cell_accepting_warning(pending.at, &pending.text);
                if result.is_ok() {
                    self.edit = None;
                    self.book.forget_copy();
                }
                self.after_write(result, cx);
            }
            ValidationAnswer::Cancel => self.edit = None,
            _ => {}
        }
        cx.notify();
    }
}

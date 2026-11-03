//! Excel's calculation options (File ▸ Options ▸ Formulas, and Formulas ▸
//! Calculation Options): automatic, automatic except for data tables or
//! manual; recalculate before saving; iterative calculation with its
//! maximum iterations and maximum change; and precision as displayed.

use crate::sheet::Workbook;
use crate::Backend;
pub use jade_xlsx_reader::{CalculationMode, Iteration};

/// The options as the workbook has them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CalculationOptions {
    pub mode: CalculationMode,
    pub recalculate_before_save: bool,
    pub iteration: Iteration,
    pub precision_as_displayed: bool,
}

/// The two numbers the dialog takes typed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalculationField {
    MaximumIterations,
    MaximumChange,
}

/// What a key did in the dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalculationKey {
    Take,
    Close,
    Handled,
    Unhandled,
}

/// The Calculation Options dialog.
pub struct CalculationOptionsDialog {
    pub options: CalculationOptions,
    /// The maximum iterations as typed.
    pub count_text: String,
    /// The maximum change as typed.
    pub delta_text: String,
    /// The number field keys type into, if any.
    pub field: Option<CalculationField>,
}

impl CalculationOptionsDialog {
    pub fn new(options: CalculationOptions) -> Self {
        Self {
            count_text: options.iteration.count.to_string(),
            delta_text: options.iteration.delta.to_string(),
            options,
            field: None,
        }
    }

    pub fn set_mode(&mut self, mode: CalculationMode) {
        self.options.mode = mode;
        // Excel offers Recalculate before saving with manual calculation
        // only; automatic always saves calculated values.
        if mode != CalculationMode::Manual {
            self.options.recalculate_before_save = true;
        }
    }

    pub fn toggle_recalculate_before_save(&mut self) {
        if self.options.mode == CalculationMode::Manual {
            self.options.recalculate_before_save = !self.options.recalculate_before_save;
        }
    }

    pub fn toggle_iteration(&mut self) {
        self.options.iteration.enabled = !self.options.iteration.enabled;
        if !self.options.iteration.enabled {
            self.field = None;
        }
    }

    pub fn toggle_precision_as_displayed(&mut self) {
        self.options.precision_as_displayed = !self.options.precision_as_displayed;
    }

    /// The field clicked, to type into; iteration's fields only while it is on.
    pub fn focus(&mut self, field: CalculationField) {
        if self.options.iteration.enabled {
            self.field = Some(field);
        }
    }

    /// The maximum iterations stepped by `by` from what is typed (an empty
    /// box from zero), between 1 and Excel's 32767.
    pub fn step_count(&mut self, by: i32) {
        let typed = self.count_text.trim().parse::<i64>().unwrap_or(0);
        let count = typed + by as i64;
        self.count_text = count.clamp(1, 32_767).to_string();
    }

    fn count(&self) -> Option<u32> {
        self.count_text
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|count| (1..=32_767).contains(count))
    }

    fn delta(&self) -> Option<f64> {
        self.delta_text
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|delta| delta.is_finite() && *delta >= 0.0)
    }

    /// Whether the typed numbers are ones Excel takes: 1 to 32767
    /// iterations and a change of zero or more.
    pub fn valid(&self) -> bool {
        !self.options.iteration.enabled || (self.count().is_some() && self.delta().is_some())
    }

    /// The options with the typed numbers in.
    pub fn result(&self) -> CalculationOptions {
        let mut options = self.options;
        if let Some(count) = self.count() {
            options.iteration.count = count;
        }
        if let Some(delta) = self.delta() {
            options.iteration.delta = delta;
        }
        options
    }

    /// Enter takes the options, Escape closes, Tab moves between the
    /// numbers, and digits, a point and Backspace type into the one focused.
    pub fn key(&mut self, key: &str) -> CalculationKey {
        match key {
            "enter" => {
                if self.valid() {
                    CalculationKey::Take
                } else {
                    CalculationKey::Handled
                }
            }
            "escape" => CalculationKey::Close,
            "tab" if self.options.iteration.enabled => {
                self.field = Some(match self.field {
                    Some(CalculationField::MaximumIterations) => CalculationField::MaximumChange,
                    _ => CalculationField::MaximumIterations,
                });
                CalculationKey::Handled
            }
            "up" | "down" if self.field == Some(CalculationField::MaximumIterations) => {
                self.step_count(if key == "up" { 1 } else { -1 });
                CalculationKey::Handled
            }
            "backspace" => match self.text_mut() {
                Some(text) => {
                    text.pop();
                    CalculationKey::Handled
                }
                None => CalculationKey::Unhandled,
            },
            typed if typed.chars().count() == 1 => {
                let character = typed.chars().next().unwrap_or_default();
                let field = self.field;
                match self.text_mut() {
                    Some(text)
                        if character.is_ascii_digit()
                            || (field == Some(CalculationField::MaximumChange)
                                && character == '.'
                                && !text.contains('.')) =>
                    {
                        text.push(character);
                        CalculationKey::Handled
                    }
                    _ => CalculationKey::Unhandled,
                }
            }
            _ => CalculationKey::Unhandled,
        }
    }

    fn text_mut(&mut self) -> Option<&mut String> {
        match self.field? {
            CalculationField::MaximumIterations => Some(&mut self.count_text),
            CalculationField::MaximumChange => Some(&mut self.delta_text),
        }
    }

    /// Sets `book`'s options as the dialog says; `Ok(true)` when any changed.
    pub fn apply<B: Backend>(&self, book: &mut Workbook<B>) -> Result<bool, String> {
        if !self.valid() {
            return Ok(false);
        }
        book.set_calculation_options(&self.result())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> CalculationOptions {
        CalculationOptions {
            mode: CalculationMode::Auto,
            recalculate_before_save: true,
            iteration: Iteration::default(),
            precision_as_displayed: false,
        }
    }

    #[test]
    fn typing_the_iteration_numbers() {
        let mut dialog = CalculationOptionsDialog::new(options());
        assert_eq!(dialog.count_text, "100");
        assert_eq!(dialog.delta_text, "0.001");
        // The fields take nothing while iteration is off.
        dialog.focus(CalculationField::MaximumIterations);
        assert_eq!(dialog.field, None);
        dialog.toggle_iteration();
        assert_eq!(dialog.key("tab"), CalculationKey::Handled);
        assert_eq!(dialog.field, Some(CalculationField::MaximumIterations));
        for key in ["backspace", "backspace", "backspace", "5", "0"] {
            dialog.key(key);
        }
        dialog.key("tab");
        for key in ["backspace", "backspace", "backspace", "1", ".", "."] {
            dialog.key(key);
        }
        assert_eq!(dialog.delta_text, "0.1");
        let result = dialog.result();
        assert!(result.iteration.enabled);
        assert_eq!(result.iteration.count, 50);
        assert_eq!(result.iteration.delta, 0.1);
        // Zero iterations is not one Excel takes.
        dialog.focus(CalculationField::MaximumIterations);
        dialog.key("backspace");
        dialog.key("backspace");
        assert!(!dialog.valid());
        assert_eq!(dialog.key("enter"), CalculationKey::Handled);
        dialog.key("up");
        assert_eq!(dialog.count_text, "1");
        assert_eq!(dialog.key("enter"), CalculationKey::Take);
    }

    #[test]
    fn recalculate_before_save_is_for_manual_calculation() {
        let mut dialog = CalculationOptionsDialog::new(options());
        dialog.toggle_recalculate_before_save();
        assert!(dialog.options.recalculate_before_save);
        dialog.set_mode(CalculationMode::Manual);
        dialog.toggle_recalculate_before_save();
        assert!(!dialog.options.recalculate_before_save);
        dialog.set_mode(CalculationMode::AutoNoTable);
        assert!(dialog.options.recalculate_before_save);
    }
}

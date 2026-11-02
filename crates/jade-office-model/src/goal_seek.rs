//! Data ▸ What-If Analysis ▸ Goal Seek: the value of one cell that makes a
//! formula come to a target, found by trying values in it as Excel does —
//! at most 100 tries, done when the formula is within 0.001 of the target.

use jade_xlsx_headless_api::CellInput;
use jade_xlsx_reader::{CellRef, CellValue};

use crate::form::{Form, Label};
use crate::sheet::{reference_text, Checkpoint, Workbook};
use crate::what_if::{absolute, single_cell, Outcome, ToolDialog};
use crate::Backend;

/// Excel's Maximum Iterations and Maximum Change for Goal Seek.
const TRIES: usize = 100;
const CLOSE_ENOUGH: f64 = 0.001;

/// What Goal Seek found.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GoalSought {
    /// Whether the formula came within 0.001 of the target.
    pub found: bool,
    /// The value left in the changing cell: the best tried.
    pub value: f64,
    /// What the formula comes to with it.
    pub result: f64,
}

impl<B: Backend> Workbook<B> {
    /// Goal Seek on the active sheet: tries values in `changing` until the
    /// formula in `set` comes to `target`, and leaves the best of them
    /// there — one undo step. Refused, as Excel refuses it, when `set` holds
    /// no formula or `changing` holds one.
    pub fn goal_seek(
        &mut self,
        set: CellRef,
        target: f64,
        changing: CellRef,
    ) -> Result<GoalSought, String> {
        if !self.edit_text(set).starts_with('=') {
            return Err("goal_seek.alert.formula".into());
        }
        if self.edit_text(changing).starts_with('=') {
            return Err("goal_seek.alert.value".into());
        }
        let sheet = self.sheet_index();
        let start = self.number(changing).unwrap_or(0.0);
        let mut probe = self
            .engine()
            .what_if()
            .map_err(|error| error.to_string())?;
        let workbook = self.engine().workbook();
        let (value, result) = seek(
            |x| {
                probe
                    .number(workbook, (sheet, changing), x, (sheet, set))
                    .map(|found| found.map(|result| result - target))
                    .map_err(|error| error.to_string())
            },
            start,
        )?;
        let result = result.map(|off| off + target).unwrap_or(f64::NAN);
        let name = self.sheet_name();
        self.engine_mut()
            .set_cells(
                &name,
                &[(reference_text(changing), CellInput::Number(value))],
            )
            .map_err(|error| error.to_string())?;
        self.wrote(1)?;
        Ok(GoalSought {
            found: (result - target).abs() <= CLOSE_ENOUGH,
            value,
            result: match self.engine().workbook().sheets[sheet].cell_at(set) {
                Some(cell) => match cell.value {
                    CellValue::Number(number) => number,
                    _ => result,
                },
                None => result,
            },
        })
    }
}

/// The `x` that brings `off(x)` — the formula less its target — nearest to
/// nought in [`TRIES`] tries from `start`, and what `off` was there; `None`
/// when no try gave a number. Secant steps until the root is bracketed,
/// then false position with the Illinois change, which keeps both ends
/// moving.
pub(crate) fn seek(
    mut off: impl FnMut(f64) -> Result<Option<f64>, String>,
    start: f64,
) -> Result<(f64, Option<f64>), String> {
    let mut best: Option<(f64, f64)> = None;
    let keep = |x: f64, y: f64, best: &mut Option<(f64, f64)>| {
        if best.is_none_or(|(_, found)| y.abs() < found.abs()) {
            *best = Some((x, y));
        }
    };
    let mut tries = 0;
    let mut a = (start, None);
    // The first number to start from: the cell's own value, else a step
    // either way from it.
    for x in [start, start + 1.0, start - 1.0, start * 2.0 + 1.0] {
        tries += 1;
        if let Some(y) = off(x)? {
            a = (x, Some(y));
            keep(x, y, &mut best);
            break;
        }
    }
    let Some(mut fa) = a.1 else {
        return Ok((start, None));
    };
    let mut xa = a.0;
    if fa.abs() <= CLOSE_ENOUGH * 1e-6 {
        return Ok((xa, Some(fa)));
    }
    let mut xb = if xa == 0.0 { 0.01 } else { xa * 1.01 };
    let mut fb = loop {
        tries += 1;
        if let Some(y) = off(xb)? {
            break y;
        }
        if tries >= TRIES {
            return Ok(best.map_or((start, None), |(x, y)| (x, Some(y))));
        }
        xb = (xa + xb) / 2.0;
    };
    keep(xb, fb, &mut best);
    // Which end false position kept last time, for the Illinois halving.
    let mut kept = 0i8;
    // A try that gave no number is tried again halfway back.
    let mut retry: Option<f64> = None;
    while tries < TRIES {
        if fb.abs() <= f64::EPSILON * (1.0 + xb.abs()) {
            break;
        }
        let bracketed = fa.signum() != fb.signum();
        let x = match retry.take() {
            Some(x) => x,
            None => {
                let mut x = if fb != fa {
                    xb - fb * (xb - xa) / (fb - fa)
                } else {
                    f64::NAN
                };
                if bracketed {
                    let (low, high) = (xa.min(xb), xa.max(xb));
                    if !(x > low && x < high) {
                        x = (xa + xb) / 2.0;
                    }
                } else if !x.is_finite() || (x - xb).abs() > 1e6 * (1.0 + xb.abs()) {
                    // Flat, or a step too far: on in the same direction,
                    // further.
                    let reach = 1e6 * (1.0 + xb.abs());
                    let step = if xb != xa { (xb - xa) * 2.0 } else { 1.0 };
                    x = xb + step.clamp(-reach, reach);
                }
                x
            }
        };
        tries += 1;
        let Some(y) = off(x)? else {
            retry = Some((x + xb) / 2.0);
            continue;
        };
        keep(x, y, &mut best);
        if x == xb {
            break;
        }
        if bracketed {
            if y.signum() == fb.signum() {
                xb = x;
                fb = y;
                if kept == 1 {
                    fa /= 2.0;
                }
                kept = 1;
            } else {
                xa = x;
                fa = y;
                if kept == -1 {
                    fb /= 2.0;
                }
                kept = -1;
            }
        } else {
            (xa, fa) = (xb, fb);
            (xb, fb) = (x, y);
        }
        // Close enough to stop once the step itself is too small to matter.
        if (xa - xb).abs() <= f64::EPSILON * (1.0 + xb.abs()) {
            break;
        }
    }
    Ok(best.map_or((start, None), |(x, y)| (x, Some(y))))
}

/// The Goal Seek dialog: what it asks, then its status — OK keeps the
/// value found, Cancel puts the cell back as it was.
pub struct GoalSeekDialog {
    pub form: Form,
    /// The workbook before the value went in, while the status shows.
    before: Option<Checkpoint>,
}

impl GoalSeekDialog {
    /// Set cell starts as the active cell, as in Excel.
    pub fn open(at: CellRef) -> Self {
        Self {
            form: Form::new("goal_seek.dialog")
                .field("set", "goal_seek.set", &absolute(at))
                .field("to", "goal_seek.to", "")
                .field("changing", "goal_seek.changing", "")
                .button("ok", "validation.ok")
                .button("cancel", "validation.cancel"),
            before: None,
        }
    }
}

impl ToolDialog for GoalSeekDialog {
    fn form(&self) -> &Form {
        &self.form
    }

    fn form_mut(&mut self) -> &mut Form {
        &mut self.form
    }

    fn press<B: Backend>(
        &mut self,
        button: &str,
        book: &mut Workbook<B>,
    ) -> Result<Outcome, String> {
        match (button, self.before.is_some()) {
            // The status's OK: the value stays.
            ("ok", true) => Ok(Outcome::close()),
            ("cancel", true) => {
                if let Some(before) = self.before.take() {
                    book.restore(before);
                }
                Ok(Outcome::closed_writing())
            }
            ("ok", false) => self.seek(book),
            _ => Ok(Outcome::close()),
        }
    }
}

impl GoalSeekDialog {
    fn seek<B: Backend>(&mut self, book: &mut Workbook<B>) -> Result<Outcome, String> {
        let form = &mut self.form;
        form.alert = None;
        let Some(set) = single_cell(book, form.text("set")) else {
            form.alert = Some("what_if.alert.reference".into());
            form.focus = Some(0);
            return Ok(Outcome::stay());
        };
        let Ok(target) = form.text("to").parse::<f64>() else {
            form.alert = Some("goal_seek.alert.number".into());
            form.focus = Some(1);
            return Ok(Outcome::stay());
        };
        let Some(changing) = single_cell(book, form.text("changing")) else {
            form.alert = Some("what_if.alert.reference".into());
            form.focus = Some(2);
            return Ok(Outcome::stay());
        };
        let before = book.checkpoint();
        let sought = match book.goal_seek(set, target, changing) {
            Ok(sought) => sought,
            Err(error) if error.starts_with("goal_seek.alert.") => {
                form.alert = Some(Label::Key(if error.ends_with("formula") {
                    "goal_seek.alert.formula"
                } else {
                    "goal_seek.alert.value"
                }));
                return Ok(Outcome::stay());
            }
            Err(error) => return Err(error),
        };
        self.before = Some(before);
        let cell = absolute(set);
        self.form = Form::new("goal_seek.status")
            .note(
                "outcome",
                Label::Format(
                    if sought.found {
                        "goal_seek.found"
                    } else {
                        "goal_seek.not_found"
                    },
                    vec![("cell", cell)],
                ),
            )
            .note(
                "target",
                Label::Format("goal_seek.target", vec![("value", format_value(target))]),
            )
            .note(
                "current",
                Label::Format(
                    "goal_seek.current",
                    vec![("value", book.display_text(set))],
                ),
            )
            .button("ok", "validation.ok")
            .button("cancel", "validation.cancel");
        Ok(Outcome::writing())
    }
}

/// A number as the status shows the target: as typed, without trailing
/// noughts.
fn format_value(value: f64) -> String {
    crate::sheet::format_number(value)
}

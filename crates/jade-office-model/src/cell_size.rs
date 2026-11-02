//! Home ▸ Format ▸ Row Height…, Column Width…, AutoFit Row Height and
//! AutoFit Column Width, as Excel's: a height in points (0–409) or a width
//! in characters (0–255) typed for the selected rows or columns.

use jade_xlsx_reader::Axis;

use crate::sheet::Workbook;
use crate::text::TextEdit;
use crate::Backend;

/// What a key did in the dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SizeKey {
    Take,
    Close,
    Handled,
    Unhandled,
}

/// The Row Height (rows) or Column Width (columns) dialog.
pub struct SizeDialog {
    pub axis: Axis,
    pub first: u32,
    pub last: u32,
    pub value: TextEdit,
    /// What was typed is not a size Excel takes: OK keeps the dialog open.
    pub problem: bool,
}

impl SizeDialog {
    /// Opened on the first row's height or the first column's width; a
    /// selection whose sizes differ starts empty, as in Excel.
    pub fn open<B: Backend>(book: &Workbook<B>, axis: Axis, first: u32, last: u32) -> Self {
        let size = |index| book.axis_size(axis, index);
        let current = size(first);
        let same = (first..=last.min(first + 1_000)).all(|index| size(index) == current);
        let text = if same {
            crate::sheet::format_number(current)
        } else {
            String::new()
        };
        Self {
            axis,
            first,
            last,
            value: TextEdit::selected(text),
            problem: false,
        }
    }

    /// Up to 409 points or 255 characters, as Excel allows.
    pub fn most(&self) -> f64 {
        match self.axis {
            Axis::Rows => 409.0,
            Axis::Columns => 255.0,
        }
    }

    /// The size typed, when Excel takes it.
    pub fn size(&self) -> Option<f64> {
        let value: f64 = self.value.text.trim().parse().ok()?;
        (value.is_finite() && (0.0..=self.most()).contains(&value)).then_some(value)
    }

    pub fn key(&mut self, key: &str, command: bool) -> SizeKey {
        if self.value.marked.is_some() {
            return SizeKey::Unhandled;
        }
        match key {
            "enter" => SizeKey::Take,
            "escape" => SizeKey::Close,
            "a" if command => {
                self.value.whole = true;
                SizeKey::Handled
            }
            key if self.value.edit_key(key) => {
                self.problem = false;
                SizeKey::Handled
            }
            _ => SizeKey::Unhandled,
        }
    }

    /// Sizes the rows or columns as typed; `None` (the dialog saying so)
    /// when it is not a size Excel takes.
    pub fn apply<B: Backend>(&mut self, book: &mut Workbook<B>) -> Option<Result<bool, String>> {
        let Some(size) = self.size() else {
            self.problem = true;
            return None;
        };
        Some(book.set_axis_size(self.axis, self.first, self.last, size))
    }
}

impl<B: Backend> Workbook<B> {
    /// A row's height in points or a column's width in characters, as
    /// Excel's Row Height and Column Width dialogs show them.
    pub fn axis_size(&self, axis: Axis, index: u32) -> f64 {
        let px = self.axis_size_px(axis, index);
        match axis {
            Axis::Rows => (px * 72.0 / 96.0 * 4.0).round() / 4.0,
            Axis::Columns => {
                let chars =
                    jade_xlsx_layout::units::column_width_px_to_chars(px, self.max_digit_width());
                (f64::from(chars) * 100.0).round() / 100.0
            }
        }
    }

    /// The rows `first..=last` made `size` points high, or the columns that
    /// many characters wide. One undo step; `false` when they already are.
    pub fn set_axis_size(
        &mut self,
        axis: Axis,
        first: u32,
        last: u32,
        size: f64,
    ) -> Result<bool, String> {
        if (first..=last.min(first + 1_000)).all(|index| self.axis_size(axis, index) == size)
            && last - first <= 1_000
        {
            return Ok(false);
        }
        let name = self.sheet_name();
        self.engine_mut()
            .set_axis_size(&name, axis, first, last, size as f32)
            .map_err(|error| error.to_string())?;
        self.restructured()
    }

    /// AutoFit Row Height, or a row's bottom edge double-clicked: the rows
    /// as high as what they hold. One undo step.
    pub fn auto_fit_rows(&mut self, first: u32, last: u32) -> Result<bool, String> {
        let sizes = {
            let sheet = &self.engine().workbook().sheets[self.sheet_index()];
            jade_xlsx_layout::resize::auto_fit_rows(
                sheet,
                &self.engine().workbook().styles,
                first,
                last,
                &self.layout_options(),
            )
        };
        if sizes.is_empty() {
            return Ok(false);
        }
        let name = self.sheet_name();
        self.engine_mut()
            .set_axis_sizes(&name, Axis::Rows, &sizes)
            .map_err(|error| error.to_string())?;
        self.restructured()
    }
}

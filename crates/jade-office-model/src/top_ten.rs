//! A filter button's Top 10 dialog, as Excel's: the largest or smallest
//! numbers, how many, and items or a percentage of them.

use crate::sheet::{FilterButton, Workbook};
use crate::Backend;

/// What a key did in the dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopTenKey {
    Take,
    Close,
    Handled,
    Unhandled,
}

/// The Top 10 dialog for one filter button.
pub struct TopTenDialog {
    pub button: FilterButton,
    /// Top (true) or Bottom.
    pub top: bool,
    pub count: u32,
    /// Percent rather than items.
    pub percent: bool,
}

impl TopTenDialog {
    /// Excel's start: Top 10 Items.
    pub fn new(button: FilterButton) -> Self {
        Self {
            button,
            top: true,
            count: 10,
            percent: false,
        }
    }

    /// The count up or down by `by`, between 1 and Excel's 500 items (or
    /// 100 percent).
    pub fn step(&mut self, by: i32) {
        let most = if self.percent { 100 } else { 500 };
        self.count = (self.count as i32 + by).clamp(1, most) as u32;
    }

    /// Items or percent, the count held to what that allows.
    pub fn set_percent(&mut self, percent: bool) {
        self.percent = percent;
        self.step(0);
    }

    /// The dialog's keys: Enter filters, Escape closes, Up and Down change
    /// the count.
    pub fn key(&mut self, key: &str) -> TopTenKey {
        match key {
            "enter" => TopTenKey::Take,
            "escape" => TopTenKey::Close,
            "up" => {
                self.step(1);
                TopTenKey::Handled
            }
            "down" => {
                self.step(-1);
                TopTenKey::Handled
            }
            _ => TopTenKey::Unhandled,
        }
    }

    /// Filters `book` as the dialog says.
    pub fn apply<B: Backend>(&self, book: &mut Workbook<B>) -> Result<bool, String> {
        book.filter_rank(&self.button, self.count, self.top, self.percent)
    }
}

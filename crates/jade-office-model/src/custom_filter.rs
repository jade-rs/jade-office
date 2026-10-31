//! A filter button's Custom Filter, as Excel's Custom AutoFilter asks for
//! it: a comparison — equals, greater than, begins with, contains and the
//! rest — and the value typed to compare with. The text comparisons are
//! wildcards (`abc*`, `*abc*`), as Excel writes them.

use jade_xlsx_headless_api::{AutoFilterChange, CustomFilterCondition, FilterComparisonOperator};

use crate::sheet::{FilterButton, Workbook};
use crate::text::TextEdit;
use crate::Backend;

/// The comparisons the dialog offers, in Excel's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterTest {
    Equals,
    NotEquals,
    Greater,
    GreaterOrEqual,
    Less,
    LessOrEqual,
    BeginsWith,
    EndsWith,
    Contains,
    NotContains,
}

impl FilterTest {
    pub const ALL: [FilterTest; 10] = [
        FilterTest::Equals,
        FilterTest::NotEquals,
        FilterTest::Greater,
        FilterTest::GreaterOrEqual,
        FilterTest::Less,
        FilterTest::LessOrEqual,
        FilterTest::BeginsWith,
        FilterTest::EndsWith,
        FilterTest::Contains,
        FilterTest::NotContains,
    ];

    /// The i18n key of its name.
    pub fn label(self) -> &'static str {
        match self {
            FilterTest::Equals => "filter.custom.equals",
            FilterTest::NotEquals => "filter.custom.not_equals",
            FilterTest::Greater => "filter.custom.greater",
            FilterTest::GreaterOrEqual => "filter.custom.greater_or_equal",
            FilterTest::Less => "filter.custom.less",
            FilterTest::LessOrEqual => "filter.custom.less_or_equal",
            FilterTest::BeginsWith => "filter.custom.begins_with",
            FilterTest::EndsWith => "filter.custom.ends_with",
            FilterTest::Contains => "filter.custom.contains",
            FilterTest::NotContains => "filter.custom.not_contains",
        }
    }

    /// The condition the file stores for it, against `value`.
    pub fn condition(self, value: &str) -> CustomFilterCondition {
        use FilterComparisonOperator as Op;
        let (operator, value) = match self {
            FilterTest::Equals => (Op::Equal, value.to_string()),
            FilterTest::NotEquals => (Op::NotEqual, value.to_string()),
            FilterTest::Greater => (Op::GreaterThan, value.to_string()),
            FilterTest::GreaterOrEqual => (Op::GreaterThanOrEqual, value.to_string()),
            FilterTest::Less => (Op::LessThan, value.to_string()),
            FilterTest::LessOrEqual => (Op::LessThanOrEqual, value.to_string()),
            FilterTest::BeginsWith => (Op::Equal, format!("{value}*")),
            FilterTest::EndsWith => (Op::Equal, format!("*{value}")),
            FilterTest::Contains => (Op::Equal, format!("*{value}*")),
            FilterTest::NotContains => (Op::NotEqual, format!("*{value}*")),
        };
        CustomFilterCondition { operator, value }
    }
}

/// What a key did in the dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CustomFilterKey {
    Take,
    Close,
    Handled,
    Unhandled,
}

/// The Custom Filter dialog for one filter button.
pub struct CustomFilterDialog {
    pub button: FilterButton,
    pub test: FilterTest,
    pub value: TextEdit,
}

impl CustomFilterDialog {
    pub fn new(button: FilterButton) -> Self {
        Self {
            button,
            test: FilterTest::Equals,
            value: TextEdit::new(String::new()),
        }
    }

    /// The dialog's keys: Enter filters, Escape closes, the rest edit the
    /// value.
    pub fn key(&mut self, key: &str, command: bool) -> CustomFilterKey {
        if self.value.marked.is_some() {
            return CustomFilterKey::Unhandled;
        }
        match key {
            "enter" => CustomFilterKey::Take,
            "escape" => CustomFilterKey::Close,
            "a" if command => {
                self.value.whole = true;
                CustomFilterKey::Handled
            }
            key if self.value.edit_key(key) => CustomFilterKey::Handled,
            _ => CustomFilterKey::Unhandled,
        }
    }
}

impl<B: Backend> Workbook<B> {
    /// Custom Filter: only the rows whose cell in the button's column passes
    /// `test` against `value` show. Nothing typed, nothing filtered.
    pub fn filter_custom(
        &mut self,
        button: &FilterButton,
        test: FilterTest,
        value: &str,
    ) -> Result<bool, String> {
        let value = value.trim();
        if value.is_empty() {
            return Ok(false);
        }
        let change = AutoFilterChange::Custom {
            target: Self::filter_target(button),
            reference: None,
            column: button.column,
            conditions: vec![test.condition(value)],
            and: false,
        };
        self.change_filter(&change)
    }
}

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

/// The Custom Filter dialog for one filter button: a condition, and a
/// second one joined to it by And or Or, as Excel's.
pub struct CustomFilterDialog {
    pub button: FilterButton,
    pub test: FilterTest,
    pub value: TextEdit,
    pub second_test: FilterTest,
    pub second: TextEdit,
    /// And (true) or Or between the two.
    pub and: bool,
    /// Typing goes into the second value.
    pub in_second: bool,
}

impl CustomFilterDialog {
    pub fn new(button: FilterButton) -> Self {
        Self {
            button,
            test: FilterTest::Equals,
            value: TextEdit::new(String::new()),
            second_test: FilterTest::Equals,
            second: TextEdit::new(String::new()),
            and: true,
            in_second: false,
        }
    }

    /// The value typing goes into.
    pub fn field(&self) -> &TextEdit {
        if self.in_second {
            &self.second
        } else {
            &self.value
        }
    }

    pub fn field_mut(&mut self) -> &mut TextEdit {
        if self.in_second {
            &mut self.second
        } else {
            &mut self.value
        }
    }

    /// The dialog's keys: Enter filters, Escape closes, Tab goes between the
    /// two values, the rest edit the one typed into.
    pub fn key(&mut self, key: &str, command: bool) -> CustomFilterKey {
        if self.field().marked.is_some() {
            return CustomFilterKey::Unhandled;
        }
        match key {
            "enter" => CustomFilterKey::Take,
            "escape" => CustomFilterKey::Close,
            "tab" => {
                self.in_second = !self.in_second;
                CustomFilterKey::Handled
            }
            "a" if command => {
                self.field_mut().whole = true;
                CustomFilterKey::Handled
            }
            key if self.field_mut().edit_key(key) => CustomFilterKey::Handled,
            _ => CustomFilterKey::Unhandled,
        }
    }

    /// Filters `book` as the dialog says.
    pub fn apply<B: Backend>(&self, book: &mut Workbook<B>) -> Result<bool, String> {
        book.filter_custom_pair(
            &self.button,
            (self.test, &self.value.text),
            Some((self.second_test, &self.second.text)),
            self.and,
        )
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
        self.filter_custom_pair(button, (test, value), None, false)
    }

    /// Custom Filter with a second condition, joined by And (`and`) or Or.
    /// A condition with nothing typed is left out, as Excel leaves it.
    pub fn filter_custom_pair(
        &mut self,
        button: &FilterButton,
        first: (FilterTest, &str),
        second: Option<(FilterTest, &str)>,
        and: bool,
    ) -> Result<bool, String> {
        let conditions: Vec<CustomFilterCondition> = std::iter::once(first)
            .chain(second)
            .filter_map(|(test, value)| {
                let value = value.trim();
                (!value.is_empty()).then(|| test.condition(value))
            })
            .collect();
        if conditions.is_empty() {
            return Ok(false);
        }
        let change = AutoFilterChange::Custom {
            target: Self::filter_target(button),
            reference: None,
            column: button.column,
            and: and && conditions.len() == 2,
            conditions,
        };
        self.change_filter(&change)
    }
}

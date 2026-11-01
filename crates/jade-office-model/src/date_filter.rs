//! A filter button's date filter, as Excel's list groups a date column:
//! the years and months its dates fall in, and the rows of those ticked —
//! and Date Filters ▸ Today, This Month… and Above/Below Average, worked
//! out afresh whenever the filter is applied.

use jade_xlsx_headless_api::{AutoFilterChange, DateFilterGroup, DateFilterGrouping};
use jade_xlsx_reader::{CellRef, DateSystem};

use crate::sheet::{FilterButton, Workbook};
use crate::Backend;

/// The year (and month, 1–12) a date serial falls on, by the workbook's
/// date system; `None` for one before the epoch.
pub fn year_month(serial: f64, dates: DateSystem) -> Option<(u32, u32)> {
    if !serial.is_finite() || serial < 1.0 {
        return None;
    }
    // Days from 1970-01-01: Excel's 1900 system counts from 1899-12-30
    // (its phantom 29 February 1900 is before every date that matters),
    // the Mac's 1904 system from 1904-01-01.
    let days = serial.floor() as i64
        + match dates {
            DateSystem::Nineteen04 => -24107,
            _ => -25569,
        };
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    u32::try_from(year).ok().map(|year| (year, month as u32))
}

/// The Date Filters a filter button offers, as Excel's `dynamicFilter`
/// types with the i18n keys of their names.
pub const DATE_FILTERS: [(&str, &str); 8] = [
    ("today", "filter.dynamic.today"),
    ("yesterday", "filter.dynamic.yesterday"),
    ("thisWeek", "filter.dynamic.this_week"),
    ("thisMonth", "filter.dynamic.this_month"),
    ("lastMonth", "filter.dynamic.last_month"),
    ("thisQuarter", "filter.dynamic.this_quarter"),
    ("thisYear", "filter.dynamic.this_year"),
    ("yearToDate", "filter.dynamic.year_to_date"),
];

impl<B: Backend> Workbook<B> {
    /// A dynamic filter on the button's column: `kind` is the
    /// `dynamicFilter` type (`today`, `thisMonth`, `aboveAverage`…).
    pub fn filter_dynamic(&mut self, button: &FilterButton, kind: &str) -> Result<bool, String> {
        let change = AutoFilterChange::Dynamic {
            target: Self::filter_target(button),
            reference: None,
            column: button.column,
            kind: kind.to_string(),
        };
        self.change_filter(&change)
    }

    /// The years a filter button's column has dates in, each with the
    /// months it has them in, both in order: what the date filter lists.
    pub fn filter_date_groups(&self, button: &FilterButton) -> Vec<(u32, Vec<u32>)> {
        let column = button.range.start.column + button.column;
        let dates = self.date_system();
        let mut groups: std::collections::BTreeMap<u32, std::collections::BTreeSet<u32>> =
            Default::default();
        for row in button.range.start.row + 1..=button.range.end.row {
            let at = CellRef::new(column, row);
            let Some(serial) = self.number(at) else {
                continue;
            };
            let is_date = self
                .format_at(at)
                .number_format
                .as_deref()
                .is_some_and(jade_xlsx_layout::number_format::is_date_format);
            if !is_date {
                continue;
            }
            if let Some((year, month)) = year_month(serial, dates) {
                groups.entry(year).or_default().insert(month);
            }
        }
        groups
            .into_iter()
            .map(|(year, months)| (year, months.into_iter().collect()))
            .collect()
    }

    /// The date filter: only the rows whose date in the button's column is
    /// in `year` — or, given a month, in that month of it.
    pub fn filter_date(
        &mut self,
        button: &FilterButton,
        year: u32,
        month: Option<u32>,
    ) -> Result<bool, String> {
        self.filter_dates(button, &[(year, month)])
    }

    /// The date filter with several years and months ticked: the rows whose
    /// date falls in any of them show.
    pub fn filter_dates(
        &mut self,
        button: &FilterButton,
        picks: &[(u32, Option<u32>)],
    ) -> Result<bool, String> {
        let groups = picks
            .iter()
            .map(|&(year, month)| DateFilterGroup {
                grouping: if month.is_some() {
                    DateFilterGrouping::Month
                } else {
                    DateFilterGrouping::Year
                },
                year,
                month,
                day: None,
                hour: None,
                minute: None,
                second: None,
            })
            .collect();
        let change = AutoFilterChange::Dates {
            target: Self::filter_target(button),
            reference: None,
            column: button.column,
            groups,
            include_blank: false,
        };
        self.change_filter(&change)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serials_fall_on_their_years_and_months() {
        // 45306 is 2024-01-15 in the 1900 system.
        assert_eq!(year_month(45306.0, DateSystem::Nineteen00), Some((2024, 1)));
        assert_eq!(year_month(45351.5, DateSystem::Nineteen00), Some((2024, 2)));
        assert_eq!(year_month(43831.0, DateSystem::Nineteen00), Some((2020, 1)));
        // The same day in the Mac's 1904 system is 1462 fewer.
        assert_eq!(
            year_month(45306.0 - 1462.0, DateSystem::Nineteen04),
            Some((2024, 1))
        );
        assert_eq!(year_month(0.0, DateSystem::Nineteen00), None);
    }
}

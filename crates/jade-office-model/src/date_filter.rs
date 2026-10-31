//! A filter button's date filter, as Excel's list groups a date column:
//! the years and months its dates fall in, and the rows of one of them.

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

impl<B: Backend> Workbook<B> {
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
        let group = DateFilterGroup {
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
        };
        let change = AutoFilterChange::Dates {
            target: Self::filter_target(button),
            reference: None,
            column: button.column,
            groups: vec![group],
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

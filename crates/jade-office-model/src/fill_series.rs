//! Home ▸ Fill ▸ Series…, as Excel's dialog: a series down each column or
//! across each row of the selection — linear, growth, by date (day,
//! weekday, month or year) or AutoFill — with a step, an optional stop
//! value and, for linear and growth, a trend fitted to the values there.

use jade_xlsx_headless_api::CellInput;
use jade_xlsx_reader::{CellRange, CellRef};

use crate::sheet::{range_text_full, reference_text, Workbook};
use crate::text::TextEdit;
use crate::Backend;

/// Which way each series runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeriesIn {
    /// Across each row of the selection.
    Rows,
    /// Down each column.
    Columns,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeriesType {
    /// Each value the one before plus the step.
    Linear,
    /// Each value the one before times the step.
    Growth,
    /// Dates a step of the date unit apart.
    Date,
    /// What dragging the fill handle from the first cell gives.
    AutoFill,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateUnit {
    Day,
    /// Days skipping Saturdays and Sundays.
    Weekday,
    Month,
    Year,
}

/// What the Series dialog asks for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeriesOptions {
    pub series_in: SeriesIn,
    pub kind: SeriesType,
    pub unit: DateUnit,
    pub step: f64,
    /// Where the series stops: no value past it is written.
    pub stop: Option<f64>,
    /// Linear and growth: a best-fit line (or exponential curve) through
    /// the values already there, replacing them, the step ignored.
    pub trend: bool,
}

impl SeriesOptions {
    /// Excel's start for `range`: rows when it is wider than it is tall,
    /// columns otherwise, linear (dates for a date) by 1 with no stop.
    pub fn for_range<B: Backend>(book: &Workbook<B>, range: CellRange) -> Self {
        let wide = range.end.column - range.start.column > range.end.row - range.start.row;
        let date = book.shows_a_date(range.start);
        Self {
            series_in: if wide {
                SeriesIn::Rows
            } else {
                SeriesIn::Columns
            },
            kind: if date {
                SeriesType::Date
            } else {
                SeriesType::Linear
            },
            unit: DateUnit::Day,
            step: 1.0,
            stop: None,
            trend: false,
        }
    }
}

/// The far end a series going `series_in` from `at` has to reach to get to
/// `stop`: the cells the dialog fills when only one is selected.
fn reach_stop(at: CellRef, series_in: SeriesIn, count: u32) -> CellRange {
    let end = match series_in {
        SeriesIn::Rows => CellRef::new((at.column + count).min(16_383), at.row),
        SeriesIn::Columns => CellRef::new(at.column, (at.row + count).min(1_048_575)),
    };
    CellRange { start: at, end }
}

/// The most cells a single selected cell's series runs to its stop over.
const MOST: u32 = 100_000;

impl<B: Backend> Workbook<B> {
    /// Whether `at`'s number format shows a date.
    pub(crate) fn shows_a_date(&self, at: CellRef) -> bool {
        self.number(at).is_some() && self.format_at(at).number_format.as_deref().is_some_and(is_date_code)
    }

    /// Fill ▸ Series over `range` as `options` say, one undo step; `false`
    /// when there was no number to start from anywhere. One cell selected
    /// runs on until the stop value, as in Excel.
    pub fn fill_series_dialog(
        &mut self,
        range: CellRange,
        options: &SeriesOptions,
    ) -> Result<bool, String> {
        if options.kind != SeriesType::AutoFill && !options.step.is_finite() {
            return Err("the step value is not a number".into());
        }
        let range = if range.start == range.end {
            match options.stop {
                Some(stop) if options.kind != SeriesType::AutoFill => {
                    let Some(first) = self.number(range.start) else {
                        return Ok(false);
                    };
                    let count = self
                        .series_values(first, options, MOST, &[first])
                        .into_iter()
                        .take_while(|value| !past(*value, stop, options))
                        .count() as u32;
                    reach_stop(range.start, options.series_in, count.saturating_sub(1))
                }
                _ => return Ok(false),
            }
        } else {
            range
        };
        self.atomic_edit(|book| {
            let name = book.sheet_name();
            let mut steps = 0;
            for line in lines(range, options.series_in) {
                steps += book.fill_line(&name, &line, options)?;
            }
            if steps == 0 {
                return Ok(false);
            }
            book.wrote(steps)?;
            Ok(true)
        })
    }

    /// One row or column of the series: the undo steps it took.
    fn fill_line(
        &mut self,
        name: &str,
        line: &[CellRef],
        options: &SeriesOptions,
    ) -> Result<usize, String> {
        let first = line[0];
        let whole = CellRange {
            start: first,
            end: *line.last().unwrap(),
        };
        if options.kind == SeriesType::AutoFill {
            // The fill handle dragged from the cells filled at the start.
            let known = line
                .iter()
                .take_while(|at| !self.edit_text(**at).is_empty())
                .count();
            if known == 0 || known == line.len() {
                return Ok(0);
            }
            let source = CellRange {
                start: first,
                end: line[known - 1],
            };
            let summary = self
                .engine_mut()
                .fill(name, &range_text_full(source), &range_text_full(whole))
                .map_err(|error| error.to_string())?;
            return Ok(usize::from(!summary.cells.is_empty()));
        }
        let known: Vec<f64> = line.iter().map_while(|at| self.number(*at)).collect();
        let Some(&start) = known.first() else {
            return Ok(0);
        };
        let values = self.series_values(start, options, line.len() as u32, &known);
        let mut steps = 0;
        let mut written = first;
        for (index, (&at, value)) in line.iter().zip(values).enumerate() {
            // The first cell starts the series; a trend replaces it too.
            if index == 0 && !(options.trend && options.kind != SeriesType::Date) {
                continue;
            }
            if options.stop.is_some_and(|stop| past(value, stop, options)) {
                break;
            }
            self.engine_mut()
                .set(name, &reference_text(at), CellInput::Number(value))
                .map_err(|error| error.to_string())?;
            steps += 1;
            written = at;
        }
        // The values take the first cell's format, as Excel's do.
        if written != first {
            let source = CellRange {
                start: first,
                end: first,
            };
            let target = CellRange {
                start: line[1],
                end: written,
            };
            let summary = self
                .engine_mut()
                .copy_format(name, &range_text_full(source), &range_text_full(target))
                .map_err(|error| error.to_string())?;
            if !summary.cells.is_empty() {
                steps += 1;
            }
        }
        Ok(steps)
    }

    /// The first `count` values of the series that starts at `start`,
    /// `known` being the numbers at its start (for a trend).
    fn series_values(
        &self,
        start: f64,
        options: &SeriesOptions,
        count: u32,
        known: &[f64],
    ) -> Vec<f64> {
        let date1904 = matches!(
            self.date_system(),
            jade_xlsx_reader::DateSystem::Nineteen04
        );
        let step = options.step;
        (0..count)
            .map(|k| {
                let k = f64::from(k);
                match options.kind {
                    SeriesType::Linear if options.trend => {
                        let (intercept, slope) = least_squares(known, step);
                        intercept + slope * k
                    }
                    SeriesType::Growth if options.trend => {
                        let logs: Vec<f64> = known.iter().map(|value| value.ln()).collect();
                        if logs.iter().all(|value| value.is_finite()) {
                            let (intercept, slope) = least_squares(&logs, step.ln());
                            (intercept + slope * k).exp()
                        } else {
                            start * step.powf(k)
                        }
                    }
                    SeriesType::Linear | SeriesType::AutoFill => start + step * k,
                    SeriesType::Growth => start * step.powf(k),
                    SeriesType::Date => {
                        add_dates(start, (step * k).round() as i64, options.unit, date1904)
                    }
                }
            })
            .map(round_to_15_digits)
            .collect()
    }
}

/// Whether `value` is past the stop, in the direction the series runs.
fn past(value: f64, stop: f64, options: &SeriesOptions) -> bool {
    let falling = match options.kind {
        SeriesType::Growth => options.step < 1.0,
        _ => options.step < 0.0,
    };
    if falling {
        value < stop
    } else {
        value > stop
    }
}

/// The cells of each series: the selection's rows or columns.
fn lines(range: CellRange, series_in: SeriesIn) -> Vec<Vec<CellRef>> {
    match series_in {
        SeriesIn::Rows => (range.start.row..=range.end.row)
            .map(|row| {
                (range.start.column..=range.end.column)
                    .map(|column| CellRef::new(column, row))
                    .collect()
            })
            .collect(),
        SeriesIn::Columns => (range.start.column..=range.end.column)
            .map(|column| {
                (range.start.row..=range.end.row)
                    .map(|row| CellRef::new(column, row))
                    .collect()
            })
            .collect(),
    }
}

/// The best-fit line through `values` at 0, 1, 2…, as (intercept, slope);
/// one value is a line through it with slope `step`, as Excel's trend has.
fn least_squares(values: &[f64], step: f64) -> (f64, f64) {
    if values.len() < 2 {
        return (values.first().copied().unwrap_or(0.0), step);
    }
    let n = values.len() as f64;
    let mean_x = (n - 1.0) / 2.0;
    let mean_y = values.iter().sum::<f64>() / n;
    let (mut top, mut bottom) = (0.0, 0.0);
    for (x, y) in values.iter().enumerate() {
        let dx = x as f64 - mean_x;
        top += dx * (y - mean_y);
        bottom += dx * dx;
    }
    let slope = top / bottom;
    (mean_y - slope * mean_x, slope)
}

/// Excel keeps fifteen significant digits: 0.1 stepped three times is 0.3.
fn round_to_15_digits(value: f64) -> f64 {
    if value == 0.0 || !value.is_finite() {
        return value;
    }
    let digits = 14 - value.abs().log10().floor() as i32;
    if !(0..=300).contains(&digits) {
        return value;
    }
    let scale = 10f64.powi(digits);
    (value * scale).round() / scale
}

/// Whether a number format code shows a date: a day or year outside any
/// quoted text, colour or locale in brackets.
pub(crate) fn is_date_code(code: &str) -> bool {
    let mut quoted = false;
    let mut bracket = false;
    let mut escaped = false;
    for c in code.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' => escaped = true,
            '"' => quoted = !quoted,
            '[' if !quoted => bracket = true,
            ']' if !quoted => bracket = false,
            'd' | 'D' | 'y' | 'Y' | 'e' | 'g' if !quoted && !bracket => return true,
            _ => {}
        }
    }
    false
}

/// `serial` (a date) moved `by` days, weekdays, months or years. A month
/// or year that has no such day takes its last, as Excel's series does.
fn add_dates(serial: f64, by: i64, unit: DateUnit, date1904: bool) -> f64 {
    let time = serial.fract();
    let day = serial.trunc() as i64;
    // Days since 1970-01-01 from Excel's serial, past its 1900 leap day.
    let epoch = if date1904 { 24_107 } else { 25_569 };
    let to_civil = |day: i64| {
        let mut unix = day - epoch;
        if !date1904 && day < 61 {
            unix += 1;
        }
        civil_from_days(unix)
    };
    let to_serial = |year: i64, month: u32, dom: u32| {
        let unix = days_from_civil(year, month, dom);
        let mut day = unix + epoch;
        if !date1904 && day < 61 {
            day -= 1;
        }
        day
    };
    let moved = match unit {
        DateUnit::Day => day + by,
        DateUnit::Weekday => {
            let mut at = day;
            let mut left = by.abs();
            let forward = by >= 0;
            while left > 0 {
                at += if forward { 1 } else { -1 };
                // 1970-01-01 was a Thursday.
                let weekday = (to_civil_unix(at, epoch, date1904) + 3).rem_euclid(7);
                if weekday < 5 {
                    left -= 1;
                }
            }
            at
        }
        DateUnit::Month | DateUnit::Year => {
            let (year, month, dom) = to_civil(day);
            let months = if unit == DateUnit::Month { by } else { by * 12 };
            let total = year * 12 + i64::from(month - 1) + months;
            let (year, month) = (total.div_euclid(12), (total.rem_euclid(12) + 1) as u32);
            to_serial(year, month, dom.min(days_in_month(year, month)))
        }
    };
    moved as f64 + time
}

/// Days since 1970-01-01 of Excel's serial `day`.
fn to_civil_unix(day: i64, epoch: i64, date1904: bool) -> i64 {
    let mut unix = day - epoch;
    if !date1904 && day < 61 {
        unix += 1;
    }
    unix
}

fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days since 1970-01-01 of a proleptic Gregorian date.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let day_of_year =
        (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The date `days` after 1970-01-01, as (year, month, day).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// Which box of the dialog has the keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeriesField {
    Step,
    Stop,
}

/// What a key did in the dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeriesKey {
    Take,
    Close,
    Handled,
    Unhandled,
}

/// The Series dialog over a selection.
pub struct SeriesDialog {
    pub range: CellRange,
    pub series_in: SeriesIn,
    pub kind: SeriesType,
    pub unit: DateUnit,
    pub trend: bool,
    pub step: TextEdit,
    pub stop: TextEdit,
    pub field: SeriesField,
    /// The step or stop is not a number: OK keeps the dialog open.
    pub problem: bool,
}

impl SeriesDialog {
    pub fn open<B: Backend>(book: &Workbook<B>, range: CellRange) -> Self {
        let options = SeriesOptions::for_range(book, range);
        Self {
            range,
            series_in: options.series_in,
            kind: options.kind,
            unit: options.unit,
            trend: false,
            step: TextEdit::selected("1".to_string()),
            stop: TextEdit::new(String::new()),
            field: SeriesField::Step,
            problem: false,
        }
    }

    pub fn field_mut(&mut self) -> &mut TextEdit {
        match self.field {
            SeriesField::Step => &mut self.step,
            SeriesField::Stop => &mut self.stop,
        }
    }

    pub fn field(&self) -> &TextEdit {
        match self.field {
            SeriesField::Step => &self.step,
            SeriesField::Stop => &self.stop,
        }
    }

    /// What OK fills with; `None` when the step or a stop typed is not a
    /// number (a date in the stop box counts, as in Excel).
    pub fn options(&self, date1904: bool) -> Option<SeriesOptions> {
        let read = |text: &str| -> Option<f64> {
            match crate::typing::read(text.trim(), date1904).input {
                CellInput::Number(value) => Some(value),
                _ => None,
            }
        };
        let step = if self.kind == SeriesType::AutoFill {
            1.0
        } else {
            read(&self.step.text)?
        };
        let stop = if self.stop.text.trim().is_empty() {
            None
        } else {
            Some(read(&self.stop.text)?)
        };
        Some(SeriesOptions {
            series_in: self.series_in,
            kind: self.kind,
            unit: self.unit,
            step,
            stop,
            trend: self.trend && matches!(self.kind, SeriesType::Linear | SeriesType::Growth),
        })
    }

    /// Enter fills, Escape closes, Tab moves between the boxes, and the
    /// rest edit the box with the keys.
    pub fn key(&mut self, key: &str, command: bool) -> SeriesKey {
        if self.field().marked.is_some() {
            return SeriesKey::Unhandled;
        }
        match key {
            "enter" => SeriesKey::Take,
            "escape" => SeriesKey::Close,
            "tab" => {
                self.field = match self.field {
                    SeriesField::Step => SeriesField::Stop,
                    SeriesField::Stop => SeriesField::Step,
                };
                SeriesKey::Handled
            }
            "a" if command => {
                self.field_mut().whole = true;
                SeriesKey::Handled
            }
            key if self.field_mut().edit_key(key) => {
                self.problem = false;
                SeriesKey::Handled
            }
            _ => SeriesKey::Unhandled,
        }
    }

    /// Fills `book` as the dialog says; `Err` with nothing written when a
    /// box holds something that is not a number.
    pub fn apply<B: Backend>(&mut self, book: &mut Workbook<B>) -> Option<Result<bool, String>> {
        let date1904 = matches!(book.date_system(), jade_xlsx_reader::DateSystem::Nineteen04);
        let Some(options) = self.options(date1904) else {
            self.problem = true;
            return None;
        };
        Some(book.fill_series_dialog(self.range, &options))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_move_by_months_to_the_last_day_when_short() {
        // 2024-01-31 is 45322; a month on is 2024-02-29 (45351).
        assert_eq!(add_dates(45322.0, 1, DateUnit::Month, false), 45351.0);
        assert_eq!(add_dates(45322.0, 12, DateUnit::Month, false), 45688.0);
        assert_eq!(add_dates(45322.0, 1, DateUnit::Year, false), 45688.0);
        // 2024-01-05 is a Friday: one weekday on is Monday the 8th.
        assert_eq!(add_dates(45296.0, 1, DateUnit::Weekday, false), 45299.0);
        assert_eq!(add_dates(45299.0, -1, DateUnit::Weekday, false), 45296.0);
        assert_eq!(add_dates(45296.0, 7, DateUnit::Day, false), 45303.0);
    }

    #[test]
    fn date_codes_are_told_from_others() {
        assert!(is_date_code("yyyy/m/d"));
        assert!(is_date_code("[$-411]ggge\"年\"m\"月\"d\"日\""));
        assert!(!is_date_code("0.00"));
        assert!(!is_date_code("\"day\" 0"));
        assert!(!is_date_code("[Red]0"));
    }

    #[test]
    fn a_trend_fits_the_values_there() {
        assert_eq!(least_squares(&[1.0, 3.0, 5.0], 1.0), (1.0, 2.0));
        assert_eq!(least_squares(&[4.0], 3.0), (4.0, 3.0));
        assert_eq!(round_to_15_digits(0.1 + 0.2), 0.3);
    }
}

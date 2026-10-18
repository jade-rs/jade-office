//! Typed text, read the way Excel reads it.
//!
//! Excel turns what looks like a number into one — `10%`, `1,234`,
//! `$1,234.50`, `¥1,000`, `(100)`, `2024/1/15`, `12:30` — and gives a
//! General cell the format the text was written in, so it shows as typed
//! and still adds up. A leading apostrophe keeps text as text. Anything not
//! quite one of these stays the text it was.

use jade_xlsx_headless_api::CellInput;

/// What typed text becomes: the value, and the number format to give the
/// cell when it is still General.
#[derive(Debug, Clone, PartialEq)]
pub struct Typed {
    pub input: CellInput,
    pub format: Option<&'static str>,
}

impl Typed {
    fn plain(input: CellInput) -> Self {
        Self {
            input,
            format: None,
        }
    }

    fn number(value: f64, format: Option<&'static str>) -> Self {
        Self {
            input: CellInput::Number(value),
            format,
        }
    }
}

/// Reads `text` as typed into a cell. `date1904` is the workbook's date
/// system, which dates count from.
pub fn read(text: &str, date1904: bool) -> Typed {
    if let Some(formula) = text.strip_prefix('=').filter(|formula| !formula.is_empty()) {
        return Typed::plain(CellInput::Formula(formula.to_string()));
    }
    if let Some(literal) = text.strip_prefix('\'') {
        return Typed::plain(CellInput::Text(literal.to_string()));
    }
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Typed::plain(if text.is_empty() {
            CellInput::Blank
        } else {
            CellInput::Text(text.to_string())
        });
    }
    if trimmed.eq_ignore_ascii_case("TRUE") {
        return Typed::plain(CellInput::Bool(true));
    }
    if trimmed.eq_ignore_ascii_case("FALSE") {
        return Typed::plain(CellInput::Bool(false));
    }
    read_number(trimmed, date1904)
        .unwrap_or_else(|| Typed::plain(CellInput::Text(text.to_string())))
}

/// Whether `text`, typed back in, would become something other than text —
/// so a cell holding it as text shows it with the apostrophe that keeps it.
pub fn reads_as_value(text: &str) -> bool {
    !matches!(read(text, false).input, CellInput::Text(_))
}

fn read_number(text: &str, date1904: bool) -> Option<Typed> {
    if let Some(value) = plain(text) {
        return Some(Typed::number(value, None));
    }
    if let Some(inner) = text
        .strip_prefix('(')
        .and_then(|rest| rest.strip_suffix(')'))
    {
        let typed = read_number(inner, date1904)?;
        if let CellInput::Number(value) = typed.input {
            return Some(Typed::number(-value, typed.format));
        }
        return None;
    }
    if let Some(inner) = text.strip_suffix('%') {
        let (value, decimals) = grouped(inner.trim_end())?;
        let format = if decimals { "0.00%" } else { "0%" };
        return Some(Typed::number(value / 100.0, Some(format)));
    }
    for (symbol, whole, cents) in [
        ("$", "$#,##0", "$#,##0.00"),
        ("¥", "\\¥#,##0", "\\¥#,##0.00"),
        ("￥", "\\¥#,##0", "\\¥#,##0.00"),
    ] {
        let (sign, rest) = match text.strip_prefix('-') {
            Some(rest) => (-1.0, rest),
            None => (1.0, text),
        };
        if let Some(amount) = rest.strip_prefix(symbol) {
            let (value, decimals) = grouped(amount)?;
            return Some(Typed::number(
                sign * value,
                Some(if decimals { cents } else { whole }),
            ));
        }
    }
    if let Some((value, decimals)) = grouped(text) {
        if text.contains(',') {
            let format = if decimals { "#,##0.00" } else { "#,##0" };
            return Some(Typed::number(value, Some(format)));
        }
    }
    date_time(text, date1904)
}

/// A number as Rust and Excel both read it: digits, a point, an exponent.
fn plain(text: &str) -> Option<f64> {
    let looks_numeric = text
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '.' | '+' | '-' | 'e' | 'E'))
        && text.chars().any(|c| c.is_ascii_digit());
    looks_numeric
        .then(|| text.parse::<f64>().ok())
        .flatten()
        .filter(|value| value.is_finite())
}

/// A number with optional thousands separators in their right places;
/// whether it has a decimal part.
fn grouped(text: &str) -> Option<(f64, bool)> {
    let (sign, digits) = match text.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", text),
    };
    let (whole, fraction) = match digits.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (digits, None),
    };
    if fraction.is_some_and(|fraction| !fraction.chars().all(|c| c.is_ascii_digit())) {
        return None;
    }
    let groups: Vec<&str> = whole.split(',').collect();
    let first = groups.first()?;
    let well_grouped = !first.is_empty()
        && first.len() <= 3
        && first.chars().all(|c| c.is_ascii_digit())
        && groups[1..]
            .iter()
            .all(|group| group.len() == 3 && group.chars().all(|c| c.is_ascii_digit()));
    let ungrouped =
        groups.len() == 1 && whole.chars().all(|c| c.is_ascii_digit()) && !whole.is_empty();
    if !(well_grouped || ungrouped) {
        return None;
    }
    let mut number = format!("{sign}{}", groups.concat());
    if let Some(fraction) = fraction {
        number.push('.');
        number.push_str(fraction);
    }
    let value = number.parse::<f64>().ok()?;
    Some((value, fraction.is_some_and(|fraction| !fraction.is_empty())))
}

/// `yyyy/m/d`, `yyyy-m-d`, either followed by a time, or a time alone
/// (`h:mm`, `h:mm:ss`): its serial and the format Excel gives it.
fn date_time(text: &str, date1904: bool) -> Option<Typed> {
    let (date_part, time_part) = match text.split_once(' ') {
        Some((date, time)) => (Some(date), Some(time.trim())),
        None if text.contains(':') => (None, Some(text)),
        None => (Some(text), None),
    };
    let time = match time_part {
        Some(time) => Some(time_of_day(time)?),
        None => None,
    };
    let date = match date_part {
        Some(date) => Some(serial(date, date1904)?),
        None => None,
    };
    let seconds_shown = time.is_some_and(|(_, seconds)| seconds);
    let fraction = time.map_or(0.0, |(fraction, _)| fraction);
    Some(match (date, time) {
        (Some(day), Some(_)) => Typed::number(
            day + fraction,
            Some(if seconds_shown {
                "yyyy/m/d h:mm:ss"
            } else {
                "yyyy/m/d h:mm"
            }),
        ),
        (Some(day), None) => Typed::number(day, Some("yyyy/m/d")),
        (None, Some(_)) => Typed::number(
            fraction,
            Some(if seconds_shown { "h:mm:ss" } else { "h:mm" }),
        ),
        (None, None) => return None,
    })
}

/// `h:mm` or `h:mm:ss` as a fraction of a day, and whether seconds were
/// written.
fn time_of_day(text: &str) -> Option<(f64, bool)> {
    let parts: Vec<&str> = text.split(':').collect();
    if !(2..=3).contains(&parts.len())
        || parts.iter().any(|part| {
            part.is_empty() || part.len() > 2 || !part.chars().all(|c| c.is_ascii_digit())
        })
    {
        return None;
    }
    let hours: u32 = parts[0].parse().ok()?;
    let minutes: u32 = parts[1].parse().ok()?;
    let seconds: u32 = parts.get(2).map_or(Some(0), |part| part.parse().ok())?;
    if hours >= 24 || minutes >= 60 || seconds >= 60 {
        return None;
    }
    let total = hours * 3600 + minutes * 60 + seconds;
    Some((f64::from(total) / 86_400.0, parts.len() == 3))
}

/// A `yyyy/m/d` or `yyyy-m-d` date's serial in the workbook's system.
fn serial(text: &str, date1904: bool) -> Option<f64> {
    let separator = if text.contains('/') { '/' } else { '-' };
    let parts: Vec<&str> = text.split(separator).collect();
    if parts.len() != 3
        || parts[0].len() != 4
        || parts[1..]
            .iter()
            .any(|part| part.is_empty() || part.len() > 2)
        || !parts
            .iter()
            .all(|part| part.chars().all(|c| c.is_ascii_digit()))
    {
        return None;
    }
    let year: i64 = parts[0].parse().ok()?;
    let month: u32 = parts[1].parse().ok()?;
    let day: u32 = parts[2].parse().ok()?;
    if !(1900..=9999).contains(&year) || !(1..=12).contains(&month) {
        return None;
    }
    if day == 0 || day > days_in_month(year, month) {
        return None;
    }
    // Days since 1899-12-30, which is serial 0 from 1900-03-01 on; Excel's
    // fictional 29 February 1900 makes everything before one lower.
    let mut days = days_from_civil(year, month, day) - days_from_civil(1899, 12, 30);
    if (year, month) < (1900, 3) {
        days -= 1;
    }
    if date1904 {
        days -= 1462;
        if days < 0 {
            return None;
        }
    }
    Some(days as f64)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_count_as_excel_counts_them() {
        assert_eq!(serial("2024/1/15", false), Some(45306.0));
        assert_eq!(serial("1900/3/1", false), Some(61.0));
        assert_eq!(serial("1900/2/28", false), Some(59.0));
        assert_eq!(serial("1900/1/1", false), Some(1.0));
        assert_eq!(serial("2024/1/15", true), Some(43844.0));
        assert_eq!(serial("2023/2/29", false), None);
    }

    #[test]
    fn apostrophes_spaces_and_near_misses() {
        assert_eq!(read("'0123", false).input, CellInput::Text("0123".into()));
        assert_eq!(read("", false).input, CellInput::Blank);
        assert!(reads_as_value("10%"));
        assert!(!reads_as_value("1,2,3"));
        assert!(!reads_as_value("inf"));
    }
}

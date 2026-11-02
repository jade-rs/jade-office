//! Word's spin boxes: number fields that show their unit — `10.5 pt`,
//! `1 字`, `0.5 行`, `25.4 mm`, `100 %` — with ▲▼ beside them.
//!
//! - The arrows, and ↑ / ↓ while the field has the keyboard, step the value
//!   to the next multiple of the field's step, within its range, and the
//!   unit stays.
//! - What is typed may carry another unit: `1 cm` in a pt field, `12pt` in
//!   a 字 field. On commit it is converted to the field's unit — lengths
//!   into each other, 字 by the character width and 行 by the line height
//!   ([`Metrics`]) — rounded to the field's decimals and kept in its range.
//! - What does not read as a number in a unit the field can take is turned
//!   down: the field goes back to the value it had.
//!
//! [`SpinField`] is the field being typed into; the shells draw it and send
//! it their keys.

use crate::text::TextEdit;

/// Points in a millimetre.
const PT_PER_MM: f64 = 72.0 / 25.4;

/// The unit a field shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpinUnit {
    Pt,
    Mm,
    Cm,
    In,
    /// 字: character widths.
    Char,
    /// 行: line heights.
    Line,
    Percent,
    /// A count, with no unit: 段数, 開始番号.
    Count,
}

impl SpinUnit {
    /// The unit as the field shows it after the number.
    pub fn label(self) -> &'static str {
        match self {
            SpinUnit::Pt => "pt",
            SpinUnit::Mm => "mm",
            SpinUnit::Cm => "cm",
            SpinUnit::In => "in",
            SpinUnit::Char => "字",
            SpinUnit::Line => "行",
            SpinUnit::Percent => "%",
            SpinUnit::Count => "",
        }
    }

    /// A unit as it may be typed, in either language and either width.
    pub fn parse(text: &str) -> Option<Self> {
        let unit: String = text
            .trim()
            .chars()
            .map(half_width)
            .collect::<String>()
            .to_lowercase();
        Some(match unit.as_str() {
            "pt" | "pts" | "point" | "points" | "ポイント" | "ﾎﾟｲﾝﾄ" | "ポ" => {
                SpinUnit::Pt
            }
            "mm" | "ミリ" | "ミリメートル" => SpinUnit::Mm,
            "cm" | "センチ" | "センチメートル" => SpinUnit::Cm,
            "in" | "inch" | "inches" | "\"" | "インチ" => SpinUnit::In,
            "字" | "文字" | "ch" | "char" | "chars" => SpinUnit::Char,
            "行" | "li" | "line" | "lines" => SpinUnit::Line,
            "%" => SpinUnit::Percent,
            _ => return None,
        })
    }

    /// Points in one of this unit, where it is a length.
    fn points(self, metrics: Metrics) -> Option<f64> {
        Some(match self {
            SpinUnit::Pt => 1.0,
            SpinUnit::Mm => PT_PER_MM,
            SpinUnit::Cm => PT_PER_MM * 10.0,
            SpinUnit::In => 72.0,
            SpinUnit::Char => metrics.char_pt,
            SpinUnit::Line => metrics.line_pt,
            SpinUnit::Percent | SpinUnit::Count => return None,
        })
    }
}

/// What 字 and 行 are in points where a field converts them: the width of
/// a character of the paragraph's font and the height of its line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    pub char_pt: f64,
    pub line_pt: f64,
}

impl Default for Metrics {
    /// Word's Japanese default: 10.5 pt characters on 18 pt lines.
    fn default() -> Self {
        Self {
            char_pt: 10.5,
            line_pt: 18.0,
        }
    }
}

/// Converts `value` in `from` to `to`, if the two convert.
pub fn convert(value: f64, from: SpinUnit, to: SpinUnit, metrics: Metrics) -> Option<f64> {
    if from == to {
        return Some(value);
    }
    let from = from.points(metrics)?;
    let to = to.points(metrics)?;
    (to != 0.0).then(|| value * from / to)
}

/// Full-width digits, letters, signs and points, as a Japanese IME types
/// them, in their ASCII forms.
fn half_width(ch: char) -> char {
    match ch {
        '\u{FF01}'..='\u{FF5E}' => char::from_u32(ch as u32 - 0xFEE0).unwrap_or(ch),
        '。' => '.',
        '−' => '-',
        '”' | '″' => '"',
        other => other,
    }
}

/// A field: its unit, range, step and decimals.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpinSpec {
    pub unit: SpinUnit,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub decimals: u8,
}

impl SpinSpec {
    pub const fn new(unit: SpinUnit, min: f64, max: f64, step: f64, decimals: u8) -> Self {
        Self {
            unit,
            min,
            max,
            step,
            decimals,
        }
    }

    /// `value` at the field's decimals and in its range.
    pub fn fit(&self, value: f64) -> f64 {
        let scale = 10f64.powi(self.decimals as i32);
        let value = (value * scale).round() / scale;
        let value = value.clamp(self.min, self.max);
        // No "-0 pt".
        if value == 0.0 {
            0.0
        } else {
            value
        }
    }

    /// The number alone, with Word's decimals: `10.5`, `12`, `0.25`.
    pub fn number(&self, value: f64) -> String {
        let text = format!("{:.*}", self.decimals as usize, self.fit(value));
        if text.contains('.') {
            text.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            text
        }
    }

    /// The field's text: `10.5 pt`, `1 字`, `3`.
    pub fn format(&self, value: f64) -> String {
        let number = self.number(value);
        match self.unit {
            SpinUnit::Count => number,
            unit => format!("{number} {}", unit.label()),
        }
    }

    /// What was typed, as a value in the field's unit: a number, then
    /// optionally a unit that converts to the field's. `None` when it is
    /// not one.
    pub fn parse(&self, text: &str, metrics: Metrics) -> Option<f64> {
        let text: String = text.trim().chars().map(half_width).collect();
        let split = text
            .char_indices()
            .find(|&(index, ch)| {
                !(ch.is_ascii_digit() || ch == '.' || ((ch == '-' || ch == '+') && index == 0))
            })
            .map_or(text.len(), |(index, _)| index);
        let (number, unit) = text.split_at(split);
        let value: f64 = number.parse().ok()?;
        if !value.is_finite() {
            return None;
        }
        let unit = unit.trim();
        let value = if unit.is_empty() {
            value
        } else {
            convert(value, SpinUnit::parse(unit)?, self.unit, metrics)?
        };
        Some(self.fit(value))
    }

    /// ▲ or ▼ from `value`: to the next multiple of the step that way, kept
    /// in the range.
    pub fn step(&self, value: f64, up: bool) -> f64 {
        if self.step <= 0.0 {
            return self.fit(value);
        }
        let steps = value / self.step;
        // A value already on the grid moves a whole step; one between moves
        // onto the grid.
        const EPSILON: f64 = 1e-6;
        let next = if up {
            (steps + EPSILON).floor() + 1.0
        } else {
            (steps - EPSILON).ceil() - 1.0
        };
        self.fit(next * self.step)
    }
}

/// What a key did to a field being typed into.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpinKey {
    /// ↑ / ↓: the value stepped; apply it.
    Stepped(f64),
    /// Enter or Tab: what was typed, if it read as a value; the field shows
    /// it (or goes back to the value it had) and lets the keyboard go.
    Committed(Option<f64>),
    /// Escape: back to the value it had.
    Reverted,
    /// An editing key the field took.
    Edited,
    /// Not the field's key.
    Unhandled,
}

/// A spin box being typed into.
pub struct SpinField {
    pub spec: SpinSpec,
    pub metrics: Metrics,
    pub edit: TextEdit,
    /// The value it had when it took the keyboard, or was last committed.
    pub value: f64,
}

impl SpinField {
    /// Takes the keyboard on `value`, all of it selected.
    pub fn focus(spec: SpinSpec, metrics: Metrics, value: f64) -> Self {
        let value = spec.fit(value);
        Self {
            spec,
            metrics,
            edit: TextEdit::selected(spec.format(value)),
            value,
        }
    }

    /// What is typed, as a value, if it reads as one.
    pub fn typed(&self) -> Option<f64> {
        self.spec.parse(&self.edit.text, self.metrics)
    }

    /// One step from what is typed (or the value, when that does not read):
    /// the new value, shown with its unit.
    pub fn step(&mut self, up: bool) -> f64 {
        let from = self.typed().unwrap_or(self.value);
        self.value = self.spec.step(from, up);
        self.edit = TextEdit::selected(self.spec.format(self.value));
        self.value
    }

    /// What is typed becomes the value, shown in the field's unit; when it
    /// does not read, the field goes back to the value it had.
    pub fn commit(&mut self) -> Option<f64> {
        let typed = self.typed();
        if let Some(value) = typed {
            self.value = value;
        }
        self.edit = TextEdit::selected(self.spec.format(self.value));
        typed
    }

    /// A key while the field has the keyboard, by gpui's key name.
    pub fn key(&mut self, key: &str) -> SpinKey {
        match key {
            "up" => SpinKey::Stepped(self.step(true)),
            "down" => SpinKey::Stepped(self.step(false)),
            "enter" | "return" | "tab" => SpinKey::Committed(self.commit()),
            "escape" => {
                self.edit = TextEdit::selected(self.spec.format(self.value));
                SpinKey::Reverted
            }
            "backspace" | "delete" | "left" | "right" | "home" | "end" => {
                self.edit.edit_key(key);
                SpinKey::Edited
            }
            _ => SpinKey::Unhandled,
        }
    }
}

/// The fields Word's dialogs have, by what they hold.
pub mod specs {
    use super::{SpinSpec, SpinUnit};

    /// フォント サイズ: 1 to 1638 pt by halves, stepping a point.
    pub const FONT_SIZE: SpinSpec = SpinSpec::new(SpinUnit::Pt, 1.0, 1638.0, 1.0, 1);
    /// 文字間隔 and 位置: ±1584 pt, by tenths, stepping a tenth.
    pub const CHAR_SPACING: SpinSpec = SpinSpec::new(SpinUnit::Pt, -1584.0, 1584.0, 0.1, 1);
    /// カーニングを行う's threshold.
    pub const KERNING: SpinSpec = SpinSpec::new(SpinUnit::Pt, 1.0, 1638.0, 1.0, 1);
    /// インデント in characters: by halves of a character.
    pub const INDENT_CHARS: SpinSpec = SpinSpec::new(SpinUnit::Char, -99.0, 99.0, 0.5, 2);
    /// インデント in millimetres.
    pub const INDENT_MM: SpinSpec = SpinSpec::new(SpinUnit::Mm, -558.0, 558.0, 1.0, 1);
    /// 最初の行's 幅 in characters (never negative).
    pub const SPECIAL_CHARS: SpinSpec = SpinSpec::new(SpinUnit::Char, 0.0, 99.0, 0.5, 2);
    pub const SPECIAL_MM: SpinSpec = SpinSpec::new(SpinUnit::Mm, 0.0, 558.0, 1.0, 1);
    /// 段落前 / 段落後: points, stepping six.
    pub const PARAGRAPH_SPACING: SpinSpec = SpinSpec::new(SpinUnit::Pt, 0.0, 1584.0, 6.0, 1);
    /// 行間 最小値 / 固定値.
    pub const LINE_POINTS: SpinSpec = SpinSpec::new(SpinUnit::Pt, 0.7, 1584.0, 0.5, 1);
    /// 行間 倍数, in lines.
    pub const LINE_MULTIPLE: SpinSpec = SpinSpec::new(SpinUnit::Line, 0.06, 132.0, 0.5, 2);
    /// Margins, paper and header distances: millimetres, stepping one.
    pub const PAGE_MM: SpinSpec = SpinSpec::new(SpinUnit::Mm, 0.0, 558.8, 1.0, 1);
    /// 字送り and 行送り.
    pub const PITCH_PT: SpinSpec = SpinSpec::new(SpinUnit::Pt, 1.0, 1584.0, 0.5, 1);
    /// Table sizes: millimetres by tenths, stepping one.
    pub const TABLE_MM: SpinSpec = SpinSpec::new(SpinUnit::Mm, 0.0, 558.8, 1.0, 1);
    /// Object sizes: millimetres by tenths, stepping one.
    pub const OBJECT_MM: SpinSpec = SpinSpec::new(SpinUnit::Mm, 0.1, 558.8, 1.0, 1);
    /// Percentages: 拡大/縮小.
    pub const PERCENT: SpinSpec = SpinSpec::new(SpinUnit::Percent, 1.0, 600.0, 10.0, 0);

    /// A count from `min` to `max`, stepping one.
    pub const fn count(min: f64, max: f64) -> SpinSpec {
        SpinSpec::new(SpinUnit::Count, min, max, 1.0, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::specs::*;
    use super::*;

    fn pt() -> SpinSpec {
        SpinSpec::new(SpinUnit::Pt, 0.0, 1584.0, 6.0, 1)
    }

    #[test]
    fn the_unit_stays_in_the_text() {
        assert_eq!(FONT_SIZE.format(10.5), "10.5 pt");
        assert_eq!(FONT_SIZE.format(12.0), "12 pt");
        assert_eq!(INDENT_CHARS.format(1.0), "1 字");
        assert_eq!(INDENT_CHARS.format(0.25), "0.25 字");
        assert_eq!(LINE_MULTIPLE.format(0.5), "0.5 行");
        assert_eq!(PAGE_MM.format(25.4), "25.4 mm");
        assert_eq!(PAGE_MM.format(12.7), "12.7 mm");
        assert_eq!(PERCENT.format(100.0), "100 %");
        assert_eq!(count(1.0, 45.0).format(3.0), "3");
        assert_eq!(CHAR_SPACING.format(-0.04), "0 pt");
    }

    #[test]
    fn the_arrows_step_to_the_next_multiple_and_keep_the_unit() {
        let spec = pt();
        assert_eq!(spec.step(0.0, true), 6.0);
        assert_eq!(spec.step(6.0, true), 12.0);
        assert_eq!(spec.step(7.0, true), 12.0);
        assert_eq!(spec.step(7.0, false), 6.0);
        assert_eq!(spec.step(6.0, false), 0.0);
        // Not below the minimum, nor past the maximum.
        assert_eq!(spec.step(0.0, false), 0.0);
        assert_eq!(spec.step(1584.0, true), 1584.0);
        assert_eq!(FONT_SIZE.step(10.5, true), 11.0);
        assert_eq!(FONT_SIZE.step(10.5, false), 10.0);
        assert_eq!(INDENT_CHARS.step(1.0, true), 1.5);
        assert_eq!(CHAR_SPACING.step(0.0, false), -0.1);
        let mut field = SpinField::focus(INDENT_CHARS, Metrics::default(), 1.0);
        assert_eq!(field.key("up"), SpinKey::Stepped(1.5));
        assert_eq!(field.edit.text, "1.5 字");
        assert_eq!(field.key("down"), SpinKey::Stepped(1.0));
        assert_eq!(field.edit.text, "1 字");
    }

    #[test]
    fn a_number_alone_is_in_the_fields_unit() {
        assert_eq!(FONT_SIZE.parse("10.5", Metrics::default()), Some(10.5));
        assert_eq!(FONT_SIZE.parse(" 12 pt ", Metrics::default()), Some(12.0));
        assert_eq!(FONT_SIZE.parse("12pt", Metrics::default()), Some(12.0));
        assert_eq!(
            FONT_SIZE.parse("１２．５ポイント", Metrics::default()),
            Some(12.5)
        );
        assert_eq!(
            CHAR_SPACING.parse("-1.5 pt", Metrics::default()),
            Some(-1.5)
        );
        assert_eq!(PERCENT.parse("90％", Metrics::default()), Some(90.0));
    }

    #[test]
    fn another_unit_converts_on_commit() {
        let metrics = Metrics::default();
        // 1 cm is 28.35 pt.
        assert_eq!(pt().parse("1 cm", metrics), Some(28.3));
        assert_eq!(pt().parse("1in", metrics), Some(72.0));
        assert_eq!(PAGE_MM.parse("1 in", metrics), Some(25.4));
        assert_eq!(PAGE_MM.parse("2.5cm", metrics), Some(25.0));
        assert_eq!(PAGE_MM.parse("72 pt", metrics), Some(25.4));
        // 字 by the character width, 行 by the line height.
        assert_eq!(INDENT_CHARS.parse("21pt", metrics), Some(2.0));
        assert_eq!(INDENT_CHARS.parse("1 行", metrics), Some(1.71));
        assert_eq!(pt().parse("2 行", metrics), Some(36.0));
        assert_eq!(INDENT_MM.parse("1 字", metrics), Some(3.7));
        let large = Metrics {
            char_pt: 12.0,
            line_pt: 18.0,
        };
        assert_eq!(INDENT_CHARS.parse("24 pt", large), Some(2.0));
    }

    #[test]
    fn what_does_not_read_or_convert_is_turned_down() {
        let metrics = Metrics::default();
        assert_eq!(FONT_SIZE.parse("", metrics), None);
        assert_eq!(FONT_SIZE.parse("abc", metrics), None);
        assert_eq!(FONT_SIZE.parse("12 furlongs", metrics), None);
        assert_eq!(FONT_SIZE.parse("1.2.3", metrics), None);
        assert_eq!(FONT_SIZE.parse("50 %", metrics), None);
        assert_eq!(PERCENT.parse("12 pt", metrics), None);
        assert_eq!(count(1.0, 45.0).parse("2 mm", metrics), None);
        let mut field = SpinField::focus(FONT_SIZE, metrics, 10.5);
        field.edit.insert("big");
        assert_eq!(field.key("enter"), SpinKey::Committed(None));
        assert_eq!(field.edit.text, "10.5 pt");
        assert_eq!(field.value, 10.5);
    }

    #[test]
    fn out_of_range_values_are_kept_in_range() {
        let metrics = Metrics::default();
        assert_eq!(FONT_SIZE.parse("0", metrics), Some(1.0));
        assert_eq!(FONT_SIZE.parse("5000", metrics), Some(1638.0));
        assert_eq!(PARAGRAPH_SPACING.parse("-6", metrics), Some(0.0));
    }

    #[test]
    fn typing_replaces_the_value_and_commit_shows_it_in_the_unit() {
        let mut field = SpinField::focus(PAGE_MM, Metrics::default(), 30.0);
        assert!(field.edit.whole);
        field.edit.insert("3cm");
        assert_eq!(field.key("tab"), SpinKey::Committed(Some(30.0)));
        field.edit.insert("1 in");
        assert_eq!(field.commit(), Some(25.4));
        assert_eq!(field.edit.text, "25.4 mm");
        // The arrows step from what is typed.
        field.edit.insert("10");
        assert_eq!(field.key("up"), SpinKey::Stepped(11.0));
        assert_eq!(field.edit.text, "11 mm");
        // Escape goes back to the last value.
        field.edit.insert("99");
        assert_eq!(field.key("escape"), SpinKey::Reverted);
        assert_eq!(field.edit.text, "11 mm");
        assert_eq!(field.key("backspace"), SpinKey::Edited);
        assert_eq!(field.edit.text, "");
        assert_eq!(field.key("a"), SpinKey::Unhandled);
    }

    #[test]
    fn units_convert_between_lengths_but_not_into_percent() {
        let metrics = Metrics::default();
        assert_eq!(
            convert(25.4, SpinUnit::Mm, SpinUnit::In, metrics),
            Some(1.0)
        );
        assert_eq!(
            convert(1.0, SpinUnit::Cm, SpinUnit::Mm, metrics),
            Some(10.0)
        );
        assert_eq!(
            convert(1.0, SpinUnit::Char, SpinUnit::Pt, metrics),
            Some(10.5)
        );
        assert_eq!(
            convert(1.0, SpinUnit::Line, SpinUnit::Pt, metrics),
            Some(18.0)
        );
        assert_eq!(convert(1.0, SpinUnit::Pt, SpinUnit::Percent, metrics), None);
        assert_eq!(SpinUnit::parse("ミリ"), Some(SpinUnit::Mm));
        assert_eq!(SpinUnit::parse("ＰＴ"), Some(SpinUnit::Pt));
        assert_eq!(SpinUnit::parse("文字"), Some(SpinUnit::Char));
    }
}

//! Home ▸ Conditional Formatting, as both shells offer it: Excel's
//! Highlight Cells and Top/Bottom rules, each asking for its value in a
//! small dialog; its data bars and colour scales, which apply at once; and
//! the Rules Manager's list in words the shells translate.

pub use jade_xlsx_headless_editor::{ConditionalFormatView, ConditionalStyle, NewConditionalRule};

use crate::sheet::{CellRange, CellRef, Workbook};
use crate::text::TextEdit;
use crate::Backend;

/// A rule from the menu that asks for its value first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    GreaterThan,
    LessThan,
    Between,
    EqualTo,
    TextContains,
    DuplicateValues,
    TopItems,
    TopPercent,
    BottomItems,
    BottomPercent,
    AboveAverage,
    BelowAverage,
}

impl Preset {
    /// Highlight Cells Rules, as the menu lists them.
    pub const HIGHLIGHT: [Preset; 6] = [
        Preset::GreaterThan,
        Preset::LessThan,
        Preset::Between,
        Preset::EqualTo,
        Preset::TextContains,
        Preset::DuplicateValues,
    ];
    /// Top/Bottom Rules.
    pub const TOP_BOTTOM: [Preset; 6] = [
        Preset::TopItems,
        Preset::TopPercent,
        Preset::BottomItems,
        Preset::BottomPercent,
        Preset::AboveAverage,
        Preset::BelowAverage,
    ];

    /// The menu item's words, and the dialog's title.
    pub fn label_key(self) -> &'static str {
        match self {
            Preset::GreaterThan => "cf.greater_than",
            Preset::LessThan => "cf.less_than",
            Preset::Between => "cf.between",
            Preset::EqualTo => "cf.equal_to",
            Preset::TextContains => "cf.text_contains",
            Preset::DuplicateValues => "cf.duplicate_values",
            Preset::TopItems => "cf.top_items",
            Preset::TopPercent => "cf.top_percent",
            Preset::BottomItems => "cf.bottom_items",
            Preset::BottomPercent => "cf.bottom_percent",
            Preset::AboveAverage => "cf.above_average",
            Preset::BelowAverage => "cf.below_average",
        }
    }

    /// What the dialog asks, above its value fields.
    pub fn prompt_key(self) -> &'static str {
        match self {
            Preset::GreaterThan => "cf.prompt.greater_than",
            Preset::LessThan => "cf.prompt.less_than",
            Preset::Between => "cf.prompt.between",
            Preset::EqualTo => "cf.prompt.equal_to",
            Preset::TextContains => "cf.prompt.text_contains",
            Preset::DuplicateValues => "cf.prompt.duplicate_values",
            Preset::TopItems => "cf.prompt.top_items",
            Preset::TopPercent => "cf.prompt.top_percent",
            Preset::BottomItems => "cf.prompt.bottom_items",
            Preset::BottomPercent => "cf.prompt.bottom_percent",
            Preset::AboveAverage => "cf.prompt.above_average",
            Preset::BelowAverage => "cf.prompt.below_average",
        }
    }

    /// How many values it asks for.
    pub fn fields(self) -> usize {
        match self {
            Preset::Between => 2,
            Preset::DuplicateValues | Preset::AboveAverage | Preset::BelowAverage => 0,
            _ => 1,
        }
    }

    fn ranked(self) -> Option<(bool, bool)> {
        match self {
            Preset::TopItems => Some((false, false)),
            Preset::TopPercent => Some((false, true)),
            Preset::BottomItems => Some((true, false)),
            Preset::BottomPercent => Some((true, true)),
            _ => None,
        }
    }
}

/// The looks the dialog's "with" list offers, in its order.
pub const STYLES: [(ConditionalStyle, &str); 5] = [
    (
        ConditionalStyle::LightRedFillDarkRedText,
        "cf.style.light_red_dark_red",
    ),
    (
        ConditionalStyle::YellowFillDarkYellowText,
        "cf.style.yellow_dark_yellow",
    ),
    (
        ConditionalStyle::GreenFillDarkGreenText,
        "cf.style.green_dark_green",
    ),
    (ConditionalStyle::LightRedFill, "cf.style.light_red"),
    (ConditionalStyle::RedText, "cf.style.red_text"),
];

/// The dialog a highlight or top/bottom rule asks through.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleDialog {
    pub preset: Preset,
    pub range: CellRange,
    /// What is typed in each value field.
    pub values: Vec<String>,
    pub style: ConditionalStyle,
}

impl RuleDialog {
    /// The dialog for `preset` over `range`, filled in from the selection
    /// as Excel fills it: a comparison with the selection's average,
    /// Between with its lowest and highest, Equal To and Text that Contains
    /// with the active cell, and a ranking with 10.
    pub fn open<B: Backend>(
        preset: Preset,
        book: &Workbook<B>,
        range: CellRange,
        active: CellRef,
    ) -> Self {
        let mut numbers = Vec::new();
        for row in range.start.row..=range.end.row.min(range.start.row + 10_000) {
            for column in range.start.column..=range.end.column.min(range.start.column + 100) {
                if let Some(number) = book.number(CellRef::new(column, row)) {
                    numbers.push(number);
                }
            }
        }
        let shown = |number: f64| {
            let rounded = (number * 100.0).round() / 100.0;
            format!("{rounded}")
        };
        let average = || {
            if numbers.is_empty() {
                String::new()
            } else {
                shown(numbers.iter().sum::<f64>() / numbers.len() as f64)
            }
        };
        let values = match preset {
            Preset::GreaterThan | Preset::LessThan => vec![average()],
            Preset::Between => {
                let low = numbers.iter().copied().fold(f64::INFINITY, f64::min);
                let high = numbers.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                if numbers.is_empty() {
                    vec![String::new(), String::new()]
                } else {
                    vec![shown(low), shown(high)]
                }
            }
            Preset::EqualTo | Preset::TextContains => vec![book.display_text(active)],
            Preset::TopItems | Preset::TopPercent | Preset::BottomItems | Preset::BottomPercent => {
                vec!["10".into()]
            }
            Preset::DuplicateValues | Preset::AboveAverage | Preset::BelowAverage => Vec::new(),
        };
        Self {
            preset,
            range,
            values,
            style: ConditionalStyle::LightRedFillDarkRedText,
        }
    }

    /// The rule as typed, or the i18n key of what is wrong with it.
    pub fn rule(&self) -> Result<NewConditionalRule, &'static str> {
        let value = |index: usize| -> Result<String, &'static str> {
            let text = self.values.get(index).map(|text| text.trim()).unwrap_or("");
            if text.is_empty() {
                return Err("cf.invalid_value");
            }
            Ok(operand(text))
        };
        if let Some((bottom, percent)) = self.preset.ranked() {
            let rank = self
                .values
                .first()
                .and_then(|text| text.trim().parse::<u32>().ok())
                .filter(|rank| *rank >= 1 && *rank <= if percent { 100 } else { 1000 })
                .ok_or("cf.invalid_rank")?;
            return Ok(if bottom {
                NewConditionalRule::Bottom { rank, percent }
            } else {
                NewConditionalRule::Top { rank, percent }
            });
        }
        Ok(match self.preset {
            Preset::GreaterThan => NewConditionalRule::GreaterThan(value(0)?),
            Preset::LessThan => NewConditionalRule::LessThan(value(0)?),
            Preset::Between => NewConditionalRule::Between(value(0)?, value(1)?),
            Preset::EqualTo => NewConditionalRule::EqualTo(value(0)?),
            Preset::TextContains => {
                let text = self.values.first().map(|text| text.trim()).unwrap_or("");
                if text.is_empty() {
                    return Err("cf.invalid_value");
                }
                NewConditionalRule::TextContains(text.to_string())
            }
            Preset::DuplicateValues => NewConditionalRule::DuplicateValues,
            Preset::AboveAverage => NewConditionalRule::AboveAverage,
            _ => NewConditionalRule::BelowAverage,
        })
    }
}

/// The dialog as a shell holds it open: a text field per value, which
/// one has the caret, and what was wrong with the last OK.
pub struct RuleAsk {
    pub dialog: RuleDialog,
    pub fields: Vec<TextEdit>,
    pub field: usize,
    pub error: Option<&'static str>,
}

/// What a key did to the dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AskKey {
    /// Enter: OK.
    Take,
    /// Escape: Cancel.
    Close,
    Handled,
    /// Not the dialog's key: a ⌘ shortcut, or the IME's mid-composition.
    Unhandled,
}

impl RuleAsk {
    /// The dialog with each value selected in its field, as Excel's are.
    pub fn new(dialog: RuleDialog) -> Self {
        let fields = dialog
            .values
            .iter()
            .map(|value| TextEdit::selected(value.clone()))
            .collect();
        Self {
            dialog,
            fields,
            field: 0,
            error: None,
        }
    }

    pub fn typing(&mut self) -> Option<&mut TextEdit> {
        self.fields.get_mut(self.field)
    }

    pub fn typed(&self) -> Option<&TextEdit> {
        self.fields.get(self.field)
    }

    /// Enter takes it, Escape leaves it, Tab moves between the fields; the
    /// rest edit the field with the caret.
    pub fn key(&mut self, key: &str, shift: bool, command: bool) -> AskKey {
        if self.typed().is_some_and(|text| text.marked.is_some()) {
            return AskKey::Unhandled;
        }
        match key {
            "enter" => return AskKey::Take,
            "escape" => return AskKey::Close,
            "tab" if !self.fields.is_empty() => {
                let count = self.fields.len();
                self.field = if shift {
                    (self.field + count - 1) % count
                } else {
                    (self.field + 1) % count
                };
                if let Some(text) = self.typing() {
                    text.whole = true;
                }
            }
            _ => {
                let Some(text) = self.typing() else {
                    return AskKey::Unhandled;
                };
                match key {
                    "backspace" => text.backspace(),
                    "delete" => text.delete(),
                    "left" => text.step(false),
                    "right" => text.step(true),
                    "home" | "up" => text.to_end(false),
                    "end" | "down" => text.to_end(true),
                    "a" if command => text.whole = true,
                    // Characters arrive as text input, not keys.
                    _ => return AskKey::Unhandled,
                }
                self.error = None;
            }
        }
        AskKey::Handled
    }

    /// The rule as typed; on a mistake, the reason is kept for the dialog
    /// to show.
    pub fn rule(&mut self) -> Option<NewConditionalRule> {
        self.dialog.values = self.fields.iter().map(|field| field.text.clone()).collect();
        match self.dialog.rule() {
            Ok(rule) => Some(rule),
            Err(key) => {
                self.error = Some(key);
                None
            }
        }
    }
}

/// What a typed value is as a rule's formula: a number or a formula as it
/// is, anything else as text, quoted the way Excel quotes it.
fn operand(text: &str) -> String {
    if let Some(formula) = text.strip_prefix('=') {
        return formula.to_string();
    }
    if text.parse::<f64>().is_ok_and(f64::is_finite)
        || text.eq_ignore_ascii_case("TRUE")
        || text.eq_ignore_ascii_case("FALSE")
    {
        return text.to_string();
    }
    format!("\"{}\"", text.replace('"', "\"\""))
}

/// A menu entry that applies at once: a data bar or a colour scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Visual {
    pub label_key: &'static str,
    /// The bar's colour, or the scale's from the lowest value up.
    pub colors: &'static [u32],
    pub scale: bool,
}

impl Visual {
    pub fn rule(&self) -> NewConditionalRule {
        if self.scale {
            NewConditionalRule::ColorScale(self.colors.to_vec())
        } else {
            NewConditionalRule::DataBar(self.colors[0])
        }
    }
}

/// Excel's data bars, in the menu's order.
pub const DATA_BARS: [Visual; 6] = [
    bar("cf.bar.blue", &[0x638EC6]),
    bar("cf.bar.green", &[0x63C384]),
    bar("cf.bar.red", &[0xFF555A]),
    bar("cf.bar.orange", &[0xFFB628]),
    bar("cf.bar.light_blue", &[0x008AEF]),
    bar("cf.bar.purple", &[0xD6007B]),
];

/// Excel's colour scales, named from the highest value down as the menu
/// names them, written from the lowest up as the file writes them.
pub const COLOR_SCALES: [Visual; 6] = [
    scale("cf.scale.green_yellow_red", &[0xF8696B, 0xFFEB84, 0x63BE7B]),
    scale("cf.scale.red_yellow_green", &[0x63BE7B, 0xFFEB84, 0xF8696B]),
    scale("cf.scale.green_white_red", &[0xF8696B, 0xFCFCFF, 0x63BE7B]),
    scale("cf.scale.blue_white_red", &[0xF8696B, 0xFCFCFF, 0x5A8AC6]),
    scale("cf.scale.green_white", &[0xFCFCFF, 0x63BE7B]),
    scale("cf.scale.white_red", &[0xF8696B, 0xFCFCFF]),
];

const fn bar(label_key: &'static str, colors: &'static [u32]) -> Visual {
    Visual {
        label_key,
        colors,
        scale: false,
    }
}

const fn scale(label_key: &'static str, colors: &'static [u32]) -> Visual {
    Visual {
        label_key,
        colors,
        scale: true,
    }
}

/// A rule in words: an i18n key and the values it names.
#[derive(Debug, Clone, PartialEq)]
pub struct Description {
    pub key: &'static str,
    pub values: Vec<(&'static str, String)>,
}

/// The Rules Manager's words for `rule`; a rule the menu does not make
/// keeps the engine's English.
pub fn describe(rule: &ConditionalFormatView) -> Description {
    let (key, values): (&'static str, Vec<(&'static str, String)>) = match &rule.rule {
        Some(NewConditionalRule::GreaterThan(value)) => {
            ("cf.describe.greater_than", vec![("value", value.clone())])
        }
        Some(NewConditionalRule::LessThan(value)) => {
            ("cf.describe.less_than", vec![("value", value.clone())])
        }
        Some(NewConditionalRule::Between(low, high)) => (
            "cf.describe.between",
            vec![("low", low.clone()), ("high", high.clone())],
        ),
        Some(NewConditionalRule::EqualTo(value)) => {
            ("cf.describe.equal_to", vec![("value", value.clone())])
        }
        Some(NewConditionalRule::TextContains(text)) => {
            ("cf.describe.text_contains", vec![("text", text.clone())])
        }
        Some(NewConditionalRule::DuplicateValues) => ("cf.describe.duplicate_values", Vec::new()),
        Some(NewConditionalRule::Top { rank, percent }) => (
            if *percent {
                "cf.describe.top_percent"
            } else {
                "cf.describe.top_items"
            },
            vec![("rank", rank.to_string())],
        ),
        Some(NewConditionalRule::Bottom { rank, percent }) => (
            if *percent {
                "cf.describe.bottom_percent"
            } else {
                "cf.describe.bottom_items"
            },
            vec![("rank", rank.to_string())],
        ),
        Some(NewConditionalRule::AboveAverage) => ("cf.describe.above_average", Vec::new()),
        Some(NewConditionalRule::BelowAverage) => ("cf.describe.below_average", Vec::new()),
        Some(NewConditionalRule::DataBar(_)) => ("cf.describe.data_bar", Vec::new()),
        Some(NewConditionalRule::ColorScale(_)) => ("cf.describe.color_scale", Vec::new()),
        None => (
            "cf.describe.other",
            vec![("rule", rule.description.clone())],
        ),
    };
    Description { key, values }
}

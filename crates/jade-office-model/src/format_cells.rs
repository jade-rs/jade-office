//! The Format Cells dialog (⌘1), as Excel's: what each tab shows for the
//! active cell, and the change that applying it makes — only what was
//! changed, so a multi-cell selection keeps the rest of each cell's look.
//!
//! The Number tab reads a format code into one of Excel's categories and
//! its options, and writes them back as the code Excel writes; a code it
//! cannot read that way is Custom, shown and kept as it is.

use jade_xlsx_headless_api::BorderLine;
use jade_xlsx_headless_editor::FormatChange;
use jade_xlsx_layout::number_format::{format_value, is_date_format};
use jade_xlsx_reader::{CellValue, DateSystem, HorizontalAlignment, VerticalAlignment};

/// The Number tab's categories, in Excel's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberCategory {
    General,
    Number,
    Currency,
    Accounting,
    Date,
    Time,
    Percentage,
    Fraction,
    Scientific,
    Text,
    Custom,
}

impl NumberCategory {
    pub const ALL: [NumberCategory; 11] = [
        NumberCategory::General,
        NumberCategory::Number,
        NumberCategory::Currency,
        NumberCategory::Accounting,
        NumberCategory::Date,
        NumberCategory::Time,
        NumberCategory::Percentage,
        NumberCategory::Fraction,
        NumberCategory::Scientific,
        NumberCategory::Text,
        NumberCategory::Custom,
    ];

    /// The key of the category's name in the shells' string tables.
    pub fn label_key(self) -> &'static str {
        match self {
            NumberCategory::General => "format.general",
            NumberCategory::Number => "format.number",
            NumberCategory::Currency => "format.currency",
            NumberCategory::Accounting => "format.accounting",
            NumberCategory::Date => "format.date",
            NumberCategory::Time => "format.time",
            NumberCategory::Percentage => "format.percentage",
            NumberCategory::Fraction => "format.fraction",
            NumberCategory::Scientific => "format.scientific",
            NumberCategory::Text => "format.text",
            NumberCategory::Custom => "format.custom",
        }
    }

    /// Whether the category has a number of decimal places to choose.
    pub fn has_decimals(self) -> bool {
        matches!(
            self,
            NumberCategory::Number
                | NumberCategory::Currency
                | NumberCategory::Accounting
                | NumberCategory::Percentage
                | NumberCategory::Scientific
        )
    }

    /// The codes the category offers to pick from, when it is a list.
    pub fn patterns(self) -> &'static [&'static str] {
        match self {
            NumberCategory::Date => DATE_CODES,
            NumberCategory::Time => TIME_CODES,
            NumberCategory::Fraction => FRACTION_CODES,
            _ => &[],
        }
    }
}

/// Japanese Excel's date types.
pub const DATE_CODES: &[&str] = &[
    "yyyy/m/d",
    "yyyy\"年\"m\"月\"d\"日\"",
    "m/d",
    "m\"月\"d\"日\"",
    "[$-411]ggge\"年\"m\"月\"d\"日\"",
    "yyyy/m/d h:mm",
    "d-mmm-yy",
];

/// Japanese Excel's time types.
pub const TIME_CODES: &[&str] = &[
    "h:mm",
    "h:mm:ss",
    "h\"時\"mm\"分\"",
    "h\"時\"mm\"分\"ss\"秒\"",
    "h:mm AM/PM",
];

pub const FRACTION_CODES: &[&str] = &["# ?/?", "# ??/??", "# ?/2", "# ?/4", "# ?/8", "# ?/10"];

/// The currency symbols the Currency and Accounting categories offer.
pub const CURRENCY_SYMBOLS: &[&str] = &["¥", "$", "€", "£"];

/// How a negative number shows, for Number and Currency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Negative {
    /// `-1,234`.
    #[default]
    Minus,
    /// `-1,234` in red.
    RedMinus,
    /// `1,234` in red.
    Red,
}

/// The Number tab: a category and its options.
#[derive(Debug, Clone, PartialEq)]
pub struct NumberFormat {
    pub category: NumberCategory,
    pub decimals: u8,
    /// The thousands separator, for Number.
    pub thousands: bool,
    pub negative: Negative,
    /// For Currency and Accounting.
    pub symbol: String,
    /// The code itself, for Date, Time, Fraction and Custom.
    pub pattern: String,
}

impl NumberFormat {
    /// Reads `code` into Excel's category and options; a code no category
    /// writes is Custom.
    pub fn from_code(code: &str) -> Self {
        let mut format = NumberFormat {
            category: NumberCategory::Custom,
            decimals: 0,
            thousands: false,
            negative: Negative::Minus,
            symbol: "¥".to_string(),
            pattern: code.to_string(),
        };
        if code.is_empty() || code.eq_ignore_ascii_case("General") {
            format.category = NumberCategory::General;
            format.pattern = String::new();
            return format;
        }
        if code == "@" {
            format.category = NumberCategory::Text;
            return format;
        }
        if is_date_format(code) {
            let bare = unquoted(code);
            format.category = if bare.contains(['y', 'd', 'g', 'e', 'Y', 'D']) {
                NumberCategory::Date
            } else {
                NumberCategory::Time
            };
            return format;
        }
        if code.contains('/') && code.contains('?') {
            format.category = NumberCategory::Fraction;
            return format;
        }
        if let Some(decimals) = code.strip_suffix('%').and_then(plain_decimals) {
            format.category = NumberCategory::Percentage;
            format.decimals = decimals;
            return format;
        }
        if let Some(decimals) = code.strip_suffix("E+00").and_then(plain_decimals) {
            format.category = NumberCategory::Scientific;
            format.decimals = decimals;
            return format;
        }
        if let Some(read) = read_number(code) {
            return read;
        }
        if let Some(read) = read_currency(code) {
            return read;
        }
        if let Some(read) = read_accounting(code) {
            return read;
        }
        format
    }

    /// The code Excel writes for the category and options.
    pub fn code(&self) -> String {
        let fraction = if self.decimals > 0 {
            format!(".{}", "0".repeat(self.decimals as usize))
        } else {
            String::new()
        };
        match self.category {
            NumberCategory::General => "General".to_string(),
            NumberCategory::Text => "@".to_string(),
            NumberCategory::Number => {
                let base = format!("{}0{fraction}", if self.thousands { "#,##" } else { "" });
                match self.negative {
                    Negative::Minus => base,
                    Negative::RedMinus => format!("{base};[Red]-{base}"),
                    Negative::Red => format!("{base};[Red]{base}"),
                }
            }
            NumberCategory::Currency => {
                let number = format!("#,##0{fraction}");
                let symbol = format!("\"{}\"", self.symbol);
                let positive = format!("{symbol}{number}");
                match self.negative {
                    Negative::Minus => format!("{positive};{symbol}\\-{number}"),
                    Negative::RedMinus => format!("{positive};[Red]{symbol}\\-{number}"),
                    Negative::Red => format!("{positive};[Red]{positive}"),
                }
            }
            NumberCategory::Accounting => {
                let number = format!("#,##0{fraction}");
                let symbol = format!("\"{}\"", self.symbol);
                format!(
                    "_ {symbol}* {number}_ ;_ {symbol}* \\-{number}_ ;_ {symbol}* \"-\"{}_ ;_ @_ ",
                    if self.decimals > 0 {
                        "??".to_string()
                    } else {
                        String::new()
                    }
                )
            }
            NumberCategory::Percentage => format!("0{fraction}%"),
            NumberCategory::Scientific => format!("0{fraction}E+00"),
            NumberCategory::Date
            | NumberCategory::Time
            | NumberCategory::Fraction
            | NumberCategory::Custom => {
                if self.pattern.is_empty() {
                    self.category
                        .patterns()
                        .first()
                        .map(|code| code.to_string())
                        .unwrap_or_else(|| "General".to_string())
                } else {
                    self.pattern.clone()
                }
            }
        }
    }

    /// `value` as the dialog's sample shows it under the code.
    pub fn sample(&self, value: f64) -> String {
        let code = self.code();
        let code = (self.category != NumberCategory::General).then_some(code.as_str());
        format_value(&CellValue::Number(value), code, DateSystem::default()).text
    }
}

/// `0`, `0.0`, `0.00`…: the number of decimal places.
fn plain_decimals(code: &str) -> Option<u8> {
    let rest = code.strip_prefix('0')?;
    if rest.is_empty() {
        return Some(0);
    }
    let zeros = rest.strip_prefix('.')?;
    (!zeros.is_empty() && zeros.bytes().all(|b| b == b'0')).then_some(zeros.len() as u8)
}

/// `#,##0.00` or `0.00`: the thousands separator and decimals.
fn read_plain(code: &str) -> Option<(bool, u8)> {
    match code.strip_prefix("#,##") {
        Some(rest) => plain_decimals(rest).map(|decimals| (true, decimals)),
        None => plain_decimals(code).map(|decimals| (false, decimals)),
    }
}

fn read_number(code: &str) -> Option<NumberFormat> {
    let mut sections = code.split(';');
    let positive = sections.next()?;
    let (thousands, decimals) = read_plain(positive)?;
    let negative = match (sections.next(), sections.next()) {
        (None, _) => Negative::Minus,
        (Some(negative), None) if negative == format!("[Red]-{positive}") => Negative::RedMinus,
        (Some(negative), None) if negative == format!("[Red]{positive}") => Negative::Red,
        _ => return None,
    };
    Some(NumberFormat {
        category: NumberCategory::Number,
        decimals,
        thousands,
        negative,
        symbol: "¥".to_string(),
        pattern: String::new(),
    })
}

fn read_currency(code: &str) -> Option<NumberFormat> {
    let (positive, negative) = code.split_once(';')?;
    let rest = positive.strip_prefix('"')?;
    let (symbol, number) = rest.split_once('"')?;
    let (thousands, decimals) = read_plain(number)?;
    if !thousands || !CURRENCY_SYMBOLS.contains(&symbol) {
        return None;
    }
    let quoted = format!("\"{symbol}\"");
    let negative = if negative == format!("{quoted}\\-{number}") {
        Negative::Minus
    } else if negative == format!("[Red]{quoted}\\-{number}") {
        Negative::RedMinus
    } else if negative == format!("[Red]{positive}") {
        Negative::Red
    } else {
        return None;
    };
    Some(NumberFormat {
        category: NumberCategory::Currency,
        decimals,
        thousands: true,
        negative,
        symbol: symbol.to_string(),
        pattern: String::new(),
    })
}

fn read_accounting(code: &str) -> Option<NumberFormat> {
    let rest = code.strip_prefix("_ \"")?;
    let (symbol, rest) = rest.split_once('"')?;
    let number = rest.strip_prefix("* ")?.split("_ ").next()?;
    let (_, decimals) = read_plain(number)?;
    let format = NumberFormat {
        category: NumberCategory::Accounting,
        decimals,
        thousands: true,
        negative: Negative::Minus,
        symbol: symbol.to_string(),
        pattern: String::new(),
    };
    (format.code() == code).then_some(format)
}

/// `code` without its quoted text and bracketed sections, for telling a
/// date's tokens from a time's.
fn unquoted(code: &str) -> String {
    let mut out = String::new();
    let mut quoted = false;
    let mut bracket = false;
    for ch in code.chars() {
        match ch {
            '"' => quoted = !quoted,
            '[' if !quoted => bracket = true,
            ']' if !quoted => bracket = false,
            _ if quoted || bracket => {}
            _ => out.push(ch),
        }
    }
    out
}

/// The tabs, in Excel's order. (Protection is not offered: the engine
/// does not write a cell's lock.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Number,
    Alignment,
    Font,
    Border,
    Fill,
}

impl Tab {
    pub const ALL: [Tab; 5] = [
        Tab::Number,
        Tab::Alignment,
        Tab::Font,
        Tab::Border,
        Tab::Fill,
    ];

    pub fn label_key(self) -> &'static str {
        match self {
            Tab::Number => "format.tab.number",
            Tab::Alignment => "format.tab.alignment",
            Tab::Font => "format.tab.font",
            Tab::Border => "format.tab.border",
            Tab::Fill => "format.tab.fill",
        }
    }
}

/// The dialog's state: every tab's fields, filled from the active cell.
#[derive(Debug, Clone)]
pub struct FormatCellsDialog {
    pub tab: Tab,
    pub number: NumberFormat,
    pub horizontal: HorizontalAlignment,
    pub vertical: VerticalAlignment,
    pub wrap: bool,
    /// Shrink to fit; Excel greys it out while Wrap text is ticked.
    pub shrink: bool,
    /// Indent level, 0–15 as the dialog's spinner offers.
    pub indent: u32,
    /// Degrees, −90 to 90, or [`VERTICAL_TEXT`] for stacked text.
    pub rotation: i16,
    pub font_name: Option<String>,
    pub font_size: Option<f32>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    /// `RRGGBB`.
    pub font_color: Option<String>,
    /// `RRGGBB`; `None` for no fill.
    pub fill_color: Option<String>,
    pub border_left: Option<BorderLine>,
    pub border_right: Option<BorderLine>,
    pub border_top: Option<BorderLine>,
    pub border_bottom: Option<BorderLine>,
    original: FormatChange,
}

/// The dialog's rotation for vertical (stacked) text.
pub const VERTICAL_TEXT: i16 = 255;
/// The deepest indent the dialog's spinner offers.
pub const MAX_INDENT: u32 = 15;

/// SpreadsheetML's rotation as the dialog's degrees: 91–180 are below the
/// horizon, so they read as −1 to −90.
pub fn rotation_degrees(rotation: u16) -> i16 {
    match rotation {
        255 => VERTICAL_TEXT,
        91..=180 => 90 - rotation as i16,
        rotation => rotation.min(90) as i16,
    }
}

/// The dialog's degrees as SpreadsheetML's rotation.
pub fn text_rotation(degrees: i16) -> u16 {
    match degrees {
        VERTICAL_TEXT => 255,
        degrees if degrees < 0 => 90 + degrees.max(-90).unsigned_abs(),
        degrees => degrees.min(90) as u16,
    }
}

/// A border edge as the dialog holds it: `None` for no line.
fn edge(line: &Option<BorderLine>) -> Option<BorderLine> {
    line.clone()
        .filter(|line| line.style != jade_xlsx_headless_api::BorderStyle::None)
}

impl FormatCellsDialog {
    /// The dialog for a cell that looks like `current` (the model's
    /// `Workbook::format_at` of the active cell).
    pub fn open(current: FormatChange) -> Self {
        Self {
            tab: Tab::default(),
            number: NumberFormat::from_code(current.number_format.as_deref().unwrap_or("")),
            horizontal: current.horizontal.unwrap_or_default(),
            vertical: current.vertical.unwrap_or_default(),
            wrap: current.wrap_text.unwrap_or(false),
            shrink: current.shrink_to_fit.unwrap_or(false),
            indent: current.indent.unwrap_or(0),
            rotation: rotation_degrees(current.text_rotation.unwrap_or(0)),
            font_name: current.font_name.clone(),
            font_size: current.font_size,
            bold: current.bold.unwrap_or(false),
            italic: current.italic.unwrap_or(false),
            underline: current.underline.unwrap_or(false),
            strike: current.strike.unwrap_or(false),
            font_color: current.font_color.clone(),
            fill_color: current.fill_color.clone().filter(|color| !color.is_empty()),
            border_left: edge(&current.border_left),
            border_right: edge(&current.border_right),
            border_top: edge(&current.border_top),
            border_bottom: edge(&current.border_bottom),
            original: current,
        }
    }

    /// The four edges at once: Excel's Outline preset (`Some`), or None.
    pub fn set_outline(&mut self, line: Option<BorderLine>) {
        self.border_left = line.clone();
        self.border_right = line.clone();
        self.border_top = line.clone();
        self.border_bottom = line;
    }

    /// What applying the dialog changes: each field that differs from what
    /// it opened with, and nothing else.
    pub fn change(&self) -> FormatChange {
        let original = &self.original;
        let differs = |now: bool, then: Option<bool>| (now != then.unwrap_or(false)).then_some(now);
        let mut change = FormatChange::default();
        let code = self.number.code();
        let was = NumberFormat::from_code(original.number_format.as_deref().unwrap_or("")).code();
        if code != was {
            change.number_format = Some(code);
        }
        if self.horizontal != original.horizontal.unwrap_or_default() {
            change.horizontal = Some(self.horizontal);
        }
        if self.vertical != original.vertical.unwrap_or_default() {
            change.vertical = Some(self.vertical);
        }
        change.wrap_text = differs(self.wrap, original.wrap_text);
        // Wrapped text cannot also shrink: Excel drops the tick.
        let shrink = self.shrink && !self.wrap;
        change.shrink_to_fit = differs(shrink, original.shrink_to_fit);
        // Only left, right and distributed text keep an indent.
        let indent = match self.horizontal {
            HorizontalAlignment::Left
            | HorizontalAlignment::Right
            | HorizontalAlignment::Distributed
            | HorizontalAlignment::General => self.indent.min(MAX_INDENT),
            _ => 0,
        };
        if indent != original.indent.unwrap_or(0) {
            change.indent = Some(indent);
        }
        let rotation = text_rotation(self.rotation);
        if rotation != original.text_rotation.unwrap_or(0) {
            change.text_rotation = Some(rotation);
        }
        change.bold = differs(self.bold, original.bold);
        change.italic = differs(self.italic, original.italic);
        change.underline = differs(self.underline, original.underline);
        change.strike = differs(self.strike, original.strike);
        if self.font_name.is_some() && self.font_name != original.font_name {
            change.font_name = self.font_name.clone();
        }
        if self.font_size.is_some() && self.font_size != original.font_size {
            change.font_size = self.font_size;
        }
        if self.font_color.is_some() && self.font_color != original.font_color {
            change.font_color = self.font_color.clone();
        }
        let fill_was = original
            .fill_color
            .clone()
            .filter(|color| !color.is_empty());
        if self.fill_color != fill_was {
            change.fill_color = Some(self.fill_color.clone().unwrap_or_default());
        }
        let line = |now: &Option<BorderLine>, then: &Option<BorderLine>| {
            (*now != edge(then)).then(|| now.clone().unwrap_or_else(BorderLine::none))
        };
        change.border_left = line(&self.border_left, &original.border_left);
        change.border_right = line(&self.border_right, &original.border_right);
        change.border_top = line(&self.border_top, &original.border_top);
        change.border_bottom = line(&self.border_bottom, &original.border_bottom);
        change
    }
}

//! Worksheet input rules, their editing dialog, and explicit Warning / Information
//! confirmations. Stop rules cannot be bypassed through the confirmation API.
use crate::{
    sheet::{CellRange, CellRef, Workbook},
    text::TextEdit,
    typing, Backend,
};
use jade_xlsx_calc::validation::{Candidate, ValidationResult};
use jade_xlsx_headless_api::CellInput;
use jade_xlsx_reader::{CellValue, DataValidation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertStyle {
    Stop,
    Warning,
    Information,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationAlert {
    pub style: AlertStyle,
    pub title: Option<String>,
    pub message: Option<String>,
    pub unsupported: bool,
}
#[derive(Debug, Clone)]
pub struct PendingValidation {
    pub at: CellRef,
    pub text: String,
    pub alert: ValidationAlert,
}

impl<B: Backend> Workbook<B> {
    pub fn clear_data_validation_ranges(&mut self, ranges: &[CellRange]) -> Result<bool, String> {
        let name = self.sheet_name();
        if !self
            .engine_mut()
            .clear_data_validation_ranges(&name, ranges)
            .map_err(|e| e.to_string())?
        {
            return Ok(false);
        }
        self.wrote(1)?;
        Ok(true)
    }
    pub fn data_validations(&self) -> &[DataValidation] {
        &self.engine().workbook().sheets[self.sheet_index()].data_validations
    }
    pub fn data_validation_at(&self, at: CellRef) -> Option<&DataValidation> {
        self.data_validations()
            .iter()
            .find(|r| r.ranges.iter().any(|range| range.contains(at)))
    }
    pub fn apply_data_validation(&mut self, rule: &DataValidation) -> Result<bool, String> {
        let name = self.sheet_name();
        self.engine_mut()
            .apply_data_validation(&name, rule)
            .map_err(|e| e.to_string())?;
        self.wrote(1)?;
        Ok(true)
    }
    pub fn clear_data_validations(&mut self, range: Option<CellRange>) -> Result<bool, String> {
        let name = self.sheet_name();
        if !self
            .engine_mut()
            .clear_data_validations(&name, range)
            .map_err(|e| e.to_string())?
        {
            return Ok(false);
        }
        self.wrote(1)?;
        Ok(true)
    }
    /// Data ▸ Circle Invalid Data: the cells on the active sheet whose value
    /// breaks their validation rule, in reading order, at most Excel's 255;
    /// worked out once per layout revision.
    pub fn invalid_cells(&self) -> std::rc::Rc<Vec<CellRef>> {
        let revision = self.layout_revision();
        if let Some((at, cells)) = &*self.invalid_cells.borrow() {
            if *at == revision {
                return cells.clone();
            }
        }
        let workbook = self.engine().workbook();
        let index = self.sheet_index();
        let sheet = &workbook.sheets[index];
        let mut found: Vec<CellRef> = Vec::new();
        'rules: for rule in &sheet.data_validations {
            for cell in sheet.rows.iter().flat_map(|row| row.cells.iter()) {
                if found.len() >= 255 {
                    break 'rules;
                }
                if matches!(cell.value, CellValue::Empty)
                    || found.contains(&cell.reference)
                    || !rule
                        .ranges
                        .iter()
                        .any(|range| range.contains(cell.reference))
                {
                    continue;
                }
                let result = jade_xlsx_calc::validation::validate(
                    workbook,
                    index,
                    cell.reference,
                    rule,
                    &Candidate::Value(cell.value.clone()),
                    self.engine().calculation_time_zone(),
                );
                if matches!(result, Ok(ValidationResult::Invalid)) {
                    found.push(cell.reference);
                }
            }
        }
        found.sort_by_key(|at| (at.row, at.column));
        let cells = std::rc::Rc::new(found);
        *self.invalid_cells.borrow_mut() = Some((revision, cells.clone()));
        cells
    }

    /// No alert when the value satisfies the rule, or alerts are disabled.
    pub fn validation_alert(
        &self,
        at: CellRef,
        text: &str,
    ) -> Result<Option<ValidationAlert>, String> {
        let Some(rule) = self.data_validation_at(at) else {
            return Ok(None);
        };
        if !rule.show_error_message || text == self.edit_text(at) {
            return Ok(None);
        }
        if text.starts_with('=') && typing::formula_problem(text.trim_start_matches('=')) {
            return Ok(None);
        }
        let date1904 = matches!(
            self.engine().workbook().date_system,
            jade_xlsx_reader::DateSystem::Nineteen04
        );
        let input = typing::read(text, date1904).input;
        let candidate = match input {
            CellInput::Number(n) => Candidate::Value(CellValue::Number(n)),
            CellInput::Text(s) => Candidate::Value(CellValue::Text(s)),
            CellInput::Bool(b) => Candidate::Value(CellValue::Bool(b)),
            CellInput::Formula(f) => Candidate::Formula(f),
            CellInput::Blank => Candidate::Value(CellValue::Empty),
        };
        let result = jade_xlsx_calc::validation::validate(
            self.engine().workbook(),
            self.sheet_index(),
            at,
            rule,
            &candidate,
            self.engine().calculation_time_zone(),
        )
        .map_err(|e| e.to_string())?;
        if result == ValidationResult::Valid {
            return Ok(None);
        }
        let style = match rule.error_style.as_deref() {
            Some("warning") => AlertStyle::Warning,
            Some("information") => AlertStyle::Information,
            _ => AlertStyle::Stop,
        };
        Ok(Some(ValidationAlert {
            style: if result == ValidationResult::Unsupported {
                AlertStyle::Stop
            } else {
                style
            },
            title: rule.error_title.clone(),
            message: rule.error.clone(),
            unsupported: result == ValidationResult::Unsupported,
        }))
    }
    pub fn validation_prompt(&self, at: CellRef) -> Option<(String, String)> {
        let rule = self.data_validation_at(at)?;
        rule.show_input_message
            .then(|| {
                (
                    rule.prompt_title.clone().unwrap_or_default(),
                    rule.prompt.clone().unwrap_or_default(),
                )
            })
            .filter(|(a, b)| !a.is_empty() || !b.is_empty())
    }
    pub fn validation_dropdown(&self, at: CellRef) -> Option<Vec<String>> {
        if self.data_validation_at(at)?.suppress_dropdown {
            return None;
        }
        self.validation_list(at)
    }
    /// A dropdown represents a source value, not a new formula. Preserve
    /// formula-looking/apostrophe text and retry as text when ordinary typing
    /// would coerce a valid textual choice into a different value type.
    pub fn set_validation_choice(&mut self, at: CellRef, choice: &str) -> Result<bool, String> {
        let items = self
            .validation_list(at)
            .ok_or("The validation choices are unavailable")?;
        if !items.iter().any(|s| s == choice) {
            return Err("The validation choice is no longer available".into());
        }
        let literal =
            choice.starts_with(['=', '\'']) || self.validation_alert(at, choice)?.is_some();
        let input = if literal {
            format!("'{choice}")
        } else {
            choice.to_owned()
        };
        self.set_cell(at, &input)
    }
}

pub const TYPES: [(&str, &str); 8] = [
    ("none", "validation.any"),
    ("whole", "validation.whole"),
    ("decimal", "validation.decimal"),
    ("list", "validation.list"),
    ("date", "validation.date"),
    ("time", "validation.time"),
    ("textLength", "validation.text_length"),
    ("custom", "validation.custom"),
];
pub const OPERATORS: [(&str, &str); 8] = [
    ("between", "validation.between"),
    ("notBetween", "validation.not_between"),
    ("equal", "validation.equal"),
    ("notEqual", "validation.not_equal"),
    ("greaterThan", "validation.greater"),
    ("lessThan", "validation.less"),
    ("greaterThanOrEqual", "validation.greater_equal"),
    ("lessThanOrEqual", "validation.less_equal"),
];
pub const STYLES: [(&str, &str); 3] = [
    ("stop", "validation.stop"),
    ("warning", "validation.warning"),
    ("information", "validation.information"),
];
pub const IME_MODES: [&str; 11] = [
    "noControl",
    "off",
    "on",
    "disabled",
    "hiragana",
    "fullKatakana",
    "halfKatakana",
    "fullAlpha",
    "halfAlpha",
    "fullHangul",
    "halfHangul",
];
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationTab {
    Settings,
    Input,
    Error,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationField {
    Ranges,
    First,
    Second,
    PromptTitle,
    Prompt,
    ErrorTitle,
    Error,
}
impl ValidationField {
    pub fn key(self) -> &'static str {
        match self {
            Self::Ranges => "validation.ranges",
            Self::First => "validation.first",
            Self::Second => "validation.second",
            Self::PromptTitle | Self::ErrorTitle => "validation.title",
            Self::Prompt | Self::Error => "validation.message",
        }
    }
    fn index(self) -> usize {
        match self {
            Self::Ranges => 0,
            Self::First => 1,
            Self::Second => 2,
            Self::PromptTitle => 3,
            Self::Prompt => 4,
            Self::ErrorTitle => 5,
            Self::Error => 6,
        }
    }
}
pub struct ValidationDialog {
    pub tab: ValidationTab,
    pub field: ValidationField,
    pub kind: usize,
    pub operator: usize,
    pub style: usize,
    pub ime: usize,
    pub allow_blank: bool,
    pub dropdown: bool,
    pub show_input: bool,
    pub show_error: bool,
    pub fields: Vec<TextEdit>,
    pub apply_to_same: bool,
    same_ranges: Vec<CellRange>,
    pub input_bounds: std::rc::Rc<std::cell::Cell<Option<[f32; 4]>>>,
    date1904: bool,
    pub problem: Option<&'static str>,
}
impl ValidationDialog {
    pub fn open<B: Backend>(book: &Workbook<B>, range: CellRange, at: CellRef) -> Self {
        let existing = book.data_validation_at(at).cloned();
        let new = existing.is_none();
        let same_ranges = existing
            .as_ref()
            .map(|old| {
                book.data_validations()
                    .iter()
                    .filter(|r| same_settings(old, r))
                    .flat_map(|r| r.ranges.clone())
                    .collect()
            })
            .unwrap_or_default();
        let old = existing.unwrap_or_default();
        let date1904 = matches!(
            book.engine().workbook().date_system,
            jade_xlsx_reader::DateSystem::Nineteen04
        );
        let origin = old.ranges.first().map(|r| r.start).unwrap_or(range.start);
        let formula = |s: Option<String>| {
            let shifted =
                jade_xlsx_reader::translate_formula(&s.unwrap_or_default(), origin, range.start);
            jade_xlsx_headless_editor::formula_text::to_display_formula(&shifted)
        };
        let mut first = if let Some(literal) = &old.literal_list {
            jade_xlsx_reader::data_validations::parse_literal_list(literal)
                .map(|items| jade_xlsx_reader::data_validations::format_literal_list(&items))
                .unwrap_or_else(|| literal.clone())
        } else {
            formula(old.formula1)
        };
        if old.validation_type.as_deref() == Some("list") && old.literal_list.is_none() {
            first = if let Some(text) = first.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
                let values = text
                    .replace("\"\"", "\"")
                    .split(',')
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                jade_xlsx_reader::data_validations::format_literal_list(&values)
            } else if first.is_empty() {
                first
            } else {
                format!("={first}")
            };
        }
        let range = crate::sheet::range_text(range);
        let fields = vec![
            range,
            first,
            formula(old.formula2),
            old.prompt_title.unwrap_or_default(),
            old.prompt.unwrap_or_default(),
            old.error_title.unwrap_or_default(),
            old.error.unwrap_or_default(),
        ]
        .into_iter()
        .map(TextEdit::new)
        .collect();
        Self {
            tab: ValidationTab::Settings,
            field: ValidationField::Ranges,
            kind: TYPES
                .iter()
                .position(|(s, _)| Some(*s) == old.validation_type.as_deref())
                .unwrap_or(0),
            operator: OPERATORS
                .iter()
                .position(|(s, _)| Some(*s) == old.operator.as_deref())
                .unwrap_or(0),
            style: STYLES
                .iter()
                .position(|(s, _)| Some(*s) == old.error_style.as_deref())
                .unwrap_or(0),
            ime: IME_MODES
                .iter()
                .position(|s| Some(*s) == old.ime_mode.as_deref())
                .unwrap_or(0),
            allow_blank: old.allow_blank || new,
            dropdown: !old.suppress_dropdown,
            show_input: old.show_input_message || new,
            show_error: old.show_error_message || new,
            fields,
            apply_to_same: false,
            same_ranges,
            input_bounds: std::rc::Rc::new(std::cell::Cell::new(None)),
            date1904,
            problem: None,
        }
    }
    pub fn visible_fields(&self) -> Vec<ValidationField> {
        use ValidationField::*;
        match self.tab {
            ValidationTab::Settings => {
                let mut fields = vec![Ranges];
                if self.kind != 0 {
                    fields.push(First);
                }
                if !matches!(TYPES[self.kind].0, "none" | "list" | "custom")
                    && matches!(OPERATORS[self.operator].0, "between" | "notBetween")
                {
                    fields.push(Second);
                }
                fields
            }
            ValidationTab::Input => vec![PromptTitle, Prompt],
            ValidationTab::Error => vec![ErrorTitle, Error],
        }
    }
    pub fn select_tab(&mut self, tab: ValidationTab) {
        self.input_bounds.set(None);
        self.tab = tab;
        self.field = self.visible_fields()[0];
        self.field_mut().whole = true;
    }
    pub fn field_mut(&mut self) -> &mut TextEdit {
        &mut self.fields[self.field.index()]
    }
    pub fn field(&self) -> &TextEdit {
        &self.fields[self.field.index()]
    }
    pub fn value(&self, field: ValidationField) -> &str {
        &self.fields[field.index()].text
    }
    pub fn focus(&mut self, field: ValidationField) {
        self.input_bounds.set(None);
        self.field = field;
        self.field_mut().whole = true;
    }
    pub fn key(&mut self, key: &str, shift: bool, command: bool) -> bool {
        if key == "tab" {
            let fields = self.visible_fields();
            let at = fields.iter().position(|f| *f == self.field).unwrap_or(0);
            self.focus(
                fields[if shift {
                    (at + fields.len() - 1) % fields.len()
                } else {
                    (at + 1) % fields.len()
                }],
            );
            return true;
        }
        if command && key == "a" {
            self.field_mut().whole = true;
            return true;
        }
        self.field_mut().edit_key(key)
    }
    pub fn rule(&self) -> Result<DataValidation, &'static str> {
        let origin = self.ranges()?[0].start;
        let ranges = self.target_ranges()?;
        let optional = |field| {
            let s = self.value(field);
            (!s.is_empty()).then(|| s.to_string())
        };
        if self.kind != 0 && optional(ValidationField::First).is_none() {
            return Err("validation.missing_formula");
        }
        if self.visible_settings_second() && optional(ValidationField::Second).is_none() {
            return Err("validation.missing_formula");
        }
        let bound = |field| {
            optional(field).map(|s| {
                if let CellInput::Number(n) = typing::read(&s, self.date1904).input {
                    n.to_string()
                } else {
                    s
                }
            })
        };
        let mut literal_list = None;
        let first = if self.kind == 0 {
            None
        } else if TYPES[self.kind].0 == "list" {
            let source = optional(ValidationField::First).unwrap_or_default();
            if source.starts_with('=') {
                Some(source.trim_start_matches('=').into())
            } else {
                let items = jade_xlsx_reader::data_validations::parse_literal_list(&source)
                    .ok_or("validation.invalid_list")?;
                let xml_scalar = |c| {
                    matches!(c,
                    '\u{9}'|'\u{A}'|'\u{D}'|'\u{20}'..='\u{D7FF}'|
                    '\u{E000}'..='\u{FFFD}'|'\u{10000}'..='\u{10FFFF}')
                };
                let rich = items
                    .iter()
                    .any(|s| s.contains([',', '\r', '\n']) || !s.chars().all(xml_scalar));
                if rich {
                    literal_list = Some(jade_xlsx_reader::data_validations::format_literal_list(
                        &items,
                    ));
                    None
                } else {
                    Some(format!("\"{}\"", items.join(",").replace('"', "\"\"")))
                }
            }
        } else {
            bound(ValidationField::First)
        };
        let second = if self.visible_settings_second() {
            bound(ValidationField::Second)
        } else {
            None
        };
        let target = ranges[0].start;
        let first = first.map(|f| jade_xlsx_reader::translate_formula(&f, origin, target));
        let second = second.map(|f| jade_xlsx_reader::translate_formula(&f, origin, target));
        Ok(DataValidation {
            ranges,
            validation_type: Some(TYPES[self.kind].0.into()),
            operator: Some(OPERATORS[self.operator].0.into()),
            allow_blank: self.allow_blank,
            show_input_message: self.show_input,
            show_error_message: self.show_error,
            error_style: Some(STYLES[self.style].0.into()),
            suppress_dropdown: !self.dropdown,
            ime_mode: Some(IME_MODES[self.ime].into()),
            formula1: first,
            formula2: second,
            literal_list,
            prompt_title: optional(ValidationField::PromptTitle),
            prompt: optional(ValidationField::Prompt),
            error_title: optional(ValidationField::ErrorTitle),
            error: optional(ValidationField::Error),
        })
    }
    pub fn ranges(&self) -> Result<Vec<CellRange>, &'static str> {
        let ranges = self
            .value(ValidationField::Ranges)
            .split_ascii_whitespace()
            .map(CellRange::parse)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "validation.bad_range")?;
        if ranges.is_empty()
            || ranges
                .iter()
                .any(|r| r.start.column > r.end.column || r.start.row > r.end.row)
        {
            return Err("validation.bad_range");
        }
        Ok(ranges)
    }
    pub fn has_matching(&self) -> bool {
        !self.same_ranges.is_empty()
    }
    pub fn target_ranges(&self) -> Result<Vec<CellRange>, &'static str> {
        let entered = self.ranges()?;
        if !self.apply_to_same {
            return Ok(entered);
        }
        let contains = |a: CellRange, b: CellRange| a.contains(b.start) && a.contains(b.end);
        let mut ranges = self.same_ranges.clone();
        for range in entered {
            if ranges.iter().any(|r| contains(*r, range)) {
                continue;
            }
            ranges.retain(|r| !contains(range, *r));
            ranges.push(range);
        }
        Ok(ranges)
    }
    fn visible_settings_second(&self) -> bool {
        !matches!(TYPES[self.kind].0, "none" | "list" | "custom")
            && matches!(OPERATORS[self.operator].0, "between" | "notBetween")
    }
}

fn same_settings(a: &DataValidation, b: &DataValidation) -> bool {
    let normalized = |r: &DataValidation| {
        let mut r = r.clone();
        r.ranges.clear();
        r.validation_type = Some(r.validation_type.unwrap_or_else(|| "none".into()));
        r.operator = Some(r.operator.unwrap_or_else(|| "between".into()));
        r.error_style = Some(r.error_style.unwrap_or_else(|| "stop".into()));
        r.ime_mode = Some(r.ime_mode.unwrap_or_else(|| "noControl".into()));
        r
    };
    normalized(a) == normalized(b)
}

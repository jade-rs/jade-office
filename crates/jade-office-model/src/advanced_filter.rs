//! Data ▸ Advanced: a list's records filtered by a criteria range, as
//! Excel's Advanced Filter does — field names along the criteria's top row,
//! the conditions of a row all to hold and any row to do, a condition read
//! as Excel reads it (`>5`, `<>`, `=`, `Dav*`, text a record starts with),
//! and a formula under a name that is no field's worked out for each record
//! from the first — the list filtered in place, or the records copied to
//! another place on the sheet, Unique records only leaving out repeats.

use std::collections::{BTreeSet, HashSet};

use jade_xlsx_headless_api::CellInput;
use jade_xlsx_reader::{quote_sheet_name, CellRange, CellRef, CellValue, DefinedName};

use crate::form::{Form, Label};
use crate::sheet::{reference_text, Workbook};
use crate::what_if::{absolute_range, Outcome, ToolDialog};
use crate::Backend;

const FILTER_DATABASE: &str = "_xlnm._FilterDatabase";
const CRITERIA: &str = "_xlnm.Criteria";
const EXTRACT: &str = "_xlnm.Extract";

/// What an Advanced Filter is asked to do.
#[derive(Debug, Clone, PartialEq)]
pub struct AdvancedFilter {
    /// The list, on the active sheet: field names along its first row.
    pub list: CellRange,
    /// The criteria range: its sheet, by index, and cells; `None` keeps
    /// every record, as Excel does with Unique records only.
    pub criteria: Option<(usize, CellRange)>,
    /// Copy to another location: where on the active sheet, its first row
    /// the field names to copy when it has any; `None` filters in place.
    pub copy_to: Option<CellRange>,
    pub unique: bool,
}

/// How a condition compares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
}

/// One condition of a criteria row.
#[derive(Debug, Clone, PartialEq)]
enum Condition {
    /// A field's value against what the criteria cell says, by its place in
    /// the list.
    Field {
        column: u32,
        op: Op,
        /// The text after the operator; a plain value is a text the record
        /// starts with.
        operand: String,
        starts: bool,
        /// The criteria cell held a number, or a boolean: matched as one.
        value: Option<CellValue>,
    },
    /// A computed criterion: the criteria cell's formula, written for the
    /// first record, holding for each — the records' rows it holds at.
    Computed(HashSet<u32>),
}

/// `text` against a pattern with Excel's wildcards: `*` any run, `?` one
/// character, `~` the next one as it is. Without case.
fn wild_match(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.to_lowercase().chars().collect();
    let text: Vec<char> = text.to_lowercase().chars().collect();
    // Tokens: Some(c) a literal (or `?` as None-marked), star as a flag.
    #[derive(Clone, Copy)]
    enum T {
        Char(char),
        One,
        Any,
    }
    let mut tokens = Vec::new();
    let mut chars = pattern.iter().copied();
    while let Some(c) = chars.next() {
        tokens.push(match c {
            '~' => T::Char(chars.next().unwrap_or('~')),
            '*' => T::Any,
            '?' => T::One,
            c => T::Char(c),
        });
    }
    // Two-pointer match with backtracking to the last star.
    let (mut p, mut t) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while t < text.len() {
        match tokens.get(p) {
            Some(T::Char(c)) if *c == text[t] => {
                p += 1;
                t += 1;
            }
            Some(T::One) => {
                p += 1;
                t += 1;
            }
            Some(T::Any) => {
                star = Some((p, t));
                p += 1;
            }
            _ => match star {
                Some((sp, st)) => {
                    p = sp + 1;
                    t = st + 1;
                    star = Some((sp, st + 1));
                }
                None => return false,
            },
        }
    }
    tokens[p..].iter().all(|token| matches!(token, T::Any))
}

/// A criteria cell's condition: its operator and the rest.
fn split_operator(text: &str) -> (Op, &str, bool) {
    for (prefix, op) in [
        (">=", Op::GreaterEqual),
        ("<=", Op::LessEqual),
        ("<>", Op::NotEqual),
        ("=", Op::Equal),
        (">", Op::Greater),
        ("<", Op::Less),
    ] {
        if let Some(rest) = text.strip_prefix(prefix) {
            return (op, rest, false);
        }
    }
    (Op::Equal, text, true)
}

impl Condition {
    /// Whether the record at `row`, its field holding `value` and showing
    /// `shown`, meets the condition.
    fn holds(&self, row: u32, value: &CellValue, shown: &str) -> bool {
        let (op, operand, starts, typed) = match self {
            Condition::Computed(rows) => return rows.contains(&row),
            Condition::Field {
                op,
                operand,
                starts,
                value,
                ..
            } => (*op, operand.as_str(), *starts, value),
        };
        let compare = |order: std::cmp::Ordering| match op {
            Op::Equal => order.is_eq(),
            Op::NotEqual => !order.is_eq(),
            Op::Greater => order.is_gt(),
            Op::GreaterEqual => order.is_ge(),
            Op::Less => order.is_lt(),
            Op::LessEqual => order.is_le(),
        };
        if let Some(typed) = typed {
            return match (typed, value) {
                (CellValue::Number(want), CellValue::Number(have)) => {
                    have.partial_cmp(want).is_some_and(compare)
                }
                (CellValue::Bool(want), CellValue::Bool(have)) => compare(have.cmp(want)),
                _ => op == Op::NotEqual,
            };
        }
        let empty = matches!(value, CellValue::Empty)
            || matches!(value, CellValue::Text(text) if text.is_empty());
        if operand.is_empty() {
            // `=` alone: blank fields; `<>` alone: any but blank.
            return match op {
                Op::Equal => empty,
                Op::NotEqual => !empty,
                _ => false,
            };
        }
        if let Ok(number) = operand.trim().parse::<f64>() {
            return match value {
                CellValue::Number(have) => have.partial_cmp(&number).is_some_and(compare),
                CellValue::Text(text) if matches!(op, Op::Equal | Op::NotEqual) => {
                    let same = text.trim().eq_ignore_ascii_case(operand.trim());
                    same == (op == Op::Equal)
                }
                _ => op == Op::NotEqual,
            };
        }
        match op {
            Op::Equal | Op::NotEqual => {
                let pattern = if starts {
                    format!("{operand}*")
                } else {
                    operand.to_string()
                };
                let text = match value {
                    CellValue::Text(text) => text.as_str(),
                    _ => shown,
                };
                wild_match(&pattern, text) == (op == Op::Equal)
            }
            _ => match value {
                CellValue::Text(text) => {
                    compare(text.to_lowercase().as_str().cmp(&operand.to_lowercase()))
                }
                _ => false,
            },
        }
    }
}

impl<B: Backend> Workbook<B> {
    /// What a cell of sheet `sheet` shows.
    fn shown_on(&self, sheet: usize, at: CellRef) -> String {
        self.sheet_names()
            .get(sheet)
            .and_then(|name| self.engine().cell(name, &reference_text(at)).ok())
            .map(|cell| cell.display)
            .unwrap_or_default()
    }

    fn held_on(&self, sheet: usize, at: CellRef) -> CellValue {
        self.engine().workbook().sheets[sheet]
            .cell_at(at)
            .map(|cell| cell.value.clone())
            .unwrap_or(CellValue::Empty)
    }

    /// The criteria range's rows of conditions; refused when it has no row
    /// under its names, or a formula the engine cannot read.
    fn criteria_rows(
        &self,
        list: CellRange,
        criteria: (usize, CellRange),
    ) -> Result<Vec<Vec<Condition>>, String> {
        let (sheet, range) = criteria;
        let here = self.sheet_index();
        let fields: Vec<String> = (list.start.column..=list.end.column)
            .map(|column| {
                self.shown_on(here, CellRef::new(column, list.start.row))
                    .trim()
                    .to_lowercase()
            })
            .collect();
        let records: Vec<u32> = (list.start.row + 1..=list.end.row).collect();
        let mut probe = None;
        let mut rows = Vec::new();
        for row in range.start.row + 1..=range.end.row {
            let mut conditions = Vec::new();
            for column in range.start.column..=range.end.column {
                let at = CellRef::new(column, row);
                let name = self
                    .shown_on(sheet, CellRef::new(column, range.start.row))
                    .trim()
                    .to_lowercase();
                let value = self.held_on(sheet, at);
                let field = fields
                    .iter()
                    .position(|field| *field == name && !name.is_empty());
                let formula = self.engine().workbook().sheets[sheet]
                    .cell_at(at)
                    .and_then(|cell| cell.formula.clone());
                match field {
                    None => {
                        // A computed criterion: worked out for each record,
                        // its references moved down from the first.
                        let Some(formula) = formula else {
                            if matches!(value, CellValue::Empty) {
                                continue;
                            }
                            // A value under a name that is no field's:
                            // nothing meets it, as in Excel.
                            conditions.push(Condition::Computed(HashSet::new()));
                            continue;
                        };
                        if probe.is_none() {
                            probe =
                                Some(self.engine().what_if().map_err(|error| error.to_string())?);
                        }
                        let cells: Vec<CellRef> = records
                            .iter()
                            .map(|record| CellRef::new(column, row + (record - records[0])))
                            .collect();
                        let holds = probe
                            .as_mut()
                            .expect("the probe was made")
                            .formula_holds(self.engine().workbook(), sheet, at, &formula, &cells)
                            .map_err(|error| error.to_string())?
                            .ok_or_else(|| "advanced_filter.alert.criteria".to_string())?;
                        conditions.push(Condition::Computed(
                            records
                                .iter()
                                .zip(holds)
                                .filter(|(_, holds)| *holds)
                                .map(|(record, _)| *record)
                                .collect(),
                        ));
                    }
                    Some(index) => {
                        let condition = match &value {
                            CellValue::Empty => continue,
                            CellValue::Number(_) | CellValue::Bool(_) => Condition::Field {
                                column: index as u32,
                                op: Op::Equal,
                                operand: String::new(),
                                starts: false,
                                value: Some(value.clone()),
                            },
                            _ => {
                                let text = match &value {
                                    CellValue::Text(text) => text.clone(),
                                    _ => self.shown_on(sheet, at),
                                };
                                let (op, rest, starts) = split_operator(&text);
                                Condition::Field {
                                    column: index as u32,
                                    op,
                                    operand: rest.to_string(),
                                    starts,
                                    value: None,
                                }
                            }
                        };
                        conditions.push(condition);
                    }
                }
            }
            rows.push(conditions);
        }
        if rows.is_empty() {
            return Err("advanced_filter.alert.criteria".into());
        }
        Ok(rows)
    }

    /// The rows of the records of `list` an Advanced Filter keeps.
    pub fn advanced_filter_rows(&self, job: &AdvancedFilter) -> Result<Vec<u32>, String> {
        let list = job.list;
        if list.end.row <= list.start.row {
            return Err("advanced_filter.alert.list".into());
        }
        let here = self.sheet_index();
        let rows = match job.criteria {
            Some(criteria) => Some(self.criteria_rows(list, criteria)?),
            None => None,
        };
        let mut kept = Vec::new();
        let mut seen = HashSet::new();
        let columns: Vec<u32> = (list.start.column..=list.end.column).collect();
        for row in list.start.row + 1..=list.end.row {
            let meets = rows.as_ref().is_none_or(|rows| {
                rows.iter().any(|conditions| {
                    conditions.iter().all(|condition| {
                        let column = match condition {
                            Condition::Field { column, .. } => list.start.column + column,
                            Condition::Computed(_) => list.start.column,
                        };
                        let at = CellRef::new(column, row);
                        condition.holds(row, &self.held_on(here, at), &self.shown_on(here, at))
                    })
                })
            });
            if !meets {
                continue;
            }
            if job.unique {
                let record: Vec<String> = columns
                    .iter()
                    .map(|&column| {
                        self.shown_on(here, CellRef::new(column, row))
                            .to_lowercase()
                    })
                    .collect();
                if !seen.insert(record) {
                    continue;
                }
            }
            kept.push(row);
        }
        Ok(kept)
    }

    /// Data ▸ Advanced on the active sheet, in one undo step: how many
    /// records it kept.
    pub fn advanced_filter(&mut self, job: &AdvancedFilter) -> Result<u32, String> {
        let kept = self.advanced_filter_rows(job)?;
        let name = self.sheet_name();
        let Some(copy_to) = job.copy_to else {
            let shown: BTreeSet<u32> = kept.iter().copied().collect();
            let count = self
                .engine_mut()
                .advanced_filter(&name, job.list, &shown, job.criteria)
                .map_err(|error| error.to_string())?;
            self.wrote(1)?;
            return Ok(count);
        };
        let here = self.sheet_index();
        let list = job.list;
        let fields: Vec<String> = (list.start.column..=list.end.column)
            .map(|column| {
                self.shown_on(here, CellRef::new(column, list.start.row))
                    .trim()
                    .to_lowercase()
            })
            .collect();
        // The fields to copy: those named along Copy to's first row, or
        // every one when it names none.
        let named: Vec<String> = (copy_to.start.column..=copy_to.end.column)
            .map(|column| {
                self.shown_on(here, CellRef::new(column, copy_to.start.row))
                    .trim()
                    .to_lowercase()
            })
            .collect();
        let columns: Vec<u32> = if named.iter().all(|name| name.is_empty()) {
            (list.start.column..=list.end.column).collect()
        } else {
            let mut columns = Vec::new();
            for name in named.iter().filter(|name| !name.is_empty()) {
                let Some(index) = fields.iter().position(|field| field == name) else {
                    return Err("advanced_filter.alert.extract".into());
                };
                columns.push(list.start.column + index as u32);
            }
            columns
        };
        let overlaps = |a: CellRange, b: CellRange| {
            a.start.row <= b.end.row
                && b.start.row <= a.end.row
                && a.start.column <= b.end.column
                && b.start.column <= a.end.column
        };
        let top = copy_to.start;
        let width = columns.len() as u32;
        // Only the header row given: the records go on down, what an earlier
        // copy left there cleared first, as Excel clears it.
        let open_ended = copy_to.start.row == copy_to.end.row;
        let room = if open_ended {
            u32::MAX
        } else {
            copy_to.end.row - copy_to.start.row
        };
        let records: Vec<u32> = kept.iter().copied().take(room as usize).collect();
        let extract = CellRange {
            start: top,
            end: CellRef::new(top.column + width - 1, top.row + records.len() as u32),
        };
        if overlaps(extract, list) {
            return Err("advanced_filter.alert.overlap".into());
        }
        let mut cells: Vec<(String, CellInput)> = Vec::new();
        if open_ended {
            let used = self.engine().workbook().sheets[here]
                .rows
                .last()
                .map_or(0, |row| row.index);
            for row in top.row + 1..=used.max(top.row) {
                for column in top.column..top.column + width {
                    let at = CellRef::new(column, row);
                    if !matches!(self.held_on(here, at), CellValue::Empty) {
                        cells.push((reference_text(at), CellInput::Blank));
                    }
                }
            }
        }
        let value = |cell: CellValue, shown: String| match cell {
            CellValue::Number(number) => CellInput::Number(number),
            CellValue::Bool(on) => CellInput::Bool(on),
            CellValue::Empty => CellInput::Blank,
            CellValue::Text(text) => CellInput::Text(text),
            _ => CellInput::Text(shown),
        };
        for (offset, &column) in columns.iter().enumerate() {
            let at = |row: u32| reference_text(CellRef::new(top.column + offset as u32, row));
            let header = CellRef::new(column, list.start.row);
            cells.push((
                at(top.row),
                value(self.held_on(here, header), self.shown_on(here, header)),
            ));
            for (index, &row) in records.iter().enumerate() {
                let from = CellRef::new(column, row);
                cells.push((
                    at(top.row + 1 + index as u32),
                    value(self.held_on(here, from), self.shown_on(here, from)),
                ));
            }
        }
        let names = self.sheet_names();
        let sheet_range = |sheet: usize, range: CellRange| {
            format!(
                "{}!{}",
                quote_sheet_name(&names[sheet]),
                absolute_range(range)
            )
        };
        let mut definitions = vec![
            DefinedName {
                name: FILTER_DATABASE.into(),
                formula: sheet_range(here, list),
                local_sheet_index: Some(here),
                hidden: true,
            },
            DefinedName {
                name: EXTRACT.into(),
                formula: sheet_range(
                    here,
                    CellRange {
                        start: top,
                        end: CellRef::new(top.column + width - 1, top.row),
                    },
                ),
                local_sheet_index: Some(here),
                hidden: false,
            },
        ];
        if let Some((sheet, range)) = job.criteria {
            definitions.push(DefinedName {
                name: CRITERIA.into(),
                formula: sheet_range(sheet, range),
                local_sheet_index: Some(here),
                hidden: false,
            });
        }
        self.atomic_edit(|book| {
            let mut steps = 0;
            book.engine_mut()
                .set_cells(&name, &cells)
                .map_err(|error| error.to_string())?;
            steps += 1;
            // The copied fields look as the list's do: its header's format on
            // the header, its first record's down the records.
            for (offset, &column) in columns.iter().enumerate() {
                let to = top.column + offset as u32;
                let pairs = [(list.start.row, top.row, top.row)].into_iter().chain(
                    (!records.is_empty()).then_some((
                        list.start.row + 1,
                        top.row + 1,
                        top.row + records.len() as u32,
                    )),
                );
                for (from, first, last) in pairs {
                    book.engine_mut()
                        .copy_format(
                            &name,
                            &reference_text(CellRef::new(column, from)),
                            &format!(
                                "{}:{}",
                                reference_text(CellRef::new(to, first)),
                                reference_text(CellRef::new(to, last))
                            ),
                        )
                        .map_err(|error| error.to_string())?;
                    steps += 1;
                }
            }
            if book
                .engine_mut()
                .set_built_in_names(&definitions, &[])
                .map_err(|error| error.to_string())?
            {
                steps += 1;
            }
            book.wrote(steps)?;
            Ok(records.len() as u32)
        })
    }

    /// Show All after an Advanced Filter in place: its records shown
    /// again. One undo step; false when the sheet was not filtered so.
    pub fn show_all_records(&mut self) -> Result<bool, String> {
        let name = self.sheet_name();
        let shown = self
            .engine_mut()
            .show_all_records(&name)
            .map_err(|error| error.to_string())?;
        if shown {
            self.wrote(1)?;
        }
        Ok(shown)
    }

    /// Whether the active sheet has records an Advanced Filter hid.
    pub fn advanced_filtered(&self) -> bool {
        let sheet = &self.engine().workbook().sheets[self.sheet_index()];
        sheet.filter_mode == Some(true)
            && sheet.auto_filter.is_none()
            && self
                .engine()
                .filter_database(&self.sheet_name())
                .ok()
                .flatten()
                .is_some()
    }

    /// The range a sheet-scoped built-in name of the active sheet names,
    /// as the dialog fills it in: `$A$1:$C$3`, or `Sheet2!$A$1:$C$3` off
    /// the sheet.
    fn local_name_text(&self, name: &str) -> String {
        let here = self.sheet_index();
        self.defined_names()
            .iter()
            .find(|defined| {
                defined.local_sheet_index == Some(here) && defined.name.eq_ignore_ascii_case(name)
            })
            .and_then(|defined| {
                let formula = defined.formula.trim_start_matches('=');
                let (sheet, range) = self.resolve_reference(formula)?;
                Some(if sheet == here {
                    absolute_range(range)
                } else {
                    format!(
                        "{}!{}",
                        quote_sheet_name(&self.sheet_names()[sheet]),
                        absolute_range(range)
                    )
                })
            })
            .unwrap_or_default()
    }
}

/// The Advanced Filter dialog: Action, List range, Criteria range, Copy to
/// and Unique records only — and Show All, with the sheet filtered.
pub struct AdvancedFilterDialog {
    pub form: Form,
}

impl AdvancedFilterDialog {
    /// List range starts as the list round the active cell, the criteria
    /// and Copy to as the sheet's names for them, as in Excel.
    pub fn open<B: Backend>(book: &Workbook<B>, selected: CellRange) -> Self {
        let list = if selected.start == selected.end {
            book.current_region(selected.start).unwrap_or(selected)
        } else {
            selected
        };
        let mut form = Form::new("advanced_filter.dialog")
            .choice(
                "action",
                "advanced_filter.action",
                vec![
                    Label::Key("advanced_filter.in_place"),
                    Label::Key("advanced_filter.copy"),
                ],
                0,
            )
            .field("list", "advanced_filter.list", &absolute_range(list))
            .field(
                "criteria",
                "advanced_filter.criteria",
                &book.local_name_text(CRITERIA),
            )
            .field(
                "copy_to",
                "advanced_filter.copy_to",
                &book.local_name_text(EXTRACT),
            )
            .check("unique", "advanced_filter.unique", false)
            .button("ok", "validation.ok")
            .button("cancel", "validation.cancel");
        if book.advanced_filtered() {
            form = form.button("show_all", "advanced_filter.show_all");
        }
        Self { form }
    }
}

impl ToolDialog for AdvancedFilterDialog {
    fn form(&self) -> &Form {
        &self.form
    }

    fn form_mut(&mut self) -> &mut Form {
        &mut self.form
    }

    fn press<B: Backend>(
        &mut self,
        button: &str,
        book: &mut Workbook<B>,
    ) -> Result<Outcome, String> {
        let form = &mut self.form;
        form.alert = None;
        match button {
            "show_all" => {
                book.show_all_records()?;
                Ok(Outcome::closed_writing())
            }
            "ok" => {
                let alert = |form: &mut Form, key: &'static str, focus: usize| {
                    form.alert = Some(Label::Key(key));
                    form.focus = Some(focus);
                    Ok(Outcome::stay())
                };
                let here = book.sheet_index();
                let list = match book.resolve_reference(form.text("list")) {
                    Some((sheet, range)) if sheet == here => range,
                    _ => return alert(form, "advanced_filter.alert.list", 1),
                };
                let criteria = match form.text("criteria").trim() {
                    "" => None,
                    text => match book.resolve_reference(text) {
                        Some(found) => Some(found),
                        None => return alert(form, "advanced_filter.alert.criteria", 2),
                    },
                };
                let copy_to = if form.chosen("action") == Some(1) {
                    match book.resolve_reference(form.text("copy_to")) {
                        Some((sheet, range)) if sheet == here => Some(range),
                        Some(_) => return alert(form, "advanced_filter.alert.active_sheet", 3),
                        None => return alert(form, "advanced_filter.alert.extract", 3),
                    }
                } else {
                    None
                };
                let job = AdvancedFilter {
                    list,
                    criteria,
                    copy_to,
                    unique: form.checked("unique"),
                };
                match book.advanced_filter(&job) {
                    Ok(_) => Ok(Outcome::closed_writing()),
                    Err(error) if error.starts_with("advanced_filter.alert.") => {
                        form.alert = Some(Label::Key(alert_key(&error)));
                        Ok(Outcome::stay())
                    }
                    Err(error) => Err(error),
                }
            }
            _ => Ok(Outcome::close()),
        }
    }
}

fn alert_key(error: &str) -> &'static str {
    match error {
        "advanced_filter.alert.criteria" => "advanced_filter.alert.criteria",
        "advanced_filter.alert.extract" => "advanced_filter.alert.extract",
        "advanced_filter.alert.overlap" => "advanced_filter.alert.overlap",
        "advanced_filter.alert.active_sheet" => "advanced_filter.alert.active_sheet",
        _ => "advanced_filter.alert.list",
    }
}

#[cfg(test)]
mod tests {
    use super::wild_match;

    #[test]
    fn wildcards_match_as_excel_s_do() {
        assert!(wild_match("dav*", "Davolio"));
        assert!(wild_match("sm?th", "smith"));
        assert!(!wild_match("sm?th", "smyyth"));
        assert!(wild_match("*a*b", "xxaxxb"));
        assert!(wild_match("~*", "*"));
        assert!(!wild_match("~*", "a"));
        assert!(wild_match("*", ""));
    }
}

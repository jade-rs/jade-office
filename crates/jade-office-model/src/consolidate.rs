//! Data ▸ Consolidate: the values of several ranges — on this sheet or
//! others — summed (or counted, averaged …) into one table at the active
//! cell, matched by where they sit or by the labels along their top row and
//! down their left column, as Excel's dialog does it. With Create links to
//! source data, each result is a formula over detail rows that link to the
//! sources, grouped in an outline and collapsed under it.

use std::collections::HashMap;

use jade_xlsx_headless_api::CellInput;
use jade_xlsx_reader::{quote_sheet_name, CellRange, CellRef, CellValue};

use crate::form::{Form, Label};
use crate::sheet::{column_label, reference_text, Axis, OutlineChange, Workbook};
use crate::what_if::{absolute_range, Outcome, ToolDialog};
use crate::Backend;

/// The functions Consolidate offers, in its list's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsolidateFunction {
    Sum,
    Count,
    Average,
    Max,
    Min,
    Product,
    CountNumbers,
    StdDev,
    StdDevP,
    Var,
    VarP,
}

impl ConsolidateFunction {
    pub const ALL: [ConsolidateFunction; 11] = [
        Self::Sum,
        Self::Count,
        Self::Average,
        Self::Max,
        Self::Min,
        Self::Product,
        Self::CountNumbers,
        Self::StdDev,
        Self::StdDevP,
        Self::Var,
        Self::VarP,
    ];

    /// Its i18n key.
    pub fn key(self) -> &'static str {
        match self {
            Self::Sum => "consolidate.fn.sum",
            Self::Count => "consolidate.fn.count",
            Self::Average => "consolidate.fn.average",
            Self::Max => "consolidate.fn.max",
            Self::Min => "consolidate.fn.min",
            Self::Product => "consolidate.fn.product",
            Self::CountNumbers => "consolidate.fn.count_numbers",
            Self::StdDev => "consolidate.fn.stddev",
            Self::StdDevP => "consolidate.fn.stddevp",
            Self::Var => "consolidate.fn.var",
            Self::VarP => "consolidate.fn.varp",
        }
    }

    /// The worksheet function a linked result is written with.
    pub fn formula_name(self) -> &'static str {
        match self {
            Self::Sum => "SUM",
            Self::Count => "COUNTA",
            Self::Average => "AVERAGE",
            Self::Max => "MAX",
            Self::Min => "MIN",
            Self::Product => "PRODUCT",
            Self::CountNumbers => "COUNT",
            Self::StdDev => "STDEV",
            Self::StdDevP => "STDEVP",
            Self::Var => "VAR",
            Self::VarP => "VARP",
        }
    }

    /// The result over the numbers of the cells put together, `filled` of
    /// them holding anything at all; an error's text where the worksheet
    /// function would give one.
    fn apply(self, numbers: &[f64], filled: usize) -> CellInput {
        let n = numbers.len() as f64;
        let mean = numbers.iter().sum::<f64>() / n;
        let devsq = numbers.iter().map(|x| (x - mean).powi(2)).sum::<f64>();
        let value = match self {
            Self::Sum => numbers.iter().sum(),
            Self::Count => filled as f64,
            Self::CountNumbers => n,
            Self::Product => {
                if numbers.is_empty() {
                    0.0
                } else {
                    numbers.iter().product()
                }
            }
            Self::Max => numbers.iter().copied().fold(f64::NAN, f64::max),
            Self::Min => numbers.iter().copied().fold(f64::NAN, f64::min),
            Self::Average => mean,
            Self::StdDev => (devsq / (n - 1.0)).sqrt(),
            Self::StdDevP => (devsq / n).sqrt(),
            Self::Var => devsq / (n - 1.0),
            Self::VarP => devsq / n,
        };
        match self {
            Self::Max | Self::Min if numbers.is_empty() => CellInput::Number(0.0),
            _ if value.is_finite() => CellInput::Number(value),
            _ => CellInput::Text("#DIV/0!".to_string()),
        }
    }
}

/// What Consolidate is asked to do.
#[derive(Debug, Clone, PartialEq)]
pub struct Consolidation {
    pub function: ConsolidateFunction,
    /// The ranges put together: each one's sheet, by index, and cells.
    pub sources: Vec<(usize, CellRange)>,
    /// Use labels in Top row.
    pub top_row: bool,
    /// Use labels in Left column.
    pub left_column: bool,
    /// Create links to source data.
    pub links: bool,
}

/// What a row (or column) of the result is matched by: where it sits in
/// its range, or the label it has, compared without case.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Key {
    At(u32),
    Label(String),
}

/// Where one source has each row key and each column key.
type Lookup<'a> = (HashMap<&'a Key, u32>, HashMap<&'a Key, u32>);

/// One source's cells, keyed.
struct Source {
    sheet: usize,
    /// Each data row's key and sheet row.
    rows: Vec<(Key, u32)>,
    /// Each data column's key and sheet column.
    columns: Vec<(Key, u32)>,
}

impl<B: Backend> Workbook<B> {
    /// The text a label cell is matched by: what it shows.
    fn label_on(&self, sheet: usize, at: CellRef) -> String {
        let names = self.sheet_names();
        names
            .get(sheet)
            .and_then(|name| self.engine().cell(name, &reference_text(at)).ok())
            .map(|cell| cell.display)
            .unwrap_or_default()
    }

    fn value_on(&self, sheet: usize, at: CellRef) -> CellValue {
        self.engine().workbook().sheets[sheet]
            .cell_at(at)
            .map(|cell| cell.value.clone())
            .unwrap_or(CellValue::Empty)
    }

    /// Data ▸ Consolidate into the active sheet at `at`, in one undo step;
    /// the range written. Refused, as Excel refuses them: no source, a
    /// source taking in the destination, links to sources on the
    /// destination's own sheet.
    pub fn consolidate(&mut self, at: CellRef, job: &Consolidation) -> Result<CellRange, String> {
        if job.sources.is_empty() {
            return Err("consolidate.alert.no_source".into());
        }
        let here = self.sheet_index();
        if job.links && job.sources.iter().any(|(sheet, _)| *sheet == here) {
            return Err("consolidate.alert.links_same_sheet".into());
        }
        let mut sources = Vec::new();
        for &(sheet, range) in &job.sources {
            let first_row = range.start.row + u32::from(job.top_row);
            let first_column = range.start.column + u32::from(job.left_column);
            let rows = (first_row..=range.end.row)
                .map(|row| {
                    let key = if job.left_column {
                        let text = self.label_on(sheet, CellRef::new(range.start.column, row));
                        Key::Label(text.to_lowercase())
                    } else {
                        Key::At(row - first_row)
                    };
                    (key, row)
                })
                .collect();
            let columns = (first_column..=range.end.column)
                .map(|column| {
                    let key = if job.top_row {
                        let text = self.label_on(sheet, CellRef::new(column, range.start.row));
                        Key::Label(text.to_lowercase())
                    } else {
                        Key::At(column - first_column)
                    };
                    (key, column)
                })
                .collect();
            sources.push(Source {
                sheet,
                rows,
                columns,
            });
        }
        // The result's rows and columns: every key, in the order first met;
        // with labels, the label as first written.
        let mut row_keys: Vec<(Key, String)> = Vec::new();
        let mut column_keys: Vec<(Key, String)> = Vec::new();
        for (index, source) in sources.iter().enumerate() {
            let range = job.sources[index].1;
            for (key, row) in &source.rows {
                if !row_keys.iter().any(|(k, _)| k == key) {
                    let label = if job.left_column {
                        self.label_on(source.sheet, CellRef::new(range.start.column, *row))
                    } else {
                        String::new()
                    };
                    row_keys.push((key.clone(), label));
                }
            }
            for (key, column) in &source.columns {
                if !column_keys.iter().any(|(k, _)| k == key) {
                    let label = if job.top_row {
                        self.label_on(source.sheet, CellRef::new(*column, range.start.row))
                    } else {
                        String::new()
                    };
                    column_keys.push((key.clone(), label));
                }
            }
        }
        // A label left blank is no label: Excel matches none to it.
        if job.left_column {
            row_keys.retain(|(key, _)| *key != Key::Label(String::new()));
        }
        if job.top_row {
            column_keys.retain(|(key, _)| *key != Key::Label(String::new()));
        }
        // Where each source has each key.
        let lookups: Vec<Lookup> = sources
            .iter()
            .map(|source| {
                (
                    source.rows.iter().rev().map(|(k, r)| (k, *r)).collect(),
                    source.columns.iter().rev().map(|(k, c)| (k, *c)).collect(),
                )
            })
            .collect();
        let destination = CellRange { start: at, end: at };
        let overlaps = |range: CellRange| {
            range.start.row <= destination.start.row
                && destination.start.row <= range.end.row
                && range.start.column <= destination.start.column
                && destination.start.column <= range.end.column
        };
        if job
            .sources
            .iter()
            .any(|(sheet, range)| *sheet == here && overlaps(*range))
        {
            return Err("consolidate.alert.overlap".into());
        }
        let label_columns = u32::from(job.left_column) + u32::from(job.links);
        let data_column = at.column + label_columns;
        let header = u32::from(job.top_row);
        let mut cells: Vec<(CellRef, CellInput)> = Vec::new();
        for (index, (_, label)) in column_keys.iter().enumerate() {
            if job.top_row {
                cells.push((
                    CellRef::new(data_column + index as u32, at.row),
                    CellInput::Text(label.clone()),
                ));
            }
        }
        let names = self.sheet_names();
        let mut groups = Vec::new();
        let mut row = at.row + header;
        for (row_key, label) in &row_keys {
            if job.links {
                // A detail row for each source with this row, then the
                // result over them.
                let top = row;
                for (index, source) in sources.iter().enumerate() {
                    let Some(&source_row) = lookups[index].0.get(row_key) else {
                        continue;
                    };
                    cells.push((
                        CellRef::new(at.column + u32::from(job.left_column), row),
                        CellInput::Text(names[source.sheet].clone()),
                    ));
                    for (offset, (column_key, _)) in column_keys.iter().enumerate() {
                        let Some(&source_column) = lookups[index].1.get(column_key) else {
                            continue;
                        };
                        let from = CellRef::new(source_column, source_row);
                        if matches!(self.value_on(source.sheet, from), CellValue::Empty) {
                            continue;
                        }
                        cells.push((
                            CellRef::new(data_column + offset as u32, row),
                            CellInput::Formula(format!(
                                "={}!{}",
                                quote_sheet_name(&names[source.sheet]),
                                crate::what_if::absolute(from)
                            )),
                        ));
                    }
                    row += 1;
                }
                if row == top {
                    continue;
                }
                if job.left_column {
                    cells.push((CellRef::new(at.column, row), CellInput::Text(label.clone())));
                }
                for offset in 0..column_keys.len() as u32 {
                    let column = column_label(data_column + offset);
                    cells.push((
                        CellRef::new(data_column + offset, row),
                        CellInput::Formula(format!(
                            "={}({column}{}:{column}{})",
                            job.function.formula_name(),
                            top + 1,
                            row
                        )),
                    ));
                }
                groups.push((top, row - 1));
                row += 1;
                continue;
            }
            if job.left_column {
                cells.push((CellRef::new(at.column, row), CellInput::Text(label.clone())));
            }
            for (offset, (column_key, _)) in column_keys.iter().enumerate() {
                let mut numbers = Vec::new();
                let mut filled = 0;
                for (index, source) in sources.iter().enumerate() {
                    let (Some(&r), Some(&c)) = (
                        lookups[index].0.get(row_key),
                        lookups[index].1.get(column_key),
                    ) else {
                        continue;
                    };
                    match self.value_on(source.sheet, CellRef::new(c, r)) {
                        CellValue::Empty => {}
                        CellValue::Number(number) => {
                            filled += 1;
                            numbers.push(number);
                        }
                        _ => filled += 1,
                    }
                }
                if filled == 0 {
                    continue;
                }
                cells.push((
                    CellRef::new(data_column + offset as u32, row),
                    job.function.apply(&numbers, filled),
                ));
            }
            row += 1;
        }
        let written = CellRange {
            start: at,
            end: CellRef::new(
                (data_column + column_keys.len() as u32).saturating_sub(1),
                row.saturating_sub(1).max(at.row),
            ),
        };
        let name = self.sheet_name();
        self.atomic_edit(|book| {
            let mut steps = 0;
            let values: Vec<(String, CellInput)> = cells
                .iter()
                .map(|(cell, value)| (reference_text(*cell), value.clone()))
                .collect();
            if !values.is_empty() {
                book.engine_mut()
                    .set_cells(&name, &values)
                    .map_err(|error| error.to_string())?;
                steps += 1;
            }
            // Each result's detail rows grouped, then collapsed under it.
            for &(top, bottom) in &groups {
                if book
                    .engine_mut()
                    .outline(&name, Axis::Rows, top, bottom, OutlineChange::Group)
                    .map_err(|error| error.to_string())?
                {
                    steps += 1;
                }
            }
            if !groups.is_empty()
                && book
                    .engine_mut()
                    .outline(&name, Axis::Rows, 0, 0, OutlineChange::ShowLevel(1))
                    .map_err(|error| error.to_string())?
            {
                steps += 1;
            }
            book.wrote(steps)?;
            Ok(written)
        })
    }
}

/// The Consolidate dialog: Function, Reference with Add and Delete, All
/// references, Use labels in Top row and Left column, Create links.
pub struct ConsolidateDialog {
    pub form: Form,
    at: CellRef,
}

impl ConsolidateDialog {
    pub fn open(selected: CellRange) -> Self {
        Self {
            form: Form::new("consolidate.dialog")
                .choice(
                    "function",
                    "consolidate.function",
                    ConsolidateFunction::ALL
                        .iter()
                        .map(|function| Label::Key(function.key()))
                        .collect(),
                    0,
                )
                .field("reference", "consolidate.reference", "")
                .list("all", "consolidate.all", Vec::new())
                .check("top", "consolidate.top_row", false)
                .check("left", "consolidate.left_column", false)
                .check("links", "consolidate.links", false)
                .button("ok", "validation.ok")
                .button("add", "consolidate.add")
                .button("delete", "consolidate.delete")
                .button("close", "scenario.close"),
            at: selected.start,
        }
    }

    fn references(&self) -> Vec<String> {
        self.form
            .items
            .iter()
            .find(|item| item.id == "all")
            .and_then(|item| match &item.control {
                crate::form::Control::List { items, .. } => Some(items.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }

    /// A typed reference as All references lists it: `Sheet2!$A$1:$C$4`.
    fn canonical<B: Backend>(book: &Workbook<B>, text: &str) -> Option<String> {
        let (sheet, range) = book.resolve_reference(text.trim())?;
        let names = book.sheet_names();
        Some(format!(
            "{}!{}",
            quote_sheet_name(names.get(sheet)?),
            absolute_range(range)
        ))
    }
}

impl ToolDialog for ConsolidateDialog {
    fn form(&self) -> &Form {
        &self.form
    }

    fn form_mut(&mut self) -> &mut Form {
        &mut self.form
    }

    fn changed<B: Backend>(
        &mut self,
        id: &'static str,
        _book: &mut Workbook<B>,
    ) -> Result<Outcome, String> {
        // A line picked in All references comes up in Reference, to change
        // or delete.
        if id == "all" {
            if let Some(line) = self
                .form
                .chosen("all")
                .and_then(|index| self.references().get(index).cloned())
            {
                self.form.set_text("reference", &line);
            }
        }
        Ok(Outcome::stay())
    }

    fn press<B: Backend>(
        &mut self,
        button: &str,
        book: &mut Workbook<B>,
    ) -> Result<Outcome, String> {
        self.form.alert = None;
        match button {
            "add" => {
                let text = self.form.text("reference").to_string();
                let Some(line) = Self::canonical(book, &text) else {
                    self.form.alert = Some("consolidate.alert.reference".into());
                    return Ok(Outcome::stay());
                };
                let mut lines = self.references();
                if !lines.contains(&line) {
                    lines.push(line);
                    lines.sort_by_key(|line| line.to_lowercase());
                }
                self.form.set_list("all", lines);
                Ok(Outcome::stay())
            }
            "delete" => {
                let mut lines = self.references();
                if let Some(index) = self.form.chosen("all") {
                    if index < lines.len() {
                        lines.remove(index);
                    }
                }
                self.form.set_list("all", lines);
                self.form.set_text("reference", "");
                Ok(Outcome::stay())
            }
            "ok" => {
                let mut lines = self.references();
                // A reference typed and not added is put in, as Excel does.
                let typed = self.form.text("reference").trim().to_string();
                if !typed.is_empty() {
                    match Self::canonical(book, &typed) {
                        Some(line) if !lines.contains(&line) => lines.push(line),
                        Some(_) => {}
                        None => {
                            self.form.alert = Some("consolidate.alert.reference".into());
                            return Ok(Outcome::stay());
                        }
                    }
                }
                let sources = lines
                    .iter()
                    .filter_map(|line| book.resolve_reference(line))
                    .collect();
                let job = Consolidation {
                    function: ConsolidateFunction::ALL[self.form.chosen("function").unwrap_or(0)],
                    sources,
                    top_row: self.form.checked("top"),
                    left_column: self.form.checked("left"),
                    links: self.form.checked("links"),
                };
                match book.consolidate(self.at, &job) {
                    Ok(_) => Ok(Outcome::closed_writing()),
                    Err(error) if error.starts_with("consolidate.alert.") => {
                        self.form.alert = Some(Label::Key(alert_key(&error)));
                        Ok(Outcome::stay())
                    }
                    Err(error) => Err(error),
                }
            }
            _ => Ok(Outcome::close()),
        }
    }
}

/// The alert's key, as a `&'static str`, for a refusal of
/// [`Workbook::consolidate`].
fn alert_key(error: &str) -> &'static str {
    match error {
        "consolidate.alert.no_source" => "consolidate.alert.no_source",
        "consolidate.alert.links_same_sheet" => "consolidate.alert.links_same_sheet",
        "consolidate.alert.overlap" => "consolidate.alert.overlap",
        _ => "consolidate.alert.reference",
    }
}

//! Data ▸ What-If Analysis ▸ Scenario Manager: named sets of values for a
//! sheet's changing cells — added, edited, deleted, shown, merged from
//! another sheet — and the reports comparing what each makes of the result
//! cells, saved in the sheet's `<scenarios>` as Excel saves them.

use jade_xlsx_headless_api::CellInput;
use jade_xlsx_reader::scenarios::{Scenario, Scenarios};
use jade_xlsx_reader::{CellRange, CellRef, CellValue};

use crate::form::{Form, Label};
use crate::report::{Output, Report};
use crate::sheet::{cell_input, reference_text, Workbook};
use crate::what_if::{absolute, absolute_range, Outcome, ToolDialog};
use crate::Backend;

/// Excel's most changing cells a scenario takes.
pub const MOST_CELLS: usize = 32;

impl<B: Backend> Workbook<B> {
    /// The active sheet's scenarios.
    pub fn scenarios(&self) -> Scenarios {
        self.scenarios_on(self.sheet_index())
    }

    pub(crate) fn scenarios_on(&self, index: usize) -> Scenarios {
        self.engine().workbook().sheets[index]
            .scenarios
            .clone()
            .unwrap_or_default()
    }

    /// The active sheet's scenarios replaced by `scenarios`: one undo step.
    pub fn set_scenarios(&mut self, scenarios: &Scenarios) -> Result<bool, String> {
        if self.scenarios() == *scenarios {
            return Ok(false);
        }
        let name = self.sheet_name();
        self.engine_mut()
            .set_scenarios(&name, Some(scenarios))
            .map_err(|error| error.to_string())?;
        self.wrote(1)?;
        Ok(true)
    }

    /// Show: the scenario at `index` puts its values in its changing cells,
    /// and is remembered as the one shown. One undo step.
    pub fn show_scenario(&mut self, index: usize) -> Result<bool, String> {
        let mut scenarios = self.scenarios();
        let Some(scenario) = scenarios.scenarios.get(index).cloned() else {
            return Ok(false);
        };
        let name = self.sheet_name();
        self.atomic_edit(|book| {
            let values: Vec<(String, CellInput)> = scenario
                .cells
                .iter()
                .map(|(at, value)| (reference_text(*at), cell_input(value)))
                .collect();
            book.engine_mut()
                .set_cells(&name, &values)
                .map_err(|error| error.to_string())?;
            scenarios.current = Some(index as u32);
            scenarios.show = Some(index as u32);
            book.engine_mut()
                .set_scenarios(&name, Some(&scenarios))
                .map_err(|error| error.to_string())?;
            book.wrote(2)?;
            Ok(true)
        })
    }

    /// Merge: the scenarios of the sheet at `from`, for the same cells of
    /// this one, added to its own; a name it has already gets a number.
    pub fn merge_scenarios(&mut self, from: usize) -> Result<bool, String> {
        if from == self.sheet_index() {
            return Ok(false);
        }
        let other = self.scenarios_on(from);
        if other.scenarios.is_empty() {
            return Ok(false);
        }
        let mut scenarios = self.scenarios();
        for mut scenario in other.scenarios {
            let taken = |name: &str, list: &Scenarios| {
                list.scenarios
                    .iter()
                    .any(|other| other.name.eq_ignore_ascii_case(name))
            };
            if taken(&scenario.name, &scenarios) {
                let base = scenario.name.clone();
                scenario.name = (2..)
                    .map(|n| format!("{base} {n}"))
                    .find(|name| !taken(name, &scenarios))
                    .unwrap_or(base);
            }
            scenarios.scenarios.push(scenario);
        }
        self.set_scenarios(&scenarios)
    }

    /// What each scenario makes of `results`: the changing cells' values and
    /// the result cells' values, scenario by scenario, worked out without
    /// changing the sheet.
    pub fn scenario_results(
        &self,
        scenarios: &[Scenario],
        results: &[CellRef],
    ) -> Result<Vec<Vec<CellValue>>, String> {
        let sheet = self.sheet_index();
        let mut probe = self.engine().what_if().map_err(|error| error.to_string())?;
        let workbook = self.engine().workbook();
        let date1904 = matches!(
            workbook.date_system,
            jade_xlsx_reader::DateSystem::Nineteen04
        );
        let outputs: Vec<(usize, CellRef)> = results.iter().map(|at| (sheet, *at)).collect();
        scenarios
            .iter()
            .map(|scenario| {
                let inputs: Vec<(usize, CellRef, CellValue)> = scenario
                    .cells
                    .iter()
                    .map(|(at, text)| (sheet, *at, typed_value(text, date1904)))
                    .collect();
                probe
                    .probe(workbook, &inputs, &outputs)
                    .map_err(|error| error.to_string())
            })
            .collect()
    }

    /// Summary ▸ Scenario summary: a sheet of its own comparing the
    /// changing cells and `results` for the current values and each
    /// scenario, as Excel lays it out. `words` are the report's headings in
    /// the user's language. Returns the new sheet.
    pub fn scenario_summary(
        &mut self,
        results: &[CellRef],
        words: &SummaryWords,
    ) -> Result<usize, String> {
        let scenarios = self.scenarios();
        let list: Vec<Scenario> = scenarios.scenarios.clone();
        let changing = changing_cells(&list);
        let found = self.scenario_results(&list, results)?;
        let mut report = Report::default();
        let at = CellRef::new;
        report.heading(at(1, 1), &words.title);
        report.heading(at(3, 2), &words.current);
        for (column, scenario) in list.iter().enumerate() {
            report.heading(at(4 + column as u32, 2), &scenario.name);
            if let Some(comment) = scenario.comment.as_deref().filter(|text| !text.is_empty()) {
                report.text(at(4 + column as u32, 3), comment);
            }
        }
        report.heading(at(1, 4), &words.changing);
        let mut row = 5;
        for cell in &changing {
            report.text(at(2, row), &self.cell_label(*cell));
            report.put(at(3, row), self.value_input(*cell));
            for (column, scenario) in list.iter().enumerate() {
                let column = 4 + column as u32;
                if let Some((_, value)) = scenario.cells.iter().find(|(c, _)| c == cell) {
                    report.put(at(column, row), cell_input(value));
                    report.shaded.push(at(column, row));
                }
            }
            row += 1;
        }
        if !results.is_empty() {
            report.heading(at(1, row), &words.results);
            row += 1;
            for (line, cell) in results.iter().enumerate() {
                report.text(at(2, row), &self.cell_label(*cell));
                report.put(at(3, row), self.value_input(*cell));
                for (column, values) in found.iter().enumerate() {
                    report.put(at(4 + column as u32, row), input_of(&values[line]));
                }
                row += 1;
            }
        }
        for (offset, line) in words.notes.iter().enumerate() {
            report.text(at(1, row + offset as u32), line);
        }
        // The result cells asked for are remembered for the next summary.
        let mut remembered = scenarios;
        remembered.result_cells = results
            .iter()
            .map(|cell| CellRange {
                start: *cell,
                end: *cell,
            })
            .collect();
        self.atomic_edit(|book| {
            let name = book.sheet_name();
            book.engine_mut()
                .set_scenarios(&name, Some(&remembered))
                .map_err(|error| error.to_string())?;
            book.wrote(1)?;
            book.write_report(&Output::NewSheet(words.sheet.clone()), &report)
        })
    }

    /// Summary ▸ Scenario PivotTable report: each scenario's results in a
    /// table of their own, on a sheet of its own, as Excel's report lays
    /// them out: the changing cells as its filter, a row per scenario.
    pub fn scenario_pivot(
        &mut self,
        results: &[CellRef],
        words: &SummaryWords,
    ) -> Result<usize, String> {
        let list = self.scenarios().scenarios;
        let changing = changing_cells(&list);
        let found = self.scenario_results(&list, results)?;
        let mut report = Report::default();
        let at = CellRef::new;
        let label = changing
            .iter()
            .map(|cell| self.cell_label(*cell))
            .collect::<Vec<_>>()
            .join(",");
        report.heading(at(0, 0), &format!("{label} {}", words.by));
        report.text(at(1, 0), &words.all);
        report.heading(at(0, 2), &words.row_labels);
        for (column, cell) in results.iter().enumerate() {
            report.heading(at(1 + column as u32, 2), &self.cell_label(*cell));
        }
        for (row, (scenario, values)) in list.iter().zip(&found).enumerate() {
            let row = 3 + row as u32;
            report.text(at(0, row), &scenario.name);
            for (column, value) in values.iter().enumerate() {
                report.put(at(1 + column as u32, row), input_of(value));
            }
        }
        self.write_report(&Output::NewSheet(words.pivot_sheet.clone()), &report)
    }

    /// A cell as a report names it: a name defined as just that cell, else
    /// `$B$3`.
    fn cell_label(&self, cell: CellRef) -> String {
        let sheet = self.sheet_name();
        let target = format!(
            "{}!{}",
            jade_xlsx_reader::quote_sheet_name(&sheet),
            absolute(cell)
        );
        self.defined_names()
            .iter()
            .find(|name| !name.hidden && name.formula.replace('\'', "") == target.replace('\'', ""))
            .map(|name| name.name.clone())
            .unwrap_or_else(|| absolute(cell))
    }

    /// What a cell holds now, to write into a report.
    fn value_input(&self, cell: CellRef) -> CellInput {
        let sheet = &self.engine().workbook().sheets[self.sheet_index()];
        match sheet.cell_at(cell).map(|found| &found.value) {
            Some(value) => input_of(value),
            None => CellInput::Blank,
        }
    }
}

/// The words a scenario report is written in.
#[derive(Debug, Clone, PartialEq)]
pub struct SummaryWords {
    pub sheet: String,
    pub pivot_sheet: String,
    pub title: String,
    pub current: String,
    pub changing: String,
    pub results: String,
    pub notes: Vec<String>,
    pub by: String,
    pub all: String,
    pub row_labels: String,
}

/// Every changing cell of `scenarios`, each once, in the order they come.
fn changing_cells(scenarios: &[Scenario]) -> Vec<CellRef> {
    let mut cells = Vec::new();
    for scenario in scenarios {
        for (at, _) in &scenario.cells {
            if !cells.contains(at) {
                cells.push(*at);
            }
        }
    }
    cells
}

fn input_of(value: &CellValue) -> CellInput {
    match value {
        CellValue::Number(number) => CellInput::Number(*number),
        CellValue::Text(text) => CellInput::Text(text.clone()),
        CellValue::Bool(flag) => CellInput::Bool(*flag),
        CellValue::Error(error) => CellInput::Error(error.clone()),
        CellValue::Empty => CellInput::Blank,
    }
}

/// A scenario's value as a cell would hold it typed.
fn typed_value(text: &str, date1904: bool) -> CellValue {
    match crate::typing::read(text, date1904).input {
        CellInput::Number(number) => CellValue::Number(number),
        CellInput::Bool(flag) => CellValue::Bool(flag),
        CellInput::Error(error) => CellValue::Error(error),
        CellInput::Blank => CellValue::Empty,
        CellInput::Text(text) => CellValue::Text(text),
        CellInput::Formula(_) => CellValue::Text(text.to_string()),
    }
}

/// The changing cells typed into the dialog, `B1:B3,C5`: every cell of
/// each range on the active sheet, at most [`MOST_CELLS`].
pub fn parse_cells<B: Backend>(book: &Workbook<B>, text: &str) -> Option<Vec<CellRef>> {
    let mut cells = Vec::new();
    for part in text
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        let (sheet, range) = book.resolve_reference(part)?;
        if sheet != book.sheet_index() {
            return None;
        }
        for row in range.start.row..=range.end.row {
            for column in range.start.column..=range.end.column {
                let cell = CellRef::new(column, row);
                if !cells.contains(&cell) {
                    cells.push(cell);
                }
                if cells.len() > MOST_CELLS {
                    return None;
                }
            }
        }
    }
    (!cells.is_empty()).then_some(cells)
}

/// The cells as the dialog shows them back: `$B$1:$B$3,$C$5` — runs down a
/// column joined into ranges.
pub fn cells_text(cells: &[CellRef]) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut at = 0;
    while at < cells.len() {
        let start = cells[at];
        let mut end = start;
        while at + 1 < cells.len()
            && cells[at + 1].column == start.column
            && cells[at + 1].row == end.row + 1
        {
            at += 1;
            end = cells[at];
        }
        parts.push(absolute_range(CellRange { start, end }));
        at += 1;
    }
    parts.join(",")
}

/// Which page of the Scenario Manager is showing.
enum Page {
    Manager,
    /// Add Scenario, or Edit Scenario for the one at its index.
    Scenario(Option<usize>),
    /// Scenario Values, for the scenario being added or edited.
    Values(Option<usize>, Scenario),
    Merge(Vec<usize>),
    Summary,
}

/// The Scenario Manager and the dialogs it opens.
pub struct ScenarioManager {
    pub form: Form,
    page: Page,
    /// The selection it was opened on: Add's changing cells start as it.
    selected: CellRange,
    /// The words a summary is written in, from the shell.
    pub words: Option<SummaryWords>,
}

impl ScenarioManager {
    pub fn open<B: Backend>(book: &Workbook<B>, selected: CellRange) -> Self {
        let mut manager = Self {
            form: Form::new("scenario.manager"),
            page: Page::Manager,
            selected,
            words: None,
        };
        manager.show_manager(book, None);
        manager
    }

    /// The scenarios the manager lists: on a protected sheet, the hidden
    /// ones are left out, as Excel leaves them.
    fn listed<B: Backend>(book: &Workbook<B>) -> Vec<(usize, Scenario)> {
        let protected = book.sheet_protection().ok().flatten().is_some();
        book.scenarios()
            .scenarios
            .into_iter()
            .enumerate()
            .filter(|(_, scenario)| !(protected && scenario.hidden))
            .collect()
    }

    fn show_manager<B: Backend>(&mut self, book: &Workbook<B>, chosen: Option<usize>) {
        let listed = Self::listed(book);
        let names: Vec<String> = listed.iter().map(|(_, s)| s.name.clone()).collect();
        let empty = names.is_empty();
        let mut form = Form::new("scenario.manager").list("scenarios", "scenario.list", names);
        if empty {
            form = form.note("empty", "scenario.none");
        }
        form = form
            .note("cells", Label::Text(String::new()))
            .note("comment", Label::Text(String::new()))
            .button("show", "scenario.show")
            .button("close", "scenario.close")
            .button("add", "scenario.add")
            .button("delete", "scenario.delete")
            .button("edit", "scenario.edit")
            .button("merge", "scenario.merge")
            .button("summary", "scenario.summary");
        if let Some(chosen) = chosen {
            form.choose("scenarios", chosen);
        }
        self.form = form;
        self.page = Page::Manager;
        self.describe(book);
    }

    /// The changing cells and comment of the scenario picked.
    fn describe<B: Backend>(&mut self, book: &Workbook<B>) {
        let listed = Self::listed(book);
        let picked = self
            .form
            .chosen("scenarios")
            .and_then(|at| listed.get(at))
            .map(|(_, scenario)| scenario.clone());
        let (cells, comment) = match picked {
            Some(scenario) => {
                let cells: Vec<CellRef> = scenario.cells.iter().map(|(at, _)| *at).collect();
                (
                    Label::Format(
                        "scenario.changing_cells",
                        vec![("cells", cells_text(&cells))],
                    ),
                    Label::Format(
                        "scenario.comment_is",
                        vec![("comment", scenario.comment.unwrap_or_default())],
                    ),
                )
            }
            None => (Label::Text(String::new()), Label::Text(String::new())),
        };
        self.form.set_note("cells", cells);
        self.form.set_note("comment", comment);
    }

    /// The scenario picked in the list, by its index among the sheet's.
    fn picked<B: Backend>(&self, book: &Workbook<B>) -> Option<(usize, Scenario)> {
        let listed = Self::listed(book);
        self.form
            .chosen("scenarios")
            .and_then(|at| listed.get(at).cloned())
    }

    fn scenario_page<B: Backend>(&mut self, book: &Workbook<B>, editing: Option<usize>) {
        let existing = editing.and_then(|at| book.scenarios().scenarios.get(at).cloned());
        let (title, name, cells, comment, locked, hidden) = match &existing {
            Some(scenario) => (
                "scenario.edit_dialog",
                scenario.name.clone(),
                cells_text(&scenario.cells.iter().map(|(at, _)| *at).collect::<Vec<_>>()),
                scenario.comment.clone().unwrap_or_default(),
                scenario.locked,
                scenario.hidden,
            ),
            None => (
                "scenario.add_dialog",
                String::new(),
                absolute_range(self.selected),
                String::new(),
                true,
                false,
            ),
        };
        self.form = Form::new(title)
            .field("name", "scenario.name", &name)
            .field("cells", "scenario.cells", &cells)
            .field("comment", "scenario.comment", &comment)
            .check("locked", "scenario.locked", locked)
            .check("hidden", "scenario.hidden", hidden)
            .button("ok", "validation.ok")
            .button("cancel", "validation.cancel");
        self.page = Page::Scenario(editing);
    }

    /// From the Add or Edit page to Scenario Values, or an alert.
    fn take_scenario<B: Backend>(&mut self, book: &Workbook<B>, editing: Option<usize>) {
        let form = &mut self.form;
        form.alert = None;
        let name = form.text("name").to_string();
        let scenarios = book.scenarios();
        if name.is_empty() {
            form.alert = Some("scenario.alert.name".into());
            return;
        }
        if scenarios
            .scenarios
            .iter()
            .enumerate()
            .any(|(at, other)| Some(at) != editing && other.name.eq_ignore_ascii_case(&name))
        {
            form.alert = Some("scenario.alert.taken".into());
            return;
        }
        let Some(cells) = parse_cells(book, form.text("cells")) else {
            form.alert = Some("scenario.alert.cells".into());
            return;
        };
        let before = editing.and_then(|at| scenarios.scenarios.get(at));
        let scenario = Scenario {
            name,
            locked: form.checked("locked"),
            hidden: form.checked("hidden"),
            user: before.and_then(|s| s.user.clone()),
            comment: Some(form.text("comment").to_string()).filter(|text| !text.is_empty()),
            cells: cells
                .iter()
                .map(|cell| {
                    // An edited scenario keeps its values for the cells it
                    // had; a new one starts from what the cells hold.
                    let kept = before
                        .and_then(|s| s.cells.iter().find(|(c, _)| c == cell))
                        .map(|(_, value)| value.clone());
                    (*cell, kept.unwrap_or_else(|| book.edit_text(*cell)))
                })
                .collect(),
        };
        let mut values = Form::new("scenario.values").note("prompt", "scenario.values_prompt");
        let ids = value_ids();
        for (line, (cell, value)) in scenario.cells.iter().enumerate() {
            values = values.field(
                ids[line],
                Label::Text(format!("{}:", book.cell_label(*cell))),
                value,
            );
        }
        self.form = values
            .button("ok", "validation.ok")
            .button("cancel", "validation.cancel")
            .button("add", "scenario.add");
        self.page = Page::Values(editing, scenario);
    }

    /// Scenario Values' OK (or Add): the scenario saved with what was typed.
    fn save_values<B: Backend>(
        &mut self,
        book: &mut Workbook<B>,
        editing: Option<usize>,
        mut scenario: Scenario,
    ) -> Result<Option<usize>, String> {
        let ids = value_ids();
        for (line, (_, value)) in scenario.cells.iter_mut().enumerate() {
            *value = self.form.text(ids[line]).to_string();
        }
        let mut scenarios = book.scenarios();
        let at = match editing {
            Some(at) if at < scenarios.scenarios.len() => {
                scenarios.scenarios[at] = scenario;
                at
            }
            _ => {
                scenarios.scenarios.push(scenario);
                scenarios.scenarios.len() - 1
            }
        };
        book.set_scenarios(&scenarios)?;
        Ok(Some(at))
    }

    fn summary_page<B: Backend>(&mut self, book: &Workbook<B>) {
        let remembered = book.scenarios().result_cells;
        let results = if remembered.is_empty() {
            absolute_range(self.selected)
        } else {
            remembered
                .iter()
                .map(|range| absolute_range(*range))
                .collect::<Vec<_>>()
                .join(",")
        };
        self.form = Form::new("scenario.summary_dialog")
            .choice(
                "type",
                "scenario.report_type",
                vec![
                    "scenario.report_summary".into(),
                    "scenario.report_pivot".into(),
                ],
                0,
            )
            .field("results", "scenario.result_cells", &results)
            .button("ok", "validation.ok")
            .button("cancel", "validation.cancel");
        self.page = Page::Summary;
    }

    fn merge_page<B: Backend>(&mut self, book: &Workbook<B>) {
        let names = book.sheet_names();
        let others: Vec<usize> = book
            .sheet_tabs()
            .into_iter()
            .filter(|&at| at != book.sheet_index())
            .collect();
        let lines: Vec<String> = others
            .iter()
            .map(|&at| {
                let count = book.scenarios_on(at).scenarios.len();
                format!("{} ({count})", names[at])
            })
            .collect();
        self.form = Form::new("scenario.merge_dialog")
            .list("sheet", "scenario.merge_from", lines)
            .button("ok", "validation.ok")
            .button("cancel", "validation.cancel");
        self.page = Page::Merge(others);
    }
}

/// The Scenario Values page's field ids, one per changing cell.
fn value_ids() -> [&'static str; MOST_CELLS] {
    [
        "v1", "v2", "v3", "v4", "v5", "v6", "v7", "v8", "v9", "v10", "v11", "v12", "v13", "v14",
        "v15", "v16", "v17", "v18", "v19", "v20", "v21", "v22", "v23", "v24", "v25", "v26", "v27",
        "v28", "v29", "v30", "v31", "v32",
    ]
}

impl ToolDialog for ScenarioManager {
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
        self.form.alert = None;
        let protected = book.sheet_protection().ok().flatten().is_some();
        match std::mem::replace(&mut self.page, Page::Manager) {
            Page::Manager => {
                self.page = Page::Manager;
                match button {
                    "close" => return Ok(Outcome::close()),
                    "add" => self.scenario_page(book, None),
                    "edit" | "delete" | "show" => {
                        let Some((at, scenario)) = self.picked(book) else {
                            return Ok(Outcome::stay());
                        };
                        if protected && scenario.locked && button != "show" {
                            self.form.alert = Some("scenario.alert.locked".into());
                            return Ok(Outcome::stay());
                        }
                        match button {
                            "edit" => self.scenario_page(book, Some(at)),
                            "delete" => {
                                let mut scenarios = book.scenarios();
                                scenarios.scenarios.remove(at);
                                if scenarios.scenarios.is_empty() {
                                    scenarios = Scenarios::default();
                                } else {
                                    let last = scenarios.scenarios.len() as u32 - 1;
                                    scenarios.current = scenarios.current.map(|c| c.min(last));
                                    scenarios.show = scenarios.show.map(|s| s.min(last));
                                }
                                book.set_scenarios(&scenarios)?;
                                let chosen = self.form.chosen("scenarios");
                                self.show_manager(book, chosen);
                                return Ok(Outcome::writing());
                            }
                            _ => {
                                book.show_scenario(at)?;
                                let chosen = self.form.chosen("scenarios");
                                self.show_manager(book, chosen);
                                return Ok(Outcome::writing());
                            }
                        }
                    }
                    "merge" => self.merge_page(book),
                    "summary" => {
                        if book.scenarios().scenarios.is_empty() {
                            return Ok(Outcome::stay());
                        }
                        self.summary_page(book)
                    }
                    _ => {}
                }
                Ok(Outcome::stay())
            }
            Page::Scenario(editing) => {
                self.page = Page::Scenario(editing);
                match button {
                    "ok" => self.take_scenario(book, editing),
                    _ => self.show_manager(book, None),
                }
                Ok(Outcome::stay())
            }
            Page::Values(editing, scenario) => match button {
                "ok" | "add" => {
                    let at = self.save_values(book, editing, scenario)?;
                    if button == "add" {
                        self.scenario_page(book, None);
                    } else {
                        let listed = Self::listed(book);
                        let chosen = at.and_then(|at| listed.iter().position(|(i, _)| *i == at));
                        self.show_manager(book, chosen);
                    }
                    Ok(Outcome::writing())
                }
                _ => {
                    self.show_manager(book, None);
                    Ok(Outcome::stay())
                }
            },
            Page::Merge(sheets) => {
                if button == "ok" {
                    if let Some(from) = self.form.chosen("sheet").and_then(|at| sheets.get(at)) {
                        book.merge_scenarios(*from)?;
                    }
                    self.show_manager(book, None);
                    return Ok(Outcome::writing());
                }
                self.show_manager(book, None);
                Ok(Outcome::stay())
            }
            Page::Summary => {
                if button != "ok" {
                    self.show_manager(book, None);
                    return Ok(Outcome::stay());
                }
                self.page = Page::Summary;
                let text = self.form.text("results").to_string();
                let results = if text.is_empty() {
                    Some(Vec::new())
                } else {
                    parse_cells(book, &text)
                };
                let Some(results) = results else {
                    self.form.alert = Some("scenario.alert.results".into());
                    return Ok(Outcome::stay());
                };
                let pivot = self.form.chosen("type") == Some(1);
                if pivot && results.is_empty() {
                    self.form.alert = Some("scenario.alert.results".into());
                    return Ok(Outcome::stay());
                }
                let words = self.words.clone().unwrap_or_else(SummaryWords::english);
                if pivot {
                    book.scenario_pivot(&results, &words)?;
                } else {
                    book.scenario_summary(&results, &words)?;
                }
                Ok(Outcome::closed_writing())
            }
        }
    }

    fn changed<B: Backend>(
        &mut self,
        id: &'static str,
        book: &mut Workbook<B>,
    ) -> Result<Outcome, String> {
        if id == "scenarios" {
            self.describe(book);
        }
        Ok(Outcome::stay())
    }

    fn set_words(&mut self, words: fn(&'static str) -> &'static str) {
        let word = |key| words(key).to_string();
        self.words = Some(SummaryWords {
            sheet: word("scenario.word.sheet"),
            pivot_sheet: word("scenario.word.pivot_sheet"),
            title: word("scenario.word.title"),
            current: word("scenario.word.current"),
            changing: word("scenario.word.changing"),
            results: word("scenario.word.results"),
            notes: vec![
                word("scenario.word.note1"),
                word("scenario.word.note2"),
                word("scenario.word.note3"),
            ],
            by: word("scenario.word.by"),
            all: word("scenario.word.all"),
            row_labels: word("scenario.word.row_labels"),
        });
    }
}

impl SummaryWords {
    /// Excel's English words, for a report written without the shell's.
    pub fn english() -> Self {
        Self {
            sheet: "Scenario Summary".into(),
            pivot_sheet: "Scenario PivotTable".into(),
            title: "Scenario Summary".into(),
            current: "Current Values:".into(),
            changing: "Changing Cells:".into(),
            results: "Result Cells:".into(),
            notes: vec![
                "Notes:  Current Values column represents values of changing cells at".into(),
                "time Scenario Summary Report was created.  Changing cells for each".into(),
                "scenario are highlighted in gray.".into(),
            ],
            by: "by".into(),
            all: "(All)".into(),
            row_labels: "Row Labels".into(),
        }
    }
}

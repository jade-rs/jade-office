//! Data ▸ Solver: an objective cell set to its maximum, minimum or a value
//! by changing variable cells, subject to constraints, by one of three
//! methods; the model kept in the sheet's hidden `solver_*` names as Excel
//! keeps it, the solution kept or the values put back, and an Answer
//! Report.

pub mod methods;

use jade_xlsx_headless_api::CellInput;
use jade_xlsx_reader::{quote_sheet_name, CellRange, CellRef, CellValue, DefinedName};

use crate::form::{Form, Label};
use crate::report::{Output, Report};
use crate::sheet::{reference_text, Checkpoint, Workbook};
use crate::what_if::{absolute, absolute_range, Outcome, ToolDialog};
use crate::Backend;
use methods::{Ending, Goal, Model, Options, Relation, Solution};

/// A constraint's relation as the Add Constraint dialog lists it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rel {
    AtMost,
    Equal,
    AtLeast,
    Int,
    Bin,
    Dif,
}

impl Rel {
    pub const ALL: [Rel; 6] = [
        Rel::AtMost,
        Rel::Equal,
        Rel::AtLeast,
        Rel::Int,
        Rel::Bin,
        Rel::Dif,
    ];

    /// As `solver_relN` saves it.
    pub fn code(self) -> u32 {
        Self::ALL.iter().position(|r| *r == self).unwrap_or(0) as u32 + 1
    }

    fn from_code(code: u32) -> Option<Self> {
        Self::ALL.get(code.checked_sub(1)? as usize).copied()
    }

    /// As the constraint list shows it.
    pub fn symbol(self) -> &'static str {
        match self {
            Rel::AtMost => "<=",
            Rel::Equal => "=",
            Rel::AtLeast => ">=",
            Rel::Int => "int",
            Rel::Bin => "bin",
            Rel::Dif => "dif",
        }
    }

    /// What `solver_rhsN` holds for the three that take no right side.
    fn word(self) -> Option<&'static str> {
        match self {
            Rel::Int => Some("integer"),
            Rel::Bin => Some("binary"),
            Rel::Dif => Some("AllDifferent"),
            _ => None,
        }
    }
}

/// One constraint, its sides as typed: `$B$1:$B$3`, `<=`, `10` or `$C$1`.
#[derive(Debug, Clone, PartialEq)]
pub struct Constraint {
    pub left: String,
    pub relation: Rel,
    pub right: String,
}

impl Constraint {
    pub fn line(&self) -> String {
        match self.relation.word() {
            Some(word) => format!("{} = {word}", self.left),
            None => format!("{} {} {}", self.left, self.relation.symbol(), self.right),
        }
    }
}

/// The solving method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    GrgNonlinear,
    SimplexLp,
    Evolutionary,
}

impl Method {
    pub const ALL: [Method; 3] = [
        Method::GrgNonlinear,
        Method::SimplexLp,
        Method::Evolutionary,
    ];

    /// As `solver_eng` saves it.
    fn code(self) -> u32 {
        match self {
            Method::GrgNonlinear => 1,
            Method::SimplexLp => 2,
            Method::Evolutionary => 3,
        }
    }

    fn from_code(code: u32) -> Self {
        match code {
            2 => Method::SimplexLp,
            3 => Method::Evolutionary,
            _ => Method::GrgNonlinear,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Method::GrgNonlinear => "solver.method.grg",
            Method::SimplexLp => "solver.method.simplex",
            Method::Evolutionary => "solver.method.evolutionary",
        }
    }
}

/// A sheet's Solver model, as the Solver Parameters dialog shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct SolverModel {
    /// Set Objective, as typed.
    pub objective: String,
    pub goal: Goal,
    /// By Changing Variable Cells, as typed: `$B$1:$B$3,$D$1`.
    pub variables: String,
    pub constraints: Vec<Constraint>,
    /// Make Unconstrained Variables Non-Negative.
    pub non_negative: bool,
    pub method: Method,
    pub options: Options,
    /// Ignore Integer Constraints.
    pub ignore_integer: bool,
}

impl Default for SolverModel {
    fn default() -> Self {
        Self {
            objective: String::new(),
            goal: Goal::Max,
            variables: String::new(),
            constraints: Vec::new(),
            non_negative: true,
            method: Method::GrgNonlinear,
            options: Options::default(),
            ignore_integer: false,
        }
    }
}

/// Excel's "no limit" for iterations and seconds.
const NO_LIMIT: f64 = 2_147_483_647.0;

/// The Solver Results dialog's words, and what the Answer Report needs.
#[derive(Debug, Clone)]
pub struct Solved {
    pub solution: Solution,
    /// The variable cells, in order.
    pub cells: Vec<CellRef>,
    /// Their values before the solve.
    pub original: Vec<f64>,
    pub objective_cell: CellRef,
    pub original_objective: f64,
    pub seconds: f64,
}

impl<B: Backend> Workbook<B> {
    /// The active sheet's Solver model, from its hidden `solver_*` names;
    /// the defaults when it has none.
    pub fn solver_model(&self) -> SolverModel {
        let sheet = self.sheet_index();
        let names: Vec<&DefinedName> = self
            .defined_names()
            .iter()
            .filter(|name| {
                name.local_sheet_index == Some(sheet) && name.name.starts_with("solver_")
            })
            .collect();
        let get = |key: &str| -> Option<String> {
            names
                .iter()
                .find(|name| name.name.eq_ignore_ascii_case(key))
                .map(|name| unquote(&name.formula))
        };
        let number = |key: &str| get(key).and_then(|text| text.parse::<f64>().ok());
        let mut model = SolverModel::default();
        let local = |text: String| strip_sheet(&text, &self.sheet_name());
        if let Some(objective) = get("solver_opt") {
            model.objective = local(objective);
        }
        if let Some(variables) = get("solver_adj") {
            model.variables = local(variables);
        }
        let target = number("solver_val").unwrap_or(0.0);
        model.goal = match number("solver_typ").map(|v| v as u32) {
            Some(2) => Goal::Min,
            Some(3) => Goal::Value(target),
            _ => Goal::Max,
        };
        let count = number("solver_num").unwrap_or(0.0) as usize;
        for at in 1..=count {
            let (Some(left), Some(code)) = (
                get(&format!("solver_lhs{at}")),
                number(&format!("solver_rel{at}")),
            ) else {
                continue;
            };
            let Some(relation) = Rel::from_code(code as u32) else {
                continue;
            };
            let right = match relation.word() {
                Some(word) => word.to_string(),
                None => local(get(&format!("solver_rhs{at}")).unwrap_or_default()),
            };
            model.constraints.push(Constraint {
                left: local(left),
                relation,
                right,
            });
        }
        if let Some(neg) = number("solver_neg") {
            model.non_negative = neg as u32 == 1;
        }
        if let Some(engine) = number("solver_eng") {
            model.method = Method::from_code(engine as u32);
        }
        let options = &mut model.options;
        if let Some(value) = number("solver_pre") {
            options.precision = value;
        }
        if let Some(value) = number("solver_itr") {
            options.iterations = value.min(u32::MAX as f64) as u32;
        }
        if let Some(value) = number("solver_tim") {
            options.seconds = value;
        }
        if let Some(value) = number("solver_tol") {
            options.integer_tolerance = value;
        }
        if let Some(value) = number("solver_cvg") {
            options.convergence = value;
        }
        if let Some(value) = number("solver_ssz") {
            options.population = value as usize;
        }
        if let Some(value) = number("solver_rsd") {
            options.seed = value as u64;
        }
        if let Some(value) = number("solver_rlx") {
            model.ignore_integer = value as u32 == 1;
        }
        model
    }

    /// The model saved in the active sheet's hidden `solver_*` names, as
    /// Excel saves it: one undo step.
    pub fn set_solver_model(&mut self, model: &SolverModel) -> Result<bool, String> {
        if self.solver_model() == *model && self.has_solver_model() {
            return Ok(false);
        }
        let sheet = self.sheet_index();
        let qualify = |text: &str| qualify(text, &self.sheet_name());
        let mut names: Vec<(String, String)> = Vec::new();
        let mut put = |key: &str, value: String| names.push((key.to_string(), value));
        put("solver_adj", qualify(&model.variables));
        put("solver_opt", qualify(&model.objective));
        let (typ, val) = match model.goal {
            Goal::Max => (1, 0.0),
            Goal::Min => (2, 0.0),
            Goal::Value(target) => (3, target),
        };
        put("solver_typ", typ.to_string());
        put("solver_val", number_text(val));
        put("solver_num", model.constraints.len().to_string());
        for (at, constraint) in model.constraints.iter().enumerate() {
            let at = at + 1;
            put(&format!("solver_lhs{at}"), qualify(&constraint.left));
            put(
                &format!("solver_rel{at}"),
                constraint.relation.code().to_string(),
            );
            let right = match constraint.relation.word() {
                Some(word) => format!("\"{word}\""),
                None => match constraint.right.trim().parse::<f64>() {
                    Ok(number) => number_text(number),
                    Err(_) => qualify(&constraint.right),
                },
            };
            put(&format!("solver_rhs{at}"), right);
        }
        let options = &model.options;
        put("solver_eng", model.method.code().to_string());
        put(
            "solver_neg",
            if model.non_negative { "1" } else { "2" }.into(),
        );
        put("solver_pre", number_text(options.precision));
        put("solver_itr", number_text(options.iterations as f64));
        put("solver_tim", number_text(options.seconds));
        put("solver_tol", number_text(options.integer_tolerance));
        put("solver_cvg", number_text(options.convergence));
        put("solver_ssz", options.population.to_string());
        put("solver_rsd", options.seed.to_string());
        put(
            "solver_rlx",
            if model.ignore_integer { "1" } else { "2" }.into(),
        );
        put("solver_scl", "1".into());
        put("solver_sho", "2".into());
        put("solver_drv", "1".into());
        put("solver_est", "1".into());
        put("solver_nwt", "1".into());
        put("solver_msl", "2".into());
        put("solver_mrt", "0.075".into());
        put("solver_ver", "3".into());
        // Constraints a shorter list no longer has are taken out.
        let stale: Vec<String> = self
            .defined_names()
            .iter()
            .filter(|name| name.local_sheet_index == Some(sheet))
            .filter(|name| {
                let lower = name.name.to_ascii_lowercase();
                ["solver_lhs", "solver_rel", "solver_rhs"]
                    .iter()
                    .any(|prefix| {
                        lower
                            .strip_prefix(prefix)
                            .and_then(|n| n.parse::<usize>().ok())
                            .is_some_and(|n| n > model.constraints.len())
                    })
            })
            .map(|name| name.name.clone())
            .collect();
        let definitions: Vec<DefinedName> = names
            .into_iter()
            .map(|(name, formula)| DefinedName {
                name,
                formula,
                local_sheet_index: Some(sheet),
                hidden: true,
            })
            .collect();
        let written = self
            .engine_mut()
            .set_defined_names(&definitions, &stale, Some(sheet))
            .map_err(|error| error.to_string())?;
        if written {
            self.wrote(1)?;
        }
        Ok(written)
    }

    fn has_solver_model(&self) -> bool {
        let sheet = self.sheet_index();
        self.defined_names()
            .iter()
            .any(|name| name.local_sheet_index == Some(sheet) && name.name == "solver_opt")
    }

    /// Solves `model`, then writes the variable cells' values found (when
    /// worth keeping) in one undo step.
    pub fn solve(&mut self, model: &SolverModel) -> Result<Solved, String> {
        let started = std::time::Instant::now();
        let sheet = self.sheet_index();
        let objective_cell =
            crate::what_if::single_cell(self, &model.objective).ok_or("solver.alert.objective")?;
        let cells = self
            .cells_of(&model.variables)
            .ok_or("solver.alert.variables")?;
        let formula = |book: &Self, at: CellRef| {
            book.engine().workbook().sheets[sheet]
                .cell_at(at)
                .is_some_and(|cell| cell.formula.is_some())
        };
        if cells.iter().any(|at| formula(self, *at)) {
            return Err("solver.alert.variable_formula".into());
        }
        // Each constraint, cell by cell.
        let mut rows: Vec<(CellRef, Relation, Side)> = Vec::new();
        let mut integer = vec![false; cells.len()];
        let mut bounds = vec![(f64::NEG_INFINITY, f64::INFINITY); cells.len()];
        let mut lower_given = vec![false; cells.len()];
        let mut different = Vec::new();
        for constraint in &model.constraints {
            let left = self
                .cells_of(&constraint.left)
                .ok_or("solver.alert.constraint")?;
            let ignore = model.ignore_integer;
            let variable = |at: &CellRef| cells.iter().position(|cell| cell == at);
            match constraint.relation {
                Rel::Int | Rel::Bin | Rel::Dif => {
                    let indices: Vec<usize> = left
                        .iter()
                        .map(|at| variable(at).ok_or("solver.alert.integer"))
                        .collect::<Result<_, _>>()?;
                    if ignore {
                        continue;
                    }
                    for &j in &indices {
                        integer[j] = true;
                        match constraint.relation {
                            Rel::Bin => {
                                bounds[j] = (bounds[j].0.max(0.0), bounds[j].1.min(1.0));
                                lower_given[j] = true;
                            }
                            Rel::Dif => {
                                let n = indices.len() as f64;
                                bounds[j] = (bounds[j].0.max(1.0), bounds[j].1.min(n));
                                lower_given[j] = true;
                            }
                            _ => {}
                        }
                    }
                    if constraint.relation == Rel::Dif {
                        different.push(indices);
                    }
                }
                Rel::AtMost | Rel::Equal | Rel::AtLeast => {
                    let relation = match constraint.relation {
                        Rel::AtMost => Relation::AtMost,
                        Rel::Equal => Relation::Equal,
                        _ => Relation::AtLeast,
                    };
                    let text = constraint.right.trim().trim_start_matches('=');
                    let rights: Vec<Side> = match text.parse::<f64>() {
                        Ok(number) => vec![Side::Number(number); left.len()],
                        Err(_) => {
                            let right = self.cells_of(text).ok_or("solver.alert.constraint")?;
                            if right.len() == 1 {
                                vec![Side::Cell(right[0]); left.len()]
                            } else if right.len() == left.len() {
                                right.into_iter().map(Side::Cell).collect()
                            } else {
                                return Err("solver.alert.sizes".into());
                            }
                        }
                    };
                    for (at, right) in left.iter().zip(rights) {
                        // A variable against a number is a bound of its own.
                        if let (Some(j), Side::Number(value)) = (variable(at), right) {
                            let (low, high) = bounds[j];
                            bounds[j] = match relation {
                                Relation::AtMost => (low, high.min(value)),
                                Relation::AtLeast => (low.max(value), high),
                                Relation::Equal => (low.max(value), high.min(value)),
                            };
                            if relation != Relation::AtMost {
                                lower_given[j] = true;
                            }
                            continue;
                        }
                        rows.push((*at, relation, right));
                    }
                }
            }
        }
        if model.non_negative {
            for (j, given) in lower_given.iter().enumerate() {
                if !given {
                    bounds[j].0 = bounds[j].0.max(0.0);
                }
            }
        }
        if bounds.iter().any(|(low, high)| low > high) {
            return Ok(Solved {
                solution: Solution {
                    ending: Ending::Infeasible,
                    values: Vec::new(),
                    objective: f64::NAN,
                    iterations: 0,
                },
                original: Vec::new(),
                cells,
                objective_cell,
                original_objective: f64::NAN,
                seconds: started.elapsed().as_secs_f64(),
            });
        }
        let current = |book: &Self, at: CellRef| -> f64 {
            match book.engine().workbook().sheets[sheet]
                .cell_at(at)
                .map(|cell| &cell.value)
            {
                Some(CellValue::Number(number)) => *number,
                _ => 0.0,
            }
        };
        let original: Vec<f64> = cells.iter().map(|at| current(self, *at)).collect();
        let original_objective = current(self, objective_cell);
        let mut outputs: Vec<(usize, CellRef)> = vec![(sheet, objective_cell)];
        for (at, _, right) in &rows {
            outputs.push((sheet, *at));
            if let Side::Cell(cell) = right {
                outputs.push((sheet, *cell));
            }
        }
        let mut probe = self.engine().what_if().map_err(|error| error.to_string())?;
        let workbook = self.engine().workbook();
        let relations: Vec<Relation> = rows.iter().map(|(_, relation, _)| *relation).collect();
        let mut evaluate = |x: &[f64]| -> Option<Vec<f64>> {
            let inputs: Vec<(usize, CellRef, CellValue)> = cells
                .iter()
                .zip(x)
                .map(|(at, value)| (sheet, *at, CellValue::Number(*value)))
                .collect();
            let values = probe.probe(workbook, &inputs, &outputs).ok()?;
            let number = |value: &CellValue| match value {
                CellValue::Number(number) => Some(*number),
                CellValue::Empty => Some(0.0),
                CellValue::Bool(flag) => Some(f64::from(u8::from(*flag))),
                _ => None,
            };
            let mut found = vec![number(&values[0])?];
            let mut at = 1;
            for (_, _, right) in &rows {
                let left = number(&values[at])?;
                at += 1;
                let right = match right {
                    Side::Number(value) => *value,
                    Side::Cell(_) => {
                        let value = number(&values[at])?;
                        at += 1;
                        value
                    }
                };
                found.push(left - right);
            }
            Some(found)
        };
        let start: Vec<f64> = original
            .iter()
            .zip(&bounds)
            .map(|(v, (low, high))| v.max(*low).min(*high))
            .collect();
        let mut problem = Model {
            start,
            goal: model.goal,
            relations,
            integer,
            bounds,
            different,
            evaluate: &mut evaluate,
        };
        let mut options = model.options;
        if options.seconds >= NO_LIMIT {
            options.seconds = 100.0;
        }
        if options.iterations as f64 >= NO_LIMIT {
            options.iterations = 10_000;
        }
        let solution = match model.method {
            Method::SimplexLp => methods::simplex_lp(&mut problem, &options),
            Method::GrgNonlinear => methods::grg_nonlinear(&mut problem, &options),
            Method::Evolutionary => methods::evolutionary(&mut problem, &options),
        };
        drop(probe);
        let solved = Solved {
            solution,
            cells,
            original,
            objective_cell,
            original_objective,
            seconds: started.elapsed().as_secs_f64(),
        };
        if solved.solution.ending.found() {
            self.put_values(&solved.cells, &solved.solution.values)?;
        }
        Ok(solved)
    }

    fn put_values(&mut self, cells: &[CellRef], values: &[f64]) -> Result<(), String> {
        let name = self.sheet_name();
        let inputs: Vec<(String, CellInput)> = cells
            .iter()
            .zip(values)
            .map(|(at, value)| (reference_text(*at), CellInput::Number(tidy(*value))))
            .collect();
        self.engine_mut()
            .set_cells(&name, &inputs)
            .map_err(|error| error.to_string())?;
        self.wrote(1)
    }

    /// Every cell of `text`'s ranges, on the active sheet.
    fn cells_of(&self, text: &str) -> Option<Vec<CellRef>> {
        let mut cells = Vec::new();
        for part in text
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
        {
            let (sheet, range) = self.resolve_reference(part)?;
            if sheet != self.sheet_index() {
                return None;
            }
            for row in range.start.row..=range.end.row {
                for column in range.start.column..=range.end.column {
                    cells.push(CellRef::new(column, row));
                }
            }
        }
        (!cells.is_empty()).then_some(cells)
    }

    /// The Answer Report, on a sheet of its own after the active one: the
    /// result, the objective cell's original and final values, each
    /// variable cell's, and each constraint's value, whether it binds and
    /// its slack.
    pub fn answer_report(
        &mut self,
        model: &SolverModel,
        solved: &Solved,
        words: &AnswerWords,
    ) -> Result<usize, String> {
        let at = CellRef::new;
        let mut report = Report::default();
        report.heading(at(0, 0), &words.title);
        for (row, line) in [&words.worksheet, &words.result, &words.engine, &words.time]
            .into_iter()
            .enumerate()
        {
            if !line.is_empty() {
                report.text(at(0, 1 + row as u32), line);
            }
        }
        let mut row = 6;
        let heading = |report: &mut Report, row: u32, title: &str, columns: &[&str]| {
            report.heading(at(0, row), title);
            for (offset, column) in columns.iter().enumerate() {
                report.heading(at(1 + offset as u32, row + 1), column);
            }
        };
        heading(
            &mut report,
            row,
            &words.objective_cell(model.goal),
            &[
                &words.cell,
                &words.name,
                &words.original,
                &words.final_value,
            ],
        );
        row += 2;
        report.text(at(1, row), &absolute(solved.objective_cell));
        report.text(at(2, row), &self.report_label(solved.objective_cell));
        report.number(at(3, row), solved.original_objective);
        report.number(at(4, row), solved.solution.objective);
        row += 2;
        heading(
            &mut report,
            row,
            &words.variable_cells,
            &[
                &words.cell,
                &words.name,
                &words.original,
                &words.final_value,
                &words.integer,
            ],
        );
        row += 2;
        let integer_cells: Vec<CellRef> = model
            .constraints
            .iter()
            .filter(|c| matches!(c.relation, Rel::Int | Rel::Bin | Rel::Dif))
            .filter_map(|c| self.cells_of(&c.left))
            .flatten()
            .collect();
        for (j, cell) in solved.cells.iter().enumerate() {
            report.text(at(1, row), &absolute(*cell));
            report.text(at(2, row), &self.report_label(*cell));
            report.number(at(3, row), solved.original[j]);
            report.number(
                at(4, row),
                solved.solution.values.get(j).copied().unwrap_or(f64::NAN),
            );
            let kind = if integer_cells.contains(cell) {
                &words.integer_value
            } else {
                &words.contin
            };
            report.text(at(5, row), kind);
            row += 1;
        }
        row += 1;
        heading(
            &mut report,
            row,
            &words.constraints,
            &[
                &words.cell,
                &words.name,
                &words.cell_value,
                &words.formula,
                &words.status,
                &words.slack,
            ],
        );
        row += 2;
        let sheet = self.sheet_index();
        for constraint in &model.constraints {
            if matches!(constraint.relation, Rel::Int | Rel::Bin | Rel::Dif) {
                continue;
            }
            let Some(left) = self.cells_of(&constraint.left) else {
                continue;
            };
            let rights: Vec<Option<(String, f64)>> = match constraint.right.trim().parse::<f64>() {
                Ok(number) => vec![Some((number_text(number), number)); left.len()],
                Err(_) => {
                    let right = self.cells_of(&constraint.right).unwrap_or_default();
                    left.iter()
                        .enumerate()
                        .map(|(i, _)| {
                            let cell = if right.len() == 1 {
                                right.first()
                            } else {
                                right.get(i)
                            };
                            cell.map(|cell| (absolute(*cell), self.number_at(sheet, *cell)))
                        })
                        .collect()
                }
            };
            for (cell, right) in left.iter().zip(rights) {
                let Some((right_text, right_value)) = right else {
                    continue;
                };
                let value = self.number_at(sheet, *cell);
                let slack = (value - right_value).abs();
                let binding =
                    slack <= model.options.precision.max(1e-9) * (1.0 + right_value.abs());
                report.text(at(1, row), &absolute(*cell));
                report.text(at(2, row), &self.report_label(*cell));
                report.number(at(3, row), value);
                report.text(
                    at(4, row),
                    &format!(
                        "{}{}{}",
                        absolute(*cell),
                        constraint.relation.symbol(),
                        right_text
                    ),
                );
                report.text(
                    at(5, row),
                    if binding {
                        &words.binding
                    } else {
                        &words.not_binding
                    },
                );
                report.number(at(6, row), if binding { 0.0 } else { slack });
                row += 1;
            }
        }
        self.write_report(&Output::NewSheet(words.sheet.clone()), &report)
    }

    fn number_at(&self, sheet: usize, at: CellRef) -> f64 {
        match self.engine().workbook().sheets[sheet]
            .cell_at(at)
            .map(|cell| &cell.value)
        {
            Some(CellValue::Number(number)) => *number,
            _ => 0.0,
        }
    }

    /// A cell named in a report as Excel names it: the text heading its
    /// row to the left, and the one heading its column above.
    fn report_label(&self, at: CellRef) -> String {
        let sheet = &self.engine().workbook().sheets[self.sheet_index()];
        let text = |cell: CellRef| match sheet.cell_at(cell).map(|c| &c.value) {
            Some(CellValue::Text(text)) => Some(text.clone()),
            _ => None,
        };
        let left = (0..at.column)
            .rev()
            .find_map(|column| text(CellRef::new(column, at.row)));
        let above = (0..at.row)
            .rev()
            .find_map(|row| text(CellRef::new(at.column, row)));
        match (left, above) {
            (Some(left), Some(above)) => format!("{left} {above}"),
            (Some(one), None) | (None, Some(one)) => one,
            (None, None) => String::new(),
        }
    }
}

/// A constraint's right side, cell by cell.
#[derive(Debug, Clone, Copy)]
enum Side {
    Number(f64),
    Cell(CellRef),
}

/// A value found, rid of float noise: 2.9999999999 is 3.
fn tidy(value: f64) -> f64 {
    let rounded = value.round();
    if (value - rounded).abs() < 1e-9 * (1.0 + value.abs()) {
        rounded
    } else {
        value
    }
}

fn number_text(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

/// A name's text without the quotes a string constant has.
fn unquote(formula: &str) -> String {
    let text = formula.trim().trim_start_matches('=');
    text.strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .unwrap_or(text)
        .to_string()
}

/// `Sheet1!$B$1,Sheet1!$C$1` as the dialog shows it on that sheet:
/// `$B$1,$C$1`.
fn strip_sheet(text: &str, sheet: &str) -> String {
    let quoted = format!("{}!", quote_sheet_name(sheet));
    let plain = format!("{sheet}!");
    text.split(',')
        .map(|part| {
            let part = part.trim();
            part.strip_prefix(&quoted)
                .or_else(|| part.strip_prefix(&plain))
                .unwrap_or(part)
                .to_string()
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// `$B$1,$C$1` as a name keeps it: `Sheet1!$B$1,Sheet1!$C$1`.
fn qualify(text: &str, sheet: &str) -> String {
    let prefix = quote_sheet_name(sheet);
    text.split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            if part.contains('!') {
                part.to_string()
            } else {
                format!("{prefix}!{}", dollar(part))
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// `B1:C3` as `$B$1:$C$3`; a name or an absolute reference as it is.
fn dollar(part: &str) -> String {
    match CellRange::parse(&part.replace('$', "")) {
        Ok(range) if part.chars().any(|c| c.is_ascii_digit()) => absolute_range(range),
        _ => part.to_string(),
    }
}

/// The Answer Report's words, in the user's language.
#[derive(Debug, Clone)]
pub struct AnswerWords {
    pub sheet: String,
    pub title: String,
    pub worksheet: String,
    pub result: String,
    pub engine: String,
    pub time: String,
    pub objective_max: String,
    pub objective_min: String,
    pub objective_value: String,
    pub variable_cells: String,
    pub constraints: String,
    pub cell: String,
    pub name: String,
    pub original: String,
    pub final_value: String,
    pub integer: String,
    pub integer_value: String,
    pub contin: String,
    pub cell_value: String,
    pub formula: String,
    pub status: String,
    pub slack: String,
    pub binding: String,
    pub not_binding: String,
}

impl AnswerWords {
    fn objective_cell(&self, goal: Goal) -> String {
        match goal {
            Goal::Max => self.objective_max.clone(),
            Goal::Min => self.objective_min.clone(),
            Goal::Value(_) => self.objective_value.clone(),
        }
    }

    /// Excel's English words.
    pub fn english() -> Self {
        Self {
            sheet: "Answer Report 1".into(),
            title: "Answer Report".into(),
            worksheet: String::new(),
            result: String::new(),
            engine: String::new(),
            time: String::new(),
            objective_max: "Objective Cell (Max)".into(),
            objective_min: "Objective Cell (Min)".into(),
            objective_value: "Objective Cell (Value Of)".into(),
            variable_cells: "Variable Cells".into(),
            constraints: "Constraints".into(),
            cell: "Cell".into(),
            name: "Name".into(),
            original: "Original Value".into(),
            final_value: "Final Value".into(),
            integer: "Integer".into(),
            integer_value: "Integer".into(),
            contin: "Contin".into(),
            cell_value: "Cell Value".into(),
            formula: "Formula".into(),
            status: "Status".into(),
            slack: "Slack".into(),
            binding: "Binding".into(),
            not_binding: "Not Binding".into(),
        }
    }
}

/// Which page of Solver shows.
enum Page {
    Parameters,
    /// Add Constraint, or Change Constraint for the one at its index.
    Constraint(Option<usize>),
    Options,
    Results(Box<Solved>),
}

/// The Solver Parameters dialog and the dialogs it opens.
pub struct SolverDialog {
    pub form: Form,
    page: Page,
    model: SolverModel,
    /// The book before the solve, for Restore Original Values and Cancel.
    before: Option<Checkpoint>,
    /// The words the Answer Report is written in, from the shell.
    pub words: Option<AnswerWords>,
    /// The Results page's Solver words, from the shell.
    pub text: Option<fn(&'static str) -> &'static str>,
}

impl SolverDialog {
    pub fn open<B: Backend>(book: &Workbook<B>, at: CellRef) -> Self {
        let mut model = book.solver_model();
        if model.objective.is_empty() {
            model.objective = absolute(at);
        }
        let mut dialog = Self {
            form: Form::new("solver.dialog"),
            page: Page::Parameters,
            model,
            before: None,
            words: None,
            text: None,
        };
        dialog.parameters(None);
        dialog
    }

    fn parameters(&mut self, chosen: Option<usize>) {
        let model = &self.model;
        let (goal, value) = match model.goal {
            Goal::Max => (0, String::from("0")),
            Goal::Min => (1, String::from("0")),
            Goal::Value(target) => (2, number_text(target)),
        };
        let lines: Vec<String> = model.constraints.iter().map(Constraint::line).collect();
        let method = Method::ALL
            .iter()
            .position(|m| *m == model.method)
            .unwrap_or(0);
        let mut form = Form::new("solver.dialog")
            .field("objective", "solver.objective", &model.objective)
            .choice(
                "goal",
                "solver.to",
                vec![
                    "solver.max".into(),
                    "solver.min".into(),
                    "solver.value_of".into(),
                ],
                goal,
            )
            .field("value", "solver.value", &value)
            .field("variables", "solver.variables", &model.variables)
            .list("constraints", "solver.constraints", lines)
            .check("non_negative", "solver.non_negative", model.non_negative)
            .choice(
                "method",
                "solver.method",
                Method::ALL.iter().map(|m| Label::Key(m.key())).collect(),
                method,
            )
            .button("solve", "solver.solve")
            .button("close", "solver.close")
            .button("add", "solver.add")
            .button("change", "solver.change")
            .button("delete", "solver.delete")
            .button("reset", "solver.reset")
            .button("options", "solver.options");
        if let Some(chosen) = chosen {
            form.choose("constraints", chosen);
        }
        self.form = form;
        self.page = Page::Parameters;
    }

    /// The Parameters page's fields read back into the model.
    fn take_parameters(&mut self) -> Result<(), &'static str> {
        let form = &self.form;
        self.model.objective = form.text("objective").trim().to_string();
        self.model.variables = form.text("variables").trim().to_string();
        self.model.non_negative = form.checked("non_negative");
        self.model.method = Method::ALL[form.chosen("method").unwrap_or(0).min(2)];
        self.model.goal = match form.chosen("goal") {
            Some(1) => Goal::Min,
            Some(2) => Goal::Value(
                form.text("value")
                    .trim()
                    .parse()
                    .map_err(|_| "solver.alert.value")?,
            ),
            _ => Goal::Max,
        };
        Ok(())
    }

    fn constraint_page(&mut self, editing: Option<usize>) {
        let existing = editing
            .and_then(|at| self.model.constraints.get(at))
            .cloned();
        let (left, relation, right) = match &existing {
            Some(c) => (c.left.clone(), c.relation, c.right.clone()),
            None => (String::new(), Rel::AtMost, String::new()),
        };
        let at = Rel::ALL.iter().position(|r| *r == relation).unwrap_or(0);
        let mut form = Form::new(if editing.is_some() {
            "solver.change_constraint"
        } else {
            "solver.add_constraint"
        })
        .field("left", "solver.cell_reference", &left)
        .choice(
            "relation",
            "solver.relation",
            Rel::ALL
                .iter()
                .map(|r| Label::Text(r.symbol().into()))
                .collect(),
            at,
        )
        .field(
            "right",
            "solver.constraint",
            if relation.word().is_some() {
                relation.word().unwrap_or_default()
            } else {
                &right
            },
        )
        .button("ok", "validation.ok");
        if editing.is_none() {
            form = form.button("add", "solver.add");
        }
        self.form = form.button("cancel", "validation.cancel");
        self.page = Page::Constraint(editing);
    }

    fn take_constraint<B: Backend>(&mut self, book: &Workbook<B>, editing: Option<usize>) -> bool {
        let form = &mut self.form;
        let relation = Rel::ALL[form.chosen("relation").unwrap_or(0).min(5)];
        let left = form.text("left").trim().to_string();
        let right = form.text("right").trim().to_string();
        if book.cells_of(&left).is_none() {
            form.alert = Some("solver.alert.cell_reference".into());
            return false;
        }
        let right = match relation.word() {
            Some(word) => word.to_string(),
            None => {
                if right.parse::<f64>().is_err() && book.cells_of(&right).is_none() {
                    form.alert = Some("solver.alert.constraint".into());
                    return false;
                }
                right
            }
        };
        let constraint = Constraint {
            left,
            relation,
            right,
        };
        match editing {
            Some(at) if at < self.model.constraints.len() => {
                self.model.constraints[at] = constraint
            }
            _ => self.model.constraints.push(constraint),
        }
        true
    }

    fn options_page(&mut self) {
        let options = &self.model.options;
        let limit = |value: f64| number_text(value);
        self.form = Form::new("solver.options_dialog")
            .field(
                "precision",
                "solver.precision",
                &format!("{}", options.precision),
            )
            .check(
                "ignore_integer",
                "solver.ignore_integer",
                self.model.ignore_integer,
            )
            .field(
                "integer_tolerance",
                "solver.integer_optimality",
                &format!("{}", options.integer_tolerance * 100.0),
            )
            .field("seconds", "solver.max_time", &limit(options.seconds))
            .field(
                "iterations",
                "solver.iterations",
                &limit(options.iterations as f64),
            )
            .field(
                "convergence",
                "solver.convergence",
                &format!("{}", options.convergence),
            )
            .field(
                "population",
                "solver.population",
                &options.population.to_string(),
            )
            .field("seed", "solver.seed", &options.seed.to_string())
            .button("ok", "validation.ok")
            .button("cancel", "validation.cancel");
        self.page = Page::Options;
    }

    fn take_options(&mut self) -> bool {
        let form = &mut self.form;
        let read = |id: &'static str| form.text(id).trim().parse::<f64>().ok();
        let (
            Some(precision),
            Some(tolerance),
            Some(seconds),
            Some(iterations),
            Some(convergence),
            Some(population),
            Some(seed),
        ) = (
            read("precision"),
            read("integer_tolerance"),
            read("seconds"),
            read("iterations"),
            read("convergence"),
            read("population"),
            read("seed"),
        )
        else {
            form.alert = Some("solver.alert.option".into());
            return false;
        };
        if precision <= 0.0
            || precision >= 1.0
            || !(0.0..=100.0).contains(&tolerance)
            || seconds < 1.0
            || iterations < 1.0
            || convergence <= 0.0
            || convergence >= 1.0
            || !(10.0..=200.0).contains(&population)
            || seed < 0.0
        {
            form.alert = Some("solver.alert.option".into());
            return false;
        }
        let ignore = form.checked("ignore_integer");
        let options = &mut self.model.options;
        options.precision = precision;
        options.integer_tolerance = tolerance / 100.0;
        options.seconds = seconds;
        options.iterations = iterations.min(u32::MAX as f64) as u32;
        options.convergence = convergence;
        options.population = population as usize;
        options.seed = seed as u64;
        self.model.ignore_integer = ignore;
        true
    }

    fn results_page(&mut self, solved: Solved) {
        let ending = solved.solution.ending;
        let mut form = Form::new("solver.results").note("ending", Label::Key(ending.key()));
        if ending.found() {
            form = form
                .choice(
                    "keep",
                    Label::Text(String::new()),
                    vec!["solver.keep".into(), "solver.restore".into()],
                    0,
                )
                .check("answer", "solver.answer_report", false);
        }
        self.form = form
            .button("ok", "validation.ok")
            .button("cancel", "validation.cancel");
        self.page = Page::Results(Box::new(solved));
    }
}

impl ToolDialog for SolverDialog {
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
        match std::mem::replace(&mut self.page, Page::Parameters) {
            Page::Parameters => {
                self.page = Page::Parameters;
                if let Err(alert) = self.take_parameters() {
                    self.form.alert = Some(alert.into());
                    return Ok(Outcome::stay());
                }
                let chosen = self.form.chosen("constraints");
                match button {
                    "close" => {
                        // Closing keeps the model, as Excel's Close does.
                        let wrote = !self.model.objective.is_empty()
                            && book.set_solver_model(&self.model)?;
                        return Ok(Outcome { close: true, wrote });
                    }
                    "add" => self.constraint_page(None),
                    "change" => {
                        if chosen.is_some() {
                            self.constraint_page(chosen)
                        }
                    }
                    "delete" => {
                        if let Some(at) = chosen.filter(|at| *at < self.model.constraints.len()) {
                            self.model.constraints.remove(at);
                            self.parameters(None);
                        }
                    }
                    "reset" => {
                        self.model = SolverModel::default();
                        self.parameters(None);
                    }
                    "options" => self.options_page(),
                    "solve" => {
                        if crate::what_if::single_cell(book, &self.model.objective).is_none() {
                            self.form.alert = Some("solver.alert.objective".into());
                            return Ok(Outcome::stay());
                        }
                        if book.cells_of(&self.model.variables).is_none() {
                            self.form.alert = Some("solver.alert.variables".into());
                            return Ok(Outcome::stay());
                        }
                        let before = book.checkpoint();
                        book.set_solver_model(&self.model)?;
                        let solved = match book.solve(&self.model) {
                            Ok(solved) => solved,
                            Err(alert) if alert.starts_with("solver.") => {
                                book.restore(before);
                                self.form.alert = Some(Label::Key(static_key(&alert)));
                                return Ok(Outcome::writing());
                            }
                            Err(error) => {
                                book.restore(before);
                                return Err(error);
                            }
                        };
                        self.before = Some(before);
                        self.results_page(solved);
                        return Ok(Outcome::writing());
                    }
                    _ => {}
                }
                Ok(Outcome::stay())
            }
            Page::Constraint(editing) => {
                self.page = Page::Constraint(editing);
                match button {
                    "ok" | "add" => {
                        if !self.take_constraint(book, editing) {
                            return Ok(Outcome::stay());
                        }
                        if button == "add" {
                            self.constraint_page(None);
                        } else {
                            self.parameters(editing);
                        }
                    }
                    _ => self.parameters(editing),
                }
                Ok(Outcome::stay())
            }
            Page::Options => {
                self.page = Page::Options;
                if button == "ok" && !self.take_options() {
                    return Ok(Outcome::stay());
                }
                self.parameters(None);
                Ok(Outcome::stay())
            }
            Page::Results(solved) => {
                let restore = button != "ok" || self.form.chosen("keep") == Some(1);
                let answer = button == "ok" && self.form.checked("answer");
                if restore || !solved.solution.ending.found() {
                    // Restore Original Values: the cells put back; the model
                    // stays saved.
                    if let Some(before) = self.before.take() {
                        book.restore(before);
                    }
                    book.set_solver_model(&self.model)?;
                } else {
                    self.before = None;
                }
                if answer {
                    let mut words = self.words.clone().unwrap_or_else(AnswerWords::english);
                    if let Some(text) = self.text {
                        let fill = |key, name, value: &str| text(key).replace(name, value);
                        words.worksheet =
                            fill("solver.word.worksheet", "{sheet}", &book.sheet_name());
                        words.result = fill(
                            "solver.word.result",
                            "{ending}",
                            text(solved.solution.ending.key()),
                        );
                        words.engine = fill(
                            "solver.word.engine",
                            "{engine}",
                            text(self.model.method.key()),
                        );
                        words.time = fill(
                            "solver.word.time",
                            "{seconds}",
                            &format!("{:.3}", solved.seconds),
                        );
                    }
                    book.answer_report(&self.model, &solved, &words)?;
                }
                Ok(Outcome::closed_writing())
            }
        }
    }

    fn set_words(&mut self, text: fn(&'static str) -> &'static str) {
        self.text = Some(text);
        let word = |key| text(key).to_string();
        self.words = Some(AnswerWords {
            sheet: word("solver.word.sheet"),
            title: word("solver.word.title"),
            worksheet: String::new(),
            result: String::new(),
            engine: String::new(),
            time: String::new(),
            objective_max: word("solver.word.objective_max"),
            objective_min: word("solver.word.objective_min"),
            objective_value: word("solver.word.objective_value"),
            variable_cells: word("solver.word.variable_cells"),
            constraints: word("solver.word.constraints"),
            cell: word("solver.word.cell"),
            name: word("solver.word.name"),
            original: word("solver.word.original"),
            final_value: word("solver.word.final"),
            integer: word("solver.word.integer"),
            integer_value: word("solver.word.integer"),
            contin: word("solver.word.contin"),
            cell_value: word("solver.word.cell_value"),
            formula: word("solver.word.formula"),
            status: word("solver.word.status"),
            slack: word("solver.word.slack"),
            binding: word("solver.word.binding"),
            not_binding: word("solver.word.not_binding"),
        });
    }

    fn changed<B: Backend>(
        &mut self,
        id: &'static str,
        _book: &mut Workbook<B>,
    ) -> Result<Outcome, String> {
        if id == "relation" {
            let relation = Rel::ALL[self.form.chosen("relation").unwrap_or(0).min(5)];
            if let Some(word) = relation.word() {
                self.form.set_text("right", word);
            } else if Rel::ALL
                .iter()
                .any(|r| r.word() == Some(self.form.text("right")))
            {
                self.form.set_text("right", "");
            }
        }
        Ok(Outcome::stay())
    }
}

/// An alert key the solve gave, as the form's `&'static str`.
fn static_key(alert: &str) -> &'static str {
    const KEYS: [&str; 7] = [
        "solver.alert.objective",
        "solver.alert.variables",
        "solver.alert.variable_formula",
        "solver.alert.constraint",
        "solver.alert.integer",
        "solver.alert.sizes",
        "solver.alert.value",
    ];
    KEYS.iter()
        .find(|key| **key == alert)
        .copied()
        .unwrap_or("solver.alert.constraint")
}

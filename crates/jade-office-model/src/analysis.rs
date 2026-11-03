//! Data ▸ Data Analysis: the Analysis ToolPak's tools, picked from a list,
//! each asking for its input range (grouped by columns or rows, labels in
//! the first row or column), its own choices, and where its table goes — a
//! range of the sheet or a sheet of its own.

pub mod stats;
pub mod tools;

use jade_xlsx_reader::{CellRange, CellRef, CellValue};

use crate::form::{Form, Label};
use crate::report::Output;
use crate::sheet::{column_label, Workbook};
use crate::what_if::{range_on_sheet, single_cell, Outcome, ToolDialog};
use crate::Backend;
use tools::{Out, Place, Series, Table};

/// The tools, in the Data Analysis list's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    AnovaSingle,
    AnovaReplication,
    AnovaTwoFactor,
    Correlation,
    Covariance,
    Descriptive,
    Exponential,
    FTest,
    Fourier,
    Histogram,
    MovingAverage,
    Random,
    Rank,
    Regression,
    Sampling,
    TPaired,
    TEqual,
    TUnequal,
    ZTest,
}

impl Tool {
    pub const ALL: [Tool; 19] = [
        Tool::AnovaSingle,
        Tool::AnovaReplication,
        Tool::AnovaTwoFactor,
        Tool::Correlation,
        Tool::Covariance,
        Tool::Descriptive,
        Tool::Exponential,
        Tool::FTest,
        Tool::Fourier,
        Tool::Histogram,
        Tool::MovingAverage,
        Tool::Random,
        Tool::Rank,
        Tool::Regression,
        Tool::Sampling,
        Tool::TPaired,
        Tool::TEqual,
        Tool::TUnequal,
        Tool::ZTest,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Tool::AnovaSingle => "tool.anova_single",
            Tool::AnovaReplication => "tool.anova_replication",
            Tool::AnovaTwoFactor => "tool.anova_two",
            Tool::Correlation => "tool.correlation",
            Tool::Covariance => "tool.covariance",
            Tool::Descriptive => "tool.descriptive",
            Tool::Exponential => "tool.exponential",
            Tool::FTest => "tool.f_test",
            Tool::Fourier => "tool.fourier",
            Tool::Histogram => "tool.histogram",
            Tool::MovingAverage => "tool.moving_average",
            Tool::Random => "tool.random",
            Tool::Rank => "tool.rank_percentile",
            Tool::Regression => "tool.regression",
            Tool::Sampling => "tool.sampling",
            Tool::TPaired => "tool.t_paired",
            Tool::TEqual => "tool.t_equal",
            Tool::TUnequal => "tool.t_unequal",
            Tool::ZTest => "tool.z_test",
        }
    }

    /// The tools reading two variable ranges.
    fn two_samples(self) -> bool {
        matches!(
            self,
            Tool::FTest | Tool::TPaired | Tool::TEqual | Tool::TUnequal | Tool::ZTest
        )
    }

    /// The tools whose input is grouped by columns or by rows.
    fn grouped(self) -> bool {
        matches!(
            self,
            Tool::AnovaSingle
                | Tool::Correlation
                | Tool::Covariance
                | Tool::Descriptive
                | Tool::Rank
        )
    }

    /// The two that write formulas into a range of the sheet, never a sheet
    /// of their own.
    fn range_only(self) -> bool {
        matches!(self, Tool::Exponential | Tool::MovingAverage)
    }
}

/// Random Number Generation's distributions, in its list's order.
const DISTRIBUTIONS: [&str; 7] = [
    "tool.uniform",
    "tool.normal",
    "tool.bernoulli",
    "tool.binomial",
    "tool.poisson",
    "tool.patterned",
    "tool.discrete",
];

enum Page {
    List,
    Tool(Tool),
}

/// The Data Analysis dialog, then the dialog of the tool picked.
pub struct AnalysisDialog {
    pub form: Form,
    page: Page,
    selected: CellRange,
    /// The words the tables are written in, from the shell; the keys as
    /// they are without one.
    pub words: Option<Box<dyn Fn(&'static str) -> String>>,
}

impl AnalysisDialog {
    pub fn open(selected: CellRange) -> Self {
        let mut dialog = Self {
            form: Form::new("tool.dialog"),
            page: Page::List,
            selected,
            words: None,
        };
        dialog.list();
        dialog
    }

    fn list(&mut self) {
        self.form = Form::new("tool.dialog")
            .list("tool", "tool.tools", Vec::new())
            .button("ok", "validation.ok")
            .button("cancel", "validation.cancel");
        // The lines are i18n keys; the shell's words name them.
        let lines = Tool::ALL.iter().map(|tool| self.word(tool.key())).collect();
        self.form.set_list("tool", lines);
        self.form.choose("tool", 0);
        self.page = Page::List;
    }

    fn word(&self, key: &'static str) -> String {
        match &self.words {
            Some(words) => words(key),
            None => key.to_string(),
        }
    }

    /// Names the list's lines again once the shell has given its words.
    pub fn set_word_source(&mut self, words: Box<dyn Fn(&'static str) -> String>) {
        self.words = Some(words);
        if matches!(self.page, Page::List) {
            let chosen = self.form.chosen("tool");
            let lines = Tool::ALL.iter().map(|tool| self.word(tool.key())).collect();
            self.form.set_list("tool", lines);
            if let Some(chosen) = chosen {
                self.form.choose("tool", chosen);
            }
        }
    }

    /// The tool's own dialog.
    pub fn tool_page(&mut self, tool: Tool) {
        let input = crate::what_if::absolute_range(self.selected);
        let mut form = Form::new(tool.key());
        if tool.two_samples() {
            form = form.field("variable1", "tool.variable1", &input).field(
                "variable2",
                "tool.variable2",
                "",
            );
        } else if tool == Tool::Regression {
            form =
                form.field("y_range", "tool.y_range", &input)
                    .field("x_range", "tool.x_range", "");
        } else if tool != Tool::Random {
            form = form.field("input", "tool.input", &input);
        }
        if tool.grouped() {
            form = form.choice(
                "grouped",
                "tool.grouped",
                vec!["tool.by_columns".into(), "tool.by_rows".into()],
                0,
            );
        }
        if tool != Tool::Random {
            form = form.check(
                "labels",
                if tool.grouped() {
                    "tool.labels_first"
                } else {
                    "tool.labels"
                },
                false,
            );
        }
        match tool {
            Tool::AnovaReplication => {
                form = form.field("rows_per_sample", "tool.rows_per_sample", "")
            }
            Tool::TPaired | Tool::TEqual | Tool::TUnequal | Tool::ZTest => {
                form = form.field("difference", "tool.difference", "");
                if tool == Tool::ZTest {
                    form = form.field("variance1", "tool.variance1", "").field(
                        "variance2",
                        "tool.variance2",
                        "",
                    );
                }
            }
            Tool::Exponential => form = form.field("damping", "tool.damping", ""),
            Tool::Histogram => form = form.field("bins", "tool.bins", ""),
            Tool::MovingAverage => form = form.field("interval", "tool.interval", ""),
            Tool::Fourier => form = form.check("inverse", "tool.inverse", false),
            Tool::Random => {
                form = form
                    .field("variables", "tool.number_of_variables", "")
                    .field("count", "tool.number_of_numbers", "")
                    .choice(
                        "distribution",
                        "tool.distribution",
                        DISTRIBUTIONS.iter().map(|key| Label::Key(key)).collect(),
                        0,
                    );
            }
            Tool::Sampling => {
                form = form
                    .choice(
                        "method",
                        "tool.sampling_method",
                        vec!["tool.periodic".into(), "tool.at_random".into()],
                        0,
                    )
                    .field("period", "tool.period", "")
                    .field("samples", "tool.number_of_samples", "");
            }
            _ => {}
        }
        if matches!(
            tool,
            Tool::AnovaSingle
                | Tool::AnovaReplication
                | Tool::AnovaTwoFactor
                | Tool::FTest
                | Tool::TPaired
                | Tool::TEqual
                | Tool::TUnequal
                | Tool::ZTest
        ) {
            form = form.field("alpha", "tool.alpha", "0.05");
        }
        if tool == Tool::Random {
            form = distribution_fields(form, 0).field("seed", "tool.seed", "");
        }
        form = if tool.range_only() {
            form.field("output_range", "tool.output_range", "")
        } else {
            form.choice(
                "output",
                "tool.output",
                vec!["tool.output_range".into(), "tool.new_sheet".into()],
                1,
            )
            .field("output_range", "tool.output_range", "")
            .field("sheet_name", "tool.sheet_name", "")
        };
        match tool {
            Tool::Descriptive => {
                form = form
                    .check("summary", "tool.summary_statistics", true)
                    .check("confidence", "tool.confidence_for_mean", false)
                    .field("level", "tool.level", "95")
                    .check("largest", "tool.kth_largest", false)
                    .field("largest_k", "tool.k", "1")
                    .check("smallest", "tool.kth_smallest", false)
                    .field("smallest_k", "tool.k", "1");
            }
            Tool::Exponential | Tool::MovingAverage => {
                form = form.check("errors", "tool.standard_errors", false)
            }
            Tool::Histogram => {
                form = form.check("pareto", "tool.pareto", false).check(
                    "cumulative",
                    "tool.cumulative_percentage",
                    false,
                )
            }
            Tool::Regression => {
                form = form
                    .check("constant_zero", "tool.constant_zero", false)
                    .check("confidence", "tool.confidence_level_check", false)
                    .field("level", "tool.level", "95")
                    .check("residuals", "tool.residuals", false)
                    .check("standardized", "tool.standard_residuals", false)
                    .check("probability", "tool.normal_probability", false)
            }
            _ => {}
        }
        self.form = form
            .button("ok", "validation.ok")
            .button("cancel", "validation.cancel");
        self.page = Page::Tool(tool);
    }

    fn alert(&mut self, key: &'static str) -> Result<Outcome, String> {
        self.form.alert = Some(Label::Key(key));
        Ok(Outcome::stay())
    }

    fn number(&self, id: &'static str, default: f64) -> Option<f64> {
        let text = self.form.text(id).trim();
        if text.is_empty() {
            return Some(default);
        }
        text.parse().ok()
    }

    /// Runs `tool` with what the form holds: its table written, or an
    /// alert.
    fn run<B: Backend>(&mut self, tool: Tool, book: &mut Workbook<B>) -> Result<Outcome, String> {
        let labels = self.form.checked("labels");
        let by_rows = self.form.chosen("grouped") == Some(1);
        let alpha = match self.number("alpha", 0.05) {
            Some(alpha) if alpha > 0.0 && alpha < 1.0 => alpha,
            _ => return self.alert("tool.alert.alpha"),
        };
        // Where the table goes.
        let new_sheet = !tool.range_only() && self.form.chosen("output") == Some(1);
        let output = if new_sheet {
            let wanted = self.form.text("sheet_name").trim().to_string();
            Output::NewSheet(if wanted.is_empty() {
                next_sheet_name(book)
            } else {
                wanted
            })
        } else {
            match single_cell(book, self.form.text("output_range"))
                .or_else(|| range_on_sheet(book, self.form.text("output_range")).map(|r| r.start))
            {
                Some(at) => Output::At(at),
                None => return self.alert("tool.alert.output"),
            }
        };
        let words_box = self.words.take();
        let fallback = |key: &'static str| key.to_string();
        let words: &dyn Fn(&'static str) -> String = match &words_box {
            Some(words) => words.as_ref(),
            None => &fallback,
        };
        let result = self
            .table(tool, book, labels, by_rows, alpha, &output, words)
            .map(|table| table.report);
        self.words = words_box;
        let report = match result {
            Ok(report) => report,
            Err(alert) => return self.alert(alert),
        };
        book.write_report(&output, &report)?;
        Ok(Outcome::closed_writing())
    }

    #[allow(clippy::too_many_arguments)]
    fn table<'w, B: Backend>(
        &self,
        tool: Tool,
        book: &Workbook<B>,
        labels: bool,
        by_rows: bool,
        alpha: f64,
        output: &Output,
        words: tools::Words<'w>,
    ) -> Result<Table<'w>, &'static str> {
        let mut table = Table::new(words);
        let form = &self.form;
        let read = |id: &'static str, by_rows: bool, label: &'static str| {
            let range = range_on_sheet(book, form.text(id)).ok_or("tool.alert.input")?;
            read_series(book, range, by_rows, labels, label, words)
        };
        let two = |table: &Table| -> Result<(Series, Series), &'static str> {
            let mut a = read("variable1", false, "tool.variable_1")?;
            let mut b = read("variable2", false, "tool.variable_2")?;
            if a.len() != 1 || b.len() != 1 {
                return Err("tool.alert.one_column");
            }
            let (mut a, mut b) = (a.remove(0), b.remove(0));
            if !labels {
                a.label = table.word("tool.variable_1");
                b.label = table.word("tool.variable_2");
            }
            if a.values.len() < 2 || b.values.len() < 2 {
                return Err("tool.alert.too_few");
            }
            Ok((a, b))
        };
        let difference = self.number("difference", 0.0).ok_or("tool.alert.number")?;
        match tool {
            Tool::AnovaSingle => {
                let groups = read(
                    "input",
                    by_rows,
                    if by_rows { "tool.row" } else { "tool.column" },
                )?;
                if groups.iter().any(|g| g.values.is_empty()) || groups.len() < 2 {
                    return Err("tool.alert.too_few");
                }
                tools::anova_single(&mut table, &groups, alpha);
            }
            Tool::AnovaTwoFactor => {
                let range = range_on_sheet(book, form.text("input")).ok_or("tool.alert.input")?;
                let (rows, columns, grid) = read_grid(book, range, labels, words)?;
                if grid.len() < 2 || columns.len() < 2 {
                    return Err("tool.alert.too_few");
                }
                tools::anova_two_factor(&mut table, &rows, &columns, &grid, alpha);
            }
            Tool::AnovaReplication => {
                let range = range_on_sheet(book, form.text("input")).ok_or("tool.alert.input")?;
                let per = form
                    .text("rows_per_sample")
                    .trim()
                    .parse::<usize>()
                    .ok()
                    .filter(|p| *p >= 2);
                let per = per.ok_or("tool.alert.rows_per_sample")?;
                // Labels are needed: column labels on top, sample labels down
                // the side.
                let (rows, columns, grid) = read_grid(book, range, true, words)?;
                if grid.is_empty() || grid.len() % per != 0 {
                    return Err("tool.alert.rows_per_sample");
                }
                let samples = grid
                    .chunks(per)
                    .enumerate()
                    .map(|(i, chunk)| (rows[i * per].clone(), chunk.to_vec()))
                    .collect::<Vec<_>>();
                tools::anova_replication(&mut table, &samples, &columns, alpha);
            }
            Tool::Correlation | Tool::Covariance => {
                let series = read(
                    "input",
                    by_rows,
                    if by_rows { "tool.row" } else { "tool.column" },
                )?;
                let length = series.first().map_or(0, |s| s.values.len());
                if series.iter().any(|s| s.values.len() != length) || length < 2 {
                    return Err("tool.alert.too_few");
                }
                tools::matrix(&mut table, &series, tool == Tool::Covariance);
            }
            Tool::Descriptive => {
                let series = read(
                    "input",
                    by_rows,
                    if by_rows { "tool.row" } else { "tool.column" },
                )?;
                let level = if form.checked("confidence") {
                    let level = self.number("level", 95.0).ok_or("tool.alert.number")?;
                    if !(level > 0.0 && level < 100.0) {
                        return Err("tool.alert.number");
                    }
                    Some(level / 100.0)
                } else {
                    None
                };
                let kth = |check: &'static str,
                           field: &'static str|
                 -> Result<Option<usize>, &'static str> {
                    if !form.checked(check) {
                        return Ok(None);
                    }
                    form.text(field)
                        .trim()
                        .parse::<usize>()
                        .ok()
                        .filter(|k| *k >= 1)
                        .map(Some)
                        .ok_or("tool.alert.number")
                };
                let choices = tools::Descriptive {
                    summary: form.checked("summary"),
                    confidence: level,
                    largest: kth("largest", "largest_k")?,
                    smallest: kth("smallest", "smallest_k")?,
                };
                if !choices.summary
                    && choices.confidence.is_none()
                    && choices.largest.is_none()
                    && choices.smallest.is_none()
                {
                    return Err("tool.alert.choose");
                }
                tools::descriptive(&mut table, &series, choices);
            }
            Tool::Exponential | Tool::MovingAverage => {
                let range = range_on_sheet(book, form.text("input")).ok_or("tool.alert.input")?;
                let vertical = range.start.column == range.end.column;
                if !vertical && range.start.row != range.end.row {
                    return Err("tool.alert.one_column");
                }
                let first = if labels { 1 } else { 0 };
                let count = if vertical {
                    range.end.row - range.start.row + 1
                } else {
                    range.end.column - range.start.column + 1
                }
                .saturating_sub(first);
                let Output::At(origin) = *output else {
                    return Err("tool.alert.output");
                };
                let sheet_prefix = String::new();
                let start = range.start;
                let input = move |i: u32| {
                    let at = if vertical {
                        CellRef::new(start.column, start.row + first + i)
                    } else {
                        CellRef::new(start.column + first + i, start.row)
                    };
                    format!("{sheet_prefix}{}{}", column_label(at.column), at.row + 1)
                };
                let output_cell =
                    move |i: u32| format!("{}{}", column_label(origin.column), origin.row + i + 1);
                let place = Place {
                    input: Box::new(input),
                    output: Box::new(output_cell),
                    vertical: true,
                };
                if tool == Tool::Exponential {
                    let damping = self.number("damping", 0.3).ok_or("tool.alert.damping")?;
                    if !(0.0..=1.0).contains(&damping) {
                        return Err("tool.alert.damping");
                    }
                    tools::exponential(
                        &mut table,
                        count as usize,
                        damping,
                        &place,
                        form.checked("errors"),
                    );
                } else {
                    let interval = self.number("interval", 3.0).ok_or("tool.alert.interval")?;
                    if interval < 1.0 || interval.fract() != 0.0 || interval as u32 > count {
                        return Err("tool.alert.interval");
                    }
                    tools::moving_average(
                        &mut table,
                        count as usize,
                        interval as u32,
                        &place,
                        form.checked("errors"),
                    );
                }
            }
            Tool::FTest => {
                let (a, b) = two(&table)?;
                tools::f_test(&mut table, &a, &b, alpha);
            }
            Tool::TPaired | Tool::TEqual | Tool::TUnequal => {
                let (a, b) = two(&table)?;
                if tool == Tool::TPaired && a.values.len() != b.values.len() {
                    return Err("tool.alert.same_count");
                }
                let kind = match tool {
                    Tool::TPaired => tools::TTest::Paired,
                    Tool::TEqual => tools::TTest::Equal,
                    _ => tools::TTest::Unequal,
                };
                tools::t_test(&mut table, kind, &a, &b, difference, alpha);
            }
            Tool::ZTest => {
                let (a, b) = two(&table)?;
                let known = |id| {
                    form.text(id)
                        .trim()
                        .parse::<f64>()
                        .ok()
                        .filter(|v| *v > 0.0)
                };
                let (Some(va), Some(vb)) = (known("variance1"), known("variance2")) else {
                    return Err("tool.alert.variance");
                };
                tools::z_test(&mut table, &a, &b, (va, vb), difference, alpha);
            }
            Tool::Fourier => {
                let range = range_on_sheet(book, form.text("input")).ok_or("tool.alert.input")?;
                let mut cells = cells_in(range);
                let label = if labels && !cells.is_empty() {
                    let first = cells.remove(0);
                    Some(text_at(book, first))
                } else {
                    None
                };
                let values: Option<Vec<(f64, f64)>> = cells
                    .iter()
                    .map(|at| match value_at(book, *at) {
                        CellValue::Number(v) => Some((v, 0.0)),
                        CellValue::Text(text) => tools::parse_complex(&text),
                        CellValue::Empty => Some((0.0, 0.0)),
                        _ => None,
                    })
                    .collect();
                let values = values.ok_or("tool.alert.complex")?;
                if !values.len().is_power_of_two() || values.len() > 4096 || values.len() < 2 {
                    return Err("tool.alert.power_of_two");
                }
                tools::fourier(
                    &mut table,
                    &values,
                    form.checked("inverse"),
                    label.as_deref(),
                );
            }
            Tool::Histogram => {
                let mut series = read("input", false, "tool.column")?;
                let values: Vec<f64> = series.drain(..).flat_map(|s| s.values).collect();
                if values.is_empty() {
                    return Err("tool.alert.input");
                }
                let bins = if form.text("bins").trim().is_empty() {
                    tools::automatic_bins(&values)
                } else {
                    read("bins", false, "tool.column")?
                        .into_iter()
                        .flat_map(|s| s.values)
                        .collect()
                };
                tools::histogram(
                    &mut table,
                    &values,
                    &bins,
                    tools::HistogramChoices {
                        pareto: form.checked("pareto"),
                        cumulative: form.checked("cumulative"),
                    },
                );
            }
            Tool::Random => {
                let columns = self
                    .number("variables", 1.0)
                    .filter(|v| *v >= 1.0 && v.fract() == 0.0);
                let rows = form
                    .text("count")
                    .trim()
                    .parse::<u32>()
                    .ok()
                    .filter(|v| *v >= 1);
                let (Some(columns), Some(rows)) = (columns, rows) else {
                    return Err("tool.alert.number");
                };
                let seed = match form.text("seed").trim() {
                    "" => None,
                    text => Some(text.parse::<u64>().map_err(|_| "tool.alert.number")?),
                };
                let distribution = self.distribution(book)?;
                let mut random = tools::Random::new(seed);
                tools::random_numbers(&mut table, columns as u32, rows, &distribution, &mut random);
            }
            Tool::Rank => {
                let series = read(
                    "input",
                    by_rows,
                    if by_rows { "tool.row" } else { "tool.column" },
                )?;
                tools::rank(&mut table, &series);
            }
            Tool::Regression => {
                let mut y = read("y_range", false, "tool.y")?;
                if y.len() != 1 {
                    return Err("tool.alert.one_column");
                }
                let mut y = y.remove(0);
                if !labels {
                    y.label = "Y".into();
                }
                let mut x = read("x_range", false, "tool.x_variable")?;
                if !labels {
                    let name = table.word("tool.x_variable");
                    let single = x.len() == 1;
                    for (i, series) in x.iter_mut().enumerate() {
                        series.label = if single {
                            name.clone()
                        } else {
                            format!("{name} {}", i + 1)
                        };
                    }
                }
                if x.iter().any(|s| s.values.len() != y.values.len()) {
                    return Err("tool.alert.same_count");
                }
                if y.values.len() <= x.len() + 1 {
                    return Err("tool.alert.too_few");
                }
                let confidence = if form.checked("confidence") {
                    let level = self.number("level", 95.0).ok_or("tool.alert.number")?;
                    if !(level > 0.0 && level < 100.0) {
                        return Err("tool.alert.number");
                    }
                    Some(level / 100.0)
                } else {
                    None
                };
                tools::regression(
                    &mut table,
                    &y,
                    &x,
                    tools::RegressionChoices {
                        constant_zero: form.checked("constant_zero"),
                        confidence,
                        residuals: form.checked("residuals"),
                        standardized: form.checked("standardized"),
                        probability: form.checked("probability"),
                    },
                )?;
            }
            Tool::Sampling => {
                let series = read("input", false, "tool.column")?;
                let values: Vec<f64> = series.into_iter().flat_map(|s| s.values).collect();
                let periodic = form.chosen("method") != Some(1);
                let number = |id| {
                    form.text(id)
                        .trim()
                        .parse::<usize>()
                        .ok()
                        .filter(|v| *v >= 1)
                };
                let mut random = tools::Random::new(None);
                if periodic {
                    let period = number("period").ok_or("tool.alert.number")?;
                    if period > values.len() {
                        return Err("tool.alert.number");
                    }
                    tools::sampling(&mut table, &values, Some(period), 0, &mut random);
                } else {
                    let count = number("samples").ok_or("tool.alert.number")?;
                    tools::sampling(&mut table, &values, None, count, &mut random);
                }
            }
        }
        // A label row that is just the series' default names, Excel keeps.
        let _ = Out::Blank;
        Ok(table)
    }

    /// Random Number Generation's distribution, from its parameter fields.
    fn distribution<B: Backend>(
        &self,
        book: &Workbook<B>,
    ) -> Result<tools::Distribution, &'static str> {
        use tools::Distribution as D;
        let form = &self.form;
        let get = |id: &'static str, default: f64| -> Result<f64, &'static str> {
            let text = form.text(id).trim();
            if text.is_empty() {
                return Ok(default);
            }
            text.parse().map_err(|_| "tool.alert.number")
        };
        Ok(match form.chosen("distribution").unwrap_or(0) {
            0 => {
                let (low, high) = (get("p1", 0.0)?, get("p2", 1.0)?);
                if high < low {
                    return Err("tool.alert.number");
                }
                D::Uniform(low, high)
            }
            1 => {
                let (mean, deviation) = (get("p1", 0.0)?, get("p2", 1.0)?);
                if deviation < 0.0 {
                    return Err("tool.alert.number");
                }
                D::Normal(mean, deviation)
            }
            2 => {
                let p = get("p1", 0.5)?;
                if !(0.0..=1.0).contains(&p) {
                    return Err("tool.alert.number");
                }
                D::Bernoulli(p)
            }
            3 => {
                let (p, trials) = (get("p1", 0.5)?, get("p2", 1.0)?);
                if !(0.0..=1.0).contains(&p) || trials < 1.0 {
                    return Err("tool.alert.number");
                }
                D::Binomial(p, trials as u32)
            }
            4 => {
                let lambda = get("p1", 1.0)?;
                if lambda <= 0.0 {
                    return Err("tool.alert.number");
                }
                D::Poisson(lambda)
            }
            5 => D::Patterned(
                get("p1", 1.0)?,
                get("p2", 1.0)?,
                get("p3", 1.0)?,
                get("p4", 1.0)? as u32,
                get("p5", 1.0)? as u32,
            ),
            _ => {
                let range = range_on_sheet(book, form.text("p1")).ok_or("tool.alert.input")?;
                if range.end.column != range.start.column + 1 {
                    return Err("tool.alert.discrete");
                }
                let mut table = Vec::new();
                for row in range.start.row..=range.end.row {
                    let number = |column| match value_at(book, CellRef::new(column, row)) {
                        CellValue::Number(v) => Some(v),
                        _ => None,
                    };
                    let (Some(value), Some(p)) =
                        (number(range.start.column), number(range.end.column))
                    else {
                        return Err("tool.alert.discrete");
                    };
                    table.push((value, p));
                }
                let total: f64 = table.iter().map(|(_, p)| p).sum();
                if (total - 1.0).abs() > 1e-9 {
                    return Err("tool.alert.discrete");
                }
                D::Discrete(table)
            }
        })
    }
}

/// The Random Number Generation form with the fields its distribution
/// takes, after the distribution choice.
fn distribution_fields(form: Form, distribution: usize) -> Form {
    let fields: &[(&'static str, &'static str)] = match distribution {
        0 => &[("p1", "tool.uniform_between"), ("p2", "tool.and")],
        1 => &[("p1", "tool.mean"), ("p2", "tool.standard_deviation")],
        2 => &[("p1", "tool.p_value_probability")],
        3 => &[("p1", "tool.p_value_probability"), ("p2", "tool.trials")],
        4 => &[("p1", "tool.lambda")],
        5 => &[
            ("p1", "tool.from"),
            ("p2", "tool.to"),
            ("p3", "tool.in_steps_of"),
            ("p4", "tool.repeat_each"),
            ("p5", "tool.repeat_sequence"),
        ],
        _ => &[("p1", "tool.value_probability_range")],
    };
    fields
        .iter()
        .fold(form, |form, (id, label)| form.field(id, *label, ""))
}

/// The first unused `SheetN`, as a new sheet ply is named.
fn next_sheet_name<B: Backend>(book: &Workbook<B>) -> String {
    let names = book.sheet_names();
    (1..)
        .map(|n| format!("Sheet{n}"))
        .find(|name| !names.iter().any(|taken| taken.eq_ignore_ascii_case(name)))
        .unwrap_or_default()
}

fn cells_in(range: CellRange) -> Vec<CellRef> {
    let mut cells = Vec::new();
    for row in range.start.row..=range.end.row {
        for column in range.start.column..=range.end.column {
            cells.push(CellRef::new(column, row));
        }
    }
    cells
}

fn value_at<B: Backend>(book: &Workbook<B>, at: CellRef) -> CellValue {
    book.engine().workbook().sheets[book.sheet_index()]
        .cell_at(at)
        .map(|cell| cell.value.clone())
        .unwrap_or(CellValue::Empty)
}

fn text_at<B: Backend>(book: &Workbook<B>, at: CellRef) -> String {
    match value_at(book, at) {
        CellValue::Text(text) => text,
        CellValue::Number(number) => format!("{number}"),
        CellValue::Bool(flag) => if flag { "TRUE" } else { "FALSE" }.into(),
        CellValue::Error(error) => error,
        CellValue::Empty => String::new(),
    }
}

/// The input's series, a column (or row) each: its label from the first
/// cell when `labels`, else `Column 1`, `Row 2`…; its numbers, blanks left
/// out. Text among the numbers is an alert, as Excel's.
pub fn read_series<B: Backend>(
    book: &Workbook<B>,
    range: CellRange,
    by_rows: bool,
    labels: bool,
    label_key: &'static str,
    words: tools::Words,
) -> Result<Vec<Series>, &'static str> {
    let lines: Vec<Vec<CellRef>> = if by_rows {
        (range.start.row..=range.end.row)
            .map(|row| {
                (range.start.column..=range.end.column)
                    .map(|column| CellRef::new(column, row))
                    .collect()
            })
            .collect()
    } else {
        (range.start.column..=range.end.column)
            .map(|column| {
                (range.start.row..=range.end.row)
                    .map(|row| CellRef::new(column, row))
                    .collect()
            })
            .collect()
    };
    let mut series = Vec::new();
    for (index, mut cells) in lines.into_iter().enumerate() {
        let label = if labels && !cells.is_empty() {
            text_at(book, cells.remove(0))
        } else {
            format!("{} {}", words(label_key), index + 1)
        };
        let mut values = Vec::new();
        for at in cells {
            match value_at(book, at) {
                CellValue::Number(number) => values.push(number),
                CellValue::Empty => {}
                _ => return Err("tool.alert.text"),
            }
        }
        series.push(Series { label, values });
    }
    Ok(series)
}

/// Row labels, column labels and the numbers between.
type Grid = (Vec<String>, Vec<String>, Vec<Vec<f64>>);

/// A grid of numbers with its row and column labels (the first column and
/// row when `labels`).
fn read_grid<B: Backend>(
    book: &Workbook<B>,
    range: CellRange,
    labels: bool,
    words: tools::Words,
) -> Result<Grid, &'static str> {
    let skip = u32::from(labels);
    let columns: Vec<String> = (range.start.column + skip..=range.end.column)
        .enumerate()
        .map(|(i, column)| {
            if labels {
                text_at(book, CellRef::new(column, range.start.row))
            } else {
                format!("{} {}", words("tool.column"), i + 1)
            }
        })
        .collect();
    let mut rows = Vec::new();
    let mut grid = Vec::new();
    for (i, row) in (range.start.row + skip..=range.end.row).enumerate() {
        rows.push(if labels {
            text_at(book, CellRef::new(range.start.column, row))
        } else {
            format!("{} {}", words("tool.row"), i + 1)
        });
        let mut line = Vec::new();
        for column in range.start.column + skip..=range.end.column {
            match value_at(book, CellRef::new(column, row)) {
                CellValue::Number(number) => line.push(number),
                _ => return Err("tool.alert.text"),
            }
        }
        grid.push(line);
    }
    Ok((rows, columns, grid))
}

impl ToolDialog for AnalysisDialog {
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
        match self.page {
            Page::List => match button {
                "ok" => {
                    let tool = Tool::ALL[self
                        .form
                        .chosen("tool")
                        .unwrap_or(0)
                        .min(Tool::ALL.len() - 1)];
                    self.tool_page(tool);
                    Ok(Outcome::stay())
                }
                _ => Ok(Outcome::close()),
            },
            Page::Tool(tool) => match button {
                "ok" => self.run(tool, book),
                _ => Ok(Outcome::close()),
            },
        }
    }

    fn set_words(&mut self, words: fn(&'static str) -> &'static str) {
        self.set_word_source(Box::new(move |key| words(key).to_string()));
    }

    fn changed<B: Backend>(
        &mut self,
        id: &'static str,
        _book: &mut Workbook<B>,
    ) -> Result<Outcome, String> {
        if id == "distribution" {
            // The parameter fields follow the distribution.
            let chosen = self.form.chosen("distribution").unwrap_or(0);
            let kept: Vec<(&'static str, String)> =
                ["variables", "count", "seed", "output_range", "sheet_name"]
                    .iter()
                    .map(|id| (*id, self.form.text(id).to_string()))
                    .collect();
            let output = self.form.chosen("output");
            self.tool_page(Tool::Random);
            let rebuilt = std::mem::replace(&mut self.form, Form::new(Tool::Random.key()));
            // Swap the default distribution fields for the chosen one's.
            let mut form = Form::new(rebuilt.title.clone());
            let mut fields_done = false;
            for item in rebuilt.items {
                if matches!(item.id, "p1" | "p2" | "p3" | "p4" | "p5") {
                    if !fields_done {
                        form = distribution_fields(form, chosen);
                        fields_done = true;
                    }
                    continue;
                }
                form.items.push(item);
            }
            form.buttons = rebuilt.buttons;
            form.focus = form
                .items
                .iter()
                .position(|item| matches!(item.control, crate::form::Control::Field { .. }));
            self.form = form;
            self.form.choose("distribution", chosen);
            if let Some(output) = output {
                self.form.choose("output", output);
            }
            for (id, text) in kept {
                self.form.set_text(id, &text);
            }
        }
        Ok(Outcome::stay())
    }
}

//! Each Analysis ToolPak tool's output table, laid out as Excel lays it
//! out, from the numbers read off the sheet. Words are i18n keys the shell
//! puts in the user's language.

use super::stats::*;
use crate::report::Report;
use jade_xlsx_headless_api::CellInput;
use jade_xlsx_reader::CellRef;

/// The words of a table, by key.
pub type Words<'a> = &'a dyn Fn(&'static str) -> String;

/// A column (or row) of the input: its label and its numbers.
#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    pub label: String,
    pub values: Vec<f64>,
}

/// A table being written from its top left corner, a row at a time.
pub struct Table<'a> {
    pub report: Report,
    pub row: u32,
    words: Words<'a>,
}

/// One cell of a row.
pub enum Out {
    Key(&'static str),
    Text(String),
    Number(f64),
    Formula(String),
    Blank,
}

impl<'a> Table<'a> {
    pub fn new(words: Words<'a>) -> Self {
        Self {
            report: Report::default(),
            row: 0,
            words,
        }
    }

    pub fn word(&self, key: &'static str) -> String {
        (self.words)(key)
    }

    pub fn line(&mut self, cells: Vec<Out>) {
        for (column, cell) in cells.into_iter().enumerate() {
            let at = CellRef::new(column as u32, self.row);
            match cell {
                Out::Key(key) => {
                    let text = self.word(key);
                    self.report.text(at, &text)
                }
                Out::Text(text) => self.report.text(at, &text),
                Out::Number(value) => self.report.number(at, value),
                Out::Formula(formula) => self.report.put(at, CellInput::Formula(formula)),
                Out::Blank => {}
            }
        }
        self.row += 1;
    }

    pub fn blank(&mut self) {
        self.row += 1;
    }

    /// A heading row: its words in italic in Excel; bold here.
    pub fn heading(&mut self, cells: Vec<Out>) {
        let row = self.row;
        let count = cells.len();
        self.line(cells);
        for column in 0..count as u32 {
            let at = CellRef::new(column, row);
            if self.report.cells.iter().any(|(cell, _)| *cell == at) {
                self.report.bold.push(at);
            }
        }
    }

    pub fn percent(&mut self, at: CellRef) {
        self.report.formats.push((at, "0.00%".into()));
    }
}

fn n(value: f64) -> Out {
    Out::Number(value)
}

fn k(key: &'static str) -> Out {
    Out::Key(key)
}

fn text(value: &str) -> Out {
    Out::Text(value.to_string())
}

/// The ANOVA table's rows: name, SS, df, and (but for Total) MS, F, P and F
/// crit against `error`.
struct Source {
    name: Out,
    ss: f64,
    df: f64,
}

fn anova_table(table: &mut Table, sources: Vec<Source>, error: Source, total: (f64, f64), alpha: f64) {
    table.line(vec![k("tool.anova")]);
    table.heading(vec![
        k("tool.source"),
        k("tool.ss"),
        k("tool.df"),
        k("tool.ms"),
        k("tool.f"),
        k("tool.p_value"),
        k("tool.f_crit"),
    ]);
    let ms_error = error.ss / error.df;
    for source in sources {
        let ms = source.ss / source.df;
        let f = ms / ms_error;
        table.line(vec![
            source.name,
            n(source.ss),
            n(source.df),
            n(ms),
            n(f),
            n(1.0 - f_cdf(f, source.df, error.df)),
            n(f_inverse(1.0 - alpha, source.df, error.df)),
        ]);
    }
    table.line(vec![error.name, n(error.ss), n(error.df), n(ms_error)]);
    table.blank();
    table.line(vec![k("tool.total"), n(total.0), n(total.1)]);
}

fn summary_line(name: Out, values: &[f64]) -> Vec<Out> {
    vec![
        name,
        n(values.len() as f64),
        n(sum(values)),
        n(mean(values)),
        n(variance(values)),
    ]
}

/// Anova: Single Factor.
pub fn anova_single(table: &mut Table, groups: &[Series], alpha: f64) {
    table.line(vec![k("tool.anova_single")]);
    table.blank();
    table.line(vec![k("tool.summary")]);
    table.heading(vec![k("tool.groups"), k("tool.count"), k("tool.sum"), k("tool.average"), k("tool.variance")]);
    for group in groups {
        table.line(summary_line(text(&group.label), &group.values));
    }
    table.blank();
    table.blank();
    let all: Vec<f64> = groups.iter().flat_map(|g| g.values.iter().copied()).collect();
    let grand = mean(&all);
    let between: f64 = groups
        .iter()
        .map(|g| g.values.len() as f64 * (mean(&g.values) - grand).powi(2))
        .sum();
    let within: f64 = groups.iter().map(|g| devsq(&g.values)).sum();
    let count = all.len() as f64;
    let k_groups = groups.len() as f64;
    anova_table(
        table,
        vec![Source {
            name: k("tool.between"),
            ss: between,
            df: k_groups - 1.0,
        }],
        Source {
            name: k("tool.within"),
            ss: within,
            df: count - k_groups,
        },
        (between + within, count - 1.0),
        alpha,
    );
}

/// Anova: Two-Factor Without Replication, `grid[row][column]`.
pub fn anova_two_factor(
    table: &mut Table,
    rows: &[String],
    columns: &[String],
    grid: &[Vec<f64>],
    alpha: f64,
) {
    table.line(vec![k("tool.anova_two")]);
    table.blank();
    table.heading(vec![k("tool.summary"), k("tool.count"), k("tool.sum"), k("tool.average"), k("tool.variance")]);
    for (label, values) in rows.iter().zip(grid) {
        table.line(summary_line(text(label), values));
    }
    table.blank();
    let by_column: Vec<Vec<f64>> = (0..columns.len())
        .map(|c| grid.iter().map(|row| row[c]).collect())
        .collect();
    for (label, values) in columns.iter().zip(&by_column) {
        table.line(summary_line(text(label), values));
    }
    table.blank();
    table.blank();
    let all: Vec<f64> = grid.iter().flatten().copied().collect();
    let grand = mean(&all);
    let r = grid.len() as f64;
    let c = columns.len() as f64;
    let ss_rows: f64 = grid.iter().map(|row| c * (mean(row) - grand).powi(2)).sum();
    let ss_columns: f64 = by_column.iter().map(|column| r * (mean(column) - grand).powi(2)).sum();
    let total = devsq(&all);
    let error = total - ss_rows - ss_columns;
    anova_table(
        table,
        vec![
            Source {
                name: k("tool.rows"),
                ss: ss_rows,
                df: r - 1.0,
            },
            Source {
                name: k("tool.columns"),
                ss: ss_columns,
                df: c - 1.0,
            },
        ],
        Source {
            name: k("tool.error"),
            ss: error,
            df: (r - 1.0) * (c - 1.0),
        },
        (total, r * c - 1.0),
        alpha,
    );
}

/// Anova: Two-Factor With Replication: `samples[s]` the rows of sample
/// `s`, each `per` rows of the columns.
pub fn anova_replication(
    table: &mut Table,
    samples: &[(String, Vec<Vec<f64>>)],
    columns: &[String],
    alpha: f64,
) {
    table.line(vec![k("tool.anova_replication")]);
    table.blank();
    let mut head = vec![k("tool.summary")];
    head.extend(columns.iter().map(|c| text(c)));
    head.push(k("tool.total"));
    table.heading(head);
    let cell_values = |rows: &Vec<Vec<f64>>, column: usize| -> Vec<f64> { rows.iter().map(|row| row[column]).collect() };
    let block = |table: &mut Table, groups: Vec<Vec<f64>>| {
        let stats: [(&'static str, fn(&[f64]) -> f64); 4] = [
            ("tool.count", |v| v.len() as f64),
            ("tool.sum", sum),
            ("tool.average", mean),
            ("tool.variance", variance),
        ];
        for (key, stat) in stats {
            let mut line = vec![k(key)];
            line.extend(groups.iter().map(|g| n(stat(g))));
            table.line(line);
        }
    };
    for (label, rows) in samples {
        table.blank();
        table.line(vec![text(label)]);
        let mut groups: Vec<Vec<f64>> = (0..columns.len()).map(|c| cell_values(rows, c)).collect();
        groups.push(rows.iter().flatten().copied().collect());
        block(table, groups);
    }
    table.blank();
    table.line(vec![k("tool.total")]);
    let groups: Vec<Vec<f64>> = (0..columns.len())
        .map(|c| samples.iter().flat_map(|(_, rows)| cell_values(rows, c)).collect())
        .collect();
    block(table, groups.clone());
    table.blank();
    table.blank();
    let all: Vec<f64> = samples.iter().flat_map(|(_, rows)| rows.iter().flatten().copied()).collect();
    let grand = mean(&all);
    let per = samples.first().map_or(0, |(_, rows)| rows.len()) as f64;
    let a = samples.len() as f64;
    let b = columns.len() as f64;
    let ss_sample: f64 = samples
        .iter()
        .map(|(_, rows)| {
            let values: Vec<f64> = rows.iter().flatten().copied().collect();
            b * per * (mean(&values) - grand).powi(2)
        })
        .sum();
    let ss_columns: f64 = groups.iter().map(|g| a * per * (mean(g) - grand).powi(2)).sum();
    let ss_within: f64 = samples
        .iter()
        .map(|(_, rows)| (0..columns.len()).map(|c| devsq(&cell_values(rows, c))).sum::<f64>())
        .sum();
    let total = devsq(&all);
    let interaction = total - ss_sample - ss_columns - ss_within;
    anova_table(
        table,
        vec![
            Source {
                name: k("tool.sample"),
                ss: ss_sample,
                df: a - 1.0,
            },
            Source {
                name: k("tool.columns"),
                ss: ss_columns,
                df: b - 1.0,
            },
            Source {
                name: k("tool.interaction"),
                ss: interaction,
                df: (a - 1.0) * (b - 1.0),
            },
        ],
        Source {
            name: k("tool.within"),
            ss: ss_within,
            df: a * b * (per - 1.0),
        },
        (total, a * b * per - 1.0),
        alpha,
    );
}

/// Correlation, or with `covariance` Covariance: the lower triangle of the
/// matrix.
pub fn matrix(table: &mut Table, series: &[Series], covariance_of: bool) {
    let mut head = vec![Out::Blank];
    head.extend(series.iter().map(|s| text(&s.label)));
    table.heading(head);
    for (i, row) in series.iter().enumerate() {
        let mut line = vec![text(&row.label)];
        for column in &series[..=i] {
            line.push(n(if covariance_of {
                covariance(&row.values, &column.values)
            } else if std::ptr::eq(row, column) {
                1.0
            } else {
                correlation(&row.values, &column.values)
            }));
        }
        table.line(line);
    }
}

/// Descriptive Statistics' choices.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Descriptive {
    pub summary: bool,
    pub confidence: Option<f64>,
    pub largest: Option<usize>,
    pub smallest: Option<usize>,
}

/// Descriptive Statistics: two columns for each series.
pub fn descriptive(table: &mut Table, series: &[Series], choices: Descriptive) {
    let start = table.row;
    let mut longest = start;
    for (index, one) in series.iter().enumerate() {
        table.row = start;
        let mut sub = Table::new(table.words);
        sub.heading(vec![text(&one.label)]);
        sub.blank();
        let v = &one.values;
        let count = v.len() as f64;
        if choices.summary {
            let lines: Vec<(&'static str, Out)> = vec![
                ("tool.mean", n(mean(v))),
                ("tool.standard_error", n(deviation(v) / count.sqrt())),
                ("tool.median", n(median(v))),
                ("tool.mode", mode(v).map_or(Out::Text("#N/A".into()), n)),
                ("tool.standard_deviation", n(deviation(v))),
                ("tool.sample_variance", n(variance(v))),
                ("tool.kurtosis", n(kurtosis(v))),
                ("tool.skewness", n(skewness(v))),
                ("tool.range", n(kth(v, 1, false) - kth(v, 1, true))),
                ("tool.minimum", n(kth(v, 1, true))),
                ("tool.maximum", n(kth(v, 1, false))),
                ("tool.sum", n(sum(v))),
                ("tool.count", n(count)),
            ];
            for (key, value) in lines {
                sub.line(vec![k(key), value]);
            }
        }
        if let Some(rank) = choices.largest {
            let label = format!("{}({rank})", sub.word("tool.largest"));
            sub.line(vec![Out::Text(label), n(kth(v, rank, false))]);
        }
        if let Some(rank) = choices.smallest {
            let label = format!("{}({rank})", sub.word("tool.smallest"));
            sub.line(vec![Out::Text(label), n(kth(v, rank, true))]);
        }
        if let Some(level) = choices.confidence {
            let label = format!("{}({:.1}%)", sub.word("tool.confidence_level"), level * 100.0);
            let half = t_inverse(1.0 - (1.0 - level) / 2.0, count - 1.0) * deviation(v) / count.sqrt();
            sub.line(vec![Out::Text(label), n(half)]);
        }
        let shift = 2 * index as u32;
        for (at, value) in sub.report.cells {
            table
                .report
                .cells
                .push((CellRef::new(at.column + shift, at.row + start), value));
        }
        for at in sub.report.bold {
            table.report.bold.push(CellRef::new(at.column + shift, at.row + start));
        }
        longest = longest.max(start + sub.row);
    }
    table.row = longest;
}

/// Where a formula-writing tool's input and output are, for its formulas'
/// references.
pub struct Place {
    /// The input's first number, as the formulas name it (with its sheet
    /// when the output is on another).
    pub input: Box<dyn Fn(u32) -> String>,
    /// The output column's cell `i`, as the formulas name it.
    pub output: Box<dyn Fn(u32) -> String>,
    pub vertical: bool,
}

/// Exponential Smoothing: a forecast formula for each value, the first
/// `#N/A`; with `errors` a standard error column.
pub fn exponential(table: &mut Table, count: usize, damping: f64, place: &Place, errors: bool) {
    let keep = 1.0 - damping;
    for i in 0..count as u32 {
        let forecast = match i {
            0 => Out::Text("#N/A".into()),
            1 => Out::Formula((place.input)(0)),
            _ => Out::Formula(format!(
                "{}*{}+{}*{}",
                number_literal(keep),
                (place.input)(i - 1),
                number_literal(damping),
                (place.output)(i - 1)
            )),
        };
        let error = if !errors {
            Out::Blank
        } else if i < 4 {
            Out::Text("#N/A".into())
        } else {
            Out::Formula(format!(
                "SQRT(SUMXMY2({}:{},{}:{})/3)",
                (place.input)(i - 3),
                (place.input)(i - 1),
                (place.output)(i - 3),
                (place.output)(i - 1)
            ))
        };
        table.line(vec![forecast, error]);
    }
}

/// Moving Average over `interval` values; with `errors` a standard error
/// column.
pub fn moving_average(table: &mut Table, count: usize, interval: u32, place: &Place, errors: bool) {
    for i in 0..count as u32 {
        let average = if i + 1 < interval {
            Out::Text("#N/A".into())
        } else {
            Out::Formula(format!(
                "AVERAGE({}:{})",
                (place.input)(i + 1 - interval),
                (place.input)(i)
            ))
        };
        let error = if !errors {
            Out::Blank
        } else if i + 2 < 2 * interval {
            Out::Text("#N/A".into())
        } else {
            Out::Formula(format!(
                "SQRT(SUMXMY2({}:{},{}:{})/{interval})",
                (place.input)(i + 1 - interval),
                (place.input)(i),
                (place.output)(i + 1 - interval),
                (place.output)(i)
            ))
        };
        table.line(vec![average, error]);
    }
}

fn number_literal(value: f64) -> String {
    let text = format!("{}", (value * 1e12).round() / 1e12);
    text
}

/// F-Test Two-Sample for Variances.
pub fn f_test(table: &mut Table, a: &Series, b: &Series, alpha: f64) {
    table.line(vec![k("tool.f_test")]);
    table.blank();
    table.heading(vec![Out::Blank, text(&a.label), text(&b.label)]);
    let (va, vb) = (variance(&a.values), variance(&b.values));
    let (da, db) = (a.values.len() as f64 - 1.0, b.values.len() as f64 - 1.0);
    table.line(vec![k("tool.mean"), n(mean(&a.values)), n(mean(&b.values))]);
    table.line(vec![k("tool.variance"), n(va), n(vb)]);
    table.line(vec![k("tool.observations"), n(da + 1.0), n(db + 1.0)]);
    table.line(vec![k("tool.df"), n(da), n(db)]);
    let f = va / vb;
    table.line(vec![k("tool.f"), n(f)]);
    let (p, critical) = if f < 1.0 {
        (f_cdf(f, da, db), f_inverse(alpha, da, db))
    } else {
        (1.0 - f_cdf(f, da, db), f_inverse(1.0 - alpha, da, db))
    };
    table.line(vec![k("tool.p_f_one"), n(p)]);
    table.line(vec![k("tool.f_critical_one"), n(critical)]);
}

/// t-Test: which of the three.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TTest {
    Paired,
    Equal,
    Unequal,
}

/// The t-Tests, and their one- and two-tail lines.
pub fn t_test(table: &mut Table, kind: TTest, a: &Series, b: &Series, difference: f64, alpha: f64) {
    table.line(vec![k(match kind {
        TTest::Paired => "tool.t_paired",
        TTest::Equal => "tool.t_equal",
        TTest::Unequal => "tool.t_unequal",
    })]);
    table.blank();
    table.heading(vec![Out::Blank, text(&a.label), text(&b.label)]);
    let (x, y) = (&a.values, &b.values);
    let (na, nb) = (x.len() as f64, y.len() as f64);
    let (va, vb) = (variance(x), variance(y));
    table.line(vec![k("tool.mean"), n(mean(x)), n(mean(y))]);
    table.line(vec![k("tool.variance"), n(va), n(vb)]);
    table.line(vec![k("tool.observations"), n(na), n(nb)]);
    let (t, df) = match kind {
        TTest::Paired => {
            table.line(vec![k("tool.pearson"), n(correlation(x, y))]);
            let d: Vec<f64> = x.iter().zip(y).map(|(p, q)| p - q).collect();
            let t = (mean(&d) - difference) / (variance(&d) / na).sqrt();
            (t, na - 1.0)
        }
        TTest::Equal => {
            let pooled = ((na - 1.0) * va + (nb - 1.0) * vb) / (na + nb - 2.0);
            table.line(vec![k("tool.pooled"), n(pooled)]);
            let t = (mean(x) - mean(y) - difference) / (pooled * (1.0 / na + 1.0 / nb)).sqrt();
            (t, na + nb - 2.0)
        }
        TTest::Unequal => {
            let (sa, sb) = (va / na, vb / nb);
            let df = (sa + sb).powi(2) / (sa * sa / (na - 1.0) + sb * sb / (nb - 1.0));
            let t = (mean(x) - mean(y) - difference) / (sa + sb).sqrt();
            (t, df.round())
        }
    };
    table.line(vec![k("tool.hypothesized"), n(difference)]);
    table.line(vec![k("tool.df"), n(df)]);
    table.line(vec![k("tool.t_stat"), n(t)]);
    let one = 1.0 - t_cdf(t.abs(), df);
    table.line(vec![k("tool.p_t_one"), n(one)]);
    table.line(vec![k("tool.t_critical_one"), n(t_inverse(1.0 - alpha, df))]);
    table.line(vec![k("tool.p_t_two"), n(2.0 * one)]);
    table.line(vec![k("tool.t_critical_two"), n(t_inverse(1.0 - alpha / 2.0, df))]);
}

/// z-Test: Two Sample for Means, the variances known.
#[allow(clippy::too_many_arguments)]
pub fn z_test(
    table: &mut Table,
    a: &Series,
    b: &Series,
    known: (f64, f64),
    difference: f64,
    alpha: f64,
) {
    table.line(vec![k("tool.z_test")]);
    table.blank();
    table.heading(vec![Out::Blank, text(&a.label), text(&b.label)]);
    let (na, nb) = (a.values.len() as f64, b.values.len() as f64);
    table.line(vec![k("tool.mean"), n(mean(&a.values)), n(mean(&b.values))]);
    table.line(vec![k("tool.known_variance"), n(known.0), n(known.1)]);
    table.line(vec![k("tool.observations"), n(na), n(nb)]);
    table.line(vec![k("tool.hypothesized"), n(difference)]);
    let z = (mean(&a.values) - mean(&b.values) - difference) / (known.0 / na + known.1 / nb).sqrt();
    table.line(vec![k("tool.z"), n(z)]);
    let one = 1.0 - normal_cdf(z.abs());
    table.line(vec![k("tool.p_z_one"), n(one)]);
    table.line(vec![k("tool.z_critical_one"), n(normal_inverse(1.0 - alpha))]);
    table.line(vec![k("tool.p_z_two"), n(2.0 * one)]);
    table.line(vec![k("tool.z_critical_two"), n(normal_inverse(1.0 - alpha / 2.0))]);
}

/// A complex number as Excel's engineering functions write it: `3+4i`.
pub fn complex_text(re: f64, im: f64) -> String {
    let tidy = |v: f64| {
        let v = if v.abs() < 1e-12 { 0.0 } else { v };
        let text = format!("{}", (v * 1e14).round() / 1e14);
        text
    };
    match (re.abs() < 1e-12, im.abs() < 1e-12) {
        (_, true) => tidy(re),
        (true, false) => format!("{}i", unit(&tidy(im))),
        (false, false) => {
            let sign = if im < 0.0 { "" } else { "+" };
            format!("{}{sign}{}i", tidy(re), unit(&tidy(im)))
        }
    }
}

fn unit(text: &str) -> String {
    match text {
        "1" => String::new(),
        "-1" => "-".into(),
        other => other.to_string(),
    }
}

/// `3+4i`, `-2.5i`, `7` read back.
pub fn parse_complex(text: &str) -> Option<(f64, f64)> {
    let text = text.trim().replace(' ', "");
    if let Ok(value) = text.parse::<f64>() {
        return Some((value, 0.0));
    }
    let body = text.strip_suffix('i').or_else(|| text.strip_suffix('j'))?;
    let split = body
        .char_indices()
        .skip(1)
        .filter(|(i, c)| (*c == '+' || *c == '-') && !body[..*i].ends_with(['e', 'E']))
        .map(|(i, _)| i)
        .last();
    let imaginary = |part: &str| match part {
        "" | "+" => Some(1.0),
        "-" => Some(-1.0),
        other => other.parse::<f64>().ok(),
    };
    match split {
        Some(at) => Some((body[..at].parse().ok()?, imaginary(&body[at..])?)),
        None => Some((0.0, imaginary(body)?)),
    }
}

/// Fourier Analysis: the discrete Fourier transform (or with `inverse` its
/// inverse) of 2ⁿ values, by the radix-2 FFT.
pub fn fourier(table: &mut Table, values: &[(f64, f64)], inverse: bool, label: Option<&str>) {
    let mut data = values.to_vec();
    let count = data.len();
    // Bit-reversed order.
    let bits = count.trailing_zeros();
    for i in 0..count {
        let j = i.reverse_bits() >> (usize::BITS - bits);
        if j > i {
            data.swap(i, j);
        }
    }
    let sign = if inverse { 1.0 } else { -1.0 };
    let mut size = 2;
    while size <= count {
        let angle = sign * 2.0 * std::f64::consts::PI / size as f64;
        for start in (0..count).step_by(size) {
            for k in 0..size / 2 {
                let (wr, wi) = ((angle * k as f64).cos(), (angle * k as f64).sin());
                let (ar, ai) = data[start + k];
                let (br, bi) = data[start + k + size / 2];
                let (tr, ti) = (br * wr - bi * wi, br * wi + bi * wr);
                data[start + k] = (ar + tr, ai + ti);
                data[start + k + size / 2] = (ar - tr, ai - ti);
            }
        }
        size *= 2;
    }
    if let Some(label) = label {
        table.heading(vec![text(label)]);
    }
    for (re, im) in data {
        let (re, im) = if inverse {
            (re / count as f64, im / count as f64)
        } else {
            (re, im)
        };
        table.line(vec![Out::Text(complex_text(re, im))]);
    }
}

/// Histogram's choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistogramChoices {
    pub pareto: bool,
    pub cumulative: bool,
}

/// The bins Excel makes when none are given: evenly spaced from the least
/// value to the most.
pub fn automatic_bins(values: &[f64]) -> Vec<f64> {
    let low = kth(values, 1, true);
    let high = kth(values, 1, false);
    let count = ((values.len() as f64).sqrt().floor() as usize).max(1);
    let width = (high - low) / count as f64;
    (0..=count).map(|i| low + width * i as f64).collect()
}

/// Histogram: each bin's count of the values at most it and above the one
/// before, then More; sorted by count beside it with `pareto`.
pub fn histogram(table: &mut Table, values: &[f64], bins: &[f64], choices: HistogramChoices) {
    let mut bins = bins.to_vec();
    bins.sort_by(f64::total_cmp);
    let mut counts = vec![0usize; bins.len() + 1];
    for v in values {
        let at = bins.iter().position(|bin| v <= bin).unwrap_or(bins.len());
        counts[at] += 1;
    }
    let total = values.len() as f64;
    let mut rows: Vec<(Out, usize)> = bins.iter().map(|b| (n(*b), 0)).collect();
    rows.push((k("tool.more"), 0));
    for (row, count) in rows.iter_mut().zip(&counts) {
        row.1 = *count;
    }
    let mut head = vec![k("tool.bin"), k("tool.frequency")];
    if choices.cumulative {
        head.push(k("tool.cumulative"));
    }
    let mut sorted: Vec<(usize, usize)> = counts.iter().copied().enumerate().collect();
    // Most first; More stays after a bin with the same count.
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    if choices.pareto {
        head.push(Out::Blank);
        head.push(k("tool.bin"));
        head.push(k("tool.frequency"));
        if choices.cumulative {
            head.push(k("tool.cumulative"));
        }
        // The blank between the halves is no heading.
        let blank = if choices.cumulative { 3 } else { 2 };
        let mut cells = Vec::new();
        for (i, cell) in head.into_iter().enumerate() {
            cells.push(if i == blank { Out::Blank } else { cell });
        }
        table.heading(cells);
    } else {
        table.heading(head);
    }
    let mut running = 0usize;
    let mut sorted_running = 0usize;
    let label_of = |index: usize| -> Out {
        if index == bins.len() {
            k("tool.more")
        } else {
            n(bins[index])
        }
    };
    for (i, count) in counts.iter().enumerate() {
        running += count;
        let row = table.row;
        let mut line = vec![label_of(i), n(*count as f64)];
        let mut percent_columns = Vec::new();
        if choices.cumulative {
            percent_columns.push(line.len() as u32);
            line.push(n(running as f64 / total));
        }
        if choices.pareto {
            let (index, sorted_count) = sorted[i];
            sorted_running += sorted_count;
            line.push(Out::Blank);
            line.push(label_of(index));
            line.push(n(sorted_count as f64));
            if choices.cumulative {
                percent_columns.push(line.len() as u32);
                line.push(n(sorted_running as f64 / total));
            }
        }
        table.line(line);
        for column in percent_columns {
            table.percent(CellRef::new(column, row));
        }
    }
}

/// Rank and Percentile: for each series, its values largest first with
/// their place in the input, rank and percent rank.
pub fn rank(table: &mut Table, series: &[Series]) {
    let start = table.row;
    let mut longest = start;
    for (index, one) in series.iter().enumerate() {
        let shift = 4 * index as u32;
        let v = &one.values;
        let mut order: Vec<usize> = (0..v.len()).collect();
        order.sort_by(|&a, &b| v[b].total_cmp(&v[a]).then(a.cmp(&b)));
        let mut sub = Table::new(table.words);
        sub.heading(vec![k("tool.point"), text(&one.label), k("tool.rank"), k("tool.percent")]);
        let below = |value: f64| v.iter().filter(|w| **w < value).count() as f64;
        for at in order {
            let rank = v.iter().filter(|w| **w > v[at]).count() + 1;
            let percent = if v.len() > 1 {
                (below(v[at]) / (v.len() as f64 - 1.0) * 1000.0).floor() / 1000.0
            } else {
                1.0
            };
            let row = sub.row;
            sub.line(vec![n(at as f64 + 1.0), n(v[at]), n(rank as f64), n(percent)]);
            sub.percent(CellRef::new(3, row));
        }
        for (at, value) in sub.report.cells {
            table.report.cells.push((CellRef::new(at.column + shift, at.row + start), value));
        }
        for at in sub.report.bold {
            table.report.bold.push(CellRef::new(at.column + shift, at.row + start));
        }
        for (at, format) in sub.report.formats {
            table
                .report
                .formats
                .push((CellRef::new(at.column + shift, at.row + start), format));
        }
        longest = longest.max(start + sub.row);
    }
    table.row = longest;
}

/// Regression's choices.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegressionChoices {
    pub constant_zero: bool,
    /// A confidence level besides 95%.
    pub confidence: Option<f64>,
    pub residuals: bool,
    pub standardized: bool,
    pub probability: bool,
}

/// Regression: least squares of `y` on the `x` series, Excel's SUMMARY
/// OUTPUT.
pub fn regression(table: &mut Table, y: &Series, x: &[Series], choices: RegressionChoices) -> Result<(), &'static str> {
    let count = y.values.len();
    let constant = !choices.constant_zero;
    let design: Vec<Vec<f64>> = (0..count)
        .map(|i| {
            let mut row = Vec::new();
            if constant {
                row.push(1.0);
            }
            row.extend(x.iter().map(|s| s.values[i]));
            row
        })
        .collect();
    let (b, inverse) = least_squares(&design, &y.values).ok_or("tool.alert.singular")?;
    let predicted: Vec<f64> = design.iter().map(|row| row.iter().zip(&b).map(|(p, q)| p * q).sum()).collect();
    let residuals: Vec<f64> = y.values.iter().zip(&predicted).map(|(a, p)| a - p).collect();
    let ss_residual: f64 = residuals.iter().map(|e| e * e).sum();
    let ss_total = if constant {
        devsq(&y.values)
    } else {
        y.values.iter().map(|v| v * v).sum()
    };
    let ss_regression = ss_total - ss_residual;
    let observations = count as f64;
    let predictors = x.len() as f64;
    let df_regression = predictors;
    let df_residual = observations - predictors - if constant { 1.0 } else { 0.0 };
    let df_total = if constant { observations - 1.0 } else { observations };
    let r2 = ss_regression / ss_total;
    let adjusted = 1.0 - (1.0 - r2) * df_total / df_residual;
    let ms_residual = ss_residual / df_residual;
    let ms_regression = ss_regression / df_regression;
    let f = ms_regression / ms_residual;
    table.line(vec![k("tool.summary_output")]);
    table.blank();
    table.heading(vec![k("tool.regression_statistics")]);
    table.line(vec![k("tool.multiple_r"), n(r2.sqrt())]);
    table.line(vec![k("tool.r_square"), n(r2)]);
    table.line(vec![k("tool.adjusted_r_square"), n(adjusted)]);
    table.line(vec![k("tool.standard_error"), n(ms_residual.sqrt())]);
    table.line(vec![k("tool.observations"), n(observations)]);
    table.blank();
    table.line(vec![k("tool.anova")]);
    table.heading(vec![Out::Blank, k("tool.df"), k("tool.ss"), k("tool.ms"), k("tool.f"), k("tool.significance_f")]);
    table.line(vec![
        k("tool.regression"),
        n(df_regression),
        n(ss_regression),
        n(ms_regression),
        n(f),
        n(1.0 - f_cdf(f, df_regression, df_residual)),
    ]);
    table.line(vec![k("tool.residual"), n(df_residual), n(ss_residual), n(ms_residual)]);
    table.line(vec![k("tool.total"), n(df_total), n(ss_total)]);
    table.blank();
    let level = choices.confidence.unwrap_or(0.95);
    let percent = |level: f64| format!("{:.1}", level * 100.0);
    let lower95 = format!("{} 95%", table.word("tool.lower"));
    let upper95 = format!("{} 95%", table.word("tool.upper"));
    let lower = format!("{} {}%", table.word("tool.lower"), percent(level));
    let upper = format!("{} {}%", table.word("tool.upper"), percent(level));
    table.heading(vec![
        Out::Blank,
        k("tool.coefficients"),
        k("tool.standard_error"),
        k("tool.t_stat"),
        k("tool.p_value"),
        Out::Text(lower95),
        Out::Text(upper95),
        Out::Text(lower),
        Out::Text(upper),
    ]);
    let t95 = t_inverse(0.975, df_residual);
    let t_level = t_inverse(1.0 - (1.0 - level) / 2.0, df_residual);
    let mut names: Vec<Out> = Vec::new();
    if constant {
        names.push(k("tool.intercept"));
    } else {
        // Excel still lists the intercept, at zero, with #N/A beside it.
        table.line(vec![
            k("tool.intercept"),
            n(0.0),
            text("#N/A"),
            text("#N/A"),
            text("#N/A"),
            text("#N/A"),
            text("#N/A"),
            text("#N/A"),
            text("#N/A"),
        ]);
    }
    names.extend(x.iter().map(|s| text(&s.label)));
    for (i, name) in names.into_iter().enumerate() {
        let error = (inverse[i][i] * ms_residual).sqrt();
        let t = b[i] / error;
        table.line(vec![
            name,
            n(b[i]),
            n(error),
            n(t),
            n(2.0 * (1.0 - t_cdf(t.abs(), df_residual))),
            n(b[i] - t95 * error),
            n(b[i] + t95 * error),
            n(b[i] - t_level * error),
            n(b[i] + t_level * error),
        ]);
    }
    if choices.residuals || choices.standardized {
        table.blank();
        table.blank();
        table.blank();
        table.line(vec![k("tool.residual_output")]);
        table.blank();
        let mut head = vec![k("tool.observation"), Out::Text(format!("{} {}", table.word("tool.predicted"), y.label))];
        if choices.residuals {
            head.push(k("tool.residuals"));
        }
        if choices.standardized {
            head.push(k("tool.standard_residuals"));
        }
        table.heading(head);
        let spread = (ss_residual / (observations - 1.0)).sqrt();
        for i in 0..count {
            let mut line = vec![n(i as f64 + 1.0), n(predicted[i])];
            if choices.residuals {
                line.push(n(residuals[i]));
            }
            if choices.standardized {
                line.push(n(residuals[i] / spread));
            }
            table.line(line);
        }
    }
    if choices.probability {
        table.blank();
        table.blank();
        table.line(vec![k("tool.probability_output")]);
        table.blank();
        table.heading(vec![k("tool.percentile"), text(&y.label)]);
        let mut sorted = y.values.clone();
        sorted.sort_by(f64::total_cmp);
        for (i, value) in sorted.iter().enumerate() {
            table.line(vec![n(100.0 * (i as f64 + 0.5) / observations), n(*value)]);
        }
    }
    Ok(())
}

/// A seeded random number source for the tools, as Excel's Random Seed
/// makes the numbers repeat.
pub struct Random(u64);

impl Random {
    pub fn new(seed: Option<u64>) -> Self {
        let seed = seed.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |d| d.as_nanos() as u64)
        });
        Self(seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407) | 1)
    }

    /// Uniform on [0, 1).
    pub fn next(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn normal(&mut self) -> f64 {
        let u = self.next().max(1e-300);
        let v = self.next();
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }

    pub fn poisson(&mut self, lambda: f64) -> f64 {
        let limit = (-lambda).exp();
        let mut product = self.next();
        let mut count = 0.0;
        while product > limit && count < 1e7 {
            product *= self.next();
            count += 1.0;
        }
        count
    }
}

/// Random Number Generation's distributions.
#[derive(Debug, Clone, PartialEq)]
pub enum Distribution {
    Uniform(f64, f64),
    Normal(f64, f64),
    Bernoulli(f64),
    Binomial(f64, u32),
    Poisson(f64),
    /// From, to, step, each number repeated, the sequence repeated.
    Patterned(f64, f64, f64, u32, u32),
    /// Values and their probabilities.
    Discrete(Vec<(f64, f64)>),
}

/// Random Number Generation: `rows` numbers in each of `columns` columns.
pub fn random_numbers(table: &mut Table, columns: u32, rows: u32, distribution: &Distribution, random: &mut Random) {
    let mut sequence: Vec<f64> = Vec::new();
    if let Distribution::Patterned(from, to, step, each, times) = distribution {
        let mut one = Vec::new();
        let mut value = *from;
        let mut guard = 0;
        while (if *step >= 0.0 { value <= to + 1e-12 } else { value >= to - 1e-12 }) && guard < 100_000 {
            for _ in 0..(*each).max(1) {
                one.push(value);
            }
            value += step;
            guard += 1;
            if *step == 0.0 {
                break;
            }
        }
        for _ in 0..(*times).max(1) {
            sequence.extend(&one);
        }
    }
    let draw = |random: &mut Random, row: u32| -> f64 {
        match distribution {
            Distribution::Uniform(low, high) => low + random.next() * (high - low),
            Distribution::Normal(mean, deviation) => mean + deviation * random.normal(),
            Distribution::Bernoulli(p) => f64::from(u8::from(random.next() < *p)),
            Distribution::Binomial(p, trials) => (0..*trials).filter(|_| random.next() < *p).count() as f64,
            Distribution::Poisson(lambda) => random.poisson(*lambda),
            Distribution::Patterned(..) => {
                if sequence.is_empty() {
                    0.0
                } else {
                    sequence[row as usize % sequence.len()]
                }
            }
            Distribution::Discrete(table) => {
                let total: f64 = table.iter().map(|(_, p)| p).sum();
                let mut pick = random.next() * total;
                for (value, p) in table {
                    if pick < *p {
                        return *value;
                    }
                    pick -= p;
                }
                table.last().map_or(0.0, |(value, _)| *value)
            }
        }
    };
    // Patterned numbers run down each column; the rest are drawn a row at a
    // time, column by column.
    for row in 0..rows {
        let line: Vec<Out> = (0..columns.max(1)).map(|_| n(draw(random, row))).collect();
        table.line(line);
    }
}

/// Sampling: every `period`th value, or `count` values picked at random
/// (with replacement).
pub fn sampling(table: &mut Table, values: &[f64], periodic: Option<usize>, count: usize, random: &mut Random) {
    match periodic {
        Some(period) => {
            let period = period.max(1);
            for value in values.iter().skip(period - 1).step_by(period) {
                table.line(vec![n(*value)]);
            }
        }
        None => {
            for _ in 0..count {
                let at = ((random.next() * values.len() as f64) as usize).min(values.len().saturating_sub(1));
                if let Some(value) = values.get(at) {
                    table.line(vec![n(*value)]);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(key: &'static str) -> String {
        key.to_string()
    }

    fn value(table: &Table, column: u32, row: u32) -> f64 {
        match table.report.cells.iter().find(|(at, _)| *at == CellRef::new(column, row)) {
            Some((_, CellInput::Number(v))) => *v,
            other => panic!("{column},{row}: {other:?}"),
        }
    }

    fn series(label: &str, values: &[f64]) -> Series {
        Series {
            label: label.into(),
            values: values.to_vec(),
        }
    }

    #[test]
    fn anova_single_factor_matches_excel() {
        let mut table = Table::new(&words);
        let groups = [
            series("A", &[4.0, 5.0, 6.0]),
            series("B", &[6.0, 7.0, 8.0]),
            series("C", &[9.0, 10.0, 11.0]),
        ];
        anova_single(&mut table, &groups, 0.05);
        // Rows: title, blank, SUMMARY, heading, 3 groups, 2 blank, ANOVA,
        // heading, Between at 11.
        assert!((value(&table, 1, 11) - 38.0).abs() < 1e-9);
        assert!((value(&table, 2, 11) - 2.0).abs() < 1e-9);
        assert!((value(&table, 4, 11) - 19.0).abs() < 1e-9);
        // F crit = F.INV.RT(0.05, 2, 6) = 5.143253.
        assert!((value(&table, 6, 11) - 5.143_252_85).abs() < 1e-6);
        assert!((value(&table, 1, 14) - 44.0).abs() < 1e-9);
    }

    #[test]
    fn regression_fits_a_line_exactly() {
        let mut table = Table::new(&words);
        let y = series("Y", &[3.0, 5.0, 7.1, 8.9, 11.0]);
        let x = [series("X", &[1.0, 2.0, 3.0, 4.0, 5.0])];
        regression(
            &mut table,
            &y,
            &x,
            RegressionChoices {
                constant_zero: false,
                confidence: None,
                residuals: true,
                standardized: true,
                probability: false,
            },
        )
        .unwrap();
        // The intercept on row 16, the slope on 17.
        assert!((value(&table, 1, 17) - 1.99).abs() < 1e-9, "{:?}", table.report.cells);
        assert!((value(&table, 1, 16) - 1.03).abs() < 1e-9);
        assert!(value(&table, 1, 4) > 0.999);
    }

    #[test]
    fn t_test_paired_and_complex_text() {
        let mut table = Table::new(&words);
        let a = series("Variable 1", &[10.0, 12.0, 9.0, 11.0]);
        let b = series("Variable 2", &[9.0, 11.0, 9.0, 10.0]);
        t_test(&mut table, TTest::Paired, &a, &b, 0.0, 0.05);
        // t Stat on row 9: mean difference 0.75, sd 0.5, n 4: t = 3.
        assert!((value(&table, 1, 9) - 3.0).abs() < 1e-9);
        assert_eq!(complex_text(3.0, -4.0), "3-4i");
        assert_eq!(complex_text(0.0, 1.0), "i");
        assert_eq!(parse_complex("3-4i"), Some((3.0, -4.0)));
        assert_eq!(parse_complex("-i"), Some((0.0, -1.0)));
        assert_eq!(parse_complex("2.5e-3+1i"), Some((0.0025, 1.0)));
    }

    #[test]
    fn fourier_of_a_step_and_back() {
        let mut table = Table::new(&words);
        fourier(&mut table, &[(1.0, 0.0), (1.0, 0.0), (0.0, 0.0), (0.0, 0.0)], false, None);
        let texts: Vec<String> = table
            .report
            .cells
            .iter()
            .map(|(_, v)| match v {
                CellInput::Text(t) => t.clone(),
                other => format!("{other:?}"),
            })
            .collect();
        assert_eq!(texts, ["2", "1-i", "0", "1+i"]);
    }

    #[test]
    fn histogram_counts_into_bins_and_more() {
        let mut table = Table::new(&words);
        histogram(
            &mut table,
            &[1.0, 2.0, 2.0, 3.0, 5.0, 9.0],
            &[2.0, 4.0],
            HistogramChoices {
                pareto: false,
                cumulative: true,
            },
        );
        assert_eq!(value(&table, 1, 1), 3.0);
        assert_eq!(value(&table, 1, 2), 1.0);
        assert_eq!(value(&table, 1, 3), 2.0);
        assert!((value(&table, 2, 3) - 1.0).abs() < 1e-12);
    }
}

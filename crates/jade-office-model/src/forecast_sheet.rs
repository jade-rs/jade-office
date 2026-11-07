//! Data ▸ Forecast Sheet: a timeline and its values carried on to a
//! forecast end by exponential smoothing, as Excel's command makes it — a
//! new sheet with the data, the forecast and its confidence bounds worked
//! out by `FORECAST.ETS` and `FORECAST.ETS.CONFINT`, the forecast
//! statistics by `FORECAST.ETS.STAT` when asked for, and a line or column
//! chart of them all.

use jade_xlsx_headless_api::CellInput;
use jade_xlsx_headless_editor::{ChartChange, ChartKind, SeriesReferences};
use jade_xlsx_reader::{quote_sheet_name, CellRange, CellRef, CellValue};

use crate::form::{Form, Label};
use crate::report::{Output, Report};
use crate::sheet::{column_label, reference_text, Workbook};
use crate::what_if::{absolute_range, Outcome, ToolDialog};
use crate::Backend;

/// How points falling on the same date are put together, as
/// `FORECAST.ETS`'s aggregation argument numbers them.
pub const AGGREGATIONS: [(&str, u8); 7] = [
    ("forecast.agg.average", 1),
    ("forecast.agg.count", 2),
    ("forecast.agg.counta", 3),
    ("forecast.agg.max", 4),
    ("forecast.agg.median", 5),
    ("forecast.agg.min", 6),
    ("forecast.agg.sum", 7),
];

/// What a Forecast Sheet is asked for.
#[derive(Debug, Clone, PartialEq)]
pub struct Forecast {
    /// The timeline's cells and the values', on the active sheet, each
    /// with its header on top.
    pub timeline: CellRange,
    pub values: CellRange,
    /// Forecast End, on the timeline.
    pub end: f64,
    /// Forecast Start: the point the forecast starts at; `None` for the
    /// timeline's last.
    pub start: Option<f64>,
    /// Confidence Interval, as a fraction: 0.95.
    pub confidence: Option<f64>,
    /// Seasonality: `None` detected automatically, `Some(0)` none.
    pub seasonality: Option<u32>,
    pub statistics: bool,
    /// Fill Missing Points Using zeros rather than interpolation.
    pub zeros: bool,
    /// Aggregate Duplicates Using, as [`AGGREGATIONS`] numbers it.
    pub aggregation: u8,
    /// A column chart rather than a line one.
    pub column_chart: bool,
}

/// The words a forecast sheet's headings are written in.
#[derive(Debug, Clone)]
pub struct ForecastWords {
    pub forecast: String,
    pub lower: String,
    pub upper: String,
    pub statistic: String,
    pub value: String,
    pub statistics: [String; 7],
}

impl Default for ForecastWords {
    fn default() -> Self {
        Self {
            forecast: "Forecast({})".into(),
            lower: "Lower Confidence Bound({})".into(),
            upper: "Upper Confidence Bound({})".into(),
            statistic: "Statistic".into(),
            value: "Value".into(),
            statistics: [
                "Alpha".into(),
                "Beta".into(),
                "Gamma".into(),
                "MASE".into(),
                "SMAPE".into(),
                "MAE".into(),
                "RMSE".into(),
            ],
        }
    }
}

/// The steps between a sorted timeline's points: the one most often met.
fn step(points: &[f64]) -> Option<f64> {
    let mut steps: Vec<f64> = points
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .filter(|step| *step > 0.0)
        .collect();
    steps.sort_by(f64::total_cmp);
    let mut best: Option<(f64, usize)> = None;
    let mut index = 0;
    while index < steps.len() {
        let run = steps[index..]
            .iter()
            .take_while(|step| (**step - steps[index]).abs() < 1e-9)
            .count();
        if best.is_none_or(|(_, count)| run > count) {
            best = Some((steps[index], run));
        }
        index += run;
    }
    best.map(|(step, _)| step)
}

impl<B: Backend> Workbook<B> {
    /// The timeline's numbers and the values', by row, under their headers.
    fn forecast_points(&self, job: &Forecast) -> Result<Vec<(f64, Option<f64>)>, String> {
        let height = job.timeline.end.row - job.timeline.start.row;
        if height < 2
            || job.values.end.row - job.values.start.row != height
            || job.timeline.start.column != job.timeline.end.column
            || job.values.start.column != job.values.end.column
        {
            return Err("forecast.alert.ranges".into());
        }
        let here = self.sheet_index();
        let sheet = &self.engine().workbook().sheets[here];
        let value = |at: CellRef| sheet.cell_at(at).map(|cell| cell.value.clone());
        (1..=height)
            .map(|offset| {
                let time = match value(CellRef::new(
                    job.timeline.start.column,
                    job.timeline.start.row + offset,
                )) {
                    Some(CellValue::Number(number)) => number,
                    _ => return Err("forecast.alert.timeline".to_string()),
                };
                let at = CellRef::new(job.values.start.column, job.values.start.row + offset);
                Ok((
                    time,
                    match value(at) {
                        Some(CellValue::Number(number)) => Some(number),
                        _ => None,
                    },
                ))
            })
            .collect()
    }

    /// The timeline's last point and its step, for the dialog's Forecast
    /// End (a third of the data on past the last point, as Excel offers).
    pub fn forecast_defaults(&self, timeline: CellRange, values: CellRange) -> Option<(f64, f64)> {
        let job = Forecast {
            timeline,
            values,
            end: 0.0,
            start: None,
            confidence: None,
            seasonality: None,
            statistics: false,
            zeros: false,
            aggregation: 1,
            column_chart: false,
        };
        let mut points: Vec<f64> = self
            .forecast_points(&job)
            .ok()?
            .into_iter()
            .map(|(time, _)| time)
            .collect();
        points.sort_by(f64::total_cmp);
        let step = step(&points)?;
        let last = *points.last()?;
        let ahead = (points.len() as f64 / 3.0).ceil().max(1.0);
        Some((last, last + step * ahead))
    }

    /// Data ▸ Forecast Sheet: a new sheet after the active one with the
    /// forecast table and its chart, in one undo step; the sheet's index.
    pub fn forecast_sheet(
        &mut self,
        job: &Forecast,
        words: &ForecastWords,
    ) -> Result<usize, String> {
        let mut points = self.forecast_points(job)?;
        points.sort_by(|a, b| a.0.total_cmp(&b.0));
        let times: Vec<f64> = points.iter().map(|(time, _)| *time).collect();
        let Some(step) = step(&times) else {
            return Err("forecast.alert.timeline".into());
        };
        let last = *times.last().expect("two points or more");
        let start = job.start.unwrap_or(last).min(last);
        if job.end <= start {
            return Err("forecast.alert.end".into());
        }
        if job
            .confidence
            .is_some_and(|level| !(0.0..1.0).contains(&level) || level == 0.0)
        {
            return Err("forecast.alert.confidence".into());
        }
        let here = self.sheet_index();
        let header = |at: CellRef| {
            self.engine()
                .cell(&self.sheet_names()[here], &reference_text(at))
                .map(|cell| cell.display)
                .unwrap_or_default()
        };
        let time_name = header(job.timeline.start);
        let value_name = header(job.values.start);
        let time_format = self.number_format(CellRef::new(
            job.timeline.start.column,
            job.timeline.start.row + 1,
        ));
        let value_format = self.number_format(CellRef::new(
            job.values.start.column,
            job.values.start.row + 1,
        ));
        // The data the forecast is made from: the points up to its start.
        let known = points.iter().take_while(|(time, _)| *time <= start).count();
        let mut report = Report::default();
        let named = |pattern: &str| pattern.replace("{}", &value_name);
        for (column, text) in [
            time_name.clone(),
            value_name.clone(),
            named(&words.forecast),
            named(&words.lower),
            named(&words.upper),
        ]
        .iter()
        .enumerate()
        {
            report.text(CellRef::new(column as u32, 0), text);
        }
        for (index, (time, value)) in points.iter().enumerate() {
            let row = index as u32 + 1;
            report.number(CellRef::new(0, row), *time);
            if let Some(value) = value {
                report.number(CellRef::new(1, row), *value);
            }
        }
        let first_data = 2;
        let last_data = known as u32 + 1;
        let values = format!("$B${first_data}:$B${last_data}");
        let timeline = format!("$A${first_data}:$A${last_data}");
        let seasonality = job.seasonality.map_or(1, |season| season);
        let completion = u8::from(!job.zeros);
        let aggregation = job.aggregation;
        let confidence = job.confidence.unwrap_or(0.95);
        // The last known point starts the forecast's lines: its value in
        // each, so they join the data's.
        if let Some(value) = points[known - 1].1 {
            for column in 2..=4 {
                report.number(CellRef::new(column, known as u32), value);
            }
        }
        let mut row = known as u32 + 1;
        let mut time = start;
        loop {
            time += step;
            if time > job.end + step * 1e-9 {
                break;
            }
            let target = format!("A{}", row + 1);
            // Points past the data already on the sheet: the timeline on.
            if row as usize > points.len() {
                report.number(CellRef::new(0, row), time);
            }
            let forecast = format!(
                "=FORECAST.ETS({target},{values},{timeline},{seasonality},{completion},{aggregation})"
            );
            let interval = format!(
                "FORECAST.ETS.CONFINT({target},{values},{timeline},{confidence},{seasonality},{completion},{aggregation})"
            );
            report.put(CellRef::new(2, row), CellInput::Formula(forecast));
            report.put(
                CellRef::new(3, row),
                CellInput::Formula(format!("=C{}-{interval}", row + 1)),
            );
            report.put(
                CellRef::new(4, row),
                CellInput::Formula(format!("=C{}+{interval}", row + 1)),
            );
            row += 1;
        }
        let last_row = row.max(points.len() as u32 + 1) - 1;
        for row in 1..=last_row {
            if let Some(code) = &time_format {
                report.formats.push((CellRef::new(0, row), code.clone()));
            }
            if let Some(code) = &value_format {
                for column in 1..=4 {
                    report
                        .formats
                        .push((CellRef::new(column, row), code.clone()));
                }
            }
        }
        for column in 0..5 {
            report.bold.push(CellRef::new(column, 0));
        }
        if job.statistics {
            report.heading(CellRef::new(6, 0), &words.statistic);
            report.heading(CellRef::new(7, 0), &words.value);
            for (index, name) in words.statistics.iter().enumerate() {
                let row = index as u32 + 1;
                report.text(CellRef::new(6, row), name);
                report.put(
                    CellRef::new(7, row),
                    CellInput::Formula(format!(
                        "=FORECAST.ETS.STAT({values},{timeline},{},{seasonality},{completion},{aggregation})",
                        index + 1
                    )),
                );
            }
        }
        let names = self.sheet_names();
        let wanted = (names.len() + 1..)
            .map(|n| format!("Sheet{n}"))
            .find(|name| !names.iter().any(|taken| taken.eq_ignore_ascii_case(name)))
            .unwrap_or_default();
        self.atomic_edit(|book| {
            let (index, mut steps) = book.write_report_steps(&Output::NewSheet(wanted), &report)?;
            let sheet = book.sheet_names()[index].clone();
            let qualified = quote_sheet_name(&sheet);
            let kind = if job.column_chart {
                ChartKind::Column
            } else {
                ChartKind::Line
            };
            let id = book
                .engine_mut()
                .insert_chart(&sheet, &format!("B1:E{}", last_row + 1), kind)
                .map_err(|error| error.to_string())?;
            steps += 1;
            let categories = format!("{qualified}!$A$2:$A${}", last_row + 1);
            let series = (1..=4u32)
                .map(|column| {
                    let letter = column_label(column);
                    SeriesReferences {
                        name: Some(format!("{qualified}!${letter}$1")),
                        categories: Some(categories.clone()),
                        text_categories: false,
                        values: format!("{qualified}!${letter}$2:${letter}${}", last_row + 1),
                        bubble_sizes: None,
                        kind: None,
                        secondary: false,
                    }
                })
                .collect();
            book.engine_mut()
                .set_chart(
                    &sheet,
                    id,
                    &ChartChange {
                        series: Some(series),
                        ..Default::default()
                    },
                )
                .map_err(|error| error.to_string())?;
            steps += 1;
            book.wrote(steps)?;
            if index != book.sheet_index() {
                book.select_sheet(index)?;
            }
            Ok(index)
        })
    }
}

/// The Forecast Sheet dialog: Forecast End, and its Options — Forecast
/// Start, Confidence Interval, Seasonality, Include forecast statistics,
/// the timeline and values ranges, Fill Missing Points Using, Aggregate
/// Duplicates Using — and the chart's kind.
pub struct ForecastDialog {
    pub form: Form,
    words: ForecastWords,
}

impl ForecastDialog {
    /// Opened on the selection — or the list round the active cell — its
    /// first column the timeline, its second the values.
    pub fn open<B: Backend>(book: &Workbook<B>, selected: CellRange) -> Self {
        let range = if selected.start == selected.end {
            book.current_region(selected.start).unwrap_or(selected)
        } else {
            selected
        };
        let column = |at: u32| CellRange {
            start: CellRef::new(at, range.start.row),
            end: CellRef::new(at, range.end.row),
        };
        let timeline = column(range.start.column);
        let values = column((range.start.column + 1).min(range.end.column));
        let (last, end) = book
            .forecast_defaults(timeline, values)
            .unwrap_or((0.0, 0.0));
        let number = |value: f64| crate::sheet::format_number(value);
        Self {
            form: Form::new("forecast.dialog")
                .choice(
                    "chart",
                    "forecast.chart",
                    vec![Label::Key("forecast.line"), Label::Key("forecast.column")],
                    0,
                )
                .field("end", "forecast.end", &number(end))
                .field("start", "forecast.start", &number(last))
                .field("confidence", "forecast.confidence", "95%")
                .choice(
                    "seasonality",
                    "forecast.seasonality",
                    vec![
                        Label::Key("forecast.season_auto"),
                        Label::Key("forecast.season_manual"),
                    ],
                    0,
                )
                .field("season", "forecast.season", "")
                .check("statistics", "forecast.statistics", false)
                .field("timeline", "forecast.timeline", &absolute_range(timeline))
                .field("values", "forecast.values", &absolute_range(values))
                .choice(
                    "missing",
                    "forecast.missing",
                    vec![
                        Label::Key("forecast.interpolation"),
                        Label::Key("forecast.zeros"),
                    ],
                    0,
                )
                .choice(
                    "aggregate",
                    "forecast.aggregate",
                    AGGREGATIONS
                        .iter()
                        .map(|(key, _)| Label::Key(key))
                        .collect(),
                    0,
                )
                .button("ok", "forecast.create")
                .button("cancel", "validation.cancel"),
            words: ForecastWords::default(),
        }
    }
}

/// A number typed into the dialog: `95%` a fraction, a date as the sheet
/// reads one.
fn typed_number(text: &str) -> Option<f64> {
    let text = text.trim();
    if let Some(percent) = text.strip_suffix('%') {
        return percent
            .trim()
            .parse::<f64>()
            .ok()
            .map(|value| value / 100.0);
    }
    text.parse::<f64>().ok()
}

impl ToolDialog for ForecastDialog {
    fn form(&self) -> &Form {
        &self.form
    }

    fn form_mut(&mut self) -> &mut Form {
        &mut self.form
    }

    fn set_words(&mut self, words: fn(&'static str) -> &'static str) {
        let word = |key| words(key).to_string();
        self.words = ForecastWords {
            forecast: word("forecast.word.forecast"),
            lower: word("forecast.word.lower"),
            upper: word("forecast.word.upper"),
            statistic: word("forecast.word.statistic"),
            value: word("forecast.word.value"),
            statistics: [
                word("forecast.word.alpha"),
                word("forecast.word.beta"),
                word("forecast.word.gamma"),
                word("forecast.word.mase"),
                word("forecast.word.smape"),
                word("forecast.word.mae"),
                word("forecast.word.rmse"),
            ],
        };
    }

    fn press<B: Backend>(
        &mut self,
        button: &str,
        book: &mut Workbook<B>,
    ) -> Result<Outcome, String> {
        if button != "ok" {
            return Ok(Outcome::close());
        }
        let form = &mut self.form;
        form.alert = None;
        let alert = |form: &mut Form, key: &'static str| {
            form.alert = Some(Label::Key(key));
            Ok(Outcome::stay())
        };
        let here = book.sheet_index();
        let range = |text: &str| match book.resolve_reference(text) {
            Some((sheet, range)) if sheet == here => Some(range),
            _ => None,
        };
        let (Some(timeline), Some(values)) =
            (range(form.text("timeline")), range(form.text("values")))
        else {
            return alert(form, "forecast.alert.ranges");
        };
        // A date typed in: what the sheet would make of it.
        let date1904 = book.date_system() == jade_xlsx_reader::DateSystem::Nineteen04;
        let point = |text: &str| match crate::typing::read(text, date1904).input {
            CellInput::Number(number) => Some(number),
            _ => None,
        };
        let Some(end) = point(form.text("end")) else {
            return alert(form, "forecast.alert.end");
        };
        let start = match form.text("start").trim() {
            "" => None,
            text => match point(text) {
                Some(start) => Some(start),
                None => return alert(form, "forecast.alert.start"),
            },
        };
        let confidence = match typed_number(form.text("confidence")) {
            Some(level) => Some(level),
            None => return alert(form, "forecast.alert.confidence"),
        };
        let seasonality = if form.chosen("seasonality") == Some(1) {
            match form.text("season").trim().parse::<u32>() {
                Ok(season) if season <= 8760 => Some(season),
                _ => return alert(form, "forecast.alert.season"),
            }
        } else {
            None
        };
        let job = Forecast {
            timeline,
            values,
            end,
            start,
            confidence,
            seasonality,
            statistics: form.checked("statistics"),
            zeros: form.chosen("missing") == Some(1),
            aggregation: AGGREGATIONS[form.chosen("aggregate").unwrap_or(0)].1,
            column_chart: form.chosen("chart") == Some(1),
        };
        match book.forecast_sheet(&job, &self.words) {
            Ok(_) => Ok(Outcome::closed_writing()),
            Err(error) if error.starts_with("forecast.alert.") => {
                let key = match error.as_str() {
                    "forecast.alert.timeline" => "forecast.alert.timeline",
                    "forecast.alert.end" => "forecast.alert.end",
                    "forecast.alert.confidence" => "forecast.alert.confidence",
                    _ => "forecast.alert.ranges",
                };
                alert(&mut self.form, key)
            }
            Err(error) => Err(error),
        }
    }
}

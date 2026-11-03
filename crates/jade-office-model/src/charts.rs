//! Charts and other drawing objects on a sheet, as both shells work them:
//! Insert ▸ Chart's types, the handles of a selected object and what a
//! drag on its border or a handle does to it, a chart as Chart Design
//! shows it, and its dialogs — Chart Title, Select Data Source and its Edit
//! Series, the Format dialogs, Chart Elements and Chart Styles.

pub use jade_xlsx_headless_editor::{
    AxisFormat, AxisId, AxisKind, ChartChange, ChartFormat, ChartKind, Crosses, ErrorBarFormat,
    Fill, LabelFormat, LegendPosition, SeriesFormat, SeriesReferences, TableFormat,
    TrendlineFormat,
};
use jade_xlsx_reader::chart::{AreaFill, Chart};

use crate::chart_form::{Form, FormKey};
use crate::sheet::{CellRange, CellRef, Workbook};
use crate::Backend;

/// A type as Insert ▸ Chart and Change Chart Type offer it: one of a
/// chart's types, or a combination — the first series drawn as one type,
/// the rest as another, on the secondary axis or not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartPick {
    Kind(ChartKind),
    Combo(ChartKind, ChartKind, bool),
}

/// Insert ▸ Chart's gallery: its families, in Excel's order, each with its
/// types and their words.
pub const CHART_GALLERY: [(&str, &[(ChartPick, &str)]); 9] = [
    (
        "chart.family.column",
        &[
            (ChartPick::Kind(ChartKind::Column), "chart.kind.column"),
            (
                ChartPick::Kind(ChartKind::StackedColumn),
                "chart.kind.stacked_column",
            ),
            (
                ChartPick::Kind(ChartKind::PercentStackedColumn),
                "chart.kind.percent_column",
            ),
        ],
    ),
    (
        "chart.family.bar",
        &[
            (ChartPick::Kind(ChartKind::Bar), "chart.kind.bar"),
            (
                ChartPick::Kind(ChartKind::StackedBar),
                "chart.kind.stacked_bar",
            ),
            (
                ChartPick::Kind(ChartKind::PercentStackedBar),
                "chart.kind.percent_bar",
            ),
        ],
    ),
    (
        "chart.family.line",
        &[
            (ChartPick::Kind(ChartKind::Line), "chart.kind.line"),
            (
                ChartPick::Kind(ChartKind::StackedLine),
                "chart.kind.stacked_line",
            ),
            (
                ChartPick::Kind(ChartKind::PercentStackedLine),
                "chart.kind.percent_line",
            ),
            (
                ChartPick::Kind(ChartKind::LineWithMarkers),
                "chart.kind.line_markers",
            ),
            (
                ChartPick::Kind(ChartKind::StackedLineWithMarkers),
                "chart.kind.stacked_line_markers",
            ),
            (
                ChartPick::Kind(ChartKind::PercentStackedLineWithMarkers),
                "chart.kind.percent_line_markers",
            ),
        ],
    ),
    (
        "chart.family.area",
        &[
            (ChartPick::Kind(ChartKind::Area), "chart.kind.area"),
            (
                ChartPick::Kind(ChartKind::StackedArea),
                "chart.kind.stacked_area",
            ),
            (
                ChartPick::Kind(ChartKind::PercentStackedArea),
                "chart.kind.percent_area",
            ),
        ],
    ),
    (
        "chart.family.pie",
        &[
            (ChartPick::Kind(ChartKind::Pie), "chart.kind.pie"),
            (ChartPick::Kind(ChartKind::Doughnut), "chart.kind.doughnut"),
        ],
    ),
    (
        "chart.family.scatter",
        &[
            (ChartPick::Kind(ChartKind::Scatter), "chart.kind.scatter"),
            (
                ChartPick::Kind(ChartKind::ScatterSmoothMarkers),
                "chart.kind.scatter_smooth_markers",
            ),
            (
                ChartPick::Kind(ChartKind::ScatterSmooth),
                "chart.kind.scatter_smooth",
            ),
            (
                ChartPick::Kind(ChartKind::ScatterLinesMarkers),
                "chart.kind.scatter_lines_markers",
            ),
            (
                ChartPick::Kind(ChartKind::ScatterLines),
                "chart.kind.scatter_lines",
            ),
        ],
    ),
    (
        "chart.family.bubble",
        &[(ChartPick::Kind(ChartKind::Bubble), "chart.kind.bubble")],
    ),
    (
        "chart.family.radar",
        &[
            (ChartPick::Kind(ChartKind::Radar), "chart.kind.radar"),
            (
                ChartPick::Kind(ChartKind::RadarWithMarkers),
                "chart.kind.radar_markers",
            ),
            (
                ChartPick::Kind(ChartKind::FilledRadar),
                "chart.kind.filled_radar",
            ),
        ],
    ),
    (
        "chart.family.combo",
        &[
            (
                ChartPick::Combo(ChartKind::Column, ChartKind::Line, false),
                "chart.kind.column_line",
            ),
            (
                ChartPick::Combo(ChartKind::Column, ChartKind::Line, true),
                "chart.kind.column_line_secondary",
            ),
            (
                ChartPick::Combo(ChartKind::StackedArea, ChartKind::Column, false),
                "chart.kind.area_column",
            ),
        ],
    ),
];

/// The words for a chart type, as the gallery has them.
pub fn kind_label(kind: ChartKind) -> &'static str {
    if kind == ChartKind::Combo {
        return "chart.family.combo";
    }
    CHART_GALLERY
        .iter()
        .flat_map(|(_, kinds)| kinds.iter())
        .find(|(pick, _)| *pick == ChartPick::Kind(kind))
        .map_or("chart.family.combo", |(_, label)| label)
}

/// The types a series of a combination can be drawn as, in the Edit
/// Series dialog's order.
pub const SERIES_KINDS: [ChartKind; 9] = [
    ChartKind::Column,
    ChartKind::StackedColumn,
    ChartKind::Bar,
    ChartKind::Line,
    ChartKind::LineWithMarkers,
    ChartKind::StackedLine,
    ChartKind::Area,
    ChartKind::StackedArea,
    ChartKind::Scatter,
];

/// Chart Design ▸ Legend's places, with their words; `None` is no legend.
pub const LEGENDS: [(Option<LegendPosition>, &str); 5] = [
    (None, "chart.legend.none"),
    (Some(LegendPosition::Right), "chart.legend.right"),
    (Some(LegendPosition::Top), "chart.legend.top"),
    (Some(LegendPosition::Left), "chart.legend.left"),
    (Some(LegendPosition::Bottom), "chart.legend.bottom"),
];

/// One series of a chart, as Select Data Source lists it.
#[derive(Debug, Clone, PartialEq)]
pub struct SeriesInfo {
    pub references: SeriesReferences,
    /// The name it shows: its cell's text, or "Series 2".
    pub name: String,
}

/// A chart as Chart Design shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct ChartInfo {
    /// `None` for a type Insert ▸ Chart does not make (3-D, stock…).
    pub kind: Option<ChartKind>,
    pub title: String,
    pub legend: Option<LegendPosition>,
    /// The range its series come from, on its own sheet: `A1:C4`.
    pub data: Option<String>,
    /// Its series, in their order.
    pub series: Vec<SeriesInfo>,
    /// The chart as the file has it: its axes, elements and formatting.
    pub parsed: Chart,
}

impl ChartInfo {
    /// Whether its series can be drawn as types of their own: a chart of
    /// columns, bars, lines or areas, or a combination already.
    pub fn combinable(&self) -> bool {
        self.kind.is_some_and(|kind| {
            kind == ChartKind::Combo
                || matches!(kind.element(), "barChart" | "lineChart" | "areaChart")
        })
    }
}

/// One of the eight handles on a selected object's frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

impl Handle {
    pub const ALL: [Handle; 8] = [
        Handle::TopLeft,
        Handle::Top,
        Handle::TopRight,
        Handle::Right,
        Handle::BottomRight,
        Handle::Bottom,
        Handle::BottomLeft,
        Handle::Left,
    ];

    /// Where on the frame it sits, as fractions of its width and height.
    pub fn at(self) -> (f64, f64) {
        match self {
            Handle::TopLeft => (0.0, 0.0),
            Handle::Top => (0.5, 0.0),
            Handle::TopRight => (1.0, 0.0),
            Handle::Right => (1.0, 0.5),
            Handle::BottomRight => (1.0, 1.0),
            Handle::Bottom => (0.5, 1.0),
            Handle::BottomLeft => (0.0, 1.0),
            Handle::Left => (0.0, 0.5),
        }
    }
}

/// How near a handle a press has to land, in px.
const HANDLE_REACH: f64 = 5.0;
/// The smallest an object is resized to, in px.
const SMALLEST: f64 = 8.0;

/// The handle of a frame `rect` (x, y, width, height) at a point.
pub fn handle_at(rect: (f64, f64, f64, f64), x: f64, y: f64) -> Option<Handle> {
    Handle::ALL.into_iter().find(|handle| {
        let (fx, fy) = handle.at();
        (rect.0 + rect.2 * fx - x).abs() <= HANDLE_REACH
            && (rect.1 + rect.3 * fy - y).abs() <= HANDLE_REACH
    })
}

/// An object's bounds (x, y, width, height) after a drag of `by`: on its
/// border it moves; on a handle that edge or corner follows the pointer,
/// the opposite one stays, and it never shrinks past the smallest.
pub fn dragged(
    bounds: (f64, f64, f64, f64),
    handle: Option<Handle>,
    by: (f64, f64),
) -> (f64, f64, f64, f64) {
    let (x, y, width, height) = bounds;
    let Some(handle) = handle else {
        return ((x + by.0).max(0.0), (y + by.1).max(0.0), width, height);
    };
    let (fx, fy) = handle.at();
    let (mut left, mut top, mut right, mut bottom) = (x, y, x + width, y + height);
    if fx == 0.0 {
        left = (left + by.0).min(right - SMALLEST).max(0.0);
    } else if fx == 1.0 {
        right = (right + by.0).max(left + SMALLEST);
    }
    if fy == 0.0 {
        top = (top + by.1).min(bottom - SMALLEST).max(0.0);
    } else if fy == 1.0 {
        bottom = (bottom + by.1).max(top + SMALLEST);
    }
    (left, top, right - left, bottom - top)
}

/// What Chart Design's text dialog asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartField {
    /// Chart Title: empty for none.
    Title,
    /// Select Data: the chart's range.
    Data,
}

impl ChartField {
    pub fn label_key(self) -> &'static str {
        match self {
            ChartField::Title => "chart.title",
            ChartField::Data => "chart.data",
        }
    }
}

/// Select Data Source: the chart's range, its series to pick from, and —
/// while one is added or edited — Edit Series. What it changes it changes
/// at once, as Excel's does; Cancel takes it all back.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectData {
    /// Chart data range.
    pub range: crate::text::TextEdit,
    /// The range as it was opened, so OK knows when it was retyped.
    opened_range: String,
    pub series: Vec<SeriesInfo>,
    pub selected: Option<usize>,
    /// Edit Series: the series it edits (`None` for Add) and its fields.
    pub editing: Option<(Option<usize>, Form)>,
    /// Whether a series can be drawn as a type of its own.
    pub combinable: bool,
    kind: Option<ChartKind>,
    /// How many changes the dialog made, for Cancel to undo.
    writes: usize,
}

/// What a chart dialog is showing.
#[derive(Debug, Clone, PartialEq)]
pub enum AskPage {
    /// Chart Title… or the range alone: one line of text.
    Text(ChartField, crate::text::TextEdit),
    /// Select Data Source.
    Data(SelectData),
    /// A Format dialog, Data Labels, Hidden and Empty Cells.
    Format(FormatPage),
}

/// Chart Elements' switches, and Chart Styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartElement {
    /// The value axis's major gridlines.
    Gridlines,
    MinorGridlines,
    /// A data table under the plot, with legend keys.
    DataTable,
    /// A chart style, 1 to 48.
    Style(u32),
}

/// A chart dialog, as a shell holds it open.
#[derive(Debug, Clone, PartialEq)]
pub struct ChartAsk {
    pub id: u32,
    pub page: AskPage,
}

/// What a key did to the dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartKey {
    Take,
    Close,
    Handled,
    Unhandled,
}

/// What a click (or Enter, or Escape) in a chart dialog does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AskAction {
    /// OK: of Edit Series while it is open, else of the dialog.
    Take,
    /// Cancel: of Edit Series while it is open, else of the dialog, taking
    /// back what it changed.
    Cancel,
    /// A series in the list picked.
    Pick(usize),
    Add,
    Edit,
    Remove,
    /// The picked series one up (`true`) or down the order.
    Move(bool),
    /// Switch Row/Column.
    Switch,
    /// The range field takes the typing.
    FocusRange,
    /// Edit Series' field at an index clicked.
    Field(usize),
    /// Edit Series' choice at an index set to an option.
    Choose(usize, usize),
}

/// What an action did: whether it wrote, and whether the dialog is done.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AskDone {
    pub wrote: bool,
    pub closed: bool,
}

/// A reference as typed — `=Sheet1!$B$2:$B$5`, `B2:B5`, `'My data'!B2` —
/// as a chart part writes it: sheet-qualified, absolute; on `sheet` when
/// it names none. `None` when it is no reference.
pub fn typed_reference(text: &str, sheet: &str) -> Option<String> {
    let text = text.trim().trim_start_matches('=').trim();
    let (on, at) = match text.rsplit_once('!') {
        Some((on, at)) => {
            let on = on.trim();
            let on = if on.len() >= 2 && on.starts_with('\'') && on.ends_with('\'') {
                on[1..on.len() - 1].replace("''", "'")
            } else {
                on.to_string()
            };
            (on, at)
        }
        None => (sheet.to_string(), text),
    };
    if on.is_empty() {
        return None;
    }
    let range = CellRange::parse(&at.replace('$', "")).ok()?;
    let absolute =
        |at: CellRef| format!("${}${}", crate::sheet::column_label(at.column), at.row + 1);
    let plain = on
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
        && !on.starts_with(|c: char| c.is_ascii_digit())
        && CellRef::parse(&on).is_err();
    let sheet = if plain {
        on
    } else {
        format!("'{}'", on.replace('\'', "''"))
    };
    Some(if range.start == range.end {
        format!("{sheet}!{}", absolute(range.start))
    } else {
        format!("{sheet}!{}:{}", absolute(range.start), absolute(range.end))
    })
}

/// A series' name as Edit Series shows it: `=Sheet1!$B$1`, or its text.
fn shown_name(name: Option<&str>) -> String {
    match name {
        None => String::new(),
        Some(name) => match name
            .strip_prefix('"')
            .and_then(|inner| inner.strip_suffix('"'))
        {
            Some(text) => text.replace("\"\"", "\""),
            None => format!("={name}"),
        },
    }
}

fn shown_reference(reference: Option<&str>) -> String {
    reference.map(|r| format!("={r}")).unwrap_or_default()
}

/// What Edit Series asks for, as typed.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SeriesEdit {
    /// A reference to the cell naming it, or a name of its own.
    pub name: String,
    pub values: String,
    /// Category labels, or a scatter's X values.
    pub categories: String,
    /// A bubble's sizes.
    pub sizes: String,
    /// In a combination, the type it is drawn as.
    pub kind: Option<ChartKind>,
    pub secondary: bool,
}

impl SelectData {
    fn new(info: &ChartInfo) -> Self {
        let range = info.data.clone().unwrap_or_default();
        Self {
            range: crate::text::TextEdit::selected(range.clone()),
            opened_range: range,
            series: info.series.clone(),
            selected: (!info.series.is_empty()).then_some(0),
            editing: None,
            combinable: info.combinable(),
            kind: info.kind,
            writes: 0,
        }
    }

    /// The chart as it is now, after a change.
    fn reload(&mut self, info: &ChartInfo) {
        self.series = info.series.clone();
        self.combinable = info.combinable();
        self.kind = info.kind;
        let range = info.data.clone().unwrap_or_default();
        self.range = crate::text::TextEdit::selected(range.clone());
        self.opened_range = range;
        self.selected = match self.selected {
            _ if self.series.is_empty() => None,
            Some(at) => Some(at.min(self.series.len() - 1)),
            None => Some(0),
        };
    }

    /// Edit Series for the series at `at`, or for a new one.
    fn edit_form(&self, at: Option<usize>) -> Form {
        let references = at.and_then(|at| self.series.get(at)).map(|s| &s.references);
        let xy = self.kind.is_some_and(ChartKind::is_xy);
        let mut form = Form::new(if at.is_some() {
            "chart.series.edit"
        } else {
            "chart.series.add"
        })
        .text(
            "chart.series.name",
            shown_name(references.and_then(|r| r.name.as_deref())),
        )
        .text(
            if xy {
                "chart.series.x_values"
            } else {
                "chart.series.categories"
            },
            shown_reference(references.and_then(|r| r.categories.as_deref())),
        )
        .text(
            if xy {
                "chart.series.y_values"
            } else {
                "chart.series.values"
            },
            shown_reference(references.map(|r| r.values.as_str())),
        );
        if self.kind == Some(ChartKind::Bubble) {
            form = form.text(
                "chart.series.sizes",
                shown_reference(references.and_then(|r| r.bubble_sizes.as_deref())),
            );
        }
        if self.combinable {
            let kind = references
                .and_then(|r| r.kind)
                .or(self.kind)
                .filter(|kind| *kind != ChartKind::Combo)
                .unwrap_or(ChartKind::Column);
            let chosen = SERIES_KINDS
                .iter()
                .position(|known| *known == kind)
                .unwrap_or(0);
            let labels: Vec<&'static str> = SERIES_KINDS.iter().map(|k| kind_label(*k)).collect();
            form = form.choice("chart.series.type", &labels, chosen).check(
                "chart.series.secondary",
                references.is_some_and(|r| r.secondary),
            );
        }
        // Typing starts in the values, as Excel's Edit Series has it.
        form.focus = 2;
        form
    }

    /// What Edit Series has typed.
    fn typed_edit(&self, form: &Form) -> SeriesEdit {
        let pick = |a: &str, b: &str| {
            if form.has(a) {
                form.text_of(a).to_string()
            } else {
                form.text_of(b).to_string()
            }
        };
        SeriesEdit {
            name: form.text_of("chart.series.name").to_string(),
            values: pick("chart.series.values", "chart.series.y_values"),
            categories: pick("chart.series.categories", "chart.series.x_values"),
            sizes: form.text_of("chart.series.sizes").to_string(),
            kind: form
                .has("chart.series.type")
                .then(|| SERIES_KINDS[form.chosen("chart.series.type")]),
            secondary: form.checked("chart.series.secondary"),
        }
    }
}

impl ChartAsk {
    /// The dialog for `field` of the chart `id`, filled in with what it has.
    pub fn new(id: u32, field: ChartField, info: &ChartInfo) -> Self {
        let text = match field {
            ChartField::Title => info.title.clone(),
            ChartField::Data => info.data.clone().unwrap_or_default(),
        };
        Self {
            id,
            page: AskPage::Text(field, crate::text::TextEdit::selected(text)),
        }
    }

    /// A Format dialog for `target` of the chart `id`; `None` for what
    /// the chart has not (a secondary axis, a series past its last).
    pub fn format(id: u32, target: FormatTarget, info: &ChartInfo) -> Option<Self> {
        Some(Self {
            id,
            page: AskPage::Format(FormatPage::new(target, info)?),
        })
    }

    /// Select Data Source, on the chart `id`.
    pub fn select_data(id: u32, info: &ChartInfo) -> Self {
        Self {
            id,
            page: AskPage::Data(SelectData::new(info)),
        }
    }

    /// The text typing goes into.
    pub fn typing(&mut self) -> Option<&mut crate::text::TextEdit> {
        match &mut self.page {
            AskPage::Text(_, text) => Some(text),
            AskPage::Format(page) => page.form.typing(),
            AskPage::Data(data) => match &mut data.editing {
                Some((_, form)) => form.typing(),
                None => Some(&mut data.range),
            },
        }
    }

    pub fn typed(&self) -> Option<&crate::text::TextEdit> {
        match &self.page {
            AskPage::Text(_, text) => Some(text),
            AskPage::Format(page) => page.form.typed(),
            AskPage::Data(data) => match &data.editing {
                Some((_, form)) => form.typed(),
                None => Some(&data.range),
            },
        }
    }

    /// Enter takes it, Escape leaves it; the rest edit the text.
    pub fn key(&mut self, key: &str, command: bool) -> ChartKey {
        let form = match &mut self.page {
            AskPage::Data(SelectData {
                editing: Some((_, form)),
                ..
            }) => Some(form),
            AskPage::Format(page) => Some(&mut page.form),
            _ => None,
        };
        if let Some(form) = form {
            return match form.key(key, command) {
                FormKey::Take => ChartKey::Take,
                FormKey::Close => ChartKey::Close,
                FormKey::Handled => ChartKey::Handled,
                FormKey::Unhandled => ChartKey::Unhandled,
            };
        }
        let Some(text) = self.typing() else {
            return ChartKey::Unhandled;
        };
        if text.marked.is_some() {
            return ChartKey::Unhandled;
        }
        match key {
            "enter" => return ChartKey::Take,
            "escape" => return ChartKey::Close,
            "a" if command => text.whole = true,
            key if text.edit_key(key) => {}
            _ => return ChartKey::Unhandled,
        }
        ChartKey::Handled
    }

    /// The change the text dialog's OK makes.
    pub fn change(&self) -> ChartChange {
        match &self.page {
            AskPage::Text(field, text) => {
                let text = text.text.trim();
                match field {
                    ChartField::Title => ChartChange {
                        title: Some((!text.is_empty()).then(|| text.to_string())),
                        ..ChartChange::default()
                    },
                    ChartField::Data => ChartChange {
                        data: Some(text.to_string()),
                        ..ChartChange::default()
                    },
                }
            }
            AskPage::Data(data) => ChartChange {
                data: Some(data.range.text.trim().to_string()),
                ..ChartChange::default()
            },
            AskPage::Format(page) => ChartChange {
                format: page.changes().unwrap_or_default(),
                ..ChartChange::default()
            },
        }
    }

    /// Does `action` on `book`: the chart changed as it says, the dialog
    /// then showing the chart as it is.
    pub fn act<B: Backend>(
        &mut self,
        book: &mut Workbook<B>,
        action: AskAction,
    ) -> Result<AskDone, String> {
        let id = self.id;
        let data = match &mut self.page {
            AskPage::Text(..) => {
                return match action {
                    AskAction::Take => {
                        let wrote = book.set_chart(id, &self.change())?;
                        Ok(AskDone {
                            wrote,
                            closed: true,
                        })
                    }
                    AskAction::Cancel => Ok(AskDone {
                        wrote: false,
                        closed: true,
                    }),
                    _ => Ok(AskDone::default()),
                };
            }
            AskPage::Format(page) => {
                return match action {
                    AskAction::Take => {
                        let format = page.changes()?;
                        let wrote = book.format_chart(id, &format)?;
                        Ok(AskDone {
                            wrote,
                            closed: true,
                        })
                    }
                    AskAction::Cancel => Ok(AskDone {
                        wrote: false,
                        closed: true,
                    }),
                    AskAction::Field(at) => {
                        page.form.click(at);
                        Ok(AskDone::default())
                    }
                    AskAction::Choose(at, option) => {
                        page.form.choose(at, option);
                        Ok(AskDone::default())
                    }
                    // ◀ ▶: the previous or next series or axis, what was
                    // typed for this one kept first.
                    AskAction::Move(back) => {
                        let format = page.changes()?;
                        let wrote = book.format_chart(id, &format)?;
                        let info = book.chart(id).ok_or("no such chart")?;
                        if let Some(next) = page.step(!back, &info) {
                            *page = next;
                        }
                        Ok(AskDone {
                            wrote,
                            closed: false,
                        })
                    }
                    _ => Ok(AskDone::default()),
                };
            }
            AskPage::Data(data) => data,
        };
        let mut wrote = false;
        let mut closed = false;
        match action {
            AskAction::Take => match data.editing.take() {
                Some((at, form)) => {
                    let edit = data.typed_edit(&form);
                    if let Err(error) = book.set_chart_series(id, at, &edit) {
                        data.editing = Some((at, form));
                        return Err(error);
                    }
                    wrote = true;
                    if at.is_none() {
                        data.selected = Some(data.series.len());
                    }
                }
                None => {
                    let typed = data.range.text.trim().to_string();
                    if typed != data.opened_range && !typed.is_empty() {
                        wrote = book.set_chart(
                            id,
                            &ChartChange {
                                data: Some(typed),
                                ..ChartChange::default()
                            },
                        )?;
                    }
                    closed = true;
                }
            },
            AskAction::Cancel => {
                if data.editing.take().is_none() {
                    for _ in 0..data.writes {
                        book.undo()?;
                    }
                    data.writes = 0;
                    return Ok(AskDone {
                        wrote: true,
                        closed: true,
                    });
                }
            }
            AskAction::Pick(at) => {
                if at < data.series.len() {
                    data.selected = Some(at);
                }
            }
            AskAction::Add => data.editing = Some((None, data.edit_form(None))),
            AskAction::Edit => {
                if let Some(at) = data.selected {
                    data.editing = Some((Some(at), data.edit_form(Some(at))));
                }
            }
            AskAction::Remove => {
                if let Some(at) = data.selected {
                    wrote = book.remove_chart_series(id, at)?;
                }
            }
            AskAction::Move(up) => {
                if let Some(at) = data.selected {
                    let to = if up { at.checked_sub(1) } else { Some(at + 1) };
                    if let Some(to) = to.filter(|to| *to < data.series.len()) {
                        wrote = book.move_chart_series(id, at, to)?;
                        data.selected = Some(to);
                    }
                }
            }
            AskAction::Switch => {
                wrote = book.set_chart(
                    id,
                    &ChartChange {
                        switch_rows: true,
                        ..ChartChange::default()
                    },
                )?;
            }
            AskAction::FocusRange => {}
            AskAction::Field(at) => {
                if let Some((_, form)) = &mut data.editing {
                    form.click(at);
                }
            }
            AskAction::Choose(at, option) => {
                if let Some((_, form)) = &mut data.editing {
                    form.choose(at, option);
                }
            }
        }
        if wrote {
            data.writes += 1;
            if let Some(info) = book.chart(id) {
                data.reload(&info);
            }
        }
        Ok(AskDone { wrote, closed })
    }
}

/// The series of a chart, `kind` deciding whether each still needs a type
/// of its own: when they are all drawn as one type, on one axis, the
/// chart is of that type; else a combination of each one's.
fn settled(
    kind: Option<ChartKind>,
    mut series: Vec<SeriesReferences>,
) -> (Option<ChartKind>, Vec<SeriesReferences>) {
    let base = kind.filter(|kind| *kind != ChartKind::Combo);
    let own = |one: &SeriesReferences| one.kind.or(base).unwrap_or(ChartKind::Column);
    let first = series.first().map(own);
    let one_type =
        series.iter().all(|one| Some(own(one)) == first) && series.iter().all(|one| !one.secondary);
    if one_type {
        let kind = first.or(base);
        for one in &mut series {
            one.kind = None;
        }
        return (kind.or(Some(ChartKind::Column)), series);
    }
    for one in &mut series {
        one.kind = Some(own(one));
    }
    (Some(ChartKind::Combo), series)
}

impl<B: Backend> Workbook<B> {
    /// Insert ▸ Chart over `range` as `pick` says; the new chart's id.
    pub fn insert_chart_as(&mut self, range: CellRange, pick: ChartPick) -> Result<u32, String> {
        match pick {
            ChartPick::Kind(kind) => self.insert_chart(range, kind),
            ChartPick::Combo(first, rest, secondary) => {
                self.insert_chart_with(range, |engine, sheet, range| {
                    engine.insert_combo_chart(sheet, range, first, rest, secondary)
                })
            }
        }
    }

    /// Change Chart Type: the chart `id` drawn as `pick` — a combination
    /// its first series one type, the rest another.
    pub fn set_chart_type(&mut self, id: u32, pick: ChartPick) -> Result<bool, String> {
        let change = match pick {
            ChartPick::Kind(kind) => ChartChange {
                kind: Some(kind),
                ..ChartChange::default()
            },
            ChartPick::Combo(first, rest, secondary) => {
                let info = self.chart(id).ok_or("no such chart")?;
                let series = info
                    .series
                    .into_iter()
                    .enumerate()
                    .map(|(at, series)| SeriesReferences {
                        kind: Some(if at == 0 { first } else { rest }),
                        secondary: at > 0 && secondary,
                        ..series.references
                    })
                    .collect();
                ChartChange {
                    kind: Some(ChartKind::Combo),
                    series: Some(series),
                    ..ChartChange::default()
                }
            }
        };
        self.set_chart(id, &change)
    }

    /// Edit Series' OK: the series at `at` — or, for `None`, a new one at
    /// the end — as `edit` has it.
    pub fn set_chart_series(
        &mut self,
        id: u32,
        at: Option<usize>,
        edit: &SeriesEdit,
    ) -> Result<bool, String> {
        let info = self.chart(id).ok_or("no such chart")?;
        let sheet = self.sheet_name();
        let values = typed_reference(&edit.values, &sheet)
            .ok_or_else(|| format!("not a reference: {}", edit.values))?;
        let optional = |text: &str| -> Result<Option<String>, String> {
            if text.trim().trim_start_matches('=').trim().is_empty() {
                return Ok(None);
            }
            typed_reference(text, &sheet)
                .map(Some)
                .ok_or_else(|| format!("not a reference: {text}"))
        };
        let categories = optional(&edit.categories)?;
        let name = {
            let typed = edit.name.trim();
            let bare = typed.trim_start_matches('=').trim();
            if bare.is_empty() {
                None
            } else if let Some(text) = bare
                .strip_prefix('"')
                .and_then(|inner| inner.strip_suffix('"'))
            {
                Some(format!("\"{text}\""))
            } else {
                Some(
                    typed_reference(bare, &sheet)
                        .unwrap_or_else(|| format!("\"{}\"", typed.replace('"', "\"\""))),
                )
            }
        };
        let text_categories = categories
            .as_deref()
            .is_some_and(|reference| self.reference_has_text(reference));
        let mut series: Vec<SeriesReferences> =
            info.series.into_iter().map(|one| one.references).collect();
        let edited = SeriesReferences {
            name,
            categories,
            text_categories,
            values,
            bubble_sizes: optional(&edit.sizes)?,
            kind: edit.kind,
            secondary: edit.secondary,
        };
        match at {
            Some(at) if at < series.len() => series[at] = edited,
            _ => series.push(edited),
        }
        let (kind, series) = settled(info.kind, series);
        self.set_chart(
            id,
            &ChartChange {
                kind,
                series: Some(series),
                ..ChartChange::default()
            },
        )
    }

    /// Select Data Source's Remove: the series at `at` gone; the last one
    /// stays.
    pub fn remove_chart_series(&mut self, id: u32, at: usize) -> Result<bool, String> {
        let info = self.chart(id).ok_or("no such chart")?;
        if info.series.len() < 2 || at >= info.series.len() {
            return Ok(false);
        }
        let mut series: Vec<SeriesReferences> =
            info.series.into_iter().map(|one| one.references).collect();
        series.remove(at);
        let (kind, series) = settled(info.kind, series);
        self.set_chart(
            id,
            &ChartChange {
                kind,
                series: Some(series),
                ..ChartChange::default()
            },
        )
    }

    /// Select Data Source's Move Up and Move Down: the series at `from`
    /// put at `to`.
    pub fn move_chart_series(&mut self, id: u32, from: usize, to: usize) -> Result<bool, String> {
        let info = self.chart(id).ok_or("no such chart")?;
        if from >= info.series.len() || to >= info.series.len() || from == to {
            return Ok(false);
        }
        let mut series: Vec<SeriesReferences> =
            info.series.into_iter().map(|one| one.references).collect();
        let moved = series.remove(from);
        series.insert(to, moved);
        self.set_chart(
            id,
            &ChartChange {
                series: Some(series),
                ..ChartChange::default()
            },
        )
    }

    /// The Format dialogs' and Chart Elements' changes to the chart `id`,
    /// one undo step; `false` for none.
    pub fn format_chart(&mut self, id: u32, format: &[ChartFormat]) -> Result<bool, String> {
        let real = format.iter().any(|change| match change {
            ChartFormat::Axis(_, axis) => *axis != AxisFormat::default(),
            ChartFormat::Series(_, series) => *series != SeriesFormat::default(),
            _ => true,
        });
        if !real {
            return Ok(false);
        }
        self.set_chart(
            id,
            &ChartChange {
                format: format.to_vec(),
                ..ChartChange::default()
            },
        )
    }

    /// Chart Elements: a switch flipped, or a chart style chosen.
    pub fn chart_element(&mut self, id: u32, element: ChartElement) -> Result<bool, String> {
        let info = self.chart(id).ok_or("no such chart")?;
        let axis = &info.parsed.value_axis;
        let change = match element {
            ChartElement::Gridlines => ChartFormat::Axis(
                AxisId::Value,
                AxisFormat {
                    major_gridlines: Some(!axis.gridlines),
                    ..AxisFormat::default()
                },
            ),
            ChartElement::MinorGridlines => ChartFormat::Axis(
                AxisId::Value,
                AxisFormat {
                    minor_gridlines: Some(!axis.minor_gridlines),
                    ..AxisFormat::default()
                },
            ),
            ChartElement::DataTable => ChartFormat::Table(
                info.parsed
                    .data_table
                    .is_none()
                    .then_some(TableFormat { keys: true }),
            ),
            ChartElement::Style(style) => {
                if info.parsed.style == Some(style) || !(1..=48).contains(&style) {
                    return Ok(false);
                }
                ChartFormat::Style(style)
            }
        };
        self.format_chart(id, &[change])
    }

    /// Whether any cell `reference` covers holds text.
    fn reference_has_text(&self, reference: &str) -> bool {
        let workbook = self.engine().workbook();
        let sheet = &workbook.sheets[self.sheet_index()];
        jade_xlsx_reader::chart::resolve_chart_reference(reference, sheet, Some(workbook))
            .is_some_and(|resolved| {
                resolved.points.iter().any(|point| {
                    point.cell.is_some_and(|cell| {
                        matches!(cell.value, jade_xlsx_reader::CellValue::Text(_))
                    })
                })
            })
    }
}

/// The series of `chart` with the names they show — `text` reading a
/// name's cell when the file keeps none — in their order.
pub(crate) fn series_infos(
    chart: &jade_xlsx_reader::chart::Chart,
    text: impl Fn(&str) -> Option<String>,
) -> Vec<SeriesInfo> {
    let mut all: Vec<&jade_xlsx_reader::chart::ChartSeries> = chart
        .series
        .iter()
        .chain(chart.extra_plots.iter().flat_map(|plot| plot.series.iter()))
        .filter(|series| series.values.formula.is_some())
        .collect();
    all.sort_by_key(|series| series.order);
    jade_xlsx_headless_editor::chart_series(chart)
        .into_iter()
        .zip(all)
        .enumerate()
        .map(|(at, (references, series))| SeriesInfo {
            references,
            name: series
                .name
                .points
                .first()
                .cloned()
                .flatten()
                .or_else(|| text(series.name.formula.as_deref()?))
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| format!("Series {}", at + 1)),
        })
        .collect()
}

// ---- Format dialogs: Format Axis, Format Data Series, Data Labels,
// Hidden and Empty Cells, Chart Area and Plot Area.

/// What a format dialog formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatTarget {
    Axis(AxisId),
    /// The series at an index in the chart's order.
    Series(usize),
    /// Chart Elements ▸ Data Labels, on every series.
    Labels,
    /// Hidden and Empty Cells.
    Blanks,
    /// The chart area (`false`) or the plot area (`true`).
    Area(bool),
}

/// What a format dialog says it formats, under its title.
#[derive(Debug, Clone, PartialEq)]
pub enum Subject {
    /// Words of the app's (an i18n key).
    Key(&'static str),
    /// A series' name.
    Text(String),
}

/// A format dialog: its form, and the form as it opened, so that OK
/// changes only what was changed.
#[derive(Debug, Clone, PartialEq)]
pub struct FormatPage {
    pub target: FormatTarget,
    pub form: Form,
    pub opened: Form,
    pub subject: Subject,
    /// Whether ◀ ▶ step to another series or axis.
    pub steps: bool,
}

pub const FILL_KINDS: [&str; 5] = [
    "chart.fill.automatic",
    "chart.fill.none",
    "chart.fill.solid",
    "chart.fill.gradient",
    "chart.fill.pattern",
];
pub const LINE_KINDS: [&str; 3] = [
    "chart.fill.automatic",
    "chart.fill.none",
    "chart.line.solid",
];
/// Dash types, as Format Data Series ▸ Line lists them, with the
/// presets they are saved as.
pub const DASHES: [(&str, &str); 6] = [
    ("solid", "chart.dash.solid"),
    ("sysDot", "chart.dash.round_dot"),
    ("sysDash", "chart.dash.square_dot"),
    ("dash", "chart.dash.dash"),
    ("dashDot", "chart.dash.dash_dot"),
    ("lgDash", "chart.dash.long_dash"),
];
pub const MARKERS: [(&str, &str); 11] = [
    ("", "chart.fill.automatic"),
    ("none", "chart.fill.none"),
    ("circle", "chart.marker.circle"),
    ("square", "chart.marker.square"),
    ("diamond", "chart.marker.diamond"),
    ("triangle", "chart.marker.triangle"),
    ("x", "chart.marker.x"),
    ("star", "chart.marker.star"),
    ("plus", "chart.marker.plus"),
    ("dash", "chart.marker.dash"),
    ("dot", "chart.marker.dot"),
];
pub const TRENDLINES: [(&str, &str); 7] = [
    ("", "chart.fill.none"),
    ("exp", "chart.trend.exponential"),
    ("linear", "chart.trend.linear"),
    ("log", "chart.trend.logarithmic"),
    ("poly", "chart.trend.polynomial"),
    ("power", "chart.trend.power"),
    ("movingAvg", "chart.trend.moving_average"),
];
pub const ERROR_AMOUNTS: [(&str, &str); 5] = [
    ("", "chart.fill.none"),
    ("fixedVal", "chart.error.fixed"),
    ("percentage", "chart.error.percentage"),
    ("stdDev", "chart.error.std_dev"),
    ("stdErr", "chart.error.std_err"),
];
pub const ERROR_DIRECTIONS: [(&str, &str); 3] = [
    ("both", "chart.error.both"),
    ("minus", "chart.error.minus"),
    ("plus", "chart.error.plus"),
];
pub const LABEL_POSITIONS: [(&str, &str); 10] = [
    ("", "chart.fill.automatic"),
    ("ctr", "chart.labels.center"),
    ("inEnd", "chart.labels.inside_end"),
    ("inBase", "chart.labels.inside_base"),
    ("outEnd", "chart.labels.outside_end"),
    ("t", "chart.labels.above"),
    ("b", "chart.labels.below"),
    ("l", "chart.labels.left"),
    ("r", "chart.labels.right"),
    ("bestFit", "chart.labels.best_fit"),
];
pub const CROSSINGS: [&str; 3] = [
    "chart.fill.automatic",
    "chart.axis.crosses_max",
    "chart.axis.crosses_value",
];
pub const AXIS_TYPES: [&str; 3] = ["chart.fill.automatic", "chart.axis.text", "chart.axis.date"];
pub const BLANKS: [(&str, &str); 3] = [
    ("gap", "chart.blanks.gaps"),
    ("zero", "chart.blanks.zero"),
    ("span", "chart.blanks.connect"),
];

fn keys<const N: usize>(list: &[(&str, &'static str); N]) -> Vec<&'static str> {
    list.iter().map(|(_, key)| *key).collect()
}

fn position_of<const N: usize>(list: &[(&str, &'static str); N], value: &str) -> usize {
    list.iter().position(|(v, _)| *v == value).unwrap_or(0)
}

fn hex(color: &jade_xlsx_reader::chart::ChartColor) -> String {
    color
        .rgb
        .map(|[r, g, b]| format!("{r:02X}{g:02X}{b:02X}"))
        .unwrap_or_default()
}

/// The fill choice and its two colours, as a dialog shows `fill`.
fn fill_fields(
    form: Form,
    fill: Option<&AreaFill>,
    color: &jade_xlsx_reader::chart::ChartColor,
) -> Form {
    let (kind, first, second) = match fill {
        None if color.rgb.is_some() => (2, hex(color), String::new()),
        None => (0, String::new(), String::new()),
        Some(AreaFill::None) => (1, String::new(), String::new()),
        Some(AreaFill::Solid(color)) => (2, hex(color), String::new()),
        Some(AreaFill::Gradient(stops, _)) => (
            3,
            stops.first().map(|(_, c)| hex(c)).unwrap_or_default(),
            stops.last().map(|(_, c)| hex(c)).unwrap_or_default(),
        ),
        Some(AreaFill::Pattern(_, fg, bg)) => (4, hex(fg), hex(bg)),
    };
    form.choice("chart.format.fill", &FILL_KINDS, kind)
        .text("chart.format.color", first)
        .text("chart.format.color2", second)
}

/// `RRGGBB` (a `#` before it allowed), or an error naming the field.
fn typed_color(text: &str, fallback: &str) -> Result<String, String> {
    let text = text.trim().trim_start_matches('#');
    let text = if text.is_empty() { fallback } else { text };
    if text.len() == 6 && text.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(text.to_ascii_uppercase())
    } else {
        Err(format!("{text} is not a colour (RRGGBB)"))
    }
}

/// The fill the form says, `None` for automatic.
fn typed_fill(form: &Form) -> Result<Option<Fill>, String> {
    let first = form.text_of("chart.format.color");
    let second = form.text_of("chart.format.color2");
    Ok(match form.chosen("chart.format.fill") {
        0 => None,
        1 => Some(Fill::None),
        2 => Some(Fill::Solid(typed_color(first, "4472C4")?)),
        3 => Some(Fill::Gradient(
            typed_color(first, "4472C4")?,
            typed_color(second, "FFFFFF")?,
            90,
        )),
        _ => Some(Fill::Pattern(
            "pct50".into(),
            typed_color(first, "4472C4")?,
            typed_color(second, "FFFFFF")?,
        )),
    })
}

/// A number typed in `key`: `None` when blank (automatic).
fn typed_number(form: &Form, key: &str) -> Result<Option<f64>, String> {
    let text = form.text_of(key);
    if text.is_empty() || text.eq_ignore_ascii_case("auto") {
        return Ok(None);
    }
    text.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .map(Some)
        .ok_or_else(|| format!("{text} is not a number"))
}

/// Whether any of `keys` was changed since the dialog opened.
fn changed(page: &FormatPage, keys: &[&str]) -> bool {
    keys.iter().any(|key| {
        let field = |form: &Form| {
            form.fields
                .iter()
                .find(|field| field.key == *key)
                .map(|field| match &field.value {
                    crate::chart_form::FieldValue::Text(text) => text.text.trim().to_string(),
                    crate::chart_form::FieldValue::Choice { chosen, .. } => chosen.to_string(),
                    crate::chart_form::FieldValue::Check(on) => on.to_string(),
                })
        };
        field(&page.form) != field(&page.opened)
    })
}

/// The chart's series in its order, each with the element of the plot
/// it is drawn in.
fn ordered_series(chart: &Chart) -> Vec<(&jade_xlsx_reader::chart::ChartSeries, &str)> {
    let mut all: Vec<_> = chart
        .series
        .iter()
        .map(|series| (series, chart.kind.as_str()))
        .chain(chart.extra_plots.iter().flat_map(|plot| {
            plot.series
                .iter()
                .map(move |series| (series, plot.kind.as_str()))
        }))
        .collect();
    all.sort_by_key(|(series, _)| series.order);
    all
}

impl FormatPage {
    /// The dialog for `target` of `info`'s chart.
    pub fn new(target: FormatTarget, info: &ChartInfo) -> Option<Self> {
        let chart = &info.parsed;
        let xy = matches!(chart.kind.as_str(), "scatterChart" | "bubbleChart");
        let (form, subject, steps) = match target {
            FormatTarget::Axis(id) => {
                let axis = match id {
                    AxisId::Category => Some(&chart.category_axis),
                    AxisId::Value => Some(&chart.value_axis),
                    AxisId::SecondaryValue => chart.secondary_value_axis.as_ref(),
                    AxisId::SecondaryCategory => chart.secondary_category_axis.as_ref(),
                }?;
                let other = match id {
                    AxisId::Category => Some(&chart.value_axis),
                    AxisId::Value => Some(&chart.category_axis),
                    AxisId::SecondaryValue => chart.secondary_category_axis.as_ref(),
                    AxisId::SecondaryCategory => chart.secondary_value_axis.as_ref(),
                };
                let values = xy || matches!(id, AxisId::Value | AxisId::SecondaryValue);
                let text = |value: &Option<String>| value.clone().unwrap_or_default();
                let mut form = Form::new("chart.format_axis")
                    .text("chart.axis.title", text(&axis.title))
                    .check("chart.axis.visible", !axis.hidden);
                if values {
                    form = form
                        .text("chart.axis.minimum", text(&axis.minimum))
                        .text("chart.axis.maximum", text(&axis.maximum))
                        .text("chart.axis.major_unit", text(&axis.major_unit))
                        .text("chart.axis.minor_unit", text(&axis.minor_unit))
                        .check("chart.axis.log", axis.log_base.is_some())
                        .text(
                            "chart.axis.log_base",
                            axis.log_base.clone().unwrap_or_else(|| "10".into()),
                        );
                } else {
                    form = form.choice(
                        "chart.axis.type",
                        &AXIS_TYPES,
                        if axis.date { 2 } else { 0 },
                    );
                }
                // Where the other axis crosses this one, as that axis says.
                let (crosses, at) = match other {
                    Some(other) if other.crosses_at.is_some() => {
                        (2, other.crosses_at.clone().unwrap_or_default())
                    }
                    Some(other) if other.crosses.as_deref() == Some("max") => (1, String::new()),
                    _ => (0, String::new()),
                };
                form = form
                    .choice("chart.axis.crosses", &CROSSINGS, crosses)
                    .text("chart.axis.crosses_at", at)
                    .check("chart.axis.reverse", axis.reverse)
                    .text("chart.axis.number_format", text(&axis.number_format))
                    .check("chart.axis.major_gridlines", axis.gridlines)
                    .check("chart.axis.minor_gridlines", axis.minor_gridlines);
                let name = match id {
                    AxisId::Category if xy => "chart.axis.horizontal_value",
                    AxisId::Category => "chart.axis.horizontal",
                    AxisId::Value => "chart.axis.vertical",
                    AxisId::SecondaryValue => "chart.axis.secondary_vertical",
                    AxisId::SecondaryCategory => "chart.axis.secondary_horizontal",
                };
                (form, Subject::Key(name), true)
            }
            FormatTarget::Series(at) => {
                let all = ordered_series(chart);
                let (series, plot) = all.get(at).copied()?;
                let name = info
                    .series
                    .get(at)
                    .map(|series| series.name.clone())
                    .unwrap_or_else(|| format!("Series {}", at + 1));
                let mut form = fill_fields(
                    Form::new("chart.format_series"),
                    series.fill.as_ref(),
                    &series.color,
                );
                let line_kind = if series.line.none {
                    1
                } else if series.line.width.is_some() || series.line.dash.is_some() {
                    2
                } else {
                    0
                };
                form = form
                    .choice("chart.format.line", &LINE_KINDS, line_kind)
                    .text(
                        "chart.format.line_width",
                        series
                            .line
                            .width
                            .map(|emu| {
                                format!("{}", (emu as f64 / 12_700.0 * 100.0).round() / 100.0)
                            })
                            .unwrap_or_default(),
                    )
                    .choice(
                        "chart.format.dash",
                        &keys(&DASHES),
                        position_of(&DASHES, series.line.dash.as_deref().unwrap_or("solid")),
                    );
                if matches!(plot, "lineChart" | "scatterChart" | "radarChart") {
                    form = form
                        .choice("chart.format.marker", &keys(&MARKERS), 0)
                        .text("chart.format.marker_size", series.marker_size.to_string())
                        .check("chart.format.smooth", series.smooth);
                }
                if plot == "barChart" {
                    let (gap, overlap) = if chart.kind == "barChart" {
                        (chart.gap_width, chart.overlap)
                    } else {
                        chart
                            .extra_plots
                            .iter()
                            .find(|p| p.kind == "barChart")
                            .map_or((150, 0), |p| (p.gap_width, p.overlap))
                    };
                    form = form
                        .text("chart.format.gap_width", gap.to_string())
                        .text("chart.format.overlap", overlap.to_string());
                }
                let labels = series.data_labels.as_ref();
                form = form
                    .check("chart.labels.value", labels.is_some_and(|l| l.show_value))
                    .check(
                        "chart.labels.category",
                        labels.is_some_and(|l| l.show_category),
                    )
                    .check("chart.labels.series", labels.is_some_and(|l| l.show_series));
                let trend = series.trendlines.first();
                form = form
                    .choice(
                        "chart.trend.type",
                        &keys(&TRENDLINES),
                        trend.map_or(0, |t| position_of(&TRENDLINES, &t.kind)),
                    )
                    .text(
                        "chart.trend.order",
                        trend
                            .map(|t| {
                                if t.kind == "movingAvg" {
                                    t.period
                                } else {
                                    t.order
                                }
                            })
                            .filter(|n| *n > 0)
                            .unwrap_or(2)
                            .to_string(),
                    )
                    .text(
                        "chart.trend.forward",
                        trend.and_then(|t| t.forward.clone()).unwrap_or_default(),
                    )
                    .text(
                        "chart.trend.backward",
                        trend.and_then(|t| t.backward.clone()).unwrap_or_default(),
                    )
                    .check(
                        "chart.trend.equation",
                        trend.is_some_and(|t| t.display_equation),
                    )
                    .check(
                        "chart.trend.r_squared",
                        trend.is_some_and(|t| t.display_r_squared),
                    );
                let bars = series.error_bars.first();
                form = form
                    .choice(
                        "chart.error.amount",
                        &keys(&ERROR_AMOUNTS),
                        bars.map_or(0, |b| position_of(&ERROR_AMOUNTS, &b.value_type)),
                    )
                    .text(
                        "chart.error.value",
                        bars.and_then(|b| b.value.clone())
                            .unwrap_or_else(|| "5".into()),
                    )
                    .choice(
                        "chart.error.direction",
                        &keys(&ERROR_DIRECTIONS),
                        bars.map_or(0, |b| position_of(&ERROR_DIRECTIONS, &b.bar_type)),
                    )
                    .check("chart.error.cap", bars.is_none_or(|b| !b.no_end_cap));
                (form, Subject::Text(name), all.len() > 1)
            }
            FormatTarget::Labels => {
                let labels = chart.data_labels.as_ref().or_else(|| {
                    chart
                        .series
                        .first()
                        .and_then(|series| series.data_labels.as_ref())
                });
                let round = matches!(chart.kind.as_str(), "pieChart" | "doughnutChart");
                let mut form = Form::new("chart.data_labels")
                    .check("chart.labels.value", labels.is_some_and(|l| l.show_value))
                    .check(
                        "chart.labels.category",
                        labels.is_some_and(|l| l.show_category),
                    )
                    .check("chart.labels.series", labels.is_some_and(|l| l.show_series));
                if round {
                    form = form.check(
                        "chart.labels.percent",
                        labels.is_some_and(|l| l.show_percent),
                    );
                }
                form = form.choice(
                    "chart.labels.position",
                    &keys(&LABEL_POSITIONS),
                    labels
                        .and_then(|l| l.position.as_deref())
                        .map_or(0, |p| position_of(&LABEL_POSITIONS, p)),
                );
                (form, Subject::Key("chart.data_labels.all"), false)
            }
            FormatTarget::Blanks => {
                let form = Form::new("chart.blanks")
                    .choice(
                        "chart.blanks.show_as",
                        &keys(&BLANKS),
                        position_of(&BLANKS, &chart.blank_mode),
                    )
                    .check("chart.blanks.hidden", !chart.visible_only);
                (form, Subject::Key("chart.blanks.subject"), false)
            }
            FormatTarget::Area(plot) => {
                let fill = if plot {
                    &chart.plot_fill
                } else {
                    &chart.chart_fill
                };
                let form = fill_fields(
                    Form::new(if plot {
                        "chart.format_plot_area"
                    } else {
                        "chart.format_chart_area"
                    }),
                    fill.as_ref(),
                    &Default::default(),
                );
                let subject = if plot {
                    "chart.plot_area"
                } else {
                    "chart.chart_area"
                };
                (form, Subject::Key(subject), false)
            }
        };
        let mut form = form;
        if !form.has("chart.axis.minimum") && form.focus == usize::MAX {
            form.focus = 0;
        }
        Some(Self {
            target,
            opened: form.clone(),
            form,
            subject,
            steps,
        })
    }

    /// The changes OK makes: what was changed since the dialog opened.
    pub fn changes(&self) -> Result<Vec<ChartFormat>, String> {
        let form = &self.form;
        let mut out = Vec::new();
        match self.target {
            FormatTarget::Axis(id) => {
                let mut format = AxisFormat::default();
                if changed(self, &["chart.axis.title"]) {
                    let title = form.text_of("chart.axis.title");
                    format.title = Some((!title.is_empty()).then(|| title.to_string()));
                }
                if changed(self, &["chart.axis.visible"]) {
                    format.visible = Some(form.checked("chart.axis.visible"));
                }
                for (key, slot) in [
                    ("chart.axis.minimum", &mut format.minimum),
                    ("chart.axis.maximum", &mut format.maximum),
                    ("chart.axis.major_unit", &mut format.major_unit),
                    ("chart.axis.minor_unit", &mut format.minor_unit),
                ] {
                    if changed(self, &[key]) {
                        *slot = Some(typed_number(form, key)?);
                    }
                }
                // The bounds as the dialog shows them, changed or not.
                let bounds = (
                    typed_number(form, "chart.axis.minimum")?,
                    typed_number(form, "chart.axis.maximum")?,
                );
                if let (Some(min), Some(max)) = bounds {
                    if min >= max {
                        return Err("the minimum must be less than the maximum".into());
                    }
                }
                if changed(self, &["chart.axis.log", "chart.axis.log_base"]) {
                    format.log_base = Some(if form.checked("chart.axis.log") {
                        let base = typed_number(form, "chart.axis.log_base")?.unwrap_or(10.0);
                        if !(2.0..=1000.0).contains(&base) {
                            return Err("a logarithmic base is from 2 to 1000".into());
                        }
                        Some(base)
                    } else {
                        None
                    });
                }
                if changed(self, &["chart.axis.reverse"]) {
                    format.reverse = Some(form.checked("chart.axis.reverse"));
                }
                if changed(self, &["chart.axis.number_format"]) {
                    let code = form.text_of("chart.axis.number_format");
                    format.number_format = Some((!code.is_empty()).then(|| code.to_string()));
                }
                if changed(self, &["chart.axis.major_gridlines"]) {
                    format.major_gridlines = Some(form.checked("chart.axis.major_gridlines"));
                }
                if changed(self, &["chart.axis.minor_gridlines"]) {
                    format.minor_gridlines = Some(form.checked("chart.axis.minor_gridlines"));
                }
                if changed(self, &["chart.axis.type"]) {
                    format.kind = Some(match form.chosen("chart.axis.type") {
                        0 => AxisKind::Automatic,
                        1 => AxisKind::Text,
                        _ => AxisKind::Date,
                    });
                }
                out.push(ChartFormat::Axis(id, format));
                if changed(self, &["chart.axis.crosses", "chart.axis.crosses_at"]) {
                    let crosses = match form.chosen("chart.axis.crosses") {
                        0 => Crosses::AutoZero,
                        1 => Crosses::Maximum,
                        _ => Crosses::At(
                            typed_number(form, "chart.axis.crosses_at")?
                                .ok_or("type the value the other axis crosses at")?,
                        ),
                    };
                    let other = match id {
                        AxisId::Category => AxisId::Value,
                        AxisId::Value => AxisId::Category,
                        AxisId::SecondaryValue => AxisId::SecondaryCategory,
                        AxisId::SecondaryCategory => AxisId::SecondaryValue,
                    };
                    out.push(ChartFormat::Axis(
                        other,
                        AxisFormat {
                            crosses: Some(crosses),
                            ..AxisFormat::default()
                        },
                    ));
                }
            }
            FormatTarget::Series(at) => {
                let mut format = SeriesFormat::default();
                if changed(
                    self,
                    &[
                        "chart.format.fill",
                        "chart.format.color",
                        "chart.format.color2",
                    ],
                ) {
                    format.fill = Some(typed_fill(form)?);
                }
                if changed(self, &["chart.format.line"]) {
                    format.line_color = Some(match form.chosen("chart.format.line") {
                        0 => None,
                        1 => Some(None),
                        _ => Some(Some(typed_color(
                            form.text_of("chart.format.color"),
                            "4472C4",
                        )?)),
                    });
                }
                if changed(self, &["chart.format.line_width"]) {
                    if let Some(width) = typed_number(form, "chart.format.line_width")? {
                        if !(0.0..=1584.0).contains(&width) {
                            return Err("a line's width is from 0 to 1584 points".into());
                        }
                        format.line_width = Some(width);
                    }
                }
                if changed(self, &["chart.format.dash"]) {
                    format.line_dash = Some(
                        DASHES[form.chosen("chart.format.dash").min(DASHES.len() - 1)]
                            .0
                            .into(),
                    );
                }
                if changed(self, &["chart.format.marker", "chart.format.marker_size"]) {
                    let symbol =
                        MARKERS[form.chosen("chart.format.marker").min(MARKERS.len() - 1)].0;
                    let size = typed_number(form, "chart.format.marker_size")?.unwrap_or(5.0);
                    if !(2.0..=72.0).contains(&size) {
                        return Err("a marker's size is from 2 to 72".into());
                    }
                    if !symbol.is_empty() {
                        format.marker = Some((symbol.into(), size as u32));
                    }
                }
                if changed(self, &["chart.format.smooth"]) {
                    format.smooth = Some(form.checked("chart.format.smooth"));
                }
                if changed(self, &["chart.format.gap_width"]) {
                    let gap = typed_number(form, "chart.format.gap_width")?.unwrap_or(150.0);
                    if !(0.0..=500.0).contains(&gap) {
                        return Err("the gap width is from 0% to 500%".into());
                    }
                    format.gap_width = Some(gap as u32);
                }
                if changed(self, &["chart.format.overlap"]) {
                    let overlap = typed_number(form, "chart.format.overlap")?.unwrap_or(0.0);
                    if !(-100.0..=100.0).contains(&overlap) {
                        return Err("the overlap is from -100% to 100%".into());
                    }
                    format.overlap = Some(overlap as i32);
                }
                if changed(
                    self,
                    &[
                        "chart.labels.value",
                        "chart.labels.category",
                        "chart.labels.series",
                    ],
                ) {
                    let labels = LabelFormat {
                        value: form.checked("chart.labels.value"),
                        category: form.checked("chart.labels.category"),
                        series: form.checked("chart.labels.series"),
                        ..LabelFormat::default()
                    };
                    format.labels =
                        Some((labels.value || labels.category || labels.series).then_some(labels));
                }
                if changed(
                    self,
                    &[
                        "chart.trend.type",
                        "chart.trend.order",
                        "chart.trend.forward",
                        "chart.trend.backward",
                        "chart.trend.equation",
                        "chart.trend.r_squared",
                    ],
                ) {
                    let kind =
                        TRENDLINES[form.chosen("chart.trend.type").min(TRENDLINES.len() - 1)].0;
                    format.trendline = Some(if kind.is_empty() {
                        None
                    } else {
                        let n = typed_number(form, "chart.trend.order")?.unwrap_or(2.0);
                        if kind == "poly" && !(2.0..=6.0).contains(&n) {
                            return Err("a polynomial's order is from 2 to 6".into());
                        }
                        if kind == "movingAvg" && !(2.0..=255.0).contains(&n) {
                            return Err("a moving average's period is from 2 to 255".into());
                        }
                        Some(TrendlineFormat {
                            kind: kind.into(),
                            order: n as u32,
                            period: n as u32,
                            forward: typed_number(form, "chart.trend.forward")?,
                            backward: typed_number(form, "chart.trend.backward")?,
                            display_equation: form.checked("chart.trend.equation"),
                            display_r_squared: form.checked("chart.trend.r_squared"),
                        })
                    });
                }
                if changed(
                    self,
                    &[
                        "chart.error.amount",
                        "chart.error.value",
                        "chart.error.direction",
                        "chart.error.cap",
                    ],
                ) {
                    let value_type = ERROR_AMOUNTS[form
                        .chosen("chart.error.amount")
                        .min(ERROR_AMOUNTS.len() - 1)]
                    .0;
                    format.error_bars = Some(if value_type.is_empty() {
                        None
                    } else {
                        let value = typed_number(form, "chart.error.value")?.unwrap_or(1.0);
                        if value < 0.0 {
                            return Err("an error amount is not negative".into());
                        }
                        Some(ErrorBarFormat {
                            direction: "y".into(),
                            bar_type: ERROR_DIRECTIONS[form
                                .chosen("chart.error.direction")
                                .min(ERROR_DIRECTIONS.len() - 1)]
                            .0
                            .into(),
                            value_type: value_type.into(),
                            value,
                            end_cap: form.checked("chart.error.cap"),
                        })
                    });
                }
                out.push(ChartFormat::Series(at, format));
            }
            FormatTarget::Labels => {
                let labels = LabelFormat {
                    value: form.checked("chart.labels.value"),
                    category: form.checked("chart.labels.category"),
                    series: form.checked("chart.labels.series"),
                    percent: form.checked("chart.labels.percent"),
                    position: Some(
                        LABEL_POSITIONS[form
                            .chosen("chart.labels.position")
                            .min(LABEL_POSITIONS.len() - 1)]
                        .0
                        .to_string(),
                    )
                    .filter(|p| !p.is_empty()),
                    separator: None,
                };
                let any = labels.value || labels.category || labels.series || labels.percent;
                out.push(ChartFormat::Labels(any.then_some(labels)));
            }
            FormatTarget::Blanks => {
                out.push(ChartFormat::Blanks(
                    BLANKS[form.chosen("chart.blanks.show_as").min(BLANKS.len() - 1)]
                        .0
                        .into(),
                    form.checked("chart.blanks.hidden"),
                ));
            }
            FormatTarget::Area(plot) => {
                out.push(ChartFormat::AreaFill(plot, typed_fill(form)?));
            }
        }
        Ok(out)
    }

    /// The next (`true`) or previous series or axis, if there is one.
    fn step(&self, forward: bool, info: &ChartInfo) -> Option<Self> {
        match self.target {
            FormatTarget::Series(at) => {
                let count = info.series.len();
                let next = if forward {
                    (at + 1) % count
                } else {
                    (at + count - 1) % count
                };
                Self::new(FormatTarget::Series(next), info)
            }
            FormatTarget::Axis(id) => {
                let order = [AxisId::Value, AxisId::Category, AxisId::SecondaryValue];
                let from = order.iter().position(|a| *a == id).unwrap_or(0);
                (1..order.len()).find_map(|k| {
                    let i = if forward {
                        (from + k) % order.len()
                    } else {
                        (from + order.len() - k) % order.len()
                    };
                    Self::new(FormatTarget::Axis(order[i]), info)
                })
            }
            _ => None,
        }
    }
}

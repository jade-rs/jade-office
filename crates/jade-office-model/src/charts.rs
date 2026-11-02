//! Charts and other drawing objects on a sheet, as both shells work them:
//! Insert ▸ Chart's types, the handles of a selected object and what a
//! drag on its border or a handle does to it, a chart as Chart Design
//! shows it, and its dialogs — Chart Title, Select Data Source and its Edit
//! Series.

pub use jade_xlsx_headless_editor::{ChartChange, ChartKind, LegendPosition, SeriesReferences};

use crate::form::{Form, FormKey};
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
    let absolute = |at: CellRef| format!("${}${}", crate::sheet::column_label(at.column), at.row + 1);
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
            form = form
                .choice("chart.series.type", &labels, chosen)
                .check(
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
            AskPage::Data(data) => match &mut data.editing {
                Some((_, form)) => form.typing(),
                None => Some(&mut data.range),
            },
        }
    }

    pub fn typed(&self) -> Option<&crate::text::TextEdit> {
        match &self.page {
            AskPage::Text(_, text) => Some(text),
            AskPage::Data(data) => match &data.editing {
                Some((_, form)) => form.typed(),
                None => Some(&data.range),
            },
        }
    }

    /// Enter takes it, Escape leaves it; the rest edit the text.
    pub fn key(&mut self, key: &str, command: bool) -> ChartKey {
        if let AskPage::Data(SelectData {
            editing: Some((_, form)),
            ..
        }) = &mut self.page
        {
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
                Some(typed_reference(bare, &sheet).unwrap_or_else(|| {
                    format!("\"{}\"", typed.replace('"', "\"\""))
                }))
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

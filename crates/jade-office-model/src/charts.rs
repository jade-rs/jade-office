//! Charts and other drawing objects on a sheet, as both shells work them:
//! Insert ▸ Chart's types, the handles of a selected object and what a
//! drag on its border or a handle does to it, and a chart as Chart Design
//! shows it.

pub use jade_xlsx_headless_editor::{ChartChange, ChartKind, LegendPosition};

/// Insert ▸ Chart's types, in its order, with their words.
pub const CHART_KINDS: [(ChartKind, &str); 10] = [
    (ChartKind::Column, "chart.kind.column"),
    (ChartKind::StackedColumn, "chart.kind.stacked_column"),
    (ChartKind::Bar, "chart.kind.bar"),
    (ChartKind::StackedBar, "chart.kind.stacked_bar"),
    (ChartKind::Line, "chart.kind.line"),
    (ChartKind::LineWithMarkers, "chart.kind.line_markers"),
    (ChartKind::Area, "chart.kind.area"),
    (ChartKind::Pie, "chart.kind.pie"),
    (ChartKind::Doughnut, "chart.kind.doughnut"),
    (ChartKind::Scatter, "chart.kind.scatter"),
];

/// Chart Design ▸ Legend's places, with their words; `None` is no legend.
pub const LEGENDS: [(Option<LegendPosition>, &str); 5] = [
    (None, "chart.legend.none"),
    (Some(LegendPosition::Right), "chart.legend.right"),
    (Some(LegendPosition::Top), "chart.legend.top"),
    (Some(LegendPosition::Left), "chart.legend.left"),
    (Some(LegendPosition::Bottom), "chart.legend.bottom"),
];

/// A chart as Chart Design shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct ChartInfo {
    /// `None` for a type Insert ▸ Chart does not make (3-D, radar…).
    pub kind: Option<ChartKind>,
    pub title: String,
    pub legend: Option<LegendPosition>,
    /// The range its series come from, on its own sheet: `A1:C4`.
    pub data: Option<String>,
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

/// Chart Design's text dialog, as a shell holds it open.
pub struct ChartAsk {
    pub id: u32,
    pub field: ChartField,
    pub text: crate::text::TextEdit,
}

/// What a key did to the dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartKey {
    Take,
    Close,
    Handled,
    Unhandled,
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
            field,
            text: crate::text::TextEdit::selected(text),
        }
    }

    /// Enter takes it, Escape leaves it; the rest edit the text.
    pub fn key(&mut self, key: &str, command: bool) -> ChartKey {
        if self.text.marked.is_some() {
            return ChartKey::Unhandled;
        }
        let text = &mut self.text;
        match key {
            "enter" => return ChartKey::Take,
            "escape" => return ChartKey::Close,
            "backspace" => text.backspace(),
            "delete" => text.delete(),
            "left" => text.step(false),
            "right" => text.step(true),
            "home" | "up" => text.to_end(false),
            "end" | "down" => text.to_end(true),
            "a" if command => text.whole = true,
            _ => return ChartKey::Unhandled,
        }
        ChartKey::Handled
    }

    /// The change the dialog's OK makes.
    pub fn change(&self) -> ChartChange {
        let text = self.text.text.trim();
        match self.field {
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
}

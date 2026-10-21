//! 挿入 → SmartArt and グラフ, and their contextual tools in the native app:
//! SmartArt's layout gallery and its text pane (the outline of its points,
//! typed into, promoted and demoted), a chart's type, title, legend and its
//! data edited in a sheet.

use gpui::{div, prelude::*, px, AnyElement, Context, SharedString};
use jade_docx_editor::document::{ChartKind, ChartSeries, RunChart};
use jade_docx_editor::smartart::{self, Diagram};
use jade_docx_editor::word_document::FloatingObjectTarget;
use jade_office_model::text::TextEdit;
use jade_office_model::word::diagrams::WordDiagrams;
use jade_office_model::word::objects::ObjectSort;

use crate::i18n::{self, text as t};
use crate::ribbon::{dropdown, group, listener, menu_caption, menu_panel, menu_row, row, Action, Popup};
use crate::word_objects_ribbon::{ObjectAction, ObjectPopup};
use crate::tooltip::Tooltip;
use crate::{theme, DocumentView};

/// The diagram tools' lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagramPopup {
    InsertSmartArt,
    InsertChart,
    Layouts,
    ChartType,
    Legend,
}

/// What a diagram control does.
#[derive(Clone, Debug, PartialEq)]
pub enum DiagramAction {
    InsertSmartArt(&'static str),
    InsertChart(ChartKind),
    ChangeLayout(&'static str),
    TextPane,
    ChartType(ChartKind),
    Legend(Option<&'static str>),
    ChartTitle(bool),
    EditData,
}

/// SmartArt's text pane: the outline of its points.
pub struct TextPane {
    pub target: FloatingObjectTarget,
    pub layout: String,
    pub lines: Vec<(usize, TextEdit)>,
    pub focus: Option<usize>,
}

/// データの編集: the chart's data as a sheet.
pub struct ChartSheet {
    pub target: FloatingObjectTarget,
    pub chart: RunChart,
    /// Row 0 holds the series names, column 0 the categories.
    pub cells: Vec<Vec<TextEdit>>,
    pub focus: Option<(usize, usize)>,
}

/// The diagram tools' state.
#[derive(Default)]
pub struct DiagramTools {
    pub text_pane: Option<TextPane>,
    pub chart_sheet: Option<ChartSheet>,
}

impl DiagramTools {
    /// The field typing goes into, if one of these has it.
    pub fn focused_mut(&mut self) -> Option<&mut TextEdit> {
        if let Some(pane) = self.text_pane.as_mut() {
            if let Some(index) = pane.focus {
                return pane.lines.get_mut(index).map(|(_, edit)| edit);
            }
        }
        if let Some(sheet) = self.chart_sheet.as_mut() {
            if let Some((row, column)) = sheet.focus {
                return sheet.cells.get_mut(row).and_then(|cells| cells.get_mut(column));
            }
        }
        None
    }

    pub fn focused(&self) -> Option<&TextEdit> {
        if let Some(pane) = self.text_pane.as_ref() {
            if let Some(index) = pane.focus {
                return pane.lines.get(index).map(|(_, edit)| edit);
            }
        }
        if let Some(sheet) = self.chart_sheet.as_ref() {
            if let Some((row, column)) = sheet.focus {
                return sheet.cells.get(row).and_then(|cells| cells.get(column));
            }
        }
        None
    }
}

fn japanese() -> bool {
    i18n::language() == i18n::Language::Japanese
}

fn act(action: DiagramAction) -> Action {
    Action::Object(ObjectAction::Diagram(action))
}

fn list_button(
    id: &'static str,
    label: &'static str,
    open: bool,
    which: DiagramPopup,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap_1()
        .h(px(26.0))
        .px_2()
        .rounded_md()
        .text_xs()
        .text_color(theme::text())
        .whitespace_nowrap()
        .cursor_pointer()
        .when(open, |button| button.bg(theme::selected()))
        .hover(|button| button.bg(theme::hovered()))
        .on_click(listener(Action::Popup(Popup::Object(ObjectPopup::Diagram(which))), cx))
        .child(label)
        .child(
            gpui::svg()
                .path("icons/chevron-down.svg")
                .size_3()
                .text_color(theme::text_dim()),
        )
        .into_any_element()
}

fn text_button(
    id: &'static str,
    label: &'static str,
    tooltip: &'static str,
    action: DiagramAction,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .h(px(26.0))
        .px_2()
        .rounded_md()
        .text_xs()
        .text_color(theme::text())
        .whitespace_nowrap()
        .cursor_pointer()
        .hover(|button| button.bg(theme::hovered()))
        .tooltip(Tooltip::text(tooltip))
        .on_click(listener(act(action), cx))
        .child(label)
        .into_any_element()
}

pub fn chart_kind_label(kind: ChartKind) -> &'static str {
    t(match kind {
        ChartKind::Bar => "objects.chart.column",
        ChartKind::Line => "objects.chart.line",
        ChartKind::Pie => "objects.chart.pie",
    })
}

const CHART_KINDS: [ChartKind; 3] = [ChartKind::Bar, ChartKind::Line, ChartKind::Pie];

impl DocumentView {
    /// 挿入's SmartArt and グラフ buttons.
    pub(crate) fn insert_diagram_buttons(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let open = self.ribbon.popup;
        let smartart_menu = (open == Some(Popup::Object(ObjectPopup::Diagram(DiagramPopup::InsertSmartArt)))).then(|| {
            layout_menu("insert-smartart-menu", true, cx)
        });
        let chart_menu = (open == Some(Popup::Object(ObjectPopup::Diagram(DiagramPopup::InsertChart)))).then(|| {
            menu_panel("insert-chart-menu")
                .w(px(180.0))
                .children(CHART_KINDS.into_iter().enumerate().map(|(index, kind)| {
                    menu_row(("insert-chart", index), chart_kind_label(kind), false, act(DiagramAction::InsertChart(kind)), cx)
                }))
                .into_any_element()
        });
        vec![
            dropdown(
                list_button("insert-smartart", t("objects.smartart"), open == Some(Popup::Object(ObjectPopup::Diagram(DiagramPopup::InsertSmartArt))), DiagramPopup::InsertSmartArt, cx),
                smartart_menu,
            ),
            dropdown(
                list_button("insert-chart", t("objects.chart"), open == Some(Popup::Object(ObjectPopup::Diagram(DiagramPopup::InsertChart))), DiagramPopup::InsertChart, cx),
                chart_menu,
            ),
        ]
    }

    /// SmartArt's and a chart's own groups on the contextual tab.
    pub(crate) fn diagram_groups(&mut self, sort: ObjectSort, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let open = self.ribbon.popup;
        match sort {
            ObjectSort::SmartArt => {
                let layouts = (open == Some(Popup::Object(ObjectPopup::Diagram(DiagramPopup::Layouts))))
                    .then(|| layout_menu("smartart-layouts-menu", false, cx));
                vec![group(
                    t("objects.group.smartart"),
                    vec![row(vec![
                        dropdown(list_button("smartart-layouts", t("objects.smartart.layouts"), open == Some(Popup::Object(ObjectPopup::Diagram(DiagramPopup::Layouts))), DiagramPopup::Layouts, cx), layouts),
                        text_button("smartart-text-pane", t("objects.smartart.text_pane"), t("objects.smartart.text_pane_title"), DiagramAction::TextPane, cx),
                    ])],
                )]
            }
            ObjectSort::Chart => {
                let types = (open == Some(Popup::Object(ObjectPopup::Diagram(DiagramPopup::ChartType)))).then(|| {
                    menu_panel("chart-type-menu")
                        .w(px(180.0))
                        .children(CHART_KINDS.into_iter().enumerate().map(|(index, kind)| {
                            menu_row(("chart-type", index), chart_kind_label(kind), false, act(DiagramAction::ChartType(kind)), cx)
                        }))
                        .into_any_element()
                });
                let legends = (open == Some(Popup::Object(ObjectPopup::Diagram(DiagramPopup::Legend)))).then(|| {
                    let places = [
                        (None, "objects.legend.none"),
                        (Some("r"), "objects.legend.right"),
                        (Some("t"), "objects.legend.top"),
                        (Some("l"), "objects.legend.left"),
                        (Some("b"), "objects.legend.bottom"),
                    ];
                    menu_panel("legend-menu")
                        .w(px(160.0))
                        .children(places.into_iter().enumerate().map(|(index, (place, label))| {
                            menu_row(("legend", index), t(label), false, act(DiagramAction::Legend(place)), cx)
                        }))
                        .child(menu_caption(t("objects.chart.title")))
                        .child(menu_row("title-on", t("objects.chart.title_on"), false, act(DiagramAction::ChartTitle(true)), cx))
                        .child(menu_row("title-off", t("objects.chart.title_off"), false, act(DiagramAction::ChartTitle(false)), cx))
                        .into_any_element()
                });
                vec![group(
                    t("objects.group.chart"),
                    vec![
                        row(vec![
                            text_button("chart-data", t("objects.chart.edit_data"), t("objects.chart.edit_data_title"), DiagramAction::EditData, cx),
                            dropdown(list_button("chart-type", t("objects.chart.type"), open == Some(Popup::Object(ObjectPopup::Diagram(DiagramPopup::ChartType))), DiagramPopup::ChartType, cx), types),
                        ]),
                        row(vec![dropdown(
                            list_button("chart-legend", t("objects.chart.elements"), open == Some(Popup::Object(ObjectPopup::Diagram(DiagramPopup::Legend))), DiagramPopup::Legend, cx),
                            legends,
                        )]),
                    ],
                )]
            }
            _ => Vec::new(),
        }
    }

    /// Runs a diagram control.
    pub(crate) fn run_diagram_action(&mut self, action: DiagramAction, cx: &mut Context<Self>) {
        self.ribbon.popup = None;
        match action {
            DiagramAction::InsertSmartArt(layout) => {
                self.place_document_caret();
                self.object_edit(cx, |view| view.document.insert_smartart(layout, None));
                self.select_inserted_at_caret();
                self.open_text_pane(cx);
            }
            DiagramAction::InsertChart(kind) => {
                self.place_document_caret();
                self.object_edit(cx, |view| view.document.insert_chart(kind, japanese()));
                self.select_inserted_at_caret();
            }
            DiagramAction::ChangeLayout(layout) => {
                let Some(target) = self.object_target() else { return };
                let Some(mut diagram) = self.document.smartart(target) else { return };
                diagram.layout = layout.to_string();
                self.object_edit(cx, |view| view.document.set_smartart(target, diagram));
                if let Some(pane) = self.objects.diagrams.text_pane.as_mut() {
                    pane.layout = layout.to_string();
                }
            }
            DiagramAction::TextPane => {
                if self.objects.diagrams.text_pane.is_some() {
                    self.objects.diagrams.text_pane = None;
                    cx.notify();
                } else {
                    self.open_text_pane(cx);
                }
            }
            DiagramAction::ChartType(kind) => {
                let Some(target) = self.object_target() else { return };
                let Some(mut chart) = self.document.chart_data(target) else { return };
                chart.kind = kind;
                self.object_edit(cx, |view| view.document.set_chart(target, chart));
            }
            DiagramAction::Legend(place) => {
                let Some(target) = self.object_target() else { return };
                self.object_edit(cx, |view| view.document.set_chart_legend(target, place));
            }
            DiagramAction::ChartTitle(on) => {
                let Some(target) = self.object_target() else { return };
                let Some(mut chart) = self.document.chart_data(target) else { return };
                chart.title = if on {
                    if chart.title.is_empty() {
                        t("objects.chart.default_title").to_string()
                    } else {
                        chart.title
                    }
                } else {
                    String::new()
                };
                self.object_edit(cx, |view| view.document.set_chart(target, chart));
            }
            DiagramAction::EditData => self.open_chart_sheet(cx),
        }
    }

    /// After inserting at the caret: the object just put in is selected.
    fn select_inserted_at_caret(&mut self) {
        let (paragraph, _) = self.insert_at;
        let found = self
            .document
            .word_document()
            .paragraph(paragraph)
            .map(|paragraph| {
                paragraph
                    .runs
                    .iter()
                    .enumerate()
                    .filter(|(_, run)| run.image.is_some())
                    .map(|(index, _)| index)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for run in found {
            let target = FloatingObjectTarget {
                paragraph_index: paragraph,
                run_index: run,
                kind: jade_docx_editor::word_document::FloatingObjectKind::Image,
            };
            if let Some(object) = crate::word_objects::region_hit(&self.document, target) {
                self.selected_object = Some(object);
                self.ribbon_tab = crate::RibbonTab::ObjectFormat;
                return;
            }
        }
    }

    /// テキスト ウィンドウ for the selected SmartArt graphic.
    fn open_text_pane(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.object_target() else { return };
        let Some(diagram) = self.document.smartart(target) else { return };
        self.objects.diagrams.text_pane = Some(TextPane {
            target,
            layout: diagram.layout.clone(),
            lines: smartart::outline(&diagram.nodes)
                .into_iter()
                .map(|(level, text)| (level, TextEdit::new(text)))
                .collect(),
            focus: Some(0),
        });
        cx.notify();
    }

    /// The text pane's points back into the graphic.
    fn apply_text_pane(&mut self, cx: &mut Context<Self>) {
        let Some(pane) = self.objects.diagrams.text_pane.as_ref() else { return };
        let target = pane.target;
        let lines: Vec<(usize, String)> = pane
            .lines
            .iter()
            .map(|(level, edit)| (*level, edit.text.clone()))
            .collect();
        let diagram = Diagram {
            layout: pane.layout.clone(),
            nodes: smartart::from_outline(&lines),
        };
        self.object_edit(cx, |view| view.document.set_smartart(target, diagram));
    }

    /// Keys in the text pane: Enter adds a point after this one (and puts
    /// what was typed in), Tab demotes, ⇧Tab promotes, Backspace in an
    /// empty point removes it, Escape closes.
    pub(crate) fn text_pane_key(&mut self, key: &str, shift: bool, cx: &mut Context<Self>) -> bool {
        let Some(pane) = self.objects.diagrams.text_pane.as_mut() else {
            return false;
        };
        let Some(index) = pane.focus else {
            if key == "escape" {
                self.objects.diagrams.text_pane = None;
                cx.notify();
                return true;
            }
            return false;
        };
        match key {
            "escape" => {
                self.objects.diagrams.text_pane = None;
            }
            "enter" | "return" => {
                let level = pane.lines[index].0;
                pane.lines.insert(index + 1, (level, TextEdit::new(String::new())));
                pane.focus = Some(index + 1);
                self.apply_text_pane(cx);
            }
            "tab" => {
                let (level, _) = pane.lines[index];
                let ceiling = if index == 0 { 0 } else { pane.lines[index - 1].0 + 1 };
                pane.lines[index].0 = if shift {
                    level.saturating_sub(1)
                } else {
                    (level + 1).min(ceiling)
                };
                self.apply_text_pane(cx);
            }
            "up" => pane.focus = Some(index.saturating_sub(1)),
            "down" => pane.focus = Some((index + 1).min(pane.lines.len() - 1)),
            "backspace" if pane.lines[index].1.text.is_empty() && pane.lines.len() > 1 => {
                pane.lines.remove(index);
                pane.focus = Some(index.saturating_sub(1));
                self.apply_text_pane(cx);
            }
            "backspace" => pane.lines[index].1.backspace(),
            "delete" => pane.lines[index].1.delete(),
            "left" => pane.lines[index].1.step(false),
            "right" => pane.lines[index].1.step(true),
            _ => return false,
        }
        cx.notify();
        true
    }

    /// The text pane, beside the pages while it is open.
    pub(crate) fn text_pane_element(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pane = self.objects.diagrams.text_pane.as_ref()?;
        let rows: Vec<AnyElement> = pane
            .lines
            .iter()
            .enumerate()
            .map(|(index, (level, edit))| {
                let focused = pane.focus == Some(index);
                let text = if focused {
                    let (before, after) = edit.split();
                    format!("{before}▏{after}")
                } else if edit.text.is_empty() {
                    t("objects.smartart.placeholder").to_string()
                } else {
                    edit.text.clone()
                };
                div()
                    .id(("smartart-line", index))
                    .flex()
                    .items_center()
                    .gap_1()
                    .pl(px(8.0 + *level as f32 * 16.0))
                    .h(px(24.0))
                    .rounded_sm()
                    .text_xs()
                    .when(focused, |line| line.bg(theme::selected()))
                    .when(edit.text.is_empty() && !focused, |line| line.text_color(theme::text_faint()))
                    .cursor_text()
                    .child("•")
                    .child(text)
                    .on_click(cx.listener(move |view, _event, _window, cx| {
                        if let Some(pane) = view.objects.diagrams.text_pane.as_mut() {
                            pane.focus = Some(index);
                        }
                        cx.notify();
                    }))
                    .into_any_element()
            })
            .collect();
        Some(
            div()
                .id("smartart-text-pane")
                .flex()
                .flex_col()
                .flex_none()
                .w(px(240.0))
                .h_full()
                .gap_1()
                .px_3()
                .pt_3()
                .overflow_y_scroll()
                .bg(theme::panel())
                .border_l_1()
                .border_color(theme::border())
                .child(div().text_sm().child(t("objects.smartart.text_pane")))
                .child(div().text_xs().text_color(theme::text_faint()).child(t("objects.smartart.text_pane_hint")))
                .children(rows)
                .child(
                    div()
                        .id("smartart-apply")
                        .mt_2()
                        .px_2()
                        .py_0p5()
                        .rounded_md()
                        .text_xs()
                        .border_1()
                        .border_color(theme::border())
                        .cursor_pointer()
                        .child(t("objects.smartart.apply"))
                        .on_click(cx.listener(|view, _event, _window, cx| view.apply_text_pane(cx))),
                )
                .into_any_element(),
        )
    }

    /// データの編集 for the selected chart.
    fn open_chart_sheet(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.object_target() else { return };
        let Some(chart) = self.document.chart_data(target) else { return };
        let mut cells = Vec::new();
        let mut header = vec![TextEdit::new(String::new())];
        header.extend(chart.series.iter().map(|series| TextEdit::new(series.name.clone())));
        cells.push(header);
        for (index, category) in chart.categories.iter().enumerate() {
            let mut row = vec![TextEdit::new(category.clone())];
            row.extend(chart.series.iter().map(|series| {
                TextEdit::new(
                    series.values[index]
                        .map(|value| format!("{value}"))
                        .unwrap_or_default(),
                )
            }));
            cells.push(row);
        }
        self.objects.diagrams.chart_sheet = Some(ChartSheet {
            target,
            chart,
            cells,
            focus: Some((1, 1)),
        });
        cx.notify();
    }

    /// The sheet's data into the chart.
    fn apply_chart_sheet(&mut self, cx: &mut Context<Self>) {
        let Some(sheet) = self.objects.diagrams.chart_sheet.take() else { return };
        let mut chart = sheet.chart.clone();
        let columns = sheet.cells.first().map(|row| row.len()).unwrap_or(1);
        chart.categories = sheet.cells.iter().skip(1).map(|row| row[0].text.clone()).collect();
        chart.series = (1..columns)
            .map(|column| ChartSeries {
                name: sheet.cells[0][column].text.clone(),
                values: sheet
                    .cells
                    .iter()
                    .skip(1)
                    .map(|row| row[column].text.trim().replace(',', "").parse::<f64>().ok())
                    .collect(),
            })
            .collect();
        let target = sheet.target;
        self.object_edit(cx, |view| view.document.set_chart(target, chart));
    }

    /// Keys in the sheet: Tab and the arrows move, Enter puts the data in,
    /// Escape closes.
    pub(crate) fn chart_sheet_key(&mut self, key: &str, shift: bool, cx: &mut Context<Self>) -> bool {
        let Some(sheet) = self.objects.diagrams.chart_sheet.as_mut() else {
            return false;
        };
        let rows = sheet.cells.len();
        let columns = sheet.cells.first().map(|row| row.len()).unwrap_or(1);
        let (row, column) = sheet.focus.unwrap_or((0, 0));
        match key {
            "escape" => self.objects.diagrams.chart_sheet = None,
            "enter" | "return" => {
                self.apply_chart_sheet(cx);
                return true;
            }
            "tab" if shift => sheet.focus = Some((row, column.saturating_sub(1))),
            "tab" => sheet.focus = Some((row, (column + 1).min(columns - 1))),
            "up" => sheet.focus = Some((row.saturating_sub(1), column)),
            "down" => sheet.focus = Some(((row + 1).min(rows - 1), column)),
            "backspace" => sheet.cells[row][column].backspace(),
            "delete" => sheet.cells[row][column].delete(),
            "left" => sheet.cells[row][column].step(false),
            "right" => sheet.cells[row][column].step(true),
            _ => return false,
        }
        cx.notify();
        true
    }

    /// The sheet, over the pages while it is open.
    pub(crate) fn chart_sheet_element(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let sheet = self.objects.diagrams.chart_sheet.as_ref()?;
        let focus = sheet.focus;
        let grid: Vec<AnyElement> = sheet
            .cells
            .iter()
            .enumerate()
            .map(|(row, cells)| {
                div()
                    .flex()
                    .children(cells.iter().enumerate().map(|(column, edit)| {
                        let focused = focus == Some((row, column));
                        let text = if focused {
                            let (before, after) = edit.split();
                            format!("{before}▏{after}")
                        } else {
                            edit.text.clone()
                        };
                        div()
                            .id(SharedString::from(format!("chart-cell-{row}-{column}")))
                            .w(px(88.0))
                            .h(px(22.0))
                            .px_1()
                            .border_1()
                            .border_color(if focused { theme::accent() } else { theme::border() })
                            .when(row == 0 || column == 0, |cell| cell.bg(theme::hovered()))
                            .text_xs()
                            .overflow_hidden()
                            .child(text)
                            .on_click(cx.listener(move |view, _event, _window, cx| {
                                if let Some(sheet) = view.objects.diagrams.chart_sheet.as_mut() {
                                    sheet.focus = Some((row, column));
                                }
                                cx.notify();
                            }))
                    }))
                    .into_any_element()
            })
            .collect();
        let button = |id: &'static str, label: &'static str, cx: &mut Context<Self>, on: fn(&mut ChartSheet)| {
            div()
                .id(id)
                .px_2()
                .py_0p5()
                .rounded_md()
                .text_xs()
                .border_1()
                .border_color(theme::border())
                .cursor_pointer()
                .child(label)
                .on_click(cx.listener(move |view, _event, _window, cx| {
                    if let Some(sheet) = view.objects.diagrams.chart_sheet.as_mut() {
                        on(sheet);
                    }
                    cx.notify();
                }))
                .into_any_element()
        };
        Some(
            div()
                .id("chart-sheet")
                .occlude()
                .p_4()
                .flex()
                .flex_col()
                .gap_2()
                .rounded_lg()
                .bg(theme::panel())
                .border_1()
                .border_color(theme::border())
                .shadow_lg()
                .text_sm()
                .child(t("objects.chart.edit_data"))
                .children(grid)
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(button("add-row", t("objects.chart.add_category"), cx, |sheet| {
                            let columns = sheet.cells[0].len();
                            let mut row = vec![TextEdit::new(format!("{} {}", t("objects.chart.category"), sheet.cells.len()))];
                            row.extend((1..columns).map(|_| TextEdit::new(String::new())));
                            sheet.cells.push(row);
                        }))
                        .child(button("add-column", t("objects.chart.add_series"), cx, |sheet| {
                            let name = format!("{} {}", t("objects.chart.series"), sheet.cells[0].len());
                            for (index, row) in sheet.cells.iter_mut().enumerate() {
                                row.push(TextEdit::new(if index == 0 { name.clone() } else { String::new() }));
                            }
                        }))
                        .child(button("remove-row", t("objects.chart.remove_category"), cx, |sheet| {
                            if sheet.cells.len() > 2 {
                                sheet.cells.pop();
                                sheet.focus = Some((1, 1));
                            }
                        }))
                        .child(button("remove-column", t("objects.chart.remove_series"), cx, |sheet| {
                            if sheet.cells[0].len() > 2 {
                                for row in sheet.cells.iter_mut() {
                                    row.pop();
                                }
                                sheet.focus = Some((1, 1));
                            }
                        })),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            div()
                                .id("chart-sheet-cancel")
                                .px_2()
                                .py_0p5()
                                .rounded_md()
                                .text_xs()
                                .border_1()
                                .border_color(theme::border())
                                .cursor_pointer()
                                .child(t("format.cancel"))
                                .on_click(cx.listener(|view, _event, _window, cx| {
                                    view.objects.diagrams.chart_sheet = None;
                                    cx.notify();
                                })),
                        )
                        .child(
                            div()
                                .id("chart-sheet-ok")
                                .px_3()
                                .py_0p5()
                                .rounded_md()
                                .text_xs()
                                .bg(theme::accent())
                                .text_color(gpui::white())
                                .cursor_pointer()
                                .child(t("format.ok"))
                                .on_click(cx.listener(|view, _event, _window, cx| view.apply_chart_sheet(cx))),
                        ),
                )
                .into_any_element(),
        )
    }
}

/// The SmartArt layouts, for insertion or レイアウトの変更.
fn layout_menu(id: &'static str, insert: bool, cx: &mut Context<DocumentView>) -> AnyElement {
    menu_panel(id)
        .w(px(220.0))
        .children(smartart::LAYOUTS.iter().enumerate().map(|(index, (layout, ja, en, _))| {
            let action = if insert {
                DiagramAction::InsertSmartArt(layout)
            } else {
                DiagramAction::ChangeLayout(layout)
            };
            menu_row((id, index), if japanese() { *ja } else { *en }, false, act(action), cx)
        }))
        .into_any_element()
}

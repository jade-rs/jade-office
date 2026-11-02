// Shared native/browser chart menu and dialogs: Insert ▸ Chart's gallery,
// Chart Design's items, Chart Title, Select Data Source and Edit Series.
// Each shell gives `chart_selection`, `chart_info`, `chart_ask`,
// `chart_dialog_top`, `tool` and `chart_act`.

/// What the chart button's items do.
#[derive(Clone, Copy)]
pub(super) enum ChartTool {
    /// Insert ▸ Chart over the selection.
    Insert(ChartPick),
    /// Insert ▸ Pictures from a file, at the active cell.
    Picture,
    /// Insert ▸ Shapes, at the active cell: its preset geometry.
    Shape(&'static str),
    /// Insert ▸ Shapes ▸ Freeform: drawn on the sheet a click at a time.
    Freeform,
    /// Insert ▸ Shapes ▸ Scribble: drawn freehand, dragged.
    Scribble,
    /// Chart Design, on the selected chart.
    Kind(ChartPick),
    Legend(Option<LegendPosition>),
    Ask(ChartField),
    /// Select Data Source.
    SelectData,
    /// Switch Row/Column.
    SwitchRows,
    Delete,
}

impl View {
    /// The chart button's dropdown: Insert ▸ Chart's gallery, pictures and
    /// shapes — or, with a chart selected, Change Chart Type's gallery and
    /// Chart Design's items.
    pub(super) fn chart_menu(&self, cx: &mut Context<Self>) -> gpui::Div {
        let heading = |key: &'static str| {
            div()
                .px_3()
                .pt_2()
                .pb_1()
                .text_color(theme::text_dim())
                .child(i18n::text(key))
        };
        let item = |id: (&'static str, usize),
                    label: &'static str,
                    tool: ChartTool,
                    chosen: bool,
                    cx: &mut Context<Self>| {
            div()
                .id(id)
                .px_3()
                .py_1()
                .cursor_pointer()
                .truncate()
                .when(chosen, |item| item.bg(theme::selected()))
                .hover(|item| item.bg(theme::hovered()))
                .child(i18n::text(label))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(
                    cx.listener(move |view, _event, _window, cx| view.tool(Tool::Chart(tool), cx)),
                )
        };
        let selected = self.chart_selection();
        let current = selected
            .and_then(|id| self.chart_info(id))
            .and_then(|info| info.kind);
        // The gallery: a box for each family, three to a row.
        let mut gallery = div().w(px(480.0)).flex().flex_wrap().text_xs();
        let mut at = 0;
        for (family, kinds) in CHART_GALLERY {
            let mut column = div().w(px(160.0)).pb_1().child(heading(family));
            for (pick, label) in kinds.iter().copied() {
                let tool = if selected.is_some() {
                    ChartTool::Kind(pick)
                } else {
                    ChartTool::Insert(pick)
                };
                let chosen = matches!(pick, ChartPick::Kind(kind) if Some(kind) == current);
                column = column.child(item(("chart-kind", at), label, tool, chosen, cx));
                at += 1;
            }
            gallery = gallery.child(column);
        }
        let separator = || div().h(px(1.0)).my_1().bg(theme::border());
        let side = if selected.is_some() {
            let mut side = div().w(px(200.0)).child(heading("chart.legend"));
            for (index, (legend, label)) in LEGENDS.into_iter().enumerate() {
                side = side.child(item(
                    ("chart-legend", index),
                    label,
                    ChartTool::Legend(legend),
                    false,
                    cx,
                ));
            }
            side.child(separator())
                .child(item(
                    ("chart-title", 0),
                    "chart.title_menu",
                    ChartTool::Ask(ChartField::Title),
                    false,
                    cx,
                ))
                .child(item(
                    ("chart-data", 0),
                    "chart.data_menu",
                    ChartTool::SelectData,
                    false,
                    cx,
                ))
                .child(item(
                    ("chart-switch", 0),
                    "chart.switch_rows",
                    ChartTool::SwitchRows,
                    false,
                    cx,
                ))
                .child(separator())
                .child(item(
                    ("chart-delete", 0),
                    "chart.delete",
                    ChartTool::Delete,
                    false,
                    cx,
                ))
        } else {
            div()
                .w(px(220.0))
                .child(item(
                    ("chart-picture", 0),
                    "sheet.insert_picture",
                    ChartTool::Picture,
                    false,
                    cx,
                ))
                .child(heading("sheet.shapes"))
                .child(item(
                    ("chart-freeform", 0),
                    "sheet.freeform",
                    ChartTool::Freeform,
                    false,
                    cx,
                ))
                .child(item(
                    ("chart-scribble", 0),
                    "sheet.scribble",
                    ChartTool::Scribble,
                    false,
                    cx,
                ))
                .child(
                    div().flex().flex_wrap().text_xs().children(
                        SHAPES
                            .into_iter()
                            .enumerate()
                            .map(|(index, (preset, label))| {
                                item(
                                    ("chart-shape", index),
                                    label,
                                    ChartTool::Shape(preset),
                                    false,
                                    cx,
                                )
                                .w(px(110.0))
                                .px_2()
                            }),
                    ),
                )
        };
        div()
            .py_1()
            .child(heading(if selected.is_some() {
                "chart.change_type"
            } else {
                "chart.insert"
            }))
            .child(
                div()
                    .flex()
                    .child(gallery)
                    .child(div().w(px(1.0)).mx_1().bg(theme::border()))
                    .child(side),
            )
    }

    /// A button of a chart dialog doing `action`.
    fn chart_button(
        &self,
        id: (&'static str, usize),
        label: &'static str,
        primary: bool,
        action: AskAction,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(id)
            .px_3()
            .py_1()
            .rounded_sm()
            .border_1()
            .border_color(theme::border())
            .cursor_pointer()
            .when(primary, |button| {
                button.bg(theme::accent()).text_color(gpui::white())
            })
            .hover(|button| button.opacity(0.85))
            .child(i18n::text(label))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |view, _event, _window, cx| view.chart_act(action, cx)))
    }

    /// A line of text in a dialog: with the caret when it takes the typing;
    /// a click gives it the typing.
    fn chart_text_box(
        &self,
        id: (&'static str, usize),
        text: &TextEdit,
        typing: bool,
        action: AskAction,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let content = if typing {
            super::caret_line(text, theme::accent().into(), false)
        } else {
            div().child(text.text.clone())
        };
        div()
            .id(id)
            .px_2()
            .py_1()
            .min_h(px(22.0))
            .rounded_sm()
            .border_1()
            .border_color(if typing {
                theme::accent()
            } else {
                theme::border()
            })
            .bg(theme::canvas())
            .cursor_text()
            .child(content)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |view, _event, _window, cx| view.chart_act(action, cx)))
    }

    /// A form's fields, a row each: a text, a choice's options, a tick box.
    fn chart_form_rows(&self, form: &Form, cx: &mut Context<Self>) -> gpui::Div {
        let mut rows = div().flex().flex_col().gap_2();
        for (index, field) in form.fields.iter().enumerate() {
            let label = div()
                .text_color(theme::text_dim())
                .child(i18n::text(field.key));
            let row = match &field.value {
                FieldValue::Text(text) => div().flex().flex_col().gap_1().child(label).child(
                    self.chart_text_box(
                        ("chart-form-text", index),
                        text,
                        form.focus == index,
                        AskAction::Field(index),
                        cx,
                    ),
                ),
                FieldValue::Choice { options, chosen } => {
                    let chips = options.iter().enumerate().map(|(option, key)| {
                        let picked = option == *chosen;
                        div()
                            .id(("chart-form-choice", index * 64 + option))
                            .px_2()
                            .py(px(2.0))
                            .rounded_sm()
                            .border_1()
                            .border_color(if picked {
                                theme::accent()
                            } else {
                                theme::border()
                            })
                            .when(picked, |chip| chip.bg(theme::selected()))
                            .cursor_pointer()
                            .child(i18n::text(key))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(move |view, _event, _window, cx| {
                                view.chart_act(AskAction::Choose(index, option), cx)
                            }))
                    });
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(label)
                        .child(div().flex().flex_wrap().gap_1().children(chips))
                }
                FieldValue::Check(on) => div().child(
                    div()
                    .id(("chart-form-check", index))
                    .flex()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .child(
                        div()
                            .size(px(12.0))
                            .rounded_sm()
                            .border_1()
                            .border_color(theme::accent())
                            .when(*on, |tick| tick.bg(theme::accent())),
                    )
                    .child(i18n::text(field.key))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |view, _event, _window, cx| {
                        view.chart_act(AskAction::Field(index), cx)
                    })),
                ),
            };
            rows = rows.child(row);
        }
        rows
    }

    /// Chart Title…, Select Data Source and Edit Series.
    pub(super) fn chart_dialog(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let ask = self.chart_ask()?;
        let footer = |this: &Self, cx: &mut Context<Self>| {
            div()
                .flex()
                .justify_end()
                .gap_2()
                .child(this.chart_button(("chart-cancel", 0), "cf.cancel", false, AskAction::Cancel, cx))
                .child(this.chart_button(("chart-ok", 0), "cf.ok", true, AskAction::Take, cx))
        };
        let (width, body) = match &ask.page {
            AskPage::Text(field, text) => (
                320.0,
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().text_sm().child(i18n::text(field.label_key())))
                    .child(self.chart_text_box(("chart-text", 0), text, true, AskAction::FocusRange, cx)),
            ),
            AskPage::Data(SelectData {
                editing: Some((_, form)),
                ..
            }) => (
                360.0,
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().text_sm().child(i18n::text(form.title)))
                    .child(self.chart_form_rows(form, cx)),
            ),
            AskPage::Data(data) => {
                let rows = data.series.iter().enumerate().map(|(index, series)| {
                    div()
                        .id(("chart-series", index))
                        .px_2()
                        .py(px(2.0))
                        .cursor_pointer()
                        .truncate()
                        .when(data.selected == Some(index), |row| row.bg(theme::selected()))
                        .hover(|row| row.bg(theme::hovered()))
                        .child(series.name.clone())
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |view, event: &gpui::ClickEvent, _window, cx| {
                            view.chart_act(AskAction::Pick(index), cx);
                            if event.click_count() >= 2 {
                                view.chart_act(AskAction::Edit, cx);
                            }
                        }))
                });
                let tools = div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .child(self.chart_button(("chart-series-add", 0), "chart.series.add_button", false, AskAction::Add, cx))
                    .child(self.chart_button(("chart-series-edit", 0), "chart.series.edit_button", false, AskAction::Edit, cx))
                    .child(self.chart_button(("chart-series-remove", 0), "chart.series.remove", false, AskAction::Remove, cx))
                    .child(self.chart_button(("chart-series-up", 0), "chart.series.up", false, AskAction::Move(true), cx))
                    .child(self.chart_button(("chart-series-down", 0), "chart.series.down", false, AskAction::Move(false), cx));
                (
                    420.0,
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(div().text_sm().child(i18n::text("chart.select_data")))
                        .child(div().text_color(theme::text_dim()).child(i18n::text("chart.data")))
                        .child(self.chart_text_box(("chart-range", 0), &data.range, true, AskAction::FocusRange, cx))
                        .child(
                            div()
                                .flex()
                                .justify_center()
                                .child(self.chart_button(("chart-switch", 0), "chart.switch_rows", false, AskAction::Switch, cx)),
                        )
                        .child(div().text_color(theme::text_dim()).child(i18n::text("chart.series.legend")))
                        .child(tools)
                        .child(
                            div()
                                .id("chart-series-list")
                                .max_h(px(160.0))
                                .overflow_y_scroll()
                                .rounded_sm()
                                .border_1()
                                .border_color(theme::border())
                                .bg(theme::canvas())
                                .children(rows),
                        ),
                )
            }
        };
        let panel = div()
            .w(px(width))
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(theme::border())
            .bg(theme::panel())
            .shadow_lg()
            .text_xs()
            .text_color(theme::text())
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(body)
            .child(footer(self, cx));
        Some(
            div()
                .absolute()
                .top(px(self.chart_dialog_top()))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(panel),
        )
    }
}

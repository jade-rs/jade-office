// Shared native/browser Fill ▸ Series dialog.
#[derive(Clone, Copy)]
pub(super) enum SeriesAction {
    In(SeriesIn),
    Kind(SeriesType),
    Unit(DateUnit),
    Trend,
    Focus(SeriesField),
    Take,
    Cancel,
}
impl View {
    pub(super) fn series_panel(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let dialog = self.series_editor()?;
        let chip = |id: &'static str, key: &'static str, on: bool, action: SeriesAction| {
            div()
                .id(id)
                .px_2()
                .py_1()
                .rounded_sm()
                .border_1()
                .border_color(if on { theme::accent() } else { theme::border() })
                .bg(if on { theme::selected() } else { theme::window() })
                .cursor_pointer()
                .child(i18n::text(key))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.series_action(action, cx);
                        cx.stop_propagation();
                    }),
                )
        };
        let heading = |key: &'static str| {
            div()
                .mt_3()
                .mb_1()
                .text_xs()
                .text_color(theme::text_dim())
                .child(i18n::text(key))
        };
        let row = || div().flex().flex_wrap().gap_1();
        let dates = dialog.kind == SeriesType::Date;
        let field = |id: &'static str, key: &'static str, which: SeriesField| {
            let focused = dialog.field == which;
            let text = match which {
                SeriesField::Step => &dialog.step,
                SeriesField::Stop => &dialog.stop,
            };
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(div().w(px(80.)).child(i18n::text(key)))
                .child(
                    div()
                        .id(id)
                        .w(px(120.))
                        .h(px(24.))
                        .px_1()
                        .flex()
                        .items_center()
                        .rounded_sm()
                        .border_1()
                        .border_color(if focused {
                            theme::accent()
                        } else {
                            theme::border()
                        })
                        .bg(theme::window())
                        .overflow_hidden()
                        .cursor_text()
                        .child(if focused {
                            super::caret_line(text, theme::accent().into(), false)
                        } else {
                            div().child(gpui::SharedString::from(text.text.clone()))
                        })
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.series_action(SeriesAction::Focus(which), cx);
                                cx.stop_propagation();
                            }),
                        ),
                )
        };
        let panel = div()
            .w(px(420.))
            .p_4()
            .rounded_md()
            .bg(theme::panel())
            .border_1()
            .border_color(theme::border())
            .shadow_lg()
            .text_sm()
            .text_color(theme::text())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(i18n::text("sheet.series.title")),
            )
            .child(heading("sheet.series.in"))
            .child(
                row()
                    .child(chip(
                        "series-rows",
                        "sheet.series.rows",
                        dialog.series_in == SeriesIn::Rows,
                        SeriesAction::In(SeriesIn::Rows),
                    ))
                    .child(chip(
                        "series-columns",
                        "sheet.series.columns",
                        dialog.series_in == SeriesIn::Columns,
                        SeriesAction::In(SeriesIn::Columns),
                    )),
            )
            .child(heading("sheet.series.type"))
            .child(
                row()
                    .child(chip(
                        "series-linear",
                        "sheet.series.linear",
                        dialog.kind == SeriesType::Linear,
                        SeriesAction::Kind(SeriesType::Linear),
                    ))
                    .child(chip(
                        "series-growth",
                        "sheet.series.growth",
                        dialog.kind == SeriesType::Growth,
                        SeriesAction::Kind(SeriesType::Growth),
                    ))
                    .child(chip(
                        "series-date",
                        "sheet.series.date",
                        dates,
                        SeriesAction::Kind(SeriesType::Date),
                    ))
                    .child(chip(
                        "series-autofill",
                        "sheet.series.autofill",
                        dialog.kind == SeriesType::AutoFill,
                        SeriesAction::Kind(SeriesType::AutoFill),
                    )),
            )
            .when(dates, |panel| {
                panel.child(heading("sheet.series.unit")).child(
                    row()
                        .child(chip(
                            "series-day",
                            "sheet.series.day",
                            dialog.unit == DateUnit::Day,
                            SeriesAction::Unit(DateUnit::Day),
                        ))
                        .child(chip(
                            "series-weekday",
                            "sheet.series.weekday",
                            dialog.unit == DateUnit::Weekday,
                            SeriesAction::Unit(DateUnit::Weekday),
                        ))
                        .child(chip(
                            "series-month",
                            "sheet.series.month",
                            dialog.unit == DateUnit::Month,
                            SeriesAction::Unit(DateUnit::Month),
                        ))
                        .child(chip(
                            "series-year",
                            "sheet.series.year",
                            dialog.unit == DateUnit::Year,
                            SeriesAction::Unit(DateUnit::Year),
                        )),
                )
            })
            .when(
                matches!(dialog.kind, SeriesType::Linear | SeriesType::Growth),
                |panel| {
                    panel.child(div().mt_3().child(chip(
                        "series-trend",
                        "sheet.series.trend",
                        dialog.trend,
                        SeriesAction::Trend,
                    )))
                },
            )
            .child(
                div()
                    .mt_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(field("series-step", "sheet.series.step", SeriesField::Step))
                    .child(field("series-stop", "sheet.series.stop", SeriesField::Stop)),
            )
            .when(dialog.problem, |panel| {
                panel.child(
                    div()
                        .mt_2()
                        .text_xs()
                        .text_color(theme::accent())
                        .child(i18n::text("sheet.series.not_a_number")),
                )
            })
            .child(
                div()
                    .mt_4()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(chip(
                        "series-cancel",
                        "validation.cancel",
                        false,
                        SeriesAction::Cancel,
                    ))
                    .child(chip("series-ok", "validation.ok", false, SeriesAction::Take)),
            );
        Some(super::conditional::centred(panel))
    }
}

/// What a press in the dialog does to it; OK and Cancel are the view's.
fn change_series(dialog: &mut SeriesDialog, action: SeriesAction) {
    match action {
        SeriesAction::In(series_in) => dialog.series_in = series_in,
        SeriesAction::Kind(kind) => dialog.kind = kind,
        SeriesAction::Unit(unit) => dialog.unit = unit,
        SeriesAction::Trend => dialog.trend = !dialog.trend,
        SeriesAction::Focus(field) => dialog.field = field,
        SeriesAction::Take | SeriesAction::Cancel => {}
    }
}

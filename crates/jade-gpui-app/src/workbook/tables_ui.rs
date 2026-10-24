#[derive(Clone, Copy)]
pub(super) enum TableAction {
    Focus(TableField),
    Headers,
    Totals,
    Filter,
    First,
    Last,
    Rows,
    Columns,
    Styles,
    Style(usize),
    Column,
    Total,
    Take,
    Convert,
    Cancel,
}
fn table_problem(error: String) -> String {
    match error.as_str() {
        "table.invalid_range" => i18n::text("table.invalid_range").into(),
        "table.missing_name" => i18n::text("table.missing_name").into(),
        _ => error,
    }
}
fn change_table_dialog(d: &mut TableDialog, action: TableAction) {
    match action {
        TableAction::Focus(f) => d.focus(f),
        TableAction::Headers => {
            d.has_headers = !d.has_headers;
            if d.editing() {
                d.filter_buttons = d.has_headers;
            }
        }
        TableAction::Totals => d.has_totals = !d.has_totals,
        TableAction::Filter => d.filter_buttons = !d.filter_buttons,
        TableAction::First => d.style.first_column = !d.style.first_column,
        TableAction::Last => d.style.last_column = !d.style.last_column,
        TableAction::Rows => d.style.row_stripes = !d.style.row_stripes,
        TableAction::Columns => d.style.column_stripes = !d.style.column_stripes,
        TableAction::Styles => d.show_styles = !d.show_styles,
        TableAction::Style(i) => {
            if let Some(style) = d.styles.get(i) {
                d.style.name = style.clone();
                d.show_styles = false;
            }
        }
        TableAction::Column => d.next_column(),
        TableAction::Total => d.next_total(),
        _ => {}
    }
}
fn table_style_label(name: Option<&str>) -> String {
    let Some(name) = name else {
        return i18n::text("table.no_style").into();
    };
    for (prefix, label) in [
        ("TableStyleLight", "table.light"),
        ("TableStyleMedium", "table.medium"),
        ("TableStyleDark", "table.dark"),
    ] {
        if let Some(n) = name.strip_prefix(prefix) {
            return format!("{} {n}", i18n::text(label));
        }
    }
    name.into()
}
impl View {
    pub(super) fn table_panel(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let d = self.table_editor()?;
        let title = if d.editing() {
            "table.design"
        } else {
            "table.create"
        };
        let mut content = div().flex().flex_col().gap_2().child(
            div()
                .font_weight(gpui::FontWeight::BOLD)
                .child(i18n::text(title)),
        );
        for field in [TableField::Range, TableField::Name] {
            content = content.child(self.table_field(field, d, cx));
        }
        content = content.child(self.table_button(
            "table-headers",
            format!(
                "{} {}",
                if d.has_headers { "☑" } else { "☐" },
                i18n::text(if d.editing() {
                    "table.headers"
                } else {
                    "table.has_headers"
                })
            ),
            TableAction::Headers,
            cx,
        ));
        content = content.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(i18n::text("table.style"))
                .child(self.table_button(
                    "table-styles",
                    table_style_label(d.style.name.as_deref()),
                    TableAction::Styles,
                    cx,
                )),
        );
        if d.show_styles {
            let mut choices = div()
                .id("table-style-list")
                .max_h(px(120.))
                .overflow_y_scroll()
                .flex()
                .flex_wrap()
                .gap_1();
            for (i, style) in d.styles.iter().enumerate() {
                choices = choices.child(self.table_button(
                    ("table-style", i),
                    table_style_label(style.as_deref()),
                    TableAction::Style(i),
                    cx,
                ));
            }
            content = content.child(choices);
        }
        for (key, on, action) in [
            (
                "table.first_column",
                d.style.first_column,
                TableAction::First,
            ),
            ("table.last_column", d.style.last_column, TableAction::Last),
            ("table.banded_rows", d.style.row_stripes, TableAction::Rows),
            (
                "table.banded_columns",
                d.style.column_stripes,
                TableAction::Columns,
            ),
        ] {
            content = content.child(self.table_button(
                key,
                format!("{} {}", if on { "☑" } else { "☐" }, i18n::text(key)),
                action,
                cx,
            ));
        }
        if d.editing() {
            content = content.child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.table_button(
                        "table-totals",
                        format!(
                            "{} {}",
                            if d.has_totals { "☑" } else { "☐" },
                            i18n::text("table.totals")
                        ),
                        TableAction::Totals,
                        cx,
                    ))
                    .child(self.table_button(
                        "table-filter",
                        format!(
                            "{} {}",
                            if d.filter_buttons { "☑" } else { "☐" },
                            i18n::text("table.filter")
                        ),
                        TableAction::Filter,
                        cx,
                    )),
            );
            content = content
                .child(
                    div()
                        .mt_2()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(i18n::text("table.column"))
                        .child(self.table_button(
                            "table-column",
                            d.column_name().unwrap_or_default().into(),
                            TableAction::Column,
                            cx,
                        )),
                )
                .child(self.table_field(TableField::Formula, d, cx));
            if d.has_totals {
                content = content.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(i18n::text("table.total_function"))
                        .child(self.table_button(
                            "table-total-function",
                            i18n::text(TOTALS[d.total_kind].0).into(),
                            TableAction::Total,
                            cx,
                        )),
                );
                if d.total_kind >= 9 {
                    content = content.child(self.table_field(TableField::TotalValue, d, cx));
                }
            }
        }
        if let Some(error) = &d.problem {
            content = content.child(
                div()
                    .text_color(gpui::rgb(0xd43f3f))
                    .child(gpui::SharedString::from(error.clone())),
            );
        }
        let mut buttons = div().mt_2().flex().justify_between();
        if d.editing() {
            buttons = buttons.child(self.table_button(
                "table-convert",
                i18n::text("table.convert").into(),
                TableAction::Convert,
                cx,
            ));
        }
        buttons = buttons.child(
            div()
                .flex()
                .gap_2()
                .child(self.table_button(
                    "table-cancel",
                    i18n::text("validation.cancel").into(),
                    TableAction::Cancel,
                    cx,
                ))
                .child(self.table_button(
                    "table-ok",
                    i18n::text("validation.ok").into(),
                    TableAction::Take,
                    cx,
                )),
        );
        let panel = div()
            .w(px(480.))
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
                    .id("table-dialog-content")
                    .max_h(px(self.table_body_height()))
                    .overflow_y_scroll()
                    .child(content),
            )
            .child(buttons);
        Some(
            div()
                .absolute()
                .inset_0()
                .pt(px(50.0))
                .flex()
                .justify_center()
                .bg(gpui::rgba(0x00000022))
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                .child(panel),
        )
    }
    fn table_field(&self, field: TableField, d: &TableDialog, cx: &mut Context<Self>) -> gpui::Div {
        let focused = d.field == field;
        let store = d.input_bounds.clone();
        let value = if focused {
            let text = d.field();
            let at = text.caret();
            format!("{}│{}", &text.text[..at], &text.text[at..])
        } else {
            d.value(field).into()
        };
        div()
            .child(
                div()
                    .text_xs()
                    .text_color(theme::text_dim())
                    .child(i18n::text(field.key())),
            )
            .child(
                div()
                    .id(field.key())
                    .mt_1()
                    .min_h(px(28.))
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .border_1()
                    .border_color(if focused {
                        theme::accent()
                    } else {
                        theme::border()
                    })
                    .relative()
                    .bg(theme::window())
                    .child(gpui::SharedString::from(value))
                    .cursor_text()
                    .child(
                        gpui::canvas(
                            move |bounds, _, _| {
                                if focused {
                                    store.set(Some([
                                        f32::from(bounds.origin.x),
                                        f32::from(bounds.origin.y),
                                        f32::from(bounds.size.width),
                                        f32::from(bounds.size.height),
                                    ]));
                                }
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.sheet_table_action(TableAction::Focus(field), cx);
                            cx.stop_propagation();
                        }),
                    ),
            )
    }
    fn table_button(
        &self,
        id: impl Into<gpui::ElementId>,
        label: String,
        action: TableAction,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(id)
            .px_2()
            .py_1()
            .rounded_sm()
            .border_1()
            .border_color(theme::border())
            .bg(theme::window())
            .cursor_pointer()
            .child(gpui::SharedString::from(label))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.sheet_table_action(action, cx);
                    cx.stop_propagation();
                }),
            )
    }
}

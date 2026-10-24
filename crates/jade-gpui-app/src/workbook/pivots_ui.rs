#[derive(Clone, Copy)]
pub(super) enum PivotAction {
    Focus(PivotDialogField),
    CreateFocus(PivotCreationField),
    NewSheet,
    Field(usize),
    Area(PivotArea),
    MoveField(bool),
    Values(bool),
    Measure(usize),
    AddMeasure,
    RemoveMeasure,
    MoveMeasure(bool),
    Aggregation,
    Member(u32),
    All(bool),
    MembersPage(bool),
    RefreshItems,
    Layout,
    Sort,
    Subtotals,
    SubtotalTop,
    RowGrand,
    ColumnGrand,
    Fit,
    Styles,
    Style(usize),
    RowStripe,
    ColumnStripe,
    RowHeader,
    ColumnHeader,
    LastColumn,
    Take,
    Cancel,
}
fn pivot_problem(error: String) -> String {
    match error.as_str() {
        "pivot.invalid_source" => i18n::text("pivot.invalid_source").into(),
        "pivot.invalid_origin" => i18n::text("pivot.invalid_origin").into(),
        "pivot.stale" => i18n::text("pivot.stale").into(),
        "pivot.apply_source_first" => i18n::text("pivot.apply_source_first").into(),
        "pivot.hidden_destination" => i18n::text("pivot.hidden_destination").into(),
        _ => error,
    }
}
fn change_pivot_creation(d: &mut PivotCreation, action: PivotAction) {
    d.problem = None;
    match action {
        PivotAction::CreateFocus(f) => d.focus(f),
        PivotAction::NewSheet => {
            d.new_sheet = !d.new_sheet;
            if !d.new_sheet && d.focus == PivotCreationField::SheetName {
                d.focus(PivotCreationField::Destination);
            }
        }
        _ => {}
    }
}
fn change_pivot_dialog(d: &mut PivotDialog, action: PivotAction) {
    d.flush();
    d.problem = None;
    match action {
        PivotAction::Focus(f) => d.focus(f),
        PivotAction::Field(i) => d.select_field(i),
        PivotAction::Area(a) => d.set_area(a),
        PivotAction::MoveField(forward) => d.move_field(forward),
        PivotAction::Values(rows) => d.values_on_rows(rows),
        PivotAction::Measure(i) => d.select_measure(i),
        PivotAction::AddMeasure => d.add_measure(),
        PivotAction::RemoveMeasure => d.remove_measure(),
        PivotAction::MoveMeasure(forward) => d.move_measure(forward),
        PivotAction::Aggregation => d.next_aggregation(),
        PivotAction::Member(i) => d.toggle_member(i),
        PivotAction::All(all) => d.all_members(all),
        PivotAction::MembersPage(forward) => d.next_members(forward),
        PivotAction::Layout => d.next_layout(),
        PivotAction::Sort => {
            let f = &mut d.draft.field_options[d.selected_field];
            f.sort = Some(
                match f.sort.as_deref() {
                    Some("ascending") => "descending",
                    Some("descending") => "manual",
                    _ => "ascending",
                }
                .into(),
            );
        }
        PivotAction::Subtotals => {
            let f = &mut d.draft.field_options[d.selected_field];
            f.subtotals = Some(!f.subtotals.unwrap_or(true));
        }
        PivotAction::SubtotalTop => {
            let f = &mut d.draft.field_options[d.selected_field];
            f.subtotals_at_top = Some(!f.subtotals_at_top.unwrap_or(true));
        }
        PivotAction::RowGrand => {
            let l = d.draft.layout.as_mut().unwrap();
            l.row_grand_totals = !l.row_grand_totals;
        }
        PivotAction::ColumnGrand => {
            let l = d.draft.layout.as_mut().unwrap();
            l.column_grand_totals = !l.column_grand_totals;
        }
        PivotAction::Fit => d.draft.fit_columns = Some(!d.draft.fit_columns.unwrap_or(true)),
        PivotAction::Styles => d.show_styles = !d.show_styles,
        PivotAction::Style(i) => {
            if let Some(name) = d.styles.get(i).cloned() {
                let s = d.draft.style.get_or_insert(PivotStyle {
                    name: None,
                    row_headers: true,
                    column_headers: true,
                    row_stripes: false,
                    column_stripes: false,
                    last_column: false,
                });
                s.name = name;
                d.show_styles = false;
            }
        }
        PivotAction::RowStripe
        | PivotAction::ColumnStripe
        | PivotAction::RowHeader
        | PivotAction::ColumnHeader
        | PivotAction::LastColumn => {
            let s = d.draft.style.get_or_insert(PivotStyle {
                name: None,
                row_headers: true,
                column_headers: true,
                row_stripes: false,
                column_stripes: false,
                last_column: false,
            });
            match action {
                PivotAction::RowStripe => s.row_stripes = !s.row_stripes,
                PivotAction::ColumnStripe => s.column_stripes = !s.column_stripes,
                PivotAction::RowHeader => s.row_headers = !s.row_headers,
                PivotAction::ColumnHeader => s.column_headers = !s.column_headers,
                _ => s.last_column = !s.last_column,
            }
        }
        _ => {}
    }
}
fn pivot_aggregation_key(kind: &str) -> &'static str {
    match kind {
        "sum" => "pivot.sum",
        "count" => "pivot.count",
        "average" => "pivot.average",
        "max" => "pivot.max",
        "min" => "pivot.min",
        "product" => "pivot.product",
        "countNums" => "pivot.count_numbers",
        "stdDev" => "pivot.stddev",
        "stdDevp" => "pivot.stddevp",
        "var" => "pivot.var",
        "varp" => "pivot.varp",
        _ => "pivot.values",
    }
}
fn pivot_style_label(name: Option<&str>) -> String {
    let Some(name) = name else {
        return i18n::text("table.no_style").into();
    };
    for (prefix, key) in [
        ("PivotStyleLight", "table.light"),
        ("PivotStyleMedium", "table.medium"),
        ("PivotStyleDark", "table.dark"),
    ] {
        if let Some(number) = name.strip_prefix(prefix) {
            return format!("{} {number}", i18n::text(key));
        }
    }
    name.into()
}
impl View {
    pub(super) fn pivot_panel(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        if let Some(d) = self.pivot_creator() {
            return Some(self.pivot_creation_panel(d, cx));
        }
        let d = self.pivot_editor()?;
        let l = d.draft.layout.as_ref()?;
        let axis_label = |axis: &[i32]| {
            axis.iter()
                .map(|i| {
                    if *i == -2 {
                        i18n::text("pivot.values").to_string()
                    } else {
                        d.draft
                            .field_options
                            .get(*i as usize)
                            .and_then(|f| f.caption.clone())
                            .unwrap_or_default()
                    }
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        let mut content = div().flex().flex_col().gap_2().child(
            div()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(gpui::SharedString::from(format!(
                    "{} — {}",
                    i18n::text("pivot.edit"),
                    d.name
                ))),
        );
        content = content
            .child(self.pivot_field(PivotDialogField::Source, d, cx))
            .child(
                div()
                    .text_xs()
                    .text_color(theme::text_dim())
                    .child(i18n::text("pivot.source_hint")),
            )
            .child(self.pivot_field(PivotDialogField::Origin, d, cx));
        if d.shared_reports > 1 {
            content = content.child(div().text_xs().child(gpui::SharedString::from(i18n::format(
                "pivot.shared_source",
                &[("count", &d.shared_reports)],
            ))));
        }
        let mut fields = div()
            .id("pivot-field-list")
            .max_h(px(120.))
            .overflow_y_scroll()
            .flex()
            .flex_wrap()
            .gap_1();
        for (i, name) in d.field_names.iter().enumerate() {
            fields = fields.child(self.pivot_button(
                ("pivot-field", i),
                format!("{} {name}", if i == d.selected_field { "●" } else { "○" }),
                PivotAction::Field(i),
                cx,
            ));
        }
        content = content
            .child(div().mt_2().child(i18n::text("pivot.fields")))
            .child(fields);
        let mut areas = div().flex().flex_wrap().gap_1();
        for (i, (key, area)) in [
            ("pivot.rows", PivotArea::Rows),
            ("pivot.columns", PivotArea::Columns),
            ("pivot.filters", PivotArea::Pages),
            ("pivot.unused", PivotArea::None),
        ]
        .into_iter()
        .enumerate()
        {
            areas = areas.child(self.pivot_button(
                ("pivot-area", i),
                format!(
                    "{} {}",
                    if d.area(d.selected_field) == area {
                        "●"
                    } else {
                        "○"
                    },
                    i18n::text(key)
                ),
                PivotAction::Area(area),
                cx,
            ));
        }
        content = content.child(areas).child(
            div()
                .flex()
                .gap_1()
                .child(self.pivot_button(
                    "pivot-field-up",
                    "↑".into(),
                    PivotAction::MoveField(false),
                    cx,
                ))
                .child(self.pivot_button(
                    "pivot-field-down",
                    "↓".into(),
                    PivotAction::MoveField(true),
                    cx,
                )),
        );
        for (key, label) in [
            ("pivot.rows", axis_label(&l.rows)),
            ("pivot.columns", axis_label(&l.columns)),
            (
                "pivot.filters",
                l.pages
                    .iter()
                    .map(|p| d.field_names[p.field as usize].clone())
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
        ] {
            content = content.child(div().text_xs().child(gpui::SharedString::from(format!(
                "{}: {label}",
                i18n::text(key)
            ))));
        }
        let layout = match l.report_layout {
            PivotReportLayout::Compact => "pivot.compact",
            PivotReportLayout::Outline => "pivot.outline",
            PivotReportLayout::Tabular => "pivot.tabular",
        };
        content = content.child(
            div()
                .flex()
                .flex_wrap()
                .gap_1()
                .child(self.pivot_button(
                    "pivot-layout",
                    i18n::text(layout).into(),
                    PivotAction::Layout,
                    cx,
                ))
                .child(self.pivot_button(
                    "pivot-values-rows",
                    i18n::text("pivot.values_rows").into(),
                    PivotAction::Values(true),
                    cx,
                ))
                .child(self.pivot_button(
                    "pivot-values-columns",
                    i18n::text("pivot.values_columns").into(),
                    PivotAction::Values(false),
                    cx,
                )),
        );
        content = content.child(self.pivot_field(PivotDialogField::FieldCaption, d, cx));
        let options = &d.draft.field_options[d.selected_field];
        let sort = match options.sort.as_deref() {
            Some("ascending") => "pivot.ascending",
            Some("descending") => "pivot.descending",
            _ => "pivot.manual",
        };
        content = content.child(
            div()
                .flex()
                .flex_wrap()
                .gap_1()
                .child(self.pivot_button(
                    "pivot-sort",
                    i18n::text(sort).into(),
                    PivotAction::Sort,
                    cx,
                ))
                .child(self.pivot_toggle(
                    "pivot.subtotals",
                    options.subtotals.unwrap_or(true),
                    PivotAction::Subtotals,
                    cx,
                ))
                .child(self.pivot_toggle(
                    "pivot.subtotal_top",
                    options.subtotals_at_top.unwrap_or(true),
                    PivotAction::SubtotalTop,
                    cx,
                )),
        );
        content = content.child(div().mt_2().child(i18n::text("pivot.values")));
        let mut measures = div().flex().flex_wrap().gap_1();
        for (i, m) in d.draft.measures.as_ref()?.iter().enumerate() {
            measures = measures.child(self.pivot_button(
                ("pivot-measure", i),
                format!(
                    "{} {}",
                    if i == d.selected_measure {
                        "●"
                    } else {
                        "○"
                    },
                    m.caption
                ),
                PivotAction::Measure(i),
                cx,
            ));
        }
        content = content.child(measures).child(
            div()
                .flex()
                .flex_wrap()
                .gap_1()
                .child(self.pivot_button(
                    "pivot-add-measure",
                    i18n::text("pivot.add_value").into(),
                    PivotAction::AddMeasure,
                    cx,
                ))
                .child(self.pivot_button(
                    "pivot-remove-measure",
                    i18n::text("pivot.remove_value").into(),
                    PivotAction::RemoveMeasure,
                    cx,
                ))
                .child(self.pivot_button(
                    "pivot-measure-up",
                    "↑".into(),
                    PivotAction::MoveMeasure(false),
                    cx,
                ))
                .child(self.pivot_button(
                    "pivot-measure-down",
                    "↓".into(),
                    PivotAction::MoveMeasure(true),
                    cx,
                )),
        );
        if let Some(m) = d.draft.measures.as_ref()?.get(d.selected_measure) {
            content = content
                .child(self.pivot_field(PivotDialogField::MeasureCaption, d, cx))
                .child(self.pivot_button(
                    "pivot-aggregation",
                    i18n::text(pivot_aggregation_key(&m.aggregation)).into(),
                    PivotAction::Aggregation,
                    cx,
                ));
        }
        content = content
            .child(div().mt_2().child(i18n::text("pivot.visible_items")))
            .child(self.pivot_button(
                "pivot-refresh-items",
                i18n::text("pivot.refresh_items").into(),
                PivotAction::RefreshItems,
                cx,
            ))
            .child(self.pivot_field(PivotDialogField::Search, d, cx))
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(self.pivot_button(
                        "pivot-all",
                        i18n::text("pivot.select_all").into(),
                        PivotAction::All(true),
                        cx,
                    ))
                    .child(self.pivot_button(
                        "pivot-none",
                        i18n::text("pivot.select_none").into(),
                        PivotAction::All(false),
                        cx,
                    )),
            );
        if d.members[d.selected_field].is_empty() {
            content = content.child(
                div()
                    .text_xs()
                    .text_color(theme::text_dim())
                    .child(i18n::text("pivot.no_items")),
            );
        }
        let mut members = div()
            .id("pivot-member-list")
            .max_h(px(140.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_1();
        for member in d.shown_members() {
            let label = if member.blank {
                i18n::text("pivot.blank").into()
            } else {
                member.label.clone()
            };
            members = members.child(self.pivot_button(
                ("pivot-member", member.index as usize),
                format!(
                    "{} {label}",
                    if d.visible(member.index) {
                        "☑"
                    } else {
                        "☐"
                    }
                ),
                PivotAction::Member(member.index),
                cx,
            ));
        }
        content = content.child(members).child(
            div()
                .flex()
                .gap_1()
                .child(self.pivot_button(
                    "pivot-prev-members",
                    i18n::text("pivot.previous_items").into(),
                    PivotAction::MembersPage(false),
                    cx,
                ))
                .child(self.pivot_button(
                    "pivot-next-members",
                    i18n::text("pivot.next_items").into(),
                    PivotAction::MembersPage(true),
                    cx,
                )),
        );
        content = content.child(
            div()
                .flex()
                .flex_wrap()
                .gap_1()
                .child(self.pivot_toggle(
                    "pivot.row_grand",
                    l.row_grand_totals,
                    PivotAction::RowGrand,
                    cx,
                ))
                .child(self.pivot_toggle(
                    "pivot.column_grand",
                    l.column_grand_totals,
                    PivotAction::ColumnGrand,
                    cx,
                ))
                .child(self.pivot_toggle(
                    "pivot.autofit",
                    d.draft.fit_columns.unwrap_or(true),
                    PivotAction::Fit,
                    cx,
                )),
        );
        content = content.child(self.pivot_button(
            "pivot-styles",
            pivot_style_label(d.draft.style.as_ref().and_then(|s| s.name.as_deref())),
            PivotAction::Styles,
            cx,
        ));
        if d.show_styles {
            let mut styles = div()
                .id("pivot-styles-list")
                .max_h(px(120.))
                .overflow_y_scroll()
                .flex()
                .flex_wrap()
                .gap_1();
            for (i, style) in d.styles.iter().enumerate() {
                styles = styles.child(self.pivot_button(
                    ("pivot-style", i),
                    pivot_style_label(style.as_deref()),
                    PivotAction::Style(i),
                    cx,
                ));
            }
            content = content.child(styles);
        }
        let style = d.draft.style.clone().unwrap_or(PivotStyle {
            name: None,
            row_headers: true,
            column_headers: true,
            row_stripes: false,
            column_stripes: false,
            last_column: false,
        });
        let mut toggles = div().flex().flex_wrap().gap_1();
        for (key, on, action) in [
            (
                "table.banded_rows",
                style.row_stripes,
                PivotAction::RowStripe,
            ),
            (
                "table.banded_columns",
                style.column_stripes,
                PivotAction::ColumnStripe,
            ),
            (
                "pivot.row_headers",
                style.row_headers,
                PivotAction::RowHeader,
            ),
            (
                "pivot.column_headers",
                style.column_headers,
                PivotAction::ColumnHeader,
            ),
            (
                "table.last_column",
                style.last_column,
                PivotAction::LastColumn,
            ),
        ] {
            toggles = toggles.child(self.pivot_toggle(key, on, action, cx));
        }
        content = content.child(toggles);
        if let Some(problem) = &d.problem {
            content = content.child(
                div()
                    .text_color(gpui::rgb(0xd43f3f))
                    .child(gpui::SharedString::from(problem.clone())),
            );
        }
        let buttons = div()
            .mt_2()
            .flex()
            .justify_end()
            .gap_2()
            .child(self.pivot_button(
                "pivot-cancel",
                i18n::text("validation.cancel").into(),
                PivotAction::Cancel,
                cx,
            ))
            .child(self.pivot_button(
                "pivot-apply",
                i18n::text("pivot.apply").into(),
                PivotAction::Take,
                cx,
            ));
        Some(self.pivot_modal(content, buttons))
    }
    fn pivot_modal(&self, content: gpui::Div, buttons: gpui::Div) -> gpui::Div {
        div()
            .absolute()
            .inset_0()
            .pt(px(35.))
            .flex()
            .justify_center()
            .bg(gpui::rgba(0x00000022))
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .w(px(self.pivot_body_width()))
                    .p_4()
                    .rounded_md()
                    .bg(theme::panel())
                    .border_1()
                    .border_color(theme::border())
                    .shadow_lg()
                    .text_sm()
                    .text_color(theme::text())
                    .child(
                        div()
                            .id("pivot-editor-content")
                            .max_h(px(self.pivot_body_height()))
                            .overflow_y_scroll()
                            .child(content),
                    )
                    .child(buttons),
            )
    }
    fn pivot_toggle(
        &self,
        key: &'static str,
        on: bool,
        action: PivotAction,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        self.pivot_button(
            key,
            format!("{} {}", if on { "☑" } else { "☐" }, i18n::text(key)),
            action,
            cx,
        )
    }
    fn pivot_button(
        &self,
        id: impl Into<gpui::ElementId>,
        label: String,
        action: PivotAction,
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
                    this.pivot_action(action, cx);
                    cx.stop_propagation();
                }),
            )
    }
    fn pivot_field(
        &self,
        f: PivotDialogField,
        d: &PivotDialog,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let key = match f {
            PivotDialogField::Source => "pivot.source",
            PivotDialogField::Origin => "pivot.origin",
            PivotDialogField::MeasureCaption => "pivot.value_caption",
            PivotDialogField::FieldCaption => "pivot.field_caption",
            PivotDialogField::Search => "pivot.search",
        };
        let focused = d.focus == f;
        let store = d.input_bounds.clone();
        let value = if focused {
            let text = d.field();
            let at = text.caret();
            format!("{}│{}", &text.text[..at], &text.text[at..])
        } else {
            d.value(f).to_string()
        };
        div()
            .child(
                div()
                    .text_xs()
                    .text_color(theme::text_dim())
                    .child(i18n::text(key)),
            )
            .child(
                div()
                    .id(key)
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
                    .cursor_text()
                    .child(gpui::SharedString::from(value))
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
                            this.pivot_action(PivotAction::Focus(f), cx);
                            cx.stop_propagation();
                        }),
                    ),
            )
    }
    fn pivot_creation_panel(&self, d: &PivotCreation, cx: &mut Context<Self>) -> gpui::Div {
        let mut content = div().flex().flex_col().gap_2().child(
            div()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(i18n::text("pivot.create")),
        );
        for field in [
            PivotCreationField::Source,
            PivotCreationField::Name,
            PivotCreationField::Destination,
        ] {
            content = content.child(self.pivot_creation_field(field, d, cx));
        }
        content = content.child(self.pivot_toggle(
            "pivot.new_sheet",
            d.new_sheet,
            PivotAction::NewSheet,
            cx,
        ));
        if d.new_sheet {
            content =
                content.child(self.pivot_creation_field(PivotCreationField::SheetName, d, cx));
        }
        content = content.child(
            div()
                .text_xs()
                .text_color(theme::text_dim())
                .child(i18n::text("pivot.create_hint")),
        );
        if let Some(problem) = &d.problem {
            content = content.child(
                div()
                    .text_color(gpui::rgb(0xd43f3f))
                    .child(gpui::SharedString::from(problem.clone())),
            );
        }
        let buttons = div()
            .mt_2()
            .flex()
            .justify_end()
            .gap_2()
            .child(self.pivot_button(
                "pivot-create-cancel",
                i18n::text("validation.cancel").into(),
                PivotAction::Cancel,
                cx,
            ))
            .child(self.pivot_button(
                "pivot-create-take",
                i18n::text("pivot.create").into(),
                PivotAction::Take,
                cx,
            ));
        self.pivot_modal(content, buttons)
    }
    fn pivot_creation_field(
        &self,
        f: PivotCreationField,
        d: &PivotCreation,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let key = match f {
            PivotCreationField::Source => "pivot.source",
            PivotCreationField::Name => "pivot.name",
            PivotCreationField::Destination => "pivot.destination",
            PivotCreationField::SheetName => "pivot.new_sheet_name",
        };
        let focused = d.focus == f;
        let store = d.input_bounds.clone();
        let value = if focused {
            let text = d.field();
            let at = text.caret();
            format!("{}│{}", &text.text[..at], &text.text[at..])
        } else {
            d.value(f).to_string()
        };
        div()
            .child(
                div()
                    .text_xs()
                    .text_color(theme::text_dim())
                    .child(i18n::text(key)),
            )
            .child(
                div()
                    .id(key)
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
                    .cursor_text()
                    .child(gpui::SharedString::from(value))
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
                            this.pivot_action(PivotAction::CreateFocus(f), cx);
                            cx.stop_propagation();
                        }),
                    ),
            )
    }
}

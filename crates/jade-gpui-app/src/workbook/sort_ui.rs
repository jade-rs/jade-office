// Shared native/browser Data ▸ Sort dialog.
#[derive(Clone, Copy)]
pub(super) enum SortAction {
    Column(usize, usize),
    Order(usize, bool),
    AddLevel,
    RemoveLevel(usize),
    Header,
    Case,
    Take,
    Cancel,
}
impl View {
    pub(super) fn sort_panel(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let dialog = self.sort_editor()?;
        let names = dialog.column_names();
        let mut levels = div()
            .id("sort-levels")
            .max_h(px(260.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_2();
        for (index, level) in dialog.levels.iter().enumerate() {
            let mut columns = div().flex().flex_wrap().gap_1();
            for (column, name) in names.iter().enumerate() {
                columns = columns.child(self.sort_chip(
                    ("sort-column", index * 1000 + column),
                    name.clone(),
                    level.column == column,
                    SortAction::Column(index, column),
                    cx,
                ));
            }
            let mut order = div()
                .flex()
                .gap_1()
                .child(self.sort_chip(
                    ("sort-ascending", index),
                    i18n::text("sort.ascending").into(),
                    !level.descending,
                    SortAction::Order(index, false),
                    cx,
                ))
                .child(self.sort_chip(
                    ("sort-descending", index),
                    i18n::text("sort.descending").into(),
                    level.descending,
                    SortAction::Order(index, true),
                    cx,
                ));
            if dialog.levels.len() > 1 {
                order = order.child(self.sort_chip(
                    ("sort-remove", index),
                    i18n::text("sort.remove_level").into(),
                    false,
                    SortAction::RemoveLevel(index),
                    cx,
                ));
            }
            levels = levels.child(
                div()
                    .child(
                        div()
                            .mb_1()
                            .text_xs()
                            .text_color(theme::text_dim())
                            .child(i18n::text(if index == 0 {
                                "sort.sort_by"
                            } else {
                                "sort.then_by"
                            })),
                    )
                    .child(columns)
                    .child(div().mt_1().child(order)),
            );
        }
        let panel = div()
            .w(px(460.))
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
                    .mb_2()
                    .child(i18n::text("sort.dialog")),
            )
            .child(
                div()
                    .mb_2()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .child(self.sort_chip(
                        "sort-add",
                        i18n::text("sort.add_level").into(),
                        false,
                        SortAction::AddLevel,
                        cx,
                    ))
                    .child(self.sort_chip(
                        "sort-header",
                        i18n::text("sort.header").into(),
                        dialog.header,
                        SortAction::Header,
                        cx,
                    ))
                    .child(self.sort_chip(
                        "sort-case",
                        i18n::text("sort.case").into(),
                        dialog.case_sensitive,
                        SortAction::Case,
                        cx,
                    )),
            )
            .child(levels)
            .child(
                div()
                    .mt_4()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(self.sort_chip(
                        "sort-cancel",
                        i18n::text("validation.cancel").into(),
                        false,
                        SortAction::Cancel,
                        cx,
                    ))
                    .child(self.sort_chip(
                        "sort-ok",
                        i18n::text("validation.ok").into(),
                        false,
                        SortAction::Take,
                        cx,
                    )),
            );
        Some(super::conditional::centred(panel))
    }

    fn sort_chip(
        &self,
        id: impl Into<gpui::ElementId>,
        label: String,
        on: bool,
        action: SortAction,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(id)
            .px_2()
            .py_1()
            .rounded_sm()
            .border_1()
            .border_color(if on { theme::accent() } else { theme::border() })
            .bg(if on { theme::selected() } else { theme::window() })
            .cursor_pointer()
            .child(gpui::SharedString::from(label))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.sort_action(action, cx);
                    cx.stop_propagation();
                }),
            )
    }
}

/// What a press in the dialog does to it; OK and Cancel are the view's.
fn change_sort_dialog(dialog: &mut SortDialog, action: SortAction) {
    match action {
        SortAction::Column(level, column) => {
            if let Some(level) = dialog.levels.get_mut(level) {
                level.column = column;
            }
        }
        SortAction::Order(level, descending) => {
            if let Some(level) = dialog.levels.get_mut(level) {
                level.descending = descending;
            }
        }
        SortAction::AddLevel => dialog.add_level(),
        SortAction::RemoveLevel(level) => dialog.remove_level(level),
        SortAction::Header => dialog.header = !dialog.header,
        SortAction::Case => dialog.case_sensitive = !dialog.case_sensitive,
        SortAction::Take | SortAction::Cancel => {}
    }
}

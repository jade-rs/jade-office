// Shared native/browser Name Manager dialog.
#[derive(Clone, Copy)]
pub(super) enum NameManagerAction {
    /// A name picked from the list, by its row.
    Select(usize),
    /// The selected name made to refer to what is typed.
    Take,
    Delete,
    Close,
}
impl View {
    pub(super) fn name_manager_panel(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let (dialog, rows, sheets) = self.name_manager_editor()?;
        let header = div()
            .flex()
            .gap_2()
            .px_1()
            .text_xs()
            .text_color(theme::text_dim())
            .child(div().w(px(110.)).child(i18n::text("name_manager.name")))
            .child(div().flex_1().child(i18n::text("name_manager.refers_to")))
            .child(div().w(px(80.)).child(i18n::text("name_manager.scope")));
        let list = div()
            .id("name-manager-list")
            .mt_1()
            .h(px(160.))
            .overflow_y_scroll()
            .border_1()
            .border_color(theme::border())
            .bg(theme::window())
            .when(rows.is_empty(), |list| {
                list.child(
                    div()
                        .p_2()
                        .text_color(theme::text_dim())
                        .child(i18n::text("name_manager.none")),
                )
            })
            .children(rows.iter().enumerate().map(|(index, row)| {
                let scope = match row.scope {
                    Some(sheet) => sheets.get(sheet).cloned().unwrap_or_default(),
                    None => i18n::text("name_manager.workbook").to_string(),
                };
                let selected = dialog.selected == Some(index);
                div()
                    .id(("name-manager-row", index))
                    .flex()
                    .gap_2()
                    .px_1()
                    .py(px(2.))
                    .cursor_pointer()
                    .when(selected, |row| row.bg(theme::selected()))
                    .child(div().w(px(110.)).truncate().child(row.name.clone()))
                    .child(div().flex_1().truncate().child(row.refers_to.clone()))
                    .child(div().w(px(80.)).truncate().child(scope))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.name_manager_action(NameManagerAction::Select(index), cx);
                            cx.stop_propagation();
                        }),
                    )
            }));
        let field = div()
            .mt_1()
            .h(px(24.))
            .px_1()
            .flex()
            .items_center()
            .rounded_sm()
            .border_1()
            .border_color(theme::accent())
            .bg(theme::window())
            .overflow_hidden()
            .cursor_text()
            .child(super::caret_line(
                &dialog.refers_to,
                theme::accent().into(),
                false,
            ));
        let chip = |id: &'static str, key: &'static str, action: NameManagerAction| {
            div()
                .id(id)
                .px_2()
                .py_1()
                .rounded_sm()
                .border_1()
                .border_color(theme::border())
                .bg(theme::window())
                .cursor_pointer()
                .child(i18n::text(key))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.name_manager_action(action, cx);
                        cx.stop_propagation();
                    }),
                )
        };
        let panel = div()
            .w(px(520.))
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
                    .mb_3()
                    .child(i18n::text("name_manager.dialog")),
            )
            .child(header)
            .child(list)
            .child(
                div()
                    .mt_3()
                    .text_xs()
                    .text_color(theme::text_dim())
                    .child(i18n::text("name_manager.refers_to")),
            )
            .child(field)
            .child(
                div()
                    .mt_4()
                    .flex()
                    .justify_between()
                    .child(chip(
                        "name-manager-delete",
                        "name_manager.delete",
                        NameManagerAction::Delete,
                    ))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(chip(
                                "name-manager-apply",
                                "name_manager.apply",
                                NameManagerAction::Take,
                            ))
                            .child(chip(
                                "name-manager-close",
                                "name_manager.close",
                                NameManagerAction::Close,
                            )),
                    ),
            );
        Some(super::conditional::centred(panel))
    }
}

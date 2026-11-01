// Shared native/browser Name Manager dialog.
#[derive(Clone, Copy)]
pub(super) enum NameManagerAction {
    /// A name picked from the list, by its row.
    Select(usize),
    /// The selected name made to refer to what is typed.
    Take,
    Delete,
    Close,
    /// Filter: which names the list shows.
    Filter(NameFilter),
    /// New…: New Name opened on the selection.
    New,
    /// New Name's Cancel.
    CancelNew,
    /// New Name's Scope: a sheet by index, or the workbook.
    Scope(Option<usize>),
    /// Which New Name field takes typing: the name, or the reference.
    Focus(bool),
}
impl View {
    pub(super) fn name_manager_panel(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let (dialog, rows, sheets) = self.name_manager_editor()?;
        if let Some(new) = dialog.new.as_ref() {
            return Some(self.new_name_panel(dialog, new, &sheets, cx));
        }
        let filters = div()
            .flex()
            .flex_wrap()
            .gap_1()
            .mb_2()
            .text_xs()
            .children(NameFilter::ALL.into_iter().map(|filter| {
                let chosen = dialog.filter == filter;
                div()
                    .id(filter.label())
                    .px_2()
                    .py(px(2.))
                    .rounded_sm()
                    .border_1()
                    .border_color(if chosen {
                        theme::accent()
                    } else {
                        theme::border()
                    })
                    .when(chosen, |chip| chip.bg(theme::selected()))
                    .cursor_pointer()
                    .child(i18n::text(filter.label()))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.name_manager_action(NameManagerAction::Filter(filter), cx);
                            cx.stop_propagation();
                        }),
                    )
            }));
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
            .child(filters)
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
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(chip(
                                "name-manager-new",
                                "name_manager.new",
                                NameManagerAction::New,
                            ))
                            .child(chip(
                                "name-manager-delete",
                                "name_manager.delete",
                                NameManagerAction::Delete,
                            )),
                    )
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

    /// New Name: the name, its scope (the workbook or one sheet) and what
    /// it refers to, OK making it and Cancel going back to the list.
    fn new_name_panel(
        &self,
        dialog: &NameManager,
        new: &NewName,
        sheets: &[String],
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let field = |id: &'static str, text: &TextEdit, on_name: bool| {
            let focused = new.on_name == on_name;
            div()
                .id(id)
                .mt_1()
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
                    div().whitespace_nowrap().child(text.text.clone())
                })
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.name_manager_action(NameManagerAction::Focus(on_name), cx);
                        cx.stop_propagation();
                    }),
                )
        };
        let label = |key: &'static str| {
            div()
                .mt_3()
                .text_xs()
                .text_color(theme::text_dim())
                .child(i18n::text(key))
        };
        let scopes = std::iter::once((None, i18n::text("name_manager.workbook").to_string()))
            .chain(
                sheets
                    .iter()
                    .enumerate()
                    .map(|(index, name)| (Some(index), name.clone())),
            )
            .map(|(scope, name)| {
                let chosen = new.scope == scope;
                div()
                    .id(("name-manager-scope", scope.map_or(0, |index| index + 1)))
                    .px_2()
                    .py(px(2.))
                    .rounded_sm()
                    .border_1()
                    .border_color(if chosen {
                        theme::accent()
                    } else {
                        theme::border()
                    })
                    .when(chosen, |chip| chip.bg(theme::selected()))
                    .cursor_pointer()
                    .child(name)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.name_manager_action(NameManagerAction::Scope(scope), cx);
                            cx.stop_propagation();
                        }),
                    )
            });
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
                    .mb_1()
                    .child(i18n::text("name_manager.new_dialog")),
            )
            .child(label("name_manager.name"))
            .child(field("name-manager-new-name", &new.name, true))
            .child(label("name_manager.scope"))
            .child(div().mt_1().flex().flex_wrap().gap_1().text_xs().children(scopes))
            .child(label("name_manager.refers_to"))
            .child(field("name-manager-new-refers", &dialog.refers_to, false))
            .child(
                div()
                    .mt_4()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(chip(
                        "name-manager-new-ok",
                        "name_manager.ok",
                        NameManagerAction::Take,
                    ))
                    .child(chip(
                        "name-manager-new-cancel",
                        "name_manager.cancel",
                        NameManagerAction::CancelNew,
                    )),
            );
        super::conditional::centred(panel)
    }
}

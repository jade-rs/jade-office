// Shared native/browser Custom Filter dialog.
#[derive(Clone, Copy)]
pub(super) enum CustomFilterAction {
    /// The first condition's comparison, or (true) the second's.
    Test(bool, FilterTest),
    /// And (true) or Or between them.
    And(bool),
    /// Typing into the second value (true) or the first.
    Field(bool),
    Take,
    Cancel,
}
impl View {
    pub(super) fn custom_filter_panel(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let dialog = self.custom_filter_editor()?;
        let tests = |second: bool, chosen: FilterTest, cx: &mut Context<Self>| {
            div()
                .flex()
                .flex_wrap()
                .gap_1()
                .children(FilterTest::ALL.into_iter().enumerate().map(|(index, test)| {
                    self.custom_filter_chip(
                        ("custom-filter-test", index + usize::from(second) * 100),
                        i18n::text(test.label()).into(),
                        chosen == test,
                        CustomFilterAction::Test(second, test),
                        cx,
                    )
                }))
        };
        let field = |second: bool, cx: &mut Context<Self>| {
            let text = if second { &dialog.second } else { &dialog.value };
            let typing = dialog.in_second == second;
            div()
                .id(("custom-filter-field", usize::from(second)))
                .mt_2()
                .h(px(24.))
                .px_1()
                .flex()
                .items_center()
                .rounded_sm()
                .border_1()
                .border_color(if typing { theme::accent() } else { theme::border() })
                .bg(theme::window())
                .overflow_hidden()
                .cursor_text()
                .map(|input| {
                    if typing {
                        input.child(super::caret_line(text, theme::accent().into(), false))
                    } else {
                        input.child(gpui::SharedString::from(text.text.clone()))
                    }
                })
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.custom_filter_action(CustomFilterAction::Field(second), cx);
                        cx.stop_propagation();
                    }),
                )
        };
        let joins = div()
            .my_3()
            .flex()
            .gap_1()
            .child(self.custom_filter_chip(
                "custom-filter-and",
                i18n::text("filter.custom.and").into(),
                dialog.and,
                CustomFilterAction::And(true),
                cx,
            ))
            .child(self.custom_filter_chip(
                "custom-filter-or",
                i18n::text("filter.custom.or").into(),
                !dialog.and,
                CustomFilterAction::And(false),
                cx,
            ));
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
                    .child(i18n::text("filter.custom.dialog")),
            )
            .child(tests(false, dialog.test, cx))
            .child(field(false, cx))
            .child(joins)
            .child(tests(true, dialog.second_test, cx))
            .child(field(true, cx))
            .child(
                div()
                    .mt_4()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(self.custom_filter_chip(
                        "custom-filter-cancel",
                        i18n::text("validation.cancel").into(),
                        false,
                        CustomFilterAction::Cancel,
                        cx,
                    ))
                    .child(self.custom_filter_chip(
                        "custom-filter-ok",
                        i18n::text("validation.ok").into(),
                        false,
                        CustomFilterAction::Take,
                        cx,
                    )),
            );
        Some(super::conditional::centred(panel))
    }

    fn custom_filter_chip(
        &self,
        id: impl Into<gpui::ElementId>,
        label: String,
        on: bool,
        action: CustomFilterAction,
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
                    this.custom_filter_action(action, cx);
                    cx.stop_propagation();
                }),
            )
    }
}

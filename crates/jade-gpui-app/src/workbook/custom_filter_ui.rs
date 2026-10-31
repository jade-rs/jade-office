// Shared native/browser Custom Filter dialog.
#[derive(Clone, Copy)]
pub(super) enum CustomFilterAction {
    Test(FilterTest),
    Take,
    Cancel,
}
impl View {
    pub(super) fn custom_filter_panel(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let dialog = self.custom_filter_editor()?;
        let tests = div()
            .flex()
            .flex_wrap()
            .gap_1()
            .children(FilterTest::ALL.into_iter().map(|test| {
                self.custom_filter_chip(
                    test.label(),
                    i18n::text(test.label()).into(),
                    dialog.test == test,
                    CustomFilterAction::Test(test),
                    cx,
                )
            }));
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
                    .mb_2()
                    .child(i18n::text("filter.custom.dialog")),
            )
            .child(tests)
            .child(
                div()
                    .mt_3()
                    .h(px(24.))
                    .px_1()
                    .flex()
                    .items_center()
                    .rounded_sm()
                    .border_1()
                    .border_color(theme::accent())
                    .bg(theme::window())
                    .overflow_hidden()
                    .child(super::caret_line(&dialog.value, theme::accent().into(), false)),
            )
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

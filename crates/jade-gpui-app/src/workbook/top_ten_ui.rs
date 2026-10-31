// Shared native/browser Top 10 dialog.
#[derive(Clone, Copy)]
pub(super) enum TopTenAction {
    Top(bool),
    Step(i32),
    Percent(bool),
    Take,
    Cancel,
}
impl View {
    pub(super) fn top_ten_panel(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let dialog = self.top_ten_editor()?;
        let chip = |id: &'static str, label: String, on: bool, action: TopTenAction| {
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
                        this.top_ten_action(action, cx);
                        cx.stop_propagation();
                    }),
                )
        };
        let panel = div()
            .w(px(380.))
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
                    .child(i18n::text("filter.top_ten.dialog")),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .child(chip(
                        "top-ten-top",
                        i18n::text("filter.top_ten.top").into(),
                        dialog.top,
                        TopTenAction::Top(true),
                    ))
                    .child(chip(
                        "top-ten-bottom",
                        i18n::text("filter.top_ten.bottom").into(),
                        !dialog.top,
                        TopTenAction::Top(false),
                    ))
                    .child(chip("top-ten-less", "−".into(), false, TopTenAction::Step(-1)))
                    .child(chip(
                        "top-ten-count",
                        dialog.count.to_string(),
                        true,
                        TopTenAction::Step(0),
                    ))
                    .child(chip("top-ten-more", "+".into(), false, TopTenAction::Step(1)))
                    .child(chip(
                        "top-ten-items",
                        i18n::text("filter.top_ten.items").into(),
                        !dialog.percent,
                        TopTenAction::Percent(false),
                    ))
                    .child(chip(
                        "top-ten-percent",
                        i18n::text("filter.top_ten.percent").into(),
                        dialog.percent,
                        TopTenAction::Percent(true),
                    )),
            )
            .child(
                div()
                    .mt_4()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(chip(
                        "top-ten-cancel",
                        i18n::text("validation.cancel").into(),
                        false,
                        TopTenAction::Cancel,
                    ))
                    .child(chip(
                        "top-ten-ok",
                        i18n::text("validation.ok").into(),
                        false,
                        TopTenAction::Take,
                    )),
            );
        Some(super::conditional::centred(panel))
    }
}

/// What a press in the dialog does to it; OK and Cancel are the view's.
fn change_top_ten(dialog: &mut TopTenDialog, action: TopTenAction) {
    match action {
        TopTenAction::Top(top) => dialog.top = top,
        TopTenAction::Step(by) => dialog.step(by),
        TopTenAction::Percent(percent) => dialog.set_percent(percent),
        TopTenAction::Take | TopTenAction::Cancel => {}
    }
}

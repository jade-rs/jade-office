// Shared native/browser More Colors dialog.
#[derive(Clone, Copy)]
pub(super) enum MoreColorsAction {
    Take,
    Cancel,
}
impl View {
    pub(super) fn more_colors_panel(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let dialog = self.more_colors_editor()?;
        let color = dialog.color();
        let button = |id: &'static str, key: &'static str, action: MoreColorsAction| {
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
                        this.more_colors_action(action, cx);
                        cx.stop_propagation();
                    }),
                )
        };
        let panel = div()
            .w(px(320.))
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
                    .child(i18n::text("sheet.tool.more_colors_dialog")),
            )
            .child(
                div()
                    .mb_1()
                    .text_xs()
                    .text_color(theme::text_dim())
                    .child(i18n::text("sheet.tool.more_colors_code")),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .h(px(24.))
                            .px_1()
                            .flex()
                            .items_center()
                            .rounded_sm()
                            .border_1()
                            .border_color(theme::accent())
                            .bg(theme::window())
                            .overflow_hidden()
                            .child(super::caret_line(&dialog.code, theme::accent().into(), false)),
                    )
                    .child(
                        div()
                            .size(px(24.))
                            .rounded_sm()
                            .border_1()
                            .border_color(theme::border())
                            .when_some(color, |swatch, color| {
                                swatch.bg(gpui::rgb(u32::from_str_radix(&color, 16).unwrap_or(0)))
                            }),
                    ),
            )
            .child(
                div()
                    .mt_4()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(button(
                        "more-colors-cancel",
                        "validation.cancel",
                        MoreColorsAction::Cancel,
                    ))
                    .child(button("more-colors-ok", "validation.ok", MoreColorsAction::Take)),
            );
        Some(super::conditional::centred(panel))
    }
}

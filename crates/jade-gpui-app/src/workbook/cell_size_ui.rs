// Shared native/browser Row Height and Column Width dialogs.
#[derive(Clone, Copy)]
pub(super) enum SizeAction {
    Take,
    Cancel,
}
impl View {
    pub(super) fn size_panel(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let dialog = self.size_editor()?;
        let (title, label) = match dialog.axis {
            Axis::Rows => ("sheet.size.row_height", "sheet.size.row_height_label"),
            Axis::Columns => ("sheet.size.column_width", "sheet.size.column_width_label"),
        };
        let button = |id: &'static str, key: &'static str, action: SizeAction| {
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
                        this.size_action(action, cx);
                        cx.stop_propagation();
                    }),
                )
        };
        let panel = div()
            .w(px(260.))
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
                    .child(i18n::text(title)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(i18n::text(label))
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
                            .child(super::caret_line(
                                &dialog.value,
                                theme::accent().into(),
                                false,
                            )),
                    ),
            )
            .when(dialog.problem, |panel| {
                panel.child(
                    div()
                        .mt_2()
                        .text_xs()
                        .text_color(theme::accent())
                        .child(i18n::format(
                            "sheet.size.out_of_range",
                            &[("most", &dialog.most())],
                        )),
                )
            })
            .child(
                div()
                    .mt_4()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(button("size-cancel", "validation.cancel", SizeAction::Cancel))
                    .child(button("size-ok", "validation.ok", SizeAction::Take)),
            );
        Some(super::conditional::centred(panel))
    }
}

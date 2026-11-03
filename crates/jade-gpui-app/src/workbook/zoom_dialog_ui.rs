// Shared native/browser Zoom dialog.
#[derive(Clone, Copy)]
pub(super) enum ZoomAction {
    Choose(ZoomChoice),
    Take,
    Cancel,
}
impl View {
    pub(super) fn zoom_dialog_panel(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let dialog = self.zoom_dialog_editor()?;
        let chosen = dialog.choice();
        let chip = |id: gpui::ElementId, label: gpui::SharedString, on: bool, action: ZoomAction| {
            div()
                .id(id)
                .px_2()
                .py_1()
                .rounded_sm()
                .border_1()
                .border_color(if on { theme::accent() } else { theme::border() })
                .bg(if on { theme::selected() } else { theme::window() })
                .cursor_pointer()
                .child(label)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.zoom_dialog_action(action, cx);
                        cx.stop_propagation();
                    }),
                )
        };
        let mut choices = div().flex().flex_col().gap_1();
        for percent in ZoomDialog::PRESETS {
            let choice = ZoomChoice::Percent(percent);
            choices = choices.child(chip(
                gpui::ElementId::Name(format!("zoom-preset-{percent}").into()),
                format!("{percent}%").into(),
                chosen == choice,
                ZoomAction::Choose(choice),
            ));
        }
        choices = choices
            .child(chip(
                "zoom-fit-selection".into(),
                i18n::text("sheet.zoom.fit_selection").into(),
                chosen == ZoomChoice::FitSelection,
                ZoomAction::Choose(ZoomChoice::FitSelection),
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(chip(
                        "zoom-custom".into(),
                        i18n::text("sheet.zoom.custom").into(),
                        chosen == ZoomChoice::Custom,
                        ZoomAction::Choose(ZoomChoice::Custom),
                    ))
                    .child(
                        div()
                            .w(px(64.))
                            .h(px(24.))
                            .px_1()
                            .flex()
                            .items_center()
                            .rounded_sm()
                            .border_1()
                            .border_color(if chosen == ZoomChoice::Custom {
                                theme::accent()
                            } else {
                                theme::border()
                            })
                            .bg(theme::window())
                            .overflow_hidden()
                            .child(super::caret_line(
                                &dialog.custom,
                                theme::accent().into(),
                                false,
                            )),
                    )
                    .child("%"),
            );
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
                    .child(i18n::text("sheet.zoom.title")),
            )
            .child(
                div()
                    .mb_1()
                    .text_xs()
                    .text_color(theme::text_dim())
                    .child(i18n::text("sheet.zoom.magnification")),
            )
            .child(choices)
            .when(dialog.problem, |panel| {
                panel.child(
                    div()
                        .mt_2()
                        .text_xs()
                        .text_color(theme::accent())
                        .child(i18n::text("sheet.zoom.out_of_range")),
                )
            })
            .child(
                div()
                    .mt_4()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(chip(
                        "zoom-cancel".into(),
                        i18n::text("validation.cancel").into(),
                        false,
                        ZoomAction::Cancel,
                    ))
                    .child(chip(
                        "zoom-ok".into(),
                        i18n::text("validation.ok").into(),
                        false,
                        ZoomAction::Take,
                    )),
            );
        Some(super::conditional::centred(panel))
    }
}

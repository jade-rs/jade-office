// Shared native/browser hyperlink dialog.
#[derive(Clone, Copy)]
pub(super) enum LinkAction {
    Focus(LinkField),
    Place(usize),
    Take,
    Cancel,
}
impl View {
    pub(super) fn link_panel(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let dialog = self.link_editor()?;
        let mut panel = div()
            .w(px(440.))
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
                    .child(i18n::text("link.dialog")),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme::text_dim())
                    .child(i18n::text("link.address_hint")),
            );
        for (i, field) in LinkField::ALL.into_iter().enumerate() {
            let focused = dialog.field == field;
            let store = dialog.input_bounds.clone();
            let value = if focused {
                let text = dialog.field();
                let at = text.caret();
                format!("{}│{}", &text.text[..at], &text.text[at..])
            } else {
                dialog.value(field).to_string()
            };
            panel = panel.child(
                div()
                    .mt_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::text_dim())
                            .child(i18n::text(field.key())),
                    )
                    .child(
                        div()
                            .id(("link-field", i))
                            .mt_1()
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
                            .child(gpui::SharedString::from(value))
                            .cursor_text()
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
                                    this.link_action(LinkAction::Focus(field), cx);
                                    cx.stop_propagation();
                                }),
                            ),
                    ),
            );
        }
        if !dialog.places.is_empty() {
            let mut places = div()
                .id("link-places")
                .max_h(px(100.))
                .overflow_y_scroll()
                .flex()
                .flex_wrap()
                .gap_1()
                .mt_2();
            for (i, (name, _)) in dialog.places.iter().enumerate() {
                places = places.child(self.link_button(
                    ("link-place", i),
                    name.clone(),
                    LinkAction::Place(i),
                    cx,
                ));
            }
            panel = panel
                .child(div().mt_2().text_xs().child(i18n::text("link.places")))
                .child(places);
        }
        if let Some(problem) = dialog.problem {
            panel = panel.child(
                div()
                    .mt_2()
                    .text_color(gpui::rgb(0xd43f3f))
                    .child(i18n::text(problem)),
            );
        }
        panel = panel.child(
            div()
                .mt_4()
                .flex()
                .justify_end()
                .gap_2()
                .child(self.link_button(
                    "link-cancel",
                    i18n::text("validation.cancel").into(),
                    LinkAction::Cancel,
                    cx,
                ))
                .child(self.link_button(
                    "link-ok",
                    i18n::text("validation.ok").into(),
                    LinkAction::Take,
                    cx,
                )),
        );
        Some(super::conditional::centred(panel))
    }
    fn link_button(
        &self,
        id: impl Into<gpui::ElementId>,
        label: String,
        action: LinkAction,
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
                    this.link_action(action, cx);
                    cx.stop_propagation();
                }),
            )
    }
}

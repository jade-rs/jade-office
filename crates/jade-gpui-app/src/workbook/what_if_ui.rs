// Shared native/browser: the What-If Analysis dialogs and the analysis
// tools beside them, each a form the model describes and answers.
#[derive(Clone, Copy)]
pub(super) enum WhatIfAction {
    /// A click on the form's control at `.0`, on its option or line `.1`.
    Control(usize, usize),
    /// A button, by its place along the bottom.
    Button(usize),
}

/// A form's words in the user's language.
fn what_if_text(label: &Label) -> String {
    match label {
        Label::Key(key) => i18n::text(key).to_string(),
        Label::Text(text) => text.clone(),
        Label::Format(key, values) => {
            let values: Vec<(&str, &dyn std::fmt::Display)> = values
                .iter()
                .map(|(name, value)| (*name, value as &dyn std::fmt::Display))
                .collect();
            i18n::format(key, &values)
        }
    }
}

impl View {
    pub(super) fn what_if_panel(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let form = self.what_if_editor()?.form();
        let caption = |label: &Label| {
            div()
                .mt_2()
                .mb_1()
                .text_xs()
                .text_color(theme::text_dim())
                .child(what_if_text(label))
        };
        let mut body = div()
            .id("what-if-body")
            .max_h(px(440.))
            .overflow_y_scroll();
        for (index, item) in form.items.iter().enumerate() {
            body = body.child(match &item.control {
                Control::Field { label, edit } => {
                    let typing = form.focus == Some(index);
                    div().child(caption(label)).child(
                        div()
                            .id(("what-if-field", index))
                            .h(px(24.))
                            .px_1()
                            .flex()
                            .items_center()
                            .rounded_sm()
                            .border_1()
                            .border_color(if typing {
                                theme::accent()
                            } else {
                                theme::border()
                            })
                            .bg(theme::window())
                            .overflow_hidden()
                            .cursor_text()
                            .map(|input| {
                                if typing {
                                    input.child(super::caret_line(
                                        edit,
                                        theme::accent().into(),
                                        false,
                                    ))
                                } else {
                                    input.child(gpui::SharedString::from(edit.text.clone()))
                                }
                            })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    this.what_if_action(WhatIfAction::Control(index, 0), cx);
                                    cx.stop_propagation();
                                }),
                            ),
                    )
                }
                Control::Choice {
                    label,
                    options,
                    chosen,
                } => div().child(caption(label)).child(
                    div().flex().flex_wrap().gap_1().children(
                        options.iter().enumerate().map(|(option, name)| {
                            self.what_if_chip(
                                ("what-if-option", index * 1000 + option),
                                what_if_text(name),
                                *chosen == option,
                                WhatIfAction::Control(index, option),
                                cx,
                            )
                        }),
                    ),
                ),
                Control::Check { label, on } => div().mt_2().flex().child(self.what_if_chip(
                    ("what-if-check", index),
                    format!("{} {}", if *on { "☑" } else { "☐" }, what_if_text(label)),
                    *on,
                    WhatIfAction::Control(index, 0),
                    cx,
                )),
                Control::List {
                    label,
                    items,
                    chosen,
                } => div().child(caption(label)).child(
                    div()
                        .id(("what-if-list", index))
                        .h(px(120.))
                        .overflow_y_scroll()
                        .rounded_sm()
                        .border_1()
                        .border_color(theme::border())
                        .bg(theme::window())
                        .children(items.iter().enumerate().map(|(line, text)| {
                            div()
                                .id(("what-if-line", index * 10000 + line))
                                .px_1()
                                .whitespace_nowrap()
                                .cursor_pointer()
                                .when(*chosen == Some(line), |row| row.bg(theme::selected()))
                                .child(gpui::SharedString::from(text.clone()))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        this.what_if_action(
                                            WhatIfAction::Control(index, line),
                                            cx,
                                        );
                                        cx.stop_propagation();
                                    }),
                                )
                        })),
                ),
                Control::Note(label) => div().mt_1().child(what_if_text(label)),
            });
        }
        let mut buttons = div().mt_4().flex().flex_wrap().justify_end().gap_2();
        for (index, (_, label)) in form.buttons.iter().enumerate() {
            buttons = buttons.child(self.what_if_chip(
                ("what-if-button", index),
                what_if_text(label),
                false,
                WhatIfAction::Button(index),
                cx,
            ));
        }
        let panel = div()
            .w(px(480.))
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
                    .child(what_if_text(&form.title)),
            )
            .child(body)
            .children(form.alert.as_ref().map(|alert| {
                div()
                    .mt_3()
                    .text_color(gpui::rgb(0xc4_2b_1c))
                    .child(what_if_text(alert))
            }))
            .child(buttons);
        Some(super::conditional::centred(panel))
    }

    fn what_if_chip(
        &self,
        id: impl Into<gpui::ElementId>,
        label: String,
        on: bool,
        action: WhatIfAction,
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
                    this.what_if_action(action, cx);
                    cx.stop_propagation();
                }),
            )
    }
}

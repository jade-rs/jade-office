// Shared rendering for the native and browser shells. The including module
// supplies View, the theme alias and state/controller accessors.
#[derive(Clone, Copy)]
pub(super) enum ValidationAction {
    Tab(ValidationTab),
    Focus(ValidationField),
    Kind,
    Operator,
    Style,
    Ime,
    Blank,
    Same,
    Dropdown,
    Input,
    Error,
    Take,
    Clear,
    Cancel,
}
#[derive(Clone, Copy)]
pub(super) enum ValidationAnswer {
    Accept,
    Retry,
    Cancel,
}

impl View {
    pub(super) fn validation_panel(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let dialog = self.validation_editor()?;
        let mut panel = div()
            .w(px(440.0))
            .p_4()
            .rounded_md()
            .bg(theme::panel())
            .border_1()
            .border_color(theme::border())
            .shadow_lg()
            .text_sm()
            .text_color(theme::text())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
        panel = panel.child(
            div()
                .font_weight(gpui::FontWeight::BOLD)
                .mb_2()
                .child(i18n::text("validation.dialog")),
        );
        let mut tabs = div().flex().gap_2().mb_3();
        for (i, (tab, label)) in [
            (ValidationTab::Settings, "validation.settings"),
            (ValidationTab::Input, "validation.input_tab"),
            (ValidationTab::Error, "validation.error_tab"),
        ]
        .into_iter()
        .enumerate()
        {
            tabs = tabs.child(self.validation_button(
                ("validation-tab", i),
                i18n::text(label).into(),
                dialog.tab == tab,
                ValidationAction::Tab(tab),
                cx,
            ));
        }
        panel = panel.child(tabs);
        let selector = |label: &'static str,
                        value: String,
                        action: ValidationAction,
                        cx: &mut Context<Self>| {
            div()
                .flex()
                .items_center()
                .justify_between()
                .mb_2()
                .child(i18n::text(label))
                .child(self.validation_button(label, value.into(), false, action, cx))
        };
        let tick =
            |label: &'static str, on: bool, action: ValidationAction, cx: &mut Context<Self>| {
                self.validation_button(
                    label,
                    format!("{} {}", if on { "☑" } else { "☐" }, i18n::text(label)).into(),
                    false,
                    action,
                    cx,
                )
            };
        match dialog.tab {
            ValidationTab::Settings => {
                panel = panel.child(selector(
                    "validation.allow",
                    i18n::text(TYPES[dialog.kind].1).to_string(),
                    ValidationAction::Kind,
                    cx,
                ));
                if !matches!(TYPES[dialog.kind].0, "none" | "list" | "custom") {
                    panel = panel.child(selector(
                        "validation.data",
                        i18n::text(OPERATORS[dialog.operator].1).to_string(),
                        ValidationAction::Operator,
                        cx,
                    ));
                }
                panel = panel.child(
                    div()
                        .flex()
                        .gap_2()
                        .mb_2()
                        .child(tick(
                            "validation.ignore_blank",
                            dialog.allow_blank,
                            ValidationAction::Blank,
                            cx,
                        ))
                        .when(TYPES[dialog.kind].0 == "list", |row| {
                            row.child(tick(
                                "validation.dropdown",
                                dialog.dropdown,
                                ValidationAction::Dropdown,
                                cx,
                            ))
                        }),
                );
                if dialog.has_matching() {
                    panel = panel.child(tick(
                        "validation.apply_same",
                        dialog.apply_to_same,
                        ValidationAction::Same,
                        cx,
                    ));
                }
                const IME_KEYS: [&str; 11] = [
                    "validation.ime.none",
                    "validation.ime.off",
                    "validation.ime.on",
                    "validation.ime.disabled",
                    "validation.ime.hiragana",
                    "validation.ime.katakana",
                    "validation.ime.half_katakana",
                    "validation.ime.full_alpha",
                    "validation.ime.half_alpha",
                    "validation.ime.hangul",
                    "validation.ime.half_hangul",
                ];
                panel = panel.child(selector(
                    "validation.ime",
                    i18n::text(IME_KEYS[dialog.ime]).to_string(),
                    ValidationAction::Ime,
                    cx,
                ));
            }
            ValidationTab::Input => {
                panel = panel.child(tick(
                    "validation.show_input",
                    dialog.show_input,
                    ValidationAction::Input,
                    cx,
                ))
            }
            ValidationTab::Error => {
                panel = panel.child(tick(
                    "validation.show_error",
                    dialog.show_error,
                    ValidationAction::Error,
                    cx,
                ));
                panel = panel.child(selector(
                    "validation.alert_style",
                    i18n::text(STYLES[dialog.style].1).to_string(),
                    ValidationAction::Style,
                    cx,
                ));
            }
        }
        for (i, field) in dialog.visible_fields().into_iter().enumerate() {
            let value = if dialog.field == field {
                let text = dialog.field();
                let at = text.caret();
                format!("{}│{}", &text.text[..at], &text.text[at..])
            } else {
                dialog.value(field).to_string()
            };
            let store = dialog.input_bounds.clone();
            let focused = dialog.field == field;
            let label = if field == ValidationField::First && TYPES[dialog.kind].0 == "list" {
                "validation.source"
            } else {
                field.key()
            };
            panel = panel.child(
                div()
                    .mt_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::text_dim())
                            .child(i18n::text(label)),
                    )
                    .child(
                        div()
                            .id(("validation-field", i))
                            .mt_1()
                            .min_h(px(28.0))
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .border_1()
                            .border_color(if dialog.field == field {
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
                                .top_0()
                                .left_0()
                                .size_full(),
                            )
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.validation_action(ValidationAction::Focus(field), cx)
                            })),
                    ),
            );
        }
        if let Some(problem) = &dialog.problem {
            panel = panel.child(
                div()
                    .mt_2()
                    .text_color(gpui::rgb(0xd43f3f))
                    .child(gpui::SharedString::from(i18n::text(problem).to_string())),
            );
        }
        panel = panel.child(
            div()
                .mt_4()
                .flex()
                .justify_between()
                .child(self.validation_button(
                    "validation-clear",
                    i18n::text("validation.clear").into(),
                    false,
                    ValidationAction::Clear,
                    cx,
                ))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(self.validation_button(
                            "validation-cancel",
                            i18n::text("validation.cancel").into(),
                            false,
                            ValidationAction::Cancel,
                            cx,
                        ))
                        .child(self.validation_button(
                            "validation-ok",
                            i18n::text("validation.ok").into(),
                            true,
                            ValidationAction::Take,
                            cx,
                        )),
                ),
        );
        Some(super::conditional::centred(panel))
    }
    fn validation_button(
        &self,
        id: impl Into<gpui::ElementId>,
        label: gpui::SharedString,
        chosen: bool,
        action: ValidationAction,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(id)
            .px_2()
            .py_1()
            .rounded_sm()
            .border_1()
            .border_color(theme::border())
            .cursor_pointer()
            .when(chosen, |b| {
                b.bg(theme::accent()).text_color(gpui::rgb(0xffffff))
            })
            .hover(|b| b.bg(theme::hovered()))
            .child(label)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |view, _, _, cx| view.validation_action(action, cx)))
    }
    pub(super) fn validation_alert_panel(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let pending = self.validation_pending()?;
        let alert = &pending.alert;
        let title = alert
            .title
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(i18n::text("validation.dialog"));
        let message = if alert.unsupported {
            i18n::text("validation.unsupported")
        } else {
            alert
                .message
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or(i18n::text("sheet.validation_refused"))
        };
        let mut buttons = div().flex().gap_2().justify_end().mt_4();
        if alert.style != AlertStyle::Stop {
            buttons = buttons.child(self.validation_answer_button(
                "validation-accept",
                if alert.style == AlertStyle::Warning {
                    "validation.yes"
                } else {
                    "validation.ok"
                },
                ValidationAnswer::Accept,
                cx,
            ));
        }
        if alert.style != AlertStyle::Information {
            buttons = buttons.child(self.validation_answer_button(
                "validation-retry",
                if alert.style == AlertStyle::Warning {
                    "validation.no"
                } else {
                    "validation.retry"
                },
                ValidationAnswer::Retry,
                cx,
            ));
        }
        buttons = buttons.child(self.validation_answer_button(
            "validation-dismiss",
            "validation.cancel",
            ValidationAnswer::Cancel,
            cx,
        ));
        Some(super::conditional::centred(
            div()
                .w(px(420.0))
                .p_4()
                .rounded_md()
                .bg(theme::panel())
                .border_1()
                .border_color(theme::border())
                .shadow_lg()
                .text_color(theme::text())
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .font_weight(gpui::FontWeight::BOLD)
                        .mb_3()
                        .child(gpui::SharedString::from(title.to_string())),
                )
                .child(gpui::SharedString::from(message.to_string()))
                .child(buttons),
        ))
    }
    fn validation_answer_button(
        &self,
        id: &'static str,
        label: &'static str,
        answer: ValidationAnswer,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(id)
            .px_3()
            .py_1()
            .rounded_sm()
            .border_1()
            .border_color(theme::border())
            .cursor_pointer()
            .child(i18n::text(label))
            .hover(|b| b.bg(theme::hovered()))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |view, _, _, cx| view.answer_validation(answer, cx)))
    }
    pub(super) fn validation_input_panel(&self) -> Option<gpui::Div> {
        let (title, message) = self.validation_input_message()?;
        Some(
            div()
                .absolute()
                .right(px(12.0))
                .bottom(px(42.0))
                .w(px(260.0))
                .p_3()
                .rounded_sm()
                .border_1()
                .border_color(gpui::rgb(0xb5a76b))
                .bg(gpui::rgb(0xfffbe6))
                .text_color(gpui::rgb(0x202020))
                .text_sm()
                .child(
                    div()
                        .font_weight(gpui::FontWeight::BOLD)
                        .mb_1()
                        .child(gpui::SharedString::from(title)),
                )
                .child(gpui::SharedString::from(message)),
        )
    }
}

#[derive(Clone, Copy)]
pub(super) enum ProtectionAction {
    Focus(usize),
    Toggle(usize),
    Range(RangeAction),
    Take,
    Cancel,
}
impl View {
    pub(super) fn protection_panel(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let d = self.protection_editor()?;
        let mut panel = div()
            .w(px(400.))
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
                    .child(i18n::text(match (d.target, d.unprotect) {
                        (ProtectionTarget::Workbook, true) => "protection.unprotect",
                        (ProtectionTarget::Workbook, false) => "protection.protect",
                        (ProtectionTarget::Sheet, true) => "protection.sheet_unprotect",
                        (ProtectionTarget::Sheet, false) => "protection.sheet_protect",
                        (ProtectionTarget::Ranges, _) => {
                            if d.is_authentication_prompt() {
                                if d.is_operation_prompt() {
                                    "protection.range_operation_title"
                                } else {
                                    "protection.range_entry_title"
                                }
                            } else {
                                "protection.ranges_menu"
                            }
                        }
                    })),
            );
        if let Some(editor) = &d.range_editor {
            panel = panel.child(div().text_xs().text_color(theme::text_dim()).child(
                if let Some(cell) = d.entry_reference() {
                    i18n::format("protection.range_entry_hint", &[("cell", &cell)])
                } else if d.is_operation_prompt() {
                    i18n::text("protection.range_operation_hint").to_string()
                } else {
                    i18n::text(if editor.protected {
                        "protection.range_session_hint"
                    } else {
                        "protection.range_edit_hint"
                    })
                    .to_string()
                },
            ));
            let mut list = div()
                .id("editable-range-list")
                .mt_2()
                .max_h(px(150.))
                .overflow_y_scroll();
            if editor.entries.is_empty() {
                list = list.child(i18n::text("protection.range_empty"));
            }
            for (index, entry) in editor.entries.iter().enumerate() {
                let selected = editor.selected.as_deref() == Some(entry.key.as_str());
                let status = if entry.ambiguous {
                    "protection.range_invalid"
                } else if entry.unlocked {
                    "protection.range_unlocked"
                } else if entry.has_password {
                    "protection.range_password_set"
                } else {
                    "protection.range_passwordless"
                };
                list = list.child(
                    div()
                        .id(("editable-range", index))
                        .p_2()
                        .rounded_sm()
                        .bg(theme::window())
                        .border_1()
                        .border_color(if selected {
                            theme::accent()
                        } else {
                            theme::border()
                        })
                        .cursor_pointer()
                        .child(format!(
                            "{}: {} ({})",
                            entry.name,
                            entry.reference,
                            i18n::text(status)
                        ))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.workbook_protection_action(
                                    ProtectionAction::Range(RangeAction::Select(index)),
                                    cx,
                                );
                                cx.stop_propagation();
                            }),
                        ),
                );
            }
            panel = panel.child(list);
            if editor.protected && !d.is_authentication_prompt() {
                panel = panel.child(div().mt_2().child(self.protection_button(
                    "ranges-lock",
                    i18n::text("protection.range_lock"),
                    ProtectionAction::Range(RangeAction::LockAll),
                    cx,
                )));
            } else if !editor.protected {
                panel = panel.child(
                    div()
                        .mt_2()
                        .flex()
                        .gap_2()
                        .child(self.protection_button(
                            "ranges-new",
                            i18n::text("protection.range_new"),
                            ProtectionAction::Range(RangeAction::New),
                            cx,
                        ))
                        .child(self.protection_button(
                            "ranges-delete",
                            i18n::text("protection.range_delete"),
                            ProtectionAction::Range(RangeAction::Delete),
                            cx,
                        )),
                );
                if editor.selected.is_some() {
                    panel = panel.child(
                        div()
                            .mt_2()
                            .cursor_pointer()
                            .child(format!(
                                "{} {}",
                                if editor.replace_password {
                                    "☑"
                                } else {
                                    "☐"
                                },
                                i18n::text("protection.range_change_password")
                            ))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.workbook_protection_action(
                                        ProtectionAction::Range(RangeAction::Password),
                                        cx,
                                    );
                                    cx.stop_propagation();
                                }),
                            ),
                    );
                    if editor.replace_password {
                        panel = panel.child(
                            div()
                                .text_xs()
                                .text_color(theme::text_dim())
                                .child(i18n::text("protection.range_clear_password_hint")),
                        );
                    }
                }
            }
        }
        if !d.unprotect && d.range_editor.is_none() {
            panel = panel.child(
                div()
                    .text_xs()
                    .text_color(theme::text_dim())
                    .child(i18n::text(if d.target == ProtectionTarget::Sheet {
                        "protection.sheet_hint"
                    } else {
                        "protection.hint"
                    })),
            );
        }
        for i in d.visible_fields() {
            let focused = d.focused == i;
            let bounds = d.input_bounds.clone();
            panel = panel.child(
                div()
                    .mt_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::text_dim())
                            .child(i18n::text(if d.range_editor.is_some() {
                                match i {
                                    0 => "protection.range_title",
                                    1 => "protection.range_reference_label",
                                    2 => "protection.password",
                                    _ => "protection.confirm",
                                }
                            } else if i == 0 {
                                "protection.password"
                            } else {
                                "protection.confirm"
                            })),
                    )
                    .child(
                        div()
                            .id(("protection-field", i))
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
                            .cursor_text()
                            .child(gpui::SharedString::from(d.masked(i)))
                            .child(
                                gpui::canvas(
                                    move |rect, _, _| {
                                        if focused {
                                            bounds.set(Some([
                                                f32::from(rect.origin.x),
                                                f32::from(rect.origin.y),
                                                f32::from(rect.size.width),
                                                f32::from(rect.size.height),
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
                                    this.workbook_protection_action(ProtectionAction::Focus(i), cx);
                                    cx.stop_propagation();
                                }),
                            ),
                    ),
            );
        }
        if d.target == ProtectionTarget::Sheet && !d.unprotect {
            let labels = [
                "protection.allow_select_locked",
                "protection.allow_select_unlocked",
                "protection.allow_format_cells",
                "protection.allow_format_columns",
                "protection.allow_format_rows",
                "protection.allow_insert_columns",
                "protection.allow_insert_rows",
                "protection.allow_insert_hyperlinks",
                "protection.allow_delete_columns",
                "protection.allow_delete_rows",
                "protection.allow_sort",
                "protection.allow_auto_filter",
                "protection.allow_pivot_tables",
                "protection.allow_objects",
                "protection.allow_scenarios",
            ];
            let mut options = div()
                .id("sheet-protection-permissions")
                .mt_2()
                .max_h(px(240.))
                .overflow_y_scroll();
            for (i, allowed) in d.permissions().into_iter().enumerate() {
                options = options.child(
                    div()
                        .id(("sheet-permission", i))
                        .py_1()
                        .cursor_pointer()
                        .child(format!(
                            "{} {}",
                            if allowed { "☑" } else { "☐" },
                            i18n::text(labels[i])
                        ))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.workbook_protection_action(ProtectionAction::Toggle(i), cx);
                                cx.stop_propagation();
                            }),
                        ),
                );
            }
            panel = panel
                .child(
                    div()
                        .mt_2()
                        .child(i18n::text("protection.sheet_permissions")),
                )
                .child(options);
        }
        if let Some(problem) = d.problem {
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
                .child(self.protection_button(
                    "protection-cancel",
                    i18n::text(
                        if d.range_editor.is_some() && !d.is_authentication_prompt() {
                            "protection.range_close"
                        } else {
                            "validation.cancel"
                        },
                    ),
                    ProtectionAction::Cancel,
                    cx,
                ))
                .child(self.protection_button(
                    "protection-ok",
                    i18n::text(if d.range_editor.is_some() {
                        if d.unprotect {
                            "protection.range_unlock"
                        } else {
                            "protection.range_apply"
                        }
                    } else {
                        "validation.ok"
                    }),
                    ProtectionAction::Take,
                    cx,
                )),
        );
        Some(super::conditional::centred(
            div().child(
                div()
                    .id("protection-dialog-scroll")
                    .max_h(px(440.))
                    .overflow_y_scroll()
                    .child(panel),
            ),
        ))
    }
    fn protection_button(
        &self,
        id: &'static str,
        label: &'static str,
        action: ProtectionAction,
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
            .child(label)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.workbook_protection_action(action, cx);
                    cx.stop_propagation();
                }),
            )
    }
}

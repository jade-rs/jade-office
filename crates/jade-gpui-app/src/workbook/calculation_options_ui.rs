// Shared native/browser Calculation Options dialog.
#[derive(Clone, Copy)]
pub(super) enum CalculationAction {
    Mode(CalculationMode),
    RecalculateBeforeSave,
    Iteration,
    Focus(CalculationField),
    Step(i32),
    PrecisionAsDisplayed,
    Take,
    Cancel,
}
impl View {
    pub(super) fn calculation_options_panel(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let dialog = self.calculation_options_editor()?;
        let options = &dialog.options;
        let chip = |id: &'static str, label: String, on: bool, enabled: bool, action| {
            div()
                .id(id)
                .px_2()
                .py_1()
                .rounded_sm()
                .border_1()
                .border_color(if on { theme::accent() } else { theme::border() })
                .bg(if on { theme::selected() } else { theme::window() })
                .text_color(if enabled { theme::text() } else { theme::text_dim() })
                .cursor_pointer()
                .child(gpui::SharedString::from(label))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.calculation_options_action(action, cx);
                        cx.stop_propagation();
                    }),
                )
        };
        let check = |on: bool| if on { "☑ " } else { "☐ " };
        let section = |key: &'static str| {
            div()
                .mt_3()
                .mb_1()
                .text_xs()
                .text_color(theme::text_dim())
                .child(i18n::text(key))
        };
        let iterating = options.iteration.enabled;
        let field = |id: &'static str, text: &str, which: CalculationField| {
            let focused = dialog.field == Some(which);
            let caret = if focused { "|" } else { "" };
            chip(
                id,
                format!("{text}{caret}"),
                focused,
                iterating,
                CalculationAction::Focus(which),
            )
            .min_w(px(72.))
        };
        let modes = [
            ("calc-auto", "calculation.automatic", CalculationMode::Auto),
            (
                "calc-auto-no-table",
                "calculation.automatic_except_tables",
                CalculationMode::AutoNoTable,
            ),
            ("calc-manual", "calculation.manual", CalculationMode::Manual),
        ];
        let mut mode_row = div().flex().flex_col().gap_1();
        for (id, key, mode) in modes {
            mode_row = mode_row.child(chip(
                id,
                format!(
                    "{}{}",
                    if options.mode == mode { "◉ " } else { "○ " },
                    i18n::text(key)
                ),
                options.mode == mode,
                true,
                CalculationAction::Mode(mode),
            ));
        }
        let manual = options.mode == CalculationMode::Manual;
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
                    .child(i18n::text("calculation.dialog")),
            )
            .child(section("calculation.workbook"))
            .child(mode_row)
            .child(div().mt_1().child(chip(
                "calc-before-save",
                format!(
                    "{}{}",
                    check(options.recalculate_before_save),
                    i18n::text("calculation.recalculate_before_save")
                ),
                false,
                manual,
                CalculationAction::RecalculateBeforeSave,
            )))
            .child(section("calculation.iteration_section"))
            .child(chip(
                "calc-iterate",
                format!(
                    "{}{}",
                    check(iterating),
                    i18n::text("calculation.enable_iteration")
                ),
                false,
                true,
                CalculationAction::Iteration,
            ))
            .child(
                div()
                    .mt_1()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(div().w(px(150.)).child(i18n::text("calculation.maximum_iterations")))
                    .child(chip("calc-count-less", "−".into(), false, iterating, CalculationAction::Step(-1)))
                    .child(field(
                        "calc-count",
                        &dialog.count_text,
                        CalculationField::MaximumIterations,
                    ))
                    .child(chip("calc-count-more", "+".into(), false, iterating, CalculationAction::Step(1))),
            )
            .child(
                div()
                    .mt_1()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(div().w(px(150.)).child(i18n::text("calculation.maximum_change")))
                    .child(field(
                        "calc-delta",
                        &dialog.delta_text,
                        CalculationField::MaximumChange,
                    )),
            )
            .child(section("calculation.precision_section"))
            .child(chip(
                "calc-precision",
                format!(
                    "{}{}",
                    check(options.precision_as_displayed),
                    i18n::text("calculation.precision_as_displayed")
                ),
                false,
                true,
                CalculationAction::PrecisionAsDisplayed,
            ))
            .when(options.precision_as_displayed, |panel| {
                panel.child(
                    div()
                        .mt_1()
                        .text_xs()
                        .text_color(theme::text_dim())
                        .child(i18n::text("calculation.precision_warning")),
                )
            })
            .when(!dialog.valid(), |panel| {
                panel.child(
                    div()
                        .mt_2()
                        .text_xs()
                        .text_color(theme::text_dim())
                        .child(i18n::text("calculation.invalid_numbers")),
                )
            })
            .child(
                div()
                    .mt_4()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(chip(
                        "calc-cancel",
                        i18n::text("validation.cancel").into(),
                        false,
                        true,
                        CalculationAction::Cancel,
                    ))
                    .child(chip(
                        "calc-ok",
                        i18n::text("validation.ok").into(),
                        false,
                        dialog.valid(),
                        CalculationAction::Take,
                    )),
            );
        Some(super::conditional::centred(panel))
    }
}

/// What a press in the dialog does to it; OK and Cancel are the view's.
fn change_calculation_options(dialog: &mut CalculationOptionsDialog, action: CalculationAction) {
    match action {
        CalculationAction::Mode(mode) => dialog.set_mode(mode),
        CalculationAction::RecalculateBeforeSave => dialog.toggle_recalculate_before_save(),
        CalculationAction::Iteration => dialog.toggle_iteration(),
        CalculationAction::Focus(field) => dialog.focus(field),
        CalculationAction::Step(by) => {
            if dialog.options.iteration.enabled {
                dialog.step_count(by);
            }
        }
        CalculationAction::PrecisionAsDisplayed => dialog.toggle_precision_as_displayed(),
        CalculationAction::Take | CalculationAction::Cancel => {}
    }
}

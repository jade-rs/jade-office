// Shared native/browser Page Setup dialog.
#[derive(Clone, Copy)]
pub(super) enum PageSetupAction {
    Landscape(bool),
    Paper(u32),
    /// The scale up or down by points.
    Scale(i32),
    /// One of `FITS`, by its place.
    Fit(usize),
    Margins(MarginPreset),
    /// A margin stepped down (-1) or up (+1).
    Margin(MarginSide, i32),
    Print(PrintOption),
    Take,
    Cancel,
}
impl View {
    pub(super) fn page_setup_panel(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let dialog = self.page_setup_editor()?;
        let setup = &dialog.setup;
        let section = |key: &'static str| {
            div()
                .mt_3()
                .mb_1()
                .text_xs()
                .text_color(theme::text_dim())
                .child(i18n::text(key))
        };
        let row = || div().flex().flex_wrap().gap_1();
        let orientation = row()
            .child(self.page_setup_chip(
                "page-portrait",
                i18n::text("page_setup.portrait").into(),
                !dialog.landscape(),
                PageSetupAction::Landscape(false),
                cx,
            ))
            .child(self.page_setup_chip(
                "page-landscape",
                i18n::text("page_setup.landscape").into(),
                dialog.landscape(),
                PageSetupAction::Landscape(true),
                cx,
            ));
        let papers = row().children(PAPERS.iter().map(|(code, key)| {
            self.page_setup_chip(
                ("page-paper", *code as usize),
                i18n::text(key).into(),
                setup.paper_size == Some(*code),
                PageSetupAction::Paper(*code),
                cx,
            )
        }));
        let fitting = setup.fit_to();
        let scaling = row()
            .child(self.page_setup_chip(
                "page-scale-down",
                "−".into(),
                false,
                PageSetupAction::Scale(-10),
                cx,
            ))
            .child(self.page_setup_chip(
                "page-scale",
                format!("{}%", dialog.scale()),
                fitting.is_none(),
                PageSetupAction::Scale(0),
                cx,
            ))
            .child(self.page_setup_chip(
                "page-scale-up",
                "+".into(),
                false,
                PageSetupAction::Scale(10),
                cx,
            ))
            .children(FITS.iter().enumerate().map(|(index, (fit, key))| {
                self.page_setup_chip(
                    ("page-fit", index),
                    i18n::text(key).into(),
                    fitting == Some(*fit),
                    PageSetupAction::Fit(index),
                    cx,
                )
            }));
        let preset = dialog.margin_preset();
        let margins = row().children(MarginPreset::ALL.into_iter().map(|margins| {
            self.page_setup_chip(
                margins.label(),
                i18n::text(margins.label()).into(),
                preset == Some(margins),
                PageSetupAction::Margins(margins),
                cx,
            )
        }));
        // Custom margins: each side stepped on its own, in inches.
        let custom = div()
            .mt_2()
            .flex()
            .flex_wrap()
            .gap_1()
            .children(MarginSide::ALL.into_iter().enumerate().map(|(index, side)| {
                div()
                    .w(px(200.))
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .w(px(64.))
                            .text_xs()
                            .text_color(theme::text_dim())
                            .child(i18n::text(side.label())),
                    )
                    .child(self.page_setup_chip(
                        ("page-margin-down", index),
                        "−".into(),
                        false,
                        PageSetupAction::Margin(side, -1),
                        cx,
                    ))
                    .child(
                        div()
                            .w(px(44.))
                            .text_center()
                            .child(format!("{:.2}\"", dialog.margin(side))),
                    )
                    .child(self.page_setup_chip(
                        ("page-margin-up", index),
                        "+".into(),
                        false,
                        PageSetupAction::Margin(side, 1),
                        cx,
                    ))
            }));
        let options = row().children(PrintOption::ALL.into_iter().map(|option| {
            self.page_setup_chip(
                option.label(),
                i18n::text(option.label()).into(),
                dialog.print_option(option),
                PageSetupAction::Print(option),
                cx,
            )
        }));
        let panel = div()
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
                    .child(i18n::text("page_setup.dialog")),
            )
            .child(section("page_setup.orientation"))
            .child(orientation)
            .child(section("page_setup.paper"))
            .child(papers)
            .child(section("page_setup.scaling"))
            .child(scaling)
            .child(section("page_setup.margins"))
            .child(margins)
            .child(custom)
            .child(section("page_setup.print"))
            .child(options)
            .child(
                div()
                    .mt_4()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(self.page_setup_chip(
                        "page-cancel",
                        i18n::text("validation.cancel").into(),
                        false,
                        PageSetupAction::Cancel,
                        cx,
                    ))
                    .child(self.page_setup_chip(
                        "page-ok",
                        i18n::text("validation.ok").into(),
                        false,
                        PageSetupAction::Take,
                        cx,
                    )),
            );
        Some(super::conditional::centred(panel))
    }

    fn page_setup_chip(
        &self,
        id: impl Into<gpui::ElementId>,
        label: String,
        on: bool,
        action: PageSetupAction,
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
                    this.page_setup_action(action, cx);
                    cx.stop_propagation();
                }),
            )
    }
}

/// What a press in the dialog does to it; OK and Cancel are the view's.
fn change_page_setup_dialog(dialog: &mut PageSetupDialog, action: PageSetupAction) {
    match action {
        PageSetupAction::Landscape(landscape) => dialog.set_landscape(landscape),
        PageSetupAction::Paper(code) => dialog.set_paper(code),
        PageSetupAction::Scale(by) => dialog.step_scale(by),
        PageSetupAction::Fit(index) => {
            if let Some((fit, _)) = FITS.get(index) {
                dialog.set_fit(*fit);
            }
        }
        PageSetupAction::Margins(preset) => dialog.set_margins(preset),
        PageSetupAction::Margin(side, steps) => dialog.step_margin(side, steps),
        PageSetupAction::Print(option) => dialog.toggle_print_option(option),
        PageSetupAction::Take | PageSetupAction::Cancel => {}
    }
}

// Shared native/browser Data ▸ Remove Duplicates dialog.
#[derive(Clone, Copy)]
pub(super) enum DuplicatesAction {
    Column(usize),
    SelectAll(bool),
    Header,
    Take,
    Cancel,
}
impl View {
    pub(super) fn duplicates_panel(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let dialog = self.duplicates_editor()?;
        let mut columns = div()
            .id("duplicates-columns")
            .max_h(px(220.))
            .overflow_y_scroll()
            .flex()
            .flex_wrap()
            .gap_1();
        for (index, name) in dialog
            .column_names(i18n::text("duplicates.column"))
            .into_iter()
            .enumerate()
        {
            columns = columns.child(self.duplicates_chip(
                ("duplicates-column", index),
                name,
                dialog.compared[index],
                DuplicatesAction::Column(index),
                cx,
            ));
        }
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
                    .mb_2()
                    .child(i18n::text("duplicates.dialog")),
            )
            .child(
                div()
                    .mb_2()
                    .text_xs()
                    .text_color(theme::text_dim())
                    .child(i18n::text("duplicates.columns")),
            )
            .child(
                div()
                    .mb_2()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .child(self.duplicates_chip(
                        "duplicates-select-all",
                        i18n::text("duplicates.select_all").into(),
                        false,
                        DuplicatesAction::SelectAll(true),
                        cx,
                    ))
                    .child(self.duplicates_chip(
                        "duplicates-unselect-all",
                        i18n::text("duplicates.unselect_all").into(),
                        false,
                        DuplicatesAction::SelectAll(false),
                        cx,
                    ))
                    .child(self.duplicates_chip(
                        "duplicates-header",
                        i18n::text("sort.header").into(),
                        dialog.header,
                        DuplicatesAction::Header,
                        cx,
                    )),
            )
            .child(columns)
            .child(
                div()
                    .mt_4()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(self.duplicates_chip(
                        "duplicates-cancel",
                        i18n::text("validation.cancel").into(),
                        false,
                        DuplicatesAction::Cancel,
                        cx,
                    ))
                    .child(self.duplicates_chip(
                        "duplicates-ok",
                        i18n::text("validation.ok").into(),
                        false,
                        DuplicatesAction::Take,
                        cx,
                    )),
            );
        Some(super::conditional::centred(panel))
    }

    fn duplicates_chip(
        &self,
        id: impl Into<gpui::ElementId>,
        label: String,
        on: bool,
        action: DuplicatesAction,
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
                    this.duplicates_action(action, cx);
                    cx.stop_propagation();
                }),
            )
    }
}

/// What a press in the dialog does to it; OK and Cancel are the view's.
fn change_duplicates_dialog(dialog: &mut DuplicatesDialog, action: DuplicatesAction) {
    match action {
        DuplicatesAction::Column(index) => {
            if let Some(compared) = dialog.compared.get_mut(index) {
                *compared = !*compared;
            }
        }
        DuplicatesAction::SelectAll(on) => dialog.select_all(on),
        DuplicatesAction::Header => dialog.header = !dialog.header,
        DuplicatesAction::Take | DuplicatesAction::Cancel => {}
    }
}

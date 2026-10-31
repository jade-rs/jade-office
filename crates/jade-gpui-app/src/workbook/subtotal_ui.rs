// Shared native/browser Data ▸ Subtotal dialog.
#[derive(Clone, Copy)]
pub(super) enum SubtotalAction {
    By(usize),
    Function(usize),
    Column(usize),
    Replace,
    Take,
    Cancel,
}
impl View {
    pub(super) fn subtotal_panel(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let dialog = self.subtotal_editor()?;
        let section = |key: &'static str| {
            div()
                .mt_3()
                .mb_1()
                .text_xs()
                .text_color(theme::text_dim())
                .child(i18n::text(key))
        };
        let row = |id: &'static str| {
            div()
                .id(id)
                .max_h(px(96.))
                .overflow_y_scroll()
                .flex()
                .flex_wrap()
                .gap_1()
        };
        let mut by = row("subtotal-by");
        let mut columns = row("subtotal-columns");
        for (index, name) in dialog.headers.iter().enumerate() {
            by = by.child(self.subtotal_chip(
                ("subtotal-by", index),
                name.clone(),
                dialog.by == index,
                SubtotalAction::By(index),
                cx,
            ));
            columns = columns.child(self.subtotal_chip(
                ("subtotal-column", index),
                name.clone(),
                dialog.columns[index],
                SubtotalAction::Column(index),
                cx,
            ));
        }
        let mut functions = row("subtotal-functions");
        for (index, function) in SubtotalFunction::ALL.into_iter().enumerate() {
            functions = functions.child(self.subtotal_chip(
                ("subtotal-function", index),
                i18n::text(subtotal_keys(function).0).to_string(),
                dialog.function == function,
                SubtotalAction::Function(index),
                cx,
            ));
        }
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
                    .child(i18n::text("subtotal.dialog")),
            )
            .child(section("subtotal.by"))
            .child(by)
            .child(section("subtotal.function"))
            .child(functions)
            .child(section("subtotal.columns"))
            .child(columns)
            .child(div().mt_3().flex().child(self.subtotal_chip(
                "subtotal-replace",
                i18n::text("subtotal.replace").into(),
                dialog.replace,
                SubtotalAction::Replace,
                cx,
            )))
            .child(
                div()
                    .mt_4()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(self.subtotal_chip(
                        "subtotal-cancel",
                        i18n::text("validation.cancel").into(),
                        false,
                        SubtotalAction::Cancel,
                        cx,
                    ))
                    .child(self.subtotal_chip(
                        "subtotal-ok",
                        i18n::text("validation.ok").into(),
                        false,
                        SubtotalAction::Take,
                        cx,
                    )),
            );
        Some(super::conditional::centred(panel))
    }

    fn subtotal_chip(
        &self,
        id: impl Into<gpui::ElementId>,
        label: String,
        on: bool,
        action: SubtotalAction,
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
                    this.subtotal_action(action, cx);
                    cx.stop_propagation();
                }),
            )
    }
}

/// What a press in the dialog does to it; OK and Cancel are the view's.
fn change_subtotal_dialog(dialog: &mut SubtotalDialog, action: SubtotalAction) {
    match action {
        SubtotalAction::By(index) => dialog.by = index,
        SubtotalAction::Function(index) => dialog.function = SubtotalFunction::ALL[index],
        SubtotalAction::Column(index) => {
            if let Some(ticked) = dialog.columns.get_mut(index) {
                *ticked = !*ticked;
            }
        }
        SubtotalAction::Replace => dialog.replace = !dialog.replace,
        SubtotalAction::Take | SubtotalAction::Cancel => {}
    }
}

/// The row labels for the function picked, in the user's language.
fn subtotal_labels(dialog: &SubtotalDialog) -> (String, String) {
    let (_, row, grand) = subtotal_keys(dialog.function);
    (i18n::text(row).to_string(), i18n::text(grand).to_string())
}

/// A function's i18n keys: its name in the dialog, a group's row label and
/// the grand total's.
fn subtotal_keys(function: SubtotalFunction) -> (&'static str, &'static str, &'static str) {
    match function {
        SubtotalFunction::Sum => (
            "subtotal.fn.sum",
            "subtotal.row.sum",
            "subtotal.grand.sum",
        ),
        SubtotalFunction::Count => (
            "subtotal.fn.count",
            "subtotal.row.count",
            "subtotal.grand.count",
        ),
        SubtotalFunction::Average => (
            "subtotal.fn.average",
            "subtotal.row.average",
            "subtotal.grand.average",
        ),
        SubtotalFunction::Max => (
            "subtotal.fn.max",
            "subtotal.row.max",
            "subtotal.grand.max",
        ),
        SubtotalFunction::Min => (
            "subtotal.fn.min",
            "subtotal.row.min",
            "subtotal.grand.min",
        ),
        SubtotalFunction::Product => (
            "subtotal.fn.product",
            "subtotal.row.product",
            "subtotal.grand.product",
        ),
        SubtotalFunction::CountNumbers => (
            "subtotal.fn.count_numbers",
            "subtotal.row.count_numbers",
            "subtotal.grand.count_numbers",
        ),
        SubtotalFunction::StdDev => (
            "subtotal.fn.stdev",
            "subtotal.row.stdev",
            "subtotal.grand.stdev",
        ),
        SubtotalFunction::StdDevP => (
            "subtotal.fn.stdevp",
            "subtotal.row.stdevp",
            "subtotal.grand.stdevp",
        ),
        SubtotalFunction::Var => (
            "subtotal.fn.var",
            "subtotal.row.var",
            "subtotal.grand.var",
        ),
        SubtotalFunction::VarP => (
            "subtotal.fn.varp",
            "subtotal.row.varp",
            "subtotal.grand.varp",
        ),
    }
}

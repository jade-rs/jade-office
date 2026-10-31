// Shared native/browser Insert Function dialog.
#[derive(Clone, Copy)]
pub(super) enum InsertFunctionAction {
    Pick(usize),
    Take,
    Cancel,
}
impl View {
    pub(super) fn insert_function_panel(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let dialog = self.insert_function_editor()?;
        let matches = dialog.matches();
        let chosen = dialog.chosen();
        let mut list = div()
            .id("insert-function-list")
            .mt_2()
            .max_h(px(240.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .rounded_sm()
            .border_1()
            .border_color(theme::border())
            .bg(theme::window());
        for (index, name) in matches.iter().enumerate() {
            list = list.child(
                div()
                    .id(("insert-function-name", index))
                    .px_2()
                    .py_0p5()
                    .cursor_pointer()
                    .when(Some(*name) == chosen, |row| row.bg(theme::selected()))
                    .hover(|row| row.bg(theme::hovered()))
                    .child(*name)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                            this.insert_function_action(InsertFunctionAction::Pick(index), cx);
                            if event.click_count >= 2 {
                                this.insert_function_action(InsertFunctionAction::Take, cx);
                            }
                            cx.stop_propagation();
                        }),
                    ),
            );
        }
        let tip = chosen.and_then(|name| {
            let text = format!("={name}(");
            jade_formula_tips::argument_tip(&text, text.len())
        });
        let panel = div()
            .w(px(380.))
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
                    .child(i18n::text("insert_function.dialog")),
            )
            .child(
                div()
                    .mb_1()
                    .text_xs()
                    .text_color(theme::text_dim())
                    .child(i18n::text("insert_function.search")),
            )
            .child(
                div()
                    .h(px(24.))
                    .px_1()
                    .flex()
                    .items_center()
                    .rounded_sm()
                    .border_1()
                    .border_color(theme::accent())
                    .bg(theme::window())
                    .overflow_hidden()
                    .child(super::caret_line(&dialog.search, theme::accent().into(), false)),
            )
            .child(list)
            .child(
                div()
                    .mt_2()
                    .min_h(px(18.))
                    .text_xs()
                    .flex()
                    .flex_wrap()
                    .children(tip.into_iter().flat_map(|tip| tip.pieces()).map(
                        |(piece, _)| div().child(piece),
                    )),
            )
            .child(
                div()
                    .mt_4()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(self.insert_function_button(
                        "insert-function-cancel",
                        "validation.cancel",
                        InsertFunctionAction::Cancel,
                        cx,
                    ))
                    .child(self.insert_function_button(
                        "insert-function-ok",
                        "validation.ok",
                        InsertFunctionAction::Take,
                        cx,
                    )),
            );
        Some(super::conditional::centred(panel))
    }

    fn insert_function_button(
        &self,
        id: &'static str,
        key: &'static str,
        action: InsertFunctionAction,
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
            .child(i18n::text(key))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.insert_function_action(action, cx);
                    cx.stop_propagation();
                }),
            )
    }
}

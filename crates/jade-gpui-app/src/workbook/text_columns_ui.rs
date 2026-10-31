// Shared native/browser Data ▸ Text to Columns dialog.
#[derive(Clone, Copy)]
pub(super) enum TextColumnsAction {
    Tab,
    Semicolon,
    Comma,
    Space,
    Consecutive,
    Take,
    Cancel,
}
impl View {
    pub(super) fn text_columns_panel(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let dialog = self.text_columns_editor()?;
        let split = &dialog.split;
        let section = |key: &'static str| {
            div()
                .mt_3()
                .mb_1()
                .text_xs()
                .text_color(theme::text_dim())
                .child(i18n::text(key))
        };
        let delimiters = div()
            .flex()
            .flex_wrap()
            .gap_1()
            .child(self.text_columns_chip(
                "text-columns-tab",
                "text_columns.tab",
                split.tab,
                TextColumnsAction::Tab,
                cx,
            ))
            .child(self.text_columns_chip(
                "text-columns-semicolon",
                "text_columns.semicolon",
                split.semicolon,
                TextColumnsAction::Semicolon,
                cx,
            ))
            .child(self.text_columns_chip(
                "text-columns-comma",
                "text_columns.comma",
                split.comma,
                TextColumnsAction::Comma,
                cx,
            ))
            .child(self.text_columns_chip(
                "text-columns-space",
                "text_columns.space",
                split.space,
                TextColumnsAction::Space,
                cx,
            ));
        let mut preview = div()
            .id("text-columns-preview")
            .max_h(px(120.))
            .overflow_y_scroll()
            .p_1()
            .rounded_sm()
            .border_1()
            .border_color(theme::border())
            .bg(theme::window())
            .text_xs()
            .font_family("Menlo");
        for pieces in dialog.preview() {
            preview = preview.child(gpui::SharedString::from(pieces.join(" │ ")));
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
                    .child(i18n::text("text_columns.dialog")),
            )
            .child(section("text_columns.delimiters"))
            .child(delimiters)
            .child(div().mt_2().flex().child(self.text_columns_chip(
                "text-columns-consecutive",
                "text_columns.consecutive",
                split.consecutive,
                TextColumnsAction::Consecutive,
                cx,
            )))
            .child(section("text_columns.preview"))
            .child(preview)
            .child(
                div()
                    .mt_4()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(self.text_columns_chip(
                        "text-columns-cancel",
                        "validation.cancel",
                        false,
                        TextColumnsAction::Cancel,
                        cx,
                    ))
                    .child(self.text_columns_chip(
                        "text-columns-ok",
                        "validation.ok",
                        false,
                        TextColumnsAction::Take,
                        cx,
                    )),
            );
        Some(super::conditional::centred(panel))
    }

    fn text_columns_chip(
        &self,
        id: &'static str,
        key: &'static str,
        on: bool,
        action: TextColumnsAction,
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
            .child(i18n::text(key))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.text_columns_action(action, cx);
                    cx.stop_propagation();
                }),
            )
    }
}

/// What a press in the dialog does to it; OK and Cancel are the view's.
fn change_text_columns_dialog(dialog: &mut TextColumnsDialog, action: TextColumnsAction) {
    let split = &mut dialog.split;
    match action {
        TextColumnsAction::Tab => split.tab = !split.tab,
        TextColumnsAction::Semicolon => split.semicolon = !split.semicolon,
        TextColumnsAction::Comma => split.comma = !split.comma,
        TextColumnsAction::Space => split.space = !split.space,
        TextColumnsAction::Consecutive => split.consecutive = !split.consecutive,
        TextColumnsAction::Take | TextColumnsAction::Cancel => {}
    }
}

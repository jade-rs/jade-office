// Shared native/browser Data ▸ Text to Columns dialog.
#[derive(Clone, Copy)]
pub(super) enum TextColumnsAction {
    Tab,
    Semicolon,
    Comma,
    Space,
    Consecutive,
    /// Fixed width (true) or delimited.
    Fixed(bool),
    /// A click on the fixed-width ruler: a break before that character.
    Break(usize),
    /// From Text's "Start import at row" moved up or down.
    Start(isize),
    /// A column of the preview chosen, for its format.
    Column(usize),
    /// The chosen column's format.
    Format(ColumnFormat),
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
        let fixed = split.fixed_width.as_deref();
        let mode = div()
            .mt_2()
            .flex()
            .flex_wrap()
            .gap_1()
            .child(self.text_columns_chip(
                "text-columns-delimited",
                "text_columns.delimited",
                fixed.is_none(),
                TextColumnsAction::Fixed(false),
                cx,
            ))
            .child(self.text_columns_chip(
                "text-columns-fixed",
                "text_columns.fixed_width",
                fixed.is_some(),
                TextColumnsAction::Fixed(true),
                cx,
            ));
        let widest = dialog
            .sample
            .iter()
            .max_by_key(|line| line.chars().count())
            .cloned()
            .unwrap_or_default();
        let mut ruler = div()
            .id("text-columns-ruler")
            .overflow_x_scroll()
            .flex()
            .p_1()
            .rounded_sm()
            .border_1()
            .border_color(theme::border())
            .bg(theme::window())
            .text_xs()
            .font_family("Menlo");
        for (at, character) in widest.chars().enumerate() {
            let broken = fixed.is_some_and(|breaks| breaks.contains(&at));
            ruler = ruler.child(
                div()
                    .id(("text-columns-break", at))
                    .w(px(8.))
                    .flex_none()
                    .border_l_2()
                    .border_color(if broken {
                        gpui::Hsla::from(theme::accent())
                    } else {
                        gpui::transparent_black()
                    })
                    .cursor_pointer()
                    .hover(|cell| cell.bg(theme::hovered()))
                    .child(gpui::SharedString::from(if character == ' ' {
                        "·".to_string()
                    } else {
                        character.to_string()
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.text_columns_action(TextColumnsAction::Break(at), cx);
                            cx.stop_propagation();
                        }),
                    ),
            );
        }
        // The preview as columns; a column's heading picks it for the
        // format choices below, a skipped one is greyed.
        let rows = dialog.preview();
        let columns = rows.iter().map(Vec::len).max().unwrap_or(0).min(30);
        let format_of = |column: usize| dialog.formats.get(column).copied().unwrap_or_default();
        let mut preview = div()
            .id("text-columns-preview")
            .max_h(px(150.))
            .overflow_scroll()
            .p_1()
            .rounded_sm()
            .border_1()
            .border_color(theme::border())
            .bg(theme::window())
            .text_xs()
            .child(div().flex().children((0..columns).map(|column| {
                let chosen = dialog.column == column;
                div()
                    .id(("text-columns-heading", column))
                    .w(px(80.))
                    .flex_none()
                    .px_1()
                    .truncate()
                    .cursor_pointer()
                    .text_color(theme::text_dim())
                    .when(chosen, |heading| heading.bg(theme::selected()))
                    .child(format!(
                        "{} · {}",
                        column + 1,
                        i18n::text(format_of(column).label())
                    ))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.text_columns_action(TextColumnsAction::Column(column), cx);
                            cx.stop_propagation();
                        }),
                    )
            })));
        for pieces in rows {
            preview = preview.child(div().flex().children((0..columns).map(|column| {
                let skipped = format_of(column) == ColumnFormat::Skip;
                div()
                    .w(px(80.))
                    .flex_none()
                    .px_1()
                    .truncate()
                    .when(skipped, |cell| cell.text_color(theme::text_dim()))
                    .when(dialog.column == column, |cell| cell.bg(theme::selected()))
                    .child(gpui::SharedString::from(
                        pieces.get(column).cloned().unwrap_or_default().replace('\n', " "),
                    ))
            })));
        }
        let formats = div().flex().flex_wrap().gap_1().children(
            ColumnFormat::ALL.into_iter().enumerate().map(|(index, format)| {
                self.text_columns_chip(
                    ("text-columns-format", index),
                    format.label(),
                    dialog.format() == format,
                    TextColumnsAction::Format(format),
                    cx,
                )
            }),
        );
        let start = dialog.import.as_ref().map(|import| {
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(self.text_columns_chip(
                    "text-columns-start-up",
                    "text_columns.minus",
                    false,
                    TextColumnsAction::Start(-1),
                    cx,
                ))
                .child(div().w(px(32.)).text_center().child(format!("{}", import.start + 1)))
                .child(self.text_columns_chip(
                    "text-columns-start-down",
                    "text_columns.plus",
                    false,
                    TextColumnsAction::Start(1),
                    cx,
                ))
        });
        let panel = div()
            .w(px(520.))
            .p_4()
            .rounded_md()
            .bg(theme::panel())
            .border_1()
            .border_color(theme::border())
            .shadow_lg()
            .text_sm()
            .text_color(theme::text())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(div().font_weight(gpui::FontWeight::BOLD).child(i18n::text(
                if dialog.import.is_some() {
                    "text_columns.import_dialog"
                } else {
                    "text_columns.dialog"
                },
            )))
            .child(mode)
            .when_some(start, |panel, start| {
                panel.child(section("text_columns.start_row")).child(start)
            })
            .map(|panel| {
                if fixed.is_some() {
                    panel.child(section("text_columns.breaks")).child(ruler)
                } else {
                    panel
                        .child(section("text_columns.delimiters"))
                        .child(delimiters)
                        .child(div().mt_2().flex().child(self.text_columns_chip(
                            "text-columns-consecutive",
                            "text_columns.consecutive",
                            split.consecutive,
                            TextColumnsAction::Consecutive,
                            cx,
                        )))
                }
            })
            .child(section("text_columns.preview"))
            .child(preview)
            .child(section("text_columns.column_format"))
            .child(formats)
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
        id: impl Into<gpui::ElementId>,
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
    match action {
        TextColumnsAction::Fixed(on) => {
            if on != dialog.split.fixed_width.is_some() {
                dialog.set_fixed_width(on);
            }
            return;
        }
        TextColumnsAction::Break(at) => return dialog.toggle_break(at),
        TextColumnsAction::Start(step) => return dialog.step_start(step),
        TextColumnsAction::Column(column) => return dialog.select_column(column),
        TextColumnsAction::Format(format) => return dialog.set_format(format),
        _ => {}
    }
    let split = &mut dialog.split;
    match action {
        TextColumnsAction::Tab => split.tab = !split.tab,
        TextColumnsAction::Semicolon => split.semicolon = !split.semicolon,
        TextColumnsAction::Comma => split.comma = !split.comma,
        TextColumnsAction::Space => split.space = !split.space,
        TextColumnsAction::Consecutive => split.consecutive = !split.consecutive,
        TextColumnsAction::Fixed(_)
        | TextColumnsAction::Break(_)
        | TextColumnsAction::Start(_)
        | TextColumnsAction::Column(_)
        | TextColumnsAction::Format(_)
        | TextColumnsAction::Take
        | TextColumnsAction::Cancel => {}
    }
}

// Shared native/browser: what ⌘+ (or ⌃-) asks when cells, not whole rows
// or columns, are selected, as Excel's Insert and Delete dialogs do.

/// A choice of the dialog.
#[derive(Clone, Copy)]
pub(super) enum ShiftChoice {
    /// Shift cells down (or up) on rows, right (or left) on columns.
    Cells(Axis),
    EntireRows,
    EntireColumns,
    Cancel,
}

impl ShiftChoice {
    /// The choices in Excel's order, with the i18n keys of their names, for
    /// inserting (`true`) or deleting.
    fn all(inserted: bool) -> [(ShiftChoice, &'static str); 4] {
        if inserted {
            [
                (ShiftChoice::Cells(Axis::Columns), "shift.cells_right"),
                (ShiftChoice::Cells(Axis::Rows), "shift.cells_down"),
                (ShiftChoice::EntireRows, "shift.entire_row"),
                (ShiftChoice::EntireColumns, "shift.entire_column"),
            ]
        } else {
            [
                (ShiftChoice::Cells(Axis::Columns), "shift.cells_left"),
                (ShiftChoice::Cells(Axis::Rows), "shift.cells_up"),
                (ShiftChoice::EntireRows, "shift.entire_row"),
                (ShiftChoice::EntireColumns, "shift.entire_column"),
            ]
        }
    }

    /// The command a choice runs.
    fn command(self, inserted: bool) -> Option<MenuCommand> {
        Some(match (self, inserted) {
            (ShiftChoice::Cells(axis), _) => MenuCommand::ShiftCells(axis, inserted),
            (ShiftChoice::EntireRows, true) => MenuCommand::InsertRows,
            (ShiftChoice::EntireRows, false) => MenuCommand::DeleteRows,
            (ShiftChoice::EntireColumns, true) => MenuCommand::InsertColumns,
            (ShiftChoice::EntireColumns, false) => MenuCommand::DeleteColumns,
            (ShiftChoice::Cancel, _) => return None,
        })
    }
}

impl View {
    pub(super) fn shift_choice_panel(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let inserted = self.shift_choice_open()?;
        let chip = |id: (&'static str, usize), key: &'static str, choice: ShiftChoice| {
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
                        this.shift_choice_action(choice, cx);
                        cx.stop_propagation();
                    }),
                )
        };
        let panel = div()
            .w(px(300.))
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
                    .mb_3()
                    .child(i18n::text(if inserted { "shift.insert" } else { "shift.delete" })),
            )
            .child(
                div().flex().flex_col().gap_1().children(
                    ShiftChoice::all(inserted)
                        .into_iter()
                        .enumerate()
                        .map(|(index, (choice, key))| chip(("shift-choice", index), key, choice)),
                ),
            )
            .child(div().mt_3().flex().justify_end().child(chip(
                ("shift-cancel", 0),
                "validation.cancel",
                ShiftChoice::Cancel,
            )));
        Some(super::conditional::centred(panel))
    }

    /// Keys while the dialog is open: Enter takes Excel's default (shift
    /// cells down, or up), Escape closes.
    pub(super) fn shift_choice_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        match key {
            "enter" => self.shift_choice_action(ShiftChoice::Cells(Axis::Rows), cx),
            "escape" => self.shift_choice_action(ShiftChoice::Cancel, cx),
            _ => {}
        }
        true
    }
}

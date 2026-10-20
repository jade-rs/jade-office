//! Find and Replace (⌘F, and Ctrl+H with the Replace field): a bar over
//! the sheet's top right with what to find and what to replace it with,
//! Find Next and Previous, Replace and Replace All, and Excel's options —
//! Match case, Match entire cell contents, Within: Workbook. The same bar
//! as the native app's, over `jade-office-model`'s `find`.

use jade_web::find::FindBar;

use super::*;

/// What the bar's buttons and keys do.
#[derive(Clone, Copy)]
pub(super) enum FindCommand {
    /// Find Next, or Previous (true).
    Next(bool),
    Replace,
    ReplaceAll,
    MatchCase,
    EntireCell,
    Workbook,
    /// The Replace field shown or hidden.
    ToggleReplace,
    /// Typing into the Replace field (true) or Find.
    Field(bool),
    Close,
}

impl DocView {
    /// ⌘F opens the bar; Ctrl+H opens it with the Replace field, or shows
    /// that field in a bar already open.
    pub(super) fn open_sheet_find(&mut self, replacing: bool, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        match sheet.finding.as_mut() {
            Some(bar) if replacing && !bar.replacing => bar.toggle_replacing(),
            Some(_) => {}
            None => sheet.finding = Some(FindBar::new(replacing)),
        }
        cx.notify();
    }

    pub(super) fn sheet_find_command(&mut self, command: FindCommand, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let Some(bar) = sheet.finding.as_mut() else {
            return;
        };
        match command {
            FindCommand::Next(back) => return self.find_next(back, cx),
            FindCommand::Replace => return self.replace_one(cx),
            FindCommand::ReplaceAll => return self.replace_all(cx),
            FindCommand::MatchCase => bar.options.match_case = !bar.options.match_case,
            FindCommand::EntireCell => bar.options.entire_cell = !bar.options.entire_cell,
            FindCommand::Workbook => bar.options.workbook = !bar.options.workbook,
            FindCommand::ToggleReplace => bar.toggle_replacing(),
            FindCommand::Field(replace) => bar.in_replace = replace && bar.replacing,
            // Where the search got to stays selected.
            FindCommand::Close => sheet.finding = None,
        }
        cx.notify();
    }

    /// Find Next (Enter in the bar; Shift+Enter goes back): the next cell
    /// with what is typed, on this sheet or, Within: Workbook, the next.
    pub(super) fn find_next(&mut self, back: bool, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let Some(bar) = sheet.finding.as_ref() else {
            return;
        };
        let (needle, options) = (bar.find.text.clone(), bar.options());
        let from = (sheet.book.sheet_index(), sheet.selection.active);
        match sheet.book.find_next(&needle, options, from, back) {
            Some((index, cell)) => {
                if index != sheet.book.sheet_index() {
                    // Another sheet's tab keeps the bar open.
                    let finding = sheet.finding.take();
                    self.select_sheet(index, cx);
                    if let Some(sheet) = self.sheet_mut() {
                        sheet.finding = finding;
                    }
                }
                self.select_cell(cell, false, cx);
                self.status = SharedString::default();
            }
            None => self.status = i18n::text("sheet.not_found").into(),
        }
        cx.notify();
    }

    /// Replace: the active cell, if it has what is typed, then on to the
    /// next, as Excel's button does.
    fn replace_one(&mut self, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let Some(bar) = sheet.finding.as_ref() else {
            return;
        };
        let (needle, replacement, options) = (
            bar.find.text.clone(),
            bar.replace.text.clone(),
            bar.options(),
        );
        let active = sheet.selection.active;
        let result = sheet
            .book
            .replace_at(active, &needle, &replacement, options);
        let failed = result.is_err();
        self.edited(result, cx);
        if !failed {
            self.find_next(false, cx);
        }
    }

    /// Replace All, saying how many cells it changed.
    fn replace_all(&mut self, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let Some(bar) = sheet.finding.as_ref() else {
            return;
        };
        let (needle, replacement, options) = (
            bar.find.text.clone(),
            bar.replace.text.clone(),
            bar.options(),
        );
        match sheet.book.replace_all(&needle, &replacement, options) {
            Ok(0) => {
                self.status = i18n::text("sheet.not_found").into();
                cx.notify();
            }
            Ok(count) => {
                self.edited(Ok(true), cx);
                self.status =
                    i18n::format("sheet.replaced", &[("n", &count as &dyn std::fmt::Display)])
                        .into();
            }
            Err(error) => self.edited(Err(error), cx),
        }
    }

    /// Keys while the bar is open: Enter finds the next (Shift, the one
    /// before), Tab goes between the fields, Ctrl+H shows Replace, Escape
    /// closes it; the rest edit the field.
    pub(super) fn sheet_find_key(
        &mut self,
        key: &str,
        modifiers: gpui::Modifiers,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(bar) = self.sheet_ref().and_then(|sheet| sheet.finding.as_ref()) else {
            return false;
        };
        if bar.field().marked.is_some() {
            return false;
        }
        let replacing = bar.replacing;
        match key {
            "enter" => self.sheet_find_command(FindCommand::Next(modifiers.shift), cx),
            "tab" if replacing => {
                if let Some(bar) = self.sheet_mut().and_then(|sheet| sheet.finding.as_mut()) {
                    bar.next_field();
                }
                cx.notify();
            }
            "tab" | "escape" => self.sheet_find_command(FindCommand::Close, cx),
            "h" if modifiers.control && !modifiers.platform => self.open_sheet_find(true, cx),
            // ⌘F again leaves the bar as it is.
            "f" if modifiers.platform || modifiers.control => {}
            _ => return self.name_key(key, modifiers, cx),
        }
        true
    }

    /// The bar, while it is open.
    pub(super) fn sheet_find_bar(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let bar = self.sheet_ref()?.finding.as_ref()?;
        let button = |id: &'static str, on: bool, command: FindCommand| {
            div()
                .id(id)
                .h(px(20.0))
                .px_1p5()
                .flex()
                .items_center()
                .rounded_sm()
                .cursor_pointer()
                .when(on, |button| button.bg(palette::selected()))
                .hover(|button| button.bg(palette::hovered()))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |view, _event, window, cx| {
                    window.focus(&view.focus, cx);
                    view.sheet_find_command(command, cx);
                }))
        };
        let field = |id: &'static str, label: &'static str, replace: bool| {
            let typing = bar.in_replace == replace;
            let text = if replace { &bar.replace } else { &bar.find };
            div()
                .id(id)
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .w(px(64.0))
                        .text_color(palette::dim())
                        .child(i18n::text(label)),
                )
                .child(
                    div()
                        .flex_1()
                        .h(px(20.0))
                        .px_1()
                        .flex()
                        .items_center()
                        .rounded_sm()
                        .border_1()
                        .border_color(if typing {
                            palette::accent()
                        } else {
                            palette::border()
                        })
                        .overflow_hidden()
                        .map(|input| {
                            if typing {
                                input.child(caret_line(text, palette::accent().into(), false))
                            } else {
                                input.child(text.text.clone())
                            }
                        }),
                )
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |view, _event, window, cx| {
                    window.focus(&view.focus, cx);
                    view.sheet_find_command(FindCommand::Field(replace), cx);
                }))
        };
        let options = bar.options;
        Some(
            div()
                .absolute()
                .top_2()
                .right_4()
                .w(px(360.0))
                .flex()
                .flex_col()
                .gap_1()
                .px_2()
                .py_1p5()
                .rounded_md()
                .border_1()
                .border_color(palette::accent())
                .bg(palette::panel())
                .shadow_lg()
                .text_xs()
                .text_color(palette::text())
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(field("find-what", "sheet.find", false))
                .when(bar.replacing, |panel| {
                    panel.child(field("replace-with", "sheet.replace_with", true))
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            button(
                                "find-match-case",
                                options.match_case,
                                FindCommand::MatchCase,
                            )
                            .child(i18n::text("sheet.match_case")),
                        )
                        .child(
                            button(
                                "find-entire-cell",
                                options.entire_cell,
                                FindCommand::EntireCell,
                            )
                            .child(i18n::text("sheet.entire_cell")),
                        )
                        .child(
                            button("find-workbook", options.workbook, FindCommand::Workbook)
                                .child(i18n::text("sheet.within_workbook")),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            button(
                                "find-replace-toggle",
                                bar.replacing,
                                FindCommand::ToggleReplace,
                            )
                            .child(i18n::text("sheet.replace_toggle")),
                        )
                        .child(div().flex_1())
                        .when(bar.replacing, |row| {
                            row.child(
                                button("replace-all", false, FindCommand::ReplaceAll)
                                    .child(i18n::text("sheet.replace_all")),
                            )
                            .child(
                                button("replace-one", false, FindCommand::Replace)
                                    .child(i18n::text("sheet.replace")),
                            )
                        })
                        .child(
                            button("find-previous", false, FindCommand::Next(true))
                                .child(i18n::text("sheet.find_previous")),
                        )
                        .child(
                            button("find-next", false, FindCommand::Next(false))
                                .child(i18n::text("sheet.find_next")),
                        )
                        .child(button("find-close", false, FindCommand::Close).child("✕")),
                ),
        )
    }
}

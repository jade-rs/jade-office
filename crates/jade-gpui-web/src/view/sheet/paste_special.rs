//! Paste Special (⌃⌘V or Ctrl+Alt+V, or the right-click menu): a dialog
//! choosing All, Formulas, Values or Formats, an Operation, and Skip
//! blanks and Transpose, over `jade-office-model`'s `paste_special` — the
//! native app's dialog. The browser hands over clipboard text only in a
//! paste event, so what pastes here is this workbook's own copy.

use jade_web::paste_special::{PasteOperation, PasteSpecial, PasteWhat};

use super::conditional::centred;
use super::*;

impl DocView {
    /// Opens the dialog, when there is something it could paste.
    pub(super) fn open_paste_special(&mut self, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        if !sheet.book.can_paste_special(None) {
            return;
        }
        sheet.pasting = Some(PasteSpecial::default());
        cx.notify();
    }

    /// The dialog's choice pasted on the selection's top-left, the pasted
    /// range selected.
    fn take_paste_special(&mut self, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let Some(special) = sheet.pasting.take() else {
            return;
        };
        let at = sheet.selection.range().start;
        let result = sheet.book.paste_special(at, None, special);
        if let Ok(Some(landed)) = &result {
            sheet.selection = Selection {
                active: landed.start,
                anchor: landed.start,
                focus: landed.end,
            };
        }
        self.edited(result.map(|landed| landed.is_some()), cx);
    }

    /// Keys while the dialog is open: arrows choose, Return pastes, Escape
    /// leaves.
    pub(super) fn paste_special_key(
        &mut self,
        key: &str,
        _modifiers: gpui::Modifiers,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(sheet) = self.sheet_mut() else {
            return false;
        };
        if sheet.pasting.is_none() {
            return false;
        }
        match key {
            "enter" => self.take_paste_special(cx),
            "escape" => {
                sheet.pasting = None;
                cx.notify();
            }
            key => {
                if let Some(special) = sheet.pasting.as_mut() {
                    if special.key(key) {
                        cx.notify();
                    }
                }
            }
        }
        // Nothing else reaches the sheet while the dialog is open.
        true
    }

    /// The dialog, while it is open.
    pub(super) fn paste_special_dialog(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let special = self.sheet_ref()?.pasting?;
        let choice = |what: PasteWhat| {
            let chosen = special.what == what;
            div()
                .id(what.label())
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .py_0p5()
                .rounded_sm()
                .cursor_pointer()
                .hover(|row| row.bg(palette::hovered()))
                .child(
                    div()
                        .size(px(10.0))
                        .rounded_full()
                        .border_1()
                        .border_color(palette::dim())
                        .when(chosen, |dot| dot.bg(palette::accent())),
                )
                .child(i18n::text(what.label()))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |view, _event, _window, cx| {
                    if let Some(special) = view.sheet_mut().and_then(|sheet| sheet.pasting.as_mut())
                    {
                        special.what = what;
                    }
                    cx.notify();
                }))
        };
        let operation = |operation: PasteOperation| {
            let chosen = special.operation == operation;
            div()
                .id(operation.label())
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .py_0p5()
                .rounded_sm()
                .cursor_pointer()
                .hover(|row| row.bg(palette::hovered()))
                .child(
                    div()
                        .size(px(10.0))
                        .rounded_full()
                        .border_1()
                        .border_color(palette::dim())
                        .when(chosen, |dot| dot.bg(palette::accent())),
                )
                .child(i18n::text(operation.label()))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |view, _event, _window, cx| {
                    if let Some(special) = view.sheet_mut().and_then(|sheet| sheet.pasting.as_mut())
                    {
                        special.operation = operation;
                    }
                    cx.notify();
                }))
        };
        let tick =
            |id: &'static str, label: &'static str, on: bool, flip: fn(&mut PasteSpecial)| {
                div()
                    .id(id)
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_0p5()
                    .rounded_sm()
                    .cursor_pointer()
                    .hover(|row| row.bg(palette::hovered()))
                    .child(
                        div()
                            .size(px(10.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .border_1()
                            .border_color(palette::dim())
                            .child(if on { "✓" } else { "" }),
                    )
                    .child(i18n::text(label))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |view, _event, _window, cx| {
                        if let Some(special) =
                            view.sheet_mut().and_then(|sheet| sheet.pasting.as_mut())
                        {
                            flip(special);
                        }
                        cx.notify();
                    }))
            };
        let panel = self.dialog_panel(i18n::text("sheet.paste_special.title").to_string(), 260.0);
        Some(centred(
            panel
                .child(
                    div()
                        .text_color(palette::dim())
                        .child(i18n::text("sheet.paste_special.paste")),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .children(PasteWhat::ALL.into_iter().map(choice)),
                )
                .child(
                    div()
                        .text_color(palette::dim())
                        .child(i18n::text("sheet.paste_special.operation")),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .children(PasteOperation::ALL.into_iter().map(operation)),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .border_t_1()
                        .border_color(palette::border())
                        .pt_1()
                        .child(tick(
                            "paste-skip-blanks",
                            "sheet.paste_special.skip_blanks",
                            special.skip_blanks,
                            |special| special.skip_blanks = !special.skip_blanks,
                        ))
                        .child(tick(
                            "paste-transpose",
                            "sheet.paste_special.transpose",
                            special.transpose,
                            |special| special.transpose = !special.transpose,
                        )),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child(self.dialog_button(
                            "paste-cancel",
                            "sheet.paste_special.cancel",
                            false,
                            cx,
                            |view, cx| {
                                if let Some(sheet) = view.sheet_mut() {
                                    sheet.pasting = None;
                                }
                                cx.notify();
                            },
                        ))
                        .child(self.dialog_button(
                            "paste-ok",
                            "sheet.paste_special.ok",
                            true,
                            cx,
                            |view, cx| view.take_paste_special(cx),
                        )),
                ),
        ))
    }
}

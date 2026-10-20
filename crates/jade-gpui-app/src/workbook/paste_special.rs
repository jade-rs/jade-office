//! Paste Special (⌃⌘V, or the right-click menu): a dialog choosing All,
//! Formulas, Values or Formats, with Skip blanks and Transpose, over
//! `jade-office-model`'s `paste_special`.

use gpui::{div, prelude::*, px, Context, Modifiers, MouseButton};
use jade_office_model::grid::Selection;
use jade_office_model::paste_special::{PasteSpecial, PasteWhat};

use super::conditional::centred;
use super::WorkbookView;
use crate::{i18n, theme};

impl WorkbookView {
    /// Opens the dialog, when there is something it could paste.
    pub(super) fn open_paste_special(&mut self, cx: &mut Context<Self>) {
        let text = cx.read_from_clipboard().and_then(|item| item.text());
        if !self.book.can_paste_special(text.as_deref()) {
            return;
        }
        self.pasting = Some(PasteSpecial::default());
        cx.notify();
    }

    /// The dialog's choice pasted on the selection's top-left, the pasted
    /// range selected.
    fn take_paste_special(&mut self, cx: &mut Context<Self>) {
        let Some(special) = self.pasting.take() else {
            return;
        };
        let text = cx.read_from_clipboard().and_then(|item| item.text());
        let result = self
            .book
            .paste_special(self.range().start, text.as_deref(), special);
        if let Ok(Some(landed)) = &result {
            self.selection = Selection {
                active: landed.start,
                anchor: landed.start,
                focus: landed.end,
            };
        }
        self.after_write(result.map(|landed| landed.is_some()), cx);
    }

    /// Keys while the dialog is open: arrows choose, Return pastes, Escape
    /// leaves.
    pub(super) fn paste_special_key(
        &mut self,
        key: &str,
        _modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.pasting.is_none() {
            return false;
        }
        match key {
            "enter" => self.take_paste_special(cx),
            "escape" => {
                self.pasting = None;
                cx.notify();
            }
            key => {
                if let Some(special) = self.pasting.as_mut() {
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
        let special = self.pasting?;
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
                .hover(|row| row.bg(theme::hovered()))
                .child(
                    div()
                        .size(px(10.0))
                        .rounded_full()
                        .border_1()
                        .border_color(theme::text_dim())
                        .when(chosen, |dot| dot.bg(theme::accent())),
                )
                .child(i18n::text(what.label()))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |view, _event, _window, cx| {
                    if let Some(special) = view.pasting.as_mut() {
                        special.what = what;
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
                    .hover(|row| row.bg(theme::hovered()))
                    .child(
                        div()
                            .size(px(10.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .border_1()
                            .border_color(theme::text_dim())
                            .child(if on { "✓" } else { "" }),
                    )
                    .child(i18n::text(label))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |view, _event, _window, cx| {
                        if let Some(special) = view.pasting.as_mut() {
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
                        .text_color(theme::text_dim())
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
                        .flex()
                        .flex_col()
                        .border_t_1()
                        .border_color(theme::border())
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
                                view.pasting = None;
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

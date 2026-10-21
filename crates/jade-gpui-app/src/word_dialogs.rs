//! Word's small dialogs, as bars over the pages that take the keyboard while
//! they are open, as the header and comment bars do: ハイパーリンクの挿入
//! (⌘K). What each does is `jade-office-model`'s; the browser shell has the
//! same (`jade-gpui-web/src/view/word_dialogs.rs`).

use gpui::{div, prelude::*, px, AnyElement, Context, KeyDownEvent, SharedString};
use jade_office_model::text::TextEdit;
use jade_office_model::word::{LinkForm, LinkTarget, WordLinks};

use crate::{t, theme, DocumentView};

/// The dialog open over the pages.
pub enum Dialog {
    /// ハイパーリンクの挿入 / ハイパーリンクの編集.
    Link(LinkForm),
}

impl Dialog {
    /// The field typing goes into.
    pub fn field(&self) -> Option<&TextEdit> {
        match self {
            Dialog::Link(form) => Some(form.field()),
        }
    }

    pub fn field_mut(&mut self) -> Option<&mut TextEdit> {
        match self {
            Dialog::Link(form) => Some(form.field_mut()),
        }
    }
}

/// A labelled one-line field, its caret drawn when it has the keyboard.
pub fn field_box(label: &str, field: &TextEdit, focused: bool, width: f32) -> AnyElement {
    let (before, after) = field.split();
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .text_xs()
                .text_color(theme::text_dim())
                .child(label.to_string()),
        )
        .child(
            div()
                .flex()
                .items_center()
                .w(px(width))
                .h(px(24.0))
                .px_2()
                .rounded_md()
                .bg(theme::canvas())
                .border_1()
                .border_color(if focused {
                    theme::accent()
                } else {
                    theme::border()
                })
                .text_sm()
                .text_color(theme::text())
                .overflow_hidden()
                .whitespace_nowrap()
                .child(before.to_string())
                .when(focused, |row| {
                    row.child(div().w(px(1.0)).h(px(16.0)).bg(theme::accent()))
                })
                .child(after.to_string()),
        )
        .into_any_element()
}

/// The bar a dialog sits in, over the pages.
pub fn bar(title: &str, keys: &str, body: Vec<AnyElement>) -> AnyElement {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap_3()
        .flex_none()
        .px_4()
        .py_2()
        .bg(theme::panel())
        .border_b_1()
        .border_color(theme::border())
        .child(
            div()
                .text_xs()
                .text_color(theme::text_faint())
                .child(title.to_string()),
        )
        .children(body)
        .child(
            div()
                .text_xs()
                .text_color(theme::text_faint())
                .child(keys.to_string()),
        )
        .into_any_element()
}

impl DocumentView {
    fn open_dialog(&mut self, dialog: Dialog, cx: &mut Context<Self>) {
        self.find_bar = None;
        self.story_bar = None;
        self.comment_bar = None;
        self.ribbon.popup = None;
        self.dialog = Some(dialog);
        cx.notify();
    }

    /// The selection the shell holds, handed to the document so a dialog
    /// reads and writes the text the user sees selected.
    fn sync_selection(&mut self) {
        if self.document.selection().is_none() {
            self.document
                .place_caret(self.insert_at.0, self.insert_at.1);
        }
    }

    /// ⌘K: ハイパーリンクの挿入 on the selection, or the link the caret is
    /// in, to edit.
    pub(crate) fn open_link_dialog(&mut self, cx: &mut Context<Self>) {
        self.sync_selection();
        let draft = self.document.link_draft();
        self.open_dialog(Dialog::Link(LinkForm::new(draft)), cx);
    }

    /// ⌘-click: a link's address to the system, a bookmark's place to the
    /// caret. Returns whether there was a link under the point.
    pub(crate) fn follow_link_at(
        &mut self,
        x: f64,
        document_y: f64,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(link) = self.document.hyperlink_at_point(x, document_y) else {
            return false;
        };
        match self.document.link_target(&link) {
            Some(LinkTarget::Address(address)) => cx.open_url(&address),
            Some(LinkTarget::Position(position)) => {
                self.selection_anchor = None;
                self.drag_anchor = None;
                self.jump_to(position, cx);
                if let Some(caret) = self.caret {
                    let (page, _) = self.document.page_and_offset(caret.document_y);
                    self.scroll.scroll_to_item(page);
                    self.current_page = page;
                }
            }
            None => {
                self.table_status = Some(SharedString::from(t("link.missing_bookmark")));
                cx.notify();
            }
        }
        true
    }

    /// Keys an open dialog takes: Enter applies it, Escape closes it, Tab
    /// moves between its fields, and the field's own editing keys.
    pub(crate) fn handle_dialog_key(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(dialog) = self.dialog.as_mut() else {
            return false;
        };
        if dialog.field().is_some_and(|field| field.marked.is_some()) {
            return false;
        }
        let key = event.keystroke.key.as_str();
        match key {
            "escape" => self.dialog = None,
            "enter" | "return" => return self.submit_dialog(cx),
            "tab" => match dialog {
                Dialog::Link(form) => form.switch_field(),
            },
            _ => {
                let edited = dialog.field_mut().is_some_and(|field| field.edit_key(key));
                if !edited {
                    return false;
                }
            }
        }
        cx.notify();
        true
    }

    /// OK: what the dialog says, done to the document as one undo step.
    pub(crate) fn submit_dialog(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(dialog) = self.dialog.take() else {
            return false;
        };
        let result = match &dialog {
            Dialog::Link(form) => form.submit(&mut self.document),
        };
        match result {
            Ok(caret) => {
                self.dirty = true;
                self.table_status = None;
                self.selection_anchor = None;
                self.insert_at = caret;
                self.caret = self.document.place_caret(caret.0, caret.1);
                self.rebuild_all_images();
            }
            Err(error) => {
                self.table_status = Some(SharedString::from(error));
                // Left open, so what was typed can be put right.
                self.dialog = Some(dialog);
            }
        }
        cx.notify();
        true
    }

    /// The open dialog, over the pages.
    pub(crate) fn dialog_element(&self) -> Option<AnyElement> {
        let dialog = self.dialog.as_ref()?;
        Some(match dialog {
            Dialog::Link(form) => bar(
                t(if form.draft.editing {
                    "link.edit"
                } else {
                    "link.insert"
                }),
                t("link.keys"),
                vec![
                    field_box(t("link.display"), &form.display, !form.on_address, 200.0),
                    field_box(t("link.address"), &form.address, form.on_address, 280.0),
                ],
            ),
        })
    }
}

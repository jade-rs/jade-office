//! Word's small dialogs over a document's pages, as bars that take the
//! keyboard while they are open, as the header and comment bars do:
//! ハイパーリンクの挿入 (⌘K). What each does is `jade-office-model`'s; the
//! native app has the same (`jade-gpui-app/src/word_dialogs.rs`).

use jade_web::word::{LinkForm, LinkTarget, WordLinks};

use super::sheet::caret_line;
use super::*;

/// The dialog open over the pages.
pub(super) enum Dialog {
    /// ハイパーリンクの挿入 / ハイパーリンクの編集.
    Link(LinkForm),
}

impl Dialog {
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
fn field_box(label: &'static str, field: &TextEdit, focused: bool, width: f32) -> gpui::AnyElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(div().text_color(palette::dim()).child(label))
        .child(
            div()
                .w(px(width))
                .h(px(24.0))
                .px_2()
                .flex()
                .items_center()
                .overflow_hidden()
                .rounded_md()
                .border_1()
                .border_color(if focused {
                    palette::accent()
                } else {
                    palette::border()
                })
                .bg(palette::canvas())
                .child(if focused {
                    caret_line(field, palette::accent().into(), false)
                } else {
                    div().whitespace_nowrap().child(field.text.clone())
                }),
        )
        .into_any_element()
}

/// The bar a dialog sits in, over the pages.
fn bar(title: &'static str, keys: &'static str, body: Vec<gpui::AnyElement>) -> gpui::AnyElement {
    div()
        .flex()
        .flex_none()
        .flex_wrap()
        .items_center()
        .gap_3()
        .px_4()
        .py_1p5()
        .bg(palette::panel())
        .border_b_1()
        .border_color(palette::border())
        .text_xs()
        .text_color(palette::text())
        .child(div().text_color(palette::faint()).child(title))
        .children(body)
        .child(div().text_color(palette::dim()).child(keys))
        .into_any_element()
}

impl DocView {
    pub(super) fn dialog_open(&self) -> bool {
        matches!(
            &self.document,
            Document::Pages {
                dialog: Some(_),
                ..
            }
        )
    }

    fn open_dialog(&mut self, opened: Dialog, cx: &mut Context<Self>) {
        let Document::Pages {
            dialog,
            find,
            story,
            comment,
            ribbon,
            ..
        } = &mut self.document
        else {
            return;
        };
        *find = None;
        *story = None;
        *comment = None;
        ribbon.popup = None;
        *dialog = Some(Box::new(opened));
        cx.notify();
    }

    /// ⌘K: ハイパーリンクの挿入 on the selection, or the link the caret is
    /// in, to edit.
    pub(super) fn open_link_dialog(&mut self, cx: &mut Context<Self>) {
        let Document::Pages { document, .. } = &self.document else {
            return;
        };
        let form = LinkForm::new(document.link_draft());
        self.open_dialog(Dialog::Link(form), cx);
    }

    /// ⌘-click: a link's address opened in a new tab, a bookmark's place
    /// given the caret. Returns whether there was a link under the point.
    pub(super) fn follow_link_at(&mut self, x: f64, y: f64, cx: &mut Context<Self>) -> bool {
        let Document::Pages { document, .. } = &mut self.document else {
            return false;
        };
        let Some(link) = document.hyperlink_at_point(x, y) else {
            return false;
        };
        match document.link_target(&link) {
            Some(LinkTarget::Address(address)) => cx.open_url(&address),
            Some(LinkTarget::Position((paragraph, offset))) => {
                document.place_caret(paragraph, offset);
                self.reveal_caret();
                cx.notify();
            }
            None => {
                self.status = i18n::text("link.missing_bookmark").into();
                cx.notify();
            }
        }
        true
    }

    /// Keys an open dialog takes: Enter applies it, Escape closes it, Tab
    /// moves between its fields, and the field's own editing keys.
    pub(super) fn dialog_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let key = event.keystroke.key.as_str();
        let Document::Pages { dialog, .. } = &mut self.document else {
            return false;
        };
        let Some(open) = dialog.as_deref_mut() else {
            return false;
        };
        if open.field().is_some_and(|field| field.marked.is_some()) {
            return false;
        }
        match key {
            "escape" => *dialog = None,
            "enter" => return self.submit_dialog(cx),
            "tab" => match open {
                Dialog::Link(form) => form.switch_field(),
            },
            _ => {
                if !open.field_mut().is_some_and(|field| field.edit_key(key)) {
                    return false;
                }
            }
        }
        cx.notify();
        true
    }

    /// OK: what the dialog says, done to the document as one undo step.
    fn submit_dialog(&mut self, cx: &mut Context<Self>) -> bool {
        let Document::Pages {
            document, dialog, ..
        } = &mut self.document
        else {
            return false;
        };
        let Some(open) = dialog.take() else {
            return false;
        };
        let result = match open.as_ref() {
            Dialog::Link(form) => form.submit(document.as_mut()),
        };
        match result {
            Ok((paragraph, offset)) => {
                document.place_caret(paragraph, offset);
                self.refresh_repainted();
                self.reveal_caret();
                self.word_result(Ok(()), cx);
            }
            Err(error) => {
                // Left open, so what was typed can be put right.
                *dialog = Some(open);
                self.word_result(Err(error), cx);
            }
        }
        true
    }

    pub(super) fn dialog_element(&self) -> Option<gpui::AnyElement> {
        let Document::Pages {
            dialog: Some(dialog),
            ..
        } = &self.document
        else {
            return None;
        };
        Some(match dialog.as_ref() {
            Dialog::Link(form) => bar(
                i18n::text(if form.draft.editing {
                    "link.edit"
                } else {
                    "link.insert"
                }),
                i18n::text("link.keys"),
                vec![
                    field_box(
                        i18n::text("link.display"),
                        &form.display,
                        !form.on_address,
                        200.0,
                    ),
                    field_box(
                        i18n::text("link.address"),
                        &form.address,
                        form.on_address,
                        280.0,
                    ),
                ],
            ),
        })
    }
}

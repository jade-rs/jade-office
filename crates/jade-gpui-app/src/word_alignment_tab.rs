//! ヘッダーとフッター › 位置合わせタブの挿入: the dialog's 配置, 基準 and
//! リーダー in a bar over the pages, OK putting an absolute position tab at
//! the caret (`jade-office-model`'s `alignment_tab`; the browser shell has
//! the same).

use gpui::{div, prelude::*, AnyElement, Context};
use jade_office_model::word::alignment_tab::{AlignmentTabForm, TabAlign, LEADERS};

use crate::word_dialog::{button, chip, on};
use crate::word_dialogs::{bar as frame, Dialog};
use crate::{i18n, DocumentView};

impl DocumentView {
    /// Opens the dialog on Word's defaults.
    pub(crate) fn open_alignment_tab_dialog(&mut self, cx: &mut Context<Self>) {
        self.sync_selection();
        self.open_dialog(Dialog::AlignmentTab(AlignmentTabForm::default()), cx);
    }

    fn edit_alignment_tab(&mut self, cx: &mut Context<Self>, edit: impl FnOnce(&mut AlignmentTabForm)) {
        if let Some(Dialog::AlignmentTab(form)) = self.dialog.as_mut() {
            edit(form);
            cx.notify();
        }
    }
}

/// The bar: 配置, 基準, リーダー, OK and キャンセル.
pub fn bar_body(form: &AlignmentTabForm, cx: &mut Context<DocumentView>) -> Vec<AnyElement> {
    let t = i18n::text;
    let caption = |key: &'static str| {
        div()
            .text_xs()
            .text_color(crate::theme::text_dim())
            .child(t(key))
            .into_any_element()
    };
    let mut body = vec![caption("word.ptab.alignment")];
    for (index, align) in TabAlign::ALL.into_iter().enumerate() {
        body.push(
            chip(("ptab-align", index), t(align.label()), form.alignment == align)
                .on_click(on(cx, move |view, cx| {
                    view.edit_alignment_tab(cx, |form| form.alignment = align)
                }))
                .into_any_element(),
        );
    }
    body.push(caption("word.ptab.relative_to"));
    for (index, (indent, key)) in [(false, "word.ptab.margin"), (true, "word.ptab.indent")]
        .into_iter()
        .enumerate()
    {
        body.push(
            chip(("ptab-base", index), t(key), form.relative_to_indent == indent)
                .on_click(on(cx, move |view, cx| {
                    view.edit_alignment_tab(cx, |form| form.relative_to_indent = indent)
                }))
                .into_any_element(),
        );
    }
    body.push(caption("word.ptab.leader"));
    for (index, (leader, label)) in LEADERS.into_iter().enumerate() {
        let text = if leader.is_none() { t(label) } else { label };
        body.push(
            chip(("ptab-leader", index), text, form.leader == leader)
                .on_click(on(cx, move |view, cx| {
                    view.edit_alignment_tab(cx, |form| form.leader = leader)
                }))
                .into_any_element(),
        );
    }
    body.push(
        button("ptab-ok", t("format.ok"), true)
            .on_click(on(cx, |view, cx| {
                view.submit_dialog(cx);
            }))
            .into_any_element(),
    );
    body.push(
        button("ptab-cancel", t("format.cancel"), false)
            .on_click(on(cx, |view, cx| {
                view.dialog = None;
                cx.notify();
            }))
            .into_any_element(),
    );
    body
}

/// The dialog's bar over the pages.
pub fn bar(form: &AlignmentTabForm, cx: &mut Context<DocumentView>) -> AnyElement {
    let body = bar_body(form, cx);
    frame(
        i18n::text("word.ptab.title"),
        i18n::text("word.ptab.keys"),
        body,
    )
}

//! The browser's 位置合わせタブの挿入, as the native app's
//! `word_alignment_tab`: 配置, 基準 and リーダー in a bar over the pages, OK
//! putting an absolute position tab at the caret.

use gpui::{div, prelude::*, AnyElement, Context};
use jade_web::word::alignment_tab::{AlignmentTabForm, TabAlign, LEADERS};

use super::word_dialogs::Dialog;
use super::word_kit::{button, chip, on};
use super::{DocView, Document};
use crate::i18n;
use crate::palette;

impl DocView {
    /// Opens the dialog on Word's defaults.
    pub(super) fn open_alignment_tab_dialog(&mut self, cx: &mut Context<Self>) {
        self.open_dialog(Dialog::AlignmentTab(AlignmentTabForm::default()), cx);
    }

    fn edit_alignment_tab(&mut self, cx: &mut Context<Self>, edit: impl FnOnce(&mut AlignmentTabForm)) {
        if let Document::Pages {
            dialog: Some(dialog),
            ..
        } = &mut self.document
        {
            if let Dialog::AlignmentTab(form) = dialog.as_mut() {
                edit(form);
                cx.notify();
            }
        }
    }

    /// The bar's contents: 配置, 基準, リーダー, OK and キャンセル.
    pub(super) fn alignment_tab_body(
        &self,
        form: &AlignmentTabForm,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let t = i18n::text;
        let caption = |key: &'static str| {
            div()
                .text_color(palette::dim())
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
                    if let Document::Pages { dialog, .. } = &mut view.document {
                        *dialog = None;
                    }
                    cx.notify();
                }))
                .into_any_element(),
        );
        body
    }
}

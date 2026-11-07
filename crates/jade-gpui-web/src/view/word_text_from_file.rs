//! The browser's 挿入 › テキスト › ファイルからテキスト…, as the native app's
//! `word_text_from_file`: the page's picker chooses a `.docx` or `.txt`,
//! whose text goes in at the caret.

use gpui::{div, prelude::*, px, AnyElement, Context};
use jade_web::word::text_from_file::insert_text_from_file;

use super::word_tools_ui::Pick;
use super::DocView;
use crate::{i18n, palette};

impl DocView {
    /// Opens the page's picker for the file.
    pub(crate) fn pick_text_from_file(&mut self, cx: &mut Context<Self>) {
        self.tools.picking = Some(Pick::TextFromFile);
        self.host.pick();
        cx.notify();
    }

    /// The file the picker brought, put in at the caret.
    pub(super) fn insert_text_from_bytes(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        let bytes = bytes.to_vec();
        self.word_edit(cx, move |document| {
            insert_text_from_file(document, &bytes).map(|_| ())
        });
    }
}

/// The ribbon button.
pub fn button(cx: &mut Context<DocView>) -> AnyElement {
    div()
        .id("text-from-file")
        .flex()
        .flex_none()
        .items_center()
        .h(px(24.0))
        .px_2()
        .rounded_md()
        .text_xs()
        .text_color(palette::text())
        .whitespace_nowrap()
        .cursor_pointer()
        .hover(|button| button.bg(palette::hovered()))
        .on_click(cx.listener(|view, _, _, cx| view.pick_text_from_file(cx)))
        .child(i18n::text("word.text_from_file"))
        .into_any_element()
}

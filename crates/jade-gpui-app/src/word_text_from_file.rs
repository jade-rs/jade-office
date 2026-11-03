//! 挿入 › テキスト › ファイルからテキスト…: a `.docx` or `.txt` chosen and its
//! text put in at the caret (`jade_office_model::word::text_from_file`).

use gpui::{div, prelude::*, px, AnyElement, Context, SharedString};
use jade_office_model::word::text_from_file::insert_text_from_file;

use crate::tooltip::Tooltip;
use crate::{i18n, theme, DocumentView};

impl DocumentView {
    /// Asks for the file, then inserts it.
    pub(crate) fn choose_text_from_file(&mut self, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(SharedString::from(i18n::text("word.text_from_file.prompt"))),
        });
        cx.spawn(async move |view, cx| {
            let Ok(Ok(Some(paths))) = prompt.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            view.update(cx, |view, cx| view.insert_text_from_path(&path, cx))
                .ok();
        })
        .detach();
    }

    pub(crate) fn insert_text_from_path(&mut self, path: &std::path::Path, cx: &mut Context<Self>) {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) => return self.tool_done(Err(error.to_string()), cx),
        };
        self.sync_selection();
        self.tools_caret();
        match insert_text_from_file(&mut self.document, &bytes) {
            Ok(caret) => {
                self.selection_anchor = None;
                self.insert_at = caret;
                self.caret = self.document.place_caret(caret.0, caret.1);
                self.dirty = true;
                self.ribbon.read_styles(&self.document);
                self.rebuild_after_edit();
                cx.notify();
            }
            Err(error) => self.tool_done(Err(error), cx),
        }
    }
}

/// The ribbon button.
pub(crate) fn button(cx: &mut Context<DocumentView>) -> AnyElement {
    div()
        .id("text-from-file")
        .flex()
        .flex_none()
        .items_center()
        .h(px(24.0))
        .px_2()
        .rounded_md()
        .text_xs()
        .whitespace_nowrap()
        .text_color(theme::text())
        .cursor_pointer()
        .hover(|button| button.bg(theme::hovered()))
        .on_click(cx.listener(|view, _, _, cx| view.choose_text_from_file(cx)))
        .tooltip(Tooltip::text(i18n::text("word.text_from_file.title")))
        .child(i18n::text("word.text_from_file"))
        .into_any_element()
}

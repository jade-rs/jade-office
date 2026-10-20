//! ヘッダーの編集 / フッターの編集 over a Word document's pages: the story's
//! first line — Word's left, centre and right zones, tab-separated — typed
//! into a bar over the pages, put in with Enter, left as it was with
//! Escape. One undo step (`jade-office-model`'s `WordPage::set_story_text`).

use jade_web::word::page::{Story, WordPage};

use super::sheet::caret_line;
use super::*;

impl DocView {
    /// Whether a header or footer is being typed into its bar.
    pub(super) fn story_editing(&self) -> bool {
        matches!(&self.document, Document::Pages { story: Some(_), .. })
    }

    pub(super) fn open_story(&mut self, story: Story, cx: &mut Context<Self>) {
        let Document::Pages {
            document,
            story: bar,
            find,
            ..
        } = &mut self.document
        else {
            return;
        };
        *find = None;
        *bar = Some(Box::new(crate::word_page::StoryBar {
            story,
            text: TextEdit::new(document.story_text(story)),
        }));
        cx.notify();
    }

    /// Keys the bar takes: Enter, Escape, Tab between the zones, and the
    /// field's own editing keys.
    pub(super) fn story_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let key = event.keystroke.key.as_str();
        let Document::Pages {
            document, story, ..
        } = &mut self.document
        else {
            return false;
        };
        let Some(bar) = story.as_deref_mut() else {
            return false;
        };
        if bar.text.marked.is_some() {
            return false;
        }
        match key {
            "escape" => *story = None,
            "enter" => {
                let Some(bar) = story.take() else {
                    return false;
                };
                let result = document.set_story_text(bar.story, &bar.text.text);
                self.refresh_repainted();
                return {
                    self.word_result(result, cx);
                    true
                };
            }
            "tab" => bar.text.insert("\t"),
            "backspace" => bar.text.backspace(),
            "delete" => bar.text.delete(),
            "left" => bar.text.step(false),
            "right" => bar.text.step(true),
            "home" | "up" => bar.text.to_end(false),
            "end" | "down" => bar.text.to_end(true),
            _ => return false,
        }
        cx.notify();
        true
    }

    pub(super) fn story_bar_element(&self) -> Option<gpui::AnyElement> {
        let Document::Pages {
            story: Some(bar), ..
        } = &self.document
        else {
            return None;
        };
        // A tab shows as Word's zone mark, three bytes for its one.
        let caret = bar.text.caret();
        let tabs_before = bar.text.text[..caret].matches('\t').count();
        let shown = TextEdit {
            text: bar.text.text.replace('\t', "⇥"),
            marked: None,
            caret: caret + tabs_before * 2,
            whole: false,
        };
        Some(
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap_3()
                .px_4()
                .py_1p5()
                .bg(palette::panel())
                .border_b_1()
                .border_color(palette::border())
                .text_xs()
                .text_color(palette::text())
                .child(div().text_color(palette::faint()).child(bar.label()))
                .child(
                    div()
                        .min_w(px(320.0))
                        .h(px(24.0))
                        .px_2()
                        .flex()
                        .items_center()
                        .rounded_md()
                        .border_1()
                        .border_color(palette::accent())
                        .bg(palette::canvas())
                        .child(caret_line(&shown, palette::accent().into(), false)),
                )
                .child(
                    div()
                        .text_color(palette::dim())
                        .child(i18n::text("page.story_keys")),
                )
                .into_any_element(),
        )
    }
}

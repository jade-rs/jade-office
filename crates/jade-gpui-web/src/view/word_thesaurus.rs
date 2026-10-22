//! 類義語辞典 in the browser, as the native app's `word_thesaurus`: the
//! pane with the words that mean what the word at the caret means, a click
//! on one looking it up, 挿入 putting it in the word's place. The browser
//! has no system thesaurus, so the words are the model's built-in English
//! list.

use jade_web::word::thesaurus::{matching_case, BuiltinThesaurus, Sense, Thesaurus, WordThesaurus};

use super::*;

type Target = ((usize, usize), (usize, usize), String);

/// The pane, while it is open.
pub(crate) struct WebThesaurus {
    word: String,
    senses: Vec<Sense>,
    target: Option<Target>,
}

impl DocView {
    /// 類義語辞典: the pane for the word at the caret, or shut.
    pub(super) fn toggle_thesaurus(&mut self, cx: &mut Context<Self>) {
        let Document::Pages {
            document,
            thesaurus,
            ..
        } = &mut self.document
        else {
            return;
        };
        if thesaurus.take().is_none() {
            let target = document.word_for_thesaurus();
            let word = target
                .as_ref()
                .map(|(_, _, word)| word.clone())
                .unwrap_or_default();
            *thesaurus = Some(Box::new(WebThesaurus {
                senses: BuiltinThesaurus.lookup(&word),
                word,
                target,
            }));
        }
        cx.notify();
    }

    fn thesaurus_look_up(&mut self, word: &str, cx: &mut Context<Self>) {
        if let Document::Pages {
            thesaurus: Some(pane),
            ..
        } = &mut self.document
        {
            pane.word = word.to_string();
            pane.senses = BuiltinThesaurus.lookup(word);
        }
        cx.notify();
    }

    fn thesaurus_insert(&mut self, synonym: String, cx: &mut Context<Self>) {
        let Document::Pages {
            thesaurus: Some(pane),
            ..
        } = &mut self.document
        else {
            return;
        };
        let Some(target) = pane.target.clone() else {
            return;
        };
        let replaced = matching_case(&target.2, &synonym);
        pane.target = Some((
            target.0,
            (target.0 .0, target.0 .1 + replaced.len()),
            replaced,
        ));
        self.word_edit(cx, move |document| {
            document.replace_with_synonym(&target, &synonym)
        });
    }

    /// The pane beside the pages, while it is open.
    pub(super) fn thesaurus_pane(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let Document::Pages {
            thesaurus: Some(pane),
            ..
        } = &self.document
        else {
            return None;
        };
        let mut rows: Vec<gpui::AnyElement> = vec![div()
            .text_sm()
            .text_color(palette::text())
            .child(pane.word.clone())
            .into_any_element()];
        if pane.senses.is_empty() {
            rows.push(
                div()
                    .text_color(palette::dim())
                    .child(i18n::text(if pane.word.is_empty() {
                        "thesaurus.no_word"
                    } else {
                        "thesaurus.none"
                    }))
                    .into_any_element(),
            );
        }
        let mut index = 0usize;
        for sense in &pane.senses {
            rows.push(
                div()
                    .mt_1()
                    .text_color(palette::dim())
                    .child(sense.label.clone())
                    .into_any_element(),
            );
            for synonym in &sense.synonyms {
                let (looked, inserted) = (synonym.clone(), synonym.clone());
                rows.push(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .pl_2()
                        .child(
                            div()
                                .id(("thesaurus-word", index))
                                .px_1()
                                .rounded_md()
                                .cursor_pointer()
                                .hover(|word| word.bg(palette::hovered()))
                                .child(synonym.clone())
                                .on_click(cx.listener(move |view, _event, _window, cx| {
                                    view.thesaurus_look_up(&looked, cx)
                                })),
                        )
                        .when(pane.target.is_some(), |row| {
                            row.child(
                                div()
                                    .id(("thesaurus-insert", index))
                                    .px_1p5()
                                    .rounded_md()
                                    .text_color(palette::accent())
                                    .cursor_pointer()
                                    .hover(|button| button.bg(palette::hovered()))
                                    .child(i18n::text("thesaurus.insert"))
                                    .on_click(cx.listener(move |view, _event, _window, cx| {
                                        view.thesaurus_insert(inserted.clone(), cx)
                                    })),
                            )
                        })
                        .into_any_element(),
                );
                index += 1;
            }
        }
        Some(
            div()
                .id("thesaurus-pane")
                .flex()
                .flex_col()
                .flex_none()
                .w(px(240.0))
                .h_full()
                .gap_1()
                .px_3()
                .pt_3()
                .overflow_y_scroll()
                .bg(palette::panel())
                .border_l_1()
                .border_color(palette::border())
                .text_xs()
                .text_color(palette::text())
                .child(div().text_sm().child(i18n::text("thesaurus.title")))
                .children(rows)
                .into_any_element(),
        )
    }
}

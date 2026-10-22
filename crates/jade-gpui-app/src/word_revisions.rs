//! 校閲 › 変更履歴ウィンドウ: every tracked change beside the pages — what
//! it is (挿入, 削除, 移動, 書式変更 with what the formatting change changed,
//! as Word's balloons say it), who made it and when, and the text — each a
//! click away in the document, and accepted or rejected from the pane.

use gpui::{div, prelude::*, px, AnyElement, ClickEvent, Context};
use jade_office_model::word::markup::{describe_formatting, kind_key};
use jade_office_model::word::review::{Revision, WordReview};

use crate::{i18n, theme, DocumentView};

/// 変更履歴ウィンドウ: open or shut.
pub fn toggle(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    view.review.revisions_pane = !view.review.revisions_pane;
    cx.notify();
}

fn decide(
    view: &mut DocumentView,
    revision: &Revision,
    accept: bool,
    cx: &mut Context<DocumentView>,
) {
    let result = view
        .document
        .decide_revisions(std::slice::from_ref(revision), accept);
    view.reviewed(result, cx);
}

/// What a change says in the pane: its kind, and for a formatting change
/// what it changed.
pub fn label(view: &DocumentView, revision: &Revision) -> String {
    let kind = i18n::text(kind_key(revision));
    let changed = describe_formatting(&*view.document, revision);
    if changed.is_empty() {
        kind.to_string()
    } else {
        let names: Vec<&str> = changed.into_iter().map(i18n::text).collect();
        format!("{kind}: {}", names.join(", "))
    }
}

fn action(id: (&'static str, usize), key: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px_1p5()
        .rounded_md()
        .text_color(theme::accent())
        .cursor_pointer()
        .hover(|button| button.bg(theme::hovered()))
        .child(i18n::text(key))
}

/// The pane, beside the pages while it is open.
pub fn pane(view: &DocumentView, cx: &mut Context<DocumentView>) -> Option<AnyElement> {
    if !view.review.revisions_pane {
        return None;
    }
    let revisions = view.document.revisions();
    let count = revisions.len();
    let cards: Vec<AnyElement> = revisions
        .into_iter()
        .enumerate()
        .map(|(index, revision)| {
            let heading = label(view, &revision);
            let who = format!(
                "{}{}",
                revision.author.clone().unwrap_or_default(),
                revision
                    .date
                    .as_deref()
                    .and_then(|date| date.get(..10))
                    .map(|day| format!(" · {day}"))
                    .unwrap_or_default()
            );
            let text: String = revision.text.chars().take(60).collect();
            let (go, accept, reject) = (revision.clone(), revision.clone(), revision);
            div()
                .id(("revision-card", index))
                .flex()
                .flex_col()
                .gap_1()
                .p_2()
                .rounded_md()
                .border_1()
                .border_color(theme::border())
                .bg(theme::canvas())
                .cursor_pointer()
                .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                    view.select_range(go.start(), go.end(), cx)
                }))
                .child(div().text_color(theme::text()).child(heading))
                .child(div().text_color(theme::text_dim()).child(who))
                .when(!text.trim().is_empty(), |card| {
                    card.child(div().text_color(theme::text_faint()).child(text))
                })
                .child(
                    div()
                        .flex()
                        .gap_1()
                        .child(
                            action(("revision-accept", index), "review.accept").on_click(
                                cx.listener(move |view, _: &ClickEvent, _, cx| {
                                    cx.stop_propagation();
                                    decide(view, &accept, true, cx)
                                }),
                            ),
                        )
                        .child(
                            action(("revision-reject", index), "review.reject").on_click(
                                cx.listener(move |view, _: &ClickEvent, _, cx| {
                                    cx.stop_propagation();
                                    decide(view, &reject, false, cx)
                                }),
                            ),
                        ),
                )
                .into_any_element()
        })
        .collect();
    Some(
        div()
            .id("revisions-pane")
            .flex()
            .flex_col()
            .flex_none()
            .w(px(280.0))
            .h_full()
            .gap_2()
            .p_3()
            .overflow_y_scroll()
            .bg(theme::panel())
            .border_l_1()
            .border_color(theme::border())
            .text_xs()
            .text_color(theme::text())
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(div().text_sm().child(i18n::text("review.pane_title")))
                    .child(
                        div()
                            .id("revisions-close")
                            .px_1()
                            .rounded_md()
                            .cursor_pointer()
                            .hover(|close| close.bg(theme::hovered()))
                            .child("×")
                            .on_click(cx.listener(|view, _: &ClickEvent, _, cx| toggle(view, cx))),
                    ),
            )
            .child(
                div()
                    .text_color(theme::text_dim())
                    .child(i18n::format("review.pane.count", &[("n", &count)])),
            )
            .children(cards)
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bold put on with changes tracked is a formatting change the pane
    /// names as Word's balloon does; rejected from the pane, the text is not
    /// bold any more. Centring the paragraph is the paragraph's own change.
    #[gpui::test]
    fn formatting_changes_are_listed_and_rejected(cx: &mut gpui::TestAppContext) {
        use crate::Bold;
        use gpui::Focusable;
        use jade_office_model::word::WordFormatting;
        let bytes = jade_office_model::word::new::blank_document(
            jade_office_model::word::new::Blank::English,
        );
        let (view, cx) = cx.add_window_view(|_window, cx| {
            crate::untitled_view(&bytes, "formats.docx", cx).unwrap()
        });
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        view.update(cx, |view, _| {
            view.insert_at = (0, 0);
            view.caret = view.document.place_caret(0, 0);
        });
        cx.simulate_input("Some words here");
        view.update(cx, |view, cx| {
            crate::word_review_tools::set_reviewer(
                jade_office_model::word::reviewer::Reviewer::named("Ann"),
            );
            view.toggle_tracking(cx);
            view.document.select((0, 0), (0, 4), 1).unwrap();
            view.insert_at = (0, 4);
        });
        cx.dispatch_action(Bold);
        view.update(cx, |view, cx| {
            toggle(view, cx);
            let revisions = view.document.revisions();
            assert_eq!(revisions.len(), 1, "{revisions:?}");
            assert!(revisions[0].is_formatting());
            assert_eq!(label(view, &revisions[0]), "書式変更: 太字");
            decide(view, &revisions[0], false, cx);
            assert!(view.document.revisions().is_empty());
            assert_ne!(
                jade_docx_native_render::NativeDocument::run_style_at(&view.document, 0, 1)
                    .unwrap()
                    .bold,
                Some(true)
            );

            view.document.place_caret(0, 0);
            WordFormatting::set_alignment(&mut view.document, "center").unwrap();
            let revisions = view.document.revisions();
            assert_eq!(revisions.len(), 1, "{revisions:?}");
            assert_eq!(label(view, &revisions[0]), "書式変更: 段落の書式");
        });
    }
}

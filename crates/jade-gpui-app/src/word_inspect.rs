//! 校閲 › 保護 › ドキュメント検査: the pane beside the pages that lists what
//! the document carries besides its text — comments and changes, who wrote
//! and saved it, custom XML, hidden text, headers and footers — each with
//! すべて削除. What is found and removed is `jade-office-model`'s
//! `word::inspect`; personal information and custom XML are left out of
//! every later save ([`finish_save`]).

use gpui::{div, prelude::*, px, AnyElement, ClickEvent, Context};
use jade_office_model::word::inspect::{
    clean_package, InspectCategory, InspectReport, PackageCleaning, WordInspect,
};

use crate::word_characters::button;
use crate::{i18n, theme, DocumentView};

/// The pane's report, and the edit count it was read at.
#[derive(Default)]
pub struct InspectPane {
    report: Option<(u64, InspectReport)>,
}

/// ドキュメント検査: the pane, open or shut. Opening it inspects.
pub fn toggle(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    view.review.inspect = match view.review.inspect.take() {
        Some(_) => None,
        None => Some(InspectPane::default()),
    };
    cx.notify();
}

/// What the pane reports, read again after an edit.
pub fn report(view: &mut DocumentView) -> Option<InspectReport> {
    let edits = view.edits;
    let pane = view.review.inspect.as_mut()?;
    if pane.report.as_ref().is_none_or(|(at, _)| *at != edits) {
        pane.report = Some((edits, view.document.inspect()));
    }
    pane.report.as_ref().map(|(_, report)| report.clone())
}

/// すべて削除 for `category`.
pub fn remove(view: &mut DocumentView, category: InspectCategory, cx: &mut Context<DocumentView>) {
    match category {
        InspectCategory::Personal => {
            view.review.cleaning.personal = true;
            view.dirty = true;
        }
        InspectCategory::CustomXml => {
            view.review.cleaning.custom_xml = true;
            view.dirty = true;
        }
        _ => {
            let result = view.document.remove_inspected(category);
            view.reviewed(result, cx);
        }
    }
    if let Some(pane) = view.review.inspect.as_mut() {
        pane.report = None;
    }
    cx.notify();
}

/// The package a save writes, without what すべて削除 took out of it.
pub fn finish_save(view: &DocumentView, bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    clean_package(&bytes, view.review.cleaning)
}

/// One category's card: what was found, and すべて削除 while there is
/// something to remove.
fn card(
    index: usize,
    title: &'static str,
    found: Option<String>,
    removed_on_save: bool,
    category: InspectCategory,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    let status = match (&found, removed_on_save) {
        (_, true) => i18n::text("inspect.removed_on_save").to_string(),
        (Some(found), false) => found.clone(),
        (None, false) => i18n::text("inspect.none").to_string(),
    };
    div()
        .flex()
        .flex_col()
        .gap_1()
        .p_2()
        .rounded_md()
        .border_1()
        .border_color(theme::border())
        .bg(theme::canvas())
        .child(
            div()
                .flex()
                .gap_1()
                .child(
                    div()
                        .text_color(if found.is_some() && !removed_on_save {
                            theme::accent()
                        } else {
                            theme::text_dim()
                        })
                        .child(if found.is_some() && !removed_on_save {
                            "!"
                        } else {
                            "✓"
                        }),
                )
                .child(div().text_color(theme::text()).child(i18n::text(title))),
        )
        .child(div().text_color(theme::text_dim()).child(status))
        .when(found.is_some() && !removed_on_save, |card| {
            card.child(
                div().flex().child(
                    button(
                        ("inspect-remove", index),
                        i18n::text("inspect.remove_all"),
                        false,
                    )
                    .on_click(
                        cx.listener(move |view, _: &ClickEvent, _, cx| remove(view, category, cx)),
                    ),
                ),
            )
        })
        .into_any_element()
}

/// The pane, beside the pages while it is open.
pub fn pane(view: &mut DocumentView, cx: &mut Context<DocumentView>) -> Option<AnyElement> {
    let report = report(view)?;
    let cleaning: PackageCleaning = view.review.cleaning;
    let count = |n: usize| n.to_string();
    let found =
        |n: usize, key: &'static str| (n > 0).then(|| i18n::format(key, &[("n", &count(n))]));
    let comments = (report.comments + report.revisions > 0).then(|| {
        i18n::format(
            "inspect.comments_found",
            &[
                ("comments", &count(report.comments)),
                ("revisions", &count(report.revisions)),
            ],
        )
    });
    let personal = (!report.personal.is_empty()).then(|| {
        i18n::format(
            "inspect.personal_found",
            &[(
                "names",
                &report
                    .personal
                    .iter()
                    .map(|(_, value)| value.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            )],
        )
    });
    let cards = vec![
        card(
            0,
            "inspect.comments",
            comments,
            false,
            InspectCategory::CommentsAndRevisions,
            cx,
        ),
        card(
            1,
            "inspect.personal",
            personal,
            cleaning.personal,
            InspectCategory::Personal,
            cx,
        ),
        card(
            2,
            "inspect.custom_xml",
            found(report.custom_xml_parts, "inspect.custom_xml_found"),
            cleaning.custom_xml,
            InspectCategory::CustomXml,
            cx,
        ),
        card(
            3,
            "inspect.headers",
            found(report.header_footer_lines, "inspect.headers_found"),
            false,
            InspectCategory::HeadersFooters,
            cx,
        ),
        card(
            4,
            "inspect.hidden",
            found(report.hidden_runs, "inspect.hidden_found"),
            false,
            InspectCategory::HiddenText,
            cx,
        ),
    ];
    Some(
        div()
            .id("inspect-pane")
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
                    .child(div().text_sm().child(i18n::text("inspect.title")))
                    .child(
                        div()
                            .id("inspect-close")
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
                    .child(i18n::text("inspect.intro")),
            )
            .children(cards)
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::word_review_tools::ReviewAction;
    use jade_office_model::word::review::WordReview;

    /// The pane opens from the 校閲 tab, finds a comment and removes it;
    /// who wrote the document goes from what the save writes.
    #[gpui::test]
    fn the_inspector_removes_comments_and_personal_information(cx: &mut gpui::TestAppContext) {
        use gpui::Focusable;
        let bytes = jade_office_model::word::new::blank_document(
            jade_office_model::word::new::Blank::English,
        );
        let (view, cx) = cx.add_window_view(|_window, cx| {
            crate::untitled_view(&bytes, "inspect.docx", cx).unwrap()
        });
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        view.update(cx, |view, _| {
            view.insert_at = (0, 0);
            view.caret = view.document.place_caret(0, 0);
        });
        cx.simulate_input("Hello");
        view.update(cx, |view, cx| {
            view.document.select((0, 0), (0, 5), 1).unwrap();
            let ann = jade_office_model::word::reviewer::Reviewer::named("Ann Lee");
            view.document.add_comment("check", &ann).unwrap();
            view.edits += 1;
            crate::word_review_tools::run(view, ReviewAction::Inspect, cx);
            assert_eq!(report(view).unwrap().comments, 1);
            remove(view, InspectCategory::CommentsAndRevisions, cx);
            assert_eq!(report(view).unwrap().comments, 0);
            remove(view, InspectCategory::Personal, cx);
            assert!(view.review.cleaning.personal);
            let saved = finish_save(view, view.document.export_docx().unwrap()).unwrap();
            let package = jade_opc::OpcPackage::open(&saved).unwrap();
            if let Some(core) = package.part_str("docProps/core.xml").unwrap() {
                assert!(!core.contains("<dc:creator>") || core.contains("<dc:creator/>"));
            }
            toggle(view, cx);
            assert!(view.review.inspect.is_none());
        });
    }
}

//! 校閲 › アクセシビリティ チェック: the pane beside the pages that lists
//! what Word's checker would — エラー, 警告 and ヒント — each one a click
//! away in the document, with its おすすめのアクション: 説明を追加 (the
//! 代替テキスト dialog), 装飾用にする, ヘッダー行として使用, 行内に配置. What
//! is checked is `jade-office-model`'s `word::accessibility`.

use gpui::{div, prelude::*, px, AnyElement, ClickEvent, Context};
use jade_office_model::word::accessibility::{A11yIssue, IssueKind, Severity, WordAccessibility};

use crate::word_characters::button;
use crate::{i18n, theme, DocumentView};

/// The pane's issues, and the edit count they were read at: reading them
/// walks the whole document, so not on every frame.
#[derive(Default)]
pub struct A11yPane {
    issues: Option<(u64, Vec<A11yIssue>)>,
}

impl A11yPane {
    fn issues(&mut self, view_edits: u64, read: impl FnOnce() -> Vec<A11yIssue>) -> &[A11yIssue] {
        if self.issues.as_ref().is_none_or(|(at, _)| *at != view_edits) {
            self.issues = Some((view_edits, read()));
        }
        &self.issues.as_ref().expect("just read").1
    }
}

/// アクセシビリティ チェック: the pane, open or shut.
pub fn toggle(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    view.review.accessibility = match view.review.accessibility.take() {
        Some(_) => None,
        None => Some(A11yPane::default()),
    };
    cx.notify();
}

/// The issues the pane shows, read again after an edit.
pub fn issues(view: &mut DocumentView) -> Vec<A11yIssue> {
    let edits = view.edits;
    let document = &view.document;
    match view.review.accessibility.as_mut() {
        Some(pane) => pane
            .issues(edits, || document.accessibility_issues())
            .to_vec(),
        None => Vec::new(),
    }
}

/// What a pane button does.
#[derive(Clone, Debug, PartialEq)]
enum A11yAction {
    Show,
    Describe,
    Fix,
}

fn act(
    view: &mut DocumentView,
    issue: &A11yIssue,
    action: A11yAction,
    cx: &mut Context<DocumentView>,
) {
    match action {
        A11yAction::Show => {
            if issue.start() != issue.end() {
                view.select_range(issue.start(), issue.end(), cx);
            } else {
                view.select_range(issue.start(), issue.start(), cx);
            }
        }
        A11yAction::Describe => {
            if let IssueKind::MissingAltText(target) = issue.kind {
                crate::word_review_tools::open_alt_text_for(view, target, cx);
            }
        }
        A11yAction::Fix => {
            let result = view.document.quick_fix(issue);
            view.reviewed(result, cx);
        }
    }
}

fn severity_key(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "a11y.errors",
        Severity::Warning => "a11y.warnings",
        Severity::Tip => "a11y.tips",
    }
}

fn fix_key(kind: &IssueKind) -> &'static str {
    match kind {
        IssueKind::MissingAltText(_) => "a11y.fix.decorative",
        IssueKind::NoHeaderRow { .. } => "a11y.fix.header",
        IssueKind::NotInline(_) => "a11y.fix.inline",
        _ => "a11y.fix",
    }
}

/// The pane, beside the pages while it is open.
pub fn pane(view: &mut DocumentView, cx: &mut Context<DocumentView>) -> Option<AnyElement> {
    view.review.accessibility.as_ref()?;
    let issues = issues(view);
    let mut rows: Vec<AnyElement> = Vec::new();
    if issues.is_empty() {
        rows.push(
            div()
                .text_color(theme::text_dim())
                .child(i18n::text("a11y.none"))
                .into_any_element(),
        );
    }
    let mut shown: Option<Severity> = None;
    for (index, issue) in issues.into_iter().enumerate() {
        if shown != Some(issue.severity()) {
            shown = Some(issue.severity());
            rows.push(
                div()
                    .mt_1()
                    .text_color(theme::text_dim())
                    .child(i18n::text(severity_key(issue.severity())))
                    .into_any_element(),
            );
        }
        let detail = match &issue.kind {
            IssueKind::SkippedHeading { from, to } => {
                i18n::format("a11y.skipped_detail", &[("from", from), ("to", to)])
            }
            IssueKind::LowContrast { ratio_tenths } => i18n::format(
                "a11y.contrast_detail",
                &[("ratio", &format!("{:.1}", f64::from(*ratio_tenths) / 10.0))],
            ),
            _ => issue.excerpt.clone(),
        };
        let (show, describe, fix) = (issue.clone(), issue.clone(), issue.clone());
        rows.push(
            div()
                .id(("a11y-issue", index))
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
                    act(view, &show, A11yAction::Show, cx)
                }))
                .child(
                    div()
                        .text_color(theme::text())
                        .child(i18n::text(issue.kind.label_key())),
                )
                .when(!detail.is_empty(), |card| {
                    card.child(div().text_color(theme::text_dim()).child(detail))
                })
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_1()
                        .when(matches!(issue.kind, IssueKind::MissingAltText(_)), |row| {
                            row.child(
                                button(
                                    ("a11y-describe", index),
                                    i18n::text("a11y.fix.describe"),
                                    true,
                                )
                                .on_click(cx.listener(
                                    move |view, _: &ClickEvent, _, cx| {
                                        cx.stop_propagation();
                                        act(view, &describe, A11yAction::Describe, cx)
                                    },
                                )),
                            )
                        })
                        .when(issue.kind.has_quick_fix(), |row| {
                            row.child(
                                button(
                                    ("a11y-fix", index),
                                    i18n::text(fix_key(&issue.kind)),
                                    false,
                                )
                                .on_click(cx.listener(
                                    move |view, _: &ClickEvent, _, cx| {
                                        cx.stop_propagation();
                                        act(view, &fix, A11yAction::Fix, cx)
                                    },
                                )),
                            )
                        }),
                )
                .into_any_element(),
        );
    }
    Some(
        div()
            .id("accessibility-pane")
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
                    .child(div().text_sm().child(i18n::text("a11y.title")))
                    .child(
                        div()
                            .id("a11y-close")
                            .px_1()
                            .rounded_md()
                            .cursor_pointer()
                            .hover(|close| close.bg(theme::hovered()))
                            .child("×")
                            .on_click(cx.listener(|view, _: &ClickEvent, _, cx| toggle(view, cx))),
                    ),
            )
            .children(rows)
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::word_review_tools::{ReviewAction, ReviewDialog};

    /// The checker finds a picture without alternative text; the
    /// 代替テキスト dialog takes a description from the keyboard and the
    /// issue is gone; 装飾用にする is the other way out.
    #[gpui::test]
    fn alt_text_is_written_from_the_checker(cx: &mut gpui::TestAppContext) {
        use gpui::Focusable;
        let bytes = jade_office_model::word::new::blank_document(
            jade_office_model::word::new::Blank::English,
        );
        let (view, cx) = cx
            .add_window_view(|_window, cx| crate::untitled_view(&bytes, "a11y.docx", cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        let mut png = Vec::new();
        image::RgbaImage::from_raw(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 255])
            .unwrap()
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        view.update(cx, |view, cx| {
            view.insert_at = (0, 0);
            view.caret = view.document.place_caret(0, 0);
            view.insert_picture(png, cx);
            crate::word_review_tools::run(view, ReviewAction::Accessibility, cx);
            let found = issues(view);
            let issue = found
                .iter()
                .find(|issue| matches!(issue.kind, IssueKind::MissingAltText(_)))
                .expect("the picture has no alternative text")
                .clone();
            act(view, &issue, A11yAction::Describe, cx);
            assert!(matches!(view.review.dialog, Some(ReviewDialog::AltText(_))));
        });
        cx.simulate_input("A red pixel");
        cx.simulate_keystrokes("enter");
        view.update(cx, |view, cx| {
            assert!(view.review.dialog.is_none());
            assert!(!issues(view)
                .iter()
                .any(|issue| matches!(issue.kind, IssueKind::MissingAltText(_))));
            let target = view
                .document
                .word_document()
                .paragraph(0)
                .and_then(|paragraph| paragraph.runs.iter().position(|run| run.image.is_some()))
                .map(|run_index| jade_office_model::word::picture::ObjectTarget {
                    paragraph_index: 0,
                    run_index,
                    kind: jade_office_model::word::picture::ObjectKind::Image,
                })
                .unwrap();
            assert_eq!(
                view.document
                    .alt_text(target)
                    .unwrap()
                    .description
                    .as_deref(),
                Some("A red pixel")
            );
            toggle(view, cx);
            assert!(view.review.accessibility.is_none());
        });
    }
}

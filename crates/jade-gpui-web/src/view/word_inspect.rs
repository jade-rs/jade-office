//! 校閲 › 保護 › ドキュメント検査 in the browser: the pane beside the pages
//! that lists what the document carries besides its text, each category
//! with すべて削除, as the native app's `word_inspect`. What is found and
//! removed is `jade-office-model`'s `word::inspect`; personal information
//! and custom XML are left out of every later save (`finish_save`).

use jade_web::word::inspect::{InspectCategory, WordInspect};

use super::*;
use crate::word_review::ReviewAction;

impl DocView {
    /// ドキュメント検査 and its すべて削除.
    pub(super) fn inspect_action(&mut self, action: ReviewAction, cx: &mut Context<Self>) {
        let Document::Pages { protection, .. } = &mut self.document else {
            return;
        };
        match action {
            ReviewAction::Inspect => protection.inspect = !protection.inspect,
            ReviewAction::InspectRemove(InspectCategory::Personal) => {
                protection.cleaning.personal = true;
            }
            ReviewAction::InspectRemove(InspectCategory::CustomXml) => {
                protection.cleaning.custom_xml = true;
            }
            ReviewAction::InspectRemove(category) => {
                return self.word_edit(cx, move |document| document.remove_inspected(category));
            }
            _ => {}
        }
        cx.notify();
    }

    /// The ドキュメント検査 pane, while it is open.
    pub(super) fn inspect_pane(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let Document::Pages {
            document,
            protection,
            ..
        } = &self.document
        else {
            return None;
        };
        if !protection.inspect {
            return None;
        }
        let report = document.inspect();
        let cleaning = protection.cleaning;
        let found = |n: usize, key: &'static str| (n > 0).then(|| i18n::format(key, &[("n", &n)]));
        let comments = (report.comments + report.revisions > 0).then(|| {
            i18n::format(
                "inspect.comments_found",
                &[
                    ("comments", &report.comments),
                    ("revisions", &report.revisions),
                ],
            )
        });
        let names = report
            .personal
            .iter()
            .map(|(_, value)| value.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let personal = (!report.personal.is_empty())
            .then(|| i18n::format("inspect.personal_found", &[("names", &names)]));
        let categories = [
            (
                "inspect.comments",
                comments,
                false,
                InspectCategory::CommentsAndRevisions,
            ),
            (
                "inspect.personal",
                personal,
                cleaning.personal,
                InspectCategory::Personal,
            ),
            (
                "inspect.custom_xml",
                found(report.custom_xml_parts, "inspect.custom_xml_found"),
                cleaning.custom_xml,
                InspectCategory::CustomXml,
            ),
            (
                "inspect.headers",
                found(report.header_footer_lines, "inspect.headers_found"),
                false,
                InspectCategory::HeadersFooters,
            ),
            (
                "inspect.hidden",
                found(report.hidden_runs, "inspect.hidden_found"),
                false,
                InspectCategory::HiddenText,
            ),
        ];
        let mut cards: Vec<gpui::AnyElement> = Vec::new();
        for (index, (title, found, on_save, category)) in categories.into_iter().enumerate() {
            let open = found.is_some() && !on_save;
            let status = if on_save {
                i18n::text("inspect.removed_on_save").to_string()
            } else {
                found.unwrap_or_else(|| i18n::text("inspect.none").to_string())
            };
            cards.push(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_2()
                    .rounded_md()
                    .border_1()
                    .border_color(palette::border())
                    .bg(palette::canvas())
                    .child(div().text_color(palette::text()).child(format!(
                        "{} {}",
                        if open { "!" } else { "✓" },
                        i18n::text(title)
                    )))
                    .child(div().text_color(palette::dim()).child(status))
                    .when(open, |card| {
                        card.child(
                            div().flex().child(
                                div()
                                    .id(("inspect-remove", index))
                                    .px_1p5()
                                    .rounded_md()
                                    .text_color(palette::accent())
                                    .cursor_pointer()
                                    .hover(|button| button.bg(palette::hovered()))
                                    .child(i18n::text("inspect.remove_all"))
                                    .on_click(cx.listener(move |view, _event, _window, cx| {
                                        view.inspect_action(
                                            ReviewAction::InspectRemove(category),
                                            cx,
                                        )
                                    })),
                            ),
                        )
                    })
                    .into_any_element(),
            );
        }
        Some(
            div()
                .id("inspect-pane")
                .flex()
                .flex_col()
                .flex_none()
                .w(px(280.0))
                .h_full()
                .gap_2()
                .px_3()
                .pt_3()
                .overflow_y_scroll()
                .bg(palette::panel())
                .border_l_1()
                .border_color(palette::border())
                .text_xs()
                .child(
                    div()
                        .text_sm()
                        .text_color(palette::text())
                        .child(i18n::text("inspect.title")),
                )
                .child(
                    div()
                        .text_color(palette::dim())
                        .child(i18n::text("inspect.intro")),
                )
                .children(cards)
                .into_any_element(),
        )
    }
}

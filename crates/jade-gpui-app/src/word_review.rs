//! The 校閲 tab: comments — written in a bar over the pages, answered,
//! edited, resolved and deleted from a pane beside them — and tracked
//! changes — 記録 on and off, 前へ / 次へ, 承諾 and 元に戻す for the change
//! at the selection or all of them. What each does is `jade-office-model`'s
//! `WordReview`; the browser shell has the same (`jade-gpui-web/src/view/
//! word_review.rs`).

use gpui::{div, prelude::*, px, AnyElement, Context, SharedString};
use jade_office_model::text::TextEdit;
use jade_office_model::word::review::{Comment, Revision, WordReview};

use crate::word_review_tools::{self as tools, reviewer, ReviewAction};
use crate::{t, theme, DocumentView};

/// What a comment bar's text becomes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommentEntry {
    /// 新しいコメント, on the selection or the caret's paragraph.
    New,
    /// 返信 to the comment with this id.
    Reply(String),
    /// The comment with this id, rewritten.
    Edit(String),
}

/// A comment being typed, in its bar over the pages.
pub struct CommentBar {
    pub entry: CommentEntry,
    pub text: TextEdit,
}

impl DocumentView {
    /// 新しいコメント (⌘M): the bar, and the pane to see it land in.
    pub(crate) fn add_comment(&mut self, cx: &mut Context<Self>) {
        self.open_comment_bar(CommentEntry::New, String::new(), cx);
    }

    pub(crate) fn open_comment_bar(
        &mut self,
        entry: CommentEntry,
        text: String,
        cx: &mut Context<Self>,
    ) {
        self.find_bar = None;
        self.story_bar = None;
        self.dialog = None;
        self.show_comments = true;
        self.comment_bar = Some(CommentBar {
            entry,
            text: TextEdit::new(text),
        });
        cx.notify();
    }

    /// Keys the comment bar takes: Enter puts the comment in (⌥Enter a line
    /// break in it), Escape leaves it.
    pub(crate) fn handle_comment_bar_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(bar) = self.comment_bar.as_mut() else {
            return false;
        };
        if bar.text.marked.is_some() {
            return false;
        }
        let modifiers = event.keystroke.modifiers;
        match event.keystroke.key.as_str() {
            "escape" => self.comment_bar = None,
            "enter" | "return" if modifiers.alt || modifiers.shift => bar.text.insert("\n"),
            "enter" | "return" => {
                let Some(bar) = self.comment_bar.take() else {
                    return false;
                };
                let text = bar.text.text.trim().to_string();
                if !text.is_empty() {
                    let result = match &bar.entry {
                        CommentEntry::New => {
                            self.document.add_comment(&text, &reviewer()).map(|_| ())
                        }
                        CommentEntry::Reply(id) => {
                            self.document.reply_to_comment(id, &text, &reviewer())
                        }
                        CommentEntry::Edit(id) => self.document.edit_comment(id, &text),
                    };
                    self.reviewed(result, cx);
                }
            }
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

    /// The tail of a review command: every page re-read, the caret put back.
    pub(crate) fn reviewed(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        match result {
            Ok(()) => {
                self.dirty = true;
                self.table_status = None;
                if self.document.selection().is_none() {
                    self.caret = self
                        .document
                        .place_caret(self.insert_at.0, self.insert_at.1);
                }
                // A new reviewer takes the next colour.
                tools::refresh_markup(self);
                self.rebuild_all_images();
            }
            Err(error) => self.table_status = Some(SharedString::from(error)),
        }
        cx.notify();
    }

    pub(crate) fn resolve_comment(&mut self, id: &str, resolved: bool, cx: &mut Context<Self>) {
        let result = self.document.resolve_comment(id, resolved);
        self.reviewed(result, cx);
    }

    pub(crate) fn delete_comment(&mut self, id: &str, cx: &mut Context<Self>) {
        let result = self.document.delete_comment(id);
        self.reviewed(result, cx);
    }

    /// A comment clicked in the pane: its text selected in the document.
    pub(crate) fn go_to_comment(&mut self, comment: &Comment, cx: &mut Context<Self>) {
        if let Some((start, end)) = comment.anchor {
            self.select_range(start, end, cx);
        }
    }

    /// Selects `start..end` and brings it into view.
    pub(crate) fn select_range(
        &mut self,
        start: (usize, usize),
        end: (usize, usize),
        cx: &mut Context<Self>,
    ) {
        self.caret = self.document.place_caret(end.0, end.1);
        self.insert_at = end;
        self.selection_anchor = Some(start);
        if start != end {
            let page = self.caret_page_1idx();
            if let Err(error) = self.document.select(start, end, page) {
                eprintln!("select failed: {error}");
            }
        }
        if let Some(caret) = self.caret {
            let (page, _) = self.document.page_and_offset(caret.document_y);
            self.scroll.scroll_to_item(page);
            self.current_page = page;
        }
        self.refresh_caret_page_image();
        cx.notify();
    }

    /// 変更履歴の記録.
    pub(crate) fn toggle_tracking(&mut self, cx: &mut Context<Self>) {
        let on = !self.document.is_tracking_changes();
        let result = self.document.set_tracking_changes(on, &reviewer());
        self.reviewed(result, cx);
    }

    /// 前へ / 次へ: the change before or after the caret, selected.
    pub(crate) fn step_revision(&mut self, forwards: bool, cx: &mut Context<Self>) {
        let from = match self.document.selection() {
            Some((anchor, focus)) if forwards => anchor.max(focus),
            Some((anchor, focus)) => anchor.min(focus),
            None => self.insert_at,
        };
        match self.document.step_revision(from, forwards) {
            Some(revision) => self.select_revision(&revision, cx),
            None => {
                self.table_status = Some(t("review.no_changes").into());
                cx.notify();
            }
        }
    }

    fn select_revision(&mut self, revision: &Revision, cx: &mut Context<Self>) {
        self.select_range(revision.start(), revision.end(), cx);
    }

    /// 承諾 / 元に戻す: the changes at the selection (or the caret) decided,
    /// and the next one selected, as Word's 承諾して次へ進む.
    pub(crate) fn decide_revision(&mut self, accept: bool, cx: &mut Context<Self>) {
        // The engine reads the selection; the caret is where the view put it.
        if self.document.selection().is_none() {
            self.document
                .place_caret(self.insert_at.0, self.insert_at.1);
        }
        match self.document.decide_and_step(accept) {
            Ok(next) => {
                self.reviewed(Ok(()), cx);
                if let Some(next) = next {
                    self.select_revision(&next, cx);
                }
            }
            Err(error) => self.reviewed(Err(error), cx),
        }
    }

    /// The 校閲 tab's groups, as Word for Mac lays them out: コメント,
    /// 変更履歴 (記録, the markup view and what it shows, ユーザー情報) and
    /// 変更箇所 (承諾 and 元に戻す with their lists, 前へ and 次へ).
    pub(crate) fn review_groups(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let tracking = self.document.is_tracking_changes();
        let showing = self.show_comments;
        let button = |view: &Self,
                      id: &'static str,
                      key: &'static str,
                      title: &'static str,
                      active: bool,
                      cx: &mut Context<Self>,
                      action: fn(&mut Self, &mut Context<Self>)| {
            view.ribbon_button(id, t(key), t(title), active, cx, action)
                .into_any_element()
        };
        let markup_row = tools::markup_row(self, cx);
        let mut decisions = tools::decision_buttons(self, cx);
        decisions.extend([
            button(
                self,
                "revision-previous",
                "review.previous",
                "review.previous_title",
                false,
                cx,
                |view, cx| view.step_revision(false, cx),
            ),
            button(
                self,
                "revision-next",
                "review.next",
                "review.next_title",
                false,
                cx,
                |view, cx| view.step_revision(true, cx),
            ),
        ]);
        let proofing =
            jade_office_model::word::proofing::WordProofingDisplay::proofing(&*self.document);
        vec![
            Self::ribbon_group(
                t("proof.group"),
                vec![Self::ribbon_row(vec![
                    tools::text_button(
                        "review-proofing",
                        "proof.button",
                        "proof.title",
                        self.review.proofing.pane.is_some(),
                        ReviewAction::Proofing,
                        cx,
                    ),
                    tools::text_button(
                        "review-proofing-marks",
                        "proof.marks",
                        "proof.marks_title",
                        proofing,
                        ReviewAction::ProofingMarks,
                        cx,
                    ),
                ])],
            )
            .into_any_element(),
            Self::ribbon_group(
                t("doc.comment"),
                vec![
                    Self::ribbon_row(vec![
                        button(
                            self,
                            "review-comment",
                            "review.new_comment",
                            "review.new_comment",
                            false,
                            cx,
                            |view, cx| view.add_comment(cx),
                        ),
                        tools::text_button(
                            "review-comment-delete",
                            "review.delete",
                            "review.delete_comment",
                            false,
                            ReviewAction::DeleteComment,
                            cx,
                        ),
                        button(
                            self,
                            "review-comments",
                            "review.show_comments",
                            "review.show_comments",
                            showing,
                            cx,
                            |view, cx| {
                                view.show_comments = !view.show_comments;
                                cx.notify();
                            },
                        ),
                    ]),
                    Self::ribbon_row(vec![
                        tools::text_button(
                            "review-comment-previous",
                            "review.previous",
                            "review.previous_comment",
                            false,
                            ReviewAction::StepComment(false),
                            cx,
                        ),
                        tools::text_button(
                            "review-comment-next",
                            "review.next",
                            "review.next_comment",
                            false,
                            ReviewAction::StepComment(true),
                            cx,
                        ),
                    ]),
                ],
            )
            .into_any_element(),
            Self::ribbon_group(
                t("doc.group.tracking"),
                vec![
                    Self::ribbon_row(vec![
                        button(
                            self,
                            "track",
                            "doc.track",
                            "doc.track_title",
                            tracking,
                            cx,
                            |view, cx| view.toggle_tracking(cx),
                        ),
                        tools::text_button(
                            "review-user",
                            "review.user",
                            "review.user.title",
                            false,
                            ReviewAction::UserInfo,
                            cx,
                        ),
                    ]),
                    Self::ribbon_row(markup_row),
                ],
            )
            .into_any_element(),
            Self::ribbon_group(t("review.group.changes"), vec![Self::ribbon_row(decisions)])
                .into_any_element(),
        ]
    }

    /// The comment bar, over the pages while one is being typed.
    pub(crate) fn comment_bar_element(&self) -> Option<AnyElement> {
        let bar = self.comment_bar.as_ref()?;
        let (before, after) = bar.text.split();
        let label = t(match bar.entry {
            CommentEntry::New => "review.new_comment",
            CommentEntry::Reply(_) => "review.reply",
            CommentEntry::Edit(_) => "review.edit",
        });
        Some(
            div()
                .flex()
                .items_center()
                .gap_3()
                .flex_none()
                .px_4()
                .py_2()
                .bg(theme::panel())
                .border_b_1()
                .border_color(theme::border())
                .child(div().text_xs().text_color(theme::text_faint()).child(label))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .min_w(px(360.0))
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .bg(theme::canvas())
                        .border_1()
                        .border_color(theme::accent())
                        .text_sm()
                        .text_color(theme::text())
                        .child(before.replace('\n', " ⏎ "))
                        .child(div().w(px(1.0)).h(px(16.0)).bg(theme::accent()))
                        .child(after.replace('\n', " ⏎ ")),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_faint())
                        .child(t("review.comment_keys")),
                )
                .into_any_element(),
        )
    }

    /// The comments pane: each comment with its replies, and what can be
    /// done to it.
    pub(crate) fn comments_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let comments = self.document.comments();
        let mut rows: Vec<AnyElement> = Vec::new();
        for (index, comment) in comments.into_iter().enumerate() {
            rows.push(self.comment_card(index, comment, cx));
        }
        if rows.is_empty() {
            rows.push(
                div()
                    .text_xs()
                    .text_color(theme::text_faint())
                    .child(t("review.no_comments"))
                    .into_any_element(),
            );
        }
        div()
            .id("comments-pane")
            .flex()
            .flex_col()
            .flex_none()
            .w(px(280.0))
            .h_full()
            .gap_2()
            .px_3()
            .pt_3()
            .overflow_y_scroll()
            .bg(theme::panel())
            .border_l_1()
            .border_color(theme::border())
            .child(
                div()
                    .text_sm()
                    .text_color(theme::text())
                    .child(t("review.comments")),
            )
            .children(rows)
            .into_any_element()
    }

    fn comment_card(&self, index: usize, comment: Comment, cx: &mut Context<Self>) -> AnyElement {
        let line = |comment: &Comment| {
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(div().text_xs().text_color(theme::text_dim()).child(format!(
                            "{}{}",
                            comment.author.clone().unwrap_or_default(),
                            comment
                                .date
                                .as_deref()
                                .and_then(|date| date.get(..10))
                                .map(|day| format!(" · {day}"))
                                .unwrap_or_default()
                        )))
                .child(
                    div()
                        .text_sm()
                        .text_color(if comment.resolved {
                            theme::text_faint()
                        } else {
                            theme::text()
                        })
                        .child(comment.text.clone()),
                )
        };
        let action = |id: (&'static str, usize), key: &'static str| {
            div()
                .id(id)
                .px_1p5()
                .rounded_md()
                .text_xs()
                .text_color(theme::accent())
                .cursor_pointer()
                .hover(|button| button.bg(theme::hovered()))
                .child(t(key))
        };
        let id = comment.id.clone();
        let (reply_id, edit_id, edit_text, resolve_id, delete_id) = (
            id.clone(),
            id.clone(),
            comment.text.clone(),
            id.clone(),
            id.clone(),
        );
        let resolved = comment.resolved;
        let target = comment.clone();
        div()
            .id(("comment", index))
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(theme::border())
            .bg(theme::canvas())
            .cursor_pointer()
            .on_click(cx.listener(move |view, _event, _window, cx| view.go_to_comment(&target, cx)))
            .child(line(&comment))
            .children(comment.replies.iter().map(|reply| {
                div()
                    .pl_3()
                    .border_l_2()
                    .border_color(theme::border())
                    .child(line(reply))
            }))
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(
                        action(("comment-reply", index), "review.reply").on_click(cx.listener(
                            move |view, _event, _window, cx| {
                                view.open_comment_bar(
                                    CommentEntry::Reply(reply_id.clone()),
                                    String::new(),
                                    cx,
                                )
                            },
                        )),
                    )
                    .child(
                        action(("comment-edit", index), "review.edit").on_click(cx.listener(
                            move |view, _event, _window, cx| {
                                view.open_comment_bar(
                                    CommentEntry::Edit(edit_id.clone()),
                                    edit_text.clone(),
                                    cx,
                                )
                            },
                        )),
                    )
                    .child(
                        action(
                            ("comment-resolve", index),
                            if resolved {
                                "review.reopen"
                            } else {
                                "review.resolve"
                            },
                        )
                        .on_click(cx.listener(
                            move |view, _event, _window, cx| {
                                view.resolve_comment(&resolve_id, !resolved, cx)
                            },
                        )),
                    )
                    .child(action(("comment-delete", index), "review.delete").on_click(
                        cx.listener(move |view, _event, _window, cx| {
                            view.delete_comment(&delete_id, cx)
                        }),
                    )),
            )
            .into_any_element()
    }
}

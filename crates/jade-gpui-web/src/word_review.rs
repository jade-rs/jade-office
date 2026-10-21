//! The 校閲 tab's groups: コメント — a new comment, the comments pane — and
//! 変更履歴 — 記録, 前へ, 次へ, 承諾, 元に戻す, and all of them at once. The
//! native app has the same (`jade-gpui-app/src/word_review.rs`), over the
//! same commands (`jade-office-model`'s `WordReview`).

use gpui::{AnyElement, Context};

use crate::i18n;
use crate::ribbon::{group, row, Action};
use crate::view::DocView;
use crate::word_page::text_button;

/// What the 校閲 tab's buttons do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReviewAction {
    NewComment,
    ToggleComments,
    ToggleTracking,
    /// 次へ (`true`) or 前へ.
    Step(bool),
    /// 承諾 (`true`) or 元に戻す, for the change at the selection.
    Decide(bool),
    /// すべて承諾 or すべて元に戻す.
    DecideAll(bool),
}

pub fn review(tracking: bool, showing: bool, cx: &mut Context<DocView>) -> Vec<AnyElement> {
    let review = |action| Action::Review(action);
    vec![
        group(
            i18n::text("doc.comment"),
            vec![row(vec![
                text_button(
                    "review-comment",
                    i18n::text("review.new_comment"),
                    i18n::text("review.new_comment"),
                    false,
                    review(ReviewAction::NewComment),
                    cx,
                ),
                text_button(
                    "review-comments",
                    i18n::text("review.show_comments"),
                    i18n::text("review.show_comments"),
                    showing,
                    review(ReviewAction::ToggleComments),
                    cx,
                ),
            ])],
        ),
        group(
            i18n::text("doc.group.tracking"),
            vec![
                row(vec![
                    text_button(
                        "track",
                        i18n::text("doc.track"),
                        i18n::text("doc.track_title"),
                        tracking,
                        review(ReviewAction::ToggleTracking),
                        cx,
                    ),
                    text_button(
                        "revision-previous",
                        i18n::text("review.previous"),
                        i18n::text("review.previous_title"),
                        false,
                        review(ReviewAction::Step(false)),
                        cx,
                    ),
                    text_button(
                        "revision-next",
                        i18n::text("review.next"),
                        i18n::text("review.next_title"),
                        false,
                        review(ReviewAction::Step(true)),
                        cx,
                    ),
                ]),
                row(vec![
                    text_button(
                        "accept",
                        i18n::text("review.accept"),
                        i18n::text("review.accept_title"),
                        false,
                        review(ReviewAction::Decide(true)),
                        cx,
                    ),
                    text_button(
                        "reject",
                        i18n::text("review.reject"),
                        i18n::text("review.reject_title"),
                        false,
                        review(ReviewAction::Decide(false)),
                        cx,
                    ),
                    text_button(
                        "accept-all",
                        i18n::text("doc.accept_all"),
                        i18n::text("doc.accept_all"),
                        false,
                        review(ReviewAction::DecideAll(true)),
                        cx,
                    ),
                    text_button(
                        "reject-all",
                        i18n::text("doc.reject_all"),
                        i18n::text("doc.reject_all_title"),
                        false,
                        review(ReviewAction::DecideAll(false)),
                        cx,
                    ),
                ]),
            ],
        ),
    ]
}

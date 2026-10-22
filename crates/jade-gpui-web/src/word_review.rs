//! The 校閲 tab's groups: コメント — a new comment, 削除, 前へ, 次へ, the
//! comments pane — 変更履歴 — 記録, ユーザー情報, 変更内容の表示 and
//! 変更履歴とコメントの表示 — and 変更箇所 — 承諾 and 元に戻す with their
//! lists, 前へ, 次へ. The native app has the same
//! (`jade-gpui-app/src/word_review.rs`, `word_review_tools.rs`), over the
//! same commands (`jade-office-model`'s `WordReview` and `WordMarkup`).

use gpui::{div, prelude::*, px, AnyElement, Context};
use jade_web::word::markup::{MarkupMode, MarkupOptions};

use crate::i18n;
use crate::palette;
use crate::ribbon::{
    dropdown, group, listener, menu_caption, menu_panel, menu_row, row, Action, Popup,
};
use crate::tooltip::Tooltip;
use crate::view::DocView;
use crate::word_page::text_button;

/// The lists the 校閲 tab drops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewPopup {
    /// 変更内容の表示.
    Markup,
    /// 変更履歴とコメントの表示.
    ShowMarkup,
    Accept,
    Reject,
}

/// Which changes a 承諾 or 元に戻す entry decides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// 承諾して次へ進む.
    AndNext,
    /// この変更を反映.
    This,
    /// 表示されたすべての変更を反映.
    Visible,
    /// すべての変更を反映.
    All,
    /// すべての変更を反映し、変更の記録を停止.
    AllAndStop,
}

/// What the 校閲 tab's buttons do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReviewAction {
    NewComment,
    ToggleComments,
    /// コメント › 前へ (`false`) / 次へ.
    StepComment(bool),
    /// コメント › 削除: the comment at the selection.
    DeleteComment,
    ToggleTracking,
    /// 次へ (`true`) or 前へ.
    Step(bool),
    /// 承諾 (`true`) or 元に戻す.
    Decide(bool, Decision),
    Markup(MarkupMode),
    ShowInsertionsDeletions,
    ShowFormatting,
    /// A reviewer ticked or unticked; `None` is すべての校閲者.
    Reviewer(Option<String>),
    /// ユーザー情報.
    UserInfo,
    /// アクセシビリティ チェック: the pane.
    Accessibility,
    /// 代替テキスト… for the picture last pressed on.
    AltText,
    /// 変更履歴ウィンドウ.
    RevisionsPane,
    /// 比較… (`false`) or 組み込み….
    Compare(bool),
    /// 編集の制限: the pane.
    RestrictPane,
    /// The pane's choices.
    RestrictKind(Option<jade_web::word::protection::EditingKind>),
    RestrictFormatting,
    RestrictEveryone,
    /// 保護の開始 / 保護の中止: the password bar.
    RestrictStartStop,
    /// 文書の保護….
    Protect,
    /// 最終版にする.
    MarkFinal,
    /// 編集する, on the read-only bar.
    EditAnyway,
    /// ドキュメント検査: the pane.
    Inspect,
    /// 読み上げ: start or stop.
    ReadAloud,
    /// イマーシブ リーダー: open or close.
    Immersive,
    /// 署名の表示: the pane.
    SignaturesPane,
    /// 署名欄.
    SignatureLine,
    /// 並べて比較, 同時にスクロール, ウィンドウの位置を元に戻す.
    SideBySide,
    SyncScroll,
    ResetPosition,
    /// ドキュメント検査's すべて削除.
    InspectRemove(jade_web::word::inspect::InspectCategory),
}

/// What the tab shows as current.
pub struct ReviewShown {
    pub tracking: bool,
    pub comments: bool,
    pub accessibility: bool,
    pub revisions_pane: bool,
    pub restrict_pane: bool,
    pub final_: bool,
    pub inspect: bool,
    pub signatures_pane: bool,
    pub reading: bool,
    pub immersive: bool,
    pub side_by_side: bool,
    pub sync_scroll: bool,
    pub markup: MarkupOptions,
    pub reviewers: Vec<String>,
}

fn act(action: ReviewAction) -> Action {
    Action::Review(action)
}

fn list_button(
    id: &'static str,
    label: &'static str,
    tooltip: &'static str,
    open: bool,
    popup: ReviewPopup,
    cx: &mut Context<DocView>,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap_0p5()
        .h(px(26.0))
        .px_2()
        .rounded_md()
        .text_xs()
        .whitespace_nowrap()
        .text_color(palette::text())
        .cursor_pointer()
        .when(open, |button| button.bg(palette::selected()))
        .hover(|button| button.bg(palette::hovered()))
        .tooltip(Tooltip::text(tooltip))
        .on_click(listener(Action::Popup(Popup::Review(popup)), cx))
        .child(label)
        .child(
            gpui::svg()
                .path("icons/chevron-down.svg")
                .size_3()
                .text_color(palette::dim()),
        )
        .into_any_element()
}

fn entry(
    id: impl Into<gpui::ElementId>,
    key: &'static str,
    checked: bool,
    action: ReviewAction,
    cx: &mut Context<DocView>,
) -> AnyElement {
    menu_row(id, i18n::text(key), checked, act(action), cx)
}

fn decision_list(accept: bool, open: Option<Popup>, cx: &mut Context<DocView>) -> AnyElement {
    let popup = if accept {
        ReviewPopup::Accept
    } else {
        ReviewPopup::Reject
    };
    let menu = (open == Some(Popup::Review(popup))).then(|| {
        let keys: [(&'static str, Decision); 5] = if accept {
            [
                ("review.accept_title", Decision::AndNext),
                ("review.accept.this", Decision::This),
                ("review.accept.visible", Decision::Visible),
                ("review.accept.all", Decision::All),
                ("review.accept.all_stop", Decision::AllAndStop),
            ]
        } else {
            [
                ("review.reject_title", Decision::AndNext),
                ("review.reject.this", Decision::This),
                ("review.reject.visible", Decision::Visible),
                ("review.reject.all", Decision::All),
                ("review.reject.all_stop", Decision::AllAndStop),
            ]
        };
        menu_panel(if accept {
            "review-accept-menu"
        } else {
            "review-reject-menu"
        })
        .children(
            keys.into_iter()
                .enumerate()
                .map(|(index, (key, decision))| {
                    entry(
                        (
                            if accept {
                                "review-accept"
                            } else {
                                "review-reject"
                            },
                            index,
                        ),
                        key,
                        false,
                        ReviewAction::Decide(accept, decision),
                        cx,
                    )
                }),
        )
        .into_any_element()
    });
    dropdown(
        list_button(
            if accept { "accept" } else { "reject" },
            i18n::text(if accept {
                "review.accept"
            } else {
                "review.reject"
            }),
            i18n::text(if accept {
                "review.accept_title"
            } else {
                "review.reject_title"
            }),
            menu.is_some(),
            popup,
            cx,
        ),
        menu,
    )
}

pub fn review(
    shown: &ReviewShown,
    open: Option<Popup>,
    cx: &mut Context<DocView>,
) -> Vec<AnyElement> {
    let markup = &shown.markup;
    let modes = (open == Some(Popup::Review(ReviewPopup::Markup))).then(|| {
        menu_panel("review-markup-menu")
            .children(
                MarkupMode::ALL
                    .into_iter()
                    .enumerate()
                    .map(|(index, mode)| {
                        entry(
                            ("review-markup", index),
                            mode.label_key(),
                            markup.mode == mode,
                            ReviewAction::Markup(mode),
                            cx,
                        )
                    }),
            )
            .into_any_element()
    });
    let show = (open == Some(Popup::Review(ReviewPopup::ShowMarkup))).then(|| {
        let mut rows = vec![
            entry(
                "review-show-comments",
                "review.show.comments",
                shown.comments,
                ReviewAction::ToggleComments,
                cx,
            ),
            entry(
                "review-show-insertions",
                "review.show.insertions",
                markup.insertions_deletions,
                ReviewAction::ShowInsertionsDeletions,
                cx,
            ),
            entry(
                "review-show-formatting",
                "review.show.formatting",
                markup.formatting,
                ReviewAction::ShowFormatting,
                cx,
            ),
            menu_caption(i18n::text("review.show.reviewers")),
            entry(
                "review-show-everyone",
                "review.show.everyone",
                markup.hidden_reviewers.is_empty(),
                ReviewAction::Reviewer(None),
                cx,
            ),
        ];
        for (index, name) in shown.reviewers.iter().enumerate() {
            rows.push(menu_row(
                ("review-show-reviewer", index),
                name.clone(),
                markup.shows_reviewer(name),
                act(ReviewAction::Reviewer(Some(name.clone()))),
                cx,
            ));
        }
        menu_panel("review-show-menu")
            .children(rows)
            .into_any_element()
    });
    vec![
        group(
            i18n::text("read.group"),
            vec![row(vec![
                text_button(
                    "review-read-aloud",
                    i18n::text("read.button"),
                    i18n::text("read.title"),
                    shown.reading,
                    act(ReviewAction::ReadAloud),
                    cx,
                ),
                text_button(
                    "review-immersive",
                    i18n::text("immersive.button"),
                    i18n::text("immersive.title"),
                    shown.immersive,
                    act(ReviewAction::Immersive),
                    cx,
                ),
            ])],
        ),
        group(
            i18n::text("a11y.group"),
            vec![row(vec![text_button(
                "review-accessibility",
                i18n::text("a11y.button"),
                i18n::text("a11y.title"),
                shown.accessibility,
                act(ReviewAction::Accessibility),
                cx,
            )])],
        ),
        group(
            i18n::text("doc.comment"),
            vec![
                row(vec![
                    text_button(
                        "review-comment",
                        i18n::text("review.new_comment"),
                        i18n::text("review.new_comment"),
                        false,
                        act(ReviewAction::NewComment),
                        cx,
                    ),
                    text_button(
                        "review-comment-delete",
                        i18n::text("review.delete"),
                        i18n::text("review.delete_comment"),
                        false,
                        act(ReviewAction::DeleteComment),
                        cx,
                    ),
                    text_button(
                        "review-comments",
                        i18n::text("review.show_comments"),
                        i18n::text("review.show_comments"),
                        shown.comments,
                        act(ReviewAction::ToggleComments),
                        cx,
                    ),
                ]),
                row(vec![
                    text_button(
                        "review-comment-previous",
                        i18n::text("review.previous"),
                        i18n::text("review.previous_comment"),
                        false,
                        act(ReviewAction::StepComment(false)),
                        cx,
                    ),
                    text_button(
                        "review-comment-next",
                        i18n::text("review.next"),
                        i18n::text("review.next_comment"),
                        false,
                        act(ReviewAction::StepComment(true)),
                        cx,
                    ),
                ]),
            ],
        ),
        group(
            i18n::text("doc.group.tracking"),
            vec![
                row(vec![
                    text_button(
                        "track",
                        i18n::text("doc.track"),
                        i18n::text("doc.track_title"),
                        shown.tracking,
                        act(ReviewAction::ToggleTracking),
                        cx,
                    ),
                    text_button(
                        "review-user",
                        i18n::text("review.user"),
                        i18n::text("review.user.title"),
                        false,
                        act(ReviewAction::UserInfo),
                        cx,
                    ),
                    text_button(
                        "review-pane",
                        i18n::text("review.pane"),
                        i18n::text("review.pane_title"),
                        shown.revisions_pane,
                        act(ReviewAction::RevisionsPane),
                        cx,
                    ),
                ]),
                row(vec![
                    dropdown(
                        list_button(
                            "review-markup",
                            i18n::text(markup.mode.label_key()),
                            i18n::text("review.markup.title"),
                            modes.is_some(),
                            ReviewPopup::Markup,
                            cx,
                        ),
                        modes,
                    ),
                    dropdown(
                        list_button(
                            "review-show",
                            i18n::text("review.show"),
                            i18n::text("review.show.title"),
                            show.is_some(),
                            ReviewPopup::ShowMarkup,
                            cx,
                        ),
                        show,
                    ),
                ]),
            ],
        ),
        group(
            i18n::text("protect.group"),
            vec![row(vec![
                text_button(
                    "review-restrict",
                    i18n::text("restrict.button"),
                    i18n::text("restrict.title"),
                    shown.restrict_pane,
                    act(ReviewAction::RestrictPane),
                    cx,
                ),
                text_button(
                    "review-protect",
                    i18n::text("protect.button"),
                    i18n::text("protect.title"),
                    false,
                    act(ReviewAction::Protect),
                    cx,
                ),
                text_button(
                    "review-final",
                    i18n::text("protect.final"),
                    i18n::text("protect.final_title"),
                    shown.final_,
                    act(ReviewAction::MarkFinal),
                    cx,
                ),
                text_button(
                    "review-signatures",
                    i18n::text("sign.pane"),
                    i18n::text("sign.pane_tooltip"),
                    shown.signatures_pane,
                    act(ReviewAction::SignaturesPane),
                    cx,
                ),
                text_button(
                    "review-signature-line",
                    i18n::text("sign.line.button"),
                    i18n::text("sign.line.tooltip"),
                    false,
                    act(ReviewAction::SignatureLine),
                    cx,
                ),
                text_button(
                    "review-inspect",
                    i18n::text("inspect.button"),
                    i18n::text("inspect.tooltip"),
                    shown.inspect,
                    act(ReviewAction::Inspect),
                    cx,
                ),
            ])],
        ),
        group(
            i18n::text("compare.group"),
            vec![row(vec![
                text_button(
                    "review-compare",
                    i18n::text("compare.button"),
                    i18n::text("compare.title"),
                    false,
                    act(ReviewAction::Compare(false)),
                    cx,
                ),
                text_button(
                    "review-combine",
                    i18n::text("compare.combine"),
                    i18n::text("compare.combine_title"),
                    false,
                    act(ReviewAction::Compare(true)),
                    cx,
                ),
                text_button(
                    "review-side-by-side",
                    i18n::text("side.button"),
                    i18n::text("side.title"),
                    shown.side_by_side,
                    act(ReviewAction::SideBySide),
                    cx,
                ),
                text_button(
                    "review-sync-scroll",
                    i18n::text("side.sync"),
                    i18n::text("side.sync_title"),
                    shown.side_by_side && shown.sync_scroll,
                    act(ReviewAction::SyncScroll),
                    cx,
                ),
                text_button(
                    "review-reset-position",
                    i18n::text("side.reset"),
                    i18n::text("side.reset_title"),
                    false,
                    act(ReviewAction::ResetPosition),
                    cx,
                ),
            ])],
        ),
        group(
            i18n::text("review.group.changes"),
            vec![row(vec![
                decision_list(true, open, cx),
                decision_list(false, open, cx),
                text_button(
                    "revision-previous",
                    i18n::text("review.previous"),
                    i18n::text("review.previous_title"),
                    false,
                    act(ReviewAction::Step(false)),
                    cx,
                ),
                text_button(
                    "revision-next",
                    i18n::text("review.next"),
                    i18n::text("review.next_title"),
                    false,
                    act(ReviewAction::Step(true)),
                    cx,
                ),
            ])],
        ),
    ]
}

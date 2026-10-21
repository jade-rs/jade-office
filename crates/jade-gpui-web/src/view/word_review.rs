//! Comments and tracked changes over a Word document's pages: a comment
//! typed into a bar over them — new, a reply, an edit — and the comments
//! pane beside them, each comment with its replies and what can be done to
//! it; the 校閲 tab's change tracking. What each does is `jade-office-model`'s
//! `WordReview`, the code the native app runs.

use jade_web::word::markup::{MarkupMode, MarkupOptions, WordMarkup};
use jade_web::word::review::{Comment, WordReview};
use jade_web::word::reviewer::Reviewer;

use super::sheet::caret_line;
use super::*;
use crate::word_review::{Decision, ReviewAction};

thread_local! {
    static REVIEWER: RefCell<Option<Reviewer>> = const { RefCell::new(None) };
}

/// Where ユーザー情報 is kept between visits: the browser's local storage.
const STORAGE_KEY: &str = "jade-office.reviewer";

#[cfg(target_arch = "wasm32")]
fn local_storage() -> Option<js_sys::Object> {
    use wasm_bindgen::JsCast;
    js_sys::Reflect::get(&js_sys::global(), &"localStorage".into())
        .ok()?
        .dyn_into()
        .ok()
}

#[cfg(target_arch = "wasm32")]
fn stored_reviewer() -> Option<Reviewer> {
    use wasm_bindgen::JsCast;
    let storage = local_storage()?;
    let get: js_sys::Function = js_sys::Reflect::get(&storage, &"getItem".into())
        .ok()?
        .dyn_into()
        .ok()?;
    let text = get.call1(&storage, &STORAGE_KEY.into()).ok()?.as_string()?;
    Reviewer::from_settings(&text)
}

#[cfg(not(target_arch = "wasm32"))]
fn stored_reviewer() -> Option<Reviewer> {
    None
}

#[cfg(target_arch = "wasm32")]
fn store_reviewer(reviewer: &Reviewer) {
    use wasm_bindgen::JsCast;
    let Some(storage) = local_storage() else {
        return;
    };
    if let Some(set) = js_sys::Reflect::get(&storage, &"setItem".into())
        .ok()
        .and_then(|set| set.dyn_into::<js_sys::Function>().ok())
    {
        set.call2(
            &storage,
            &STORAGE_KEY.into(),
            &reviewer.to_settings().into(),
        )
        .ok();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn store_reviewer(_reviewer: &Reviewer) {}

/// Who comments and tracked changes are put down to: what ユーザー情報
/// saved in this browser, or a name to start with.
pub(super) fn reviewer() -> Reviewer {
    REVIEWER.with(|slot| {
        slot.borrow_mut()
            .get_or_insert_with(|| stored_reviewer().unwrap_or_else(|| Reviewer::named("jade")))
            .clone()
    })
}

/// ユーザー情報's OK.
pub(super) fn set_reviewer(reviewer: Reviewer) {
    store_reviewer(&reviewer);
    REVIEWER.with(|slot| *slot.borrow_mut() = Some(reviewer));
}

/// Now, as the changes a comparison marks are dated.
pub(crate) fn now_iso() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::new_0()
            .to_iso_string()
            .as_string()
            .map(|iso| format!("{}Z", iso.get(..19).unwrap_or_default()))
            .unwrap_or_default()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        "1970-01-01T00:00:00Z".to_string()
    }
}

/// The markup view a document was saved with.
pub(super) fn markup_as_opened(document: &jade_web::editing::EditableDocument) -> MarkupOptions {
    let mut options = MarkupOptions::default();
    if !document.revision_display().0 {
        options.mode = MarkupMode::NoMarkup;
    }
    options
}

/// What a comment bar's text becomes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum CommentEntry {
    New,
    Reply(String),
    Edit(String),
}

/// A comment being typed.
pub(super) struct CommentBar {
    pub entry: CommentEntry,
    pub text: TextEdit,
}

impl DocView {
    pub(super) fn commenting(&self) -> bool {
        matches!(
            &self.document,
            Document::Pages {
                comment: Some(_),
                ..
            }
        )
    }

    fn open_comment_bar(&mut self, entry: CommentEntry, text: String, cx: &mut Context<Self>) {
        let Document::Pages {
            comment,
            story,
            find,
            show_comments,
            dialog,
            ..
        } = &mut self.document
        else {
            return;
        };
        *find = None;
        *story = None;
        *dialog = None;
        *show_comments = true;
        *comment = Some(Box::new(CommentBar {
            entry,
            text: TextEdit::new(text),
        }));
        cx.notify();
    }

    /// The 校閲 tab's buttons.
    pub(super) fn review_action(&mut self, action: ReviewAction, cx: &mut Context<Self>) {
        let Document::Pages {
            show_comments,
            markup,
            ..
        } = &mut self.document
        else {
            return;
        };
        let options = markup.as_mut();
        match action {
            ReviewAction::NewComment => {
                return self.open_comment_bar(CommentEntry::New, String::new(), cx)
            }
            ReviewAction::ToggleComments => {
                *show_comments = !*show_comments;
                options.comments = *show_comments;
                return cx.notify();
            }
            ReviewAction::UserInfo => return self.open_reviewer_dialog(cx),
            ReviewAction::AltText => return self.open_alt_text_dialog(cx),
            ReviewAction::Accessibility => {
                if let Document::Pages { accessibility, .. } = &mut self.document {
                    *accessibility = !*accessibility;
                }
                return cx.notify();
            }
            ReviewAction::Compare(combine) => return self.open_compare_dialog(combine, cx),
            ReviewAction::RestrictPane
            | ReviewAction::RestrictKind(_)
            | ReviewAction::RestrictFormatting
            | ReviewAction::RestrictEveryone
            | ReviewAction::RestrictStartStop
            | ReviewAction::Protect
            | ReviewAction::MarkFinal
            | ReviewAction::EditAnyway => return self.protection_action(action, cx),
            ReviewAction::ReadAloud => return self.toggle_read_aloud(cx),
            ReviewAction::SideBySide => {
                return cx.dispatch_action(&crate::workspace::side_by_side::ViewSideBySide)
            }
            ReviewAction::SyncScroll => {
                return cx.dispatch_action(&crate::workspace::side_by_side::SynchronousScrolling)
            }
            ReviewAction::ResetPosition => {
                return cx.dispatch_action(&crate::workspace::side_by_side::ResetWindowPosition)
            }
            ReviewAction::Immersive => return self.toggle_immersive(cx),
            ReviewAction::Inspect | ReviewAction::InspectRemove(_) => {
                return self.inspect_action(action, cx)
            }
            ReviewAction::RevisionsPane => {
                if let Document::Pages { revisions_pane, .. } = &mut self.document {
                    *revisions_pane = !*revisions_pane;
                }
                return cx.notify();
            }
            ReviewAction::Markup(mode) => options.mode = mode,
            ReviewAction::ShowInsertionsDeletions => {
                options.insertions_deletions = !options.insertions_deletions
            }
            ReviewAction::ShowFormatting => options.formatting = !options.formatting,
            ReviewAction::Reviewer(Some(ref name)) => {
                let shown = options.shows_reviewer(name);
                options.set_reviewer_shown(name, !shown);
            }
            ReviewAction::Reviewer(None) => {
                if options.hidden_reviewers.is_empty() {
                    if let Document::Pages { document, .. } = &self.document {
                        let everyone = document.reviewers();
                        if let Document::Pages { markup, .. } = &mut self.document {
                            markup.hidden_reviewers = everyone;
                        }
                    }
                } else {
                    options.hidden_reviewers.clear();
                }
            }
            ReviewAction::StepComment(forwards) => {
                if let Document::Pages { show_comments, .. } = &mut self.document {
                    *show_comments = true;
                }
                return self.word_move(cx, move |document| {
                    let from = match document.selection() {
                        Some((anchor, focus)) if forwards => anchor.max(focus),
                        Some((anchor, focus)) => anchor.min(focus),
                        None => document.caret().unwrap_or((0, 0)),
                    };
                    match document
                        .step_comment(from, forwards)
                        .and_then(|comment| comment.anchor)
                    {
                        Some((start, end)) => document.select(start, end),
                        None => Err(i18n::text("review.no_comments").to_string()),
                    }
                });
            }
            _ => {}
        }
        let Document::Pages { markup, .. } = &self.document else {
            return;
        };
        let options = (**markup).clone();
        self.word_edit(cx, move |document| {
            let result = match action {
                ReviewAction::ToggleTracking => {
                    let on = !document.is_tracking_changes();
                    document.set_tracking_changes(on, &reviewer())
                }
                ReviewAction::Step(forwards) => {
                    let from = match document.selection() {
                        Some((anchor, focus)) if forwards => anchor.max(focus),
                        Some((anchor, focus)) => anchor.min(focus),
                        None => document.caret().unwrap_or((0, 0)),
                    };
                    match document.step_revision(from, forwards) {
                        Some(revision) => document.select(revision.start(), revision.end()),
                        None => Err(i18n::text("review.no_changes").to_string()),
                    }
                }
                ReviewAction::Decide(accept, Decision::AndNext) => {
                    match document.decide_and_step(accept)? {
                        Some(next) => document.select(next.start(), next.end()),
                        None => Ok(()),
                    }
                }
                ReviewAction::Decide(accept, Decision::This) => {
                    let here = document.revisions_at_selection();
                    document.decide_revisions(&here, accept)
                }
                ReviewAction::Decide(accept, Decision::Visible) => {
                    document.decide_visible_revisions(&options, accept)
                }
                ReviewAction::Decide(accept, Decision::All) => {
                    document.decide_all_revisions(accept)
                }
                ReviewAction::Decide(accept, Decision::AllAndStop) => document
                    .decide_all_revisions(accept)
                    .and_then(|()| document.set_tracking_changes(false, &reviewer())),
                ReviewAction::DeleteComment => {
                    let (start, end) = match document.selection() {
                        Some((anchor, focus)) => (anchor.min(focus), anchor.max(focus)),
                        None => {
                            let caret = document.caret().unwrap_or((0, 0));
                            (caret, caret)
                        }
                    };
                    match document.comments().into_iter().find(|comment| {
                        comment
                            .anchor
                            .is_some_and(|(from, to)| from <= end && start <= to)
                    }) {
                        Some(comment) => document.delete_comment(&comment.id),
                        None => Err(i18n::text("review.no_comment_here").to_string()),
                    }
                }
                _ => Ok(()),
            };
            // The view follows: a new reviewer takes the next colour.
            document.show_markup(&options).map(|_| ())?;
            result
        });
    }

    /// Keys the comment bar takes: Enter puts it in (⇧Enter a line break),
    /// Escape leaves it.
    pub(super) fn comment_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        let Document::Pages { comment, .. } = &mut self.document else {
            return false;
        };
        let Some(bar) = comment.as_deref_mut() else {
            return false;
        };
        if bar.text.marked.is_some() {
            return false;
        }
        match key {
            "escape" => *comment = None,
            "enter" if modifiers.shift || modifiers.alt => bar.text.insert("\n"),
            "enter" => {
                let Some(bar) = comment.take() else {
                    return false;
                };
                let text = bar.text.text.trim().to_string();
                if text.is_empty() {
                    return {
                        cx.notify();
                        true
                    };
                }
                self.word_edit(cx, move |document| match &bar.entry {
                    CommentEntry::New => document.add_comment(&text, &reviewer()).map(|_| ()),
                    CommentEntry::Reply(id) => document.reply_to_comment(id, &text, &reviewer()),
                    CommentEntry::Edit(id) => document.edit_comment(id, &text),
                });
                return true;
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

    pub(super) fn comment_bar_element(&self) -> Option<gpui::AnyElement> {
        let Document::Pages {
            comment: Some(bar), ..
        } = &self.document
        else {
            return None;
        };
        let label = i18n::text(match bar.entry {
            CommentEntry::New => "review.new_comment",
            CommentEntry::Reply(_) => "review.reply",
            CommentEntry::Edit(_) => "review.edit",
        });
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
                .child(div().text_color(palette::faint()).child(label))
                .child(
                    div()
                        .min_w(px(360.0))
                        .min_h(px(24.0))
                        .px_2()
                        .py_0p5()
                        .flex()
                        .items_center()
                        .rounded_md()
                        .border_1()
                        .border_color(palette::accent())
                        .bg(palette::canvas())
                        .child(caret_line(&bar.text, palette::accent().into(), false)),
                )
                .child(
                    div()
                        .text_color(palette::dim())
                        .child(i18n::text("review.comment_keys")),
                )
                .into_any_element(),
        )
    }

    /// The comments pane beside the pages, while it is open.
    pub(super) fn comments_pane(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let Document::Pages {
            document,
            show_comments: true,
            ..
        } = &self.document
        else {
            return None;
        };
        let comments = document.comments();
        let empty = comments.is_empty();
        let cards: Vec<_> = comments
            .into_iter()
            .enumerate()
            .map(|(index, comment)| comment_card(index, comment, cx))
            .collect();
        Some(
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
                .bg(palette::panel())
                .border_l_1()
                .border_color(palette::border())
                .text_xs()
                .child(
                    div()
                        .text_sm()
                        .text_color(palette::text())
                        .child(i18n::text("review.comments")),
                )
                .when(empty, |pane| {
                    pane.child(
                        div()
                            .text_color(palette::faint())
                            .child(i18n::text("review.no_comments")),
                    )
                })
                .children(cards)
                .into_any_element(),
        )
    }
}

fn comment_line(comment: &Comment) -> gpui::Div {
    let day = comment
        .date
        .as_deref()
        .and_then(|date| date.get(..10))
        .map(|day| format!(" · {day}"))
        .unwrap_or_default();
    div()
        .flex()
        .flex_col()
        .gap_0p5()
        .child(div().text_color(palette::dim()).child(format!(
            "{}{day}",
            comment.author.clone().unwrap_or_default()
        )))
        .child(
            div()
                .text_sm()
                .text_color(if comment.resolved {
                    palette::faint()
                } else {
                    palette::text()
                })
                .child(comment.text.clone()),
        )
}

fn comment_action(id: (&'static str, usize), key: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px_1p5()
        .rounded_md()
        .text_color(palette::accent())
        .cursor_pointer()
        .hover(|button| button.bg(palette::hovered()))
        .child(i18n::text(key))
}

fn comment_card(index: usize, comment: Comment, cx: &mut Context<DocView>) -> gpui::AnyElement {
    let id = comment.id.clone();
    let resolved = comment.resolved;
    let anchor = comment.anchor;
    let (reply_id, edit_id, edit_text, resolve_id, delete_id) =
        (id.clone(), id.clone(), comment.text.clone(), id.clone(), id);
    div()
        .id(("comment", index))
        .flex()
        .flex_col()
        .gap_1()
        .p_2()
        .rounded_md()
        .border_1()
        .border_color(palette::border())
        .bg(palette::canvas())
        .cursor_pointer()
        .on_click(cx.listener(move |view, _event, _window, cx| {
            if let Some((start, end)) = anchor {
                view.word_move(cx, |document| document.select(start, end));
                view.reveal_caret();
            }
        }))
        .child(comment_line(&comment))
        .children(comment.replies.iter().map(|reply| {
            div()
                .pl_3()
                .border_l_2()
                .border_color(palette::border())
                .child(comment_line(reply))
        }))
        .child(
            div()
                .flex()
                .gap_1()
                .child(
                    comment_action(("comment-reply", index), "review.reply").on_click(cx.listener(
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
                    comment_action(("comment-edit", index), "review.edit").on_click(cx.listener(
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
                    comment_action(
                        ("comment-resolve", index),
                        if resolved {
                            "review.reopen"
                        } else {
                            "review.resolve"
                        },
                    )
                    .on_click(cx.listener(move |view, _event, _window, cx| {
                        let id = resolve_id.clone();
                        view.word_edit(cx, move |document| document.resolve_comment(&id, !resolved))
                    })),
                )
                .child(
                    comment_action(("comment-delete", index), "review.delete").on_click(
                        cx.listener(move |view, _event, _window, cx| {
                            let id = delete_id.clone();
                            view.word_edit(cx, move |document| document.delete_comment(&id))
                        }),
                    ),
                ),
        )
        .into_any_element()
}

impl DocView {
    /// The アクセシビリティ pane beside the pages, while it is open: Word's
    /// エラー, 警告 and ヒント, each a click away in the document, with its
    /// おすすめのアクション.
    pub(super) fn accessibility_pane(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        use jade_web::word::accessibility::{IssueKind, Severity, WordAccessibility};
        let Document::Pages {
            document,
            accessibility: true,
            ..
        } = &self.document
        else {
            return None;
        };
        let issues = document.accessibility_issues();
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        if issues.is_empty() {
            rows.push(
                div()
                    .text_color(palette::dim())
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
                        .text_color(palette::dim())
                        .child(i18n::text(match issue.severity() {
                            Severity::Error => "a11y.errors",
                            Severity::Warning => "a11y.warnings",
                            Severity::Tip => "a11y.tips",
                        }))
                        .into_any_element(),
                );
            }
            let (start, end) = (issue.start(), issue.end());
            let fix = issue.clone();
            let describe = match issue.kind {
                IssueKind::MissingAltText(target) => Some(target),
                _ => None,
            };
            let fix_key = match issue.kind {
                IssueKind::MissingAltText(_) => "a11y.fix.decorative",
                IssueKind::NoHeaderRow { .. } => "a11y.fix.header",
                IssueKind::NotInline(_) => "a11y.fix.inline",
                _ => "a11y.fix",
            };
            rows.push(
                div()
                    .id(("a11y-issue", index))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_2()
                    .rounded_md()
                    .border_1()
                    .border_color(palette::border())
                    .bg(palette::canvas())
                    .cursor_pointer()
                    .on_click(cx.listener(move |view, _event, _window, cx| {
                        view.word_move(cx, |document| document.select(start, end));
                        view.reveal_caret();
                    }))
                    .child(
                        div()
                            .text_color(palette::text())
                            .child(i18n::text(issue.kind.label_key())),
                    )
                    .when(!issue.excerpt.is_empty(), |card| {
                        card.child(
                            div()
                                .text_color(palette::dim())
                                .child(issue.excerpt.clone()),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .children(describe.map(|target| {
                                comment_action(("a11y-describe", index), "a11y.fix.describe")
                                    .on_click(cx.listener(move |view, _event, _window, cx| {
                                        cx.stop_propagation();
                                        view.open_alt_text_for(target, cx)
                                    }))
                            }))
                            .when(issue.kind.has_quick_fix(), |row| {
                                row.child(comment_action(("a11y-fix", index), fix_key).on_click(
                                    cx.listener(move |view, _event, _window, cx| {
                                        cx.stop_propagation();
                                        let issue = fix.clone();
                                        view.word_edit(cx, move |document| {
                                            document.quick_fix(&issue)
                                        })
                                    }),
                                ))
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
                        .child(i18n::text("a11y.title")),
                )
                .children(rows)
                .into_any_element(),
        )
    }
}

impl DocView {
    /// The 変更履歴ウィンドウ beside the pages, while it is open: every
    /// change, what it is — a formatting change with what it changed — who
    /// made it, and 承諾 / 元に戻す for it.
    pub(super) fn revisions_pane(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        use jade_web::word::markup::{describe_formatting, kind_key};
        let Document::Pages {
            document,
            revisions_pane: true,
            ..
        } = &self.document
        else {
            return None;
        };
        let revisions = document.revisions();
        let count = revisions.len();
        let cards: Vec<gpui::AnyElement> = revisions
            .into_iter()
            .enumerate()
            .map(|(index, revision)| {
                let changed: Vec<&str> = describe_formatting(document.as_ref(), &revision)
                    .into_iter()
                    .map(i18n::text)
                    .collect();
                let heading = if changed.is_empty() {
                    i18n::text(kind_key(&revision)).to_string()
                } else {
                    format!(
                        "{}: {}",
                        i18n::text(kind_key(&revision)),
                        changed.join(", ")
                    )
                };
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
                let (start, end) = (revision.start(), revision.end());
                let (accept, reject) = (revision.clone(), revision);
                div()
                    .id(("revision-card", index))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_2()
                    .rounded_md()
                    .border_1()
                    .border_color(palette::border())
                    .bg(palette::canvas())
                    .cursor_pointer()
                    .on_click(cx.listener(move |view, _event, _window, cx| {
                        view.word_move(cx, |document| document.select(start, end));
                        view.reveal_caret();
                    }))
                    .child(div().text_color(palette::text()).child(heading))
                    .child(div().text_color(palette::dim()).child(who))
                    .when(!text.trim().is_empty(), |card| {
                        card.child(div().text_color(palette::faint()).child(text))
                    })
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .child(
                                comment_action(("revision-accept", index), "review.accept")
                                    .on_click(cx.listener(move |view, _event, _window, cx| {
                                        cx.stop_propagation();
                                        let revision = accept.clone();
                                        view.word_edit(cx, move |document| {
                                            document.decide_revisions(&[revision], true)
                                        })
                                    })),
                            )
                            .child(
                                comment_action(("revision-reject", index), "review.reject")
                                    .on_click(cx.listener(move |view, _event, _window, cx| {
                                        cx.stop_propagation();
                                        let revision = reject.clone();
                                        view.word_edit(cx, move |document| {
                                            document.decide_revisions(&[revision], false)
                                        })
                                    })),
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
                        .child(i18n::text("review.pane_title")),
                )
                .child(
                    div()
                        .text_color(palette::dim())
                        .child(i18n::format("review.pane.count", &[("n", &count)])),
                )
                .children(cards)
                .into_any_element(),
        )
    }
}

/// Why the document is read-only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReadOnlyReason {
    Recommended,
    Final,
}

/// The browser shell's share of 保護: the 編集の制限 pane's choices, the
/// password to open the file is saved with, 最終版, and why the document
/// is read-only.
#[derive(Default)]
pub(crate) struct WebProtection {
    pub pane: bool,
    pub kind: Option<jade_web::word::protection::EditingKind>,
    pub formatting: bool,
    pub open_password: Option<String>,
    pub mark_final: Option<bool>,
    pub read_only: Option<ReadOnlyReason>,
    /// Whether the ドキュメント検査 pane is open (`word_inspect`).
    pub inspect: bool,
    /// What ドキュメント検査 left out of every later save.
    pub cleaning: jade_web::word::inspect::PackageCleaning,
}

impl WebProtection {
    /// For a document just opened: read-only when final or recommended.
    pub(super) fn opened(document: &mut jade_web::editing::EditableDocument) -> Self {
        use jade_web::editing::WordSession;
        use jade_web::word::protection::{WordProtection, WordReadOnly};
        let final_ = document
            .word_document()
            .document_properties()
            .get("_MarkAsFinal")
            .is_some_and(|value| matches!(value.as_str(), "true" | "1"));
        let recommended = document
            .write_protection()
            .is_some_and(|protection| protection.recommended);
        let read_only = if final_ {
            Some(ReadOnlyReason::Final)
        } else if recommended {
            Some(ReadOnlyReason::Recommended)
        } else {
            None
        };
        if read_only.is_some() {
            document.set_opened_read_only(true);
        }
        let restriction = document.editing_restriction();
        Self {
            kind: restriction
                .as_ref()
                .and_then(|restriction| restriction.kind),
            formatting: restriction.is_some_and(|restriction| restriction.formatting),
            read_only,
            ..Self::default()
        }
    }
}

/// The package a save hands the page: without what ドキュメント検査 removed,
/// marked final or not, encrypted with the password to open.
pub(super) fn finish_save(protection: &WebProtection, bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    use jade_web::word::protection::{encrypt_package, mark_final};
    let bytes = jade_web::word::inspect::clean_package(&bytes, protection.cleaning)?;
    let bytes = match protection.mark_final {
        Some(final_) => mark_final(&bytes, final_)?,
        None => bytes,
    };
    match protection.open_password.as_deref() {
        Some(password) if !password.is_empty() => {
            encrypt_package(&bytes, password, &mut random).map_err(|error| error.to_string())
        }
        _ => Ok(bytes),
    }
}

/// Random bytes: the browser's crypto, else the model's fallback.
pub(crate) fn random(buffer: &mut [u8]) {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        let crypto = js_sys::Reflect::get(&js_sys::global(), &"crypto".into()).ok();
        let fill = crypto.as_ref().and_then(|crypto| {
            js_sys::Reflect::get(crypto, &"getRandomValues".into())
                .ok()?
                .dyn_into::<js_sys::Function>()
                .ok()
        });
        if let (Some(crypto), Some(fill)) = (crypto, fill) {
            let array = js_sys::Uint8Array::new_with_length(buffer.len() as u32);
            if fill.call1(&crypto, &array).is_ok() {
                array.copy_to(buffer);
                return;
            }
        }
    }
    jade_web::word::protection::random_bytes(buffer);
}

fn salt() -> [u8; 16] {
    let mut salt = [0u8; 16];
    random(&mut salt);
    salt
}

impl DocView {
    /// Escape from the ribbon: the keyboard back on the page.
    pub(crate) fn focus_document(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Document::Pages { ribbon, .. } = &mut self.document {
            ribbon.popup = None;
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// The pages' scroll, for 並べて比較's 同時にスクロール.
    pub(crate) fn page_scroll(&self) -> Option<ScrollHandle> {
        match &self.document {
            Document::Pages { scroll, .. } => Some(scroll.clone()),
            _ => None,
        }
    }

    /// The password the file was opened with: it is saved with it again.
    pub(crate) fn set_open_password(&mut self, password: String) {
        if let Document::Pages { protection, .. } = &mut self.document {
            protection.open_password = Some(password);
        }
    }

    /// The 保護 group's controls.
    pub(super) fn protection_action(&mut self, action: ReviewAction, cx: &mut Context<Self>) {
        use jade_web::word::protection::{WordProtection, WordReadOnly};
        let Document::Pages {
            document,
            protection,
            ..
        } = &mut self.document
        else {
            return;
        };
        match action {
            ReviewAction::RestrictPane => protection.pane = !protection.pane,
            ReviewAction::RestrictKind(kind) => protection.kind = kind,
            ReviewAction::RestrictFormatting => protection.formatting = !protection.formatting,
            ReviewAction::RestrictEveryone => {
                return self.word_edit(cx, |document| document.allow_everyone_here());
            }
            ReviewAction::RestrictStartStop => {
                let stopping = document.is_restricted();
                return self.open_dialog_restrict(stopping, cx);
            }
            ReviewAction::Protect => return self.open_dialog_protect(cx),
            ReviewAction::MarkFinal => {
                let final_ = protection.read_only != Some(ReadOnlyReason::Final);
                protection.mark_final = Some(final_);
                protection.read_only = final_.then_some(ReadOnlyReason::Final);
                document.set_opened_read_only(final_);
            }
            ReviewAction::EditAnyway => {
                if protection.read_only.take() == Some(ReadOnlyReason::Final) {
                    protection.mark_final = Some(false);
                }
                document.set_opened_read_only(false);
            }
            _ => {}
        }
        cx.notify();
    }

    /// 保護の開始 (or 中止) with the password typed into its bar.
    pub(super) fn apply_restriction(
        &mut self,
        stopping: bool,
        password: String,
        cx: &mut Context<Self>,
    ) {
        use jade_web::word::protection::{EditingKind, WordProtection};
        let Document::Pages { protection, .. } = &self.document else {
            return;
        };
        let (kind, formatting) = (protection.kind, protection.formatting);
        let salt = salt();
        self.word_edit(cx, move |document| {
            if stopping {
                document.stop_restriction(&password)
            } else {
                let result = document.start_restriction(
                    kind,
                    formatting,
                    (!password.is_empty()).then_some(password.as_str()),
                    salt,
                );
                if result.is_ok() && kind == Some(EditingKind::TrackedChanges) {
                    document.set_tracking_changes(true, &reviewer())?;
                }
                result
            }
        });
    }

    /// 文書の保護's OK.
    pub(super) fn apply_protect(
        &mut self,
        open: String,
        modify: String,
        recommended: bool,
        cx: &mut Context<Self>,
    ) {
        use jade_web::word::protection::WordProtection;
        if let Document::Pages { protection, .. } = &mut self.document {
            protection.open_password = (!open.is_empty()).then_some(open);
        }
        let salt = salt();
        self.word_edit(cx, move |document| {
            document.set_write_protection(
                recommended,
                (!modify.is_empty()).then_some(modify.as_str()),
                salt,
            )
        });
    }

    /// The bar a read-only document opens under.
    pub(super) fn read_only_bar(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let Document::Pages { protection, .. } = &self.document else {
            return None;
        };
        let key = match protection.read_only? {
            ReadOnlyReason::Final => "protect.final_bar",
            ReadOnlyReason::Recommended => "protect.recommended_bar",
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
                .child(i18n::text(key))
                .child(
                    comment_action(("protect-edit-anyway", 0), "protect.edit_anyway").on_click(
                        cx.listener(|view, _event, _window, cx| {
                            view.protection_action(ReviewAction::EditAnyway, cx)
                        }),
                    ),
                )
                .into_any_element(),
        )
    }

    /// The 編集の制限 pane beside the pages, while it is open.
    pub(super) fn restrict_pane(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        use jade_web::word::protection::{EditingKind, WordProtection};
        let Document::Pages {
            document,
            protection,
            ..
        } = &self.document
        else {
            return None;
        };
        if !protection.pane {
            return None;
        }
        let enforced = document.is_restricted();
        let choice = |id: (&'static str, usize),
                      key: &'static str,
                      on: bool,
                      action: ReviewAction,
                      cx: &mut Context<Self>| {
            div()
                .id(id)
                .cursor_pointer()
                .text_color(if on {
                    palette::accent()
                } else {
                    palette::text()
                })
                .child(format!(
                    "{} {}",
                    if on { "☑" } else { "☐" },
                    i18n::text(key)
                ))
                .on_click(cx.listener(move |view, _event, _window, cx| {
                    view.protection_action(action.clone(), cx)
                }))
                .into_any_element()
        };
        let mut rows = vec![
            div()
                .text_color(palette::dim())
                .child(i18n::text("restrict.formatting_title"))
                .into_any_element(),
            choice(
                ("restrict-formatting", 0),
                "restrict.formatting",
                protection.formatting,
                ReviewAction::RestrictFormatting,
                cx,
            ),
            div()
                .text_color(palette::dim())
                .child(i18n::text("restrict.editing_title"))
                .into_any_element(),
        ];
        for (index, (key, kind)) in [
            ("restrict.none", None),
            ("restrict.read_only", Some(EditingKind::ReadOnly)),
            ("restrict.tracked", Some(EditingKind::TrackedChanges)),
            ("restrict.comments", Some(EditingKind::Comments)),
            ("restrict.forms", Some(EditingKind::Forms)),
        ]
        .into_iter()
        .enumerate()
        {
            rows.push(choice(
                ("restrict-kind", index),
                key,
                protection.kind == kind,
                ReviewAction::RestrictKind(kind),
                cx,
            ));
        }
        rows.push(
            div()
                .text_color(palette::dim())
                .child(i18n::text("restrict.exceptions"))
                .into_any_element(),
        );
        rows.push(
            comment_action(("restrict-everyone", 0), "restrict.everyone")
                .on_click(cx.listener(|view, _event, _window, cx| {
                    view.protection_action(ReviewAction::RestrictEveryone, cx)
                }))
                .into_any_element(),
        );
        rows.push(
            div()
                .text_color(palette::faint())
                .child(i18n::format(
                    "restrict.exception_count",
                    &[("n", &document.editable_ranges().len())],
                ))
                .into_any_element(),
        );
        rows.push(
            comment_action(
                ("restrict-start", 0),
                if enforced {
                    "restrict.stop"
                } else {
                    "restrict.start"
                },
            )
            .on_click(cx.listener(|view, _event, _window, cx| {
                view.protection_action(ReviewAction::RestrictStartStop, cx)
            }))
            .into_any_element(),
        );
        Some(
            div()
                .id("restrict-pane")
                .flex()
                .flex_col()
                .flex_none()
                .w(px(260.0))
                .h_full()
                .gap_1()
                .px_3()
                .pt_3()
                .bg(palette::panel())
                .border_l_1()
                .border_color(palette::border())
                .text_xs()
                .child(div().text_sm().child(i18n::text("restrict.title")))
                .children(rows)
                .into_any_element(),
        )
    }
}

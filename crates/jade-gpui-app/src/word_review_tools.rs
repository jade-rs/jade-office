//! The 校閲 tab beyond writing comments and walking changes one by one:
//! who the review work is recorded under (ユーザー情報), 変更内容の表示 and
//! 変更履歴とコメントの表示 — the markup views, which kinds of change and
//! whose are shown, each reviewer in a colour of their own — and the 承諾 /
//! 元に戻す lists with their この変更 / 表示された / すべての entries;
//! コメント's 前へ, 次へ and 削除. What each does is `jade-office-model`'s
//! (`word::markup`, `word::reviewer`, `word::review`); the browser shell has
//! the same (`jade-gpui-web/src/view/word_review_tools.rs`).

use std::cell::RefCell;

use gpui::{
    anchored, deferred, div, point, prelude::*, px, AnyElement, ClickEvent, Context, MouseButton,
    Window,
};
use jade_office_model::text::TextEdit;
use jade_office_model::word::accessibility::{AltTextForm, WordAccessibility};
use jade_office_model::word::markup::{MarkupMode, MarkupOptions, WordMarkup};
use jade_office_model::word::review::WordReview;
use jade_office_model::word::reviewer::{Reviewer, ReviewerField, ReviewerForm};

use crate::ribbon::{dropdown, listener, menu_caption, menu_panel, menu_row, Action, Popup};
use crate::tooltip::Tooltip;
use crate::word_characters::button;
use crate::{i18n, theme, DocumentView};

// ---- who is reviewing -----------------------------------------------------

thread_local! {
    /// The windows are all on the main thread; each test has its own.
    static REVIEWER: RefCell<Option<Reviewer>> = const { RefCell::new(None) };
}

fn settings_file() -> Option<std::path::PathBuf> {
    // Tests set a reviewer of their own and must not rewrite the user's.
    if cfg!(test) {
        return None;
    }
    let home = std::env::var_os("HOME")?;
    Some(
        std::path::PathBuf::from(home)
            .join("Library/Application Support/jade-office")
            .join("user.txt"),
    )
}

/// The account's full name, as Word for Mac first fills in ユーザー情報.
fn system_name() -> String {
    std::process::Command::new("id")
        .arg("-F")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .or_else(|| std::env::var("USER").ok())
        .unwrap_or_else(|| "jade".to_string())
}

/// Who comments and tracked changes are put down to: what ユーザー情報
/// saved, or the account's name.
pub fn reviewer() -> Reviewer {
    REVIEWER.with(|slot| {
        slot.borrow_mut()
            .get_or_insert_with(|| {
                settings_file()
                    .and_then(|file| std::fs::read_to_string(file).ok())
                    .and_then(|text| Reviewer::from_settings(&text))
                    .unwrap_or_else(|| Reviewer::named(&system_name()))
            })
            .clone()
    })
}

/// ユーザー情報's OK: the reviewer from now on, saved for next time.
pub fn set_reviewer(reviewer: Reviewer) {
    if let Some(file) = settings_file() {
        if let Some(directory) = file.parent() {
            std::fs::create_dir_all(directory).ok();
        }
        std::fs::write(file, reviewer.to_settings()).ok();
    }
    REVIEWER.with(|slot| *slot.borrow_mut() = Some(reviewer));
}

// ---- the tab's lists and what they do ---------------------------------------

/// The lists the 校閲 tab drops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewPopup {
    /// 変更内容の表示.
    Markup,
    /// 変更履歴とコメントの表示.
    ShowMarkup,
    /// 承諾's list.
    Accept,
    /// 元に戻す's list.
    Reject,
}

/// Which changes a 承諾 or 元に戻す entry decides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// 承諾して次へ進む: the change at the selection, then the next one.
    AndNext,
    /// この変更を反映: the change at the selection only.
    This,
    /// 表示されたすべての変更を反映.
    Visible,
    /// すべての変更を反映.
    All,
    /// すべての変更を反映し、変更の記録を停止.
    AllAndStop,
}

/// What the 校閲 tab's lists and buttons do.
#[derive(Clone, Debug, PartialEq)]
pub enum ReviewAction {
    Markup(MarkupMode),
    ShowComments,
    ShowInsertionsDeletions,
    ShowFormatting,
    /// A reviewer ticked or unticked; `None` is すべての校閲者.
    Reviewer(Option<String>),
    /// 承諾 (`true`) or 元に戻す.
    Decide(bool, Decision),
    /// コメント › 前へ (`false`) / 次へ.
    StepComment(bool),
    /// コメント › 削除: the comment at the selection.
    DeleteComment,
    /// ユーザー情報….
    UserInfo,
    /// スペル チェックと文章校正: the pane.
    Proofing,
    /// 入力時にスペル チェック: the marks on or off.
    ProofingMarks,
    /// アクセシビリティ チェック: the pane.
    Accessibility,
    /// 代替テキスト… for the picture or shape last clicked.
    AltText,
    /// 変更履歴ウィンドウ.
    RevisionsPane,
    /// 比較… (`false`) or 組み込み….
    Compare(bool),
}

/// The dialog the tab opens.
pub enum ReviewDialog {
    User(ReviewerForm),
    /// 代替テキスト for a picture or shape.
    AltText(AltTextForm),
    /// 文書の比較 / 組み込み (`word_compare`).
    Compare(Box<crate::word_compare::CompareDialog>),
}

impl ReviewDialog {
    fn field(&self) -> Option<&TextEdit> {
        match self {
            ReviewDialog::User(form) => Some(form.focused()),
            ReviewDialog::AltText(form) => Some(&form.description),
            ReviewDialog::Compare(dialog) => Some(&dialog.author),
        }
    }

    fn field_mut(&mut self) -> Option<&mut TextEdit> {
        match self {
            ReviewDialog::User(form) => Some(form.focused_mut()),
            ReviewDialog::AltText(form) => Some(&mut form.description),
            ReviewDialog::Compare(dialog) => Some(&mut dialog.author),
        }
    }
}

/// The view's share of the 校閲 tab.
#[derive(Default)]
pub struct ReviewTools {
    pub markup: MarkupOptions,
    pub dialog: Option<ReviewDialog>,
    /// スペル チェックと文章校正 (`word_proofing`).
    pub proofing: crate::word_proofing::Proofing,
    /// The アクセシビリティ pane, while it is open (`word_accessibility`).
    pub accessibility: Option<crate::word_accessibility::A11yPane>,
    /// Whether the 変更履歴ウィンドウ is open (`word_revisions`).
    pub revisions_pane: bool,
}

impl ReviewTools {
    /// For a document just opened: the markup view it was saved with.
    pub fn opened(document: &jade_docx_native_render::NativeDocument) -> Self {
        let mut tools = Self::default();
        if !document.revision_display().0 {
            tools.markup.mode = MarkupMode::NoMarkup;
        }
        tools
    }

    pub fn is_open(&self) -> bool {
        self.dialog.is_some()
    }

    /// The field typing goes into while a dialog is open.
    pub fn text_field(&self) -> Option<&TextEdit> {
        self.dialog.as_ref()?.field()
    }

    pub fn text_field_mut(&mut self) -> Option<&mut TextEdit> {
        self.dialog.as_mut()?.field_mut()
    }

    /// After typing reached the open dialog's field.
    pub fn typed(&mut self) {
        if let Some(ReviewDialog::User(form)) = self.dialog.as_mut() {
            if form.focus == ReviewerField::Name {
                form.name_changed();
            }
        }
    }
}

fn review(action: ReviewAction) -> Action {
    Action::Review(action)
}

/// Shows the document's changes as the tab says, after an edit or a
/// change of view: a new reviewer takes the next colour.
pub fn refresh_markup(view: &mut DocumentView) {
    let options = view.review.markup.clone();
    match view.document.show_markup(&options) {
        Ok(true) => view.rebuild_all_images(),
        Ok(false) => {}
        Err(error) => view.table_status = Some(error.into()),
    }
}

/// Runs a 校閲 tab action.
pub fn run(view: &mut DocumentView, action: ReviewAction, cx: &mut Context<DocumentView>) {
    view.ribbon.popup = None;
    let markup = &mut view.review.markup;
    match action {
        ReviewAction::Markup(mode) => markup.mode = mode,
        ReviewAction::ShowComments => {
            markup.comments = !markup.comments;
            view.show_comments = markup.comments;
        }
        ReviewAction::ShowInsertionsDeletions => {
            markup.insertions_deletions = !markup.insertions_deletions
        }
        ReviewAction::ShowFormatting => markup.formatting = !markup.formatting,
        ReviewAction::Reviewer(None) => {
            let everyone = markup.hidden_reviewers.is_empty();
            markup.hidden_reviewers = if everyone {
                view.document.reviewers()
            } else {
                Vec::new()
            };
        }
        ReviewAction::Reviewer(Some(name)) => {
            let shown = markup.shows_reviewer(&name);
            markup.set_reviewer_shown(&name, !shown);
        }
        ReviewAction::Decide(accept, decision) => return decide(view, accept, decision, cx),
        ReviewAction::StepComment(forwards) => return step_comment(view, forwards, cx),
        ReviewAction::DeleteComment => return delete_comment_here(view, cx),
        ReviewAction::UserInfo => {
            view.review.dialog = Some(ReviewDialog::User(ReviewerForm::new(&reviewer())));
            return cx.notify();
        }
        ReviewAction::Proofing => return crate::word_proofing::open_pane(view, cx),
        ReviewAction::Accessibility => return crate::word_accessibility::toggle(view, cx),
        ReviewAction::AltText => return open_alt_text(view, cx),
        ReviewAction::RevisionsPane => return crate::word_revisions::toggle(view, cx),
        ReviewAction::Compare(combine) => return crate::word_compare::open(view, combine, cx),
        ReviewAction::ProofingMarks => {
            let on = !view.document.proofing();
            return crate::word_proofing::set_marks(view, on, cx);
        }
    }
    refresh_markup(view);
    cx.notify();
}

fn decide(
    view: &mut DocumentView,
    accept: bool,
    decision: Decision,
    cx: &mut Context<DocumentView>,
) {
    if decision == Decision::AndNext {
        return view.decide_revision(accept, cx);
    }
    if view.document.selection().is_none() {
        view.document
            .place_caret(view.insert_at.0, view.insert_at.1);
    }
    let options = view.review.markup.clone();
    let result = match decision {
        Decision::AndNext => unreachable!(),
        Decision::This => {
            let here = view.document.revisions_at_selection();
            view.document.decide_revisions(&here, accept)
        }
        Decision::Visible => view.document.decide_visible_revisions(&options, accept),
        Decision::All => view.document.decide_all_revisions(accept),
        Decision::AllAndStop => view
            .document
            .decide_all_revisions(accept)
            .and_then(|()| view.document.set_tracking_changes(false, &reviewer())),
    };
    view.reviewed(result, cx);
}

/// コメント › 前へ / 次へ: the comment before or after the caret, its text
/// selected and the pane open on it.
fn step_comment(view: &mut DocumentView, forwards: bool, cx: &mut Context<DocumentView>) {
    let from = match view.document.selection() {
        Some((anchor, focus)) if forwards => anchor.max(focus),
        Some((anchor, focus)) => anchor.min(focus),
        None => view.insert_at,
    };
    match view.document.step_comment(from, forwards) {
        Some(comment) => {
            view.show_comments = true;
            view.go_to_comment(&comment, cx);
        }
        None => {
            view.table_status = Some(i18n::text("review.no_comments").into());
            cx.notify();
        }
    }
}

/// コメント › 削除: the comment the caret or the selection is in.
fn delete_comment_here(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let (start, end) = match view.document.selection() {
        Some((anchor, focus)) => (anchor.min(focus), anchor.max(focus)),
        None => (view.insert_at, view.insert_at),
    };
    let here = view.document.comments().into_iter().find(|comment| {
        comment
            .anchor
            .is_some_and(|(from, to)| from <= end && start <= to)
    });
    match here {
        Some(comment) => view.delete_comment(&comment.id, cx),
        None => {
            view.table_status = Some(i18n::text("review.no_comment_here").into());
            cx.notify();
        }
    }
}

// ---- the tab's groups ---------------------------------------------------------

/// A labelled button that opens one of the tab's lists.
fn list_button(
    id: &'static str,
    label: &'static str,
    tooltip: &'static str,
    open: bool,
    popup: ReviewPopup,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap_0p5()
        .h(px(28.0))
        .px_2()
        .rounded_md()
        .text_sm()
        .whitespace_nowrap()
        .text_color(theme::text())
        .cursor_pointer()
        .when(open, |button| button.bg(theme::selected()))
        .hover(|button| button.bg(theme::hovered()))
        .tooltip(Tooltip::text(tooltip))
        .on_click(listener(Action::Popup(Popup::Review(popup)), cx))
        .child(label)
        .child(
            gpui::svg()
                .path("icons/chevron-down.svg")
                .size_3()
                .text_color(theme::text_dim()),
        )
        .into_any_element()
}

fn entry(
    id: impl Into<gpui::ElementId>,
    key: &'static str,
    checked: bool,
    action: ReviewAction,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    menu_row(id, i18n::text(key), checked, review(action), cx)
}

/// 変更内容の表示 and 変更履歴とコメントの表示, as the 変更履歴 group's
/// second row shows them.
pub fn markup_row(view: &DocumentView, cx: &mut Context<DocumentView>) -> Vec<AnyElement> {
    let open = view.ribbon.popup;
    let markup = &view.review.markup;
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
        let reviewers = view.document.reviewers();
        let mut rows = vec![
            entry(
                "review-show-comments",
                "review.show.comments",
                markup.comments,
                ReviewAction::ShowComments,
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
        for (index, name) in reviewers.into_iter().enumerate() {
            let shown = markup.shows_reviewer(&name);
            rows.push(menu_row(
                ("review-show-reviewer", index),
                name.clone(),
                shown,
                review(ReviewAction::Reviewer(Some(name))),
                cx,
            ));
        }
        menu_panel("review-show-menu")
            .children(rows)
            .into_any_element()
    });
    vec![
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
    ]
}

/// 承諾 and 元に戻す, each with its list.
pub fn decision_buttons(view: &DocumentView, cx: &mut Context<DocumentView>) -> Vec<AnyElement> {
    let open = view.ribbon.popup;
    let list = |accept: bool, cx: &mut Context<DocumentView>| {
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
                if accept {
                    "review-accept-list"
                } else {
                    "review-reject-list"
                },
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
    };
    vec![list(true, cx), list(false, cx)]
}

/// A plain text button running a tab action.
pub fn text_button(
    id: &'static str,
    key: &'static str,
    tooltip: &'static str,
    active: bool,
    action: ReviewAction,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .min_w(px(30.0))
        .h(px(28.0))
        .px_2()
        .whitespace_nowrap()
        .rounded_md()
        .when(active, |button| button.bg(theme::selected()))
        .hover(|button| button.bg(theme::hovered()))
        .cursor_pointer()
        .text_sm()
        .text_color(theme::text())
        .child(i18n::text(key))
        .tooltip(Tooltip::text(i18n::text(tooltip)))
        .on_click(listener(review(action), cx))
        .into_any_element()
}

// ---- the dialog ----------------------------------------------------------------

/// A key while the tab's dialog is open; `true` when it was taken.
pub fn handle_key(view: &mut DocumentView, key: &str, cx: &mut Context<DocumentView>) -> bool {
    let Some(dialog) = view.review.dialog.as_mut() else {
        return false;
    };
    if dialog.field().is_some_and(|field| field.marked.is_some()) {
        return false;
    }
    match (key, &mut *dialog) {
        ("escape", _) => view.review.dialog = None,
        ("enter" | "return", _) => confirm(view, cx),
        ("tab", ReviewDialog::User(form)) => form.next_field(),
        ("tab", _) => {}
        (key, dialog) => {
            if !dialog.field_mut().is_some_and(|field| field.edit_key(key)) {
                return false;
            }
            view.review.typed();
        }
    }
    cx.notify();
    true
}

/// OK.
fn confirm(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    match view.review.dialog.take() {
        Some(ReviewDialog::User(form)) => {
            if let Some(chosen) = form.reviewer() {
                set_reviewer(chosen.clone());
                // Changes typed from now on go down to the new name.
                if view.document.is_tracking_changes() {
                    let result = view.document.set_tracking_changes(true, &chosen);
                    view.reviewed(result, cx);
                }
            }
        }
        Some(ReviewDialog::AltText(form)) => {
            let result = view.document.set_alt_text(form.target, form.alt_text());
            view.reviewed(result, cx);
        }
        Some(ReviewDialog::Compare(dialog)) => {
            crate::word_compare::confirm(view, *dialog, cx);
        }
        None => {}
    }
    cx.notify();
}

/// A labelled one-line box; a click on it gives it the keyboard.
fn field(
    id: &'static str,
    label: &'static str,
    edit: &TextEdit,
    focused: bool,
    focus: fn(&mut ReviewDialog),
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    let (before, after) = edit.split();
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .w(px(88.0))
                .text_color(theme::text_dim())
                .child(i18n::text(label)),
        )
        .child(
            div()
                .id(id)
                .flex()
                .flex_wrap()
                .items_center()
                .w(px(280.0))
                .min_h(px(24.0))
                .px_2()
                .rounded_md()
                .bg(theme::canvas())
                .border_1()
                .border_color(if focused {
                    theme::accent()
                } else {
                    theme::border()
                })
                .cursor_text()
                .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                    if let Some(dialog) = view.review.dialog.as_mut() {
                        focus(dialog);
                    }
                    cx.notify();
                }))
                .when(focused && edit.whole, |row| {
                    row.child(div().bg(theme::selected()).child(edit.text.clone()))
                })
                .when(!(focused && edit.whole), |row| {
                    row.child(before.to_string())
                })
                .when(focused, |row| {
                    row.child(div().w(px(1.0)).h(px(16.0)).bg(theme::accent()))
                })
                .when(!(focused && edit.whole), |row| row.child(after.to_string())),
        )
        .into_any_element()
}

/// The open dialog, over the window.
pub fn overlay(
    view: &mut DocumentView,
    window: &mut Window,
    cx: &mut Context<DocumentView>,
) -> Option<AnyElement> {
    let (title, body) = match view.review.dialog.as_ref()? {
        ReviewDialog::User(form) => (
            "review.user.title",
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_color(theme::text_dim())
                        .child(i18n::text("review.user.hint")),
                )
                .child(field(
                    "review-user-name",
                    "review.user.name",
                    &form.name,
                    form.focus == ReviewerField::Name,
                    |dialog| {
                        if let ReviewDialog::User(form) = dialog {
                            form.focus = ReviewerField::Name;
                        }
                    },
                    cx,
                ))
                .child(field(
                    "review-user-initials",
                    "review.user.initials",
                    &form.initials,
                    form.focus == ReviewerField::Initials,
                    |dialog| {
                        if let ReviewDialog::User(form) = dialog {
                            form.focus = ReviewerField::Initials;
                        }
                    },
                    cx,
                ))
                .into_any_element(),
        ),
        ReviewDialog::Compare(dialog) => (
            if dialog.form.combine {
                "compare.combine_title"
            } else {
                "compare.title"
            },
            crate::word_compare::body(dialog, cx),
        ),
        ReviewDialog::AltText(form) => {
            let decorative = form.decorative;
            (
                "alt.title",
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .w(px(380.0))
                    .child(
                        div()
                            .text_color(theme::text_dim())
                            .child(i18n::text("alt.hint")),
                    )
                    .when(!decorative, |body| {
                        body.child(field(
                            "alt-description",
                            "alt.description",
                            &form.description,
                            true,
                            |_| {},
                            cx,
                        ))
                    })
                    .child(
                        crate::word_characters::check(
                            "alt-decorative",
                            i18n::text("alt.decorative"),
                            decorative,
                        )
                        .on_click(cx.listener(
                            |view, _: &ClickEvent, _, cx| {
                                if let Some(ReviewDialog::AltText(form)) =
                                    view.review.dialog.as_mut()
                                {
                                    form.decorative = !form.decorative;
                                }
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        div()
                            .text_color(theme::text_faint())
                            .child(i18n::text("alt.decorative_hint")),
                    )
                    .into_any_element(),
            )
        }
    };
    Some(modal("review-dialog", i18n::text(title), body, window, cx))
}

/// 代替テキスト… for the picture or shape last clicked.
pub fn open_alt_text(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let Some(object) = view.selected_object else {
        view.table_status = Some(i18n::text("alt.no_object").into());
        return cx.notify();
    };
    let target = jade_office_model::word::picture::ObjectTarget {
        paragraph_index: object.paragraph,
        run_index: object.run,
        kind: object.kind,
    };
    open_alt_text_for(view, target, cx);
}

/// 代替テキスト… for `target`.
pub fn open_alt_text_for(
    view: &mut DocumentView,
    target: jade_office_model::word::picture::ObjectTarget,
    cx: &mut Context<DocumentView>,
) {
    let alt = view.document.alt_text(target).unwrap_or_default();
    view.review.dialog = Some(ReviewDialog::AltText(AltTextForm::new(target, alt)));
    cx.notify();
}

/// A dialog over the window: its title, its body, キャンセル and OK.
pub fn modal(
    id: &'static str,
    title: &'static str,
    body: AnyElement,
    window: &mut Window,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    let viewport = window.viewport_size();
    deferred(
        anchored().position(point(px(0.0), px(0.0))).child(
            div()
                .id("review-dialog-backdrop")
                .w(viewport.width)
                .h(viewport.height)
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::rgba(0x00000040))
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .id(id)
                        .max_h(viewport.height - px(40.0))
                        .overflow_y_scroll()
                        .p_4()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .rounded_lg()
                        .border_1()
                        .border_color(theme::border())
                        .bg(theme::panel())
                        .shadow_lg()
                        .font_family(crate::UI_FONT_FAMILY)
                        .text_xs()
                        .text_color(theme::text())
                        .child(div().text_sm().child(title))
                        .child(body)
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .gap_2()
                                .child(
                                    button("review-cancel", i18n::text("format.cancel"), false)
                                        .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
                                            view.review.dialog = None;
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    button("review-ok", i18n::text("format.ok"), true).on_click(
                                        cx.listener(|view, _: &ClickEvent, _, cx| {
                                            confirm(view, cx)
                                        }),
                                    ),
                                ),
                        ),
                ),
        ),
    )
    .with_priority(3)
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A blank document in a window, with the keyboard on it.
    fn blank(
        cx: &mut gpui::TestAppContext,
    ) -> (gpui::Entity<DocumentView>, &mut gpui::VisualTestContext) {
        use gpui::Focusable;
        let bytes = jade_office_model::word::new::blank_document(
            jade_office_model::word::new::Blank::Japanese,
        );
        let (view, cx) = cx
            .add_window_view(|_window, cx| crate::untitled_view(&bytes, "校閲.docx", cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        view.update(cx, |view, _| {
            view.insert_at = (0, 0);
            view.caret = view.document.place_caret(0, 0);
        });
        (view, cx)
    }

    /// The tab's views change what the pages show and not the document;
    /// a reviewer unticked has their changes decided last, 表示された
    /// changes first.
    #[gpui::test]
    fn markup_views_and_reviewers_are_chosen_from_the_tab(cx: &mut gpui::TestAppContext) {
        let (view, cx) = blank(cx);
        view.update(cx, |view, cx| {
            set_reviewer(Reviewer::new("Ann Lee", "AL"));
            view.toggle_tracking(cx);
        });
        cx.simulate_input("Ann wrote this ");
        view.update(cx, |view, cx| {
            let paragraph = view.insert_at.0;
            let before = view.document.paragraph_text(paragraph).unwrap();
            assert!(view.document.reviewers().contains(&"Ann Lee".to_string()));

            run(view, ReviewAction::Markup(MarkupMode::Simple), cx);
            assert_eq!(view.document.revision_display(), (false, false));
            run(view, ReviewAction::Markup(MarkupMode::Original), cx);
            assert_eq!(view.document.revision_display(), (false, true));
            run(view, ReviewAction::Markup(MarkupMode::All), cx);
            assert_eq!(view.document.revision_display(), (true, false));
            assert_eq!(view.document.paragraph_text(paragraph).unwrap(), before);

            run(view, ReviewAction::Reviewer(Some("Ann Lee".into())), cx);
            assert!(!view.review.markup.shows_reviewer("Ann Lee"));
            run(view, ReviewAction::Decide(true, Decision::Visible), cx);
            assert!(view
                .document
                .revisions()
                .iter()
                .any(|revision| revision.author.as_deref() == Some("Ann Lee")));
            // すべての校閲者: everyone back, then everyone off.
            run(view, ReviewAction::Reviewer(None), cx);
            assert!(view.review.markup.hidden_reviewers.is_empty());
            run(view, ReviewAction::Reviewer(None), cx);
            assert!(!view.review.markup.shows_reviewer("Ann Lee"));
            run(view, ReviewAction::Reviewer(None), cx);
            run(view, ReviewAction::Decide(true, Decision::AllAndStop), cx);
            assert!(view.document.revisions().is_empty());
            assert!(!view.document.is_tracking_changes());
        });
    }

    /// ユーザー情報 takes a name and initials from the keyboard, and the
    /// next comment goes down to them.
    #[gpui::test]
    fn user_information_names_the_next_comment(cx: &mut gpui::TestAppContext) {
        let (view, cx) = blank(cx);
        cx.simulate_input("Some text");
        view.update(cx, |view, cx| run(view, ReviewAction::UserInfo, cx));
        cx.simulate_input("Mary Major");
        cx.simulate_keystrokes("enter");
        assert_eq!(reviewer(), Reviewer::new("Mary Major", "MM"));
        view.update(cx, |view, cx| {
            assert!(view.review.dialog.is_none());
            let paragraph = view.insert_at.0;
            view.document
                .select((paragraph, 0), (paragraph, 2), 1)
                .unwrap();
            view.open_comment_bar(crate::word_review::CommentEntry::New, String::new(), cx);
        });
        cx.simulate_input("Mine.");
        cx.simulate_keystrokes("enter");
        view.read_with(cx, |view, _| {
            let comment = view
                .document
                .comments()
                .into_iter()
                .find(|comment| comment.text == "Mine.")
                .unwrap();
            assert_eq!(comment.author.as_deref(), Some("Mary Major"));
        });
    }
}

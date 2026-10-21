//! 編集 / 校閲 / 閲覧 — the switch at the right of the ribbon's tabs — and
//! フォーカス mode.
//!
//! - 編集: the document as usual.
//! - 校閲: every change is recorded (変更履歴の記録 on), as Word's
//!   Reviewing mode does.
//! - 閲覧: nothing can change: typing, deleting, pasting and the commands
//!   that edit do nothing, so the document can be read without accidents.
//! - フォーカス: the ribbon, the page list and the panes go, the window
//!   fills the screen, and Escape brings them back.

use gpui::{anchored, deferred, div, prelude::*, px, AnyElement, Context, MouseButton, Window};

use crate::ribbon::Action;
use crate::tooltip::Tooltip;
use crate::word_dialog::on;
use crate::{i18n, theme, DocumentView};
use jade_office_model::word::review::WordReview;

/// The switch's three modes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EditMode {
    #[default]
    Editing,
    Reviewing,
    Viewing,
}

impl EditMode {
    pub const ALL: [EditMode; 3] = [EditMode::Editing, EditMode::Reviewing, EditMode::Viewing];

    pub fn label_key(self) -> &'static str {
        match self {
            EditMode::Editing => "modes.editing",
            EditMode::Reviewing => "modes.reviewing",
            EditMode::Viewing => "modes.viewing",
        }
    }

    fn hint_key(self) -> &'static str {
        match self {
            EditMode::Editing => "modes.editing_hint",
            EditMode::Reviewing => "modes.reviewing_hint",
            EditMode::Viewing => "modes.viewing_hint",
        }
    }
}

/// Changes the mode, turning change tracking on for 校閲 and off when
/// leaving it.
pub fn set_mode(view: &mut DocumentView, mode: EditMode, cx: &mut Context<DocumentView>) {
    let before = view.views.mode;
    view.views.mode_menu = false;
    if before == mode {
        return cx.notify();
    }
    view.views.mode = mode;
    let tracking = view.document.is_tracking_changes();
    if (mode == EditMode::Reviewing) != tracking
        && (mode == EditMode::Reviewing || before == EditMode::Reviewing)
    {
        view.toggle_tracking(cx);
    }
    cx.notify();
}

/// Whether the document cannot change (閲覧, 閲覧モード).
pub fn read_only(view: &DocumentView) -> bool {
    view.views.mode == EditMode::Viewing
        || view.views.kind == jade_office_model::word::view::ViewKind::Read
}

/// Whether a key edits the document.
pub fn is_edit_key(key: &str) -> bool {
    matches!(key, "backspace" | "delete" | "enter" | "tab")
}

/// Whether a ribbon command is refused while nothing can change: anything
/// that is not opening a list, the view's own commands or a way around the
/// document.
pub fn refuses(view: &DocumentView, action: &Action) -> bool {
    use crate::word_references::RefAction;
    if !read_only(view) || action.is_chrome() {
        return false;
    }
    !matches!(
        action,
        Action::Views(_)
            | Action::References(
                RefAction::ToggleNavigation
                    | RefAction::OpenGoTo
                    | RefAction::StepComment(_)
                    | RefAction::StepNote(..)
                    | RefAction::ShowNotes
                    | RefAction::ToggleShowBookmarks
            )
    )
}

/// The switch at the right of the tab strip, its list under it while open.
pub fn switch(view: &DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    let mode = view.views.mode;
    let menu = view.views.mode_menu.then(|| {
        deferred(
            anchored().snap_to_window_with_margin(px(8.0)).child(
                div()
                    .id("mode-menu")
                    .occlude()
                    .mt_1()
                    .p_1()
                    .w(px(260.0))
                    .flex()
                    .flex_col()
                    .rounded_md()
                    .bg(theme::panel())
                    .border_1()
                    .border_color(theme::border())
                    .shadow_lg()
                    .text_xs()
                    .text_color(theme::text())
                    .children(EditMode::ALL.into_iter().enumerate().map(|(index, each)| {
                        div()
                            .id(("mode", index))
                            .flex()
                            .flex_col()
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .cursor_pointer()
                            .when(each == mode, |row| row.bg(theme::selected()))
                            .hover(|row| row.bg(theme::hovered()))
                            .on_click(on(cx, move |view, cx| set_mode(view, each, cx)))
                            .child(i18n::text(each.label_key()))
                            .child(
                                div()
                                    .text_color(theme::text_faint())
                                    .child(i18n::text(each.hint_key())),
                            )
                    })),
            ),
        )
        .with_priority(2)
    });
    div()
        .relative()
        .flex_none()
        .child(
            div()
                .id("mode-switch")
                .flex()
                .items_center()
                .gap_1()
                .px_2()
                .py_1()
                .mr_2()
                .rounded_md()
                .border_1()
                .border_color(theme::border())
                .cursor_pointer()
                .hover(|button| button.bg(theme::hovered()))
                .text_sm()
                .tooltip(Tooltip::text(i18n::text("modes.title")))
                .on_click(on(cx, |view, cx| {
                    view.views.mode_menu = !view.views.mode_menu;
                    cx.notify();
                }))
                .child(i18n::text(mode.label_key()))
                .child(
                    gpui::svg()
                        .path("icons/chevron-down.svg")
                        .size_3()
                        .text_color(theme::text_dim()),
                ),
        )
        .children(menu.map(|menu| div().absolute().top_full().right_0().child(menu)))
        .into_any_element()
}

/// フォーカス: on or off, the window to full screen and back.
pub fn toggle_focus(view: &mut DocumentView, window: &mut Window, cx: &mut Context<DocumentView>) {
    view.views.focus = !view.views.focus;
    view.views.mini = false;
    if window.is_fullscreen() != view.views.focus {
        window.toggle_fullscreen();
    }
    cx.notify();
}

/// The フォーカス button on 表示.
pub fn focus_button(view: &DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    div()
        .id("views-focus")
        .flex()
        .flex_none()
        .items_center()
        .h(px(26.0))
        .px_2()
        .rounded_md()
        .text_xs()
        .whitespace_nowrap()
        .cursor_pointer()
        .when(view.views.focus, |button| button.bg(theme::selected()))
        .hover(|button| button.bg(theme::hovered()))
        .tooltip(Tooltip::text(i18n::text("views.focus_title")))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(|view, _event, window, cx| toggle_focus(view, window, cx)))
        .child(i18n::text("views.focus"))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 閲覧: typing, Backspace and Bold change nothing; 校閲 records
    /// changes; 編集 again stops recording.
    #[gpui::test]
    fn the_modes_guard_and_record(cx: &mut gpui::TestAppContext) {
        let (view, cx) = crate::word_views::tests::open_contract(cx);
        let before = view.update(cx, |view, cx| {
            set_mode(view, EditMode::Viewing, cx);
            view.current_paragraph_text()
        });
        cx.simulate_input("xyz");
        cx.simulate_keystrokes("backspace");
        view.update(cx, |view, cx| {
            assert_eq!(view.current_paragraph_text(), before);
            assert!(!view.dirty);
            let mut range = view.insert_at;
            range.1 = 0;
            view.document.select(range, view.insert_at, 1).ok();
            view.run_ribbon(
                Action::Toggle(jade_office_model::word::CharacterFormat::Bold),
                cx,
            );
            assert!(!view.dirty, "閲覧 refuses formatting");
            set_mode(view, EditMode::Reviewing, cx);
            assert!(view.document.is_tracking_changes());
            set_mode(view, EditMode::Editing, cx);
            assert!(!view.document.is_tracking_changes());
        });
    }

    /// フォーカス hides the chrome, and Escape brings it back.
    #[gpui::test]
    fn focus_mode_comes_and_goes(cx: &mut gpui::TestAppContext) {
        let (view, cx) = crate::word_views::tests::open_contract(cx);
        cx.update(|window, cx| {
            view.update(cx, |view, cx| toggle_focus(view, window, cx));
        });
        cx.run_until_parked();
        view.read_with(cx, |view, _| assert!(view.views.focus));
        cx.simulate_keystrokes("escape");
        view.read_with(cx, |view, _| assert!(!view.views.focus));
    }
}

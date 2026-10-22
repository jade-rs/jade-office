//! 編集 / 校閲 / 閲覧 — the switch at the right of the ribbon's tabs — and
//! フォーカス, in the browser.
//!
//! - 編集: the document as usual.
//! - 校閲: every change is recorded (変更履歴の記録 on), as Word's Reviewing
//!   mode does.
//! - 閲覧: nothing can change — typing, deleting, pasting and the commands
//!   that edit do nothing — so the document can be read without accidents.
//! - フォーカス: the ribbon, the page list, the panes and the workspace
//!   round them go, the page fills the browser (full screen when it lets
//!   us), and Escape brings them back.

use gpui::{anchored, deferred, div, prelude::*, px, AnyElement, Context, Window};
use jade_web::word::review::WordReview;
use jade_web::word::view::ViewKind;

use super::ViewsAction;
use crate::ribbon::Action;
use crate::view::word_kit::on;
use crate::view::DocView as DocumentView;
use crate::{i18n, palette as theme};

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
        return;
    }
    view.views.mode = mode;
    let tracking = view.doc().is_tracking_changes();
    if (mode == EditMode::Reviewing) != tracking
        && (mode == EditMode::Reviewing || before == EditMode::Reviewing)
    {
        view.review_action(crate::word_review::ReviewAction::ToggleTracking, cx);
    }
}

/// Whether the document cannot change (閲覧, 閲覧モード).
pub fn read_only(view: &DocumentView) -> bool {
    view.views.mode == EditMode::Viewing || view.views.kind == ViewKind::Read
}

/// Whether a ribbon command is refused while nothing can change: anything
/// that is not opening a list, the view's own commands or a way around the
/// document.
pub fn refuses(view: &DocumentView, action: &Action) -> bool {
    use crate::view::word_references::RefAction;
    use crate::word_review::ReviewAction;
    if !read_only(view) || action.is_chrome() {
        return false;
    }
    !matches!(
        action,
        Action::Views(_)
            | Action::View(_)
            | Action::Review(
                ReviewAction::StepComment(_)
                    | ReviewAction::ToggleComments
                    | ReviewAction::Immersive
                    | ReviewAction::ReadAloud
                    | ReviewAction::SideBySide
                    | ReviewAction::SyncScroll
                    | ReviewAction::ResetPosition
                    | ReviewAction::Accessibility
                    | ReviewAction::RevisionsPane
            )
            | Action::References(
                RefAction::ToggleNavigation
                    | RefAction::OpenGoTo
                    | RefAction::StepNote(..)
                    | RefAction::ShowNotes
                    | RefAction::ToggleShowBookmarks
            )
            | Action::Word(
                crate::word_tools::ToolCommand::ToggleRuler
                    | crate::word_tools::ToolCommand::RulerUnit(_)
                    | crate::word_tools::ToolCommand::ToggleStylesPane
            )
    )
}

/// The switch at the right of the tab strip, its list under it while open.
pub fn switch(mode: EditMode, open: bool, cx: &mut Context<DocumentView>) -> AnyElement {
    let menu = open.then(|| {
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
                            .on_click(crate::ribbon::listener(
                                Action::Views(ViewsAction::SetMode(each)),
                                cx,
                            ))
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
                .py_0p5()
                .mr_2()
                .rounded_md()
                .border_1()
                .border_color(theme::border())
                .cursor_pointer()
                .hover(|button| button.bg(theme::hovered()))
                .text_xs()
                .tooltip(crate::tooltip::Tooltip::text(i18n::text("modes.title")))
                .on_click(crate::ribbon::listener(
                    Action::Views(ViewsAction::ToggleModeMenu),
                    cx,
                ))
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

/// フォーカス: on or off, the browser to full screen and back.
pub fn toggle_focus(view: &mut DocumentView, window: &mut Window) {
    view.views.focus = !view.views.focus;
    view.views.mini = false;
    if window.is_fullscreen() != view.views.focus {
        window.toggle_fullscreen();
    }
}

/// フォーカス's bar: the way back.
pub fn focus_bar(cx: &mut Context<DocumentView>) -> AnyElement {
    div()
        .absolute()
        .top_2()
        .right_4()
        .child(
            div()
                .id("focus-exit")
                .px_2()
                .py_0p5()
                .rounded_md()
                .bg(gpui::rgba(0xffffff22))
                .text_xs()
                .text_color(gpui::rgb(0xe4e4e7))
                .cursor_pointer()
                .hover(|button| button.bg(gpui::rgba(0xffffff44)))
                .on_click(crate::ribbon::listener(
                    Action::Views(ViewsAction::ToggleFocus),
                    cx,
                ))
                .child(i18n::text("views.focus_title")),
        )
        .into_any_element()
}

/// 閲覧モード's bar over its columns: where the reader is, and the way back
/// to editing.
pub fn read_bar(current: usize, count: usize, cx: &mut Context<DocumentView>) -> AnyElement {
    let step = |forward: bool, cx: &Context<DocumentView>| {
        on(cx, move |view: &mut DocumentView, cx| {
            view.read_step(forward, cx)
        })
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap_3()
        .px_4()
        .py_2()
        .bg(theme::panel())
        .border_b_1()
        .border_color(theme::border())
        .text_sm()
        .child(
            div()
                .id("read-back")
                .px_2()
                .py_0p5()
                .rounded_md()
                .border_1()
                .border_color(theme::border())
                .cursor_pointer()
                .hover(|button| button.bg(theme::hovered()))
                .on_click(crate::ribbon::listener(
                    Action::Views(ViewsAction::SetView(ViewKind::Print)),
                    cx,
                ))
                .child(i18n::text("views.read.edit")),
        )
        .child(
            div()
                .id("read-previous")
                .px_2()
                .cursor_pointer()
                .on_click(step(false, cx))
                .child("◀"),
        )
        .child(div().text_color(theme::text_dim()).child(i18n::format(
            "views.read.position",
            &[("current", &(current + 1)), ("count", &count)],
        )))
        .child(
            div()
                .id("read-next")
                .px_2()
                .cursor_pointer()
                .on_click(step(true, cx))
                .child("▶"),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme::text_faint())
                .child(i18n::text("views.read.keys")),
        )
        .into_any_element()
}

impl DocumentView {
    /// 閲覧モード: a column on or back, the column to the left of the view.
    pub(crate) fn read_step(&mut self, forward: bool, cx: &mut Context<Self>) {
        let zoom = self.zoom.unwrap_or(1.0);
        let crate::view::Document::Pages { page_size, .. } = &self.document else {
            return;
        };
        let grid = self.arrangement(*page_size, zoom).grid;
        if let crate::view::Document::Pages {
            scroll,
            current,
            pages,
            ..
        } = &mut self.document
        {
            let count = pages.len();
            *current = if forward {
                (*current + 1).min(count.saturating_sub(1))
            } else {
                current.saturating_sub(1)
            };
            let (x, y) = grid.origin(*current);
            let max = scroll.max_offset();
            let target = if grid.horizontal {
                gpui::point(
                    gpui::px(-(x - grid.padding).clamp(0.0, f32::from(max.x).max(0.0))),
                    scroll.offset().y,
                )
            } else {
                gpui::point(
                    scroll.offset().x,
                    gpui::px(-(y - grid.padding).clamp(0.0, f32::from(max.y).max(0.0))),
                )
            };
            scroll.set_offset(target);
        }
        cx.notify();
    }

    /// The keys 閲覧モード and フォーカス take: ← → and the space bar turn
    /// the columns, Escape goes back. `true` when taken.
    pub(crate) fn read_key(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.views.focus && key == "escape" {
            self.views_action(ViewsAction::ToggleFocus, window, cx);
            return true;
        }
        if self.views.kind != ViewKind::Read {
            return false;
        }
        match key {
            "right" | "down" | "space" | "pagedown" => self.read_step(true, cx),
            "left" | "up" | "pageup" => self.read_step(false, cx),
            "escape" => self.views_action(ViewsAction::SetView(ViewKind::Print), window, cx),
            _ => return false,
        }
        true
    }
}

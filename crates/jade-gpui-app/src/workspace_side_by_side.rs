//! 表示 › ウィンドウ › 並べて比較: two Word documents beside each other,
//! with 同時にスクロール — scrolling one scrolls the other by as much — and
//! ウィンドウの位置を元に戻す, which lines both up at the same place again.
//! The document in front is paired with the Word document opened beside it
//! in the tab bar, as Word pairs the other open window.

use std::time::Duration;

use gpui::{div, prelude::*, px, AnyElement, AnyView, App, Context, Entity, Global, Pixels, Point};

use super::Workspace;
use crate::shell::Kind;
use crate::{i18n, theme, DocumentView};

gpui::actions!(
    jade,
    [ViewSideBySide, SynchronousScrolling, ResetWindowPosition]
);

/// The pair, while one is shown.
#[derive(Default)]
pub struct SideBySide {
    /// The two tabs, by path: the one compared from, and its partner.
    pair: Option<(std::path::PathBuf, std::path::PathBuf)>,
    sync: bool,
    /// Both offsets when last looked at.
    last: Option<(Point<Pixels>, Point<Pixels>)>,
    ticking: bool,
}

/// What the 表示 tab shows as on.
#[derive(Clone, Copy, Default)]
pub struct SideBySideShown {
    pub on: bool,
    pub sync: bool,
}

impl Global for SideBySideShown {}

const TICK: Duration = Duration::from_millis(30);

impl Workspace {
    fn word_view(&self, path: &std::path::Path) -> Option<Entity<DocumentView>> {
        self.tabs
            .iter()
            .find(|tab| tab.path == path && tab.kind == Kind::Document)
            .and_then(|tab| tab.view.clone().downcast::<DocumentView>().ok())
    }

    fn publish_side_by_side(&self, cx: &mut App) {
        cx.set_global(SideBySideShown {
            on: self.side.pair.is_some(),
            sync: self.side.sync,
        });
    }

    /// 並べて比較: on with the Word document beside the one in front, or off.
    pub(super) fn toggle_side_by_side(&mut self, cx: &mut Context<Self>) {
        if self.side.pair.take().is_none() {
            let Some(active) = self
                .active
                .filter(|&at| self.tabs[at].kind == Kind::Document)
            else {
                return;
            };
            // The next Word tab, else the one before.
            let partner = (active + 1..self.tabs.len())
                .chain((0..active).rev())
                .find(|&at| self.tabs[at].kind == Kind::Document);
            let Some(partner) = partner else {
                self.error = Some(i18n::text("side.needs_two").into());
                return cx.notify();
            };
            self.side = SideBySide {
                pair: Some((
                    self.tabs[active].path.clone(),
                    self.tabs[partner].path.clone(),
                )),
                sync: true,
                last: None,
                ticking: self.side.ticking,
            };
            self.start_sync(cx);
        }
        self.publish_side_by_side(cx);
        cx.notify();
    }

    /// 同時にスクロール.
    pub(super) fn toggle_sync(&mut self, cx: &mut Context<Self>) {
        if self.side.pair.is_none() {
            return;
        }
        self.side.sync = !self.side.sync;
        self.side.last = None;
        self.start_sync(cx);
        self.publish_side_by_side(cx);
        cx.notify();
    }

    /// ウィンドウの位置を元に戻す: the partner scrolled to where the first is.
    pub(super) fn reset_side_by_side(&mut self, cx: &mut Context<Self>) {
        let Some((first, second)) = self.side.pair.clone() else {
            return;
        };
        let (Some(first), Some(second)) = (self.word_view(&first), self.word_view(&second)) else {
            return;
        };
        let offset = first.read(cx).scroll.offset();
        second.read(cx).scroll.set_offset(offset);
        second.update(cx, |_, cx| cx.notify());
        self.side.last = None;
    }

    /// 同時にスクロール: whichever moved since the last look, the other
    /// moves by as much. `false` once there is nothing to keep in step.
    pub(super) fn sync_scroll(&mut self, cx: &mut Context<Self>) -> bool {
        let Some((first, second)) = self.side.pair.clone().filter(|_| self.side.sync) else {
            self.side.ticking = false;
            return false;
        };
        let (Some(first), Some(second)) = (self.word_view(&first), self.word_view(&second)) else {
            // One of them was closed.
            self.side = SideBySide::default();
            self.publish_side_by_side(cx);
            cx.notify();
            return false;
        };
        let (a, b) = (
            first.read(cx).scroll.offset(),
            second.read(cx).scroll.offset(),
        );
        if let Some((last_a, last_b)) = self.side.last {
            if a != last_a {
                second.read(cx).scroll.set_offset(b + (a - last_a));
                second.update(cx, |_, cx| cx.notify());
            } else if b != last_b {
                first.read(cx).scroll.set_offset(a + (b - last_b));
                first.update(cx, |_, cx| cx.notify());
            }
        }
        self.side.last = Some((
            first.read(cx).scroll.offset(),
            second.read(cx).scroll.offset(),
        ));
        true
    }

    fn start_sync(&mut self, cx: &mut Context<Self>) {
        if self.side.ticking || !self.side.sync || cfg!(test) {
            return;
        }
        self.side.ticking = true;
        let executor = cx.background_executor().clone();
        cx.spawn(async move |workspace, cx| loop {
            executor.timer(TICK).await;
            let going = workspace
                .update(cx, |workspace, cx| workspace.sync_scroll(cx))
                .unwrap_or(false);
            if !going {
                break;
            }
        })
        .detach();
    }

    /// The editor area: the tab in front, or the pair beside each other
    /// while one of them is in front.
    pub(super) fn editor_content(&self, active: Option<AnyView>) -> Option<AnyElement> {
        let active = active?;
        let (first, second) = self.side.pair.as_ref()?;
        let front = self.active_tab()?;
        if &front.path != first && &front.path != second {
            return Some(active.into_any_element());
        }
        let view = |path: &std::path::Path| {
            self.tabs
                .iter()
                .find(|tab| tab.path == path)
                .map(|tab| tab.view.clone())
        };
        let (Some(left), Some(right)) = (view(first), view(second)) else {
            return Some(active.into_any_element());
        };
        Some(
            div()
                .size_full()
                .flex()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .overflow_hidden()
                        .child(left),
                )
                .child(div().w(px(1.0)).h_full().bg(theme::border()))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .overflow_hidden()
                        .child(right),
                )
                .into_any_element(),
        )
    }
}

/// The 表示 tab's ウィンドウ group.
pub fn view_group(view: &mut DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    let shown = cx
        .try_global::<SideBySideShown>()
        .copied()
        .unwrap_or_default();
    let button = |id: &'static str,
                  key: &'static str,
                  tooltip: &'static str,
                  on: bool,
                  enabled: bool,
                  run: fn(&mut App),
                  cx: &mut Context<DocumentView>| {
        view.ribbon_button(
            id,
            i18n::text(key),
            i18n::text(tooltip),
            on,
            cx,
            move |_, cx| {
                if enabled {
                    run(cx)
                }
            },
        )
        .into_any_element()
    };
    let compare = button(
        "view-side-by-side",
        "side.button",
        "side.title",
        shown.on,
        true,
        |cx| cx.dispatch_action(&ViewSideBySide),
        cx,
    );
    let sync = button(
        "view-sync-scroll",
        "side.sync",
        "side.sync_title",
        shown.on && shown.sync,
        shown.on,
        |cx| cx.dispatch_action(&SynchronousScrolling),
        cx,
    );
    let reset = button(
        "view-reset-position",
        "side.reset",
        "side.reset_title",
        false,
        shown.on,
        |cx| cx.dispatch_action(&ResetWindowPosition),
        cx,
    );
    DocumentView::ribbon_group(
        i18n::text("side.group"),
        vec![DocumentView::ribbon_row(vec![compare, sync, reset])],
    )
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::point;

    fn fixture(name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core")).join(name)
    }

    /// 並べて比較 pairs the Word document in front with the other one and
    /// shows both; 同時にスクロール moves the second by as much as the first
    /// was scrolled, and the other way; turning it off leaves them apart.
    #[gpui::test]
    fn two_documents_side_by_side_scroll_together(cx: &mut gpui::TestAppContext) {
        let (workspace, cx) = cx.add_window_view(|_window, cx| Workspace::new(cx));
        cx.update(|window, cx| {
            workspace.update(cx, |workspace, cx| {
                workspace
                    .open(&fixture("contract.docx"), window, cx)
                    .unwrap();
                workspace
                    .open(&fixture("public/fixtures/review-kinds.docx"), window, cx)
                    .unwrap();
                workspace.active = Some(0);
            })
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            workspace.update(cx, |workspace, cx| {
                workspace.toggle_side_by_side(cx);
                assert!(workspace.side.pair.is_some());
                assert!(cx.global::<SideBySideShown>().on);
                let (first, second) = workspace.side.pair.clone().unwrap();
                let (a, b) = (
                    workspace.word_view(&first).unwrap(),
                    workspace.word_view(&second).unwrap(),
                );
                workspace.sync_scroll(cx);
                a.read(cx).scroll.set_offset(point(px(0.0), px(-300.0)));
                workspace.sync_scroll(cx);
                // The handles are not laid out in a test, so the offset is
                // what was set, unclamped.
                assert_eq!(b.read(cx).scroll.offset().y, px(-300.0));
                b.read(cx).scroll.set_offset(point(px(0.0), px(-500.0)));
                workspace.sync_scroll(cx);
                assert_eq!(a.read(cx).scroll.offset().y, px(-500.0));
                workspace.toggle_sync(cx);
                a.read(cx).scroll.set_offset(point(px(0.0), px(-100.0)));
                workspace.sync_scroll(cx);
                assert_eq!(b.read(cx).scroll.offset().y, px(-500.0));
                workspace.toggle_side_by_side(cx);
                assert!(!cx.global::<SideBySideShown>().on);
            })
        });
    }
}

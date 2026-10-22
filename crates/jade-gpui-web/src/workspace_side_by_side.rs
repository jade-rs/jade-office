//! 並べて比較 in the browser: two Word documents beside each other, with
//! 同時にスクロール and ウィンドウの位置を元に戻す, as the native app's
//! `workspace::side_by_side`. The document in front is paired with the Word
//! document opened beside it in the tab bar.

use std::time::Duration;

use gpui::{div, prelude::*, px, AnyElement, AnyView, App, Context, Entity, Global, Pixels, Point};

use super::Workspace;
use crate::view::{DocView, Kind};
use crate::{i18n, palette};

gpui::actions!(
    jade,
    [ViewSideBySide, SynchronousScrolling, ResetWindowPosition]
);

/// The pair, while one is shown.
#[derive(Default)]
pub struct SideBySide {
    pair: Option<(gpui::EntityId, gpui::EntityId)>,
    sync: bool,
    last: Option<(Point<Pixels>, Point<Pixels>)>,
    ticking: bool,
}

/// What the ribbon shows as on.
#[derive(Clone, Copy, Default)]
pub struct SideBySideShown {
    pub on: bool,
    pub sync: bool,
}

impl Global for SideBySideShown {}

const TICK: Duration = Duration::from_millis(30);

impl Workspace {
    fn tab_by_id(&self, id: gpui::EntityId) -> Option<&Entity<DocView>> {
        self.tabs.iter().find(|tab| tab.entity_id() == id)
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
            let is_word = |tab: &Entity<DocView>, cx: &App| tab.read(cx).kind == Kind::Document;
            let Some(active) = self.active.filter(|&at| is_word(&self.tabs[at], cx)) else {
                return;
            };
            let partner = (active + 1..self.tabs.len())
                .chain((0..active).rev())
                .find(|&at| is_word(&self.tabs[at], cx));
            let Some(partner) = partner else {
                self.error = Some(i18n::text("side.needs_two").into());
                return cx.notify();
            };
            self.side = SideBySide {
                pair: Some((
                    self.tabs[active].entity_id(),
                    self.tabs[partner].entity_id(),
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
        let Some((first, second)) = self.side.pair else {
            return;
        };
        let (Some(first), Some(second)) = (self.tab_by_id(first), self.tab_by_id(second)) else {
            return;
        };
        if let (Some(a), Some(b)) = (first.read(cx).page_scroll(), second.read(cx).page_scroll()) {
            b.set_offset(a.offset());
            second.update(cx, |_, cx| cx.notify());
        }
        self.side.last = None;
    }

    /// 同時にスクロール: whichever moved since the last look, the other
    /// moves by as much. `false` once there is nothing to keep in step.
    pub(super) fn sync_scroll(&mut self, cx: &mut Context<Self>) -> bool {
        let Some((first, second)) = self.side.pair.filter(|_| self.side.sync) else {
            self.side.ticking = false;
            return false;
        };
        let (Some(first), Some(second)) = (
            self.tab_by_id(first).cloned(),
            self.tab_by_id(second).cloned(),
        ) else {
            self.side = SideBySide::default();
            self.publish_side_by_side(cx);
            cx.notify();
            return false;
        };
        let (Some(a), Some(b)) = (first.read(cx).page_scroll(), second.read(cx).page_scroll())
        else {
            return true;
        };
        let (at_a, at_b) = (a.offset(), b.offset());
        if let Some((last_a, last_b)) = self.side.last {
            if at_a != last_a {
                b.set_offset(at_b + (at_a - last_a));
                second.update(cx, |_, cx| cx.notify());
            } else if at_b != last_b {
                a.set_offset(at_a + (at_b - last_b));
                first.update(cx, |_, cx| cx.notify());
            }
        }
        self.side.last = Some((a.offset(), b.offset()));
        true
    }

    fn start_sync(&mut self, cx: &mut Context<Self>) {
        if self.side.ticking || !self.side.sync {
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

    /// The editor area: the pair beside each other while one of them is in
    /// front.
    pub(super) fn editor_content(&self) -> Option<AnyElement> {
        let (first, second) = self.side.pair?;
        let front = self.active_tab()?.entity_id();
        if front != first && front != second {
            return None;
        }
        let left = AnyView::from(self.tab_by_id(first)?.clone());
        let right = AnyView::from(self.tab_by_id(second)?.clone());
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
                .child(div().w(px(1.0)).h_full().bg(palette::border()))
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

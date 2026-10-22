//! 表示 → ウィンドウ in the browser: 分割 (a second pane over the same
//! document, scrolled on its own, above the first) and 新しいウィンドウ (the
//! document again in a window of its own inside the page, titled
//! `名前:2`). Both show the document's own pages, so an edit in one is in
//! the other at once; a click or a drag in either goes to the same
//! document, mapped through where that pane drew the pages.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    anchored, canvas, deferred, div, point, prelude::*, px, AnyElement, Bounds, Context,
    MouseButton, Pixels, ScrollHandle, SharedString, Window,
};

use super::{Arrangement, ViewsAction};
use crate::ribbon::Action;
use crate::view::{DocView as DocumentView, Document};
use crate::{i18n, palette as theme};

pub type PageBounds = Rc<RefCell<Vec<Option<Bounds<Pixels>>>>>;

/// A pane the pages are drawn in besides the view's own.
#[derive(Clone)]
pub struct Pane {
    pub scroll: ScrollHandle,
    pub bounds: PageBounds,
}

impl Default for Pane {
    fn default() -> Self {
        Pane {
            scroll: ScrollHandle::new(),
            bounds: Rc::new(RefCell::new(Vec::new())),
        }
    }
}

/// 分割 on or off.
pub fn toggle_split(view: &mut DocumentView) {
    view.views.split = match view.views.split.take() {
        Some(_) => None,
        None => {
            let pane = Pane::default();
            if let Document::Pages { current, .. } = &view.document {
                pane.scroll.scroll_to_item(*current);
            }
            Some(pane)
        }
    };
}

/// 新しいウィンドウ: the document in a window of its own, in the page.
pub fn open_window(view: &mut DocumentView) {
    if view.views.window.is_some() {
        return;
    }
    view.views.windows += 1;
    let pane = Pane::default();
    if let Document::Pages { current, .. } = &view.document {
        pane.scroll.scroll_to_item(*current);
    }
    view.views.window = Some(pane);
}

impl DocumentView {
    /// Runs `f` with the view mapping points through `bounds`, as if the
    /// pages were where that pane drew them.
    pub(crate) fn with_pane_bounds<R>(
        &mut self,
        bounds: &PageBounds,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let swap = |view: &mut Self| {
            if let Document::Pages { page_bounds, .. } = &view.document {
                std::mem::swap(&mut *page_bounds.borrow_mut(), &mut *bounds.borrow_mut());
            }
        };
        swap(self);
        let result = f(self);
        swap(self);
        result
    }

    /// A drag or a release, through the pane the press was in.
    pub(crate) fn through_drag_pane<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        match self.views.drag_pane.clone() {
            Some(bounds) => self.with_pane_bounds(&bounds, f),
            None => f(self),
        }
    }

    /// The pages a pane shows, from its scroll position.
    pub(crate) fn pane_range(
        &self,
        pane: &Pane,
        arrangement: &Arrangement,
    ) -> std::ops::Range<usize> {
        let Document::Pages { pages, .. } = &self.document else {
            return 0..0;
        };
        let offset = pane.scroll.offset();
        let size = pane.scroll.bounds().size;
        arrangement.grid.visible(
            (-f32::from(offset.x), -f32::from(offset.y)),
            (
                f32::from(size.width).max(200.0),
                f32::from(size.height).max(200.0),
            ),
            pages.len(),
        )
    }

    /// The document's pages drawn in `pane`, their mouse going to the
    /// document through that pane's bounds.
    pub(crate) fn pane_pages(
        &self,
        pane: &Pane,
        id: &'static str,
        zoom: f32,
        arrangement: &Arrangement,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let Document::Pages {
            document,
            pages,
            page_size,
            highlight,
            ..
        } = &self.document
        else {
            return div().id(id);
        };
        let (width, height) = (page_size.0 * zoom, page_size.1 * zoom);
        let highlight = highlight
            .as_ref()
            .map(|(_, rects)| rects.clone())
            .unwrap_or_default();
        let caret = document.caret_rect().map(|caret| {
            let (page, y) = document.page_and_offset(caret.document_y);
            (
                page,
                caret.x as f32 * zoom,
                y as f32 * zoom,
                caret.height as f32 * zoom,
            )
        });
        pane.bounds.borrow_mut().resize(pages.len(), None);
        let surfaces = pages.iter().enumerate().map(|(index, page)| {
            let crop = arrangement.crops.get(index).copied().flatten();
            let store = Rc::clone(&pane.bounds);
            let (down, right) = (Rc::clone(&pane.bounds), Rc::clone(&pane.bounds));
            let inner = div()
                .relative()
                .w(px(width))
                .h(px(height))
                .child(page.element())
                .children(
                    highlight
                        .iter()
                        .filter(|rect| rect.page == index)
                        .map(|rect| {
                            div()
                                .absolute()
                                .left(px(rect.x as f32 * zoom))
                                .top(px(rect.y as f32 * zoom))
                                .w(px(rect.width as f32 * zoom))
                                .h(px(rect.height as f32 * zoom))
                                .bg(gpui::rgba(0x584f_e833))
                        }),
                )
                .children(caret.filter(|(page, ..)| *page == index).map(
                    |(_, x, y, caret_height)| {
                        div()
                            .absolute()
                            .left(px(x))
                            .top(px(y))
                            .w(px(1.5))
                            .h(px(caret_height.max(8.0)))
                            .bg(theme::ink())
                    },
                ))
                .child(
                    canvas(
                        move |bounds, _window, _cx| {
                            if let Some(slot) = store.borrow_mut().get_mut(index) {
                                *slot = Some(bounds);
                            }
                            bounds
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
                );
            super::frame_page(
                div().id((id, index)),
                inner,
                crop,
                (width, height),
                zoom,
                arrangement.sheet,
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, event, window, cx| {
                    view.views.drag_pane = Some(Rc::clone(&down));
                    view.with_pane_bounds(&down, |view| {
                        view.page_mouse_down(index, event, window, cx)
                    });
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |view, event, window, cx| {
                    view.with_pane_bounds(&right, |view| {
                        view.page_right_click(index, event, window, cx)
                    });
                }),
            )
        });
        super::lay_out_pages(div().id(id), arrangement, surfaces.collect())
            .track_scroll(&pane.scroll)
            .on_scroll_wheel(cx.listener(|_view, _event, _window, cx| cx.notify()))
    }

    /// 分割's second pane, over the view's own, while it is on.
    pub(crate) fn split_pane(
        &self,
        zoom: f32,
        arrangement: &Arrangement,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let pane = self.views.split.as_ref()?;
        Some(
            div()
                .flex()
                .flex_col()
                .flex_none()
                .h(gpui::relative(0.45))
                .min_h_0()
                .border_b_4()
                .border_color(theme::border())
                .child(
                    self.pane_pages(pane, "split-pages", zoom, arrangement, cx)
                        .size_full(),
                )
                .into_any_element(),
        )
    }

    /// 新しいウィンドウ's window, floating over the page, while it is open.
    pub(crate) fn window_pane(
        &self,
        zoom: f32,
        arrangement: &Arrangement,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let pane = self.views.window.as_ref()?;
        let viewport = window.viewport_size();
        let (width, height) = (viewport.width * 0.46, viewport.height * 0.68);
        let title = SharedString::from(format!("{}:{}", self.name, self.views.windows + 1));
        Some(
            deferred(
                anchored()
                    .position(point(viewport.width - width - px(24.0), px(150.0)))
                    .child(
                        div()
                            .id("new-window")
                            .occlude()
                            .w(width)
                            .h(height)
                            .flex()
                            .flex_col()
                            .rounded_md()
                            .overflow_hidden()
                            .border_1()
                            .border_color(theme::border())
                            .bg(theme::window())
                            .shadow_lg()
                            .child(
                                div()
                                    .flex()
                                    .flex_none()
                                    .items_center()
                                    .gap_2()
                                    .px_3()
                                    .py_1p5()
                                    .bg(theme::panel())
                                    .border_b_1()
                                    .border_color(theme::border())
                                    .text_xs()
                                    .child(div().text_color(theme::text()).child(title))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .text_color(theme::text_faint())
                                            .child(i18n::text("views.window_hint")),
                                    )
                                    .child(
                                        div()
                                            .id("new-window-close")
                                            .px_1()
                                            .rounded_sm()
                                            .cursor_pointer()
                                            .hover(|button| button.bg(theme::hovered()))
                                            .on_click(crate::ribbon::listener(
                                                Action::Views(ViewsAction::CloseWindow),
                                                cx,
                                            ))
                                            .child("✕"),
                                    ),
                            )
                            .child(
                                self.pane_pages(pane, "window-pages", zoom, arrangement, cx)
                                    .flex_1()
                                    .min_h_0(),
                            ),
                    ),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }
}

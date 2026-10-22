//! 表示 → ウィンドウ: 分割 (a second pane over the same document, scrolled
//! on its own) and 新しいウィンドウ (the document in a window of its own,
//! as `名前:2`). Both show the document's own pages, so an edit in one is
//! in the other at once; a click or a drag in either goes to the same
//! document, mapped through where that pane drew the pages.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    canvas, div, img, prelude::*, px, AnyElement, App, Bounds, Context, Entity, FocusHandle,
    Focusable, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    ScrollHandle, SharedString, WeakEntity, Window,
};

use crate::{i18n, theme, DocumentView};

type PageBounds = Rc<RefCell<Vec<Option<Bounds<Pixels>>>>>;

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

/// Runs `f` with the view mapping points through `bounds`, as if the pages
/// were where that pane drew them.
pub fn with_bounds<R>(
    view: &mut DocumentView,
    bounds: &PageBounds,
    f: impl FnOnce(&mut DocumentView) -> R,
) -> R {
    view.page_bounds.swap(bounds);
    let result = f(view);
    view.page_bounds.swap(bounds);
    result
}

/// 分割 on or off.
pub fn toggle_split(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    view.views.split = match view.views.split.take() {
        Some(_) => None,
        None => {
            let pane = Pane::default();
            pane.scroll.scroll_to_item(view.current_page);
            Some(pane)
        }
    };
    cx.notify();
}

/// The pages of the document drawn in `pane`, their mouse going to `doc`.
pub fn pages(
    view: &DocumentView,
    pane: &Pane,
    doc: WeakEntity<DocumentView>,
    id: &'static str,
) -> gpui::Stateful<gpui::Div> {
    let (page_width, page_height) = {
        let (width, height) = view.document.layout_page_size();
        (width as f32, height as f32)
    };
    let zoom = view.zoom;
    super::arrange_pages(div().id(id), &view.views)
        .track_scroll(&pane.scroll)
        .children(view.images.iter().enumerate().map(|(index, page)| {
            let crop = super::page_crop(view, index);
            let store = Rc::clone(&pane.bounds);
            let (down, moved, up) = (doc.clone(), doc.clone(), doc.clone());
            let (down_bounds, move_bounds, up_bounds) = (
                pane.bounds.clone(),
                pane.bounds.clone(),
                pane.bounds.clone(),
            );
            super::frame_page(
                div().id((id, index)).relative().flex_none(),
                &view.views,
                crop,
                zoom,
            )
            .on_mouse_down(
                MouseButton::Left,
                move |event: &MouseDownEvent, _window, cx: &mut App| {
                    down.update(cx, |view, cx| {
                        with_bounds(view, &down_bounds, |view| {
                            view.click_page(index, event, cx);
                            if event.click_count == 2 {
                                view.select_word_at_caret(cx);
                            } else if event.click_count >= 3 {
                                view.select_paragraph(cx);
                            }
                        })
                    })
                    .ok();
                },
            )
            .on_mouse_move(move |event: &MouseMoveEvent, _window, cx: &mut App| {
                if event.dragging() {
                    moved
                        .update(cx, |view, cx| {
                            with_bounds(view, &move_bounds, |view| view.drag_to(index, event, cx))
                        })
                        .ok();
                }
            })
            .on_mouse_up(
                MouseButton::Left,
                move |_event: &MouseUpEvent, _window, cx: &mut App| {
                    up.update(cx, |view, cx| {
                        with_bounds(view, &up_bounds, |view| {
                            view.drag_anchor = None;
                            super::select::mouse_up(view, cx);
                            super::mini::after_mouse_up(view);
                            if view.painter.is_some() {
                                view.paint_format(false, cx);
                            }
                        })
                    })
                    .ok();
                },
            )
            .child(
                super::page_contents(crop, zoom)
                    .child(
                        img(page.clone())
                            .w(px(page_width * zoom))
                            .h(px(page_height * zoom)),
                    )
                    .children(super::grid_overlay(view, index))
                    .children(super::select::overlay(view, index))
                    .children(view.caret_overlay(index))
                    .child(
                        canvas(
                            move |bounds, _window, _cx| {
                                let mut store = store.borrow_mut();
                                if store.len() <= index {
                                    store.resize(index + 1, None);
                                }
                                store[index] = Some(bounds);
                                bounds
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full(),
                    ),
            )
        }))
}

/// The second pane of 分割, over the view's own, while it is on.
pub fn pane(view: &DocumentView, cx: &mut Context<DocumentView>) -> Option<AnyElement> {
    let pane = view.views.split.as_ref()?;
    Some(
        div()
            .flex()
            .flex_col()
            .flex_none()
            .h(gpui::relative(0.45))
            .border_b_4()
            .border_color(theme::border())
            .child(pages(view, pane, cx.weak_entity(), "split-pages").size_full())
            .into_any_element(),
    )
}

/// 新しいウィンドウ: the document in a window of its own.
pub fn open_new_window(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    view.views.windows += 1;
    let number = view.views.windows + 1;
    let doc = cx.entity();
    let name = crate::shell::file_name(std::path::Path::new(view.path.as_ref()));
    let title = SharedString::from(format!("{name}:{number}"));
    // Opened once this update is over: the new window's first frame reads
    // the document.
    cx.defer(move |cx| {
        let options = gpui::WindowOptions {
            titlebar: Some(gpui::TitlebarOptions {
                title: Some(title.clone()),
                ..Default::default()
            }),
            window_bounds: Some(gpui::WindowBounds::Windowed(Bounds::centered(
                None,
                gpui::size(px(1000.0), px(800.0)),
                cx,
            ))),
            ..Default::default()
        };
        let opened = cx.open_window(options, move |window, cx| {
            let mirror = cx.new(|cx| MirrorWindow::new(doc, title, cx));
            window.focus(&mirror.read(cx).focus.clone());
            mirror
        });
        if let Err(error) = opened {
            eprintln!("new window failed: {error}");
        }
    });
}

/// A window over a document another view holds: its pages, scrolled on
/// their own, the clicks, drags and typing going to that view.
pub struct MirrorWindow {
    doc: Entity<DocumentView>,
    title: SharedString,
    pane: Pane,
    focus: FocusHandle,
    _observe: gpui::Subscription,
}

impl MirrorWindow {
    fn new(doc: Entity<DocumentView>, title: SharedString, cx: &mut Context<Self>) -> Self {
        let observe = cx.observe(&doc, |_, _, cx| cx.notify());
        MirrorWindow {
            doc,
            title,
            pane: Pane::default(),
            focus: cx.focus_handle(),
            _observe: observe,
        }
    }
}

impl Focusable for MirrorWindow {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl gpui::Render for MirrorWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let doc = self.doc.clone();
        let weak = doc.downgrade();
        let view = doc.read(cx);
        let pages = pages(view, &self.pane, weak.clone(), "window-pages").size_full();
        let bounds = self.pane.bounds.clone();
        let focus = self.focus.clone();
        let key_doc = weak.clone();
        let key_bounds = self.pane.bounds.clone();
        div()
            .id("mirror-window")
            .track_focus(&self.focus)
            .key_context("Document")
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::window())
            .font_family(crate::UI_FONT_FAMILY)
            .text_color(theme::text())
            // The commands typed here go to the document.
            .on_action({
                let doc = weak.clone();
                move |_: &crate::Undo, _, cx: &mut App| {
                    doc.update(cx, |view, cx| view.step_history(true, cx)).ok();
                }
            })
            .on_action({
                let doc = weak.clone();
                move |_: &crate::Redo, _, cx: &mut App| {
                    doc.update(cx, |view, cx| view.step_history(false, cx)).ok();
                }
            })
            .on_action({
                let doc = weak.clone();
                move |_: &crate::Copy, _, cx: &mut App| {
                    doc.update(cx, |view, cx| view.copy(cx)).ok();
                }
            })
            .on_action({
                let doc = weak.clone();
                move |_: &crate::Cut, _, cx: &mut App| {
                    doc.update(cx, |view, cx| view.cut(cx)).ok();
                }
            })
            .on_action({
                let doc = weak.clone();
                move |_: &crate::Paste, _, cx: &mut App| {
                    doc.update(cx, |view, cx| view.paste(cx)).ok();
                }
            })
            .on_action({
                let doc = weak.clone();
                move |_: &crate::Save, _, cx: &mut App| {
                    doc.update(cx, |view, cx| view.save(cx)).ok();
                }
            })
            .on_action({
                let doc = weak.clone();
                move |_: &crate::Bold, _, cx: &mut App| {
                    doc.update(cx, |view, cx| {
                        view.run_ribbon(
                            crate::ribbon::Action::Toggle(
                                jade_office_model::word::CharacterFormat::Bold,
                            ),
                            cx,
                        )
                    })
                    .ok();
                }
            })
            .on_action({
                let doc = weak.clone();
                move |_: &crate::Italic, _, cx: &mut App| {
                    doc.update(cx, |view, cx| {
                        view.run_ribbon(
                            crate::ribbon::Action::Toggle(
                                jade_office_model::word::CharacterFormat::Italic,
                            ),
                            cx,
                        )
                    })
                    .ok();
                }
            })
            .on_action({
                let doc = weak.clone();
                move |_: &crate::Underline, _, cx: &mut App| {
                    doc.update(cx, |view, cx| {
                        view.run_ribbon(
                            crate::ribbon::Action::Toggle(
                                jade_office_model::word::CharacterFormat::Underline,
                            ),
                            cx,
                        )
                    })
                    .ok();
                }
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx: &mut App| {
                key_doc
                    .update(cx, |view, cx| {
                        with_bounds(view, &key_bounds, |view| view.handle_key(event, cx))
                    })
                    .ok();
            })
            .child(
                div()
                    .flex_none()
                    .px_4()
                    .py_2()
                    .border_b_1()
                    .border_color(theme::border())
                    .text_sm()
                    .child(self.title.clone())
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::text_faint())
                            .child(i18n::text("views.window_hint")),
                    ),
            )
            .child(pages)
            // Typing (and the IME) go to the document, from this window.
            .child(
                canvas(
                    |bounds, _, _| bounds,
                    move |area, _, window, cx| {
                        window.handle_input(
                            &focus,
                            gpui::ElementInputHandler::new(area, doc.clone()),
                            cx,
                        );
                        let _ = &bounds;
                    },
                )
                .absolute()
                .size_0(),
            )
            .on_mouse_down(MouseButton::Left, {
                let focus = self.focus.clone();
                move |_, window, _cx| window.focus(&focus)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 分割: a second pane of the same pages; a click there puts the caret
    /// where that pane drew it.
    #[gpui::test]
    fn a_split_pane_shows_and_takes_clicks(cx: &mut gpui::TestAppContext) {
        let (view, cx) = crate::word_views::tests::open_contract(cx);
        view.update(cx, toggle_split);
        cx.run_until_parked();
        view.update(cx, |view, cx| {
            let pane = view.views.split.clone().expect("split");
            let bounds = pane.bounds.borrow()[0].expect("the pane drew page 1");
            let main = view.page_bounds.borrow()[0].expect("the view drew page 1");
            assert_ne!(bounds.origin, main.origin, "two places for one page");
            let caret = view.caret.unwrap();
            let zoom = view.zoom;
            let event = MouseDownEvent {
                button: MouseButton::Left,
                position: gpui::point(
                    bounds.origin.x + px((caret.x as f32 + 1.0) * zoom),
                    bounds.origin.y + px((caret.document_y + caret.height / 2.0) as f32 * zoom),
                ),
                modifiers: gpui::Modifiers::default(),
                click_count: 1,
                first_mouse: false,
            };
            let before = view.insert_at;
            with_bounds(view, &pane.bounds, |view| view.click_page(0, &event, cx));
            assert_eq!(
                view.insert_at.0, before.0,
                "the same paragraph, through the pane"
            );
            toggle_split(view, cx);
            assert!(view.views.split.is_none());
        });
    }

    /// 新しいウィンドウ opens a window over the same document.
    #[gpui::test]
    fn a_new_window_shows_the_same_document(cx: &mut gpui::TestAppContext) {
        let (view, cx) = crate::word_views::tests::open_contract(cx);
        let windows = cx.update(|_, cx| cx.windows().len());
        view.update(cx, open_new_window);
        cx.run_until_parked();
        let after = cx.update(|_, cx| cx.windows().len());
        assert_eq!(after, windows + 1);
        // Typing in the document reaches the other window's pages, which
        // are the same images.
        view.read_with(cx, |view, _| assert_eq!(view.views.windows, 1));
    }
}

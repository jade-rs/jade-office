//! The small kit the Word dialogs ported from the native app are built
//! from: the modal frame over the window, text fields that take typing
//! (Japanese input included), choice chips, check boxes, labelled rows and
//! the OK / キャンセル row — the native app's `word_dialog` widgets, for the
//! browser.

use gpui::{
    anchored, deferred, div, point, prelude::*, px, AnyElement, Context, ElementId, MouseButton,
    SharedString, Window,
};
use jade_web::text::TextEdit;

use super::DocView as DocumentView;
use crate::i18n;
use crate::palette as theme;

/// A dialog over the window: the backdrop that keeps clicks off the pages,
/// the panel with its title, body, status line and buttons.
pub(crate) fn modal(
    id: &'static str,
    width: f32,
    title: impl Into<SharedString>,
    body: AnyElement,
    status: Option<SharedString>,
    buttons: AnyElement,
    window: &Window,
) -> AnyElement {
    let viewport = window.viewport_size();
    deferred(
        anchored().position(point(px(0.0), px(0.0))).child(
            div()
                .id((id, 0usize))
                .w(viewport.width)
                .h(viewport.height)
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::rgba(0x00000055))
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .id((id, 1usize))
                        .w(px(width))
                        .max_h(viewport.height - px(40.0))
                        .overflow_y_scroll()
                        .p_4()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .rounded_md()
                        .border_1()
                        .border_color(theme::border())
                        .bg(theme::panel())
                        .shadow_lg()
                        .font_family(crate::UI_FONT)
                        .text_xs()
                        .text_color(theme::text())
                        .child(div().text_sm().child(title.into()))
                        .child(body)
                        .children(
                            status
                                .map(|status| div().text_color(theme::text_faint()).child(status)),
                        )
                        .child(buttons),
                ),
        ),
    )
    .with_priority(3)
    .into_any_element()
}

/// キャンセル and OK.
pub(crate) fn ok_cancel(
    id: &'static str,
    cx: &Context<DocumentView>,
    cancel: impl Fn(&mut DocumentView, &mut Context<DocumentView>) + 'static,
    ok: impl Fn(&mut DocumentView, &mut Context<DocumentView>) + 'static,
) -> AnyElement {
    div()
        .flex()
        .justify_end()
        .gap_2()
        .child(button((id, 10usize), i18n::text("format.cancel"), false).on_click(on(cx, cancel)))
        .child(button((id, 11usize), i18n::text("format.ok"), true).on_click(on(cx, ok)))
        .into_any_element()
}

/// A text input: the text with a caret when it has the keyboard.
pub(crate) fn input(
    id: impl Into<ElementId>,
    edit: &TextEdit,
    focused: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .min_h(px(24.0))
        .px_2()
        .rounded_sm()
        .border_1()
        .border_color(if focused {
            theme::accent()
        } else {
            theme::border()
        })
        .bg(theme::canvas())
        .text_color(theme::text())
        .cursor_text()
        .overflow_hidden()
        .child(if focused {
            super::sheet::caret_line(edit, theme::accent().into(), false)
        } else {
            div()
                .whitespace_nowrap()
                .child(edit.text.replace('\n', " ⏎ "))
        })
}

/// A click handler on the view, as `cx.listener` makes one but without
/// holding `cx` borrowed: an element built with it can sit beside others
/// built with `&mut cx`.
pub(crate) fn on<E: ?Sized>(
    cx: &Context<DocumentView>,
    f: impl Fn(&mut DocumentView, &mut Context<DocumentView>) + 'static,
) -> impl Fn(&E, &mut gpui::Window, &mut gpui::App) + 'static {
    let view = cx.weak_entity();
    move |_event, _window, cx| {
        view.update(cx, |view, cx| f(view, cx)).ok();
    }
}

/// [`on`] for a handler that reads the event.
pub(crate) fn on_event<E: ?Sized>(
    cx: &Context<DocumentView>,
    f: impl Fn(&mut DocumentView, &E, &mut gpui::Window, &mut Context<DocumentView>) + 'static,
) -> impl Fn(&E, &mut gpui::Window, &mut gpui::App) + 'static {
    let view = cx.weak_entity();
    move |event, window, cx| {
        view.update(cx, |view, cx| f(view, event, window, cx)).ok();
    }
}

/// A choice among choices: highlighted when chosen.
pub fn chip(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    on: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px_2()
        .py_0p5()
        .rounded_sm()
        .border_1()
        .border_color(if on { theme::accent() } else { theme::border() })
        .when(on, |chip| chip.bg(theme::selected()))
        .cursor_pointer()
        .hover(|chip| chip.bg(theme::hovered()))
        .whitespace_nowrap()
        .child(label.into())
}

/// A check box and its label.
pub fn check(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    on: bool,
) -> gpui::Stateful<gpui::Div> {
    super::word_characters::check(id, label, on)
}

/// A dialog button; `primary` is OK's look.
pub fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    primary: bool,
) -> gpui::Stateful<gpui::Div> {
    super::word_characters::button(id, label, primary).whitespace_nowrap()
}

/// A row: its label, then what is set there.
pub fn labeled(label: impl Into<SharedString>) -> gpui::Div {
    div().flex().flex_wrap().items_center().gap_1().child(
        div()
            .w(px(120.0))
            .flex_none()
            .text_color(theme::text_dim())
            .child(label.into()),
    )
}

/// A section heading inside a dialog.
pub fn section(label: impl Into<SharedString>) -> gpui::Div {
    div()
        .pt_1()
        .text_color(theme::text_faint())
        .child(label.into())
}

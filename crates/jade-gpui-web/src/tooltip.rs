//! A text tooltip, for buttons whose icon is not the whole story — the
//! native app's (`jade-gpui-app/src/tooltip.rs`) in this shell's palette.

use gpui::{div, prelude::*, px, AnyView, App, Context, SharedString, Window};

use crate::{palette, UI_FONT};

pub struct Tooltip {
    text: SharedString,
}

impl Tooltip {
    /// A builder for `.tooltip(...)` that shows `text`.
    pub fn text(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView {
        let text = text.into();
        move |_window, cx| {
            let text = text.clone();
            cx.new(|_| Tooltip { text }).into()
        }
    }
}

impl Render for Tooltip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .font_family(UI_FONT)
            .mt(px(4.0))
            .px_2()
            .py_1()
            .rounded_md()
            .bg(palette::panel())
            .border_1()
            .border_color(palette::border())
            .shadow_md()
            .text_xs()
            .text_color(palette::text())
            .child(self.text.clone())
    }
}

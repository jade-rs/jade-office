//! A text tooltip, for buttons whose glyph or icon is not the whole story.

use gpui::{div, prelude::*, px, AnyView, App, Context, SharedString, Window};

use crate::{theme, UI_FONT_FAMILY};

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
            .font_family(UI_FONT_FAMILY)
            .mt(px(4.0))
            .px_2()
            .py_1()
            .rounded_md()
            .bg(theme::panel())
            .border_1()
            .border_color(theme::border())
            .shadow_md()
            .text_xs()
            .text_color(theme::text())
            .child(self.text.clone())
    }
}

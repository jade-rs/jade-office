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

#[cfg(test)]
mod tests {
    use super::Tooltip;

    /// A ribbon button's ScreenTip: the builder `.tooltip(...)` takes opens
    /// a view that shows the tip's text, in the UI font.
    #[gpui::test]
    fn a_screen_tip_shows_its_text(cx: &mut gpui::TestAppContext) {
        let window = cx.add_empty_window();
        let tip = crate::i18n::text("doc.indent_more_title");
        assert_ne!(tip, "doc.indent_more_title", "the tip is translated");
        let view = window.update(|window, cx| Tooltip::text(tip)(window, cx));
        let shown = window.update(|_, cx| {
            view.downcast::<Tooltip>()
                .expect("a Tooltip view")
                .read(cx)
                .text
                .clone()
        });
        assert_eq!(shown.as_ref(), tip);
    }
}

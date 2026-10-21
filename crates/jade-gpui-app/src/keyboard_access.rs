//! The ribbon from the keyboard: every control is a tab stop with a focus
//! ring; F6 moves the keyboard from the document to the ribbon and back;
//! Tab, Shift-Tab and the arrows go from control to control; Enter or Space
//! does what a click does; Escape closes an open list, then goes back to
//! the document. (Word for Mac reaches its ribbon the same way under the
//! system's 全キーボード操作.)

use gpui::{prelude::*, Context, KeyDownEvent, Window};

use crate::DocumentView;

/// Makes `element` a tab stop whose Enter or Space runs `run`, as a click
/// on it does.
pub fn activatable(
    element: gpui::Stateful<gpui::Div>,
    cx: &mut Context<DocumentView>,
    run: impl Fn(&mut DocumentView, &mut Context<DocumentView>) + 'static,
) -> gpui::Stateful<gpui::Div> {
    let run = std::rc::Rc::new(run);
    let click = run.clone();
    element
        .tab_index(0)
        .focus(crate::ribbon::focus_ring)
        .on_click(
            cx.listener(move |view, event: &gpui::ClickEvent, window, cx| {
                click(view, cx);
                // A click leaves the keyboard with the text.
                if !event.is_keyboard() && view.wants_text_focus() {
                    window.focus(&view.focus);
                }
            }),
        )
        .on_key_down(cx.listener(move |view, event: &KeyDownEvent, _window, cx| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space")
                && !event.keystroke.modifiers.modified()
            {
                run(view, cx);
                cx.stop_propagation();
            }
        }))
}

impl DocumentView {
    /// Whether the keyboard belongs with the text after a ribbon click: not
    /// while a list is open over the ribbon, whose entries are next.
    pub(crate) fn wants_text_focus(&self) -> bool {
        self.ribbon.popup.is_none()
    }
}

/// Keys the ribbon takes while one of its controls has the keyboard.
pub fn ribbon_key(
    view: &mut DocumentView,
    event: &KeyDownEvent,
    window: &mut Window,
    cx: &mut Context<DocumentView>,
) {
    let modifiers = event.keystroke.modifiers;
    match event.keystroke.key.as_str() {
        "tab" if modifiers.shift => window.focus_prev(),
        "tab" | "right" | "down" => window.focus_next(),
        "left" | "up" => window.focus_prev(),
        "escape" => {
            if view.ribbon.popup.take().is_none() {
                window.focus(&view.focus);
            }
            cx.notify();
        }
        _ => return,
    }
    cx.stop_propagation();
}

/// F6: the keyboard to the ribbon's first control, or back to the document.
pub fn toggle_ribbon_focus(
    view: &mut DocumentView,
    window: &mut Window,
    cx: &mut Context<DocumentView>,
) {
    if view.focus.is_focused(window) {
        window.focus_next();
    } else {
        window.focus(&view.focus);
    }
    cx.notify();
}

#[cfg(test)]
mod tests {
    /// F6 puts the keyboard on the ribbon; the arrows move along its tabs
    /// and Enter opens one, as a click does; Escape goes back to the text,
    /// where typing goes on as before.
    #[gpui::test]
    fn the_ribbon_is_worked_from_the_keyboard(cx: &mut gpui::TestAppContext) {
        use gpui::Focusable;
        let bytes = jade_office_model::word::new::blank_document(
            jade_office_model::word::new::Blank::English,
        );
        let (view, cx) = cx
            .add_window_view(|_window, cx| crate::untitled_view(&bytes, "keys.docx", cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        view.update(cx, |view, _| {
            view.insert_at = (0, 0);
            view.caret = view.document.place_caret(0, 0);
        });
        // F6 (bound in the app's keymap).
        cx.dispatch_action(crate::FocusRibbon);
        cx.update(|window, cx| {
            assert!(!view.focus_handle(cx).is_focused(window), "on the ribbon");
        });
        // ホーム, then 挿入: the second tab.
        cx.simulate_keystrokes("right enter");
        view.read_with(cx, |view, _| {
            assert!(view.ribbon_tab == crate::RibbonTab::Insert);
        });
        cx.simulate_keystrokes("escape");
        cx.update(|window, cx| {
            assert!(view.focus_handle(cx).is_focused(window), "back on the text");
        });
        cx.simulate_input("typed");
        view.read_with(cx, |view, _| {
            assert!(view.document.paragraph_text(0).unwrap().contains("typed"));
        });
    }
}

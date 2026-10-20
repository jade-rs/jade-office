//! Home ▸ Conditional Formatting: the toolbar's menu, the dialog a
//! highlight or top/bottom rule asks its value through, and the Rules
//! Manager listing the sheet's rules to delete.

use gpui::{div, prelude::*, px, Context, Modifiers, MouseButton};
use jade_office_model::conditional_formatting::{
    describe, AskKey, ConditionalStyle, Preset, RuleAsk, RuleDialog, Visual, COLOR_SCALES,
    DATA_BARS, STYLES,
};

use super::{caret_line, Tool, WorkbookView, SHEET_TOOLBAR_HEIGHT};
use crate::{i18n, theme};

/// What the menu's items do.
#[derive(Clone, Copy)]
pub(super) enum Conditional {
    /// A rule that asks for its value first.
    Ask(Preset),
    /// A data bar or colour scale, applied at once.
    Apply(Visual),
    ClearSelected,
    ClearSheet,
    Manage,
}

/// The words for `key` with its named values.
fn words(key: &'static str, values: &[(&'static str, String)]) -> String {
    let values: Vec<(&str, &dyn std::fmt::Display)> = values
        .iter()
        .map(|(name, value)| (*name, value as &dyn std::fmt::Display))
        .collect();
    i18n::format(key, &values)
}

impl WorkbookView {
    pub(super) fn conditional(&mut self, tool: Conditional, cx: &mut Context<Self>) {
        let range = self.range();
        let result = match tool {
            Conditional::Ask(preset) => {
                let dialog = RuleDialog::open(preset, &self.book, range, self.active());
                self.asking = Some(RuleAsk::new(dialog));
                cx.notify();
                return;
            }
            Conditional::Manage => {
                self.managing = true;
                cx.notify();
                return;
            }
            Conditional::Apply(visual) => {
                self.book
                    .add_conditional_format(range, &visual.rule(), ConditionalStyle::None)
            }
            Conditional::ClearSelected => self.book.clear_conditional_formats(Some(range)),
            Conditional::ClearSheet => self.book.clear_conditional_formats(None),
        };
        self.after_write(result, cx);
    }

    /// The dialog's OK: the rule as typed, or what is wrong with it.
    pub(super) fn take_rule(&mut self, cx: &mut Context<Self>) {
        let Some(ask) = self.asking.as_mut() else {
            return;
        };
        match ask.rule() {
            Some(rule) => {
                let (range, style) = (ask.dialog.range, ask.dialog.style);
                self.asking = None;
                let result = self.book.add_conditional_format(range, &rule, style);
                self.after_write(result, cx);
            }
            None => cx.notify(),
        }
    }

    /// Keys while the dialog is open.
    pub(super) fn rule_key(
        &mut self,
        key: &str,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) -> bool {
        let command = modifiers.platform || modifiers.control;
        let Some(ask) = self.asking.as_mut() else {
            return false;
        };
        match ask.key(key, modifiers.shift, command) {
            AskKey::Take => {
                self.take_rule(cx);
                return true;
            }
            AskKey::Close => self.asking = None,
            AskKey::Handled => {}
            AskKey::Unhandled => return false,
        }
        cx.notify();
        true
    }

    /// The Conditional Formatting dropdown under its toolbar button.
    pub(super) fn conditional_menu(&self, cx: &mut Context<Self>) -> gpui::Div {
        let heading = |key: &'static str| {
            div()
                .px_3()
                .pt_2()
                .pb_1()
                .text_color(theme::text_dim())
                .child(i18n::text(key))
        };
        let item = |label: &'static str, tool: Conditional, cx: &mut Context<Self>| {
            div()
                .id(label)
                .px_3()
                .py_1()
                .cursor_pointer()
                .hover(|item| item.bg(theme::hovered()))
                .child(i18n::text(label))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |view, _event, _window, cx| {
                    view.tool(Tool::Conditional(tool), cx)
                }))
        };
        let swatches =
            |prefix: &'static str, visuals: &'static [Visual], cx: &mut Context<Self>| {
                let mut row = div().flex().gap_1().px_3().py_1();
                for (index, visual) in visuals.iter().copied().enumerate() {
                    row = row.child(
                        div()
                            .id((prefix, index))
                            .w(px(22.0))
                            .h(px(18.0))
                            .flex()
                            .rounded_sm()
                            .border_1()
                            .border_color(theme::border())
                            .overflow_hidden()
                            .cursor_pointer()
                            .children(
                                visual
                                    .colors
                                    .iter()
                                    .rev()
                                    .map(|color| div().flex_1().h_full().bg(gpui::rgb(*color))),
                            )
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(move |view, _event, _window, cx| {
                                view.tool(Tool::Conditional(Conditional::Apply(visual)), cx)
                            })),
                    );
                }
                row
            };
        let mut menu = div().w(px(240.0)).py_1();
        for (title, presets) in [
            ("cf.highlight", Preset::HIGHLIGHT),
            ("cf.top_bottom", Preset::TOP_BOTTOM),
        ] {
            menu = menu.child(heading(title));
            for preset in presets {
                menu = menu.child(item(preset.label_key(), Conditional::Ask(preset), cx));
            }
        }
        menu = menu
            .child(heading("cf.data_bars"))
            .child(swatches("cf-bar", &DATA_BARS, cx))
            .child(heading("cf.color_scales"))
            .child(swatches("cf-scale", &COLOR_SCALES, cx))
            .child(div().h(px(1.0)).my_1().bg(theme::border()));
        for (label, tool) in [
            ("cf.clear_selected", Conditional::ClearSelected),
            ("cf.clear_sheet", Conditional::ClearSheet),
            ("cf.manage", Conditional::Manage),
        ] {
            menu = menu.child(item(label, tool, cx));
        }
        menu
    }

    /// A dialog's panel, with its title.
    pub(super) fn dialog_panel(&self, title: String, width: f32) -> gpui::Div {
        div()
            .w(px(width))
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(theme::border())
            .bg(theme::panel())
            .shadow_lg()
            .text_xs()
            .text_color(theme::text())
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(div().text_sm().child(title))
    }

    pub(super) fn dialog_button(
        &self,
        id: &'static str,
        label: &'static str,
        primary: bool,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        div()
            .id(id)
            .px_3()
            .py_1()
            .rounded_sm()
            .border_1()
            .border_color(theme::border())
            .cursor_pointer()
            .when(primary, |button| {
                button.bg(theme::accent()).text_color(gpui::white())
            })
            .hover(|button| button.opacity(0.85))
            .child(i18n::text(label))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |view, _event, _window, cx| action(view, cx)))
    }

    /// The dialog a highlight or top/bottom rule asks through: its value
    /// fields, the look to give the cells, OK and Cancel.
    pub(super) fn rule_dialog(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let ask = self.asking.as_ref()?;
        let preset = ask.dialog.preset;
        let fields = ask.fields.iter().enumerate().map(|(index, text)| {
            let focused = index == ask.field;
            div()
                .id(("cf-field", index))
                .flex_1()
                .px_2()
                .py_1()
                .rounded_sm()
                .border_1()
                .border_color(if focused {
                    theme::accent()
                } else {
                    theme::border()
                })
                .bg(theme::canvas())
                .cursor_text()
                .child(if focused {
                    caret_line(text, theme::accent().into(), false)
                } else {
                    div().child(text.text.clone())
                })
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |view, _event, _window, cx| {
                    if let Some(ask) = view.asking.as_mut() {
                        ask.field = index;
                    }
                    cx.notify();
                }))
        });
        // "between [10] and [30]": the word sits between the fields.
        let mut row = div().flex().items_center().gap_2();
        for (index, field) in fields.enumerate() {
            if index > 0 {
                row = row.child(i18n::text("cf.and"));
            }
            row = row.child(field);
        }
        let styles = STYLES.iter().enumerate().map(|(index, (style, key))| {
            let style = *style;
            let chosen = style == ask.dialog.style;
            div()
                .id(("cf-style", index))
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .py_0p5()
                .rounded_sm()
                .cursor_pointer()
                .when(chosen, |row| row.bg(theme::selected()))
                .hover(|row| row.bg(theme::hovered()))
                .child(div().w(px(10.0)).child(if chosen { "✓" } else { "" }))
                .child(i18n::text(key))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |view, _event, _window, cx| {
                    if let Some(ask) = view.asking.as_mut() {
                        ask.dialog.style = style;
                    }
                    cx.notify();
                }))
        });
        let panel = self.dialog_panel(i18n::text(preset.label_key()).to_string(), 360.0);
        let error = ask
            .error
            .map(|key| div().text_color(gpui::rgb(0xc42b1c)).child(i18n::text(key)));
        Some(centred(
            panel
                .child(i18n::text(preset.prompt_key()))
                .when(preset.fields() > 0, |panel| panel.child(row))
                .children(error)
                .child(
                    div()
                        .text_color(theme::text_dim())
                        .child(i18n::text("cf.with")),
                )
                .child(div().flex().flex_col().children(styles))
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child(self.dialog_button(
                            "cf-cancel",
                            "cf.cancel",
                            false,
                            cx,
                            |view, cx| {
                                view.asking = None;
                                cx.notify();
                            },
                        ))
                        .child(self.dialog_button("cf-ok", "cf.ok", true, cx, |view, cx| {
                            view.take_rule(cx)
                        })),
                ),
        ))
    }

    /// The Rules Manager: the sheet's rules, first the one that wins, each
    /// with where it applies and a Delete; Close.
    pub(super) fn rules_manager(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        if !self.managing {
            return None;
        }
        let rules = self.book.conditional_formats();
        let rows = rules.iter().enumerate().map(|(index, rule)| {
            let words = describe(rule);
            div()
                .flex()
                .items_center()
                .gap_2()
                .py_0p5()
                .border_b_1()
                .border_color(theme::border())
                .child(div().flex_1().child(self::words(words.key, &words.values)))
                .child(
                    div()
                        .w(px(90.0))
                        .text_color(theme::text_dim())
                        .child(rule.range.clone()),
                )
                .child(
                    div()
                        .id(("cf-delete", index))
                        .px_2()
                        .rounded_sm()
                        .cursor_pointer()
                        .hover(|button| button.bg(theme::hovered()))
                        .child(i18n::text("cf.delete"))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |view, _event, _window, cx| {
                            let result = view.book.delete_conditional_format(index);
                            view.after_write(result, cx);
                        })),
                )
        });
        let empty = rules.is_empty().then(|| {
            div()
                .text_color(theme::text_dim())
                .child(i18n::text("cf.no_rules"))
        });
        let panel = self.dialog_panel(i18n::text("cf.manage_title").to_string(), 420.0);
        Some(centred(
            panel
                .child(div().flex().flex_col().children(rows).children(empty))
                .child(div().flex().justify_end().child(self.dialog_button(
                    "cf-close",
                    "cf.close",
                    true,
                    cx,
                    |view, cx| {
                        view.managing = false;
                        cx.notify();
                    },
                ))),
        ))
    }
}

/// `panel` centred over the grid, as Excel's dialogs sit.
pub(super) fn centred(panel: gpui::Div) -> gpui::Div {
    div()
        .absolute()
        .top(px(SHEET_TOOLBAR_HEIGHT + 60.0))
        .left_0()
        .right_0()
        .flex()
        .justify_center()
        .child(panel)
}

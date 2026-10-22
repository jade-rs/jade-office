//! レイアウト ▸ ハイフネーション in the browser build: なし, 自動 and
//! ハイフネーションのオプション…, over the same model as the native app
//! (`jade-office-model`'s `word::hyphenation`).

use gpui::{div, px, AnyElement, Context, KeyDownEvent, MouseButton};
use jade_web::word::hyphenation::{HyphenationChoice, HyphenationDialog, WordHyphenation};

use super::*;
use crate::ribbon::{dropdown, group, listener, menu_panel, row, Action, Popup};

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

/// The ハイフネーション group: its button and list. `current` is what the
/// list ticks, read while レイアウト shows.
pub fn layout_group(
    open: Option<Popup>,
    current: Option<HyphenationChoice>,
    cx: &mut Context<DocView>,
) -> AnyElement {
    let is_open = open == Some(Popup::Hyphenation);
    let trigger = div()
        .id("hyphenation-button")
        .flex()
        .flex_none()
        .items_center()
        .gap_1()
        .h(px(26.0))
        .px_2()
        .rounded_md()
        .text_xs()
        .text_color(palette::text())
        .whitespace_nowrap()
        .cursor_pointer()
        .when(is_open, |button| button.bg(palette::selected()))
        .hover(|button| button.bg(palette::hovered()))
        .on_click(listener(Action::Popup(Popup::Hyphenation), cx))
        .child(t("hyphenation.button"))
        .child(
            gpui::svg()
                .path("icons/chevron-down.svg")
                .size_3()
                .text_color(palette::dim()),
        )
        .into_any_element();
    let menu = is_open.then(|| {
        menu_panel("hyphenation-menu")
            .w(px(240.0))
            .children(HyphenationChoice::ALL.into_iter().map(|choice| {
                let checked = current == Some(choice);
                div()
                    .id(choice.label_key())
                    .flex()
                    .items_center()
                    .gap_2()
                    .h(px(26.0))
                    .pl_1()
                    .pr_3()
                    .rounded_md()
                    .whitespace_nowrap()
                    .cursor_pointer()
                    .hover(|row| row.bg(palette::hovered()))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if let Document::Pages { ribbon, .. } = &mut view.document {
                            ribbon.popup = None;
                        }
                        view.choose_hyphenation(choice, cx);
                    }))
                    .child(div().size_4().flex_none().when(checked, |cell| {
                        cell.child(
                            gpui::svg()
                                .path("icons/check.svg")
                                .size_4()
                                .text_color(palette::accent()),
                        )
                    }))
                    .child(t(choice.label_key()))
                    .into_any_element()
            }))
            .into_any_element()
    });
    group(
        t("hyphenation.group"),
        vec![row(vec![dropdown(trigger, menu)])],
    )
}

/// What the list ticks.
pub fn current_choice(document: &jade_web::editing::EditableDocument) -> HyphenationChoice {
    document.hyphenation_choice()
}

/// A number the dialog steps, as it shows it.
fn stepped(text: &str, step: f32, up: bool, decimals: bool) -> String {
    let value = text.trim().parse::<f32>().unwrap_or(0.0);
    let next = (if up { value + step } else { value - step }).max(0.0);
    if decimals {
        let text = format!("{next:.2}");
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    } else if next == 0.0 {
        String::new()
    } else {
        format!("{}", next.round() as u32)
    }
}

impl DocView {
    /// A choice from the ハイフネーション list.
    pub(super) fn choose_hyphenation(&mut self, choice: HyphenationChoice, cx: &mut Context<Self>) {
        match choice {
            HyphenationChoice::Off => {
                self.word_edit(cx, |document| document.set_auto_hyphenation(false))
            }
            HyphenationChoice::Automatic => {
                self.word_edit(cx, |document| document.set_auto_hyphenation(true))
            }
            HyphenationChoice::Options => {
                if let Document::Pages { document, .. } = &self.document {
                    self.hyphenation_dialog = Some((document.hyphenation_dialog(), None));
                    cx.notify();
                }
            }
        }
    }

    fn edit_hyphenation(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut HyphenationDialog),
    ) {
        if let Some((dialog, problem)) = self.hyphenation_dialog.as_mut() {
            edit(dialog);
            *problem = None;
            cx.notify();
        }
    }

    /// OK: the settings as one undo step; a value it cannot take keeps the
    /// dialog open with why.
    fn apply_hyphenation_dialog(&mut self, cx: &mut Context<Self>) {
        let Some((dialog, _)) = self.hyphenation_dialog.take() else {
            return;
        };
        match dialog.patch() {
            Ok(_) => self.word_edit(cx, move |document| {
                document.apply_hyphenation_dialog(&dialog)
            }),
            Err(problem) => {
                self.hyphenation_dialog = Some((dialog, Some(problem)));
                cx.notify();
            }
        }
    }

    /// Keys while the dialog is open: Escape is キャンセル, Enter OK.
    pub(crate) fn hyphenation_dialog_key(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        match event.keystroke.key.as_str() {
            "escape" => {
                self.hyphenation_dialog = None;
                cx.notify();
            }
            "enter" | "return" => self.apply_hyphenation_dialog(cx),
            _ => {}
        }
        true
    }

    /// The ハイフネーション dialog, while it is open.
    pub(crate) fn hyphenation_dialog_element(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (dialog, problem) = self.hyphenation_dialog.as_ref()?;
        let check =
            |id: &'static str, key: &'static str, on: bool, auto: bool, cx: &mut Context<Self>| {
                div()
                    .id(id)
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .cursor_pointer()
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.edit_hyphenation(cx, |dialog| {
                            if auto {
                                dialog.auto = !dialog.auto;
                            } else {
                                dialog.hyphenate_caps = !dialog.hyphenate_caps;
                            }
                        })
                    }))
                    .child(
                        div()
                            .size(px(14.0))
                            .rounded_sm()
                            .border_1()
                            .border_color(if on {
                                palette::accent()
                            } else {
                                palette::border()
                            })
                            .when(on, |square| square.bg(palette::accent())),
                    )
                    .child(t(key))
            };
        let stepper = |id: &'static str,
                       label: &'static str,
                       value: String,
                       unit: &'static str,
                       zone: bool,
                       cx: &mut Context<Self>| {
            let step = move |up: bool| {
                move |view: &mut DocView,
                      _: &gpui::ClickEvent,
                      _: &mut gpui::Window,
                      cx: &mut Context<DocView>| {
                    view.edit_hyphenation(cx, |dialog| {
                        if zone {
                            dialog.zone = stepped(&dialog.zone, 0.5, up, true);
                        } else {
                            dialog.limit = stepped(&dialog.limit, 1.0, up, false);
                        }
                    })
                }
            };
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    div()
                        .w(px(150.0))
                        .text_color(palette::dim())
                        .child(t(label)),
                )
                .child(
                    div()
                        .id((id, 0usize))
                        .px_1()
                        .rounded_sm()
                        .border_1()
                        .border_color(palette::border())
                        .cursor_pointer()
                        .on_click(cx.listener(step(false)))
                        .child("−"),
                )
                .child(div().w(px(56.0)).text_center().child(if value.is_empty() {
                    t("hyphenation.unlimited").to_string()
                } else {
                    value
                }))
                .child(
                    div()
                        .id((id, 1usize))
                        .px_1()
                        .rounded_sm()
                        .border_1()
                        .border_color(palette::border())
                        .cursor_pointer()
                        .on_click(cx.listener(step(true)))
                        .child("+"),
                )
                .child(t(unit))
        };
        let body = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(check(
                "hyphenation-auto",
                "hyphenation.auto",
                dialog.auto,
                true,
                cx,
            ))
            .child(check(
                "hyphenation-caps",
                "hyphenation.caps",
                dialog.hyphenate_caps,
                false,
                cx,
            ))
            .child(stepper(
                "hyphenation-zone",
                "hyphenation.zone",
                dialog.zone.clone(),
                "setup.unit.mm",
                true,
                cx,
            ))
            .child(stepper(
                "hyphenation-limit",
                "hyphenation.limit",
                dialog.limit.clone(),
                "setup.unit.none",
                false,
                cx,
            ));
        Some(
            div()
                .id("hyphenation-dialog-backdrop")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::rgba(0x00000055))
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .id("hyphenation-dialog")
                        .w(px(420.0))
                        .p_4()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .rounded_md()
                        .border_1()
                        .border_color(palette::border())
                        .bg(palette::panel())
                        .shadow_lg()
                        .text_xs()
                        .text_color(palette::text())
                        .child(div().text_sm().child(t("hyphenation.title")))
                        .child(body)
                        .children(
                            problem.map(|problem| {
                                div().text_color(gpui::rgb(0xD13438)).child(t(problem))
                            }),
                        )
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .gap_2()
                                .child(
                                    div()
                                        .id("hyphenation-cancel")
                                        .px_3()
                                        .py_1()
                                        .rounded_md()
                                        .border_1()
                                        .border_color(palette::border())
                                        .cursor_pointer()
                                        .on_click(cx.listener(|view, _, _, cx| {
                                            view.hyphenation_dialog = None;
                                            cx.notify();
                                        }))
                                        .child(t("format.cancel")),
                                )
                                .child(
                                    div()
                                        .id("hyphenation-ok")
                                        .px_3()
                                        .py_1()
                                        .rounded_md()
                                        .border_1()
                                        .border_color(palette::accent())
                                        .text_color(palette::accent())
                                        .cursor_pointer()
                                        .on_click(cx.listener(|view, _, _, cx| {
                                            view.apply_hyphenation_dialog(cx)
                                        }))
                                        .child(t("format.ok")),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }
}

//! ページの背景 › 透かし in the browser build: the gallery, 透かしの削除 and
//! the ユーザー設定の透かし dialog — the browser's copy of the native app's
//! `word_watermark`, over the same model (`jade-office-model`'s
//! `word::watermark`).

use gpui::{div, prelude::*, px, AnyElement, ClickEvent, Context};
use jade_web::word::watermark::{
    WatermarkDialog, WatermarkLanguage, WordWatermark, WATERMARK_COLORS, WATERMARK_FONTS,
    WATERMARK_PRESETS, WATERMARK_SIZES,
};

use super::word_dialog::{check, chip, labeled, on, DialogError, Inputs, WordDialog};
use super::{DocView as DocumentView, Document};
use crate::i18n;
use crate::palette as theme;
use crate::ribbon::{dropdown, listener, menu_panel, Action, Popup};
use crate::tooltip::Tooltip;

/// The dialog's state: the model's dialog and its two text fields (the
/// テキスト, then the サイズ).
pub struct WatermarkDialogState {
    pub dialog: WatermarkDialog,
    pub inputs: Inputs,
}

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

fn hex(color: &str) -> u32 {
    u32::from_str_radix(color.trim_start_matches('#'), 16).unwrap_or(0xC0C0C0)
}

/// The 透かし button and its gallery, for ページの背景. `current` is the
/// gallery entry the document's watermark is.
pub fn watermark_button(
    popup: Option<Popup>,
    current: Option<usize>,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    let open = popup == Some(Popup::Watermark);
    let trigger = div()
        .id("watermark-button")
        .flex()
        .flex_none()
        .items_center()
        .gap_1()
        .h(px(26.0))
        .px_2()
        .rounded_md()
        .text_xs()
        .text_color(theme::text())
        .whitespace_nowrap()
        .cursor_pointer()
        .when(open, |button| button.bg(theme::selected()))
        .hover(|button| button.bg(theme::hovered()))
        .tooltip(Tooltip::text(t("watermark.button")))
        .on_click(listener(Action::Popup(Popup::Watermark), cx))
        .child(t("watermark.button"))
        .child(
            gpui::svg()
                .path("icons/chevron-down.svg")
                .size_3()
                .text_color(theme::dim()),
        )
        .into_any_element();
    let menu = open.then(|| {
        let tiles = WATERMARK_PRESETS
            .iter()
            .enumerate()
            .map(|(index, preset)| {
                let label = div()
                    .text_color(gpui::rgb(0xA0A0A0))
                    .text_sm()
                    .child(preset.text);
                div()
                    .id(("watermark-preset", index))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_between()
                    .w(px(86.0))
                    .h(px(96.0))
                    .p_1()
                    .rounded_sm()
                    .border_1()
                    .border_color(if current == Some(index) {
                        theme::accent()
                    } else {
                        theme::border()
                    })
                    .cursor_pointer()
                    .hover(|tile| tile.bg(theme::hovered()))
                    .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                        view.close_popup();
                        let result = view.doc_mut().insert_watermark_preset(index);
                        view.tool_done(result, cx);
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .items_center()
                            .justify_center()
                            .w_full()
                            .bg(gpui::rgb(0xFFFFFF))
                            .child(if preset.horizontal {
                                label.into_any_element()
                            } else {
                                // A page's diagonal, drawn as a slant.
                                label.italic().into_any_element()
                            }),
                    )
                    .child(div().text_xs().child(preset.caption()))
            })
            .collect::<Vec<_>>();
        let row = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .flex()
                .items_center()
                .h(px(26.0))
                .px_2()
                .rounded_md()
                .whitespace_nowrap()
                .cursor_pointer()
                .hover(|row| row.bg(theme::hovered()))
                .child(t(label))
        };
        menu_panel("watermark-menu")
            .w(px(380.0))
            .child(div().flex().flex_wrap().gap_1().p_1().children(tiles))
            .child(
                row("watermark-custom", "watermark.custom").on_click(cx.listener(
                    |view, _: &ClickEvent, _, cx| {
                        view.close_popup();
                        view.open_watermark_dialog(cx);
                    },
                )),
            )
            .child(
                row("watermark-remove", "watermark.remove").on_click(cx.listener(
                    |view, _: &ClickEvent, _, cx| {
                        view.close_popup();
                        let result = view.doc_mut().remove_watermark();
                        view.tool_done(result, cx);
                    },
                )),
            )
            .into_any_element()
    });
    dropdown(trigger, menu)
}

impl DocumentView {
    fn close_popup(&mut self) {
        if let Document::Pages { ribbon, .. } = &mut self.document {
            ribbon.popup = None;
        }
    }

    /// ユーザー設定の透かし…, on the document's watermark.
    pub(crate) fn open_watermark_dialog(&mut self, cx: &mut Context<Self>) {
        let dialog = self.doc().watermark_dialog();
        let inputs = Inputs::new(vec![
            dialog.text.clone(),
            dialog
                .size_label()
                .unwrap_or_else(|| t("watermark.size.auto").to_string()),
        ]);
        self.tools.dialog = Some(WordDialog::Watermark(WatermarkDialogState {
            dialog,
            inputs,
        }));
        cx.notify();
    }

    fn edit_watermark(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut WatermarkDialogState),
    ) {
        self.edit_dialog(cx, |dialog| {
            if let WordDialog::Watermark(state) = dialog {
                edit(state);
            }
        })
    }

    pub(crate) fn watermark_dialog_body(
        &self,
        state: &WatermarkDialogState,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let dialog = &state.dialog;
        let enabled = dialog.text_on;
        let kinds = [(false, "watermark.none"), (true, "watermark.text")]
            .into_iter()
            .enumerate()
            .map(|(index, (on_text, label))| {
                chip(
                    ("watermark-kind", index),
                    t(label),
                    dialog.text_on == on_text,
                )
                .on_click(on(cx, move |view, cx| {
                    view.edit_watermark(cx, |state| state.dialog.text_on = on_text)
                }))
            })
            .collect::<Vec<_>>();
        let languages = WatermarkLanguage::ALL
            .into_iter()
            .enumerate()
            .map(|(index, language)| {
                chip(
                    ("watermark-language", index),
                    t(language.label_key()),
                    dialog.language == language,
                )
                .on_click(on(cx, move |view, cx| {
                    view.edit_watermark(cx, |state| {
                        state.dialog.text = state.inputs.text(0).to_string();
                        state.dialog.set_language(language);
                        state.inputs.set(0, state.dialog.text.clone());
                    })
                }))
            })
            .collect::<Vec<_>>();
        let texts = dialog
            .language
            .texts()
            .iter()
            .enumerate()
            .map(|(index, text)| {
                chip(
                    ("watermark-text", index),
                    *text,
                    state.inputs.text(0) == *text,
                )
                .on_click(on(cx, move |view, cx| {
                    view.edit_watermark(cx, |state| {
                        state.dialog.text_on = true;
                        state.dialog.text = text.to_string();
                        state.inputs.set(0, text.to_string());
                    })
                }))
            })
            .collect::<Vec<_>>();
        let fonts = WATERMARK_FONTS
            .into_iter()
            .enumerate()
            .map(|(index, font)| {
                chip(("watermark-font", index), font, dialog.font == font).on_click(on(
                    cx,
                    move |view, cx| {
                        view.edit_watermark(cx, |state| state.dialog.font = font.into())
                    },
                ))
            })
            .collect::<Vec<_>>();
        let sizes = std::iter::once(None)
            .chain(WATERMARK_SIZES.into_iter().map(Some))
            .enumerate()
            .map(|(index, size)| {
                let label = size
                    .map(|size| size.to_string())
                    .unwrap_or_else(|| t("watermark.size.auto").to_string());
                let typed = state.inputs.text(1).to_string();
                chip(("watermark-size", index), label.clone(), typed == label).on_click(on(
                    cx,
                    move |view, cx| {
                        let label = label.clone();
                        view.edit_watermark(cx, |state| {
                            state.dialog.size = size.map(|size| size as f32);
                            state.inputs.set(1, label);
                        })
                    },
                ))
            })
            .collect::<Vec<_>>();
        let colors = WATERMARK_COLORS
            .into_iter()
            .enumerate()
            .map(|(index, (color, label))| {
                div()
                    .id(("watermark-color", index))
                    .size(px(18.0))
                    .rounded_sm()
                    .border_1()
                    .border_color(if dialog.color.eq_ignore_ascii_case(color) {
                        theme::accent()
                    } else {
                        theme::border()
                    })
                    .bg(gpui::rgb(hex(color)))
                    .cursor_pointer()
                    .tooltip(Tooltip::text(t(label)))
                    .on_click(on(cx, move |view, cx| {
                        view.edit_watermark(cx, |state| state.dialog.color = color.to_string())
                    }))
            })
            .collect::<Vec<_>>();
        let layouts = [
            (false, "watermark.diagonal"),
            (true, "watermark.horizontal"),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (horizontal, label))| {
            chip(
                ("watermark-layout", index),
                t(label),
                dialog.horizontal == horizontal,
            )
            .on_click(on(cx, move |view, cx| {
                view.edit_watermark(cx, |state| state.dialog.horizontal = horizontal)
            }))
        })
        .collect::<Vec<_>>();
        let semitransparent = check(
            "watermark-semitransparent",
            t("watermark.semitransparent"),
            dialog.semitransparent,
        )
        .on_click(on(cx, |view, cx| {
            view.edit_watermark(cx, |state| {
                state.dialog.semitransparent = !state.dialog.semitransparent
            })
        }));
        let sample = div()
            .flex()
            .items_center()
            .justify_center()
            .h(px(64.0))
            .rounded_sm()
            .border_1()
            .border_color(theme::border())
            .bg(gpui::rgb(0xFFFFFF))
            .when(enabled, |preview| {
                preview.child(
                    div()
                        .text_xl()
                        .when(!dialog.horizontal, |text| text.italic())
                        .text_color(gpui::rgba(
                            (hex(&dialog.color) << 8)
                                | if dialog.semitransparent { 0x80 } else { 0xFF },
                        ))
                        .child(state.inputs.text(0).to_string()),
                )
            });
        let options = div()
            .flex()
            .flex_col()
            .gap_2()
            .when(!enabled, |options| options.opacity(0.5))
            .child(labeled(t("watermark.language")).children(languages))
            .child(
                labeled(t("watermark.text_field")).child(self.dialog_field_element(0, 220.0, cx)),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .pl(px(124.0))
                    .children(texts),
            )
            .child(labeled(t("watermark.font")).children(fonts))
            .child(labeled(t("watermark.size")).child(self.dialog_field_element(1, 70.0, cx)))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .pl(px(124.0))
                    .children(sizes),
            )
            .child(
                labeled(t("watermark.color"))
                    .children(colors)
                    .child(semitransparent),
            )
            .child(labeled(t("watermark.layout")).children(layouts));
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().flex().gap_1().children(kinds))
            .child(options)
            .child(sample)
            .into_any_element()
    }

    pub(crate) fn apply_watermark_dialog(
        &mut self,
        mut state: WatermarkDialogState,
    ) -> Result<(), DialogError> {
        state.dialog.text = state.inputs.text(0).to_string();
        let size = state.inputs.text(1).to_string();
        let checked = state
            .dialog
            .set_size_text(&size)
            .and_then(|()| state.dialog.command().map(|_| ()));
        if let Err(key) = checked {
            return Err(DialogError::Stay(
                Box::new(WordDialog::Watermark(state)),
                t(key).to_string(),
            ));
        }
        self.doc_mut()
            .apply_watermark_dialog(&state.dialog)
            .map_err(DialogError::Failed)
    }
}

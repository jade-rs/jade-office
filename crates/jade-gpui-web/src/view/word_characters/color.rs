//! その他の色: a colour picker for the text colour — a spectrum to click,
//! a grey row, and the hex code to type — opened from the ribbon's colour
//! list or the フォント dialog's, which it goes back to.

use gpui::{div, prelude::*, px, AnyElement, ClickEvent, Context};
use jade_web::word::font::FontDialog;

use super::{caption, swatch, Dialog};
use crate::view::DocView as DocumentView;
use crate::{i18n, palette as theme};

pub struct ColorDialog {
    /// The colour picked, `RRGGBB`.
    pub color: String,
    /// The フォント dialog to go back to, when it opened this.
    pub back: Option<Box<FontDialog>>,
}

impl ColorDialog {
    fn start(current: Option<String>) -> String {
        current
            .filter(|color| color.len() == 6 && u32::from_str_radix(color, 16).is_ok())
            .unwrap_or_else(|| "000000".to_string())
    }

    pub fn for_font(current: Option<String>) -> Self {
        Self {
            color: Self::start(current),
            back: None,
        }
    }

    pub fn from_font_dialog(font: FontDialog, current: Option<String>) -> Self {
        Self {
            color: Self::start(current),
            back: Some(Box::new(font)),
        }
    }

    /// The colour OK applies.
    pub fn hex(&self) -> String {
        self.color.to_ascii_uppercase()
    }
}

/// `#RRGGBB` typed into the field, if it is one.
fn typed(text: &str) -> Option<String> {
    let hex = text.trim().trim_start_matches('#');
    (hex.len() == 6 && u32::from_str_radix(hex, 16).is_ok()).then(|| hex.to_ascii_uppercase())
}

/// The spectrum: twelve hues across, from light to dark down.
fn spectrum() -> Vec<Vec<String>> {
    let lightness = [0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2];
    lightness
        .iter()
        .map(|l| {
            (0..12)
                .map(|step| {
                    let hue = step as f32 / 12.0;
                    let color = gpui::hsla(hue, 0.85, *l, 1.0);
                    let rgb = gpui::Rgba::from(color);
                    format!(
                        "{:02X}{:02X}{:02X}",
                        (rgb.r * 255.0).round() as u8,
                        (rgb.g * 255.0).round() as u8,
                        (rgb.b * 255.0).round() as u8
                    )
                })
                .collect()
        })
        .collect()
}

pub(super) fn body(view: &mut DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    let Some(Dialog::Color(dialog)) = view.characters.dialog.as_mut() else {
        return div().into_any_element();
    };
    // A full code typed wins over the last click.
    if let Some(hex) = typed(&view.characters.field.text) {
        dialog.color = hex;
    }
    let current = dialog.color.clone();
    let field = view.characters.field.text.clone();
    let mut grid = div().flex().flex_col().gap_0p5();
    for (row_index, row) in spectrum().into_iter().enumerate() {
        grid = grid.child(
            div()
                .flex()
                .gap_0p5()
                .children(row.into_iter().enumerate().map(|(column, color)| {
                    let on = color == current;
                    let picked = color.clone();
                    swatch(("spectrum", row_index * 12 + column), &color, on).on_click(cx.listener(
                        move |view, _: &ClickEvent, _, cx| {
                            if let Some(Dialog::Color(dialog)) = view.characters.dialog.as_mut() {
                                dialog.color = picked.clone();
                            }
                            view.characters.field.text.clear();
                            view.characters.field.caret = 0;
                            cx.notify();
                        },
                    ))
                })),
        );
    }
    let greys: Vec<String> = (0..12)
        .map(|step| {
            let value = (255.0 * step as f32 / 11.0).round() as u8;
            format!("{value:02X}{value:02X}{value:02X}")
        })
        .collect();
    grid = grid.child(
        div()
            .flex()
            .gap_0p5()
            .children(greys.into_iter().enumerate().map(|(index, color)| {
                let on = color == current;
                let picked = color.clone();
                swatch(("grey", index), &color, on).on_click(cx.listener(
                    move |view, _: &ClickEvent, _, cx| {
                        if let Some(Dialog::Color(dialog)) = view.characters.dialog.as_mut() {
                            dialog.color = picked.clone();
                        }
                        view.characters.field.text.clear();
                        view.characters.field.caret = 0;
                        cx.notify();
                    },
                ))
            })),
    );
    let rgb = u32::from_str_radix(&current, 16).unwrap_or(0);
    div()
        .w(px(320.0))
        .flex()
        .flex_col()
        .gap_3()
        .child(grid)
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(caption(i18n::text("chars.color.code")))
                .child(
                    div()
                        .min_w(px(90.0))
                        .px_1()
                        .border_1()
                        .border_color(theme::accent())
                        .bg(theme::canvas())
                        .child(if field.is_empty() {
                            format!("#{current}")
                        } else {
                            format!("{field}▏")
                        }),
                )
                .child(
                    div()
                        .w(px(48.0))
                        .h(px(24.0))
                        .border_1()
                        .border_color(theme::border())
                        .bg(gpui::rgb(rgb)),
                ),
        )
        .into_any_element()
}

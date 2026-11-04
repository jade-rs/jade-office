//! デザイン › ページの色 › 塗りつぶし効果…: the dialog's グラデーション,
//! テクスチャ, パターン and 図 tabs.

use gpui::{div, linear_color_stop, linear_gradient, prelude::*, px, AnyElement, Context};
use jade_office_model::word::page_fill::{
    fill_colors, FillTab, GradientStyle, PageFillDialog, Pattern, Texture, VARIANTS,
};

use super::{chip, line, t, PageDialog, PageDialogState};
use crate::tooltip::Tooltip;
use crate::{i18n, theme, DocumentView};

/// Which colour a row of swatches sets.
#[derive(Clone, Copy)]
enum Slot {
    Color,
    Color2,
    Foreground,
    Background,
}

fn rgb(color: &str) -> gpui::Rgba {
    gpui::rgb(u32::from_str_radix(color, 16).unwrap_or(0xFFFFFF))
}

/// gpui's angle for a style: the way the first colour runs to the second.
fn gradient_angle(style: GradientStyle) -> f32 {
    match style {
        GradientStyle::Horizontal => 180.0,
        GradientStyle::Vertical => 90.0,
        GradientStyle::DiagonalUp => 45.0,
        GradientStyle::DiagonalDown => 135.0,
    }
}

impl DocumentView {
    /// ページの色's 塗りつぶし効果…: the dialog over the effect there is.
    pub(crate) fn open_page_fill(&mut self, cx: &mut Context<Self>) {
        use jade_office_model::word::page_fill::WordPageFill;
        self.page_dialog = Some(PageDialogState {
            dialog: PageDialog::Fill(Box::new(self.document.page_fill_dialog())),
            problem: None,
        });
        cx.notify();
    }

    fn edit_fill(&mut self, cx: &mut Context<Self>, edit: impl FnOnce(&mut PageFillDialog)) {
        self.edit_page_dialog(cx, |state| {
            if let PageDialog::Fill(dialog) = &mut state.dialog {
                edit(dialog);
            }
        });
    }

    /// その他のテクスチャ… or 図の選択…: a picture file for the dialog.
    fn prompt_for_fill_file(&mut self, picture: bool, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(
                i18n::text(if picture {
                    "setup.fill.choose_picture"
                } else {
                    "setup.fill.other_texture"
                })
                .into(),
            ),
        });
        cx.spawn(async move |view, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let Ok(bytes) = std::fs::read(&path) else {
                return;
            };
            let name = path.to_string_lossy().into_owned();
            view.update(cx, |view, cx| {
                view.edit_fill(cx, |dialog| {
                    if picture {
                        dialog.set_picture(&name, bytes)
                    } else {
                        dialog.set_texture_file(&name, bytes)
                    }
                })
            })
            .ok();
        })
        .detach();
    }

    fn fill_swatches(&self, slot: Slot, current: &str, cx: &mut Context<Self>) -> Vec<AnyElement> {
        fill_colors()
            .enumerate()
            .map(|(index, (color, label))| {
                let id = match slot {
                    Slot::Color => ("fill-color", index),
                    Slot::Color2 => ("fill-color2", index),
                    Slot::Foreground => ("fill-foreground", index),
                    Slot::Background => ("fill-background", index),
                };
                div()
                    .id(id)
                    .size(px(18.0))
                    .rounded_sm()
                    .border_1()
                    .border_color(if current == color {
                        theme::accent()
                    } else {
                        theme::border()
                    })
                    .bg(rgb(color))
                    .cursor_pointer()
                    .tooltip(Tooltip::text(t(label)))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.edit_fill(cx, |dialog| {
                            let target = match slot {
                                Slot::Color => &mut dialog.color,
                                Slot::Color2 => &mut dialog.color2,
                                Slot::Foreground => &mut dialog.foreground,
                                Slot::Background => &mut dialog.background,
                            };
                            *target = color.to_string();
                        })
                    }))
                    .into_any_element()
            })
            .collect()
    }

    pub(super) fn fill_body(&self, dialog: &PageFillDialog, cx: &mut Context<Self>) -> AnyElement {
        let tabs: Vec<_> =
            FillTab::ALL
                .into_iter()
                .enumerate()
                .map(|(index, tab)| {
                    chip(
                        ("fill-tab", index),
                        t(tab.label_key()),
                        dialog.tab == tab,
                        true,
                    )
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.edit_fill(cx, |dialog| dialog.tab = tab)
                    }))
                })
                .collect();
        let body = match dialog.tab {
            FillTab::Gradient => self.gradient_tab(dialog, cx),
            FillTab::Texture => self.texture_tab(dialog, cx),
            FillTab::Pattern => self.pattern_tab(dialog, cx),
            FillTab::Picture => self.picture_tab(dialog, cx),
        };
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().flex().gap_1().children(tabs))
            .child(body)
            .into_any_element()
    }

    fn gradient_tab(&self, dialog: &PageFillDialog, cx: &mut Context<Self>) -> AnyElement {
        let styles: Vec<_> = GradientStyle::ALL
            .into_iter()
            .enumerate()
            .map(|(index, style)| {
                chip(
                    ("fill-style", index),
                    t(style.label_key()),
                    dialog.style == style,
                    true,
                )
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.edit_fill(cx, |dialog| dialog.style = style)
                }))
            })
            .collect();
        let (first, second) = (rgb(&dialog.color), rgb(&dialog.color2));
        let angle = gradient_angle(dialog.style);
        let vertical = dialog.style != GradientStyle::Vertical;
        let variants: Vec<_> = VARIANTS
            .iter()
            .enumerate()
            .map(|(index, focus)| {
                let (from, to) = if *focus > 0.0 {
                    (second, first)
                } else {
                    (first, second)
                };
                let half = |from, to| {
                    div().flex_1().bg(linear_gradient(
                        angle,
                        linear_color_stop(from, 0.0),
                        linear_color_stop(to, 1.0),
                    ))
                };
                let mirrored = focus.abs() == 50.0;
                div()
                    .id(("fill-variant", index))
                    .size(px(40.0))
                    .flex()
                    .when(vertical, |preview| preview.flex_col())
                    .border_2()
                    .border_color(if dialog.variant == index {
                        theme::accent()
                    } else {
                        theme::border()
                    })
                    .cursor_pointer()
                    .child(half(from, to))
                    .when(mirrored, |preview| preview.child(half(to, from)))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.edit_fill(cx, |dialog| dialog.variant = index)
                    }))
            })
            .collect();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(line("setup.fill.color1").children(self.fill_swatches(
                Slot::Color,
                &dialog.color,
                cx,
            )))
            .child(line("setup.fill.color2").children(self.fill_swatches(
                Slot::Color2,
                &dialog.color2,
                cx,
            )))
            .child(line("setup.fill.style").children(styles))
            .child(line("setup.fill.variant").children(variants))
            .into_any_element()
    }

    fn texture_tab(&self, dialog: &PageFillDialog, cx: &mut Context<Self>) -> AnyElement {
        let textures: Vec<_> = Texture::ALL
            .into_iter()
            .enumerate()
            .map(|(index, texture)| {
                div()
                    .id(("fill-texture", index))
                    .size(px(36.0))
                    .rounded_sm()
                    .border_2()
                    .border_color(if dialog.texture == Some(texture) {
                        theme::accent()
                    } else {
                        theme::border()
                    })
                    .bg(gpui::rgb(texture.swatch()))
                    .cursor_pointer()
                    .tooltip(Tooltip::text(t(texture.label_key())))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.edit_fill(cx, |dialog| dialog.set_texture(texture))
                    }))
            })
            .collect();
        let chosen = dialog
            .texture
            .map(|texture| t(texture.label_key()).to_string())
            .or_else(|| dialog.texture_file.as_ref().map(|file| file.name.clone()));
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(line("setup.fill.tab.texture").children(textures))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        chip(
                            "fill-other-texture",
                            t("setup.fill.other_texture"),
                            false,
                            true,
                        )
                        .on_click(
                            cx.listener(|view, _, _, cx| view.prompt_for_fill_file(false, cx)),
                        ),
                    )
                    .children(chosen.map(|name| div().text_color(theme::text_dim()).child(name))),
            )
            .into_any_element()
    }

    fn pattern_tab(&self, dialog: &PageFillDialog, cx: &mut Context<Self>) -> AnyElement {
        let patterns: Vec<_> = Pattern::ALL
            .into_iter()
            .enumerate()
            .map(|(index, pattern)| {
                chip(
                    ("fill-pattern", index),
                    t(pattern.label_key()),
                    dialog.pattern == pattern,
                    true,
                )
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.edit_fill(cx, |dialog| dialog.pattern = pattern)
                }))
            })
            .collect();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(line("setup.fill.tab.pattern").children(patterns))
            .child(line("setup.fill.foreground").children(self.fill_swatches(
                Slot::Foreground,
                &dialog.foreground,
                cx,
            )))
            .child(line("setup.fill.background").children(self.fill_swatches(
                Slot::Background,
                &dialog.background,
                cx,
            )))
            .into_any_element()
    }

    fn picture_tab(&self, dialog: &PageFillDialog, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                chip(
                    "fill-choose-picture",
                    t("setup.fill.choose_picture"),
                    false,
                    true,
                )
                .on_click(cx.listener(|view, _, _, cx| view.prompt_for_fill_file(true, cx))),
            )
            .child(
                div()
                    .text_color(theme::text_dim())
                    .child(dialog.picture.as_ref().map_or_else(
                        || t("setup.fill.no_picture").to_string(),
                        |file| file.name.clone(),
                    )),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::super::{PageDialog, SetupAction};
    use crate::ribbon::Action;
    use jade_office_model::word::page_fill::{FillTab, Texture, WordPageFill};

    /// ページの色 › 塗りつぶし効果… opens on グラデーション; OK on a tab with
    /// nothing chosen leaves the page, a texture lands behind every page,
    /// and a plain page colour takes its place.
    #[gpui::test]
    fn the_fill_effects_dialog_puts_a_texture_behind_the_pages(cx: &mut gpui::TestAppContext) {
        use gpui::Focusable;
        let path = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../jade-core/contract.docx"
        ));
        let (view, cx) =
            cx.add_window_view(|_window, cx| crate::open_document_view(&path, cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        view.update(cx, |view, cx| {
            view.run_ribbon(Action::Setup(SetupAction::OpenFill), cx);
            let state = view.page_dialog.as_ref().expect("the dialog is open");
            let PageDialog::Fill(dialog) = &state.dialog else {
                panic!("the 塗りつぶし効果 dialog");
            };
            assert_eq!(dialog.tab, FillTab::Gradient);
            view.edit_fill(cx, |dialog| dialog.tab = FillTab::Texture);
            view.apply_page_dialog(cx);
            assert!(view.page_dialog.is_none());
            assert!(view.document.page_fill().is_none(), "no texture was chosen");

            view.run_ribbon(Action::Setup(SetupAction::OpenFill), cx);
            view.edit_fill(cx, |dialog| {
                dialog.tab = FillTab::Texture;
                dialog.set_texture(Texture::Cork);
            });
            view.apply_page_dialog(cx);
            let fill = view.document.page_fill().expect("a fill effect");
            assert_eq!(fill.kind, "tile");
            assert_eq!(fill.title.as_deref(), Some("Cork"));
            assert!(fill.data_uri.is_some(), "drawn from its tile");
            assert!(view.dirty);

            view.run_ribbon(Action::Setup(SetupAction::PageColor(Some("FFF9E6"))), cx);
            assert!(view.document.page_fill().is_none());
        });
    }
}

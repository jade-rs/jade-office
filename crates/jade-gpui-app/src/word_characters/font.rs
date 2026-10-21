//! The フォント dialog's two tabs, as Word for Mac lays them out.
//!
//! フォント: 日本語用のフォント and 英数字用のフォント, スタイル, サイズ, the
//! complex-script font and size, フォントの色, 下線のスタイル, 下線の色,
//! 傍点, the 文字飾り boxes and a preview. 詳細設定: 倍率, 文字間隔, 位置,
//! カーニング, the OpenType features and the text effects.

use gpui::{div, prelude::*, px, AnyElement, ClickEvent, Context, FontWeight, SharedString};
use jade_office_model::word::font::{
    theme_palette, Effect, FontDialog, FontTab, HeadlessThemeColor, EMPHASIS_MARKS, FONT_SIZES,
    LIGATURES, NUMBER_FORMS, NUMBER_SPACINGS, SCALES, STANDARD_COLORS, UNDERLINE_STYLES,
};

use super::{
    caption, check, list_panel, list_row, points, select, stepper, swatch, theme_colors, with_list,
    Dialog, DialogList,
};
use crate::{i18n, theme, DocumentView};

fn dialog(view: &DocumentView) -> Option<&FontDialog> {
    match view.characters.dialog.as_ref()? {
        Dialog::Font(dialog) => Some(dialog),
        _ => None,
    }
}

/// Changes the dialog's controls.
fn edit(view: &mut DocumentView, change: impl FnOnce(&mut FontDialog)) {
    if let Some(Dialog::Font(dialog)) = view.characters.dialog.as_mut() {
        change(dialog);
    }
}

/// What a colour list's entry chose: the colour and its theme reference,
/// or `None` for 自動 / なし.
type ColorChoice = Option<(String, Option<HeadlessThemeColor>)>;

/// A list entry's listener: changes the dialog and closes the list.
fn pick(
    cx: &mut Context<DocumentView>,
    change: impl Fn(&mut FontDialog) + 'static,
) -> impl Fn(&ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static {
    // `cx.listener`, without its borrow of `cx` in the returned type.
    let view = cx.weak_entity();
    move |_, _, cx| {
        view.update(cx, |view, cx| {
            edit(view, &change);
            view.characters.list = None;
            cx.notify();
        })
        .ok();
    }
}

pub fn underline_label(kind: &str) -> &'static str {
    i18n::text(match kind {
        "none" => "chars.underline.none",
        "single" => "chars.underline.single",
        "words" => "chars.underline.words",
        "double" => "chars.underline.double",
        "thick" => "chars.underline.thick",
        "dotted" => "chars.underline.dotted",
        "dottedHeavy" => "chars.underline.dotted_heavy",
        "dash" => "chars.underline.dash",
        "dashedHeavy" => "chars.underline.dash_heavy",
        "dashLong" => "chars.underline.dash_long",
        "dashLongHeavy" => "chars.underline.dash_long_heavy",
        "dotDash" => "chars.underline.dot_dash",
        "dashDotHeavy" => "chars.underline.dot_dash_heavy",
        "dotDotDash" => "chars.underline.dot_dot_dash",
        "dashDotDotHeavy" => "chars.underline.dot_dot_dash_heavy",
        "wave" => "chars.underline.wave",
        "wavyHeavy" => "chars.underline.wave_heavy",
        _ => "chars.underline.wave_double",
    })
}

fn emphasis_label(mark: &str) -> &'static str {
    i18n::text(match mark {
        "dot" => "chars.emphasis.dot",
        "comma" => "chars.emphasis.comma",
        "circle" => "chars.emphasis.circle",
        "underDot" => "chars.emphasis.under_dot",
        _ => "chars.emphasis.none",
    })
}

fn effect_label(effect: Effect) -> &'static str {
    i18n::text(match effect {
        Effect::Strike => "chars.effect.strike",
        Effect::DoubleStrike => "chars.effect.double_strike",
        Effect::Superscript => "chars.effect.superscript",
        Effect::Subscript => "chars.effect.subscript",
        Effect::SmallCaps => "chars.effect.small_caps",
        Effect::AllCaps => "chars.effect.all_caps",
        Effect::Hidden => "chars.effect.hidden",
        Effect::Outline => "chars.effect.outline",
        Effect::Shadow => "chars.effect.shadow",
        Effect::Emboss => "chars.effect.emboss",
        Effect::Engrave => "chars.effect.engrave",
    })
}

fn ligature_label(value: &str) -> &'static str {
    i18n::text(match value {
        "standard" => "chars.ligatures.standard",
        "standardContextual" => "chars.ligatures.standard_contextual",
        "historicalDiscretional" => "chars.ligatures.historical",
        "all" => "chars.ligatures.all",
        _ => "chars.ligatures.none",
    })
}

fn number_label(value: &str) -> &'static str {
    i18n::text(match value {
        "proportional" => "chars.number.proportional",
        "tabular" => "chars.number.tabular",
        "lining" => "chars.number.lining",
        "oldStyle" => "chars.number.old_style",
        _ => "chars.number.default",
    })
}

fn color_label(color: Option<&str>) -> String {
    match color {
        None | Some("auto") => i18n::text("doc.color.auto").to_string(),
        Some("none") => i18n::text("chars.none").to_string(),
        Some(hex) => format!("#{hex}"),
    }
}

/// The two tabs and the open one's controls.
pub(super) fn body(view: &mut DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    let Some(dialog) = dialog(view).cloned() else {
        return div().into_any_element();
    };
    let tabs = [
        (FontTab::Font, "chars.font.tab_font"),
        (FontTab::Advanced, "chars.font.tab_advanced"),
    ];
    let tab_strip = div()
        .flex()
        .gap_1()
        .children(tabs.into_iter().enumerate().map(|(index, (tab, label))| {
            div()
                .id(("font-tab", index))
                .px_3()
                .py_0p5()
                .rounded_md()
                .cursor_pointer()
                .when(dialog.tab == tab, |chip| chip.bg(theme::selected()))
                .hover(|chip| chip.bg(theme::hovered()))
                .child(i18n::text(label))
                .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                    edit(view, |dialog| dialog.tab = tab);
                    view.characters.list = None;
                    cx.notify();
                }))
        }));
    let content = match dialog.tab {
        FontTab::Font => font_tab(view, &dialog, cx),
        FontTab::Advanced => advanced_tab(view, &dialog, cx),
    };
    div()
        .w(px(600.0))
        .flex()
        .flex_col()
        .gap_3()
        .child(tab_strip)
        .child(content)
        .child(preview(&dialog))
        .into_any_element()
}

/// The fonts the open font list shows, narrowed by what was typed.
fn filtered(view: &mut DocumentView, list: DialogList, cx: &gpui::App) -> Vec<String> {
    let (japanese, latin) = view.characters.font_lists(cx);
    let filter = view.characters.field.text.to_lowercase();
    let names = match list {
        DialogList::JapaneseFont => japanese,
        _ => latin,
    };
    names
        .into_iter()
        .filter(|name| filter.is_empty() || name.to_lowercase().contains(&filter))
        .collect()
}

/// Enter in a font list: the first font the filter left, or the typed name.
pub(super) fn pick_filtered(
    view: &mut DocumentView,
    list: DialogList,
    cx: &mut Context<DocumentView>,
) {
    let typed = view.characters.field.text.trim().to_string();
    let name = filtered(view, list, cx)
        .into_iter()
        .next()
        .or_else(|| (!typed.is_empty()).then_some(typed));
    if let Some(name) = name {
        set_font(view, list, name);
    }
    view.characters.list = None;
    cx.notify();
}

fn set_font(view: &mut DocumentView, list: DialogList, name: String) {
    edit(view, |dialog| match list {
        DialogList::JapaneseFont => dialog.format.east_asia_font = Some(name),
        DialogList::ComplexFont => dialog.format.complex_font = Some(name),
        _ => dialog.format.latin_font = Some(name),
    });
}

/// A font select, with its filtered list while open.
fn font_select(
    view: &mut DocumentView,
    id: &'static str,
    value: Option<&str>,
    list: DialogList,
    width: f32,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    let open = view.characters.list == Some(list);
    let menu = open.then(|| {
        let names = filtered(view, list, cx);
        let filter = view.characters.field.text.clone();
        list_panel("font-list", width + 40.0)
            .child(
                div()
                    .px_1()
                    .py_0p5()
                    .mb_1()
                    .border_1()
                    .border_color(theme::accent())
                    .text_color(if filter.is_empty() {
                        theme::text_faint()
                    } else {
                        theme::text()
                    })
                    .child(if filter.is_empty() {
                        i18n::text("chars.font.type_to_filter").to_string()
                    } else {
                        format!("{filter}▏")
                    }),
            )
            .children(
                names
                    .into_iter()
                    .take(400)
                    .enumerate()
                    .map(|(index, name)| {
                        let checked = value == Some(name.as_str());
                        let chosen = name.clone();
                        list_row((id, index), name, checked).on_click(cx.listener(
                            move |view, _: &ClickEvent, _, cx| {
                                set_font(view, list, chosen.clone());
                                view.characters.list = None;
                                cx.notify();
                            },
                        ))
                    }),
            )
            .into_any_element()
    });
    with_list(
        select(id, value.unwrap_or("").to_string(), width, list, open, cx),
        menu,
    )
}

/// A select over fixed choices.
#[allow(clippy::too_many_arguments)]
fn choice_select<T: Clone + PartialEq + 'static>(
    view: &DocumentView,
    id: &'static str,
    list: DialogList,
    width: f32,
    current: T,
    choices: Vec<(T, SharedString)>,
    set: impl Fn(&mut FontDialog, T) + Clone + 'static,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    let open = view.characters.list == Some(list);
    let label = choices
        .iter()
        .find(|(value, _)| *value == current)
        .map(|(_, label)| label.clone())
        .unwrap_or_default();
    let menu = open.then(|| {
        list_panel("choice-list", width.max(120.0))
            .children(
                choices
                    .into_iter()
                    .enumerate()
                    .map(|(index, (value, label))| {
                        let checked = value == current;
                        let set = set.clone();
                        list_row((id, index), label, checked)
                            .on_click(pick(cx, move |dialog| set(dialog, value.clone())))
                    }),
            )
            .into_any_element()
    });
    with_list(select(id, label, width, list, open, cx), menu)
}

/// A colour select: 自動 (or なし), the theme palette and the standard row.
#[allow(clippy::too_many_arguments)]
fn color_select(
    view: &DocumentView,
    id: &'static str,
    list: DialogList,
    current: Option<String>,
    none_label: &'static str,
    set: impl Fn(&mut FontDialog, ColorChoice) + Clone + 'static,
    more: bool,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    let open = view.characters.list == Some(list);
    let menu =
        open.then(|| {
            let palette = theme_palette(&theme_colors(view));
            let none = set.clone();
            let mut panel = list_panel("color-list", 230.0).child(
                list_row((id, 0usize), i18n::text(none_label), current.is_none())
                    .on_click(pick(cx, move |dialog| none(dialog, None))),
            );
            panel = panel.child(caption(i18n::text("chars.color.theme")));
            for (row_index, row) in palette.into_iter().enumerate() {
                panel = panel.child(div().flex().gap_0p5().children(
                    row.into_iter().enumerate().map(|(column, entry)| {
                        let set = set.clone();
                        let on = current.as_deref() == Some(entry.color.as_str());
                        swatch((id, 100 + row_index * 10 + column), &entry.color, on).on_click(
                            pick(cx, move |dialog| {
                                set(
                                    dialog,
                                    Some((entry.color.clone(), Some(entry.theme.clone()))),
                                )
                            }),
                        )
                    }),
                ));
            }
            panel =
                panel
                    .child(caption(i18n::text("chars.color.standard")))
                    .child(div().flex().gap_0p5().children(
                        STANDARD_COLORS.iter().enumerate().map(|(index, color)| {
                            let set = set.clone();
                            swatch((id, 200 + index), color, current.as_deref() == Some(*color))
                                .on_click(pick(cx, move |dialog| {
                                    set(dialog, Some((color.to_string(), None)))
                                }))
                        }),
                    ));
            if more {
                panel = panel.child(
                    list_row((id, 300usize), i18n::text("chars.color.more"), false).on_click(
                        cx.listener(|view, _: &ClickEvent, _, cx| {
                            // その他の色: the picker, then back here.
                            if let Some(Dialog::Font(font)) = view.characters.dialog.take() {
                                let current = font.format.color.clone();
                                view.characters.open(Dialog::Color(
                                    super::color::ColorDialog::from_font_dialog(*font, current),
                                ));
                            }
                            cx.notify();
                        }),
                    ),
                );
            }
            panel.into_any_element()
        });
    let value = color_label(current.as_deref());
    let rgb = current
        .as_deref()
        .and_then(|hex| u32::from_str_radix(hex, 16).ok());
    with_list(
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(
                div()
                    .size(px(14.0))
                    .border_1()
                    .border_color(theme::border())
                    .when_some(rgb, |square, rgb| square.bg(gpui::rgb(rgb))),
            )
            .child(select(id, value, 120.0, list, open, cx)),
        menu,
    )
}

fn labelled(label: &'static str, control: impl IntoElement) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_0p5()
        .child(caption(i18n::text(label)))
        .child(control)
}

fn font_tab(
    view: &mut DocumentView,
    dialog: &FontDialog,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    let format = &dialog.format;
    let japanese = font_select(
        view,
        "japanese-font",
        format.east_asia_font.as_deref(),
        DialogList::JapaneseFont,
        220.0,
        cx,
    );
    let latin = font_select(
        view,
        "latin-font",
        format.latin_font.as_deref(),
        DialogList::LatinFont,
        220.0,
        cx,
    );
    let complex = font_select(
        view,
        "complex-font",
        format.complex_font.as_deref(),
        DialogList::ComplexFont,
        220.0,
        cx,
    );
    let (bold, italic) = (format.bold == Some(true), format.italic == Some(true));
    let styles = [
        (false, false, "chars.style.regular"),
        (false, true, "chars.style.italic"),
        (true, false, "chars.style.bold"),
        (true, true, "chars.style.bold_italic"),
    ];
    let style_list =
        div()
            .flex()
            .flex_col()
            .w(px(110.0))
            .children(
                styles
                    .into_iter()
                    .enumerate()
                    .map(|(index, (b, i, label))| {
                        list_row(
                            ("font-style", index),
                            i18n::text(label),
                            bold == b && italic == i,
                        )
                        .when(b, |row| row.font_weight(FontWeight::BOLD))
                        .when(i, |row| row.italic())
                        .on_click(cx.listener(
                            move |view, _: &ClickEvent, _, cx| {
                                edit(view, |dialog| dialog.set_style(b, i));
                                cx.notify();
                            },
                        ))
                    }),
            );
    let size = format.font_size.unwrap_or(10.5);
    let sizes = choice_select(
        view,
        "font-size",
        DialogList::Size,
        64.0,
        points(size),
        FONT_SIZES
            .iter()
            .map(|size| (points(*size), SharedString::from(points(*size))))
            .collect(),
        |dialog, value: String| dialog.format.font_size = value.parse().ok(),
        cx,
    );
    let complex_size = format.complex_font_size.unwrap_or(size);
    let complex_sizes = stepper(
        "complex-size",
        points(complex_size),
        true,
        cx,
        move |view, grow| {
            edit(view, |dialog| {
                let current = dialog
                    .format
                    .complex_font_size
                    .or(dialog.format.font_size)
                    .unwrap_or(10.5);
                let next = if grow {
                    FONT_SIZES
                        .iter()
                        .copied()
                        .find(|step| *step > current + 0.01)
                } else {
                    FONT_SIZES
                        .iter()
                        .rev()
                        .copied()
                        .find(|step| *step < current - 0.01)
                };
                dialog.format.complex_font_size = Some(next.unwrap_or(current));
            })
        },
    );
    let color = color_select(
        view,
        "font-color",
        DialogList::FontColor,
        format.color.clone().filter(|color| color != "auto"),
        "doc.color.auto",
        |dialog: &mut FontDialog, choice: ColorChoice| match choice {
            Some((color, theme)) => {
                dialog.format.color = Some(color);
                dialog.format.theme_color = theme;
            }
            None => {
                dialog.format.color = Some("auto".into());
                dialog.format.theme_color = None;
            }
        },
        true,
        cx,
    );
    let underline = choice_select(
        view,
        "underline-style",
        DialogList::Underline,
        150.0,
        format.underline.clone().unwrap_or_else(|| "none".into()),
        UNDERLINE_STYLES
            .iter()
            .map(|kind| (kind.to_string(), SharedString::from(underline_label(kind))))
            .collect(),
        |dialog, kind: String| dialog.format.underline = Some(kind),
        cx,
    );
    let underline_color = color_select(
        view,
        "underline-color",
        DialogList::UnderlineColor,
        format
            .underline_color
            .clone()
            .filter(|color| color != "auto"),
        "doc.color.auto",
        |dialog: &mut FontDialog, choice: ColorChoice| {
            dialog.format.underline_color = Some(
                choice
                    .map(|(color, _)| color)
                    .unwrap_or_else(|| "auto".into()),
            );
            if dialog
                .format
                .underline
                .as_deref()
                .is_none_or(|kind| kind == "none")
            {
                dialog.format.underline = Some("single".into());
            }
        },
        false,
        cx,
    );
    let emphasis = choice_select(
        view,
        "emphasis",
        DialogList::Emphasis,
        110.0,
        format.emphasis.clone().unwrap_or_else(|| "none".into()),
        EMPHASIS_MARKS
            .iter()
            .map(|mark| (mark.to_string(), SharedString::from(emphasis_label(mark))))
            .collect(),
        |dialog, mark: String| dialog.format.emphasis = Some(mark),
        cx,
    );
    let decorations = div().flex().flex_wrap().gap_x_4().gap_y_1().children(
        Effect::DECORATIONS
            .into_iter()
            .enumerate()
            .map(|(index, effect)| {
                let on = dialog.effect(effect);
                check(("decoration", index), effect_label(effect), on).on_click(cx.listener(
                    move |view, _: &ClickEvent, _, cx| {
                        edit(view, |dialog| dialog.set_effect(effect, !on));
                        cx.notify();
                    },
                ))
            }),
    );
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .gap_4()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(labelled("chars.font.japanese", japanese))
                        .child(labelled("chars.font.latin", latin))
                        .child(labelled("chars.font.complex", complex)),
                )
                .child(labelled("chars.font.style", style_list))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(labelled("chars.font.size", sizes))
                        .child(labelled("chars.font.complex_size", complex_sizes)),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(caption(i18n::text("chars.font.all_text")))
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .child(labelled("chars.font.color", color))
                        .child(labelled("chars.font.underline", underline))
                        .child(labelled("chars.font.underline_color", underline_color))
                        .child(labelled("chars.font.emphasis", emphasis)),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(caption(i18n::text("chars.font.decorations")))
                .child(decorations),
        )
        .into_any_element()
}

fn advanced_tab(
    view: &mut DocumentView,
    dialog: &FontDialog,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    let format = &dialog.format;
    let scale = choice_select(
        view,
        "scale",
        DialogList::Scale,
        80.0,
        format.scale.unwrap_or(100.0).round() as i32,
        SCALES
            .iter()
            .map(|percent| (*percent as i32, SharedString::from(format!("{percent}%"))))
            .collect(),
        |dialog, percent: i32| dialog.format.scale = Some(percent as f32),
        cx,
    );
    let spacing = dialog.spacing();
    let spacing_kind =
        |kind: &'static str, on: bool, value: f32, cx: &mut Context<DocumentView>| {
            check(kind, i18n::text(kind), on).on_click(cx.listener(
                move |view, _: &ClickEvent, _, cx| {
                    edit(view, |dialog| dialog.format.spacing = Some(value));
                    cx.notify();
                },
            ))
        };
    let spacing_row = div()
        .flex()
        .items_center()
        .gap_2()
        .child(spacing_kind(
            "chars.spacing.normal",
            spacing == 0.0,
            0.0,
            cx,
        ))
        .child(spacing_kind(
            "chars.spacing.expanded",
            spacing > 0.0,
            spacing.abs().max(1.0),
            cx,
        ))
        .child(spacing_kind(
            "chars.spacing.condensed",
            spacing < 0.0,
            -spacing.abs().max(1.0),
            cx,
        ))
        .child(stepper(
            "spacing-by",
            format!("{} pt", points(spacing.abs())),
            spacing != 0.0,
            cx,
            |view, more| {
                edit(view, |dialog| {
                    let sign = if dialog.spacing() < 0.0 { -1.0 } else { 1.0 };
                    dialog.step_spacing(if more { 0.5 * sign } else { -0.5 * sign })
                })
            },
        ));
    let position = dialog.position();
    let position_kind =
        |kind: &'static str, on: bool, value: f32, cx: &mut Context<DocumentView>| {
            check(kind, i18n::text(kind), on).on_click(cx.listener(
                move |view, _: &ClickEvent, _, cx| {
                    edit(view, |dialog| dialog.format.position = Some(value));
                    cx.notify();
                },
            ))
        };
    let position_row = div()
        .flex()
        .items_center()
        .gap_2()
        .child(position_kind(
            "chars.position.normal",
            position == 0.0,
            0.0,
            cx,
        ))
        .child(position_kind(
            "chars.position.raised",
            position > 0.0,
            position.abs().max(3.0),
            cx,
        ))
        .child(position_kind(
            "chars.position.lowered",
            position < 0.0,
            -position.abs().max(3.0),
            cx,
        ))
        .child(stepper(
            "position-by",
            format!("{} pt", points(position.abs())),
            position != 0.0,
            cx,
            |view, more| {
                edit(view, |dialog| {
                    let sign = if dialog.position() < 0.0 { -1.0 } else { 1.0 };
                    dialog.step_position(if more { 0.5 * sign } else { -0.5 * sign })
                })
            },
        ));
    let kerning = dialog.kerning();
    let kerning_row = div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            check("kerning", i18n::text("chars.kerning"), kerning.is_some()).on_click(cx.listener(
                move |view, _: &ClickEvent, _, cx| {
                    edit(view, |dialog| dialog.set_kerning(kerning.is_none()));
                    cx.notify();
                },
            )),
        )
        .child(stepper(
            "kerning-from",
            kerning.map(points).unwrap_or_default(),
            kerning.is_some(),
            cx,
            |view, more| {
                edit(view, |dialog| {
                    dialog.step_kerning(if more { 0.5 } else { -0.5 })
                })
            },
        ))
        .child(i18n::text("chars.kerning_from"));
    let ligatures = choice_select(
        view,
        "ligatures",
        DialogList::Ligatures,
        150.0,
        format.ligatures.clone().unwrap_or_else(|| "none".into()),
        LIGATURES
            .iter()
            .map(|value| (value.to_string(), SharedString::from(ligature_label(value))))
            .collect(),
        |dialog, value: String| dialog.format.ligatures = Some(value),
        cx,
    );
    let number_spacing = choice_select(
        view,
        "number-spacing",
        DialogList::NumberSpacing,
        110.0,
        format
            .number_spacing
            .clone()
            .unwrap_or_else(|| "default".into()),
        NUMBER_SPACINGS
            .iter()
            .map(|value| (value.to_string(), SharedString::from(number_label(value))))
            .collect(),
        |dialog, value: String| dialog.format.number_spacing = Some(value),
        cx,
    );
    let number_form = choice_select(
        view,
        "number-form",
        DialogList::NumberForm,
        110.0,
        format
            .number_form
            .clone()
            .unwrap_or_else(|| "default".into()),
        NUMBER_FORMS
            .iter()
            .map(|value| (value.to_string(), SharedString::from(number_label(value))))
            .collect(),
        |dialog, value: String| dialog.format.number_form = Some(value),
        cx,
    );
    let stylistic_set = choice_select(
        view,
        "stylistic-set",
        DialogList::StylisticSet,
        80.0,
        format.stylistic_set.unwrap_or(0),
        (0..=20u8)
            .map(|set| {
                (
                    set,
                    SharedString::from(if set == 0 {
                        i18n::text("chars.number.default").to_string()
                    } else {
                        set.to_string()
                    }),
                )
            })
            .collect(),
        |dialog, set: u8| dialog.format.stylistic_set = Some(set),
        cx,
    );
    let alternates = format.contextual_alternates == Some(true);
    let effects = div().flex().flex_wrap().gap_x_4().gap_y_1().children(
        Effect::TEXT_EFFECTS
            .into_iter()
            .enumerate()
            .map(|(index, effect)| {
                let on = dialog.effect(effect);
                check(("text-effect", index), effect_label(effect), on).on_click(cx.listener(
                    move |view, _: &ClickEvent, _, cx| {
                        edit(view, |dialog| dialog.set_effect(effect, !on));
                        cx.notify();
                    },
                ))
            }),
    );
    let glow = color_select(
        view,
        "glow",
        DialogList::Glow,
        format.glow_color.clone().filter(|color| color != "none"),
        "chars.none",
        |dialog: &mut FontDialog, choice: ColorChoice| {
            dialog.format.glow_color = Some(
                choice
                    .map(|(color, _)| color)
                    .unwrap_or_else(|| "none".into()),
            );
            dialog.format.glow_radius.get_or_insert(5.0);
        },
        false,
        cx,
    );
    let outline = color_select(
        view,
        "text-outline",
        DialogList::TextOutline,
        format
            .text_outline_color
            .clone()
            .filter(|color| color != "none"),
        "chars.none",
        |dialog: &mut FontDialog, choice: ColorChoice| {
            dialog.format.text_outline_color = Some(
                choice
                    .map(|(color, _)| color)
                    .unwrap_or_else(|| "none".into()),
            );
            dialog.format.text_outline_width.get_or_insert(0.75);
        },
        false,
        cx,
    );
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(caption(i18n::text("chars.advanced.spacing")))
        .child(labelled("chars.advanced.scale", scale))
        .child(labelled("chars.advanced.char_spacing", spacing_row))
        .child(labelled("chars.advanced.position", position_row))
        .child(kerning_row)
        .child(caption(i18n::text("chars.advanced.opentype")))
        .child(
            div()
                .flex()
                .gap_3()
                .child(labelled("chars.advanced.ligatures", ligatures))
                .child(labelled("chars.advanced.number_spacing", number_spacing))
                .child(labelled("chars.advanced.number_form", number_form))
                .child(labelled("chars.advanced.stylistic_set", stylistic_set)),
        )
        .child(
            check(
                "alternates",
                i18n::text("chars.advanced.alternates"),
                alternates,
            )
            .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                edit(view, |dialog| {
                    dialog.format.contextual_alternates = Some(!alternates)
                });
                cx.notify();
            })),
        )
        .child(caption(i18n::text("chars.advanced.effects")))
        .child(effects)
        .child(
            div()
                .flex()
                .gap_3()
                .child(labelled("chars.advanced.glow", glow))
                .child(labelled("chars.advanced.text_outline", outline)),
        )
        .into_any_element()
}

/// プレビュー: a sample in the chosen formatting.
fn preview(dialog: &FontDialog) -> AnyElement {
    let format = &dialog.format;
    let size = format.font_size.unwrap_or(10.5).clamp(6.0, 36.0) * 4.0 / 3.0;
    let color = format
        .color
        .as_deref()
        .and_then(|hex| u32::from_str_radix(hex, 16).ok());
    let mut sample = dialog.preview_text();
    if format.caps == Some(true) || format.small_caps == Some(true) {
        sample = sample.to_uppercase();
    }
    let underline = format
        .underline
        .as_deref()
        .is_some_and(|kind| kind != "none");
    let family = format
        .east_asia_font
        .clone()
        .or_else(|| format.latin_font.clone())
        .unwrap_or_else(|| crate::UI_FONT_FAMILY.to_string());
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(caption(i18n::text("chars.font.preview")))
        .child(
            div()
                .h(px(64.0))
                .flex()
                .items_center()
                .justify_center()
                .overflow_hidden()
                .border_1()
                .border_color(theme::border())
                .bg(gpui::white())
                .child(
                    div()
                        .font_family(family)
                        .text_size(px(size))
                        .text_color(color.map(gpui::rgb).unwrap_or(gpui::rgb(0x000000)))
                        .when(format.bold == Some(true), |text| {
                            text.font_weight(FontWeight::BOLD)
                        })
                        .when(format.italic == Some(true), |text| text.italic())
                        .when(underline, |text| text.underline())
                        .when(
                            format.strike == Some(true) || format.double_strike == Some(true),
                            |text| text.line_through(),
                        )
                        .when(format.hidden == Some(true), |text| text.opacity(0.4))
                        .child(sample),
                ),
        )
        .into_any_element()
}

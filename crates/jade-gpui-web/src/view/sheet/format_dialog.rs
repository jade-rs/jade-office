//! The Format Cells dialog (⌘1), over the sheet: Excel's Number,
//! Alignment, Font, Border and Fill tabs, as the shared model's
//! `FormatCellsDialog` holds them. OK applies what was changed as one
//! undoable step; Escape or Cancel leaves the cells as they were.

use gpui::{div, prelude::*, px, Context, MouseButton, SharedString};
use jade_web::format_cells::{FormatCellsDialog, Negative, NumberCategory, Tab, CURRENCY_SYMBOLS};
use jade_web::sheet::{BorderLine, BorderStyle, HorizontalAlignment, VerticalAlignment};

use super::super::DocView;
use super::STANDARD_COLORS;
use crate::{i18n, palette};

/// The fonts the Font tab lists: Japanese Excel's own, then Office's
/// common Latin faces.
const FONTS: &[&str] = &[
    "游ゴシック",
    "メイリオ",
    "ＭＳ Ｐゴシック",
    "ＭＳ ゴシック",
    "游明朝",
    "Calibri",
    "Arial",
    "Times New Roman",
];

/// The sizes the Font tab offers, as Excel's list does.
const SIZES: &[f32] = &[
    8.0, 9.0, 10.0, 10.5, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 24.0, 28.0, 36.0,
];

/// The line styles the Border tab offers.
const LINES: &[(BorderStyle, &str)] = &[
    (BorderStyle::Thin, "format.line.thin"),
    (BorderStyle::Medium, "format.line.medium"),
    (BorderStyle::Thick, "format.line.thick"),
    (BorderStyle::Dashed, "format.line.dashed"),
    (BorderStyle::Dotted, "format.line.dotted"),
    (BorderStyle::Double, "format.line.double"),
];

/// A choice in a row of choices: highlighted when chosen.
fn chip(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    on: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px_2()
        .py_0p5()
        .rounded_sm()
        .border_1()
        .border_color(if on {
            palette::accent()
        } else {
            palette::border()
        })
        .when(on, |chip| chip.bg(palette::selected()))
        .cursor_pointer()
        .hover(|chip| chip.bg(palette::hovered()))
        .child(label.into())
}

fn row(label: &'static str) -> gpui::Div {
    div().flex().flex_wrap().items_center().gap_1().child(
        div()
            .w(px(110.0))
            .text_color(palette::dim())
            .child(i18n::text(label)),
    )
}

fn swatch(
    id: impl Into<gpui::ElementId>,
    color: Option<&str>,
    on: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .size(px(18.0))
        .rounded_sm()
        .border_1()
        .border_color(if on {
            palette::accent()
        } else {
            palette::border()
        })
        .when_some(
            color.and_then(|hex| u32::from_str_radix(hex, 16).ok()),
            |swatch, rgb| swatch.bg(gpui::rgb(rgb)),
        )
        .cursor_pointer()
}

impl DocView {
    /// Opens the dialog on the active cell's look.
    pub(in crate::view) fn open_format_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        sheet.format_dialog = Some(FormatCellsDialog::open(
            sheet.book.format_at(sheet.selection.active),
        ));
        cx.notify();
    }

    /// OK: what the dialog changed lands on the selection.
    pub(in crate::view) fn apply_format_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let Some(dialog) = sheet.format_dialog.take() else {
            return;
        };
        let change = dialog.change();
        if !change.is_empty() {
            let result = sheet.book.format(sheet.selection.range(), &change);
            self.edited(result, cx);
        }
        cx.notify();
    }

    /// A key while the dialog is open: Enter applies, Escape cancels, and
    /// on Custom the code is typed.
    pub(in crate::view) fn format_dialog_key(
        &mut self,
        key: &str,
        key_char: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        match key {
            "escape" => {
                if let Some(sheet) = self.sheet_mut() {
                    sheet.format_dialog = None;
                }
            }
            "enter" => return self.apply_format_dialog(cx),
            _ => {
                let Some(dialog) = self
                    .sheet_mut()
                    .and_then(|sheet| sheet.format_dialog.as_mut())
                else {
                    return;
                };
                if dialog.tab != Tab::Number || dialog.number.category != NumberCategory::Custom {
                    return;
                }
                if key == "backspace" {
                    dialog.number.pattern.pop();
                } else if let Some(text) = key_char {
                    dialog.number.pattern.push_str(text);
                }
            }
        }
        cx.notify();
    }

    fn edit_format(&mut self, cx: &mut Context<Self>, edit: impl FnOnce(&mut FormatCellsDialog)) {
        if let Some(dialog) = self
            .sheet_mut()
            .and_then(|sheet| sheet.format_dialog.as_mut())
        {
            edit(dialog);
            cx.notify();
        }
    }

    pub(in crate::view) fn format_dialog_element(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let dialog = self.sheet_ref()?.format_dialog.as_ref()?;
        let tabs: Vec<_> = Tab::ALL
            .iter()
            .enumerate()
            .map(|(index, tab)| {
                let tab = *tab;
                chip(
                    ("format-tab", index),
                    i18n::text(tab.label_key()),
                    dialog.tab == tab,
                )
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.edit_format(cx, |dialog| dialog.tab = tab)
                }))
            })
            .collect();
        let body = match dialog.tab {
            Tab::Number => self.number_tab(dialog, cx),
            Tab::Alignment => self.alignment_tab(dialog, cx),
            Tab::Font => self.font_tab(dialog, cx),
            Tab::Border => self.border_tab(dialog, cx),
            Tab::Fill => self.fill_tab(dialog, cx),
        };
        Some(
            div()
                .id("format-dialog-backdrop")
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
                        .id("format-dialog")
                        .w(px(560.0))
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
                        .child(div().text_sm().child(i18n::text("format.title")))
                        .child(div().flex().gap_1().children(tabs))
                        .child(div().min_h(px(230.0)).flex().flex_col().gap_2().child(body))
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .gap_2()
                                .child(
                                    chip("format-cancel", i18n::text("format.cancel"), false)
                                        .on_click(cx.listener(|view, _, _, cx| {
                                            if let Some(sheet) = view.sheet_mut() {
                                                sheet.format_dialog = None;
                                            }
                                            cx.notify();
                                        })),
                                )
                                .child(chip("format-ok", i18n::text("format.ok"), true).on_click(
                                    cx.listener(|view, _, _, cx| view.apply_format_dialog(cx)),
                                )),
                        ),
                ),
        )
    }

    fn number_tab(&self, dialog: &FormatCellsDialog, cx: &mut Context<Self>) -> gpui::Div {
        let number = &dialog.number;
        let categories = NumberCategory::ALL
            .iter()
            .enumerate()
            .map(|(index, category)| {
                let category = *category;
                div()
                    .id(("format-category", index))
                    .px_2()
                    .py_0p5()
                    .cursor_pointer()
                    .when(number.category == category, |item| {
                        item.bg(palette::selected())
                    })
                    .hover(|item| item.bg(palette::hovered()))
                    .child(i18n::text(category.label_key()))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.edit_format(cx, |dialog| {
                            let number = &mut dialog.number;
                            if number.category != category {
                                number.category = category;
                                // A list's first entry, as Excel selects it.
                                number.pattern = category
                                    .patterns()
                                    .first()
                                    .map(|code| code.to_string())
                                    .unwrap_or_else(|| {
                                        if category == NumberCategory::Custom {
                                            number.code()
                                        } else {
                                            String::new()
                                        }
                                    });
                            }
                        })
                    }))
            });
        let mut options = div().flex_1().flex().flex_col().gap_2().child(
            row("format.sample").child(
                div()
                    .px_2()
                    .py_0p5()
                    .border_1()
                    .border_color(palette::border())
                    .child(number.sample(1234.5)),
            ),
        );
        if number.category.has_decimals() {
            options = options.child(
                row("format.decimals")
                    .child(
                        chip("format-decimals-less", "−", false).on_click(cx.listener(
                            |view, _, _, cx| {
                                view.edit_format(cx, |dialog| {
                                    dialog.number.decimals =
                                        dialog.number.decimals.saturating_sub(1)
                                })
                            },
                        )),
                    )
                    .child(div().px_2().child(number.decimals.to_string()))
                    .child(
                        chip("format-decimals-more", "+", false).on_click(cx.listener(
                            |view, _, _, cx| {
                                view.edit_format(cx, |dialog| {
                                    dialog.number.decimals = (dialog.number.decimals + 1).min(30)
                                })
                            },
                        )),
                    ),
            );
        }
        if number.category == NumberCategory::Number {
            let on = number.thousands;
            options = options.child(row("format.thousands").child(
                chip("format-thousands", if on { "✓" } else { " " }, on).on_click(cx.listener(
                    |view, _, _, cx| {
                        view.edit_format(cx, |dialog| {
                            dialog.number.thousands = !dialog.number.thousands
                        })
                    },
                )),
            ));
        }
        if matches!(
            number.category,
            NumberCategory::Currency | NumberCategory::Accounting
        ) {
            options = options.child(row("format.symbol").children(
                CURRENCY_SYMBOLS.iter().enumerate().map(|(index, symbol)| {
                    chip(("format-symbol", index), *symbol, number.symbol == *symbol).on_click(
                        cx.listener(move |view, _, _, cx| {
                            view.edit_format(cx, |dialog| dialog.number.symbol = symbol.to_string())
                        }),
                    )
                }),
            ));
        }
        if matches!(
            number.category,
            NumberCategory::Number | NumberCategory::Currency
        ) {
            options = options.child(
                row("format.negative").children(
                    [
                        (Negative::Minus, "-1,234", false),
                        (Negative::RedMinus, "-1,234", true),
                        (Negative::Red, "1,234", true),
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(index, (negative, label, red))| {
                        chip(
                            ("format-negative", index),
                            label,
                            number.negative == negative,
                        )
                        .when(red, |chip| chip.text_color(gpui::rgb(0xd00000)))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.edit_format(cx, |dialog| dialog.number.negative = negative)
                        }))
                    }),
                ),
            );
        }
        let patterns = number.category.patterns();
        if !patterns.is_empty() {
            options = options.child(row("format.type").children(patterns.iter().enumerate().map(
                |(index, code)| {
                    let mut example = number.clone();
                    example.pattern = code.to_string();
                    chip(
                        ("format-pattern", index),
                        example.sample(45000.25),
                        number.pattern == *code,
                    )
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.edit_format(cx, |dialog| dialog.number.pattern = code.to_string())
                    }))
                },
            )));
        }
        if number.category == NumberCategory::Custom {
            options = options.child(
                row("format.code").child(
                    div()
                        .flex_1()
                        .px_2()
                        .py_0p5()
                        .border_1()
                        .border_color(palette::accent())
                        .child(format!("{}▏", number.pattern)),
                ),
            );
        }
        div()
            .flex()
            .gap_3()
            .child(
                div()
                    .id("format-categories")
                    .w(px(140.0))
                    .flex()
                    .flex_col()
                    .children(categories),
            )
            .child(options)
    }

    fn alignment_tab(&self, dialog: &FormatCellsDialog, cx: &mut Context<Self>) -> gpui::Div {
        let horizontal = [
            (HorizontalAlignment::General, "format.align.general"),
            (HorizontalAlignment::Left, "format.align.left"),
            (HorizontalAlignment::Center, "format.align.center"),
            (HorizontalAlignment::Right, "format.align.right"),
        ];
        let vertical = [
            (VerticalAlignment::Top, "format.align.top"),
            (VerticalAlignment::Center, "format.align.middle"),
            (VerticalAlignment::Bottom, "format.align.bottom"),
        ];
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                row("format.horizontal").children(horizontal.into_iter().enumerate().map(
                    |(index, (value, label))| {
                        chip(
                            ("format-horizontal", index),
                            i18n::text(label),
                            dialog.horizontal == value,
                        )
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.edit_format(cx, |dialog| dialog.horizontal = value)
                        }))
                    },
                )),
            )
            .child(
                row("format.vertical").children(vertical.into_iter().enumerate().map(
                    |(index, (value, label))| {
                        chip(
                            ("format-vertical", index),
                            i18n::text(label),
                            dialog.vertical == value,
                        )
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.edit_format(cx, |dialog| dialog.vertical = value)
                        }))
                    },
                )),
            )
            .child(
                row("format.wrap").child(
                    chip(
                        "format-wrap",
                        if dialog.wrap { "✓" } else { " " },
                        dialog.wrap,
                    )
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.edit_format(cx, |dialog| dialog.wrap = !dialog.wrap)
                    })),
                ),
            )
    }

    fn font_tab(&self, dialog: &FormatCellsDialog, cx: &mut Context<Self>) -> gpui::Div {
        let styles = [
            ("format.bold", dialog.bold),
            ("format.italic", dialog.italic),
            ("format.underline", dialog.underline),
            ("format.strike", dialog.strike),
        ];
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                row("format.font").children(FONTS.iter().enumerate().map(|(index, name)| {
                    chip(
                        ("format-font", index),
                        *name,
                        dialog.font_name.as_deref() == Some(*name),
                    )
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.edit_format(cx, |dialog| dialog.font_name = Some(name.to_string()))
                    }))
                })),
            )
            .child(
                row("format.size").children(SIZES.iter().enumerate().map(|(index, size)| {
                    let size = *size;
                    chip(
                        ("format-size", index),
                        format!("{size}"),
                        dialog.font_size == Some(size),
                    )
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.edit_format(cx, |dialog| dialog.font_size = Some(size))
                    }))
                })),
            )
            .child(
                row("format.style").children(styles.into_iter().enumerate().map(
                    |(index, (label, on))| {
                        chip(("format-style", index), i18n::text(label), on).on_click(cx.listener(
                            move |view, _, _, cx| {
                                view.edit_format(cx, |dialog| match index {
                                    0 => dialog.bold = !dialog.bold,
                                    1 => dialog.italic = !dialog.italic,
                                    2 => dialog.underline = !dialog.underline,
                                    _ => dialog.strike = !dialog.strike,
                                })
                            },
                        ))
                    },
                )),
            )
            .child(
                row("format.color").children(STANDARD_COLORS.iter().enumerate().map(
                    |(index, color)| {
                        swatch(
                            ("format-font-color", index),
                            Some(color),
                            dialog.font_color.as_deref() == Some(*color),
                        )
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.edit_format(cx, |dialog| {
                                dialog.font_color = Some(color.to_string())
                            })
                        }))
                    },
                )),
            )
    }

    fn border_tab(&self, dialog: &FormatCellsDialog, cx: &mut Context<Self>) -> gpui::Div {
        // The line the edge buttons draw: the style of an edge already set,
        // or thin black.
        let current = [
            &dialog.border_top,
            &dialog.border_bottom,
            &dialog.border_left,
            &dialog.border_right,
        ]
        .into_iter()
        .find_map(|line| line.clone())
        .unwrap_or(BorderLine {
            style: BorderStyle::Thin,
            color: None,
        });
        let line = current.clone();
        let edges = [
            ("format.edge.top", dialog.border_top.is_some()),
            ("format.edge.bottom", dialog.border_bottom.is_some()),
            ("format.edge.left", dialog.border_left.is_some()),
            ("format.edge.right", dialog.border_right.is_some()),
        ];
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                row("format.presets")
                    .child(
                        chip(
                            "format-border-none",
                            i18n::text("format.border.none"),
                            false,
                        )
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.edit_format(cx, |dialog| dialog.set_outline(None))
                        })),
                    )
                    .child({
                        let line = line.clone();
                        chip(
                            "format-border-outline",
                            i18n::text("format.border.outline"),
                            false,
                        )
                        .on_click(cx.listener(move |view, _, _, cx| {
                            let line = line.clone();
                            view.edit_format(cx, |dialog| dialog.set_outline(Some(line)))
                        }))
                    }),
            )
            .child(
                row("format.edges").children(edges.into_iter().enumerate().map(
                    |(index, (label, on))| {
                        let line = line.clone();
                        chip(("format-edge", index), i18n::text(label), on).on_click(cx.listener(
                            move |view, _, _, cx| {
                                let line = line.clone();
                                view.edit_format(cx, |dialog| {
                                    let edge = match index {
                                        0 => &mut dialog.border_top,
                                        1 => &mut dialog.border_bottom,
                                        2 => &mut dialog.border_left,
                                        _ => &mut dialog.border_right,
                                    };
                                    *edge = if edge.is_some() { None } else { Some(line) };
                                })
                            },
                        ))
                    },
                )),
            )
            .child(row("format.line").children(LINES.iter().enumerate().map(
                |(index, (style, label))| {
                    let style = *style;
                    chip(
                        ("format-line", index),
                        i18n::text(label),
                        current.style == style,
                    )
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.edit_format(cx, |dialog| {
                            for edge in [
                                &mut dialog.border_top,
                                &mut dialog.border_bottom,
                                &mut dialog.border_left,
                                &mut dialog.border_right,
                            ] {
                                if let Some(line) = edge.as_mut() {
                                    line.style = style;
                                }
                            }
                        })
                    }))
                },
            )))
    }

    fn fill_tab(&self, dialog: &FormatCellsDialog, cx: &mut Context<Self>) -> gpui::Div {
        div().flex().flex_col().gap_2().child(
            row("format.fill_color")
                .child(
                    chip(
                        "format-fill-none",
                        i18n::text("format.fill.none"),
                        dialog.fill_color.is_none(),
                    )
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.edit_format(cx, |dialog| dialog.fill_color = None)
                    })),
                )
                .children(STANDARD_COLORS.iter().enumerate().map(|(index, color)| {
                    swatch(
                        ("format-fill", index),
                        Some(color),
                        dialog.fill_color.as_deref() == Some(*color),
                    )
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.edit_format(cx, |dialog| dialog.fill_color = Some(color.to_string()))
                    }))
                })),
        )
    }
}

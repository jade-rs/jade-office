//! Styles: the スタイル pane beside the pages — every style the document
//! lists, paragraph (¶), character (a) and linked (¶a), the caret's
//! highlighted, a click applying one — and the スタイルの作成 / スタイルの
//! 変更 dialog: the name, the kind, 基準にするスタイル, 次の段落のスタイル,
//! the font, size, bold, italic, underline and colour, the alignment, line
//! spacing, space before and after and indents, スタイル ギャラリーに追加 and
//! 自動的に更新する. 選択個所と一致するように更新 is in the gallery's list and
//! the pane. The calls are `jade-office-model`'s `WordStyles`.

use gpui::{div, prelude::*, px, rgb, AnyElement, Context};
use jade_office_model::word::styles::{
    display_name, format_at_caret, NewStyle, StyleChange, StyleFormat, StyleInfo, StyleKind,
    WordStyles,
};

use crate::ribbon::{dropdown, menu_panel, menu_row, Action, Popup};
use crate::word_dialog::{check, chip, labeled, on, section, DialogError, Inputs, WordDialog};
use crate::word_tools::{ToolCommand, ToolMenu};
use crate::{i18n, theme, DocumentView};

const FONTS: [&str; 10] = [
    "游明朝",
    "游ゴシック",
    "ＭＳ 明朝",
    "ＭＳ ゴシック",
    "メイリオ",
    "Hiragino Mincho ProN",
    "Hiragino Sans",
    "Calibri",
    "Arial",
    "Times New Roman",
];
const COLORS: [Option<&str>; 8] = [
    None,
    Some("000000"),
    Some("C00000"),
    Some("2F5496"),
    Some("1F3864"),
    Some("00B050"),
    Some("7030A0"),
    Some("7F7F7F"),
];
const ALIGNMENTS: [(&str, &str); 5] = [
    ("left", "doc.align_left_title"),
    ("center", "doc.center_title"),
    ("right", "doc.align_right_title"),
    ("both", "doc.justify_title"),
    ("distribute", "word.align.distribute"),
];
const LINE_SPACINGS: [f32; 4] = [1.0, 1.15, 1.5, 2.0];

/// スタイルの作成 (with `editing` none) or スタイルの変更. Fields: 0 the name,
/// 1 the size, 2 space before, 3 space after, 4 the left indent, 5 the
/// first line's indent (negative hangs).
pub struct StyleDialog {
    pub inputs: Inputs,
    pub editing: Option<String>,
    pub kind: StyleKind,
    pub based_on: Option<String>,
    pub next: Option<String>,
    /// The format being edited; `original` is what the style had, so only
    /// what changed is written.
    pub format: StyleFormat,
    pub original: StyleFormat,
    pub original_name: String,
    pub original_based_on: Option<String>,
    pub original_next: Option<String>,
    pub quick: bool,
    pub auto_update: bool,
    /// The paragraph styles to base on or follow with: id and label.
    pub choices: Vec<(String, String)>,
}

fn kind_mark(kind: StyleKind) -> &'static str {
    match kind {
        StyleKind::Paragraph => "¶",
        StyleKind::Character => "a",
        StyleKind::Linked => "¶a",
        StyleKind::Table => "⊞",
        StyleKind::Numbering => "≡",
    }
}

fn number(value: Option<f32>) -> String {
    value
        .map(|value| {
            let text = format!("{value:.1}");
            text.trim_end_matches('0').trim_end_matches('.').to_string()
        })
        .unwrap_or_default()
}

fn japanese() -> bool {
    i18n::language() == i18n::Language::Japanese
}

impl DocumentView {
    /// スタイルの作成, from the selection's format; or スタイルの変更 of `id`.
    pub(crate) fn open_style_dialog(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        let catalog = self.document.style_catalog();
        let choices: Vec<(String, String)> = catalog
            .iter()
            .filter(|style| style.listed() && style.is_paragraph())
            .map(|style| (style.id.clone(), display_name(&style.name, japanese())))
            .collect();
        let dialog = match id {
            Some(id) => {
                let Some(style) = catalog.iter().find(|style| style.id == id) else {
                    return self.tool_done(Err(format!("no style {id}")), cx);
                };
                StyleDialog {
                    inputs: Inputs::new(vec![
                        display_name(&style.name, japanese()),
                        number(style.format.size),
                        number(style.format.space_before),
                        number(style.format.space_after),
                        number(style.format.indent_left),
                        number(style.format.first_line),
                    ]),
                    editing: Some(style.id.clone()),
                    kind: style.kind,
                    based_on: style.based_on.clone(),
                    next: style.next.clone(),
                    format: style.format.clone(),
                    original: style.format.clone(),
                    original_name: display_name(&style.name, japanese()),
                    original_based_on: style.based_on.clone(),
                    original_next: style.next.clone(),
                    quick: style.quick,
                    auto_update: style.auto_update,
                    choices,
                }
            }
            None => {
                self.tools_caret();
                let format = format_at_caret(&self.document);
                let current = self.format_state().style_id.or_else(|| {
                    catalog
                        .iter()
                        .find(|style| style.is_default && style.is_paragraph())
                        .map(|style| style.id.clone())
                });
                let name = (1..)
                    .map(|n| i18n::format("word.style.new_name", &[("n", &n)]))
                    .find(|name| !catalog.iter().any(|style| style.name == *name))
                    .unwrap_or_default();
                StyleDialog {
                    inputs: Inputs::new(vec![
                        name,
                        number(format.size),
                        number(format.space_before),
                        number(format.space_after),
                        number(format.indent_left),
                        number(format.first_line),
                    ]),
                    editing: None,
                    kind: StyleKind::Linked,
                    based_on: current,
                    next: None,
                    original: StyleFormat::default(),
                    format,
                    original_name: String::new(),
                    original_based_on: None,
                    original_next: None,
                    quick: true,
                    auto_update: false,
                    choices,
                }
            }
        };
        self.tools.dialog = Some(WordDialog::Style(dialog));
        cx.notify();
    }

    fn edit_style_dialog(&mut self, cx: &mut Context<Self>, edit: impl FnOnce(&mut StyleDialog)) {
        self.edit_dialog(cx, |dialog| {
            if let WordDialog::Style(dialog) = dialog {
                edit(dialog)
            }
        });
    }

    pub(crate) fn style_dialog_body(
        &self,
        dialog: &StyleDialog,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut body = div().flex().flex_col().gap_2();
        body = body.child(
            labeled(i18n::text("word.style.name")).child(self.dialog_field_element(0, 240.0, cx)),
        );
        if dialog.editing.is_none() {
            let kinds = [
                (StyleKind::Linked, "word.style.kind_linked"),
                (StyleKind::Paragraph, "word.style.kind_paragraph"),
                (StyleKind::Character, "word.style.kind_character"),
            ];
            body = body.child(labeled(i18n::text("word.style.kind")).children(
                kinds.into_iter().enumerate().map(|(index, (kind, label))| {
                    chip(
                        ("style-kind", index),
                        i18n::text(label),
                        dialog.kind == kind,
                    )
                    .on_click(on(cx, move |view, cx| {
                        view.edit_style_dialog(cx, |dialog| dialog.kind = kind)
                    }))
                }),
            ));
        }
        let list = |id: &'static str,
                    current: Option<&String>,
                    none: bool,
                    set: fn(&mut StyleDialog, Option<String>),
                    cx: &mut Context<Self>| {
            let mut row = div()
                .id(id)
                .flex()
                .flex_wrap()
                .gap_1()
                .max_h(px(64.0))
                .overflow_y_scroll();
            if none {
                row = row.child(
                    chip(
                        (id, 999usize),
                        i18n::text("word.style.no_style"),
                        current.is_none(),
                    )
                    .on_click(on(cx, move |view, cx| {
                        view.edit_style_dialog(cx, |dialog| set(dialog, None))
                    })),
                );
            }
            for (index, (style_id, label)) in dialog.choices.iter().enumerate() {
                let style_id = style_id.clone();
                let on_now = current == Some(&style_id);
                row = row.child(chip((id, index), label.clone(), on_now).on_click(on(
                    cx,
                    move |view, cx| {
                        let style_id = style_id.clone();
                        view.edit_style_dialog(cx, move |dialog| set(dialog, Some(style_id)))
                    },
                )));
            }
            row
        };
        body = body.child(section(i18n::text("word.style.properties")));
        body = body.child(labeled(i18n::text("word.style.based_on")).child(list(
            "style-based-on",
            dialog.based_on.as_ref(),
            true,
            |dialog, value| dialog.based_on = value,
            cx,
        )));
        if dialog.kind != StyleKind::Character {
            body = body.child(labeled(i18n::text("word.style.next")).child(list(
                "style-next",
                dialog.next.as_ref().or(dialog.editing.as_ref()),
                false,
                |dialog, value| dialog.next = value,
                cx,
            )));
        }
        body = body.child(section(i18n::text("word.style.format")));
        let fonts = FONTS
            .iter()
            .enumerate()
            .map(|(index, font)| {
                let font: &'static str = font;
                let on_now = dialog.format.font.as_deref() == Some(font);
                chip(("style-font", index), font, on_now).on_click(on(cx, move |view, cx| {
                    view.edit_style_dialog(cx, |dialog| {
                        dialog.format.font = Some(font.to_string());
                        dialog.format.east_asia_font = Some(font.to_string());
                    })
                }))
            })
            .collect::<Vec<_>>();
        body = body.child(labeled(i18n::text("format.font")).children(fonts));
        let toggle = |id: &'static str,
                      label: &'static str,
                      on_now: bool,
                      flip: fn(&mut StyleFormat),
                      cx: &mut Context<Self>| {
            chip(id, label, on_now).on_click(on(cx, move |view, cx| {
                view.edit_style_dialog(cx, |dialog| flip(&mut dialog.format))
            }))
        };
        body = body.child(
            labeled(i18n::text("format.size"))
                .child(self.dialog_field_element(1, 60.0, cx))
                .child("pt")
                .child(toggle(
                    "style-bold",
                    "B",
                    dialog.format.bold == Some(true),
                    |format| format.bold = Some(format.bold != Some(true)),
                    cx,
                ))
                .child(toggle(
                    "style-italic",
                    "I",
                    dialog.format.italic == Some(true),
                    |format| format.italic = Some(format.italic != Some(true)),
                    cx,
                ))
                .child(toggle(
                    "style-underline",
                    "U",
                    dialog.format.underline == Some(true),
                    |format| format.underline = Some(format.underline != Some(true)),
                    cx,
                )),
        );
        let colors = COLORS
            .iter()
            .enumerate()
            .map(|(index, color)| {
                let color = *color;
                let on_now = dialog.format.color.as_deref() == color;
                div()
                    .id(("style-color", index))
                    .size(px(18.0))
                    .rounded_sm()
                    .border_1()
                    .border_color(if on_now {
                        theme::accent()
                    } else {
                        theme::border()
                    })
                    .when_some(
                        color.and_then(|hex| u32::from_str_radix(hex, 16).ok()),
                        |cell, value| cell.bg(rgb(value)),
                    )
                    .when(color.is_none(), |cell| {
                        cell.flex().items_center().justify_center().child("A")
                    })
                    .cursor_pointer()
                    .on_click(on(cx, move |view, cx| {
                        view.edit_style_dialog(cx, |dialog| {
                            dialog.format.color = color.map(str::to_string)
                        })
                    }))
            })
            .collect::<Vec<_>>();
        body = body.child(labeled(i18n::text("format.color")).children(colors));
        if dialog.kind != StyleKind::Character {
            let alignments = ALIGNMENTS
                .iter()
                .enumerate()
                .map(|(index, (alignment, label))| {
                    let alignment: &'static str = alignment;
                    chip(
                        ("style-alignment", index),
                        i18n::text(label),
                        dialog.format.alignment.as_deref() == Some(alignment),
                    )
                    .on_click(on(cx, move |view, cx| {
                        view.edit_style_dialog(cx, |dialog| {
                            dialog.format.alignment = Some(alignment.to_string())
                        })
                    }))
                })
                .collect::<Vec<_>>();
            let spacings = LINE_SPACINGS
                .iter()
                .enumerate()
                .map(|(index, line)| {
                    let line = *line;
                    chip(
                        ("style-line", index),
                        format!("{line}"),
                        dialog
                            .format
                            .line_spacing
                            .is_some_and(|now| (now - line).abs() < 0.01),
                    )
                    .on_click(on(cx, move |view, cx| {
                        view.edit_style_dialog(cx, |dialog| dialog.format.line_spacing = Some(line))
                    }))
                })
                .collect::<Vec<_>>();
            body = body
                .child(labeled(i18n::text("word.style.alignment")).children(alignments))
                .child(labeled(i18n::text("word.style.line_spacing")).children(spacings))
                .child(
                    labeled(i18n::text("word.style.spacing"))
                        .child(self.dialog_field_element(2, 50.0, cx))
                        .child(i18n::text("word.style.before"))
                        .child(self.dialog_field_element(3, 50.0, cx))
                        .child(i18n::text("word.style.after")),
                )
                .child(
                    labeled(i18n::text("word.style.indent"))
                        .child(self.dialog_field_element(4, 50.0, cx))
                        .child(i18n::text("word.style.left"))
                        .child(self.dialog_field_element(5, 50.0, cx))
                        .child(i18n::text("word.style.first_line")),
                );
        }
        let quick = dialog.quick;
        let auto_update = dialog.auto_update;
        body = body.child(
            div()
                .flex()
                .gap_4()
                .child(
                    check("style-quick", i18n::text("word.style.quick"), quick).on_click(on(
                        cx,
                        |view, cx| {
                            view.edit_style_dialog(cx, |dialog| dialog.quick = !dialog.quick)
                        },
                    )),
                )
                .when(dialog.kind != StyleKind::Character, |row| {
                    row.child(
                        check(
                            "style-auto",
                            i18n::text("word.style.auto_update"),
                            auto_update,
                        )
                        .on_click(on(cx, |view, cx| {
                            view.edit_style_dialog(cx, |dialog| {
                                dialog.auto_update = !dialog.auto_update
                            })
                        })),
                    )
                }),
        );
        // The sample, as Word's dialog draws one.
        let sample_size = dialog.format.size.unwrap_or(10.5).clamp(6.0, 28.0);
        let mut sample = div()
            .p_2()
            .bg(theme::paper())
            .text_color(rgb(dialog
                .format
                .color
                .as_deref()
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .unwrap_or(0x202020)))
            .text_size(px(sample_size))
            .when(dialog.format.bold == Some(true), |text| {
                text.font_weight(gpui::FontWeight::BOLD)
            })
            .when(dialog.format.italic == Some(true), |text| text.italic())
            .when(dialog.format.underline == Some(true), |text| {
                text.underline()
            });
        if let Some(font) = dialog.format.font.clone() {
            sample = sample.font_family(font);
        }
        sample = match dialog.format.alignment.as_deref() {
            Some("center") => sample.flex().justify_center(),
            Some("right") => sample.flex().justify_end(),
            _ => sample,
        };
        body.child(sample.child(i18n::text("word.style.sample")))
            .into_any_element()
    }

    pub(crate) fn apply_style_dialog(
        &mut self,
        mut dialog: StyleDialog,
    ) -> Result<(), DialogError> {
        let name = dialog.inputs.text(0).trim().to_string();
        if name.is_empty() {
            return Err(DialogError::Stay(
                Box::new(WordDialog::Style(dialog)),
                i18n::text("word.error.name").to_string(),
            ));
        }
        let parse = |text: &str| -> Result<Option<f32>, ()> {
            let text = text.trim();
            if text.is_empty() {
                return Ok(None);
            }
            text.parse::<f32>().map(Some).map_err(|_| ())
        };
        let fields: Result<Vec<Option<f32>>, ()> = (1..=5)
            .map(|index| parse(dialog.inputs.text(index)))
            .collect();
        let Ok(fields) = fields else {
            return Err(DialogError::Stay(
                Box::new(WordDialog::Style(dialog)),
                i18n::text("word.error.value").to_string(),
            ));
        };
        dialog.format.size = fields[0].map(|size| size.clamp(1.0, 1638.0));
        dialog.format.space_before = fields[1];
        dialog.format.space_after = fields[2];
        dialog.format.indent_left = fields[3];
        dialog.format.first_line = fields[4];
        match dialog.editing.clone() {
            None => {
                self.tools_caret();
                self.document
                    .create_style(&NewStyle {
                        name,
                        kind: dialog.kind,
                        based_on: dialog.based_on.clone(),
                        next: dialog.next.clone(),
                        format: dialog.format.clone(),
                        quick: dialog.quick,
                        auto_update: dialog.auto_update,
                    })
                    .map(|_| ())
                    .map_err(DialogError::Failed)
            }
            Some(id) => {
                // Only what changed is written into the style.
                let mut format = StyleFormat::default();
                macro_rules! changed {
                    ($($field:ident),*) => {$(
                        if dialog.format.$field != dialog.original.$field {
                            format.$field = dialog.format.$field.clone();
                        }
                    )*};
                }
                changed!(
                    font,
                    east_asia_font,
                    size,
                    bold,
                    italic,
                    underline,
                    color,
                    alignment,
                    line_spacing,
                    space_before,
                    space_after,
                    indent_left,
                    first_line
                );
                let change = StyleChange {
                    name: (name != dialog.original_name).then_some(name),
                    based_on: (dialog.based_on != dialog.original_based_on)
                        .then(|| dialog.based_on.clone()),
                    next: if dialog.next != dialog.original_next {
                        dialog.next.clone()
                    } else {
                        None
                    },
                    format,
                    quick: Some(dialog.quick),
                    auto_update: Some(dialog.auto_update),
                };
                self.document
                    .modify_style(&id, &change)
                    .map_err(DialogError::Failed)
            }
        }
    }

    /// The ホーム tab's スタイル tools: the pane and スタイルの作成, and for
    /// the caret's style 変更 and 選択個所と一致するように更新.
    pub(crate) fn style_tools_group(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let current = self
            .format_state()
            .style_id
            .or_else(|| self.ribbon.default_paragraph_style());
        let open = self.ribbon.popup == Some(Popup::Word(ToolMenu::StyleCard(0)));
        let menu = open.then(|| {
            let mut panel = menu_panel("style-tools-menu").w(px(240.0)).child(menu_row(
                "style-new",
                i18n::text("word.style.new_menu"),
                false,
                Action::Word(ToolCommand::OpenNewStyle),
                cx,
            ));
            if let Some(id) = current.clone() {
                panel = panel
                    .child(menu_row(
                        "style-modify",
                        i18n::text("word.style.modify_menu"),
                        false,
                        Action::Word(ToolCommand::OpenModifyStyle(id.clone())),
                        cx,
                    ))
                    .child(menu_row(
                        "style-update",
                        i18n::text("word.style.update_menu"),
                        false,
                        Action::Word(ToolCommand::UpdateStyle(id)),
                        cx,
                    ));
            }
            panel.into_any_element()
        });
        let pane = self.tools.styles_pane;
        crate::ribbon::group(
            i18n::text("word.group.style_tools"),
            vec![
                crate::ribbon::row(vec![div()
                    .id("styles-pane-toggle")
                    .flex()
                    .items_center()
                    .h(px(26.0))
                    .px_2()
                    .rounded_md()
                    .text_xs()
                    .cursor_pointer()
                    .when(pane, |button| button.bg(theme::selected()))
                    .hover(|button| button.bg(theme::hovered()))
                    .on_click(on(cx, |view, cx| {
                        view.run_tool(ToolCommand::ToggleStylesPane, cx)
                    }))
                    .child(i18n::text("word.style.pane"))
                    .into_any_element()]),
                crate::ribbon::row(vec![dropdown(
                    crate::word_page::list_button(
                        "style-tools",
                        i18n::text("word.style.tools"),
                        open,
                        Popup::Word(ToolMenu::StyleCard(0)),
                        cx,
                    ),
                    menu,
                )]),
            ],
        )
    }

    /// The スタイル pane, beside the pages.
    pub(crate) fn styles_pane(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.tools.styles_pane {
            return None;
        }
        let catalog: Vec<StyleInfo> = self
            .document
            .style_catalog()
            .into_iter()
            .filter(StyleInfo::listed)
            .collect();
        let current = self.format_state().style_id;
        let default = catalog
            .iter()
            .find(|style| style.is_default && style.is_paragraph())
            .map(|style| style.id.clone());
        let mut sorted: Vec<&StyleInfo> = catalog.iter().collect();
        sorted.sort_by(|a, b| {
            a.priority
                .cmp(&b.priority)
                .then_with(|| a.name.cmp(&b.name))
        });
        let open_row = match self.ribbon.popup {
            Some(Popup::Word(ToolMenu::PaneStyle(index))) => Some(index),
            _ => None,
        };
        let rows = sorted
            .into_iter()
            .enumerate()
            .map(|(index, style)| {
                let active = current.as_deref() == Some(style.id.as_str())
                    || (current.is_none() && default.as_deref() == Some(style.id.as_str()));
                let id = style.id.clone();
                let menu = (open_row == Some(index)).then(|| {
                    menu_panel("pane-style-menu")
                        .w(px(220.0))
                        .child(menu_row(
                            "pane-style-modify",
                            i18n::text("word.style.modify_menu"),
                            false,
                            Action::Word(ToolCommand::OpenModifyStyle(id.clone())),
                            cx,
                        ))
                        .child(menu_row(
                            "pane-style-update",
                            i18n::text("word.style.update_menu"),
                            false,
                            Action::Word(ToolCommand::UpdateStyle(id.clone())),
                            cx,
                        ))
                        .into_any_element()
                });
                let apply = style.id.clone();
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .id(("pane-style", index))
                            .flex_1()
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .cursor_pointer()
                            .when(active, |row| row.bg(theme::selected()))
                            .hover(|row| row.bg(theme::hovered()))
                            .on_click(on(cx, move |view, cx| {
                                view.run_tool(ToolCommand::ApplyStyle(apply.clone()), cx)
                            }))
                            .child(
                                div()
                                    .w(px(18.0))
                                    .text_color(theme::text_faint())
                                    .child(kind_mark(style.kind)),
                            )
                            .child(
                                div()
                                    .when(style.format.bold == Some(true), |text| {
                                        text.font_weight(gpui::FontWeight::BOLD)
                                    })
                                    .child(display_name(&style.name, japanese())),
                            ),
                    )
                    .child(dropdown(
                        div()
                            .id(("pane-style-more", index))
                            .px_1()
                            .rounded_sm()
                            .cursor_pointer()
                            .hover(|button| button.bg(theme::hovered()))
                            .on_click(crate::ribbon::listener(
                                Action::Popup(Popup::Word(ToolMenu::PaneStyle(index))),
                                cx,
                            ))
                            .child(
                                gpui::svg()
                                    .path("icons/chevron-down.svg")
                                    .size_3()
                                    .text_color(theme::text_dim()),
                            )
                            .into_any_element(),
                        menu,
                    ))
            })
            .collect::<Vec<_>>();
        Some(
            div()
                .id("styles-pane")
                .flex()
                .flex_col()
                .flex_none()
                .w(px(240.0))
                .h_full()
                .gap_1()
                .p_3()
                .bg(theme::panel())
                .border_l_1()
                .border_color(theme::border())
                .text_xs()
                .overflow_y_scroll()
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .items_center()
                        .child(div().text_sm().child(i18n::text("word.style.pane")))
                        .child(
                            div()
                                .id("pane-new-style")
                                .px_2()
                                .py_0p5()
                                .rounded_md()
                                .border_1()
                                .border_color(theme::border())
                                .cursor_pointer()
                                .hover(|button| button.bg(theme::hovered()))
                                .on_click(on(cx, |view, cx| {
                                    view.run_tool(ToolCommand::OpenNewStyle, cx)
                                }))
                                .child(i18n::text("word.style.new_short")),
                        ),
                )
                .children(rows)
                .into_any_element(),
        )
    }
}

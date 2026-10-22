//! The paragraph tools Word keeps outside the ribbon's buttons: タブとリーダー
//! (tab stops with their alignment and leader, and the default interval),
//! 線種とページ罫線と網かけの設定 (the border box line by line, and the
//! shading), ドロップキャップ, the 段落 dialog's switches — 同じスタイルの場合
//! は段落間にスペースを追加しない, 行番号を表示しない, 段落の自動ハイフネー
//! ションを行わない — and 均等割り付け. The browser's copy of the native
//! app's `word_paragraph`, over the same `jade-office-model` calls
//! (`WordParagraphOptions`); the menus that open these dialogs are in
//! `word_tools`.

use gpui::{div, prelude::*, px, rgb, AnyElement, Context};
use jade_web::word::paragraph_options::{
    BorderBox, BorderLine, DropCapPosition, ShadingSpec, Tab, TabAlignment, TabLeader,
    WordParagraphOptions,
};
use jade_web::word::ruler::{format_length, parse_length};

use super::word_dialog::{button, chip, labeled, on, section, DialogError, Inputs, WordDialog};
use super::DocView as DocumentView;
use crate::i18n;
use crate::palette as theme;

/// The line styles the borders dialog offers, with their labels.
const LINE_STYLES: [(&str, &str); 8] = [
    ("single", "word.line.single"),
    ("dotted", "word.line.dotted"),
    ("dashed", "word.line.dashed"),
    ("dashSmallGap", "word.line.dash_small"),
    ("dotDash", "word.line.dot_dash"),
    ("double", "word.line.double"),
    ("thickThinSmallGap", "word.line.thick_thin"),
    ("wave", "word.line.wave"),
];
/// Word's line widths, points.
const LINE_WIDTHS: [f32; 7] = [0.25, 0.5, 0.75, 1.0, 1.5, 2.25, 3.0];
const LINE_COLORS: [Option<&str>; 8] = [
    None,
    Some("000000"),
    Some("7F7F7F"),
    Some("C00000"),
    Some("2F5496"),
    Some("00B050"),
    Some("FFC000"),
    Some("7030A0"),
];
const FILLS: [Option<&str>; 9] = [
    None,
    Some("F2F2F2"),
    Some("D9D9D9"),
    Some("FFF2CC"),
    Some("E2EFDA"),
    Some("DDEBF7"),
    Some("FCE4D6"),
    Some("EDEDED"),
    Some("FFFF00"),
];
const PATTERNS: [(&str, &str); 8] = [
    ("clear", "word.shading.clear"),
    ("pct10", "10%"),
    ("pct25", "25%"),
    ("pct50", "50%"),
    ("solid", "word.shading.solid"),
    ("horzStripe", "word.shading.horizontal"),
    ("vertStripe", "word.shading.vertical"),
    ("diagStripe", "word.shading.diagonal"),
];

/// タブとリーダー: a working copy of the paragraph's tab stops, which OK
/// puts on it. Field 0 is the position, field 1 the default interval.
pub struct TabsDialog {
    pub inputs: Inputs,
    pub tabs: Vec<Tab>,
    pub alignment: TabAlignment,
    pub leader: TabLeader,
    pub default_tab: f32,
    pub selected: Option<usize>,
}

/// 線種とページ罫線と網かけの設定.
pub struct BordersDialog {
    pub inputs: Inputs,
    pub borders: BorderBox,
    pub style: &'static str,
    pub width: f32,
    pub color: Option<&'static str>,
    pub shading: Option<ShadingSpec>,
}

/// ドロップキャップ. Field 0 is how many lines it drops.
pub struct DropCapDialog {
    pub inputs: Inputs,
    pub position: Option<DropCapPosition>,
}

fn alignment_label(alignment: TabAlignment) -> &'static str {
    i18n::text(match alignment {
        TabAlignment::Left => "word.tab.left",
        TabAlignment::Center => "word.tab.center",
        TabAlignment::Right => "word.tab.right",
        TabAlignment::Decimal => "word.tab.decimal",
        TabAlignment::Bar => "word.tab.bar",
    })
}

fn leader_label(leader: TabLeader) -> String {
    match leader {
        TabLeader::None => i18n::text("word.tab.leader_none").to_string(),
        leader => leader.sample().to_string(),
    }
}

fn color_cell(
    id: impl Into<gpui::ElementId>,
    color: Option<&str>,
    on: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .size(px(18.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded_sm()
        .border_1()
        .border_color(if on { theme::accent() } else { theme::border() })
        .when_some(
            color.and_then(|hex| u32::from_str_radix(hex, 16).ok()),
            |cell, color| cell.bg(rgb(color)),
        )
        .when(color.is_none(), |cell| cell.child("∅"))
        .cursor_pointer()
}

impl DocumentView {
    /// A character of the body text, for lengths shown in 字.
    pub(crate) fn char_width(&self) -> f32 {
        match &self.document {
            super::Document::Pages {
                format: Some((_, state)),
                ..
            } => state.font_size.unwrap_or(10.5),
            _ => 10.5,
        }
    }

    pub(crate) fn open_tabs_dialog(&mut self, cx: &mut gpui::Context<Self>) {
        let state = self.doc().paragraph_options();
        let unit = self.tools.unit;
        let char_width = self.char_width();
        let first = state.tabs.first().copied();
        self.tools.dialog = Some(WordDialog::Tabs(TabsDialog {
            inputs: Inputs::new(vec![
                first
                    .map(|tab| format_length(unit, char_width, tab.position))
                    .unwrap_or_default(),
                format_length(unit, char_width, state.default_tab),
            ]),
            alignment: first.map_or(TabAlignment::Left, |tab| tab.alignment),
            leader: first.map_or(TabLeader::None, |tab| tab.leader),
            selected: first.map(|_| 0),
            tabs: state.tabs,
            default_tab: state.default_tab,
        }));
        cx.notify();
    }

    /// 設定: the position field's stop, with the chosen alignment and
    /// leader, into the working list.
    fn tabs_dialog_set(&mut self, cx: &mut Context<Self>) {
        let unit = self.tools.unit;
        let char_width = self.char_width();
        self.edit_dialog(cx, |dialog| {
            let WordDialog::Tabs(dialog) = dialog else {
                return;
            };
            let Some(position) = parse_length(unit, char_width, dialog.inputs.text(0)) else {
                return;
            };
            dialog
                .tabs
                .retain(|tab| (tab.position - position).abs() > 0.5);
            dialog
                .tabs
                .push(Tab::new(position.max(0.0), dialog.alignment, dialog.leader));
            dialog
                .tabs
                .sort_by(|a, b| a.position.total_cmp(&b.position));
            dialog.selected = dialog
                .tabs
                .iter()
                .position(|tab| (tab.position - position).abs() <= 0.5);
        });
    }

    pub(crate) fn tabs_dialog_body(
        &self,
        dialog: &TabsDialog,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let unit = self.tools.unit;
        let char_width = self.char_width();
        let list = div()
            .id("tabs-list")
            .flex()
            .flex_col()
            .w(px(180.0))
            .h(px(120.0))
            .overflow_y_scroll()
            .border_1()
            .border_color(theme::border())
            .rounded_sm()
            .bg(theme::canvas())
            .children(dialog.tabs.iter().enumerate().map(|(index, tab)| {
                let tab = *tab;
                div()
                    .id(("tab-stop", index))
                    .px_2()
                    .py_0p5()
                    .cursor_pointer()
                    .when(dialog.selected == Some(index), |row| {
                        row.bg(theme::selected())
                    })
                    .hover(|row| row.bg(theme::hovered()))
                    .child(format!(
                        "{}  {}",
                        format_length(unit, char_width, tab.position),
                        alignment_label(tab.alignment)
                    ))
                    .on_click(on(cx, move |view, cx| {
                        view.edit_dialog(cx, |dialog| {
                            let WordDialog::Tabs(dialog) = dialog else {
                                return;
                            };
                            dialog.selected = Some(index);
                            dialog.alignment = tab.alignment;
                            dialog.leader = tab.leader;
                            dialog
                                .inputs
                                .set(0, format_length(unit, char_width, tab.position));
                        })
                    }))
            }));
        let alignments = TabAlignment::ALL
            .into_iter()
            .enumerate()
            .map(|(index, alignment)| {
                chip(
                    ("tab-alignment", index),
                    alignment_label(alignment),
                    dialog.alignment == alignment,
                )
                .on_click(on(cx, move |view, cx| {
                    view.edit_dialog(cx, |dialog| {
                        if let WordDialog::Tabs(dialog) = dialog {
                            dialog.alignment = alignment;
                        }
                    })
                }))
            })
            .collect::<Vec<_>>();
        let leaders = TabLeader::ALL
            .into_iter()
            .enumerate()
            .map(|(index, leader)| {
                chip(
                    ("tab-leader", index),
                    leader_label(leader),
                    dialog.leader == leader,
                )
                .on_click(on(cx, move |view, cx| {
                    view.edit_dialog(cx, |dialog| {
                        if let WordDialog::Tabs(dialog) = dialog {
                            dialog.leader = leader;
                        }
                    })
                }))
            })
            .collect::<Vec<_>>();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_color(theme::text_dim())
                                    .child(i18n::text("word.tab.position")),
                            )
                            .child(self.dialog_field_element(0, 180.0, cx))
                            .child(list),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_color(theme::text_dim())
                                    .child(i18n::text("word.tab.default")),
                            )
                            .child(self.dialog_field_element(1, 120.0, cx)),
                    ),
            )
            .child(labeled(i18n::text("word.tab.alignment")).children(alignments))
            .child(labeled(i18n::text("word.tab.leader")).children(leaders))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        button("tab-set", i18n::text("word.tab.set"), false)
                            .on_click(on(cx, |view, cx| view.tabs_dialog_set(cx))),
                    )
                    .child(
                        button("tab-clear", i18n::text("word.tab.clear"), false).on_click(on(
                            cx,
                            |view, cx| {
                                view.edit_dialog(cx, |dialog| {
                                    let WordDialog::Tabs(dialog) = dialog else {
                                        return;
                                    };
                                    if let Some(index) = dialog.selected.take() {
                                        if index < dialog.tabs.len() {
                                            dialog.tabs.remove(index);
                                        }
                                    }
                                })
                            },
                        )),
                    )
                    .child(
                        button("tab-clear-all", i18n::text("word.tab.clear_all"), false).on_click(
                            on(cx, |view, cx| {
                                view.edit_dialog(cx, |dialog| {
                                    if let WordDialog::Tabs(dialog) = dialog {
                                        dialog.tabs.clear();
                                        dialog.selected = None;
                                    }
                                })
                            }),
                        ),
                    ),
            )
            .into_any_element()
    }

    pub(crate) fn apply_tabs_dialog(&mut self, mut dialog: TabsDialog) -> Result<(), DialogError> {
        let unit = self.tools.unit;
        let char_width = self.char_width();
        // A position typed and not yet 設定 goes in too, as Word's OK does.
        if let Some(position) = parse_length(unit, char_width, dialog.inputs.text(0)) {
            if !dialog
                .tabs
                .iter()
                .any(|tab| (tab.position - position).abs() <= 0.5)
                && dialog.selected.is_none()
            {
                dialog
                    .tabs
                    .push(Tab::new(position.max(0.0), dialog.alignment, dialog.leader));
            } else if let Some(index) = dialog.selected {
                if let Some(tab) = dialog.tabs.get_mut(index) {
                    tab.alignment = dialog.alignment;
                    tab.leader = dialog.leader;
                    tab.position = position.max(0.0);
                }
            }
        }
        let current = self.doc().paragraph_options().tabs;
        let mut wanted = dialog.tabs.clone();
        wanted.sort_by(|a, b| a.position.total_cmp(&b.position));
        if wanted != current {
            self.doc_mut().set_tabs(&wanted)?;
        }
        match parse_length(unit, char_width, dialog.inputs.text(1)) {
            Some(interval) if (interval - dialog.default_tab).abs() > 0.05 => {
                self.doc_mut().set_default_tab_stop(interval)?;
            }
            Some(_) => {}
            None => {
                return Err(DialogError::Stay(
                    Box::new(WordDialog::Tabs(dialog)),
                    i18n::text("word.error.length").to_string(),
                ))
            }
        }
        Ok(())
    }

    pub(crate) fn open_borders_dialog(&mut self, cx: &mut Context<Self>) {
        let state = self.doc().paragraph_options();
        let line = [
            &state.borders.top,
            &state.borders.bottom,
            &state.borders.left,
            &state.borders.right,
        ]
        .into_iter()
        .flatten()
        .next()
        .cloned();
        let style = line
            .as_ref()
            .and_then(|line| LINE_STYLES.iter().find(|(token, _)| *token == line.style))
            .map_or("single", |(token, _)| *token);
        let color = line
            .as_ref()
            .and_then(|line| line.color.as_deref())
            .and_then(|color| {
                LINE_COLORS
                    .iter()
                    .flatten()
                    .find(|known| **known == color)
                    .copied()
            });
        let space = line.as_ref().map_or(1.0, |line| line.space);
        self.tools.dialog = Some(WordDialog::Borders(BordersDialog {
            inputs: Inputs::new(vec![format!("{space}")]),
            width: line.as_ref().map_or(0.5, |line| line.width),
            borders: state.borders,
            style,
            color,
            shading: state.shading,
        }));
        cx.notify();
    }

    fn border_line(dialog: &BordersDialog) -> BorderLine {
        let mut line = BorderLine::new(dialog.style, dialog.width, dialog.color);
        line.space = dialog
            .inputs
            .text(0)
            .trim()
            .parse()
            .unwrap_or(1.0f32)
            .clamp(0.0, 31.0);
        line
    }

    pub(crate) fn borders_dialog_body(
        &self,
        dialog: &BordersDialog,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let set = |edit: fn(&mut BordersDialog)| {
            on(cx, move |view: &mut DocumentView, cx| {
                view.edit_dialog(cx, |dialog| {
                    if let WordDialog::Borders(dialog) = dialog {
                        edit(dialog)
                    }
                })
            })
        };
        let presets = div()
            .flex()
            .gap_1()
            .child(
                chip(
                    "borders-none",
                    i18n::text("word.borders.none"),
                    dialog.borders.is_empty(),
                )
                .on_click(set(|dialog| dialog.borders = BorderBox::default())),
            )
            .child(
                chip(
                    "borders-box",
                    i18n::text("word.borders.box"),
                    dialog.borders.top.is_some()
                        && dialog.borders.bottom.is_some()
                        && dialog.borders.left.is_some()
                        && dialog.borders.right.is_some(),
                )
                .on_click(set(|dialog| {
                    let line = DocumentView::border_line(&*dialog);
                    dialog.borders = BorderBox::boxed(line);
                })),
            );
        // The preview: a paragraph drawn with its box, each side a toggle,
        // as Word's プレビュー.
        let side = |id: &'static str,
                    label: &'static str,
                    active: bool,
                    pick: fn(&mut BorderBox) -> &mut Option<BorderLine>| {
            chip(id, i18n::text(label), active).on_click(on(cx, move |view, cx| {
                view.edit_dialog(cx, |dialog| {
                    if let WordDialog::Borders(dialog) = dialog {
                        let line = DocumentView::border_line(&*dialog);
                        let slot = pick(&mut dialog.borders);
                        *slot = if slot.is_some() { None } else { Some(line) };
                    }
                })
            }))
        };
        let edge = |line: &Option<BorderLine>| {
            line.as_ref().map(|line| {
                let color = line
                    .color
                    .as_deref()
                    .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                    .unwrap_or(0x000000);
                (px(line.width.clamp(1.0, 4.0)), rgb(color))
            })
        };
        let mut preview = div()
            .w(px(200.0))
            .h(px(90.0))
            .p_2()
            .bg(gpui::rgb(0xffffff))
            .text_color(rgb(0x404040))
            .text_size(px(8.0))
            .child("あいうえお かきくけこ さしすせそ たちつてと なにぬねの はひふへほ まみむめも");
        if let Some((width, color)) = edge(&dialog.borders.top) {
            preview = preview.border_t(width).border_color(color);
        }
        if let Some((width, color)) = edge(&dialog.borders.bottom) {
            preview = preview.border_b(width).border_color(color);
        }
        if let Some((width, color)) = edge(&dialog.borders.left) {
            preview = preview.border_l(width).border_color(color);
        }
        if let Some((width, color)) = edge(&dialog.borders.right) {
            preview = preview.border_r(width).border_color(color);
        }
        if let Some(fill) = dialog
            .shading
            .as_ref()
            .and_then(|shading| shading.fill.as_deref())
            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
        {
            preview = preview.bg(rgb(fill));
        }
        let styles = LINE_STYLES
            .iter()
            .enumerate()
            .map(|(index, (token, label))| {
                let token: &'static str = token;
                chip(
                    ("border-style", index),
                    i18n::text(label),
                    dialog.style == token,
                )
                .on_click(on(cx, move |view, cx| {
                    view.edit_dialog(cx, |dialog| {
                        if let WordDialog::Borders(dialog) = dialog {
                            dialog.style = token;
                        }
                    })
                }))
            })
            .collect::<Vec<_>>();
        let widths = LINE_WIDTHS
            .iter()
            .enumerate()
            .map(|(index, width)| {
                let width = *width;
                chip(
                    ("border-width", index),
                    format!("{width} pt"),
                    (dialog.width - width).abs() < 0.01,
                )
                .on_click(on(cx, move |view, cx| {
                    view.edit_dialog(cx, |dialog| {
                        if let WordDialog::Borders(dialog) = dialog {
                            dialog.width = width;
                        }
                    })
                }))
            })
            .collect::<Vec<_>>();
        let colors = LINE_COLORS
            .iter()
            .enumerate()
            .map(|(index, color)| {
                let color = *color;
                color_cell(("border-color", index), color, dialog.color == color).on_click(on(
                    cx,
                    move |view, cx| {
                        view.edit_dialog(cx, |dialog| {
                            if let WordDialog::Borders(dialog) = dialog {
                                dialog.color = color;
                            }
                        })
                    },
                ))
            })
            .collect::<Vec<_>>();
        let fill_now = dialog
            .shading
            .as_ref()
            .and_then(|shading| shading.fill.clone());
        let fills = FILLS
            .iter()
            .enumerate()
            .map(|(index, fill)| {
                let fill = *fill;
                color_cell(("shading-fill", index), fill, fill_now.as_deref() == fill).on_click(on(
                    cx,
                    move |view, cx| {
                        view.edit_dialog(cx, |dialog| {
                            if let WordDialog::Borders(dialog) = dialog {
                                let pattern = dialog
                                    .shading
                                    .as_ref()
                                    .map_or("clear".to_string(), |shading| shading.pattern.clone());
                                dialog.shading = match (fill, pattern.as_str()) {
                                    (None, "clear") => None,
                                    _ => Some(ShadingSpec {
                                        pattern,
                                        color: None,
                                        fill: fill.map(str::to_string),
                                    }),
                                };
                            }
                        })
                    },
                ))
            })
            .collect::<Vec<_>>();
        let pattern_now = dialog
            .shading
            .as_ref()
            .map_or("clear".to_string(), |shading| shading.pattern.clone());
        let patterns = PATTERNS
            .iter()
            .enumerate()
            .map(|(index, (token, label))| {
                let token: &'static str = token;
                let text = if label.starts_with("word.") {
                    i18n::text(label)
                } else {
                    label
                };
                chip(("shading-pattern", index), text, pattern_now == token).on_click(on(
                    cx,
                    move |view, cx| {
                        view.edit_dialog(cx, |dialog| {
                            if let WordDialog::Borders(dialog) = dialog {
                                let fill = dialog
                                    .shading
                                    .as_ref()
                                    .and_then(|shading| shading.fill.clone());
                                dialog.shading =
                                    (token != "clear" || fill.is_some()).then(|| ShadingSpec {
                                        pattern: token.to_string(),
                                        color: (token != "clear").then(|| "000000".to_string()),
                                        fill,
                                    });
                            }
                        })
                    },
                ))
            })
            .collect::<Vec<_>>();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(section(i18n::text("word.borders.section")))
            .child(labeled(i18n::text("word.borders.kind")).child(presets))
            .child(labeled(i18n::text("word.borders.style")).children(styles))
            .child(labeled(i18n::text("word.borders.color")).children(colors))
            .child(labeled(i18n::text("word.borders.width")).children(widths))
            .child(
                div().flex().gap_3().child(preview).child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(side(
                            "border-top",
                            "word.borders.top",
                            dialog.borders.top.is_some(),
                            |b| &mut b.top,
                        ))
                        .child(side(
                            "border-bottom",
                            "word.borders.bottom",
                            dialog.borders.bottom.is_some(),
                            |b| &mut b.bottom,
                        ))
                        .child(side(
                            "border-left",
                            "word.borders.left",
                            dialog.borders.left.is_some(),
                            |b| &mut b.left,
                        ))
                        .child(side(
                            "border-right",
                            "word.borders.right",
                            dialog.borders.right.is_some(),
                            |b| &mut b.right,
                        ))
                        .child(side(
                            "border-between",
                            "word.borders.between",
                            dialog.borders.between.is_some(),
                            |b| &mut b.between,
                        )),
                ),
            )
            .child(
                labeled(i18n::text("word.borders.space"))
                    .child(self.dialog_field_element(0, 60.0, cx))
                    .child("pt"),
            )
            .child(section(i18n::text("word.shading.section")))
            .child(labeled(i18n::text("word.shading.fill")).children(fills))
            .child(labeled(i18n::text("word.shading.pattern")).children(patterns))
            .into_any_element()
    }

    pub(crate) fn apply_borders_dialog(
        &mut self,
        dialog: BordersDialog,
    ) -> Result<(), DialogError> {
        // A changed width, style or colour reaches every side drawn.
        let line = DocumentView::border_line(&dialog);
        let restyle = |side: &Option<BorderLine>| side.as_ref().map(|_| line.clone());
        let borders = BorderBox {
            top: restyle(&dialog.borders.top),
            bottom: restyle(&dialog.borders.bottom),
            left: restyle(&dialog.borders.left),
            right: restyle(&dialog.borders.right),
            between: restyle(&dialog.borders.between),
        };
        let state = self.doc().paragraph_options();
        if borders != state.borders {
            self.doc_mut().set_border_box(&borders)?;
        }
        if dialog.shading != state.shading {
            self.doc_mut().set_shading_spec(dialog.shading.as_ref())?;
        }
        Ok(())
    }

    pub(crate) fn open_drop_cap_dialog(&mut self, cx: &mut Context<Self>) {
        let state = self.doc().paragraph_options();
        self.tools.dialog = Some(WordDialog::DropCap(DropCapDialog {
            inputs: Inputs::new(vec![state
                .drop_cap
                .map_or(3, |(_, lines)| lines)
                .to_string()]),
            position: state.drop_cap.map(|(position, _)| position),
        }));
        cx.notify();
    }

    pub(crate) fn drop_cap_dialog_body(
        &self,
        dialog: &DropCapDialog,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let positions = [
            (None, "word.drop_cap.none"),
            (Some(DropCapPosition::Dropped), "word.drop_cap.dropped"),
            (Some(DropCapPosition::InMargin), "word.drop_cap.margin"),
        ];
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                labeled(i18n::text("word.drop_cap.position")).children(
                    positions
                        .into_iter()
                        .enumerate()
                        .map(|(index, (position, label))| {
                            chip(
                                ("drop-cap-position", index),
                                i18n::text(label),
                                dialog.position == position,
                            )
                            .on_click(on(cx, move |view, cx| {
                                view.edit_dialog(cx, |dialog| {
                                    if let WordDialog::DropCap(dialog) = dialog {
                                        dialog.position = position;
                                    }
                                })
                            }))
                        }),
                ),
            )
            .child(
                labeled(i18n::text("word.drop_cap.lines"))
                    .child(self.dialog_field_element(0, 60.0, cx)),
            )
            .into_any_element()
    }

    pub(crate) fn apply_drop_cap_dialog(
        &mut self,
        dialog: DropCapDialog,
    ) -> Result<(), DialogError> {
        let lines: u8 = match dialog.inputs.text(0).trim().parse() {
            Ok(lines) if (1..=10).contains(&lines) => lines,
            _ => {
                return Err(DialogError::Stay(
                    Box::new(WordDialog::DropCap(dialog)),
                    i18n::text("word.error.lines").to_string(),
                ))
            }
        };
        self.doc_mut()
            .set_drop_cap(dialog.position.map(|position| (position, lines)))
            .map_err(DialogError::Failed)
    }
}

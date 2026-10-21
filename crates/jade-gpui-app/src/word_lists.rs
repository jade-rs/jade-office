//! Lists: the 箇条書き, 段落番号 and アウトライン libraries under their
//! buttons, with リストのレベルの変更, 番号の設定 (start a new list or carry
//! on the one before, from a value), リストのインデントの調整, 新しい
//! アウトラインの定義 — each of the nine levels' number format (Japanese
//! ones included), the text around the number, its start, alignment and
//! indents, what follows it, 正規形式の番号を使用する and the heading style it
//! is linked to — and 新しいリスト スタイルの定義. The calls are
//! `jade-office-model`'s `WordLists`, which the browser shell shares.

use gpui::{div, prelude::*, px, AnyElement, Context};
use jade_office_model::word::lists::{
    list_styles, ListDefinition, ListLevel, MultilevelPreset, Suffix, WordLists, BULLETS,
    NUMBER_FORMATS,
};
use jade_office_model::word::ruler::{format_length, parse_length};
use jade_office_model::word::styles::{display_name, WordStyles};

use crate::ribbon::{dropdown, menu_caption, menu_panel, menu_row, Action, Popup};
use crate::word_dialog::{check, chip, labeled, on, section, DialogError, Inputs, WordDialog};
use crate::word_tools::{ToolCommand, ToolMenu};
use crate::{i18n, theme, DocumentView};

/// The number formats the define dialog offers, with what they look like.
const FORMATS: [(&str, &str); 16] = [
    ("decimal", "1, 2, 3"),
    ("decimalFullWidth", "１, ２, ３"),
    ("decimalEnclosedCircle", "①, ②, ③"),
    ("decimalZero", "01, 02, 03"),
    ("japaneseCounting", "一, 二, 三"),
    ("japaneseLegal", "壱, 弐, 参"),
    ("aiueoFullWidth", "ア, イ, ウ"),
    ("irohaFullWidth", "イ, ロ, ハ"),
    ("ideographTraditional", "甲, 乙, 丙"),
    ("ideographZodiac", "子, 丑, 寅"),
    ("upperRoman", "I, II, III"),
    ("lowerRoman", "i, ii, iii"),
    ("upperLetter", "A, B, C"),
    ("lowerLetter", "a, b, c"),
    ("bullet", "●"),
    ("none", "—"),
];

/// 番号の設定.
pub struct ListStartDialog {
    pub inputs: Inputs,
    /// 前のリストから継続 rather than 新しいリストを開始.
    pub continue_previous: bool,
}

/// リストのインデントの調整: number position (field 0), text indent
/// (field 1), and what follows the number.
pub struct ListIndentDialog {
    pub inputs: Inputs,
    pub suffix: Suffix,
}

/// 新しいアウトラインの定義, or with a name 新しいリスト スタイルの定義.
/// Fields: 0 the level's text, 1 its start, 2 the number's position, 3 the
/// text's indent, 4 the list style's name.
pub struct DefineListDialog {
    pub inputs: Inputs,
    pub definition: ListDefinition,
    pub level: usize,
    pub style_name: Option<String>,
    /// The heading styles a level can be linked to: id and label.
    pub headings: Vec<(String, String)>,
}

fn suffix_label(suffix: Suffix) -> &'static str {
    i18n::text(match suffix {
        Suffix::Tab => "word.list.suffix_tab",
        Suffix::Space => "word.list.suffix_space",
        Suffix::Nothing => "word.list.suffix_nothing",
    })
}

fn preset_label(preset: MultilevelPreset) -> &'static str {
    i18n::text(match preset {
        MultilevelPreset::DecimalOutline => "word.list.preset.outline",
        MultilevelPreset::Parenthesised => "word.list.preset.paren",
        MultilevelPreset::Bullets => "word.list.preset.bullets",
        MultilevelPreset::NumberedHeadings => "word.list.preset.headings",
        MultilevelPreset::Chapters => "word.list.preset.chapters",
        MultilevelPreset::Articles => "word.list.preset.articles",
    })
}

impl DefineListDialog {
    /// The selected level as the fields have it.
    fn store_level(&mut self, unit: jade_office_model::word::ruler::RulerUnit, char_width: f32) {
        let level = &mut self.definition.levels[self.level];
        let text = self.inputs.text(0).to_string();
        if !text.is_empty() {
            level.text = text;
        }
        if let Ok(start) = self.inputs.text(1).trim().parse::<u32>() {
            level.start = start.clamp(0, 32_767);
        }
        let position = parse_length(unit, char_width, self.inputs.text(2));
        let indent = parse_length(unit, char_width, self.inputs.text(3));
        if let Some(indent) = indent {
            level.indent = indent.max(0.0);
        }
        if let Some(position) = position {
            level.hanging = (level.indent - position).max(0.0);
        }
        if self.style_name.is_some() {
            self.style_name = Some(self.inputs.text(4).to_string());
        }
    }

    fn load_level(
        &mut self,
        index: usize,
        unit: jade_office_model::word::ruler::RulerUnit,
        char_width: f32,
    ) {
        self.level = index;
        let level = &self.definition.levels[index];
        self.inputs.set(0, level.text.clone());
        self.inputs.set(1, level.start.to_string());
        self.inputs.set(
            2,
            format_length(unit, char_width, level.indent - level.hanging),
        );
        self.inputs
            .set(3, format_length(unit, char_width, level.indent));
    }
}

impl DocumentView {
    /// ホーム → the list libraries, リストのレベル, 均等割り付け and 段落's
    /// settings, as a group beside 段落.
    pub(crate) fn list_group(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let popup = self.ribbon.popup;
        let bullets_open = popup == Some(Popup::Word(ToolMenu::Bullets));
        let numbering_open = popup == Some(Popup::Word(ToolMenu::Numbering));
        let multilevel_open = popup == Some(Popup::Word(ToolMenu::Multilevel));
        let in_list = if bullets_open || numbering_open || multilevel_open {
            self.tools_caret();
            self.document.list_state()
        } else {
            None
        };
        let bullets = bullets_open.then(|| {
            menu_panel("bullets-menu")
                .w(px(220.0))
                .child(menu_caption(i18n::text("word.list.bullet_library")))
                .child(div().flex().flex_wrap().gap_1().p_1().children(
                    BULLETS.iter().enumerate().map(|(index, (glyph, _))| {
                        crate::word_dialog::chip(("bullet", index), *glyph, false)
                            .on_click(on(cx, move |view, cx| {
                                view.run_tool(ToolCommand::Bullet(index), cx)
                            }))
                    }),
                ))
                .child(menu_row(
                    "bullet-picture",
                    i18n::text("word.list.picture_bullet"),
                    false,
                    Action::Word(ToolCommand::PictureBullet),
                    cx,
                ))
                .child(menu_row(
                    "bullets-none",
                    i18n::text("doc.none"),
                    false,
                    Action::List(true),
                    cx,
                ))
                .into_any_element()
        });
        let numbering = numbering_open.then(|| {
            let mut panel = menu_panel("numbering-menu")
                .w(px(240.0))
                .child(menu_caption(i18n::text("word.list.number_library")))
                .child(div().flex().flex_wrap().gap_1().p_1().children(
                    NUMBER_FORMATS.iter().enumerate().map(|(index, format)| {
                        let level = ListLevel::numbered(0, format.format, format.text);
                        let sample = format!(
                            "{} {}",
                            level.preview(&[1], &[format.format.to_string()]),
                            level.preview(&[2], &[format.format.to_string()])
                        );
                        crate::word_dialog::chip(("number-format", index), sample, false).on_click(
                            on(cx, move |view, cx| {
                                view.run_tool(ToolCommand::NumberFormat(index), cx)
                            }),
                        )
                    }),
                ));
            if in_list.is_some() {
                panel = panel
                    .child(menu_row(
                        "list-restart",
                        i18n::text("word.list.restart"),
                        false,
                        Action::Word(ToolCommand::RestartList),
                        cx,
                    ))
                    .child(menu_row(
                        "list-continue",
                        i18n::text("word.list.continue"),
                        false,
                        Action::Word(ToolCommand::ContinueList),
                        cx,
                    ))
                    .child(menu_row(
                        "list-start",
                        i18n::text("word.list.set_value"),
                        false,
                        Action::Word(ToolCommand::OpenListStart),
                        cx,
                    ))
                    .child(menu_row(
                        "list-indent",
                        i18n::text("word.list.adjust_indent"),
                        false,
                        Action::Word(ToolCommand::OpenListIndent),
                        cx,
                    ));
            }
            panel.into_any_element()
        });
        let multilevel = multilevel_open.then(|| {
            let styles = self
                .document
                .word_document()
                .related_part_xml("styles")
                .map(|xml| list_styles(&xml))
                .unwrap_or_default();
            let mut panel =
                menu_panel("multilevel-menu")
                    .w(px(280.0))
                    .max_h(px(480.0))
                    .overflow_y_scroll()
                    .child(menu_caption(i18n::text("word.list.multilevel_library")))
                    .children(MultilevelPreset::ALL.into_iter().enumerate().map(
                        |(index, preset)| {
                            menu_row(
                                ("multilevel", index),
                                preset_label(preset),
                                false,
                                Action::Word(ToolCommand::Multilevel(preset)),
                                cx,
                            )
                        },
                    ));
            if !styles.is_empty() {
                panel = panel.child(menu_caption(i18n::text("word.list.list_styles")));
                for (index, (_, name, num_id)) in styles.into_iter().enumerate() {
                    panel = panel.child(menu_row(
                        ("list-style", index),
                        name,
                        false,
                        Action::Word(ToolCommand::ApplyListStyle(num_id)),
                        cx,
                    ));
                }
            }
            if let Some(state) = in_list.as_ref() {
                panel = panel.child(menu_caption(i18n::text("word.list.change_level")));
                for level in 0..9u8 {
                    let preview = state.definition.preview(level as usize);
                    panel = panel.child(menu_row(
                        ("list-level", level as usize),
                        i18n::format(
                            "word.list.level",
                            &[("n", &(level + 1)), ("sample", &preview)],
                        ),
                        state.level == level,
                        Action::Word(ToolCommand::ListLevel(level)),
                        cx,
                    ));
                }
            }
            panel
                .child(menu_row(
                    "define-list",
                    i18n::text("word.define_list.menu"),
                    false,
                    Action::Word(ToolCommand::OpenDefineList),
                    cx,
                ))
                .child(menu_row(
                    "define-list-style",
                    i18n::text("word.list_style.menu"),
                    false,
                    Action::Word(ToolCommand::OpenListStyle),
                    cx,
                ))
                .into_any_element()
        });
        let distributed = self.format_state().alignment.as_deref() == Some("distribute");
        let settings = self.paragraph_settings_menu(cx);
        crate::ribbon::group(
            i18n::text("word.group.lists"),
            vec![
                crate::ribbon::row(vec![
                    dropdown(
                        crate::ribbon::menu_button(
                            "bullet-library",
                            "icons/list.svg",
                            i18n::text("word.list.bullet_library"),
                            None,
                            bullets_open,
                            Popup::Word(ToolMenu::Bullets),
                            cx,
                        ),
                        bullets,
                    ),
                    dropdown(
                        crate::ribbon::menu_button(
                            "number-library",
                            "icons/list-ordered.svg",
                            i18n::text("word.list.number_library"),
                            None,
                            numbering_open,
                            Popup::Word(ToolMenu::Numbering),
                            cx,
                        ),
                        numbering,
                    ),
                    dropdown(
                        crate::ribbon::menu_button(
                            "multilevel-library",
                            "icons/list-tree.svg",
                            i18n::text("word.list.multilevel_library"),
                            None,
                            multilevel_open,
                            Popup::Word(ToolMenu::Multilevel),
                            cx,
                        ),
                        multilevel,
                    ),
                ]),
                crate::ribbon::row(vec![
                    crate::ribbon::icon_button(
                        "align-distribute",
                        "icons/align-distribute.svg",
                        i18n::text("word.align.distribute"),
                        distributed,
                        true,
                        Action::Align("distribute"),
                        cx,
                    ),
                    settings,
                ]),
            ],
        )
    }

    /// 箇条書き → 図: asks for a picture and makes it the bullet.
    pub(crate) fn prompt_for_picture_bullet(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(i18n::text("word.list.picture_bullet").into()),
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
            let extension = path
                .extension()
                .map(|extension| extension.to_string_lossy().to_string())
                .unwrap_or_default();
            view.update(cx, |view, cx| {
                view.set_picture_bullet(&bytes, &extension, cx)
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn set_picture_bullet(
        &mut self,
        bytes: &[u8],
        extension: &str,
        cx: &mut Context<Self>,
    ) {
        self.tools_caret();
        let result = self.document.apply_picture_bullet(bytes, extension);
        self.tool_done(result, cx);
    }

    pub(crate) fn open_list_start_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.document.list_state() else {
            return self.tool_done(Err(i18n::text("word.error.not_in_list").to_string()), cx);
        };
        self.tools.dialog = Some(WordDialog::ListStart(ListStartDialog {
            inputs: Inputs::new(vec![state.start.to_string()]),
            continue_previous: false,
        }));
        cx.notify();
    }

    pub(crate) fn list_start_dialog_body(
        &self,
        dialog: &ListStartDialog,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let choice = |id: &'static str, label: &'static str, value: bool| {
            chip(id, i18n::text(label), dialog.continue_previous == value).on_click(on(
                cx,
                move |view, cx| {
                    view.edit_dialog(cx, |dialog| {
                        if let WordDialog::ListStart(dialog) = dialog {
                            dialog.continue_previous = value;
                        }
                    })
                },
            ))
        };
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(choice("list-new", "word.list.start_new", false))
            .child(choice("list-continue-previous", "word.list.continue", true))
            .child(
                labeled(i18n::text("word.list.value"))
                    .child(self.dialog_field_element(0, 80.0, cx)),
            )
            .into_any_element()
    }

    pub(crate) fn apply_list_start_dialog(
        &mut self,
        dialog: ListStartDialog,
    ) -> Result<(), DialogError> {
        if dialog.continue_previous {
            return self.document.continue_list().map_err(DialogError::Failed);
        }
        match dialog.inputs.text(0).trim().parse::<u32>() {
            Ok(value) if (1..=32_767).contains(&value) => self
                .document
                .set_list_start(value)
                .map_err(DialogError::Failed),
            _ => Err(DialogError::Stay(
                Box::new(WordDialog::ListStart(dialog)),
                i18n::text("word.error.value").to_string(),
            )),
        }
    }

    pub(crate) fn open_list_indent_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.document.list_state() else {
            return self.tool_done(Err(i18n::text("word.error.not_in_list").to_string()), cx);
        };
        let level = &state.definition.levels[state.level as usize];
        let unit = self.tools.unit;
        let char_width = self.char_width();
        self.tools.dialog = Some(WordDialog::ListIndent(ListIndentDialog {
            inputs: Inputs::new(vec![
                format_length(unit, char_width, level.indent - level.hanging),
                format_length(unit, char_width, level.indent),
            ]),
            suffix: level.suffix,
        }));
        cx.notify();
    }

    pub(crate) fn list_indent_dialog_body(
        &self,
        dialog: &ListIndentDialog,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                labeled(i18n::text("word.list.number_position"))
                    .child(self.dialog_field_element(0, 100.0, cx)),
            )
            .child(
                labeled(i18n::text("word.list.text_indent"))
                    .child(self.dialog_field_element(1, 100.0, cx)),
            )
            .child(labeled(i18n::text("word.list.suffix")).children(
                Suffix::ALL.into_iter().enumerate().map(|(index, suffix)| {
                    chip(
                        ("list-suffix", index),
                        suffix_label(suffix),
                        dialog.suffix == suffix,
                    )
                    .on_click(on(cx, move |view, cx| {
                        view.edit_dialog(cx, |dialog| {
                            if let WordDialog::ListIndent(dialog) = dialog {
                                dialog.suffix = suffix;
                            }
                        })
                    }))
                }),
            ))
            .into_any_element()
    }

    pub(crate) fn apply_list_indent_dialog(
        &mut self,
        dialog: ListIndentDialog,
    ) -> Result<(), DialogError> {
        let unit = self.tools.unit;
        let char_width = self.char_width();
        let (Some(position), Some(indent)) = (
            parse_length(unit, char_width, dialog.inputs.text(0)),
            parse_length(unit, char_width, dialog.inputs.text(1)),
        ) else {
            return Err(DialogError::Stay(
                Box::new(WordDialog::ListIndent(dialog)),
                i18n::text("word.error.length").to_string(),
            ));
        };
        let suffix = dialog.suffix;
        self.document
            .edit_list_level(move |level| {
                level.indent = indent.max(0.0);
                level.hanging = (indent - position).max(0.0);
                level.suffix = suffix;
            })
            .map_err(DialogError::Failed)
    }

    /// 新しいアウトラインの定義, from the caret's list (or the outline
    /// preset); with `style` 新しいリスト スタイルの定義.
    pub(crate) fn open_define_list_dialog(
        &mut self,
        style: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let state = self.document.list_state();
        let catalog = self.document.style_catalog();
        let japanese = i18n::language() == i18n::Language::Japanese;
        let headings: Vec<(String, String)> = (1..=9)
            .map(|level| {
                let name = format!("heading {level}");
                let id = catalog
                    .iter()
                    .find(|style| style.name.eq_ignore_ascii_case(&name))
                    .map(|style| style.id.clone())
                    .unwrap_or_else(|| format!("Heading{level}"));
                (id, display_name(&name, japanese))
            })
            .collect();
        let definition = state
            .as_ref()
            .map(|state| state.definition.clone())
            .unwrap_or_else(|| MultilevelPreset::DecimalOutline.definition(&|_| None));
        let level = state.as_ref().map_or(0, |state| state.level as usize);
        let mut values = vec![String::new(); 4];
        if style.is_some() {
            values.push(String::new());
        }
        let mut dialog = DefineListDialog {
            inputs: Inputs::new(values),
            definition,
            level,
            style_name: style,
            headings,
        };
        let unit = self.tools.unit;
        let char_width = self.char_width();
        dialog.load_level(level, unit, char_width);
        if dialog.style_name.is_some() {
            dialog.inputs.focus = Some(4);
        }
        self.tools.dialog = Some(WordDialog::DefineList(dialog));
        cx.notify();
    }

    fn with_define_dialog(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut DefineListDialog, jade_office_model::word::ruler::RulerUnit, f32),
    ) {
        let unit = self.tools.unit;
        let char_width = self.char_width();
        self.edit_dialog(cx, |dialog| {
            if let WordDialog::DefineList(dialog) = dialog {
                dialog.store_level(unit, char_width);
                edit(dialog, unit, char_width);
            }
        });
    }

    pub(crate) fn define_list_dialog_body(
        &self,
        dialog: &DefineListDialog,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let level = &dialog.definition.levels[dialog.level];
        let levels = (0..9)
            .map(|index| {
                chip(
                    ("define-level", index),
                    (index + 1).to_string(),
                    dialog.level == index,
                )
                .on_click(on(cx, move |view, cx| {
                    view.with_define_dialog(cx, |dialog, unit, char_width| {
                        dialog.load_level(index, unit, char_width)
                    })
                }))
            })
            .collect::<Vec<_>>();
        let formats = FORMATS
            .iter()
            .enumerate()
            .map(|(index, (format, sample))| {
                let format: &'static str = format;
                chip(("define-format", index), *sample, level.format == format).on_click(on(
                    cx,
                    move |view, cx| {
                        view.with_define_dialog(cx, |dialog, _, _| {
                            let level = &mut dialog.definition.levels[dialog.level];
                            level.format = format.to_string();
                            if format == "bullet" {
                                level.text = "●".to_string();
                                dialog.inputs.set(0, level.text.clone());
                            } else if level.text == "●" || !level.text.contains('%') {
                                level.text = format!("%{}.", dialog.level + 1);
                                dialog.inputs.set(0, level.text.clone());
                            }
                        })
                    },
                ))
            })
            .collect::<Vec<_>>();
        let alignments = ["left", "center", "right"]
            .into_iter()
            .enumerate()
            .map(|(index, alignment)| {
                let label = i18n::text(match alignment {
                    "left" => "word.tab.left",
                    "center" => "word.tab.center",
                    _ => "word.tab.right",
                });
                chip(
                    ("define-alignment", index),
                    label,
                    level.alignment == alignment,
                )
                .on_click(on(cx, move |view, cx| {
                    view.with_define_dialog(cx, |dialog, _, _| {
                        dialog.definition.levels[dialog.level].alignment = alignment.to_string();
                    })
                }))
            })
            .collect::<Vec<_>>();
        let suffixes = Suffix::ALL
            .into_iter()
            .enumerate()
            .map(|(index, suffix)| {
                chip(
                    ("define-suffix", index),
                    suffix_label(suffix),
                    level.suffix == suffix,
                )
                .on_click(on(cx, move |view, cx| {
                    view.with_define_dialog(cx, |dialog, _, _| {
                        dialog.definition.levels[dialog.level].suffix = suffix;
                    })
                }))
            })
            .collect::<Vec<_>>();
        let mut links = vec![chip(
            "define-link-none",
            i18n::text("doc.none"),
            level.style.is_none(),
        )
        .on_click(on(cx, |view, cx| {
            view.with_define_dialog(cx, |dialog, _, _| {
                dialog.definition.levels[dialog.level].style = None;
            })
        }))];
        for (index, (id, label)) in dialog.headings.iter().enumerate() {
            let id = id.clone();
            let on_now = level.style.as_deref() == Some(id.as_str());
            links.push(
                chip(("define-link", index), label.clone(), on_now).on_click(on(
                    cx,
                    move |view, cx| {
                        let id = id.clone();
                        view.with_define_dialog(cx, move |dialog, _, _| {
                            dialog.definition.levels[dialog.level].style = Some(id);
                        })
                    },
                )),
            );
        }
        let legal = level.legal;
        // The preview: every level's number at its indent.
        let preview = div()
            .flex()
            .flex_col()
            .p_2()
            .bg(theme::paper())
            .text_color(gpui::rgb(0x202020))
            .children((0..9).map(|index| {
                let entry = &dialog.definition.levels[index];
                div()
                    .flex()
                    .gap_1()
                    .pl(px((entry.indent - entry.hanging).max(0.0) * 0.6))
                    .when(index == dialog.level, |row| {
                        row.font_weight(gpui::FontWeight::BOLD)
                    })
                    .child(dialog.definition.preview(index))
                    .child(div().text_color(gpui::rgb(0xa0a0a0)).child("────"))
            }));
        let mut body = div().flex().flex_col().gap_2();
        if dialog.style_name.is_some() {
            body = body.child(
                labeled(i18n::text("word.list_style.name"))
                    .child(self.dialog_field_element(4, 220.0, cx)),
            );
        }
        body.child(labeled(i18n::text("word.define_list.level")).children(levels))
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .w(px(380.0))
                            .child(section(i18n::text("word.define_list.format_section")))
                            .child(
                                labeled(i18n::text("word.define_list.text"))
                                    .child(self.dialog_field_element(0, 160.0, cx)),
                            )
                            .child(labeled(i18n::text("word.define_list.format")).children(formats))
                            .child(
                                labeled(i18n::text("word.define_list.start"))
                                    .child(self.dialog_field_element(1, 80.0, cx)),
                            )
                            .child(
                                check("define-legal", i18n::text("word.define_list.legal"), legal)
                                    .on_click(on(cx, |view, cx| {
                                        view.with_define_dialog(cx, |dialog, _, _| {
                                            let level = &mut dialog.definition.levels[dialog.level];
                                            level.legal = !level.legal;
                                        })
                                    })),
                            )
                            .child(section(i18n::text("word.define_list.position_section")))
                            .child(
                                labeled(i18n::text("word.define_list.alignment"))
                                    .children(alignments),
                            )
                            .child(
                                labeled(i18n::text("word.list.number_position"))
                                    .child(self.dialog_field_element(2, 100.0, cx)),
                            )
                            .child(
                                labeled(i18n::text("word.list.text_indent"))
                                    .child(self.dialog_field_element(3, 100.0, cx)),
                            )
                            .child(labeled(i18n::text("word.list.suffix")).children(suffixes))
                            .child(labeled(i18n::text("word.define_list.link")).children(links)),
                    )
                    .child(div().flex_1().child(preview)),
            )
            .into_any_element()
    }

    pub(crate) fn apply_define_list_dialog(
        &mut self,
        mut dialog: DefineListDialog,
    ) -> Result<(), DialogError> {
        let unit = self.tools.unit;
        let char_width = self.char_width();
        dialog.store_level(unit, char_width);
        if let Some(name) = dialog.style_name.clone() {
            if name.trim().is_empty() {
                return Err(DialogError::Stay(
                    Box::new(WordDialog::DefineList(dialog)),
                    i18n::text("word.error.name").to_string(),
                ));
            }
            return self
                .document
                .create_list_style(&name, &dialog.definition)
                .map(|_| ())
                .map_err(DialogError::Failed);
        }
        // The selection at its own levels: a list of this definition.
        self.document
            .apply_list_definition(&dialog.definition, None)
            .map_err(DialogError::Failed)
    }
}

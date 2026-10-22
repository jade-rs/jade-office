//! Lists: the 箇条書き, 段落番号 and アウトライン libraries under their
//! buttons, with リストのレベルの変更, 番号の設定 (start a new list or carry
//! on the one before, from a value), リストのインデントの調整, 新しい
//! アウトラインの定義 — each of the nine levels' number format (Japanese
//! ones included), the text around the number, its start, alignment and
//! indents, what follows it, 正規形式の番号を使用する and the heading style it
//! is linked to — and 新しいリスト スタイルの定義: the browser's copy of
//! the native app's `word_lists` dialogs, over `jade-office-model`'s
//! `WordLists`. The libraries' menus are `word_tools`'.

use gpui::{div, prelude::*, px, AnyElement, Context};
use jade_web::word::lists::{ListDefinition, MultilevelPreset, Suffix, WordLists};
use jade_web::word::ruler::{format_length, parse_length};
use jade_web::word::styles::{display_name, WordStyles};

use super::word_dialog::{check, chip, labeled, on, section, DialogError, Inputs, WordDialog};
use super::DocView as DocumentView;
use crate::i18n;

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

impl DefineListDialog {
    /// The selected level as the fields have it.
    fn store_level(&mut self, unit: jade_web::word::ruler::RulerUnit, char_width: f32) {
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
        unit: jade_web::word::ruler::RulerUnit,
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
    /// 箇条書き → 図: asks the page for a picture, which comes back
    /// through `open_file` and becomes the bullet.
    pub(crate) fn prompt_for_picture_bullet(&mut self, _cx: &mut Context<Self>) {
        self.tools.picking = Some(super::word_tools_ui::Pick::PictureBullet);
        self.host.pick();
    }

    pub(crate) fn set_picture_bullet(
        &mut self,
        bytes: &[u8],
        extension: &str,
        cx: &mut Context<Self>,
    ) {
        let result = self.doc_mut().apply_picture_bullet(bytes, extension);
        self.tool_done(result, cx);
    }

    pub(crate) fn open_list_start_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.doc().list_state() else {
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
            return self.doc_mut().continue_list().map_err(DialogError::Failed);
        }
        match dialog.inputs.text(0).trim().parse::<u32>() {
            Ok(value) if (1..=32_767).contains(&value) => self
                .doc_mut()
                .set_list_start(value)
                .map_err(DialogError::Failed),
            _ => Err(DialogError::Stay(
                Box::new(WordDialog::ListStart(dialog)),
                i18n::text("word.error.value").to_string(),
            )),
        }
    }

    pub(crate) fn open_list_indent_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.doc().list_state() else {
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
        self.doc_mut()
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
        let state = self.doc().list_state();
        let catalog = self.doc().style_catalog();
        let japanese = !i18n::is_english();
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
        edit: impl FnOnce(&mut DefineListDialog, jade_web::word::ruler::RulerUnit, f32),
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
            .bg(gpui::rgb(0xffffff))
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
                .doc_mut()
                .create_list_style(&name, &dialog.definition)
                .map(|_| ())
                .map_err(DialogError::Failed);
        }
        // The selection at its own levels: a list of this definition.
        self.doc_mut()
            .apply_list_definition(&dialog.definition, None)
            .map_err(DialogError::Failed)
    }
}

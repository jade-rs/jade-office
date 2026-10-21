//! The デザイン tab's ドキュメントの書式設定 — テーマ, the style sets, 配色,
//! フォント, 効果, 段落の間隔 and 既定に設定 — and テンプレート (attaching a
//! template, its styles copied in); and 挿入's クイック パーツ: the selection
//! saved into the クイック パーツ or 定型句 gallery under a name, and an entry
//! put back in at the caret. The calls are `jade-office-model`'s
//! `WordDesign`, `WordStyles` and `WordBuildingBlocks`.

use gpui::{div, prelude::*, px, rgb, AnyElement, Context};
use jade_office_model::word::building_blocks::{Gallery, WordBuildingBlocks};
use jade_office_model::word::design::{
    style_sets, WordDesign, COLOR_SCHEMES, EFFECT_SCHEMES, FONT_SCHEMES, PARAGRAPH_SPACINGS, THEMES,
};
use jade_office_model::word::styles::WordStyles;

use crate::ribbon::{dropdown, menu_caption, menu_panel, menu_row, Action, Popup};
use crate::word_dialog::{chip, labeled, on, DialogError, Inputs, WordDialog};
use crate::word_tools::{ToolCommand, ToolMenu};
use crate::{i18n, DocumentView};

/// クイック パーツの保存: the name (field 0) and the gallery.
pub struct QuickPartDialog {
    pub inputs: Inputs,
    pub gallery: Gallery,
}

fn swatches(colors: &[&str]) -> gpui::Div {
    div()
        .flex()
        .gap_0p5()
        .children(colors.iter().filter_map(|hex| {
            u32::from_str_radix(hex, 16)
                .ok()
                .map(|color| div().size(px(10.0)).bg(rgb(color)))
        }))
}

impl DocumentView {
    /// デザイン → ドキュメントの書式設定.
    pub(crate) fn design_groups(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let popup = self.ribbon.popup;
        let state = if matches!(popup, Some(Popup::Word(_))) {
            self.document.design_state()
        } else {
            Default::default()
        };
        let is_open = |menu| popup == Some(Popup::Word(menu));
        let themes = is_open(ToolMenu::Themes).then(|| {
            menu_panel("themes-menu")
                .w(px(260.0))
                .children(THEMES.iter().enumerate().map(|(index, theme)| {
                    let current = state.theme.as_deref() == Some(theme.name);
                    div()
                        .id(("theme", index))
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .px_2()
                        .h(px(30.0))
                        .rounded_md()
                        .cursor_pointer()
                        .when(current, |row| row.bg(crate::theme::selected()))
                        .hover(|row| row.bg(crate::theme::hovered()))
                        .on_click(crate::ribbon::listener(
                            Action::Word(ToolCommand::Theme(index)),
                            cx,
                        ))
                        .child(div().child(theme.name).child(
                            div().text_color(crate::theme::text_faint()).child(format!(
                                "{} / {}",
                                theme.fonts.major_latin, theme.fonts.minor_latin
                            )),
                        ))
                        .child(swatches(&theme.colors.accents))
                }))
                .into_any_element()
        });
        let colors = is_open(ToolMenu::Colors).then(|| {
            menu_panel("colors-menu")
                .w(px(260.0))
                .children(COLOR_SCHEMES.iter().enumerate().map(|(index, scheme)| {
                    let current = state.colors.as_deref() == Some(scheme.name);
                    div()
                        .id(("colors", index))
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .h(px(24.0))
                        .rounded_md()
                        .cursor_pointer()
                        .when(current, |row| row.bg(crate::theme::selected()))
                        .hover(|row| row.bg(crate::theme::hovered()))
                        .on_click(crate::ribbon::listener(
                            Action::Word(ToolCommand::Colors(index)),
                            cx,
                        ))
                        .child(
                            swatches(&[scheme.dark2, scheme.light2])
                                .child(swatches(&scheme.accents)),
                        )
                        .child(scheme.name)
                }))
                .into_any_element()
        });
        let fonts = is_open(ToolMenu::Fonts).then(|| {
            menu_panel("fonts-menu")
                .w(px(280.0))
                .children(FONT_SCHEMES.iter().enumerate().map(|(index, scheme)| {
                    menu_row(
                        ("fonts", index),
                        format!(
                            "{}  —  {} / {}  ({} / {})",
                            scheme.name,
                            scheme.major_latin,
                            scheme.minor_latin,
                            scheme.major_east_asia,
                            scheme.minor_east_asia
                        ),
                        state.fonts.as_deref() == Some(scheme.name),
                        Action::Word(ToolCommand::Fonts(index)),
                        cx,
                    )
                }))
                .into_any_element()
        });
        let effects = is_open(ToolMenu::Effects).then(|| {
            menu_panel("effects-menu")
                .w(px(200.0))
                .children(EFFECT_SCHEMES.iter().enumerate().map(|(index, scheme)| {
                    menu_row(
                        ("effects", index),
                        scheme.name,
                        state.effects.as_deref() == Some(scheme.name),
                        Action::Word(ToolCommand::Effects(index)),
                        cx,
                    )
                }))
                .into_any_element()
        });
        let japanese = i18n::language() == i18n::Language::Japanese;
        let sets = is_open(ToolMenu::StyleSets).then(|| {
            menu_panel("style-sets-menu")
                .w(px(240.0))
                .children(style_sets().iter().enumerate().map(|(index, set)| {
                    menu_row(
                        ("style-set", index),
                        if japanese { set.label_ja } else { set.name },
                        false,
                        Action::Word(ToolCommand::StyleSet(index)),
                        cx,
                    )
                }))
                .into_any_element()
        });
        let spacing = is_open(ToolMenu::Spacing).then(|| {
            menu_panel("spacing-sets-menu")
                .w(px(220.0))
                .children(PARAGRAPH_SPACINGS.iter().enumerate().map(
                    |(index, (key, label, _, after, line))| {
                        menu_row(
                            ("paragraph-spacing", index),
                            format!(
                                "{}  ({} pt, {}×)",
                                if japanese {
                                    label.to_string()
                                } else {
                                    key.to_string()
                                },
                                after,
                                line
                            ),
                            false,
                            Action::Word(ToolCommand::Spacing(index)),
                            cx,
                        )
                    },
                ))
                .into_any_element()
        });
        let button =
            |id: &'static str, label: &'static str, menu: ToolMenu, cx: &mut Context<Self>| {
                crate::word_page::list_button(
                    id,
                    i18n::text(label),
                    popup == Some(Popup::Word(menu)),
                    Popup::Word(menu),
                    cx,
                )
            };
        let format_group = crate::ribbon::group(
            i18n::text("word.group.document_format"),
            vec![
                crate::ribbon::row(vec![
                    dropdown(
                        button("themes", "word.design.themes", ToolMenu::Themes, cx),
                        themes,
                    ),
                    dropdown(
                        button(
                            "style-sets",
                            "word.design.style_sets",
                            ToolMenu::StyleSets,
                            cx,
                        ),
                        sets,
                    ),
                    dropdown(
                        button("colors", "word.design.colors", ToolMenu::Colors, cx),
                        colors,
                    ),
                ]),
                crate::ribbon::row(vec![
                    dropdown(
                        button("fonts", "word.design.fonts", ToolMenu::Fonts, cx),
                        fonts,
                    ),
                    dropdown(
                        button("effects", "word.design.effects", ToolMenu::Effects, cx),
                        effects,
                    ),
                    dropdown(
                        button("spacing", "word.design.spacing", ToolMenu::Spacing, cx),
                        spacing,
                    ),
                ]),
            ],
        );
        let text_button = |id: &'static str,
                           label: &'static str,
                           command: ToolCommand,
                           cx: &mut Context<Self>| {
            div()
                .id(id)
                .flex()
                .items_center()
                .h(px(26.0))
                .px_2()
                .rounded_md()
                .text_xs()
                .cursor_pointer()
                .hover(|button| button.bg(crate::theme::hovered()))
                .on_click(crate::ribbon::listener(Action::Word(command), cx))
                .child(i18n::text(label))
                .into_any_element()
        };
        let default_group = crate::ribbon::group(
            i18n::text("word.group.defaults"),
            vec![
                crate::ribbon::row(vec![text_button(
                    "set-as-default",
                    "word.design.set_default",
                    ToolCommand::SetAsDefault,
                    cx,
                )]),
                crate::ribbon::row(vec![text_button(
                    "attach-template",
                    "word.design.template",
                    ToolCommand::AttachTemplate,
                    cx,
                )]),
            ],
        );
        vec![format_group, default_group]
    }

    /// テンプレートとアドイン: a template file, its styles copied in.
    pub(crate) fn prompt_for_template(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(i18n::text("word.design.template").into()),
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
            view.update(cx, |view, cx| {
                view.attach_template_bytes(&bytes, &path.to_string_lossy(), cx)
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn attach_template_bytes(
        &mut self,
        bytes: &[u8],
        path: &str,
        cx: &mut Context<Self>,
    ) {
        self.tools_caret();
        let result = self.document.attach_template(bytes, path, true).map(|_| ());
        self.tool_done(result, cx);
    }

    /// 挿入 → クイック パーツ.
    pub(crate) fn quick_parts_group(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let open = self.ribbon.popup == Some(Popup::Word(ToolMenu::QuickParts));
        let menu = open.then(|| {
            let blocks = self.document.building_blocks();
            let mut panel = menu_panel("quick-parts-menu")
                .w(px(280.0))
                .max_h(px(420.0))
                .overflow_y_scroll();
            for gallery in [Gallery::QuickParts, Gallery::AutoText] {
                let entries: Vec<(usize, String, String)> = blocks
                    .iter()
                    .enumerate()
                    .filter(|(_, block)| block.gallery == gallery)
                    .map(|(index, block)| (index, block.name.clone(), block.content.plain_text()))
                    .collect();
                if entries.is_empty() {
                    continue;
                }
                panel = panel.child(menu_caption(i18n::text(match gallery {
                    Gallery::QuickParts => "word.quick_part.gallery",
                    Gallery::AutoText => "word.quick_part.autotext",
                })));
                for (index, name, text) in entries {
                    let preview: String = text.chars().take(24).collect();
                    panel = panel.child(
                        div()
                            .flex()
                            .items_center()
                            .child(div().flex_1().child(menu_row(
                                ("quick-part", index),
                                format!("{name}  —  {preview}"),
                                false,
                                Action::Word(ToolCommand::InsertQuickPart(index)),
                                cx,
                            )))
                            .child(
                                div()
                                    .id(("quick-part-delete", index))
                                    .px_1()
                                    .rounded_sm()
                                    .cursor_pointer()
                                    .hover(|button| button.bg(crate::theme::hovered()))
                                    .on_click(crate::ribbon::listener(
                                        Action::Word(ToolCommand::DeleteQuickPart(index)),
                                        cx,
                                    ))
                                    .child(
                                        gpui::svg()
                                            .path("icons/trash-2.svg")
                                            .size_3()
                                            .text_color(crate::theme::text_dim()),
                                    ),
                            ),
                    );
                }
            }
            panel
                .child(menu_row(
                    "quick-part-save",
                    i18n::text("word.quick_part.save"),
                    false,
                    Action::Word(ToolCommand::OpenSaveQuickPart),
                    cx,
                ))
                .into_any_element()
        });
        let drop_cap = self.drop_cap_dropdown(cx);
        crate::ribbon::group(
            i18n::text("word.group.text"),
            vec![
                crate::ribbon::row(vec![dropdown(
                    crate::word_page::list_button(
                        "quick-parts",
                        i18n::text("word.quick_part.menu"),
                        open,
                        Popup::Word(ToolMenu::QuickParts),
                        cx,
                    ),
                    menu,
                )]),
                crate::ribbon::row(vec![drop_cap]),
            ],
        )
    }

    pub(crate) fn insert_quick_part(&mut self, index: usize, cx: &mut Context<Self>) {
        self.tools_caret();
        match self.document.insert_building_block(index) {
            Ok(caret) => {
                self.selection_anchor = None;
                self.insert_at = caret;
                self.caret = self.document.place_caret(caret.0, caret.1);
                self.dirty = true;
                self.rebuild_after_edit();
                cx.notify();
            }
            Err(error) => self.tool_done(Err(error), cx),
        }
    }

    pub(crate) fn delete_quick_part(&mut self, index: usize, cx: &mut Context<Self>) {
        let result = self.document.delete_building_block(index);
        self.tool_done(result, cx);
    }

    pub(crate) fn open_quick_part_dialog(&mut self, cx: &mut Context<Self>) {
        let name = self
            .document
            .selected_text()
            .map(|text| text.chars().take(16).collect::<String>())
            .unwrap_or_default();
        self.tools.dialog = Some(WordDialog::QuickPart(QuickPartDialog {
            inputs: Inputs::new(vec![name]),
            gallery: Gallery::QuickParts,
        }));
        cx.notify();
    }

    pub(crate) fn quick_part_dialog_body(
        &self,
        dialog: &QuickPartDialog,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let galleries = [
            (Gallery::QuickParts, "word.quick_part.gallery"),
            (Gallery::AutoText, "word.quick_part.autotext"),
        ];
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                labeled(i18n::text("word.quick_part.name"))
                    .child(self.dialog_field_element(0, 220.0, cx)),
            )
            .child(
                labeled(i18n::text("word.quick_part.gallery_label")).children(
                    galleries
                        .into_iter()
                        .enumerate()
                        .map(|(index, (gallery, label))| {
                            chip(
                                ("quick-part-gallery", index),
                                i18n::text(label),
                                dialog.gallery == gallery,
                            )
                            .on_click(on(cx, move |view, cx| {
                                view.edit_dialog(cx, |dialog| {
                                    if let WordDialog::QuickPart(dialog) = dialog {
                                        dialog.gallery = gallery;
                                    }
                                })
                            }))
                        }),
                ),
            )
            .into_any_element()
    }

    pub(crate) fn apply_quick_part_dialog(
        &mut self,
        dialog: QuickPartDialog,
    ) -> Result<(), DialogError> {
        let name = dialog.inputs.text(0).trim().to_string();
        if name.is_empty() {
            return Err(DialogError::Stay(
                Box::new(WordDialog::QuickPart(dialog)),
                i18n::text("word.error.name").to_string(),
            ));
        }
        self.document
            .save_building_block(&name, dialog.gallery)
            .map_err(DialogError::Failed)
    }

    /// The styles a document can attach: for the tests and the menus.
    #[allow(dead_code)]
    pub(crate) fn style_count(&self) -> usize {
        self.document.style_catalog().len()
    }
}

//! デザイン → テンプレート (a template's styles copied in, as Word's
//! テンプレートとアドイン attaches one) and 挿入 → クイック パーツ: the
//! selection saved into the クイック パーツ or 定型句 gallery under a name,
//! and an entry put back in at the caret — the browser's copy of the native
//! app's `word_design`, over `jade-office-model`'s `WordDesign`,
//! `WordStyles` and `WordBuildingBlocks`. The browser has no paths: the
//! template is a file the page's picker brings.

use gpui::{div, prelude::*, px, AnyElement, Context};
use jade_web::word::building_blocks::{Gallery, WordBuildingBlocks};
use jade_web::word::styles::WordStyles;

use super::word_dialog::{chip, labeled, on, DialogError, Inputs, WordDialog};
use super::word_tools_ui::Pick;
use super::DocView as DocumentView;
use crate::i18n;
use crate::palette as theme;
use crate::ribbon::{dropdown, menu_caption, menu_panel, menu_row, Action, Popup, Ribbon};
use crate::word_tools::{ToolCommand, ToolMenu};

/// クイック パーツの保存: the name (field 0) and the gallery.
pub struct QuickPartDialog {
    pub inputs: Inputs,
    pub gallery: Gallery,
}

/// The entries the クイック パーツ menu lists: index, gallery, name and the
/// start of its text.
pub type QuickParts = Vec<(usize, Gallery, String, String)>;

/// The document's building blocks, for the menu.
pub fn quick_parts(document: &jade_web::editing::WordDoc) -> QuickParts {
    document
        .building_blocks()
        .into_iter()
        .enumerate()
        .map(|(index, block)| {
            (
                index,
                block.gallery,
                block.name.clone(),
                block.content.plain_text().chars().take(24).collect(),
            )
        })
        .collect()
}

impl DocumentView {
    /// テンプレートとアドイン: asks the page for a template, whose styles
    /// come in when it arrives (`take_picked`).
    pub(crate) fn prompt_for_template(&mut self, _cx: &mut Context<Self>) {
        self.tools.picking = Some(Pick::Template);
        self.host.pick();
    }

    pub(crate) fn attach_template_bytes(
        &mut self,
        bytes: &[u8],
        name: &str,
        cx: &mut Context<Self>,
    ) {
        let result = self
            .doc_mut()
            .attach_template(bytes, name, true)
            .map(|_| ());
        self.tool_done(result, cx);
    }

    pub(crate) fn insert_quick_part(&mut self, index: usize, cx: &mut Context<Self>) {
        let result = self.doc_mut().insert_building_block(index).map(|_| ());
        self.tool_done(result, cx);
    }

    pub(crate) fn delete_quick_part(&mut self, index: usize, cx: &mut Context<Self>) {
        let result = self.doc_mut().delete_building_block(index);
        self.tool_done(result, cx);
    }

    pub(crate) fn open_quick_part_dialog(&mut self, cx: &mut Context<Self>) {
        let name = self
            .doc()
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
        self.doc_mut()
            .save_building_block(&name, dialog.gallery)
            .map_err(DialogError::Failed)
    }

    /// The styles a document has: what a template attached brings more of.
    #[allow(dead_code)]
    pub(crate) fn style_count(&self) -> usize {
        self.doc().style_catalog().len()
    }
}

/// 挿入 → クイック パーツ: the galleries' entries, each with its delete
/// button, and 選択範囲をクイック パーツ ギャラリーに保存.
pub fn quick_parts_group(
    ribbon: &Ribbon,
    parts: &QuickParts,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    let open = ribbon.popup == Some(Popup::Word(ToolMenu::QuickParts));
    let menu = open.then(|| {
        let mut panel = menu_panel("quick-parts-menu")
            .w(px(280.0))
            .max_h(px(420.0))
            .overflow_y_scroll();
        for gallery in [Gallery::QuickParts, Gallery::AutoText] {
            let entries: Vec<_> = parts
                .iter()
                .filter(|(_, kind, ..)| *kind == gallery)
                .collect();
            if entries.is_empty() {
                continue;
            }
            panel = panel.child(menu_caption(i18n::text(match gallery {
                Gallery::QuickParts => "word.quick_part.gallery",
                Gallery::AutoText => "word.quick_part.autotext",
            })));
            for (index, _, name, preview) in entries {
                let index = *index;
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
                                .hover(|button| button.bg(theme::hovered()))
                                .on_click(crate::ribbon::listener(
                                    Action::Word(ToolCommand::DeleteQuickPart(index)),
                                    cx,
                                ))
                                .child(
                                    gpui::svg()
                                        .path("icons/trash-2.svg")
                                        .size_3()
                                        .text_color(theme::text_dim()),
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
    crate::ribbon::group(
        i18n::text("word.quick_part.menu"),
        vec![crate::ribbon::row(vec![dropdown(
            crate::word_tools::menu_trigger(
                "quick-parts",
                i18n::text("word.quick_part.menu"),
                open,
                ToolMenu::QuickParts,
                cx,
            ),
            menu,
        )])],
    )
}

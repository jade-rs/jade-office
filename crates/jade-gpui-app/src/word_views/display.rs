//! Word 環境設定 › 表示's settings for how the page is shown: フィールドの
//! 網かけ. Kept in the options file for every document, never written into
//! one, and left out of a PDF.

use std::cell::RefCell;

use gpui::{div, prelude::*, AnyElement, Context};
use jade_office_model::word::field_shading::{shading_for, FieldShadingMode};
use jade_office_model::word::fields::SelectionRange;
use jade_office_model::word::WordSession;

use super::settings;
use crate::word_dialog::{chip, on};
use crate::{i18n, theme, DocumentView};

const OPTIONS: &str = "options.txt";

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

/// フィールドの網かけ as set; Word's default is 選択時に表示する.
pub fn field_shading() -> FieldShadingMode {
    settings::value(OPTIONS, "field_shading")
        .and_then(|key| FieldShadingMode::from_key(&key))
        .unwrap_or_default()
}

pub fn set_field_shading(mode: FieldShadingMode) {
    settings::set_value(OPTIONS, "field_shading", mode.key());
}

type ShadingKey = (usize, u64, Option<SelectionRange>, FieldShadingMode);

thread_local! {
    /// The window, edit, selection and setting the shading was last found
    /// for: finding it reads the whole document, so not on every frame.
    static SHADED: RefCell<Option<ShadingKey>> = const { RefCell::new(None) };
}

/// Gives the document the shading the setting asks for — with 選択時に表示
/// する, of the field the caret is in — repainting when it changed.
pub fn apply(view: &mut DocumentView) {
    let mode = field_shading();
    let key = (
        view as *const DocumentView as usize,
        view.edits,
        view.document.selection_range(),
        mode,
    );
    if SHADED.with(|cell| cell.borrow().as_ref() == Some(&key)) {
        return;
    }
    SHADED.with(|cell| *cell.borrow_mut() = Some(key));
    let shading = shading_for(&view.document, mode);
    match view.document.set_field_shading(shading) {
        Ok(true) => {
            view.caret = view
                .document
                .place_caret(view.insert_at.0, view.insert_at.1);
            view.rebuild_all_images();
        }
        Ok(false) => {}
        Err(error) => view.table_status = Some(error.into()),
    }
}

/// The 表示 page's part for them.
pub fn settings_section(cx: &Context<DocumentView>) -> AnyElement {
    let now = field_shading();
    div()
        .flex()
        .flex_col()
        .gap_1()
        .pt_2()
        .child(
            div()
                .text_color(theme::text_dim())
                .child(t("options.field_shading")),
        )
        .child(
            div()
                .flex()
                .gap_1()
                .children(
                    FieldShadingMode::ALL
                        .into_iter()
                        .enumerate()
                        .map(|(index, mode)| {
                            chip(
                                ("options-field-shading", index),
                                t(mode.label_key()),
                                now == mode,
                            )
                            .on_click(on(cx, move |view, cx| {
                                set_field_shading(mode);
                                apply(view);
                                cx.notify();
                            }))
                            .into_any_element()
                        }),
                ),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use jade_docx_editor::canvas_renderer::FieldShading;

    /// The setting reaches the document: 常に表示する shades every field,
    /// 表示しない none, and 選択時に表示する none while the caret is in
    /// plain text.
    #[gpui::test]
    fn the_setting_reaches_the_document(cx: &mut gpui::TestAppContext) {
        let (view, cx) = crate::word_views::tests::open_contract(cx);
        view.update(cx, |view, _| {
            set_field_shading(FieldShadingMode::Always);
            apply(view);
            assert_eq!(view.document.field_shading(), &FieldShading::Always);
            set_field_shading(FieldShadingMode::WhenSelected);
            apply(view);
            assert_eq!(view.document.field_shading(), &FieldShading::Never);
            set_field_shading(FieldShadingMode::Never);
            apply(view);
            assert_eq!(view.document.field_shading(), &FieldShading::Never);
            set_field_shading(FieldShadingMode::default());
        });
        // Drawn in 環境設定 › 表示.
        view.update(cx, |view, cx| {
            super::super::prefs::open(view, super::super::prefs::OptionsPage::View);
            cx.notify();
        });
        cx.run_until_parked();
    }
}

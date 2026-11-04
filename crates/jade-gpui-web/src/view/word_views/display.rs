//! Word 環境設定 › 表示's settings for how the page is shown: フィールドの
//! 網かけ. Kept in the browser's settings for every document, never
//! written into one.

use std::cell::RefCell;

use gpui::{div, prelude::*, AnyElement, Context};
use jade_web::word::field_shading::{shading_for, FieldShadingMode};
use jade_web::word::WordSession;

use super::settings;
use crate::view::word_kit::{chip, on};
use crate::view::DocView as DocumentView;
use crate::{i18n, palette as theme};

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

type Spot = (usize, usize);
type ShadingKey = (usize, u64, Option<(Spot, Spot)>, FieldShadingMode);

thread_local! {
    /// The view, edit, selection and setting the shading was last found
    /// for: finding it reads the whole document, so not on every frame.
    static SHADED: RefCell<Option<ShadingKey>> = const { RefCell::new(None) };
}

/// Gives the document the shading the setting asks for — with 選択時に表示
/// する, of the field the caret is in — repainting when it changed.
pub fn apply(view: &mut DocumentView) {
    let mode = field_shading();
    let key = (
        view as *const DocumentView as usize,
        view.generation,
        view.doc().selection_range().map(|range| {
            (
                (range.anchor.paragraph, range.anchor.offset),
                (range.focus.paragraph, range.focus.offset),
            )
        }),
        mode,
    );
    if SHADED.with(|cell| cell.borrow().as_ref() == Some(&key)) {
        return;
    }
    SHADED.with(|cell| *cell.borrow_mut() = Some(key));
    let shading = shading_for(view.doc(), mode);
    match view.doc_mut().set_field_shading(shading) {
        Ok(true) => view.refresh_repainted(),
        Ok(false) => {}
        Err(error) => view.set_status(error.into()),
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
                                settings::set_value(OPTIONS, "field_shading", mode.key());
                                apply(view);
                                cx.notify();
                            }))
                            .into_any_element()
                        }),
                ),
        )
        .into_any_element()
}

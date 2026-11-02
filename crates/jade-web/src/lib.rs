//! Documents, slides and sheets on the pure-Rust painter, natively or in the
//! browser.
//!
//! The same reader → layout → renderer chain the native app runs, with
//! `jade-tiny-skia-painter` underneath instead of Skia. Nothing in it needs a
//! thread, the filesystem or C++, so it compiles to `wasm32-unknown-unknown`
//! unchanged; `web.rs` is the thin wasm-bindgen layer over it.
//!
//! Sheets and slides are `jade-office-model`'s editing models — the very ones
//! the native app uses — on the tiny-skia backend. Word documents are
//! rendered by [`document`], a tiny-skia host for the docx renderer.

use std::sync::Arc;

pub use jade_office_model::tiny::TinySkiaBackend;
pub use jade_office_model::{
    analysis, cell_styles, charts, color_wheel, conditional_formatting, csv, custom_filter,
    data_table, date_filter, deck, duplicates, editable_ranges, find, form, format_cells,
    formula_complete, formula_tips, freeform, function_help, goal_seek, grid, insert_function,
    links, more_colors, name_manager, number_formats, page_setup_dialog, paste_special, pivots,
    precedents, print, protected_edits, protection, recent_files, report, scenarios, sheet, solver,
    sort_dialog, spin, subtotal, tables, text, text_columns, top_ten, typing, validation, what_if,
    word, zoom_dialog,
};

pub mod document;
pub mod editing;
pub mod pdf;
#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::set_word_layout_platform_from_page;

/// An editable presentation, painted with tiny-skia.
pub type Deck = deck::Deck<TinySkiaBackend>;
/// An editable workbook, painted with tiny-skia.
pub type Workbook = sheet::Workbook<TinySkiaBackend>;

/// A tiny-skia backend with the regular and bold faces under every alias the
/// native app uses.
pub fn backend(regular: Vec<u8>, bold: Option<Vec<u8>>) -> Result<TinySkiaBackend, String> {
    backend_with(regular, bold, None)
}

/// [`backend`], with Noto Sans Thai beside the Japanese faces for Thai text
/// to fall back to.
pub fn backend_with(
    regular: Vec<u8>,
    bold: Option<Vec<u8>>,
    thai: Option<Vec<u8>>,
) -> Result<TinySkiaBackend, String> {
    let mut faces = jade_office_model::aliased_faces(Arc::new(regular), bold.map(Arc::new));
    faces.extend(thai.map(|thai| jade_office_model::thai_face(Arc::new(thai))));
    TinySkiaBackend::new(&faces)
}

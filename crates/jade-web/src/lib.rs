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
    charts, conditional_formatting, csv, deck, find, format_cells, grid, number_formats,
    paste_special, print, sheet, text, typing, word,
};

pub mod document;
pub mod editing;
pub mod pdf;
#[cfg(target_arch = "wasm32")]
mod web;

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

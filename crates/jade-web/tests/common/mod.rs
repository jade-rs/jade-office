//! Shared by the Word feature tests: the contract fixture opened for
//! editing, and a body paragraph with text typed at its start. Each test
//! binary compiles it on its own, so what one does not use is dead there.
#![allow(dead_code)]

use std::path::PathBuf;

use jade_web::editing::{EditableDocument, WordSession};

pub fn core() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"))
}

pub fn open_bytes(bytes: &[u8]) -> EditableDocument {
    let fonts_dir = core().join("fonts/Noto_Sans_JP/static");
    let regular = std::fs::read(fonts_dir.join("NotoSansJP-Regular.ttf")).unwrap();
    let bold = std::fs::read(fonts_dir.join("NotoSansJP-Bold.ttf")).unwrap();
    let fonts = jade_web::backend(regular.clone(), Some(bold.clone()))
        .unwrap()
        .fonts();
    let measurer = jade_web::document::measurer(&regular, Some(&bold)).unwrap();
    EditableDocument::open(bytes, 1.0, fonts, measurer).unwrap()
}

pub fn contract() -> EditableDocument {
    open_bytes(&std::fs::read(core().join("contract.docx")).unwrap())
}

/// A body paragraph outside any table with `text` typed at its start.
pub fn typed(document: &mut EditableDocument, text: &str) -> usize {
    let paragraph = (0..document.paragraph_count())
        .find(|&index| {
            document.cell_id_for_paragraph(index).is_none()
                && document
                    .paragraph_text(index)
                    .is_some_and(|text| text.chars().count() > 3)
        })
        .unwrap();
    document.place_caret(paragraph, 0);
    document.insert_text(text).unwrap();
    paragraph
}

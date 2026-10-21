//! The installed faces a document's fonts are embedded from, when its
//! TrueType フォントをファイルに埋め込む is on (`jade-office-model`'s
//! `word::embedding` does the embedding).

use std::path::PathBuf;

use jade_docx_native_render::system_fonts::FontLibrary;
use jade_docx_native_render::FontFace;
use jade_office_model::word::embedding::{self, Face};

/// Where embedded fonts are written to be registered from: the app's
/// caches, a file per font named by its contents.
fn cache() -> Option<PathBuf> {
    let base = if cfg!(test) {
        std::env::temp_dir().join(format!("jade-embedded-fonts-{}", std::process::id()))
    } else {
        PathBuf::from(std::env::var_os("HOME")?).join("Library/Caches/jade-office/embedded-fonts")
    };
    std::fs::create_dir_all(&base).ok()?;
    Some(base)
}

/// The faces `docx` embeds, as faces to open it with.
pub fn embedded_faces(docx: &[u8]) -> Vec<FontFace> {
    let Ok(fonts) = jade_docx_reader::docx_embedded_fonts(docx) else {
        return Vec::new();
    };
    if fonts.is_empty() {
        return Vec::new();
    }
    let Some(cache) = cache() else {
        return Vec::new();
    };
    fonts
        .into_iter()
        .filter_map(|font| {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            font.data.hash(&mut hasher);
            let path = cache.join(format!("{:016x}.ttf", hasher.finish()));
            if !path.exists() {
                std::fs::write(&path, &font.data).ok()?;
            }
            Some(FontFace {
                family: font.family,
                weight: if font.style.is_bold() { 700 } else { 400 },
                italic: font.style.is_italic(),
                path,
                index: 0,
            })
        })
        .collect()
}

/// The installed faces of `family`, as files of their own.
pub fn installed_faces(family: &str) -> Vec<Face> {
    FontLibrary::system()
        .find(family)
        .into_iter()
        .filter_map(|(face, weight)| {
            let data = std::fs::read(&face.path).ok()?;
            Some(Face {
                bold: weight >= 600,
                italic: face.italic,
                data: embedding::sfnt_face(&data, face.index)?,
            })
        })
        .collect()
}

/// `docx` as a save writes it: its fonts embedded when `embed` is on,
/// none when it is off.
pub fn embedded_for_save(docx: Vec<u8>, embed: bool, subset: bool) -> Vec<u8> {
    if !embed {
        return match embedding::embedding_settings(&docx) {
            (false, _) => docx,
            _ => embedding::remove_embedded_fonts(&docx).unwrap_or(docx),
        };
    }
    match embedding::embed_fonts(&docx, subset, &installed_faces) {
        Ok((embedded, _)) => embedded,
        Err(_) => docx,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A document's embedded fonts become faces it is opened with, and a
    /// save with embedding on writes the installed faces of its fonts.
    #[test]
    fn embedded_fonts_are_registered_and_written() {
        let font = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../jade-core/fonts/Noto_Sans_JP/static/NotoSansJP-Regular.ttf"
        ))
        .unwrap();
        let blank = jade_office_model::word::new::blank_document(
            jade_office_model::word::new::Blank::English,
        );
        let mut package = jade_opc::OpcPackage::open(&blank).unwrap();
        package.replace(
            "word/fontTable.xml",
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:fonts xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:font w:name="Jade Embedded Test"/></w:fonts>"#
                .to_vec(),
        );
        let docx = package.write().unwrap();
        let find = |family: &str| {
            if family == "Jade Embedded Test" {
                vec![Face {
                    bold: false,
                    italic: false,
                    data: font.clone(),
                }]
            } else {
                Vec::new()
            }
        };
        let (embedded, report) = embedding::embed_fonts(&docx, false, &find).unwrap();
        assert_eq!(report.families.len(), 1);
        let faces = embedded_faces(&embedded);
        assert_eq!(faces.len(), 1);
        assert_eq!(faces[0].family, "Jade Embedded Test");
        assert_eq!(std::fs::read(&faces[0].path).unwrap(), font);
        // Opened, the document lays out with it.
        let document = crate::open_document_bytes(&embedded);
        assert!(document.is_ok());
        // Saving with embedding off takes them out.
        let saved = embedded_for_save(embedded, false, false);
        assert!(embedded_faces(&saved).is_empty());
    }
}

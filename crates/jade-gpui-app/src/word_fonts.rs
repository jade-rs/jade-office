//! The installed faces a document's fonts are embedded from, when its
//! TrueType フォントをファイルに埋め込む is on (`jade-office-model`'s
//! `word::embedding` does the embedding).

use jade_docx_native_render::system_fonts::FontLibrary;
use jade_office_model::word::embedding::{self, Face};

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

    /// Opening and exporting a document use its embedded font, and a save
    /// with embedding off strips the font program.
    #[test]
    fn embedded_fonts_are_registered_and_written() {
        let font = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../jade-core/crates/jade-docx-layout/tests/fixtures/math-font.ttf"
        ))
        .unwrap();
        let blank = jade_office_model::word::new::blank_document(
            jade_office_model::word::new::Blank::English,
        );
        let mut package = jade_opc::OpcPackage::open(&blank).unwrap();
        package.replace(
            "word/document.xml",
            br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:rPr><w:rFonts w:ascii="Jade Embedded Test" w:hAnsi="Jade Embedded Test"/></w:rPr><w:t>x</w:t></w:r></w:p></w:body></w:document>"#.to_vec(),
        );
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
        use jade_docx_native_render::{PdfExportOptions, PdfRenderingMode};

        let document = crate::open_document_bytes(&embedded).unwrap();
        let pdf = lopdf::Document::load_mem(
            &document
                .export_pdf_with_options(PdfExportOptions {
                    rendering: PdfRenderingMode::Vector,
                    ..PdfExportOptions::default()
                })
                .unwrap(),
        )
        .unwrap();
        assert_eq!(pdf.extract_text(&[1]).unwrap().trim(), "x");
        let font_names: Vec<_> = pdf
            .get_page_fonts(pdf.get_pages()[&1])
            .unwrap()
            .values()
            .filter_map(|font| font.get(b"BaseFont").ok()?.as_name().ok())
            .map(String::from_utf8_lossy)
            .collect();
        assert!(
            font_names.iter().any(|name| name.contains("JadeMathTest")),
            "{font_names:?}"
        );

        let saved = embedded_for_save(embedded, false, false);
        assert!(jade_docx_reader::docx_embedded_fonts(&saved)
            .unwrap()
            .is_empty());
    }
}

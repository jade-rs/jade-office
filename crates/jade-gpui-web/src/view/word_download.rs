//! A Word document downloaded in another format — what a browser has for
//! 名前を付けて保存 and コピーを保存 — and the [互換モード] its title shows
//! when it is from an older Word. The conversions are `jade-office-model`'s
//! (`word::formats`, `word::compat`).

use gpui::Context;
use jade_web::word::compat;
use jade_web::word::formats::{renamed, save_from_docx, SaveOptions, TextEncoding, WordFormat};

use super::{DocView, Document};
use crate::i18n;

/// The formats the download menu offers, with a text file's encoding.
pub(crate) const DOWNLOADS: [(WordFormat, Option<TextEncoding>); 7] = [
    (WordFormat::Docx, None),
    (WordFormat::Dotx, None),
    (WordFormat::Rtf, None),
    (WordFormat::Odt, None),
    (WordFormat::Html, None),
    (WordFormat::Text, Some(TextEncoding::Utf8)),
    (WordFormat::Text, Some(TextEncoding::ShiftJis)),
];

/// The menu's label for a download.
pub(crate) fn label(format: WordFormat, encoding: Option<TextEncoding>) -> String {
    let japanese = !i18n::is_english();
    match encoding {
        Some(encoding) => format!(
            "{} — {}",
            format.label(japanese),
            if japanese {
                encoding.label_ja()
            } else {
                encoding.label_en()
            }
        ),
        None => format.label(japanese).to_string(),
    }
}

impl DocView {
    /// [互換モード], for a document laid out for an older Word.
    pub fn title_badge(&self) -> Option<&'static str> {
        (matches!(self.document, Document::Pages { .. })
            && compat::is_compatibility_mode(self.compat_mode))
        .then(|| i18n::text("compat.badge"))
    }

    /// Downloads the document as `format`, leaving it the file it is.
    pub fn download_as(
        &mut self,
        format: WordFormat,
        encoding: Option<TextEncoding>,
        cx: &mut Context<Self>,
    ) {
        let Document::Pages { document, .. } = &mut self.document else {
            return;
        };
        let name = renamed(&self.name, format);
        let bytes = document.word_docx().and_then(|docx| {
            save_from_docx(
                format,
                &docx,
                SaveOptions {
                    encoding,
                    ..SaveOptions::default()
                },
            )
            .map_err(|error| error.to_string())
        });
        self.status = match bytes {
            Ok(bytes) => {
                self.host.save(&name, &bytes);
                i18n::format("web.downloaded", &[("name", &name)]).into()
            }
            Err(error) => i18n::format("sheet.save_failed", &[("error", &error)]).into(),
        };
        cx.notify();
    }
}

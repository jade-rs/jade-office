//! What a save does to the document beyond writing it, as Word's
//! ファイル サイズの圧縮 and 環境設定 → 保存 set it: the pictures brought down
//! to a resolution, with their cropped parts discarded — now, or on every
//! save — and the TrueType fonts the document uses embedded in it.
//! `jade-office-model` does the work (`word::compress`, `word::fonts`).

use gpui::{actions, div, prelude::*, px, AnyElement, ClickEvent, Context, SharedString};
use jade_office_model::word::compress::{self, Resolution};
use jade_office_model::word::formats::WordFormat;

use crate::word_characters::{caption, check};
use crate::word_files::FileDialog;
use crate::{i18n, theme, DocumentView};

actions!(
    jade,
    [
        /// ファイル サイズの圧縮 (図の圧縮).
        CompressPictures,
    ]
);

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

/// How pictures are compressed, now or on save.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Compression {
    pub resolution: Resolution,
    /// 図のトリミング部分を削除する.
    pub delete_crops: bool,
}

/// The ファイル サイズの圧縮 dialog as it is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompressDialog {
    pub compression: Compression,
    /// Whether every save compresses this way too.
    pub on_save: bool,
}

impl DocumentView {
    pub(crate) fn open_compress_dialog(&mut self, cx: &mut Context<Self>) {
        let compression = self.files.compress_on_save.unwrap_or(Compression {
            resolution: Resolution::Print,
            delete_crops: true,
        });
        self.files.dialog = Some(FileDialog::Compress(CompressDialog {
            compression,
            on_save: self.files.compress_on_save.is_some(),
        }));
        cx.notify();
    }

    /// Compresses the pictures now, and remembers whether saves do too.
    pub(crate) fn compress_now(&mut self, dialog: CompressDialog, cx: &mut Context<Self>) {
        self.files.compress_on_save = dialog.on_save.then_some(dialog.compression);
        let result = self.document.export_docx().and_then(|docx| {
            compress::compress_pictures(
                &docx,
                dialog.compression.resolution,
                dialog.compression.delete_crops,
            )
        });
        match result {
            Ok((docx, report)) if report.resampled + report.uncropped > 0 => {
                match crate::open_document_bytes(&docx) {
                    Ok(document) => {
                        self.replace_document(document);
                        self.dirty = true;
                        let saved = report.bytes_before.saturating_sub(report.bytes_after);
                        self.show_message(
                            t("compress.title"),
                            &i18n::format(
                                "compress.done",
                                &[
                                    ("n", &(report.resampled + report.uncropped)),
                                    ("kb", &(saved / 1024)),
                                ],
                            ),
                            cx,
                        );
                    }
                    Err(error) => self.show_message(t("compress.title"), &error, cx),
                }
            }
            Ok(_) => self.show_message(t("compress.title"), t("compress.nothing"), cx),
            Err(error) => self.show_message(t("compress.title"), &error, cx),
        }
    }

    /// The `.docx` a save writes: its pictures compressed and its fonts
    /// embedded, as the document's save options say.
    pub(crate) fn prepared_for_save(&self, mut docx: Vec<u8>, format: WordFormat) -> Vec<u8> {
        if let Some(compression) = self.files.compress_on_save {
            if let Ok((compressed, _)) =
                compress::compress_pictures(&docx, compression.resolution, compression.delete_crops)
            {
                docx = compressed;
            }
        }
        if format.is_package() {
            docx = crate::word_fonts::embedded_for_save(
                docx,
                self.files.embed_fonts,
                self.files.embed_subset,
            );
        }
        docx
    }
}

/// The dialog's title, body and buttons.
pub fn dialog(
    state: CompressDialog,
    cx: &mut Context<DocumentView>,
) -> (SharedString, AnyElement, AnyElement) {
    let body = div()
        .flex()
        .flex_col()
        .gap_2()
        .w(px(380.0))
        .child(caption(t("compress.quality")))
        .children(Resolution::ALL.into_iter().map(|resolution| {
            check(
                SharedString::from(format!("compress-{}", resolution.ppi())),
                t(resolution.key()),
                state.compression.resolution == resolution,
            )
            .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                if let Some(FileDialog::Compress(dialog)) = view.files.dialog.as_mut() {
                    dialog.compression.resolution = resolution;
                }
                cx.notify();
            }))
        }))
        .child(
            check(
                "compress-crops",
                t("compress.delete_crops"),
                state.compression.delete_crops,
            )
            .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
                if let Some(FileDialog::Compress(dialog)) = view.files.dialog.as_mut() {
                    dialog.compression.delete_crops = !dialog.compression.delete_crops;
                }
                cx.notify();
            })),
        )
        .child(
            check("compress-on-save", t("compress.on_save"), state.on_save).on_click(cx.listener(
                |view, _: &ClickEvent, _, cx| {
                    if let Some(FileDialog::Compress(dialog)) = view.files.dialog.as_mut() {
                        dialog.on_save = !dialog.on_save;
                    }
                    cx.notify();
                },
            )),
        )
        .child(
            div()
                .text_color(theme::text_faint())
                .child(t("compress.note")),
        )
        .into_any_element();
    (
        t("compress.title").into(),
        body,
        crate::word_files::ok_cancel_buttons(cx),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ファイル サイズの圧縮 brings a large picture down to the resolution
    /// chosen, and with 保存時 on, every save does too.
    #[gpui::test]
    fn pictures_are_compressed_now_and_on_save(cx: &mut gpui::TestAppContext) {
        use gpui::Focusable;
        let path = crate::shell::scratch_copy(
            &std::path::PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../jade-core/public/fixtures/image-inline.docx"
            )),
            "pictures.docx",
        );
        let before = std::fs::read(&path).unwrap();
        let (view, cx) =
            cx.add_window_view(|_window, cx| crate::open_document_view(&path, cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.dispatch_action(CompressPictures);
        view.update(cx, |view, cx| {
            let Some(FileDialog::Compress(mut dialog)) = view.files.dialog.take() else {
                panic!("the dialog is open");
            };
            dialog.compression.resolution = Resolution::Email;
            dialog.on_save = true;
            view.compress_now(dialog, cx);
        });
        view.read_with(cx, |view, _| {
            assert!(view.files.compress_on_save.is_some());
            assert!(view.dirty);
        });
        cx.dispatch_action(crate::Save);
        cx.run_until_parked();
        let after = std::fs::read(&path).unwrap();
        assert!(
            after.len() < before.len(),
            "{} >= {}",
            after.len(),
            before.len()
        );
    }
}

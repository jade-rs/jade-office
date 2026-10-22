//! デジタル署名 and 署名欄 in the browser, as the native app's
//! `word_signatures`: a signed document opens read-only with its bar; the
//! 署名 pane lists who signed and whether it holds, and the signature
//! lines waiting; 編集する makes the signatures invalid and the next save
//! takes them out. 署名欄 inserts a signature line, its picture a small
//! page drawn with the page's fonts. Signing itself needs a certificate
//! and its key, which the browser does not give a page.

use jade_web::word::signatures::{
    signature_line_docx, SignatureLine, SignatureState, SignatureStatus, WordSignatures,
};

use jade_web::word::protection::WordReadOnly;

use super::word_dialogs::Dialog;
use super::word_review::{ReadOnlyReason, WebProtection};
use super::*;

/// Draws a signature line's picture.
pub(crate) type LinePicture = Rc<dyn Fn(&SignatureLine) -> Result<Vec<u8>, String>>;

/// For a document just opened from `bytes`: its signatures, read-only
/// with the bar when it has any, and how to draw a signature line.
pub(super) fn with_signatures(
    mut protection: WebProtection,
    bytes: &[u8],
    fonts: &Fonts,
    document: &mut jade_web::editing::EditableDocument,
) -> WebProtection {
    protection.signatures = SignatureState::opened(bytes);
    if protection.signatures.is_signed() {
        protection.read_only = Some(ReadOnlyReason::Signed);
        document.set_opened_read_only(true);
    }
    let (scale, book, measurer) = (
        fonts.scale,
        fonts.backend.fonts(),
        Rc::clone(&fonts.measurer),
    );
    protection.line_picture = Some(Rc::new(move |line: &SignatureLine| {
        let page = jade_web::editing::EditableDocument::open(
            &signature_line_docx(line),
            scale,
            Rc::clone(&book),
            Rc::clone(&measurer),
        )?;
        page.page(0)
            .and_then(|painter| painter.encode_png())
            .ok_or_else(|| "the signature line could not be drawn".to_string())
    }));
    protection
}

impl DocView {
    pub(super) fn toggle_signatures_pane(&mut self, cx: &mut Context<Self>) {
        if let Document::Pages { protection, .. } = &mut self.document {
            protection.signatures_pane = !protection.signatures_pane;
        }
        cx.notify();
    }

    /// 署名欄: the bar.
    pub(super) fn open_signature_line_dialog(&mut self, cx: &mut Context<Self>) {
        self.open_dialog(Dialog::SignatureLine(Box::default()), cx);
    }

    /// 署名欄's OK: the line at the caret.
    pub(super) fn insert_signature_line(&mut self, line: SignatureLine, cx: &mut Context<Self>) {
        let Document::Pages { protection, .. } = &self.document else {
            return;
        };
        let Some(draw) = protection.line_picture.clone() else {
            return;
        };
        self.word_edit(cx, move |document| {
            let picture = draw(&line)?;
            document.insert_signature_line(line, picture)
        });
    }

    /// The 署名 pane beside the pages, while it is open.
    pub(super) fn signatures_pane(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let Document::Pages {
            document,
            protection,
            ..
        } = &self.document
        else {
            return None;
        };
        if !protection.signatures_pane {
            return None;
        }
        let state = &protection.signatures;
        let mut rows: Vec<gpui::AnyElement> = vec![div()
            .text_color(palette::dim())
            .child(i18n::text("sign.valid_heading"))
            .into_any_element()];
        if state.signatures.is_empty() {
            rows.push(
                div()
                    .text_color(palette::faint())
                    .child(i18n::text("sign.none"))
                    .into_any_element(),
            );
        }
        for signature in &state.signatures {
            let valid = state.status(signature) == SignatureStatus::Valid;
            let detail = [signature.signed_at.clone(), signature.comment.clone()]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" · ");
            rows.push(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_2()
                    .rounded_md()
                    .border_1()
                    .border_color(palette::border())
                    .bg(palette::canvas())
                    .child(format!(
                        "{} {}",
                        if valid { "✓" } else { "✕" },
                        signature.signer.clone().unwrap_or_default()
                    ))
                    .child(div().text_color(palette::dim()).child(i18n::text(if valid {
                        "sign.valid"
                    } else {
                        "sign.invalid"
                    })))
                    .when(!detail.is_empty(), |card| {
                        card.child(div().text_color(palette::faint()).child(detail))
                    })
                    .into_any_element(),
            );
        }
        let lines = document.signature_lines();
        if !lines.is_empty() {
            rows.push(
                div()
                    .mt_2()
                    .text_color(palette::dim())
                    .child(i18n::text("sign.requested"))
                    .into_any_element(),
            );
            for line in lines {
                rows.push(
                    div()
                        .p_2()
                        .rounded_md()
                        .border_1()
                        .border_color(palette::border())
                        .child(
                            [line.signer, line.title, line.email]
                                .into_iter()
                                .flatten()
                                .collect::<Vec<_>>()
                                .join(" · "),
                        )
                        .into_any_element(),
                );
            }
        }
        let _ = cx;
        Some(
            div()
                .id("signatures-pane")
                .flex()
                .flex_col()
                .flex_none()
                .w(px(260.0))
                .h_full()
                .gap_2()
                .px_3()
                .pt_3()
                .overflow_y_scroll()
                .bg(palette::panel())
                .border_l_1()
                .border_color(palette::border())
                .text_xs()
                .child(
                    div()
                        .text_sm()
                        .text_color(palette::text())
                        .child(i18n::text("sign.pane")),
                )
                .children(rows)
                .into_any_element(),
        )
    }
}

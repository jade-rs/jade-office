//! デジタル署名 and 署名欄: a signed document opens read-only with a bar
//! saying so; the 署名 pane lists who signed, when, why and whether it
//! still holds, and the signature lines still waiting; 編集する makes the
//! signatures invalid, and the next save takes them out, as Word does.
//! 署名… signs with a certificate from the keychain (the save that follows
//! writes the signature); 挿入 › 署名欄 puts a signature line at the caret,
//! its picture a small page this app renders with its own fonts. What is
//! checked and written is `jade-office-model`'s `word::signatures`.

use gpui::{div, prelude::*, px, AnyElement, ClickEvent, Context};
use jade_office_model::text::TextEdit;
use jade_office_model::word::signatures::{
    rgba_png, signature_line_docx, PendingSign, SignatureLine, SignatureState, SignatureStatus,
    WordSignatures,
};

use crate::word_characters::button;
use crate::word_protection::ReadOnlyReason;
use crate::word_review_tools::ReviewDialog;
use crate::{i18n, theme, DocumentView};

/// The view's share of 署名.
#[derive(Default)]
pub struct SignaturesUi {
    pub state: SignatureState,
    /// The 署名 pane is open.
    pub pane: bool,
}

/// 署名…: the certificate to sign with and 署名の目的.
pub struct SignForm {
    /// The keychain's identities, by name.
    pub identities: Vec<String>,
    pub selected: usize,
    pub purpose: TextEdit,
}

/// 署名欄の設定 (shared with the browser shell).
pub type LineForm = jade_office_model::word::signatures::SignatureLineForm;

/// For a document just opened from `path`: its signatures, and read-only
/// with the bar when it has any.
pub fn opened(
    mut review: crate::word_review_tools::ReviewTools,
    path: &str,
    document: &mut jade_docx_native_render::NativeDocument,
) -> crate::word_review_tools::ReviewTools {
    let Ok(bytes) = std::fs::read(path) else {
        return review;
    };
    review.signatures.state = SignatureState::opened(&bytes);
    if review.signatures.state.is_signed() {
        review.protection.read_only = Some(ReadOnlyReason::Signed);
        document.set_opened_read_only(true);
    }
    review
}

/// 編集する on a signed document: the signatures stop holding.
pub fn edited(view: &mut DocumentView) {
    if view.review.signatures.state.is_signed() {
        view.review.signatures.state.edited = true;
        view.dirty = true;
    }
}

/// The package a save writes: the signatures taken out once the document
/// was edited; signed when 署名… waits for this save.
pub fn finish_save(view: &mut DocumentView, bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    let signer = view
        .review
        .signatures
        .state
        .pending
        .as_ref()
        .and_then(|_| keychain::signer(view.review.signatures_identity.as_deref()));
    let signed = view.review.signatures.state.pending.is_some();
    let bytes = view.review.signatures.state.finish_save(
        bytes,
        signer
            .as_ref()
            .map(|signer| signer.as_ref() as &dyn jade_office_model::word::signatures::Signer),
    )?;
    if signed {
        // Signed, it is read-only again, as Word leaves it.
        view.review.protection.read_only = Some(ReadOnlyReason::Signed);
        view.document.set_opened_read_only(true);
    }
    Ok(bytes)
}

/// 署名: the pane, open or shut.
pub fn toggle_pane(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    view.review.signatures.pane = !view.review.signatures.pane;
    cx.notify();
}

/// 署名…: the dialog, with the keychain's certificates.
pub fn open_sign(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let identities = keychain::identities();
    if identities.is_empty() {
        view.table_status = Some(i18n::text("sign.no_certificate").into());
        return cx.notify();
    }
    view.review.dialog = Some(ReviewDialog::Sign(Box::new(SignForm {
        identities,
        selected: 0,
        purpose: TextEdit::new(String::new()),
    })));
    cx.notify();
}

/// 署名's OK: the signature is written by the save that follows at once.
pub fn confirm_sign(view: &mut DocumentView, form: SignForm, cx: &mut Context<DocumentView>) {
    view.review.signatures_identity = form.identities.get(form.selected).cloned();
    view.review.signatures.state.pending = Some(PendingSign {
        signed_at: now_utc(),
        comment: form.purpose.text,
    });
    view.dirty = true;
    view.save(cx);
}

/// 挿入 › 署名欄: the dialog.
pub fn open_line(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    view.review.dialog = Some(ReviewDialog::SignatureLine(Box::default()));
    cx.notify();
}

/// The signature line's picture: its little page, rendered.
fn line_picture(line: &SignatureLine) -> Result<Vec<u8>, String> {
    let document = crate::open_document_bytes(&signature_line_docx(line))?;
    let (bgra, width, height) = document
        .page_pixels(0)
        .ok_or("the signature line could not be drawn")?;
    let mut rgba = bgra;
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    rgba_png(width as u32, height as u32, &rgba)
}

/// 署名欄's OK.
pub fn confirm_line(view: &mut DocumentView, form: LineForm, cx: &mut Context<DocumentView>) {
    let line = form.line();
    if view.document.selection().is_none() {
        view.caret = view
            .document
            .place_caret(view.insert_at.0, view.insert_at.1);
    }
    let result =
        line_picture(&line).and_then(|picture| view.document.insert_signature_line(line, picture));
    view.reviewed(result, cx);
}

pub(crate) fn now_utc() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let days = (seconds / 86_400) as i64;
    let rest = seconds % 86_400;
    // Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest / 60 % 60,
        rest % 60
    )
}

/// The 署名 dialog's body.
pub fn sign_body(form: &SignForm, cx: &mut Context<DocumentView>) -> AnyElement {
    let mut certificates: Vec<AnyElement> = Vec::new();
    for (index, name) in form.identities.iter().enumerate() {
        let on = index == form.selected;
        certificates.push(
            div()
                .id(("sign-certificate", index))
                .px_2()
                .py_1()
                .rounded_md()
                .cursor_pointer()
                .when(on, |row| row.bg(theme::selected()))
                .hover(|row| row.bg(theme::hovered()))
                .child(name.clone())
                .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                    if let Some(ReviewDialog::Sign(form)) = view.review.dialog.as_mut() {
                        form.selected = index;
                    }
                    cx.notify();
                }))
                .into_any_element(),
        );
    }
    div()
        .flex()
        .flex_col()
        .gap_2()
        .w(px(400.0))
        .child(
            div()
                .text_color(theme::text_dim())
                .child(i18n::text("sign.hint")),
        )
        .child(
            div()
                .text_color(theme::text_dim())
                .child(i18n::text("sign.certificate")),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .max_h(px(160.0))
                .border_1()
                .border_color(theme::border())
                .rounded_md()
                .p_1()
                .children(certificates),
        )
        .child(crate::word_review_tools::field(
            "sign-purpose",
            "sign.purpose",
            &form.purpose,
            true,
            |_| {},
            cx,
        ))
        .into_any_element()
}

/// The 署名欄の設定 dialog's body.
pub fn line_body(form: &LineForm, cx: &mut Context<DocumentView>) -> AnyElement {
    let ids = ["sign-line-0", "sign-line-1", "sign-line-2", "sign-line-3"];
    let focus: [fn(&mut ReviewDialog); 4] = [
        |dialog| {
            if let ReviewDialog::SignatureLine(form) = dialog {
                form.focus = 0;
            }
        },
        |dialog| {
            if let ReviewDialog::SignatureLine(form) = dialog {
                form.focus = 1;
            }
        },
        |dialog| {
            if let ReviewDialog::SignatureLine(form) = dialog {
                form.focus = 2;
            }
        },
        |dialog| {
            if let ReviewDialog::SignatureLine(form) = dialog {
                form.focus = 3;
            }
        },
    ];
    let mut body = div().flex().flex_col().gap_2();
    for index in 0..4 {
        body = body.child(crate::word_review_tools::field(
            ids[index],
            LineForm::LABELS[index],
            &form.fields[index],
            form.focus == index,
            focus[index],
            cx,
        ));
    }
    body.into_any_element()
}

/// The 署名 pane, beside the pages while it is open.
pub fn pane(view: &mut DocumentView, cx: &mut Context<DocumentView>) -> Option<AnyElement> {
    if !view.review.signatures.pane {
        return None;
    }
    let state = &view.review.signatures.state;
    let mut rows: Vec<AnyElement> = Vec::new();
    rows.push(
        div()
            .text_color(theme::text_dim())
            .child(i18n::text("sign.valid_heading"))
            .into_any_element(),
    );
    if state.signatures.is_empty() {
        rows.push(
            div()
                .text_color(theme::text_faint())
                .child(i18n::text("sign.none"))
                .into_any_element(),
        );
    }
    for signature in &state.signatures {
        let valid = state.status(signature) == SignatureStatus::Valid;
        let detail = [
            signature.signed_at.clone(),
            signature.comment.clone(),
            signature
                .issuer
                .clone()
                .map(|issuer| i18n::format("sign.issuer", &[("issuer", &issuer)])),
        ]
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
                .border_color(theme::border())
                .bg(theme::canvas())
                .child(
                    div()
                        .flex()
                        .gap_1()
                        .child(
                            div()
                                .text_color(if valid {
                                    theme::accent()
                                } else {
                                    gpui::rgb(0xC00000)
                                })
                                .child(if valid { "✓" } else { "✕" }),
                        )
                        .child(
                            div()
                                .text_color(theme::text())
                                .child(signature.signer.clone().unwrap_or_default()),
                        ),
                )
                .child(
                    div()
                        .text_color(theme::text_dim())
                        .child(i18n::text(if valid {
                            "sign.valid"
                        } else {
                            "sign.invalid"
                        })),
                )
                .when(!detail.is_empty(), |card| {
                    card.child(div().text_color(theme::text_faint()).child(detail))
                })
                .into_any_element(),
        );
    }
    let lines = view.document.signature_lines();
    if !lines.is_empty() {
        rows.push(
            div()
                .mt_2()
                .text_color(theme::text_dim())
                .child(i18n::text("sign.requested"))
                .into_any_element(),
        );
        for line in lines {
            rows.push(
                div()
                    .p_2()
                    .rounded_md()
                    .border_1()
                    .border_color(theme::border())
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
    Some(
        div()
            .id("signatures-pane")
            .flex()
            .flex_col()
            .flex_none()
            .w(px(280.0))
            .h_full()
            .gap_2()
            .p_3()
            .overflow_y_scroll()
            .bg(theme::panel())
            .border_l_1()
            .border_color(theme::border())
            .text_xs()
            .text_color(theme::text())
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(div().text_sm().child(i18n::text("sign.pane")))
                    .child(
                        div()
                            .id("signatures-close")
                            .px_1()
                            .rounded_md()
                            .cursor_pointer()
                            .hover(|close| close.bg(theme::hovered()))
                            .child("×")
                            .on_click(
                                cx.listener(|view, _: &ClickEvent, _, cx| toggle_pane(view, cx)),
                            ),
                    ),
            )
            .children(rows)
            .child(
                div().flex().child(
                    button("signatures-sign", i18n::text("sign.button"), false)
                        .on_click(cx.listener(|view, _: &ClickEvent, _, cx| open_sign(view, cx))),
                ),
            )
            .into_any_element(),
    )
}

/// The 挿入 tab's 署名欄 group.
pub fn insert_group(view: &mut DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    let _ = &view;
    DocumentView::ribbon_group(
        i18n::text("sign.line.group"),
        vec![DocumentView::ribbon_row(vec![
            crate::word_review_tools::text_button(
                "insert-signature-line",
                "sign.line.button",
                "sign.line.tooltip",
                false,
                crate::word_review_tools::ReviewAction::SignatureLine,
                cx,
            ),
        ])],
    )
    .into_any_element()
}

mod keychain {
    //! The keychain's certificates with a private key (identities), and
    //! signing with one.

    use jade_office_model::word::signatures::Signer;

    #[cfg(all(target_os = "macos", not(test)))]
    fn all() -> Vec<(String, security_framework::identity::SecIdentity)> {
        use security_framework::item::{
            ItemClass, ItemSearchOptions, Limit, Reference, SearchResult,
        };
        let Ok(results) = ItemSearchOptions::new()
            .class(ItemClass::identity())
            .load_refs(true)
            .limit(Limit::All)
            .search()
        else {
            return Vec::new();
        };
        results
            .into_iter()
            .filter_map(|result| match result {
                SearchResult::Ref(Reference::Identity(identity)) => {
                    let name = identity.certificate().ok()?.subject_summary();
                    Some((name, identity))
                }
                _ => None,
            })
            .collect()
    }

    /// The identities' names.
    pub fn identities() -> Vec<String> {
        #[cfg(all(target_os = "macos", not(test)))]
        {
            all().into_iter().map(|(name, _)| name).collect()
        }
        #[cfg(any(not(target_os = "macos"), test))]
        {
            Vec::new()
        }
    }

    #[cfg(all(target_os = "macos", not(test)))]
    struct KeychainSigner(security_framework::identity::SecIdentity);

    #[cfg(all(target_os = "macos", not(test)))]
    impl Signer for KeychainSigner {
        fn certificate(&self) -> Vec<u8> {
            self.0
                .certificate()
                .map(|certificate| certificate.to_der())
                .unwrap_or_default()
        }

        fn sign_sha256(&self, data: &[u8]) -> Result<Vec<u8>, String> {
            use security_framework::key::Algorithm;
            let key = self.0.private_key().map_err(|error| error.to_string())?;
            key.create_signature(Algorithm::RSASignatureMessagePKCS1v15SHA256, data)
                .map_err(|error| error.to_string())
        }
    }

    /// A signer for the identity named `name`.
    pub fn signer(name: Option<&str>) -> Option<Box<dyn Signer>> {
        #[cfg(all(target_os = "macos", not(test)))]
        {
            let name = name?;
            all()
                .into_iter()
                .find(|(found, _)| found == name)
                .map(|(_, identity)| Box::new(KeychainSigner(identity)) as Box<dyn Signer>)
        }
        #[cfg(any(not(target_os = "macos"), test))]
        {
            let _ = name;
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clock_reads_as_a_signature_time() {
        let now = now_utc();
        assert_eq!(now.len(), 20);
        assert!(now.ends_with('Z') && now.as_bytes()[10] == b'T');
        assert!(now.as_str() > "2025-01-01");
    }

    /// 署名欄: the dialog's fields make the line, inserted at the caret
    /// with its rendered picture; the pane lists it as waiting for a
    /// signature.
    #[gpui::test]
    fn a_signature_line_is_inserted_from_the_dialog(cx: &mut gpui::TestAppContext) {
        use gpui::Focusable;
        let bytes = jade_office_model::word::new::blank_document(
            jade_office_model::word::new::Blank::English,
        );
        let (view, cx) = cx
            .add_window_view(|_window, cx| crate::untitled_view(&bytes, "sign.docx", cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        view.update(cx, |view, cx| {
            view.insert_at = (0, 0);
            view.caret = view.document.place_caret(0, 0);
            crate::word_review_tools::run(
                view,
                crate::word_review_tools::ReviewAction::SignatureLine,
                cx,
            );
            assert!(matches!(
                view.review.dialog,
                Some(ReviewDialog::SignatureLine(_))
            ));
        });
        cx.simulate_input("Ann Lee");
        cx.simulate_keystrokes("tab");
        cx.simulate_input("Director");
        cx.simulate_keystrokes("enter");
        view.update(cx, |view, cx| {
            assert!(view.review.dialog.is_none());
            let lines = view.document.signature_lines();
            assert_eq!(lines.len(), 1, "{:?}", view.table_status);
            assert_eq!(lines[0].signer.as_deref(), Some("Ann Lee"));
            assert_eq!(lines[0].title.as_deref(), Some("Director"));
            toggle_pane(view, cx);
            assert!(pane(view, cx).is_some());
        });
    }

    /// A signed document opens read-only with the bar; 編集する makes its
    /// signatures invalid.
    #[gpui::test]
    fn editing_a_signed_document_invalidates_its_signatures(cx: &mut gpui::TestAppContext) {
        let bytes = jade_office_model::word::new::blank_document(
            jade_office_model::word::new::Blank::English,
        );
        let (view, cx) = cx.add_window_view(|_window, cx| {
            crate::untitled_view(&bytes, "signed.docx", cx).unwrap()
        });
        cx.run_until_parked();
        view.update(cx, |view, cx| {
            view.review.signatures.state.signatures =
                vec![jade_office_model::word::signatures::PackageSignature {
                    part: "_xmlsignatures/sig1.xml".into(),
                    signer: Some("Ann".into()),
                    issuer: None,
                    signed_at: None,
                    comment: None,
                    status: SignatureStatus::Valid,
                }];
            view.review.protection.read_only = Some(ReadOnlyReason::Signed);
            view.document.set_opened_read_only(true);
            crate::word_protection::edit_anyway(view, cx);
            let state = &view.review.signatures.state;
            assert!(state.edited);
            assert!(matches!(
                state.status(&state.signatures[0]),
                SignatureStatus::Invalid(_)
            ));
            let saved = finish_save(view, view.document.export_docx().unwrap()).unwrap();
            assert!(!jade_office_model::word::signatures::is_signed(&saved));
            assert!(!view.review.signatures.state.is_signed());
        });
    }
}

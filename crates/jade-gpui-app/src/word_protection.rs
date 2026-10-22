//! 校閲 › 保護 in the native app: the 編集の制限 pane (書式の制限, 編集の制限
//! with 変更不可 / 変更履歴 / コメント / フォームへの入力, 例外処理 for
//! everyone, 保護の開始 and 保護の中止 with a password), 文書の保護 (読み取り
//! パスワード, 書き込みパスワード, 読み取り専用を推奨), 最終版にする, and the
//! bar a document recommended read-only or marked final opens under. A
//! password to open is asked for when such a file is opened
//! (`workspace`) and put back on every save. What each does is
//! `jade-office-model`'s `word::protection`.

use gpui::{div, prelude::*, px, AnyElement, ClickEvent, Context};
use jade_office_model::text::TextEdit;
use jade_office_model::word::protection::{
    encrypt_package, mark_final, masked, random_bytes, salt, EditingKind, ProtectForm,
    WordProtection,
};

use crate::word_characters::{button, check};
use crate::word_review_tools::ReviewDialog;
use crate::{i18n, theme, DocumentView};

/// Why the document was opened read-only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadOnlyReason {
    Recommended,
    Final,
    /// The document carries digital signatures (`word_signatures`).
    Signed,
}

/// The view's share of 保護.
#[derive(Default)]
pub struct ProtectionUi {
    /// The 編集の制限 pane is open.
    pub pane: bool,
    /// The pane's choices.
    pub kind: Option<EditingKind>,
    pub formatting: bool,
    /// The pane's password box, and whether it has the keyboard.
    pub password: Option<TextEdit>,
    /// 読み取りパスワード: the file is encrypted with it on every save.
    pub open_password: Option<String>,
    /// 最終版: what the next save writes, once changed.
    pub mark_final: Option<bool>,
    /// The bar over the pages while the document is read-only.
    pub read_only: Option<ReadOnlyReason>,
}

impl ProtectionUi {
    /// For a document just opened: read-only, with its bar, when it is
    /// marked final or recommends it.
    pub fn opened(document: &mut jade_docx_native_render::NativeDocument) -> Self {
        let properties = document.word_document().document_properties();
        let final_ = properties
            .get("_MarkAsFinal")
            .is_some_and(|value| matches!(value.as_str(), "true" | "1"));
        let recommended = document
            .write_protection()
            .is_some_and(|protection| protection.recommended);
        let read_only = if final_ {
            Some(ReadOnlyReason::Final)
        } else if recommended {
            Some(ReadOnlyReason::Recommended)
        } else {
            None
        };
        if read_only.is_some() {
            document.set_opened_read_only(true);
        }
        let restriction = document.editing_restriction();
        Self {
            kind: restriction
                .as_ref()
                .and_then(|restriction| restriction.kind),
            formatting: restriction.is_some_and(|restriction| restriction.formatting),
            read_only,
            ..Self::default()
        }
    }
}

/// The package a save writes: marked final or not, then encrypted with
/// the password to open.
pub fn finish_save(view: &DocumentView, bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    let protection = &view.review.protection;
    let bytes = match protection.mark_final {
        Some(final_) => mark_final(&bytes, final_)?,
        None => bytes,
    };
    match protection.open_password.as_deref() {
        Some(password) if !password.is_empty() => {
            encrypt_package(&bytes, password, &mut random_bytes).map_err(|error| error.to_string())
        }
        _ => Ok(bytes),
    }
}

/// 編集する: the read-only bar's way out. A final document stops being
/// final with the next save, as Word's 編集する does.
pub fn edit_anyway(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    match view.review.protection.read_only.take() {
        Some(ReadOnlyReason::Final) => {
            view.review.protection.mark_final = Some(false);
            view.dirty = true;
        }
        Some(ReadOnlyReason::Signed) => crate::word_signatures::edited(view),
        _ => {}
    }
    view.document.set_opened_read_only(false);
    cx.notify();
}

/// 最終版にする: marked final with the next save — which follows at once,
/// as Word saves on marking — and read-only from now.
pub fn toggle_final(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let final_ = view.review.protection.read_only != Some(ReadOnlyReason::Final);
    view.review.protection.mark_final = Some(final_);
    view.review.protection.read_only = final_.then_some(ReadOnlyReason::Final);
    view.document.set_opened_read_only(final_);
    view.dirty = true;
    cx.notify();
}

/// 文書の保護….
pub fn open_protect_dialog(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let recommended = view
        .document
        .write_protection()
        .is_some_and(|protection| protection.recommended);
    let form = ProtectForm::new(view.review.protection.open_password.as_deref(), recommended);
    view.review.dialog = Some(ReviewDialog::Protect(Box::new(form)));
    cx.notify();
}

/// 文書の保護's OK.
pub fn confirm_protect(view: &mut DocumentView, form: ProtectForm, cx: &mut Context<DocumentView>) {
    let open = form.open_password.text.clone();
    view.review.protection.open_password = (!open.is_empty()).then_some(open);
    let modify = form.modify_password.text.clone();
    let result = view.document.set_write_protection(
        form.recommended,
        (!modify.is_empty()).then_some(modify.as_str()),
        salt(),
    );
    view.dirty = true;
    view.reviewed(result, cx);
}

/// The 文書の保護 dialog's body.
pub fn protect_body(form: &ProtectForm, cx: &mut Context<DocumentView>) -> AnyElement {
    let field = |id: &'static str,
                 label: &'static str,
                 text: &TextEdit,
                 focused: bool,
                 index: usize,
                 cx: &mut Context<DocumentView>| {
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .w(px(150.0))
                    .text_color(theme::text_dim())
                    .child(i18n::text(label)),
            )
            .child(
                div()
                    .id(id)
                    .flex()
                    .items_center()
                    .w(px(200.0))
                    .h(px(24.0))
                    .px_2()
                    .rounded_md()
                    .border_1()
                    .border_color(if focused {
                        theme::accent()
                    } else {
                        theme::border()
                    })
                    .bg(theme::canvas())
                    .cursor_text()
                    .child(masked(&text.text))
                    .when(focused, |row| {
                        row.child(div().w(px(1.0)).h(px(16.0)).bg(theme::accent()))
                    })
                    .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                        if let Some(ReviewDialog::Protect(form)) = view.review.dialog.as_mut() {
                            form.focus = index;
                        }
                        cx.notify();
                    })),
            )
            .into_any_element()
    };
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_color(theme::text_dim())
                .child(i18n::text("protect.open_hint")),
        )
        .child(field(
            "protect-open",
            "protect.open",
            &form.open_password,
            form.focus == 0,
            0,
            cx,
        ))
        .child(
            div()
                .text_color(theme::text_dim())
                .child(i18n::text("protect.modify_hint")),
        )
        .child(field(
            "protect-modify",
            "protect.modify",
            &form.modify_password,
            form.focus == 1,
            1,
            cx,
        ))
        .child(
            check(
                "protect-recommended",
                i18n::text("protect.recommended"),
                form.recommended,
            )
            .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
                if let Some(ReviewDialog::Protect(form)) = view.review.dialog.as_mut() {
                    form.recommended = !form.recommended;
                }
                cx.notify();
            })),
        )
        .into_any_element()
}

/// The read-only bar over the pages.
pub fn banner(view: &DocumentView, cx: &mut Context<DocumentView>) -> Option<AnyElement> {
    let reason = view.review.protection.read_only?;
    let key = match reason {
        ReadOnlyReason::Final => "protect.final_bar",
        ReadOnlyReason::Recommended => "protect.recommended_bar",
        ReadOnlyReason::Signed => "sign.bar",
    };
    Some(
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap_3()
            .px_4()
            .py_1p5()
            .bg(theme::panel())
            .border_b_1()
            .border_color(theme::border())
            .text_xs()
            .text_color(theme::text())
            .child(div().child(i18n::text(key)))
            .child(
                button(
                    "protect-edit-anyway",
                    i18n::text("protect.edit_anyway"),
                    true,
                )
                .on_click(cx.listener(|view, _: &ClickEvent, _, cx| edit_anyway(view, cx))),
            )
            .into_any_element(),
    )
}

/// 編集の制限: the pane, open or shut.
pub fn toggle_pane(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    view.review.protection.pane = !view.review.protection.pane;
    cx.notify();
}

/// What the pane's controls do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaneAction {
    Formatting,
    Kind(Option<EditingKind>),
    AllowEveryone,
    RemoveException(usize),
    Start,
    Stop,
    PasswordBox,
}

/// Runs a pane control.
pub fn pane_action(view: &mut DocumentView, action: PaneAction, cx: &mut Context<DocumentView>) {
    let password = view
        .review
        .protection
        .password
        .take()
        .map(|field| field.text)
        .unwrap_or_default();
    let result = match action {
        PaneAction::Formatting => {
            view.review.protection.formatting = !view.review.protection.formatting;
            Ok(())
        }
        PaneAction::Kind(kind) => {
            view.review.protection.kind = kind;
            Ok(())
        }
        PaneAction::PasswordBox => {
            view.review.protection.password = Some(TextEdit::new(password));
            return cx.notify();
        }
        PaneAction::AllowEveryone => {
            if view.document.selection().is_none() {
                view.document
                    .place_caret(view.insert_at.0, view.insert_at.1);
            }
            view.document.allow_everyone_here()
        }
        PaneAction::RemoveException(index) => {
            match view
                .document
                .editable_ranges()
                .get(index)
                .map(|range| range.id.clone())
            {
                Some(id) => view.document.remove_exception(&id),
                None => Ok(()),
            }
        }
        PaneAction::Start => {
            let protection = &view.review.protection;
            let (kind, formatting) = (protection.kind, protection.formatting);
            let result = view.document.start_restriction(
                kind,
                formatting,
                (!password.is_empty()).then_some(password.as_str()),
                salt(),
            );
            // 変更履歴: changes are kept from now on.
            if result.is_ok() && kind == Some(EditingKind::TrackedChanges) {
                use jade_office_model::word::review::WordReview;
                let reviewer = crate::word_review_tools::reviewer();
                view.document
                    .set_tracking_changes(true, &reviewer)
                    .and(result)
            } else {
                result
            }
        }
        PaneAction::Stop => view.document.stop_restriction(&password),
    };
    view.reviewed(result, cx);
}

fn section(title: &'static str) -> AnyElement {
    div()
        .mt_2()
        .text_color(theme::text_dim())
        .child(i18n::text(title))
        .into_any_element()
}

/// The 編集の制限 pane, beside the pages while it is open.
pub fn pane(view: &DocumentView, cx: &mut Context<DocumentView>) -> Option<AnyElement> {
    let protection = &view.review.protection;
    if !protection.pane {
        return None;
    }
    let restriction = view.document.editing_restriction();
    let enforced = restriction
        .as_ref()
        .is_some_and(|restriction| restriction.enforced);
    fn act(
        id: impl Into<gpui::ElementId>,
        key: &'static str,
        primary: bool,
        action: PaneAction,
        cx: &mut Context<DocumentView>,
    ) -> AnyElement {
        button(id, i18n::text(key), primary)
            .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| pane_action(view, action, cx)))
            .into_any_element()
    }
    let choice = |id: &'static str,
                  key: &'static str,
                  on: bool,
                  action: PaneAction,
                  cx: &mut Context<DocumentView>| {
        check(id, i18n::text(key), on)
            .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| pane_action(view, action, cx)))
            .into_any_element()
    };
    let mut rows: Vec<AnyElement> = vec![
        section("restrict.formatting_title"),
        choice(
            "restrict-formatting",
            "restrict.formatting",
            protection.formatting,
            PaneAction::Formatting,
            cx,
        ),
        section("restrict.editing_title"),
    ];
    for (index, (key, kind)) in [
        ("restrict.none", None),
        ("restrict.read_only", Some(EditingKind::ReadOnly)),
        ("restrict.tracked", Some(EditingKind::TrackedChanges)),
        ("restrict.comments", Some(EditingKind::Comments)),
        ("restrict.forms", Some(EditingKind::Forms)),
    ]
    .into_iter()
    .enumerate()
    {
        rows.push(
            check(
                ("restrict-kind", index),
                i18n::text(key),
                protection.kind == kind,
            )
            .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                pane_action(view, PaneAction::Kind(kind), cx)
            }))
            .into_any_element(),
        );
    }
    rows.push(section("restrict.exceptions"));
    rows.push(act(
        "restrict-everyone",
        "restrict.everyone",
        false,
        PaneAction::AllowEveryone,
        cx,
    ));
    for (index, range) in view.document.editable_ranges().into_iter().enumerate() {
        let text = view
            .document
            .paragraph_text(range.start.0)
            .map(|text| text.chars().take(24).collect::<String>())
            .unwrap_or_default();
        rows.push(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    div()
                        .flex_1()
                        .text_color(theme::text_dim())
                        .child(format!("{} · {text}", range.editors)),
                )
                .child(act(
                    ("restrict-remove", index),
                    "review.delete",
                    false,
                    PaneAction::RemoveException(index),
                    cx,
                ))
                .into_any_element(),
        );
    }
    rows.push(section(if enforced {
        "restrict.stop_title"
    } else {
        "restrict.start_title"
    }));
    let typing = protection.password.as_ref();
    rows.push(
        div()
            .id("restrict-password")
            .flex()
            .items_center()
            .h(px(24.0))
            .px_2()
            .rounded_md()
            .border_1()
            .border_color(if typing.is_some() {
                theme::accent()
            } else {
                theme::border()
            })
            .bg(theme::canvas())
            .cursor_text()
            .text_color(theme::text())
            .child(match typing {
                Some(field) => masked(&field.text),
                None => i18n::text("restrict.password").to_string(),
            })
            .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
                pane_action(view, PaneAction::PasswordBox, cx)
            }))
            .into_any_element(),
    );
    rows.push(if enforced {
        act("restrict-stop", "restrict.stop", true, PaneAction::Stop, cx)
    } else {
        act(
            "restrict-start",
            "restrict.start",
            true,
            PaneAction::Start,
            cx,
        )
    });
    if enforced {
        rows.push(
            div()
                .text_color(theme::text_faint())
                .child(i18n::text("restrict.enforced"))
                .into_any_element(),
        );
    }
    Some(
        div()
            .id("restrict-pane")
            .flex()
            .flex_col()
            .flex_none()
            .w(px(260.0))
            .h_full()
            .gap_1()
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
                    .child(div().text_sm().child(i18n::text("restrict.title")))
                    .child(
                        div()
                            .id("restrict-close")
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
            .into_any_element(),
    )
}

// ---- the password to open ------------------------------------------------------

/// The prompt a document with a password to open is opened through.
pub struct PasswordPrompt {
    pub path: std::path::PathBuf,
    pub field: TextEdit,
    pub wrong: bool,
}

/// Whether `path` is a Word document encrypted with a password to open.
pub fn is_encrypted_file(path: &std::path::Path) -> bool {
    std::fs::read(path)
        .is_ok_and(|bytes| jade_office_model::word::protection::is_encrypted_package(&bytes))
}

/// The document view for `path` opened with `password`, which saves back
/// encrypted with it.
pub fn open_encrypted(
    path: &std::path::Path,
    password: &str,
    cx: &mut gpui::App,
) -> Result<DocumentView, jade_office_model::word::protection::DecryptError> {
    use jade_office_model::word::protection::{decrypt_package, DecryptError};
    let bytes = std::fs::read(path).map_err(|error| DecryptError::Malformed(error.to_string()))?;
    let package = decrypt_package(&bytes, password)?;
    let document = crate::open_document_bytes(&package).map_err(DecryptError::Malformed)?;
    let mut view = crate::document_view(
        document,
        path.to_string_lossy().into_owned(),
        bytes.len() as u64,
        cx,
    );
    view.review.protection.open_password = Some(password.to_string());
    Ok(view)
}

/// Keys the prompt takes: typing goes in as dots, Enter tries the
/// password, Escape gives up.
pub fn prompt_key(
    workspace: &mut crate::workspace::Workspace,
    event: &gpui::KeyDownEvent,
    window: &mut gpui::Window,
    cx: &mut Context<crate::workspace::Workspace>,
) {
    let Some(prompt) = workspace.password.as_mut() else {
        return;
    };
    match event.keystroke.key.as_str() {
        "escape" => workspace.password = None,
        "enter" | "return" => {
            let path = prompt.path.clone();
            let password = prompt.field.text.clone();
            match open_encrypted(&path, &password, cx) {
                Ok(view) => {
                    workspace.password = None;
                    workspace.add_document(path, view, window, cx);
                }
                Err(jade_office_model::word::protection::DecryptError::WrongPassword) => {
                    prompt.wrong = true;
                    prompt.field = TextEdit::new(String::new());
                }
                Err(error) => {
                    workspace.password = None;
                    workspace.show_error(error.to_string(), cx);
                }
            }
        }
        key => {
            if !prompt.field.edit_key(key) {
                match event.keystroke.key_char.as_deref() {
                    Some(text) if !event.keystroke.modifiers.platform => {
                        prompt.field.insert(text);
                        prompt.wrong = false;
                    }
                    _ => return,
                }
            }
        }
    }
    cx.stop_propagation();
    cx.notify();
}

/// The prompt, over the window.
pub fn prompt(
    workspace: &crate::workspace::Workspace,
    window: &mut gpui::Window,
) -> Option<AnyElement> {
    use gpui::{anchored, deferred, point};
    let prompt = workspace.password.as_ref()?;
    let viewport = window.viewport_size();
    let name = prompt
        .path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    Some(
        deferred(
            anchored().position(point(px(0.0), px(0.0))).child(
                div()
                    .w(viewport.width)
                    .h(viewport.height)
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(gpui::rgba(0x00000040))
                    .occlude()
                    .child(
                        div()
                            .p_4()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .w(px(360.0))
                            .rounded_lg()
                            .border_1()
                            .border_color(theme::border())
                            .bg(theme::panel())
                            .shadow_lg()
                            .font_family(crate::UI_FONT_FAMILY)
                            .text_xs()
                            .text_color(theme::text())
                            .child(div().text_sm().child(i18n::text("protect.password_title")))
                            .child(i18n::format("protect.password_prompt", &[("name", &name)]))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .h(px(24.0))
                                    .px_2()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(theme::accent())
                                    .bg(theme::canvas())
                                    .child(masked(&prompt.field.text))
                                    .child(div().w(px(1.0)).h(px(16.0)).bg(theme::accent())),
                            )
                            .when(prompt.wrong, |body| {
                                body.child(
                                    div()
                                        .text_color(theme::accent())
                                        .child(i18n::text("protect.wrong")),
                                )
                            }),
                    ),
            ),
        )
        .with_priority(5)
        .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// From the pane: a read-only restriction with a password stops typing;
    /// 保護の中止 with it lets typing in again. 最終版にする and a password
    /// to open come back with the saved file.
    #[gpui::test]
    fn restricting_from_the_pane_and_saving_protected(cx: &mut gpui::TestAppContext) {
        use gpui::Focusable;
        let bytes = jade_office_model::word::new::blank_document(
            jade_office_model::word::new::Blank::English,
        );
        let (view, cx) = cx
            .add_window_view(|_window, cx| crate::untitled_view(&bytes, "lock.docx", cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        view.update(cx, |view, cx| {
            view.insert_at = (0, 0);
            view.caret = view.document.place_caret(0, 0);
            toggle_pane(view, cx);
            pane_action(view, PaneAction::Kind(Some(EditingKind::ReadOnly)), cx);
            pane_action(view, PaneAction::PasswordBox, cx);
        });
        cx.simulate_input("pw");
        view.update(cx, |view, cx| {
            assert_eq!(view.review.protection.password.as_ref().unwrap().text, "pw");
            pane_action(view, PaneAction::Start, cx);
            assert!(view.document.is_restricted());
            view.insert_at = (0, 0);
            view.caret = view.document.place_caret(0, 0);
        });
        cx.simulate_input("blocked");
        view.update(cx, |view, cx| {
            assert!(!view.document.paragraph_text(0).unwrap().contains("blocked"));
            pane_action(view, PaneAction::PasswordBox, cx);
        });
        cx.simulate_input("pw");
        view.update(cx, |view, cx| {
            pane_action(view, PaneAction::Stop, cx);
            assert!(!view.document.is_restricted());
            view.insert_at = (0, 0);
            view.caret = view.document.place_caret(0, 0);
        });
        cx.simulate_input("free");
        view.update(cx, |view, cx| {
            assert!(view.document.paragraph_text(0).unwrap().contains("free"));
            toggle_final(view, cx);
            view.review.protection.open_password = Some("open".into());
            let saved = finish_save(view, view.document.export_docx().unwrap()).unwrap();
            assert!(jade_office_model::word::protection::is_encrypted_package(
                &saved
            ));
            let plain =
                jade_office_model::word::protection::decrypt_package(&saved, "open").unwrap();
            assert!(jade_office_model::word::protection::is_marked_final(&plain));
            // Final: nothing more is typed until 編集する.
            assert_eq!(
                view.review.protection.read_only,
                Some(ReadOnlyReason::Final)
            );
            edit_anyway(view, cx);
            assert_eq!(view.review.protection.mark_final, Some(false));
        });
    }
    /// A file with a password to open asks for it: a wrong one is said to
    /// be wrong, the right one opens the document, which saves encrypted.
    #[gpui::test]
    fn a_password_to_open_is_asked_for(cx: &mut gpui::TestAppContext) {
        use jade_office_model::word::protection::{encrypt_package, is_encrypted_package};
        let plain = jade_office_model::word::new::blank_document(
            jade_office_model::word::new::Blank::English,
        );
        let mut counter = 0u8;
        let locked = encrypt_package(&plain, "abc", &mut |buffer| {
            for byte in buffer {
                counter = counter.wrapping_add(29);
                *byte = counter;
            }
        })
        .unwrap();
        let directory = std::env::temp_dir().join(format!("jade-password-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("locked.docx");
        std::fs::write(&path, &locked).unwrap();
        let (workspace, cx) =
            cx.add_window_view(|_window, cx| crate::workspace::Workspace::new(cx));
        cx.update(|window, cx| {
            workspace.update(cx, |workspace, cx| workspace.open(&path, window, cx))
        })
        .unwrap();
        workspace.read_with(cx, |workspace, _| assert!(workspace.password.is_some()));
        cx.simulate_keystrokes("x enter");
        workspace.read_with(cx, |workspace, _| {
            assert!(workspace.password.as_ref().unwrap().wrong);
        });
        cx.simulate_keystrokes("a b c enter");
        workspace.read_with(cx, |workspace, _| {
            assert!(workspace.password.is_none(), "opened");
        });
        // The file itself is left as it was.
        let saved = std::fs::read(&path).unwrap();
        assert!(is_encrypted_package(&saved));
        let _ = std::fs::remove_dir_all(&directory);
    }
}

//! The 差し込み文書 tab, as Word for Mac has it: 作成 (封筒, ラベル),
//! 差し込み印刷の開始 (the start menu, 宛先の選択, 宛先リストの編集),
//! 文章入力とフィールドの挿入 (差し込みフィールドの強調表示, 住所ブロック,
//! あいさつ文, 差し込みフィールドの挿入, ルール, フィールドの対応,
//! ラベルの更新), 結果のプレビュー (preview, the record arrows, エラーのチェ
//! ック) and 完了 (完了と差し込み). The merge's state and every dialog are
//! `jade-office-model`'s (`word::mailings`); this module draws them, and does
//! what only the shell can: ask for a list and save one, open the merged
//! document, write it as a PDF, print it. The browser shell has the same
//! (`jade-gpui-web/src/view/word_mailings.rs`).

use std::path::{Path, PathBuf};

use gpui::{div, prelude::*, px, AnyElement, Context, SharedString, Window};
use jade_docx_native_render::document::NativeDocument;
use jade_office_model::word::form_view::FormEvent;
use jade_office_model::word::mailings::{MailMerge, MailOutcome};

use crate::{i18n, word_form, DocumentView};

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

/// The merge a document just opened has, with the list it names read when
/// the file is where it says.
pub fn open_merge(document: &NativeDocument, path: &str) -> MailMerge {
    let japanese = i18n::language() == i18n::Language::Japanese;
    let mut merge = MailMerge::read(document, japanese);
    if let Some(source) = merge.wanted_source.clone() {
        // A relative name is beside the document.
        let candidate = if Path::new(&source).is_absolute() {
            PathBuf::from(&source)
        } else {
            Path::new(path)
                .parent()
                .map(|parent| parent.join(&source))
                .unwrap_or_else(|| PathBuf::from(&source))
        };
        if let Ok(bytes) = std::fs::read(&candidate) {
            let name = candidate
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            if let Err(error) = merge.attach_data_source(&name, Some(source), &bytes) {
                eprintln!("mail merge data source: {error}");
            }
        }
    }
    merge
}

impl DocumentView {
    /// The tab's groups.
    pub(crate) fn mailings_groups(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let groups = self.mailings.ribbon(&self.document, t);
        word_form::tool_groups(
            &groups,
            |view, event, cx| view.mailings_ribbon_event(event, cx),
            cx,
        )
    }

    fn mailings_ribbon_event(&mut self, event: FormEvent, cx: &mut Context<Self>) {
        self.sync_mailings_selection();
        let outcome = self.mailings.ribbon_event(&mut self.document, event);
        self.mailings_outcome(outcome, cx);
    }

    fn mailings_dialog_event(&mut self, event: FormEvent, cx: &mut Context<Self>) {
        let outcome = self.mailings.dialog_event(&mut self.document, event);
        self.mailings_outcome(outcome, cx);
    }

    /// The view's caret or selection made the document's, so a field goes
    /// in where the user sees the caret.
    fn sync_mailings_selection(&mut self) {
        if self.document.selection().is_none() {
            self.document
                .place_caret(self.insert_at.0, self.insert_at.1);
        }
    }

    /// Does what the merge asks of the shell.
    fn mailings_outcome(&mut self, outcome: Result<MailOutcome, String>, cx: &mut Context<Self>) {
        match outcome {
            Err(error) | Ok(MailOutcome::Problem(error)) => {
                self.table_status = Some(SharedString::from(error));
            }
            Ok(MailOutcome::None) => {}
            Ok(MailOutcome::Edited) => {
                self.table_status = None;
                self.dirty = true;
                self.show_document_selection();
                self.rebuild_all_images();
            }
            Ok(MailOutcome::PickDataSource) => self.pick_data_source(cx),
            Ok(MailOutcome::SaveDataSource { name, bytes }) => {
                self.save_data_source(name, bytes, cx)
            }
            Ok(MailOutcome::OpenDocument { name, bytes }) => {
                crate::shell::open_untitled(name, bytes, cx);
            }
            Ok(MailOutcome::SavePdf { name, bytes }) => self.save_merged_pdf(name, bytes, cx),
            Ok(MailOutcome::Print { bytes }) => {
                let printed = jade_docx_pdf_renderer::render_docx_to_pdf(
                    &bytes,
                    &jade_docx_pdf_renderer::RenderOptions::default(),
                )
                .map_err(|error| format!("PDF render failed: {error:?}"))
                .and_then(|pdf| crate::native_print::show_pdf_print_dialog(&pdf));
                if let Err(error) = printed {
                    self.table_status = Some(SharedString::from(error));
                }
            }
        }
        cx.notify();
    }

    /// 既存のリストを使用: a CSV, text file or workbook from the disk.
    fn pick_data_source(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(SharedString::from(t("mail.recipients.existing"))),
        });
        cx.spawn(async move |view, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.first().cloned() else {
                return;
            };
            let bytes = match std::fs::read(&path) {
                Ok(bytes) => bytes,
                Err(error) => {
                    eprintln!("{}: {error}", path.display());
                    return;
                }
            };
            view.update(cx, |view, cx| {
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let source = Some(path.to_string_lossy().into_owned());
                let outcome =
                    view.mailings
                        .load_data_source(&mut view.document, &name, source, &bytes);
                view.mailings_outcome(outcome, cx);
            })
            .ok();
        })
        .detach();
    }

    /// A list typed here, saved where the user says, then used.
    fn save_data_source(&mut self, name: String, bytes: Vec<u8>, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_new_path(&self.save_directory(), Some(&name));
        cx.spawn(async move |view, cx| {
            let Ok(Ok(Some(path))) = prompt.await else {
                return;
            };
            if let Err(error) = std::fs::write(&path, &bytes) {
                eprintln!("{}: {error}", path.display());
                return;
            }
            view.update(cx, |view, cx| {
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or(name);
                let source = Some(path.to_string_lossy().into_owned());
                let outcome = view
                    .mailings
                    .saved_data_source(&mut view.document, &name, source);
                view.mailings_outcome(outcome, cx);
            })
            .ok();
        })
        .detach();
    }

    /// PDF に差し込み: the merged document as a PDF, where the user says.
    fn save_merged_pdf(&mut self, name: String, bytes: Vec<u8>, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_new_path(&self.save_directory(), Some(&name));
        cx.spawn(async move |view, cx| {
            let Ok(Ok(Some(path))) = prompt.await else {
                return;
            };
            let written = jade_docx_pdf_renderer::render_docx_to_pdf(
                &bytes,
                &jade_docx_pdf_renderer::RenderOptions::default(),
            )
            .map_err(|error| format!("PDF render failed: {error:?}"))
            .and_then(|pdf| std::fs::write(&path, pdf).map_err(|error| error.to_string()));
            if let Err(error) = written {
                view.update(cx, |view, cx| {
                    view.table_status = Some(SharedString::from(error));
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    /// Keys while a mail-merge dialog is open. Whether the key was taken.
    pub(crate) fn mailings_key(&mut self, key: &str, shift: bool, cx: &mut Context<Self>) -> bool {
        if let Some(event) = word_form::key_event(key, shift) {
            self.mailings_dialog_event(event, cx);
            return true;
        }
        let Some(field) = self.mailings.field_mut() else {
            return false;
        };
        if field.marked.is_some() || !field.edit_key(key) {
            return false;
        }
        cx.notify();
        true
    }

    /// The open mail-merge dialog, over everything.
    pub(crate) fn mailings_dialog(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let form = self.mailings.dialog_view(t)?;
        Some(word_form::overlay(
            &form,
            window,
            |view, event, cx| view.mailings_dialog_event(event, cx),
            cx,
        ))
    }

    /// 差し込みフィールドの強調表示: the merge fields shaded on `page`.
    pub(crate) fn mailings_overlay(&self, page: usize) -> Vec<AnyElement> {
        if !self.mailings.highlight {
            return Vec::new();
        }
        let zoom = self.zoom;
        self.mailings
            .highlight_regions(&self.document)
            .into_iter()
            .filter_map(|region| {
                let (on, top) = self.document.page_and_offset(region.y);
                (on == page).then(|| {
                    div()
                        .absolute()
                        .left(px(region.x as f32 * zoom))
                        .top(px(top as f32 * zoom))
                        .w(px(region.width.max(2.0) as f32 * zoom))
                        .h(px(region.height as f32 * zoom))
                        .bg(gpui::rgba(0x8080803a))
                        .into_any_element()
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use jade_office_model::word::form_view::{FormEvent, FormId};
    use jade_office_model::word::mailings::MailDialog;

    fn open_contract(
        cx: &mut gpui::TestAppContext,
    ) -> (
        gpui::Entity<crate::DocumentView>,
        &mut gpui::VisualTestContext,
    ) {
        use gpui::Focusable;
        let path = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../jade-core/contract.docx"
        ));
        let (view, cx) =
            cx.add_window_view(|_window, cx| crate::open_document_view(&path, cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        (view, cx)
    }

    /// 差し込み文書: a list used, a merge field put in, a recipient
    /// previewed, and the address block dialog typed into from the keyboard.
    #[gpui::test]
    fn the_mailings_tab_merges_a_list(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_contract(cx);
        view.update(cx, |view, cx| {
            assert!(view
                .ribbon_tabs()
                .iter()
                .any(|(tab, _)| *tab == crate::RibbonTab::Mailings));
            view.ribbon_tab = crate::RibbonTab::Mailings;
            let _ = view.mailings_groups(cx);
            view.mailings_ribbon_event(FormEvent::Choose(FormId::new("mail-start"), 0), cx);
            let outcome = view.mailings.load_data_source(
                &mut view.document,
                "list.csv",
                None,
                "姓,名\n山田,花子\n鈴木,一郎\n".as_bytes(),
            );
            view.mailings_outcome(outcome, cx);
            view.mailings_ribbon_event(FormEvent::Choose(FormId::new("mail-insert-field"), 0), cx);
            view.mailings_ribbon_event(FormEvent::Click(FormId::new("mail-preview")), cx);
            let paragraph = view.insert_at.0;
            assert!(view
                .document
                .paragraph_text(paragraph)
                .unwrap()
                .contains("山田"));
            view.mailings_ribbon_event(FormEvent::Click(FormId::new("mail-next")), cx);
            assert!(view
                .document
                .paragraph_text(paragraph)
                .unwrap()
                .contains("鈴木"));
            view.mailings_ribbon_event(FormEvent::Choose(FormId::new("mail-rules"), 0), cx);
            assert!(matches!(view.mailings.dialog, Some(MailDialog::Rule(_))));
        });
        cx.simulate_input("Answer");
        view.read_with(cx, |view, _| {
            assert_eq!(
                view.mailings.field().map(|field| field.text.as_str()),
                Some("Answer")
            );
        });
        cx.simulate_keystrokes("escape");
        view.read_with(cx, |view, _| assert!(view.mailings.dialog.is_none()));
    }
}

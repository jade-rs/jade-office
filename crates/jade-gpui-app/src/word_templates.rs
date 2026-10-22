//! Word's templates as files and as the document's: the folder
//! テンプレートとして保存 offers and 新規テンプレートから opens, the Normal
//! template every new document starts from, and テンプレートとアドイン —
//! the template the document is attached to, attaching another (and with
//! 文書のスタイルを自動的に更新する, taking its styles), and detaching it.
//! What each does to the document is `jade-office-model`'s
//! (`word::templates`).

use std::path::{Path, PathBuf};

use gpui::{actions, div, prelude::*, px, AnyElement, ClickEvent, Context, SharedString};
use jade_office_model::word::templates;

use crate::word_characters::{button, caption, check};
use crate::word_files::FileDialog;
use crate::{i18n, theme, DocumentView};

actions!(
    jade,
    [
        /// テンプレートとアドイン.
        TemplatesAndAddIns,
    ]
);

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

/// The user's templates folder, made when it is not there yet: what Word
/// for Mac calls マイ テンプレート.
pub fn templates_folder() -> PathBuf {
    let folder = std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join("Library/Application Support/jade-office/Templates"))
        .unwrap_or_else(|| PathBuf::from("."));
    if !cfg!(test) {
        std::fs::create_dir_all(&folder).ok();
    }
    folder
}

/// Where the Normal template is kept.
pub fn normal_template_path() -> PathBuf {
    templates_folder().join("Normal.dotx")
}

/// The Normal template's bytes, when one has been saved.
pub fn normal_template() -> Option<Vec<u8>> {
    if cfg!(test) {
        return None;
    }
    std::fs::read(normal_template_path()).ok()
}

/// A document's styles brought up to date from its attached template, when
/// 文書のスタイルを自動的に更新する is on and the template is there — as
/// Word does each time such a document opens.
pub fn refresh_linked_styles(docx: Vec<u8>) -> Vec<u8> {
    if !templates::links_styles(&docx) {
        return docx;
    }
    let Some(template) =
        templates::attached_template(&docx).and_then(|path| std::fs::read(path).ok())
    else {
        return docx;
    };
    templates::update_styles(&docx, &template).unwrap_or(docx)
}

/// The テンプレートとアドイン dialog as it is open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TemplatesDialog {
    pub attached: Option<String>,
    pub link_styles: bool,
}

impl DocumentView {
    pub(crate) fn open_templates_dialog(&mut self, cx: &mut Context<Self>) {
        let docx = self.document.export_docx().unwrap_or_default();
        self.files.dialog = Some(FileDialog::Templates(TemplatesDialog {
            attached: templates::attached_template(&docx),
            link_styles: templates::links_styles(&docx),
        }));
        cx.notify();
    }

    /// The document changed by `change`, laid out again.
    fn change_package(
        &mut self,
        change: impl FnOnce(&[u8]) -> Result<Vec<u8>, String>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let docx = self.document.export_docx()?;
        let changed = change(&docx)?;
        let document = crate::open_document_bytes(&changed)?;
        self.replace_document(document);
        self.dirty = true;
        cx.notify();
        Ok(())
    }

    /// 添付…: asks for a template and attaches the document to it.
    fn attach_template(&mut self, link_styles: bool, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(SharedString::from(t("templates.attach"))),
        });
        cx.spawn(async move |view, cx| {
            let Ok(Ok(Some(paths))) = prompt.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            view.update(cx, |view, cx| {
                view.attach_template_at(&path, link_styles, cx)
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn attach_template_at(
        &mut self,
        path: &Path,
        link_styles: bool,
        cx: &mut Context<Self>,
    ) {
        let template = std::fs::read(path).ok();
        let name = path.to_string_lossy().into_owned();
        let result = self.change_package(
            |docx| templates::attach_template(docx, &name, template.as_deref(), link_styles),
            cx,
        );
        match result {
            Ok(()) => self.open_templates_dialog(cx),
            Err(error) => self.show_message(t("templates.title"), &error, cx),
        }
    }

    pub(crate) fn detach_template(&mut self, cx: &mut Context<Self>) {
        match self.change_package(templates::detach_template, cx) {
            Ok(()) => self.open_templates_dialog(cx),
            Err(error) => self.show_message(t("templates.title"), &error, cx),
        }
    }

    /// Normal テンプレートとして保存: this document's styles and page are
    /// what new documents start from.
    pub(crate) fn save_as_normal_template(&mut self, cx: &mut Context<Self>) {
        let written = self
            .document
            .export_docx()
            .and_then(|docx| templates::normal_template_from(&docx))
            .and_then(|normal| {
                std::fs::write(normal_template_path(), normal).map_err(|error| error.to_string())
            });
        match written {
            Ok(()) => self.show_message(t("templates.title"), t("templates.normal_saved"), cx),
            Err(error) => self.show_message(t("templates.title"), &error, cx),
        }
    }
}

/// The dialog's title, body and buttons.
pub fn dialog(
    state: &TemplatesDialog,
    cx: &mut Context<DocumentView>,
) -> (SharedString, AnyElement, AnyElement) {
    let attached = state
        .attached
        .clone()
        .unwrap_or_else(|| t("templates.normal").to_string());
    let link_styles = state.link_styles;
    let has_normal = normal_template_path().is_file();
    let body = div()
        .flex()
        .flex_col()
        .gap_2()
        .w(px(460.0))
        .child(caption(t("templates.document_template")))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .border_1()
                        .border_color(theme::border())
                        .bg(theme::canvas())
                        .truncate()
                        .child(attached),
                )
                .child(
                    button("templates-attach", t("templates.attach"), false).on_click(cx.listener(
                        move |view, _: &ClickEvent, _, cx| view.attach_template(link_styles, cx),
                    )),
                )
                .when(state.attached.is_some(), |row| {
                    row.child(
                        button("templates-detach", t("templates.detach"), false).on_click(
                            cx.listener(|view, _: &ClickEvent, _, cx| view.detach_template(cx)),
                        ),
                    )
                }),
        )
        .child(
            check("templates-link", t("templates.link_styles"), link_styles).on_click(cx.listener(
                |view, _: &ClickEvent, _, cx| {
                    let Some(FileDialog::Templates(state)) = view.files.dialog.as_ref() else {
                        return;
                    };
                    let state = state.clone();
                    match state.attached.clone() {
                        // Turned on with a template: its styles now, as Word does.
                        Some(path) => {
                            view.attach_template_at(Path::new(&path), !state.link_styles, cx)
                        }
                        None => {
                            if let Some(FileDialog::Templates(state)) = view.files.dialog.as_mut() {
                                state.link_styles = !state.link_styles;
                            }
                            cx.notify();
                        }
                    }
                },
            )),
        )
        .child(caption(t("templates.normal_heading")))
        .child(div().text_color(theme::text_faint()).child(if has_normal {
            t("templates.normal_custom")
        } else {
            t("templates.normal_default")
        }))
        .child(
            div()
                .flex()
                .gap_2()
                .child(
                    button("templates-save-normal", t("templates.save_normal"), false).on_click(
                        cx.listener(|view, _: &ClickEvent, _, cx| view.save_as_normal_template(cx)),
                    ),
                )
                .when(has_normal, |row| {
                    row.child(
                        button("templates-reset-normal", t("templates.reset_normal"), false)
                            .on_click(cx.listener(|_view, _: &ClickEvent, _, cx| {
                                std::fs::remove_file(normal_template_path()).ok();
                                cx.notify();
                            })),
                    )
                }),
        )
        .into_any_element();
    let buttons = div()
        .flex()
        .justify_end()
        .child(
            button("templates-ok", t("format.ok"), true).on_click(cx.listener(
                |view, _: &ClickEvent, _, cx| {
                    view.files.dialog = None;
                    cx.notify();
                },
            )),
        )
        .into_any_element();
    (t("templates.title").into(), body, buttons)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 添付 with 文書のスタイルを自動的に更新する takes the template's styles
    /// into the document; 切り離す attaches it to Normal again.
    #[gpui::test]
    fn a_template_is_attached_with_its_styles_and_detached(cx: &mut gpui::TestAppContext) {
        use gpui::Focusable;
        use jade_opc::OpcPackage;
        let directory = std::env::temp_dir().join(format!("jade-templates-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        // A template whose 標準 style is red.
        let mut template = OpcPackage::open(&jade_office_model::word::new::blank_document(
            jade_office_model::word::new::Blank::Japanese,
        ))
        .unwrap();
        let styles = template
            .part_str("word/styles.xml")
            .unwrap()
            .unwrap()
            .replacen(
                r#"<w:name w:val="Normal"/><w:qFormat/>"#,
                r#"<w:name w:val="Normal"/><w:qFormat/><w:rPr><w:color w:val="C00000"/></w:rPr>"#,
                1,
            );
        template.replace("word/styles.xml", styles.into_bytes());
        let template_path = directory.join("Letter.dotx");
        std::fs::write(&template_path, template.write().unwrap()).unwrap();
        let path = crate::shell::scratch_copy(
            &PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../jade-core/contract.docx"
            )),
            "templated.docx",
        );
        let (view, cx) =
            cx.add_window_view(|_window, cx| crate::open_document_view(&path, cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.dispatch_action(TemplatesAndAddIns);
        view.update(cx, |view, cx| {
            view.attach_template_at(&template_path, true, cx)
        });
        let docx = view.read_with(cx, |view, _| {
            let Some(FileDialog::Templates(state)) = &view.files.dialog else {
                panic!("the dialog stays open");
            };
            assert_eq!(
                state.attached.as_deref(),
                Some(template_path.to_string_lossy().as_ref())
            );
            assert!(state.link_styles);
            assert!(view.dirty);
            view.document.export_docx().unwrap()
        });
        let styles = OpcPackage::open(&docx)
            .unwrap()
            .part_str("word/styles.xml")
            .unwrap()
            .unwrap();
        assert!(styles.contains(r#"<w:color w:val="C00000"/>"#));
        view.update(cx, |view, cx| view.detach_template(cx));
        view.read_with(cx, |view, _| {
            let docx = view.document.export_docx().unwrap();
            assert_eq!(templates::attached_template(&docx), None);
        });
        std::fs::remove_dir_all(directory).ok();
    }
}

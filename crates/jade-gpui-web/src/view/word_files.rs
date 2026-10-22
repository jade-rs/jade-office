//! A Word document as a file, in the browser: the 互換性チェック (what an
//! older Word would lose, on its own and before a save or a download in an
//! older format, with the document's layout compatibility options) and
//! 変換 to the current format; テンプレートとアドイン (the template the
//! document is attached to, attaching one the page's picker brings with
//! its styles, detaching it, and the Normal template new documents start
//! from, kept in the browser); and ファイル サイズの圧縮 (pictures brought
//! down to a resolution, their cropped parts dropped, now or on every
//! save). What each does to the package is `jade-office-model`'s
//! (`word::compat`, `word::templates`, `word::compress`); the native app's
//! `word_compat`, `word_templates` and `word_save_options`.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{div, prelude::*, px, AnyElement, Context, SharedString, Window};
use jade_web::word::compat::{self, Issue, WordVersion};
use jade_web::word::compress::{self, Resolution};
use jade_web::word::formats::{TextEncoding, WordFormat};
use jade_web::word::templates;

use super::word_kit::{button, check, modal, on, section};
use super::word_tools_ui::Pick;
use super::{DocView, Document};
use crate::{i18n, palette as theme};

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

/// What waits for the checker's 続行.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Then {
    /// The save (the download in the file's own format).
    Save,
    /// A download in another format.
    Download(WordFormat, Option<TextEncoding>),
}

/// The checker as it is open.
pub struct Checker {
    pub version: WordVersion,
    pub issues: Vec<(Issue, usize)>,
    pub then: Option<Then>,
    /// The layout options (互換性オプション) and whether each is on.
    pub layout: Vec<(&'static str, bool)>,
}

/// How pictures are compressed, now or on save.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Compression {
    pub resolution: Resolution,
    /// 図のトリミング部分を削除する.
    pub delete_crops: bool,
}

/// The open dialog.
pub enum FileDialog {
    Compatibility(Checker),
    Templates {
        attached: Option<String>,
        link_styles: bool,
    },
    Compress {
        compression: Compression,
        on_save: bool,
    },
}

/// What the file tools' buttons do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilesAction {
    CheckCompatibility,
    Convert,
}

/// 校閲's 互換性 group: 互換性チェック and 文書の変換 (Word for Mac keeps
/// them in the ファイル menu, which the browser does not have).
pub fn review_group(compatibility_mode: bool, cx: &mut Context<DocView>) -> AnyElement {
    crate::ribbon::group(
        t("files.group.compatibility"),
        vec![crate::ribbon::row(vec![
            crate::word_page::text_button(
                "files-check-compatibility",
                t("menu.file.check_compatibility"),
                t("compat.title"),
                false,
                crate::ribbon::Action::Files(FilesAction::CheckCompatibility),
                cx,
            ),
            crate::word_page::text_button(
                "files-convert",
                t("menu.file.convert"),
                t("compat.convert_title"),
                compatibility_mode,
                crate::ribbon::Action::Files(FilesAction::Convert),
                cx,
            ),
        ])],
    )
}

/// The file tools' state, kept with the document view.
pub struct FilesUi {
    pub dialog: Option<FileDialog>,
    /// 文書を保存するときに互換性を確認する.
    pub check_on_save: bool,
    /// Every save compresses the pictures this way.
    pub compress_on_save: Option<Compression>,
}

impl Default for FilesUi {
    fn default() -> Self {
        FilesUi {
            dialog: None,
            check_on_save: true,
            compress_on_save: None,
        }
    }
}

thread_local! {
    /// The Normal template new documents start from, as the browser keeps
    /// it (`word_recover::store`), read once the page is up.
    pub(crate) static NORMAL: RefCell<Option<Rc<Vec<u8>>>> = const { RefCell::new(None) };
}

/// The Normal template the user saved, if any.
pub(crate) fn normal_template() -> Option<Rc<Vec<u8>>> {
    NORMAL.with(|normal| normal.borrow().clone())
}

fn layout_label(name: &str) -> &'static str {
    match name {
        "useFELayout" => t("compat.layout.fe"),
        "balanceSingleByteDoubleByteWidth" => t("compat.layout.balance"),
        "noTabHangInd" => t("compat.layout.no_tab_hang"),
        "adjustLineHeightInTable" => t("compat.layout.table_grid"),
        _ => t("compat.layout_options"),
    }
}

impl DocView {
    /// The document as it would be saved now.
    fn current_docx(&self) -> Result<Vec<u8>, String> {
        self.word_ref()
            .ok_or_else(|| "no document".to_string())?
            .word_docx()
    }

    /// The document laid out again from `docx`, a change to its package.
    fn reload_docx(&mut self, docx: &[u8], cx: &mut Context<Self>) -> Result<(), String> {
        if self.doc().story().is_some() {
            self.doc_mut().close_story()?;
        }
        if let Document::Pages { object, .. } = &mut self.document {
            *object = None;
        }
        self.doc_mut().reload(docx)?;
        if let Document::Pages {
            ribbon, document, ..
        } = &mut self.document
        {
            ribbon.read_styles(document);
        }
        self.refresh_repainted();
        self.word_result(Ok(()), cx);
        Ok(())
    }

    fn compatibility_issues(&self, version: WordVersion) -> Vec<(Issue, usize)> {
        self.current_docx()
            .map(|docx| compat::check(&docx, version))
            .unwrap_or_default()
    }

    /// 互換性チェック, for the document's own version when it is in
    /// 互換モード (Word 97–2003 otherwise).
    pub(crate) fn open_compatibility_checker(&mut self, cx: &mut Context<Self>) {
        let version = if compat::is_compatibility_mode(self.compat_mode) {
            WordVersion::of_mode(self.compat_mode)
        } else {
            WordVersion::Word2003
        };
        let issues = self.compatibility_issues(version);
        let layout = self
            .current_docx()
            .map(|docx| compat::layout_options(&docx))
            .unwrap_or_default();
        self.files.dialog = Some(FileDialog::Compatibility(Checker {
            version,
            issues,
            then: None,
            layout,
        }));
        cx.notify();
    }

    /// Before a save or a download in an older format: the checker, when
    /// there is something that version would lose. Whether it was shown.
    pub(crate) fn check_before_saving(&mut self, then: Then, cx: &mut Context<Self>) -> bool {
        if !self.files.check_on_save || !matches!(self.document, Document::Pages { .. }) {
            return false;
        }
        let format = match then {
            Then::Save => WordFormat::of_name(&self.name),
            Then::Download(format, _) => Some(format),
        };
        let legacy = format.is_some_and(WordFormat::is_legacy);
        let version = if legacy {
            WordVersion::Word2003
        } else if compat::is_compatibility_mode(self.compat_mode)
            && format.is_none_or(WordFormat::is_package)
        {
            WordVersion::of_mode(self.compat_mode)
        } else {
            return false;
        };
        let issues = self.compatibility_issues(version);
        if issues.is_empty() {
            return false;
        }
        self.files.dialog = Some(FileDialog::Compatibility(Checker {
            version,
            issues,
            then: Some(then),
            layout: Vec::new(),
        }));
        cx.notify();
        true
    }

    /// Turns a layout option on or off, and lays the document out again.
    fn toggle_layout_option(&mut self, name: &'static str, on: bool, cx: &mut Context<Self>) {
        let changed = self
            .current_docx()
            .and_then(|docx| compat::set_layout_option(&docx, name, on))
            .and_then(|docx| self.reload_docx(&docx, cx));
        match changed {
            Ok(()) => {
                if let Some(FileDialog::Compatibility(checker)) = self.files.dialog.as_mut() {
                    for option in checker.layout.iter_mut() {
                        if option.0 == name {
                            option.1 = on;
                        }
                    }
                }
            }
            Err(error) => self.set_status(error.into()),
        }
        cx.notify();
    }

    /// 変換: the document in the current format, laid out again as one. A
    /// Word 97–2004 document becomes a `.docx` of the same name.
    pub(crate) fn convert_document(&mut self, cx: &mut Context<Self>) {
        if !compat::is_compatibility_mode(self.compat_mode) {
            self.set_status(t("compat.already_current").into());
            return cx.notify();
        }
        let converted = self
            .current_docx()
            .and_then(|docx| compat::upgrade(&docx))
            .and_then(|docx| self.reload_docx(&docx, cx));
        match converted {
            Ok(()) => {
                self.compat_mode = compat::CURRENT_MODE;
                if WordFormat::of_name(&self.name).is_some_and(|format| !format.is_package()) {
                    self.name =
                        jade_web::word::formats::renamed(&self.name, WordFormat::Docx).into();
                    self.text_encoding = None;
                }
            }
            Err(error) => self.set_status(error.into()),
        }
        cx.notify();
    }

    // ---- テンプレートとアドイン -----------------------------------------------

    pub(crate) fn open_templates_dialog(&mut self, cx: &mut Context<Self>) {
        let docx = self.current_docx().unwrap_or_default();
        self.files.dialog = Some(FileDialog::Templates {
            attached: templates::attached_template(&docx),
            link_styles: templates::links_styles(&docx),
        });
        cx.notify();
    }

    /// 添付… with the template the page's picker brought: attached, its
    /// styles taken in when 文書のスタイルを自動的に更新する is on.
    pub(crate) fn attach_template_file(
        &mut self,
        name: &str,
        bytes: &[u8],
        cx: &mut Context<Self>,
    ) {
        let link_styles = match &self.files.dialog {
            Some(FileDialog::Templates { link_styles, .. }) => *link_styles,
            _ => true,
        };
        let result = self
            .current_docx()
            .and_then(|docx| templates::attach_template(&docx, name, Some(bytes), link_styles))
            .and_then(|docx| self.reload_docx(&docx, cx));
        match result {
            Ok(()) => self.open_templates_dialog(cx),
            Err(error) => {
                self.set_status(error.into());
                cx.notify();
            }
        }
    }

    fn detach_template(&mut self, cx: &mut Context<Self>) {
        let result = self
            .current_docx()
            .and_then(|docx| templates::detach_template(&docx))
            .and_then(|docx| self.reload_docx(&docx, cx));
        match result {
            Ok(()) => self.open_templates_dialog(cx),
            Err(error) => {
                self.set_status(error.into());
                cx.notify();
            }
        }
    }

    /// この文書のスタイルとページを既定に設定: the Normal template new
    /// documents start from, kept in the browser.
    fn save_as_normal_template(&mut self, cx: &mut Context<Self>) {
        match self
            .current_docx()
            .and_then(|docx| templates::normal_template_from(&docx))
        {
            Ok(normal) => {
                let normal = Rc::new(normal);
                NORMAL.with(|cell| *cell.borrow_mut() = Some(Rc::clone(&normal)));
                super::word_recover::store::put_template("normal", &normal);
                self.set_status(t("templates.normal_saved").into());
            }
            Err(error) => self.set_status(error.into()),
        }
        cx.notify();
    }

    fn reset_normal_template(&mut self, cx: &mut Context<Self>) {
        NORMAL.with(|cell| *cell.borrow_mut() = None);
        super::word_recover::store::remove("templates", "normal");
        cx.notify();
    }

    // ---- ファイル サイズの圧縮 ---------------------------------------------------

    pub(crate) fn open_compress_dialog(&mut self, cx: &mut Context<Self>) {
        let compression = self.files.compress_on_save.unwrap_or(Compression {
            resolution: Resolution::Print,
            delete_crops: true,
        });
        self.files.dialog = Some(FileDialog::Compress {
            compression,
            on_save: self.files.compress_on_save.is_some(),
        });
        cx.notify();
    }

    /// Compresses the pictures now, and remembers whether saves do too.
    fn compress_now(&mut self, compression: Compression, on_save: bool, cx: &mut Context<Self>) {
        self.files.compress_on_save = on_save.then_some(compression);
        let result = self.current_docx().and_then(|docx| {
            compress::compress_pictures(&docx, compression.resolution, compression.delete_crops)
        });
        let status: SharedString = match result {
            Ok((docx, report)) if report.resampled + report.uncropped > 0 => {
                match self.reload_docx(&docx, cx) {
                    Ok(()) => {
                        let saved = report.bytes_before.saturating_sub(report.bytes_after);
                        i18n::format(
                            "compress.done",
                            &[
                                ("n", &(report.resampled + report.uncropped)),
                                ("kb", &(saved / 1024)),
                            ],
                        )
                        .into()
                    }
                    Err(error) => error.into(),
                }
            }
            Ok(_) => t("compress.nothing").into(),
            Err(error) => error.into(),
        };
        self.set_status(status);
        cx.notify();
    }

    // ---- the dialogs ------------------------------------------------------------

    /// Runs a file tool's button.
    pub(crate) fn files_action(&mut self, action: FilesAction, cx: &mut Context<Self>) {
        match action {
            FilesAction::CheckCompatibility => self.open_compatibility_checker(cx),
            FilesAction::Convert => self.convert_document(cx),
        }
    }

    /// OK (or 続行): what the open dialog does.
    fn confirm_file_dialog(&mut self, cx: &mut Context<Self>) {
        match self.files.dialog.take() {
            Some(FileDialog::Compatibility(checker)) => match checker.then {
                Some(Then::Save) => {
                    let check = std::mem::replace(&mut self.files.check_on_save, false);
                    self.save(cx);
                    self.files.check_on_save = check;
                }
                Some(Then::Download(format, encoding)) => {
                    let check = std::mem::replace(&mut self.files.check_on_save, false);
                    self.download_as(format, encoding, cx);
                    self.files.check_on_save = check;
                }
                None => {}
            },
            Some(FileDialog::Compress {
                compression,
                on_save,
            }) => self.compress_now(compression, on_save, cx),
            Some(FileDialog::Templates { .. }) | None => {}
        }
        cx.notify();
    }

    /// A key while a file dialog is open; `true` when it was taken.
    pub(crate) fn files_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        if self.files.dialog.is_none() {
            return false;
        }
        match key {
            "escape" => {
                self.files.dialog = None;
                cx.notify();
            }
            "enter" => self.confirm_file_dialog(cx),
            _ => {}
        }
        true
    }

    /// The open file dialog, over the window.
    pub(crate) fn files_dialog(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (title, body, buttons, width) = match self.files.dialog.as_ref()? {
            FileDialog::Compatibility(checker) => {
                let (body, buttons) = checker_body(checker, self.files.check_on_save, cx);
                (t("compat.title"), body, buttons, 520.0)
            }
            FileDialog::Templates {
                attached,
                link_styles,
            } => {
                let (body, buttons) = templates_body(attached.clone(), *link_styles, cx);
                (t("templates.title"), body, buttons, 480.0)
            }
            FileDialog::Compress {
                compression,
                on_save,
            } => {
                let (body, buttons) = compress_body(*compression, *on_save, cx);
                (t("compress.title"), body, buttons, 400.0)
            }
        };
        Some(modal(
            "file-dialog",
            width,
            title,
            body,
            None,
            buttons,
            window,
        ))
    }
}

fn close(cx: &Context<DocView>) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) {
    on(cx, |view: &mut DocView, cx| {
        view.files.dialog = None;
        cx.notify();
    })
}

fn checker_body(
    checker: &Checker,
    check_on_save: bool,
    cx: &mut Context<DocView>,
) -> (AnyElement, AnyElement) {
    let japanese = !i18n::is_english();
    let versions = WordVersion::ALL.into_iter().map(|version| {
        check(
            SharedString::from(format!("compat-version-{}", version.mode())),
            version.label(japanese),
            checker.version == version,
        )
        .on_click(on(cx, move |view, cx| {
            let issues = view.compatibility_issues(version);
            if let Some(FileDialog::Compatibility(checker)) = view.files.dialog.as_mut() {
                checker.version = version;
                checker.issues = issues;
            }
            cx.notify();
        }))
    });
    let issues: Vec<AnyElement> = if checker.issues.is_empty() {
        vec![div()
            .text_color(theme::text_dim())
            .child(t("compat.no_issues"))
            .into_any_element()]
    } else {
        checker
            .issues
            .iter()
            .map(|(issue, count)| {
                div()
                    .flex()
                    .justify_between()
                    .gap_4()
                    .py_1()
                    .border_b_1()
                    .border_color(theme::border())
                    .child(div().flex_1().child(t(issue.key())))
                    .child(
                        div()
                            .text_color(theme::text_dim())
                            .child(i18n::format("compat.occurrences", &[("n", count)])),
                    )
                    .into_any_element()
            })
            .collect()
    };
    let layout: Vec<AnyElement> = checker
        .layout
        .iter()
        .map(|(name, on_now)| {
            let (name, on_now) = (*name, *on_now);
            check(
                SharedString::from(format!("compat-layout-{name}")),
                layout_label(name),
                on_now,
            )
            .on_click(on(cx, move |view, cx| {
                view.toggle_layout_option(name, !on_now, cx)
            }))
            .into_any_element()
        })
        .collect();
    let body = div()
        .flex()
        .flex_col()
        .gap_2()
        .child(section(t("compat.versions")))
        .child(div().flex().gap_3().children(versions))
        .child(section(t("compat.summary")))
        .child(
            div()
                .id("compat-issues")
                .max_h(px(240.0))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .children(issues),
        )
        .when(!layout.is_empty(), |body| {
            body.child(section(t("compat.layout_options")))
                .children(layout)
        })
        .child(
            check("compat-on-save", t("compat.check_on_save"), check_on_save).on_click(on(
                cx,
                |view, cx| {
                    view.files.check_on_save = !view.files.check_on_save;
                    cx.notify();
                },
            )),
        )
        .into_any_element();
    let saving = checker.then.is_some();
    let buttons = div()
        .flex()
        .justify_end()
        .gap_2()
        .when(saving, |row| {
            row.child(button("compat-cancel", t("format.cancel"), false).on_click(close(cx)))
        })
        .child(
            button(
                "compat-ok",
                if saving {
                    t("compat.continue")
                } else {
                    t("format.ok")
                },
                true,
            )
            .on_click(on(cx, |view, cx| view.confirm_file_dialog(cx))),
        )
        .into_any_element();
    (body, buttons)
}

fn templates_body(
    attached: Option<String>,
    link_styles: bool,
    cx: &mut Context<DocView>,
) -> (AnyElement, AnyElement) {
    let has_attached = attached.is_some();
    let has_normal = normal_template().is_some();
    let body = div()
        .flex()
        .flex_col()
        .gap_2()
        .child(section(t("templates.document_template")))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .border_1()
                        .border_color(theme::border())
                        .bg(theme::canvas())
                        .truncate()
                        .child(attached.unwrap_or_else(|| t("templates.normal").to_string())),
                )
                .child(
                    button("templates-attach", t("templates.attach"), false).on_click(on(
                        cx,
                        |view, _cx| {
                            view.tools.picking = Some(Pick::Template);
                            view.host.pick();
                        },
                    )),
                )
                .when(has_attached, |row| {
                    row.child(
                        button("templates-detach", t("templates.detach"), false)
                            .on_click(on(cx, |view, cx| view.detach_template(cx))),
                    )
                }),
        )
        .child(
            check("templates-link", t("templates.link_styles"), link_styles).on_click(on(
                cx,
                |view, cx| {
                    if let Some(FileDialog::Templates { link_styles, .. }) =
                        view.files.dialog.as_mut()
                    {
                        *link_styles = !*link_styles;
                    }
                    cx.notify();
                },
            )),
        )
        .child(section(t("templates.normal_heading")))
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
                    button("templates-save-normal", t("templates.save_normal"), false)
                        .on_click(on(cx, |view, cx| view.save_as_normal_template(cx))),
                )
                .when(has_normal, |row| {
                    row.child(
                        button("templates-reset-normal", t("templates.reset_normal"), false)
                            .on_click(on(cx, |view, cx| view.reset_normal_template(cx))),
                    )
                }),
        )
        .into_any_element();
    let buttons = div()
        .flex()
        .justify_end()
        .child(button("templates-ok", t("format.ok"), true).on_click(close(cx)))
        .into_any_element();
    (body, buttons)
}

fn compress_body(
    compression: Compression,
    on_save: bool,
    cx: &mut Context<DocView>,
) -> (AnyElement, AnyElement) {
    fn edit(
        view: &mut DocView,
        cx: &mut Context<DocView>,
        change: impl FnOnce(&mut Compression, &mut bool),
    ) {
        if let Some(FileDialog::Compress {
            compression,
            on_save,
        }) = view.files.dialog.as_mut()
        {
            change(compression, on_save);
        }
        cx.notify();
    }
    let body = div()
        .flex()
        .flex_col()
        .gap_2()
        .child(section(t("compress.quality")))
        .children(Resolution::ALL.into_iter().map(|resolution| {
            check(
                SharedString::from(format!("compress-{}", resolution.ppi())),
                t(resolution.key()),
                compression.resolution == resolution,
            )
            .on_click(on(cx, move |view, cx| {
                edit(view, cx, |compression, _| {
                    compression.resolution = resolution
                })
            }))
        }))
        .child(
            check(
                "compress-crops",
                t("compress.delete_crops"),
                compression.delete_crops,
            )
            .on_click(on(cx, |view, cx| {
                edit(view, cx, |compression, _| {
                    compression.delete_crops = !compression.delete_crops
                })
            })),
        )
        .child(
            check("compress-on-save", t("compress.on_save"), on_save)
                .on_click(on(cx, |view, cx| {
                    edit(view, cx, |_, on_save| *on_save = !*on_save)
                })),
        )
        .child(
            div()
                .text_color(theme::text_faint())
                .child(t("compress.note")),
        )
        .into_any_element();
    let buttons = div()
        .flex()
        .justify_end()
        .gap_2()
        .child(button("compress-cancel", t("format.cancel"), false).on_click(close(cx)))
        .child(
            button("compress-ok", t("format.ok"), true)
                .on_click(on(cx, |view, cx| view.confirm_file_dialog(cx))),
        )
        .into_any_element();
    (body, buttons)
}

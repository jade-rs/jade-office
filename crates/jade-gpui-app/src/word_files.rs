//! What a Word document is as a file: the format it opened from and saves
//! to (`jade-office-model`'s `word::formats`), 名前を付けて保存 by the
//! extension chosen, コピーを保存, テンプレートとして保存, and the
//! ファイルの変換 dialog a text file's encoding is chosen in — when one
//! opens, to read it again in another, and when one is saved.
//!
//! Saving a document over a file it came from writes it back in that
//! file's format, as Word does; a Word 97–2004 document cannot be written,
//! so ⌘S on one asks where to save a `.docx` instead.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use gpui::{
    actions, anchored, deferred, div, point, prelude::*, px, AnyElement, ClickEvent, Context,
    MouseButton, SharedString, Window,
};
use jade_office_model::word::formats::{
    open_as_docx, renamed, save_from_docx, text_preview, unencodable_characters, FormatError,
    LineEnding, OpenOptions, Opened, SaveOptions, TextEncoding, WordFormat,
};

use crate::word_characters::{button, caption, check};
use crate::{i18n, theme, DocumentView};

actions!(
    jade,
    [
        /// コピーを保存: writes the document somewhere else, in any format,
        /// and goes on editing the file it is.
        SaveCopy,
        /// テンプレートとして保存: a `.dotx` (or `.dotm`).
        SaveAsTemplate,
        /// 環境設定 → 保存: 自動回復, backups, where files are saved, and
        /// this document's font embedding.
        SavePreferences,
    ]
);

/// What the document is as a file.
pub struct Files {
    /// The format it was opened from, and is saved back to.
    pub format: WordFormat,
    /// A text file's encoding and line ending, as read and as saved.
    pub encoding: Option<TextEncoding>,
    pub line_ending: LineEnding,
    /// A text file's bytes, to read again in another encoding.
    pub source: Option<Rc<Vec<u8>>>,
    /// The dialog open over the pages.
    pub dialog: Option<FileDialog>,
    /// TrueType フォントを埋め込む, and 使用されている文字だけ: this
    /// document's, as its settings say.
    pub embed_fonts: bool,
    pub embed_subset: bool,
}

impl Default for Files {
    fn default() -> Self {
        Self {
            format: WordFormat::Docx,
            encoding: None,
            line_ending: LineEnding::CrLf,
            source: None,
            dialog: None,
            embed_fonts: false,
            embed_subset: true,
        }
    }
}

/// Where a write goes and what it does to the document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Write {
    /// 保存 / 名前を付けて保存: the document is that file from now on.
    Save(PathBuf),
    /// コピーを保存: the document stays the file it was.
    Copy(PathBuf),
}

impl Write {
    fn path(&self) -> &Path {
        match self {
            Write::Save(path) | Write::Copy(path) => path,
        }
    }
}

/// What the ファイルの変換 dialog is for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EncodingPurpose {
    /// Reading the open text file again.
    Open,
    /// Writing a text file.
    Save(Write),
}

pub enum FileDialog {
    /// ファイルの変換: an encoding, and for a save the line ending.
    Encoding {
        purpose: EncodingPurpose,
        selected: TextEncoding,
        line_ending: LineEnding,
    },
    /// A message with an OK button.
    Message {
        title: SharedString,
        body: SharedString,
    },
    /// 環境設定 → 保存, being changed.
    Preferences(Preferences),
}

/// The 保存 preferences as the dialog holds them until OK.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub settings: crate::autorecover::Settings,
    pub embed_fonts: bool,
    pub embed_subset: bool,
}

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

fn japanese() -> bool {
    i18n::language() == i18n::Language::Japanese
}

/// Reads `path` for the editor: its bytes, converted when it is not a
/// Word package.
pub fn read(path: &Path) -> Result<(Opened, Option<Rc<Vec<u8>>>), String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let name = crate::shell::file_name(path);
    let directory = path.parent().map(Path::to_path_buf);
    let resolve = move |relative: &str| -> Option<Vec<u8>> {
        let relative = relative.split(['?', '#']).next()?;
        let relative = crate::percent_decode(relative.trim_start_matches("file://"));
        let candidate = Path::new(&relative);
        let full = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            directory.as_ref()?.join(candidate)
        };
        std::fs::read(full).ok()
    };
    let opened = open_as_docx(&name, &bytes, OpenOptions::default(), &resolve)
        .map_err(|error| describe(&error, &name))?;
    let source = (opened.format == WordFormat::Text).then(|| Rc::new(bytes));
    Ok((opened, source))
}

/// What went wrong, in the user's language.
pub fn describe(error: &FormatError, name: &str) -> String {
    match error {
        FormatError::Unsupported => i18n::format("workspace.unsupported", &[("name", &name)]),
        FormatError::Encrypted => i18n::format("workspace.protected", &[("name", &name)]),
        FormatError::TooOld => i18n::format("files.too_old", &[("name", &name)]),
        FormatError::Damaged(what) => {
            i18n::format("files.damaged", &[("name", &name), ("error", what)])
        }
        FormatError::CannotWriteDoc => i18n::text("files.cannot_write_doc").to_string(),
        FormatError::Failed(why) => {
            i18n::format("files.failed", &[("name", &name), ("error", why)])
        }
    }
}

/// Whether `bytes` have characters past ASCII, so the encoding they are in
/// matters: Word asks only then.
fn needs_encoding_choice(bytes: &[u8]) -> bool {
    bytes.iter().any(|byte| *byte >= 0x80)
}

impl DocumentView {
    /// Remembers what the opened file was, and for a text file whose
    /// encoding matters, asks which one it is in.
    pub(crate) fn opened_as(&mut self, opened: &Opened, source: Option<Rc<Vec<u8>>>) {
        self.files.format = opened.format;
        self.files.encoding = opened.encoding;
        if let Some(source) = source {
            if needs_encoding_choice(&source) {
                self.files.dialog = Some(FileDialog::Encoding {
                    purpose: EncodingPurpose::Open,
                    selected: opened.encoding.unwrap_or(TextEncoding::Utf8),
                    line_ending: LineEnding::CrLf,
                });
            }
            self.files.source = Some(source);
        }
    }

    /// ⌘S: back over the file, in its own format; a new document, or one
    /// whose format cannot be written, asks where first.
    pub(crate) fn save_file(&mut self, cx: &mut Context<Self>) {
        if self.untitled {
            return self.save_file_as(cx);
        }
        if !self.files.format.can_save() {
            self.show_message(t("files.doc_title"), t("files.cannot_write_doc"), cx);
            return self.save_file_as(cx);
        }
        let target = PathBuf::from(self.path.to_string());
        self.write_file(Write::Save(target), None, cx);
    }

    /// 名前を付けて保存: the format follows the extension typed.
    pub(crate) fn save_file_as(&mut self, cx: &mut Context<Self>) {
        let name = self.suggested_name(None);
        self.prompt_and_write(name, false, cx);
    }

    /// コピーを保存.
    pub(crate) fn save_copy(&mut self, cx: &mut Context<Self>) {
        let source = crate::shell::file_name(Path::new(self.path.as_ref()));
        let stem = source
            .rsplit_once('.')
            .map_or(source.as_str(), |(stem, _)| stem);
        let format = if self.files.format.can_save() {
            self.files.format
        } else {
            WordFormat::Docx
        };
        let name = format!(
            "{}.{}",
            i18n::format("files.copy_name", &[("name", &stem)]),
            format.extension()
        );
        self.prompt_and_write(name, true, cx);
    }

    /// テンプレートとして保存: a `.dotx`, or a `.dotm` for a document with
    /// macros, in the templates folder.
    pub(crate) fn save_as_template(&mut self, cx: &mut Context<Self>) {
        let format = if matches!(self.files.format, WordFormat::Docm | WordFormat::Dotm) {
            WordFormat::Dotm
        } else {
            WordFormat::Dotx
        };
        let name = self.suggested_name(Some(format));
        let directory = crate::word_templates::templates_folder();
        let prompt = cx.prompt_for_new_path(&directory, Some(&name));
        cx.spawn(async move |view, cx| {
            let Ok(Ok(Some(path))) = prompt.await else {
                return;
            };
            let path = with_extension(path, format);
            view.update(cx, |view, cx| view.write_file(Write::Save(path), None, cx))
                .ok();
        })
        .detach();
    }

    /// The name a save prompt suggests: the document's, in `format` — or
    /// in its own format when that can be written, `.docx` when not.
    fn suggested_name(&self, format: Option<WordFormat>) -> String {
        let source = crate::shell::file_name(Path::new(self.path.as_ref()));
        let format = format.unwrap_or(if self.files.format.can_save() {
            self.files.format
        } else {
            WordFormat::Docx
        });
        renamed(&source, format)
    }

    fn prompt_and_write(&mut self, name: String, copy: bool, cx: &mut Context<Self>) {
        let directory = self.save_directory();
        let prompt = cx.prompt_for_new_path(&directory, Some(&name));
        cx.spawn(async move |view, cx| {
            let Ok(Ok(Some(path))) = prompt.await else {
                return;
            };
            let write = if copy {
                Write::Copy(path)
            } else {
                Write::Save(path)
            };
            view.update(cx, |view, cx| view.write_file(write, None, cx))
                .ok();
        })
        .detach();
    }

    /// Writes the document to `write`'s path in the format its extension
    /// names. A text file asks its encoding first, unless `options` says.
    pub(crate) fn write_file(
        &mut self,
        write: Write,
        options: Option<SaveOptions>,
        cx: &mut Context<Self>,
    ) {
        // What is open in a header or footer goes in first.
        if let Err(error) = self.close_story_editing() {
            self.table_status = Some(SharedString::from(error));
        }
        let target = write.path().to_path_buf();
        let name = crate::shell::file_name(&target);
        let format = WordFormat::of_name(&name).unwrap_or(WordFormat::Docx);
        if format == WordFormat::Pdf {
            return self.write_pdf_to(&target, cx);
        }
        if !format.can_save() {
            return self.show_message(t("files.doc_title"), t("files.cannot_write_doc"), cx);
        }
        if format == WordFormat::Text && options.is_none() {
            self.files.dialog = Some(FileDialog::Encoding {
                purpose: EncodingPurpose::Save(write),
                selected: self.files.encoding.unwrap_or(if japanese() {
                    TextEncoding::ShiftJis
                } else {
                    TextEncoding::Utf8
                }),
                line_ending: self.files.line_ending,
            });
            return cx.notify();
        }
        let options = options.unwrap_or_default();
        let written = self
            .document
            .export_docx()
            .map_err(|error| error.to_string())
            .and_then(|docx| {
                let docx = self.prepare_for_save(docx, format);
                save_from_docx(format, &docx, options).map_err(|error| describe(&error, &name))
            })
            .and_then(|bytes| {
                crate::autorecover::back_up_before_writing(&target);
                crate::shell::write_in_place(&target, &bytes).map_err(|error| error.to_string())
            });
        match written {
            Ok(()) => {
                if let Write::Save(path) = &write {
                    self.dirty = false;
                    self.untitled = false;
                    self.path = SharedString::from(path.to_string_lossy().into_owned());
                    self.files.format = format;
                    if format == WordFormat::Text {
                        self.files.encoding = options.encoding;
                        self.files.line_ending = options.line_ending;
                    }
                    crate::shell::recent::remember(path);
                    crate::autorecover::saved(self);
                }
                self.saved_to = Some(SharedString::from(name));
                cx.notify();
            }
            Err(error) => self.show_message(t("files.save_failed"), &error, cx),
        }
    }

    /// The `.docx` a save writes, given what the save settings ask of it.
    fn prepare_for_save(&self, docx: Vec<u8>, _format: WordFormat) -> Vec<u8> {
        docx
    }

    /// 環境設定 → 保存.
    pub(crate) fn open_save_preferences(&mut self, cx: &mut Context<Self>) {
        self.files.dialog = Some(FileDialog::Preferences(Preferences {
            settings: crate::autorecover::settings(),
            embed_fonts: self.files.embed_fonts,
            embed_subset: self.files.embed_subset,
        }));
        cx.notify();
    }

    /// Asks for a folder for the preference `pick` sets.
    fn choose_folder(&mut self, pick: fn(&mut Preferences, PathBuf), cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(SharedString::from(t("prefs.choose"))),
        });
        cx.spawn(async move |view, cx| {
            let Ok(Ok(Some(paths))) = prompt.await else {
                return;
            };
            let Some(folder) = paths.into_iter().next() else {
                return;
            };
            view.update(cx, |view, cx| {
                if let Some(FileDialog::Preferences(preferences)) = view.files.dialog.as_mut() {
                    pick(preferences, folder);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn show_message(&mut self, title: &str, body: &str, cx: &mut Context<Self>) {
        self.files.dialog = Some(FileDialog::Message {
            title: SharedString::from(title.to_string()),
            body: SharedString::from(body.to_string()),
        });
        cx.notify();
    }

    /// Reads the open text file again in `encoding`, in place of what is
    /// open — as Word's ファイルの変換 does before the document opens.
    fn reread_text(&mut self, encoding: TextEncoding, cx: &mut Context<Self>) {
        let Some(source) = self.files.source.clone() else {
            return;
        };
        if self.files.encoding == Some(encoding) {
            return;
        }
        let name = crate::shell::file_name(Path::new(self.path.as_ref()));
        let opened = open_as_docx(
            &name,
            &source,
            OpenOptions {
                encoding: Some(encoding),
            },
            &|_| None,
        );
        match opened
            .map_err(|error| describe(&error, &name))
            .and_then(|opened| crate::open_document_bytes(&opened.docx))
        {
            Ok(document) => {
                self.replace_document(document);
                self.files.encoding = Some(encoding);
                cx.notify();
            }
            Err(error) => self.show_message(t("files.encoding_title"), &error, cx),
        }
    }

    /// Puts `document` in place of the open one, as a file opened afresh.
    pub(crate) fn replace_document(
        &mut self,
        mut document: jade_docx_native_render::NativeDocument,
    ) {
        crate::word_characters::start_field_clock(&mut document);
        self.caret = document.place_caret(0, 0);
        self.insert_at = (0, 0);
        self.document = crate::word_story::WordDoc::new(document);
        self.ribbon.read_styles(&self.document);
        self.selected_object = None;
        self.selection_anchor = None;
        self.format = None;
        self.rebuild_all_images();
    }
}

/// `path` with `format`'s extension, when the one typed is not it.
fn with_extension(path: PathBuf, format: WordFormat) -> PathBuf {
    let name = crate::shell::file_name(&path);
    if WordFormat::of_name(&name) == Some(format) {
        path
    } else {
        path.with_extension(format.extension())
    }
}

// ---- the dialogs -------------------------------------------------------------

/// The keys an open dialog takes. Whether one was open.
pub fn handle_key(view: &mut DocumentView, key: &str, cx: &mut Context<DocumentView>) -> bool {
    let Some(dialog) = view.files.dialog.as_mut() else {
        return false;
    };
    match (dialog, key) {
        (_, "escape") => cancel(view, cx),
        (_, "enter") => confirm(view, cx),
        (FileDialog::Encoding { selected, .. }, "up" | "down") => {
            let all = TextEncoding::ALL;
            let index = all.iter().position(|e| e == selected).unwrap_or(0);
            let next = if key == "up" {
                index.saturating_sub(1)
            } else {
                (index + 1).min(all.len() - 1)
            };
            *selected = all[next];
            cx.notify();
        }
        _ => {}
    }
    true
}

fn cancel(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    view.files.dialog = None;
    cx.notify();
}

fn confirm(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let Some(dialog) = view.files.dialog.take() else {
        return;
    };
    match dialog {
        FileDialog::Encoding {
            purpose: EncodingPurpose::Open,
            selected,
            ..
        } => view.reread_text(selected, cx),
        FileDialog::Encoding {
            purpose: EncodingPurpose::Save(write),
            selected,
            line_ending,
        } => view.write_file(
            write,
            Some(SaveOptions {
                encoding: Some(selected),
                line_ending,
            }),
            cx,
        ),
        FileDialog::Message { .. } => {}
        FileDialog::Preferences(preferences) => {
            crate::autorecover::set_settings(preferences.settings);
            if view.files.embed_fonts != preferences.embed_fonts
                || view.files.embed_subset != preferences.embed_subset
            {
                view.files.embed_fonts = preferences.embed_fonts;
                view.files.embed_subset = preferences.embed_subset;
                // A setting of the document: saving it is saving the change.
                view.dirty = true;
            }
        }
    }
    cx.notify();
}

/// The open dialog, over the window.
pub fn overlay(
    view: &mut DocumentView,
    window: &mut Window,
    cx: &mut Context<DocumentView>,
) -> Option<AnyElement> {
    let (title, body, buttons) = match view.files.dialog.as_ref()? {
        FileDialog::Encoding {
            purpose,
            selected,
            line_ending,
        } => {
            let name = crate::shell::file_name(Path::new(view.path.as_ref()));
            let title: SharedString =
                i18n::format("files.encoding_title_named", &[("name", &name)]).into();
            let body = encoding_body(view, purpose.clone(), *selected, *line_ending, cx);
            (title, body, ok_cancel(cx))
        }
        FileDialog::Preferences(preferences) => (
            t("prefs.save_title").into(),
            preferences_body(preferences.clone(), cx),
            ok_cancel(cx),
        ),
        FileDialog::Message { title, body } => (
            title.clone(),
            div()
                .max_w(px(420.0))
                .text_sm()
                .child(body.clone())
                .into_any_element(),
            div()
                .flex()
                .justify_end()
                .child(
                    button("files-ok", t("format.ok"), true)
                        .on_click(cx.listener(|view, _: &ClickEvent, _, cx| cancel(view, cx))),
                )
                .into_any_element(),
        ),
    };
    Some(modal("files-dialog", title, body, buttons, window))
}

/// A dialog's frame over a dimmed window.
pub(crate) fn modal(
    id: &'static str,
    title: SharedString,
    body: AnyElement,
    buttons: AnyElement,
    window: &mut Window,
) -> AnyElement {
    let viewport = window.viewport_size();
    deferred(
        anchored().position(point(px(0.0), px(0.0))).child(
            div()
                .id(SharedString::from(format!("{id}-backdrop")))
                .w(viewport.width)
                .h(viewport.height)
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::rgba(0x00000040))
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .id(id)
                        .max_h(viewport.height - px(40.0))
                        .overflow_y_scroll()
                        .p_4()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .rounded_lg()
                        .border_1()
                        .border_color(theme::border())
                        .bg(theme::panel())
                        .shadow_lg()
                        .font_family(crate::UI_FONT_FAMILY)
                        .text_xs()
                        .text_color(theme::text())
                        .child(div().text_sm().child(title))
                        .child(body)
                        .child(buttons),
                ),
        ),
    )
    .with_priority(4)
    .into_any_element()
}

fn ok_cancel(cx: &mut Context<DocumentView>) -> AnyElement {
    div()
        .flex()
        .justify_end()
        .gap_2()
        .child(
            button("files-cancel", t("format.cancel"), false)
                .on_click(cx.listener(|view, _: &ClickEvent, _, cx| cancel(view, cx))),
        )
        .child(
            button("files-ok", t("format.ok"), true)
                .on_click(cx.listener(|view, _: &ClickEvent, _, cx| confirm(view, cx))),
        )
        .into_any_element()
}

fn encoding_body(
    view: &DocumentView,
    purpose: EncodingPurpose,
    selected: TextEncoding,
    line_ending: LineEnding,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    let list = div()
        .id("files-encodings")
        .flex()
        .flex_col()
        .w(px(240.0))
        .h(px(220.0))
        .overflow_y_scroll()
        .rounded_sm()
        .border_1()
        .border_color(theme::border())
        .bg(theme::canvas())
        .children(TextEncoding::ALL.iter().map(|encoding| {
            let encoding = *encoding;
            let label = if japanese() {
                encoding.label_ja()
            } else {
                encoding.label_en()
            };
            div()
                .id(SharedString::from(format!(
                    "files-encoding-{}",
                    encoding.key()
                )))
                .px_2()
                .py(px(3.0))
                .cursor_pointer()
                .when(encoding == selected, |row| {
                    row.bg(theme::selected()).text_color(theme::text())
                })
                .hover(|row| row.bg(theme::hovered()))
                .child(label)
                .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                    if let Some(FileDialog::Encoding { selected, .. }) = view.files.dialog.as_mut()
                    {
                        *selected = encoding;
                    }
                    cx.notify();
                }))
        }));
    let preview: AnyElement = match &purpose {
        EncodingPurpose::Open => {
            let text = view
                .files
                .source
                .as_ref()
                .map(|source| text_preview(source, selected, 12))
                .unwrap_or_default();
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(caption(t("files.preview")))
                .child(
                    div()
                        .w(px(320.0))
                        .h(px(200.0))
                        .p_2()
                        .overflow_hidden()
                        .rounded_sm()
                        .border_1()
                        .border_color(theme::border())
                        .bg(theme::canvas())
                        .whitespace_normal()
                        .child(text),
                )
                .into_any_element()
        }
        EncodingPurpose::Save(_) => {
            let lost = view
                .document
                .export_docx()
                .map(|docx| unencodable_characters(&docx, selected))
                .unwrap_or_default();
            let endings = [
                (LineEnding::CrLf, "files.crlf"),
                (LineEnding::Lf, "files.lf"),
                (LineEnding::Cr, "files.cr"),
            ];
            div()
                .flex()
                .flex_col()
                .gap_2()
                .w(px(320.0))
                .child(caption(t("files.line_ending")))
                .children(endings.into_iter().map(|(ending, key)| {
                    check(
                        SharedString::from(format!("files-ending-{key}")),
                        t(key),
                        ending == line_ending,
                    )
                    .on_click(cx.listener(
                        move |view, _: &ClickEvent, _, cx| {
                            if let Some(FileDialog::Encoding { line_ending, .. }) =
                                view.files.dialog.as_mut()
                            {
                                *line_ending = ending;
                            }
                            cx.notify();
                        },
                    ))
                }))
                .child(caption(t("files.text_only")))
                .when(!lost.is_empty(), |column| {
                    let shown: String = lost.iter().take(40).collect();
                    column.child(
                        div()
                            .text_color(gpui::rgb(0xd13438))
                            .child(i18n::format("files.lost_characters", &[("chars", &shown)])),
                    )
                })
                .into_any_element()
        }
    };
    div()
        .flex()
        .gap_4()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(caption(t("files.encoding")))
                .child(list),
        )
        .child(preview)
        .into_any_element()
}

fn with_preferences(view: &mut DocumentView, change: impl FnOnce(&mut Preferences)) {
    if let Some(FileDialog::Preferences(preferences)) = view.files.dialog.as_mut() {
        change(preferences);
    }
}

fn folder_line(
    id: &'static str,
    label: &'static str,
    folder: Option<&Path>,
    fallback: &'static str,
    pick: fn(&mut Preferences, PathBuf),
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    let shown = folder
        .map(|folder| folder.to_string_lossy().into_owned())
        .unwrap_or_else(|| t(fallback).to_string());
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(caption(t(label)))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .w(px(300.0))
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .border_1()
                        .border_color(theme::border())
                        .bg(theme::canvas())
                        .truncate()
                        .child(shown),
                )
                .child(button(id, t("prefs.change"), false).on_click(
                    cx.listener(move |view, _: &ClickEvent, _, cx| view.choose_folder(pick, cx)),
                )),
        )
        .into_any_element()
}

fn preferences_body(preferences: Preferences, cx: &mut Context<DocumentView>) -> AnyElement {
    let settings = preferences.settings.clone();
    let minutes = settings.minutes;
    div()
        .flex()
        .flex_col()
        .gap_3()
        .w(px(420.0))
        .child(caption(t("prefs.save_options")))
        .child(
            check(
                "prefs-backup",
                t("prefs.always_backup"),
                settings.always_backup,
            )
            .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
                with_preferences(view, |p| {
                    p.settings.always_backup = !p.settings.always_backup
                });
                cx.notify();
            })),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    check(
                        "prefs-autorecover",
                        t("prefs.autorecover"),
                        settings.enabled,
                    )
                    .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
                        with_preferences(view, |p| p.settings.enabled = !p.settings.enabled);
                        cx.notify();
                    })),
                )
                .child(
                    button("prefs-minutes-less", "−", false).on_click(cx.listener(
                        |view, _: &ClickEvent, _, cx| {
                            with_preferences(view, |p| {
                                p.settings.minutes = p.settings.minutes.saturating_sub(1).max(1)
                            });
                            cx.notify();
                        },
                    )),
                )
                .child(div().w(px(28.0)).text_center().child(minutes.to_string()))
                .child(
                    button("prefs-minutes-more", "+", false).on_click(cx.listener(
                        |view, _: &ClickEvent, _, cx| {
                            with_preferences(view, |p| {
                                p.settings.minutes = (p.settings.minutes + 1).min(120)
                            });
                            cx.notify();
                        },
                    )),
                )
                .child(t("prefs.minutes")),
        )
        .child(folder_line(
            "prefs-recovery-folder",
            "prefs.recovery_folder",
            settings.location.as_deref(),
            "prefs.default_folder",
            |p, folder| p.settings.location = Some(folder),
            cx,
        ))
        .child(folder_line(
            "prefs-save-folder",
            "prefs.save_folder",
            settings.save_folder.as_deref(),
            "prefs.documents",
            |p, folder| p.settings.save_folder = Some(folder),
            cx,
        ))
        .child(caption(t("prefs.this_document")))
        .child(
            check(
                "prefs-embed",
                t("prefs.embed_fonts"),
                preferences.embed_fonts,
            )
            .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
                with_preferences(view, |p| p.embed_fonts = !p.embed_fonts);
                cx.notify();
            })),
        )
        .child(
            div().pl_4().child(
                check(
                    "prefs-embed-subset",
                    t("prefs.embed_subset"),
                    preferences.embed_subset,
                )
                .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
                    with_preferences(view, |p| p.embed_subset = !p.embed_subset);
                    cx.notify();
                })),
            ),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_typed_name_takes_the_format_extension_only_when_it_lacks_it() {
        assert_eq!(
            with_extension(PathBuf::from("/a/b.dotx"), WordFormat::Dotx),
            PathBuf::from("/a/b.dotx")
        );
        assert_eq!(
            with_extension(PathBuf::from("/a/b"), WordFormat::Dotx),
            PathBuf::from("/a/b.dotx")
        );
    }

    #[test]
    fn only_text_past_ascii_needs_its_encoding_chosen() {
        assert!(!needs_encoding_choice(b"plain"));
        assert!(needs_encoding_choice("日本".as_bytes()));
    }

    fn core_fixture(path: &str) -> PathBuf {
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core/")).join(path)
    }

    fn open<'a>(
        path: &Path,
        cx: &'a mut gpui::TestAppContext,
    ) -> (gpui::Entity<DocumentView>, &'a mut gpui::VisualTestContext) {
        use gpui::Focusable;
        let path = path.to_path_buf();
        let (view, cx) =
            cx.add_window_view(|_window, cx| crate::open_document_view(&path, cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        (view, cx)
    }

    fn text_of(view: &DocumentView) -> String {
        (0..view.document.paragraph_count())
            .filter_map(|index| view.document.paragraph_text(index))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Each of Word's other formats opens converted, and ⌘S writes it back
    /// in its own format — except a Word 97–2004 document, which says why
    /// it cannot be and leaves the file as it was.
    #[gpui::test]
    fn every_format_opens_and_saves_back_as_itself(cx: &mut gpui::TestAppContext) {
        for (fixture, format) in [
            (
                "crates/jade-docx-convert/tests/fixtures/contract.rtf",
                WordFormat::Rtf,
            ),
            (
                "crates/jade-docx-convert/tests/fixtures/contract.odt",
                WordFormat::Odt,
            ),
            (
                "crates/jade-docx-convert/tests/fixtures/contract.html",
                WordFormat::Html,
            ),
            (
                "crates/jade-doc-reader/tests/fixtures/contract.doc",
                WordFormat::Doc,
            ),
        ] {
            let source = core_fixture(fixture);
            let name = format!("contract.{}", format.extension());
            let path = crate::shell::scratch_copy(&source, &name);
            let (view, cx) = open(&path, cx);
            view.read_with(cx, |view, _| {
                assert_eq!(view.files.format, format);
                assert!(text_of(view).contains("秘密保持契約書"), "{name}");
            });
            cx.simulate_input("jadesaved");
            cx.dispatch_action(crate::Save);
            cx.run_until_parked();
            if format == WordFormat::Doc {
                view.read_with(cx, |view, _| {
                    assert!(matches!(
                        view.files.dialog,
                        Some(FileDialog::Message { .. })
                    ));
                    assert!(view.dirty);
                });
                assert_eq!(
                    std::fs::read(&path).unwrap(),
                    std::fs::read(&source).unwrap()
                );
                continue;
            }
            view.read_with(cx, |view, _| assert!(!view.dirty, "{name}"));
            let saved = std::fs::read(&path).unwrap();
            assert_eq!(WordFormat::sniff(&saved), Some(format), "{name}");
            let (again, cx) = open(&path, cx);
            again.read_with(cx, |view, _| {
                assert!(text_of(view).contains("jadesaved"), "{name}")
            });
        }
    }

    /// A Shift_JIS text file opens with ファイルの変換 showing what it was
    /// read as; reading it in another encoding changes the text; saving it
    /// asks the encoding and writes it.
    #[gpui::test]
    fn a_text_file_is_read_and_written_in_the_encoding_chosen(cx: &mut gpui::TestAppContext) {
        let (bytes, _) = TextEncoding::ShiftJis
            .encode("一行目\r\n二行目\r\n三行目\r\n四行目\r\n五行目\r\n六行目\r\n七行目\r\n");
        let directory = std::env::temp_dir().join(format!("jade-text-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("memo.txt");
        std::fs::write(&path, &bytes).unwrap();
        let (view, cx) = open(&path, cx);
        view.read_with(cx, |view, _| {
            assert_eq!(view.files.encoding, Some(TextEncoding::ShiftJis));
            assert!(matches!(
                view.files.dialog,
                Some(FileDialog::Encoding {
                    purpose: EncodingPurpose::Open,
                    selected: TextEncoding::ShiftJis,
                    ..
                })
            ));
            assert!(text_of(view).contains("七行目"));
        });
        // Read as EUC, the same bytes are other characters.
        view.update(cx, |view, cx| {
            if let Some(FileDialog::Encoding { selected, .. }) = view.files.dialog.as_mut() {
                *selected = TextEncoding::EucJp;
            }
            confirm(view, cx);
        });
        view.read_with(cx, |view, _| {
            assert_eq!(view.files.encoding, Some(TextEncoding::EucJp));
            assert!(!text_of(view).contains("七行目"));
        });
        view.update(cx, |view, cx| {
            view.files.dialog = Some(FileDialog::Encoding {
                purpose: EncodingPurpose::Open,
                selected: TextEncoding::ShiftJis,
                line_ending: LineEnding::CrLf,
            });
            confirm(view, cx);
        });
        view.read_with(cx, |view, _| assert!(text_of(view).contains("七行目")));
        cx.simulate_input("追記");
        cx.dispatch_action(crate::Save);
        cx.run_until_parked();
        view.update(cx, |view, cx| {
            assert!(matches!(
                view.files.dialog,
                Some(FileDialog::Encoding {
                    purpose: EncodingPurpose::Save(_),
                    ..
                })
            ));
            if let Some(FileDialog::Encoding {
                selected,
                line_ending,
                ..
            }) = view.files.dialog.as_mut()
            {
                *selected = TextEncoding::ShiftJis;
                *line_ending = LineEnding::Lf;
            }
            confirm(view, cx);
        });
        let written = std::fs::read(&path).unwrap();
        let text = TextEncoding::ShiftJis.decode(&written);
        assert!(text.contains("追記"), "{text}");
        assert!(text.contains("一行目\n") && !text.contains('\r'));
        std::fs::remove_dir_all(directory).ok();
    }

    /// コピーを保存 writes another file in the format its name says and
    /// leaves the document the file it was, unsaved edits and all;
    /// テンプレートとして保存 declares a template.
    #[gpui::test]
    fn a_copy_and_a_template_are_written_beside_the_document(cx: &mut gpui::TestAppContext) {
        let path = crate::shell::scratch_copy(&core_fixture("contract.docx"), "copy-source.docx");
        let (view, cx) = open(&path, cx);
        cx.simulate_input("jadecopy");
        let copy = path.with_file_name("copy.odt");
        view.update(cx, |view, cx| {
            view.write_file(Write::Copy(copy.clone()), None, cx)
        });
        view.read_with(cx, |view, _| {
            assert!(view.dirty, "the document still has its edits unsaved");
            assert_eq!(PathBuf::from(view.path.as_ref()), path);
            assert_eq!(view.files.format, WordFormat::Docx);
        });
        assert_eq!(
            WordFormat::sniff(&std::fs::read(&copy).unwrap()),
            Some(WordFormat::Odt)
        );
        let template = path.with_file_name("letter.dotx");
        view.update(cx, |view, cx| {
            view.write_file(Write::Save(template.clone()), None, cx)
        });
        assert_eq!(
            WordFormat::sniff(&std::fs::read(&template).unwrap()),
            Some(WordFormat::Dotx)
        );
        view.read_with(cx, |view, _| {
            assert!(!view.dirty);
            assert_eq!(view.files.format, WordFormat::Dotx);
        });
        // A .doc is refused by name.
        let doc = path.with_file_name("old.doc");
        view.update(cx, |view, cx| {
            view.write_file(Write::Save(doc.clone()), None, cx)
        });
        assert!(!doc.exists());
        view.read_with(cx, |view, _| {
            assert!(matches!(
                view.files.dialog,
                Some(FileDialog::Message { .. })
            ))
        });
    }

    /// 自動回復 keeps a copy of unsaved edits in its folder, which a save
    /// removes; バックアップ keeps the file a save replaces.
    #[gpui::test]
    fn unsaved_edits_are_kept_until_saved_and_backups_kept(cx: &mut gpui::TestAppContext) {
        let path = crate::shell::scratch_copy(&core_fixture("contract.docx"), "recover.docx");
        let folder = path.with_file_name("AutoRecovery");
        let settings = crate::autorecover::Settings {
            location: Some(folder.clone()),
            always_backup: true,
            ..Default::default()
        };
        crate::autorecover::set_settings(settings.clone());
        let (view, cx) = open(&path, cx);
        view.update(cx, |view, _| crate::autorecover::tick(view));
        assert!(
            crate::autorecover::recovered_in(&folder).is_empty(),
            "nothing unsaved yet"
        );
        cx.simulate_input("jaderecover");
        view.update(cx, |view, _| crate::autorecover::tick(view));
        let copies = crate::autorecover::recovered_in(&folder);
        assert_eq!(copies.len(), 1);
        assert_eq!(PathBuf::from(&copies[0].original), path);
        let copy = std::fs::read(&copies[0].copy).unwrap();
        let xml = jade_opc::OpcPackage::open(&copy)
            .unwrap()
            .part_str("word/document.xml")
            .unwrap()
            .unwrap();
        assert!(xml.contains("jaderecover"));
        let before = std::fs::read(&path).unwrap();
        cx.dispatch_action(crate::Save);
        cx.run_until_parked();
        assert!(
            crate::autorecover::recovered_in(&folder).is_empty(),
            "a save removes the copy"
        );
        let backup = crate::autorecover::backup_path(&path).unwrap();
        assert_eq!(std::fs::read(backup).unwrap(), before);
        crate::autorecover::set_settings(crate::autorecover::Settings::default());
    }
}

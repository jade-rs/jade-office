//! マスター文書: a master document's サブ文書, as Word's アウトライン表示
//! → マスター文書 group works them — 文書を表示 draws each subdocument in
//! its box, with the subdocument mark at its top left and a padlock when it
//! is locked; 展開/折りたたみ brings every subdocument's text in from its
//! file or shows its link; 作成 makes the heading at the caret and what is
//! under it a subdocument; 挿入 puts a file in as one; リンク解除 makes one's
//! text the master's own; 文書のロック locks one against editing; and
//! サブ文書を開く opens one in its own tab.
//!
//! A save writes the master with each subdocument a link, and each
//! expanded, unlocked subdocument's text back to its own file
//! (`jade_office_model::word::master` does the documents' part).

use std::path::{Path, PathBuf};

use gpui::{div, prelude::*, px, svg, AnyElement, Context, SharedString};
use jade_office_model::word::master::{self, Files, Loaded, Tag, MARK};

use crate::{i18n, theme, DocumentView};

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

/// The master document tools' state.
#[derive(Default)]
pub struct Master {
    /// 文書を表示: the subdocuments' boxes are drawn.
    pub show: bool,
    boxes: Vec<SubdocumentBox>,
}

/// A subdocument's box on one page, in page pixels.
#[derive(Clone, Debug, PartialEq)]
struct SubdocumentBox {
    page: usize,
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
    locked: bool,
    /// The box's first page: the subdocument mark is drawn there.
    first: bool,
}

/// The file `target` names, from the master's folder.
pub fn resolve(directory: &Path, target: &str) -> PathBuf {
    let target = target.split(['?', '#']).next().unwrap_or(target);
    let decoded = crate::percent_decode(target.trim_start_matches("file://"));
    let path = Path::new(&decoded);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        directory.join(path)
    }
}

fn load_from(directory: PathBuf) -> impl Fn(&str) -> Option<Loaded> {
    move |target| {
        let path = resolve(&directory, target);
        let bytes = std::fs::read(&path).ok()?;
        let read_only = std::fs::metadata(&path)
            .map(|metadata| metadata.permissions().readonly())
            .unwrap_or(false);
        Some(Loaded { bytes, read_only })
    }
}

fn display_from(directory: PathBuf) -> impl Fn(&str) -> String {
    move |target| resolve(&directory, target).to_string_lossy().into_owned()
}

/// A master opened from `path`, its subdocuments collapsed to their links,
/// as Word opens one; any other document as it is.
pub fn for_editing(docx: Vec<u8>, path: &Path) -> Vec<u8> {
    if !master::is_master(&docx) {
        return docx;
    }
    let directory = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let load = load_from(directory.clone());
    let display = display_from(directory);
    master::for_editing(
        &docx,
        &Files {
            load: &load,
            display: &display,
        },
    )
    .unwrap_or(docx)
}

impl DocumentView {
    /// The folder the master's subdocuments are found from.
    fn master_directory(&self) -> PathBuf {
        self.save_directory()
    }

    /// The master as stored, before it is written to `target`: each
    /// subdocument a link, and each expanded one's text written to its file
    /// beside `target`.
    pub(crate) fn stored_master(&self, docx: Vec<u8>, target: &Path) -> Result<Vec<u8>, String> {
        if master::subdocuments(&docx).is_empty() {
            return Ok(docx);
        }
        let to = target.parent().map(Path::to_path_buf).unwrap_or_default();
        let load = load_from(self.master_directory());
        let display = display_from(to.clone());
        let (stored, saved) = master::for_saving(
            &docx,
            &Files {
                load: &load,
                display: &display,
            },
        )?;
        write_subdocuments(&to, saved)?;
        Ok(stored)
    }

    /// The subdocument the caret is in.
    pub(crate) fn subdocument_at_caret(&self) -> Option<Tag> {
        if self.editing_story() {
            return None;
        }
        master::subdocument_at(self.document.word_document(), self.insert_at.0)
    }

    /// Whether an edit at the caret, or across the selection, would change
    /// a locked subdocument or a collapsed one's link — which Word does
    /// not allow.
    pub(crate) fn edit_is_locked(&self) -> bool {
        if self.editing_story() {
            return false;
        }
        let (first, last) = match self.document.selection() {
            Some((anchor, focus)) => (anchor.0.min(focus.0), anchor.0.max(focus.0)),
            None => (self.insert_at.0, self.insert_at.0),
        };
        master::edit_is_locked(self.document.word_document(), first..=last)
    }

    /// Refuses an edit into a locked subdocument, saying why.
    pub(crate) fn refuse_locked_edit(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.edit_is_locked() {
            return false;
        }
        self.table_status = Some(SharedString::from(t("master.locked_edit")));
        cx.notify();
        true
    }

    /// The master's text changed by `change`, opened in place of what is
    /// open with the caret kept where it was.
    fn change_master(
        &mut self,
        cx: &mut Context<Self>,
        change: impl FnOnce(&[u8], &Files) -> Result<Vec<u8>, String>,
    ) {
        if let Err(error) = self.close_story_editing() {
            return self.show_message(t("master.title"), &error, cx);
        }
        let directory = self.master_directory();
        let load = load_from(directory.clone());
        let display = display_from(directory);
        let files = Files {
            load: &load,
            display: &display,
        };
        let changed = self
            .document
            .export_docx()
            .map_err(|error| error.to_string())
            .and_then(|docx| change(&docx, &files))
            .and_then(|docx| crate::open_document_bytes(&docx));
        match changed {
            Ok(document) => {
                let caret = self.insert_at;
                self.replace_document(document);
                let paragraph = caret
                    .0
                    .min(self.document.paragraph_count().saturating_sub(1));
                self.insert_at = (paragraph, 0);
                self.caret = self.document.place_caret(paragraph, 0);
                self.dirty = true;
                self.refresh_master_boxes();
                cx.notify();
            }
            Err(error) => self.show_message(t("master.title"), &error, cx),
        }
    }

    /// The master exported with [`MARK`] typed at `at`, the mark then taken
    /// back out of the editor.
    fn marked_master(&mut self, at: (usize, usize)) -> Result<Vec<u8>, String> {
        self.close_story_editing()?;
        let page = self.caret_page_1idx();
        self.document
            .insert_text(at.0, at.1, &MARK.to_string(), page)
            .map_err(|error| error.to_string())?;
        let docx = self
            .document
            .export_docx()
            .map_err(|error| error.to_string());
        self.document.undo().map_err(|error| error.to_string())?;
        docx
    }

    /// 文書を表示.
    pub(crate) fn toggle_master_view(&mut self, cx: &mut Context<Self>) {
        self.master.show = !self.master.show;
        self.refresh_master_boxes();
        cx.notify();
    }

    /// サブ文書の展開 / 折りたたみ: every subdocument, the way the first
    /// one is not.
    pub(crate) fn expand_or_collapse_subdocuments(&mut self, cx: &mut Context<Self>) {
        let Ok(docx) = self.document.export_docx() else {
            return;
        };
        let subdocuments = master::subdocuments(&docx);
        if subdocuments.is_empty() {
            return self.show_message(t("master.title"), t("master.none"), cx);
        }
        let collapse = subdocuments.iter().any(|subdocument| subdocument.expanded);
        let directory = self.master_directory();
        let mut unreachable = Vec::new();
        self.change_master(cx, |docx, files| {
            if collapse {
                let (collapsed, saved) = master::collapse(docx, None, files)?;
                write_subdocuments(&directory, saved)?;
                Ok(collapsed)
            } else {
                let (expanded, missing) = master::expand(docx, None, files)?;
                unreachable = missing;
                Ok(expanded)
            }
        });
        if !unreachable.is_empty() {
            let names = unreachable.join(", ");
            self.show_message(
                t("master.title"),
                &i18n::format("master.unreachable", &[("names", &names)]),
                cx,
            );
        }
    }

    /// 作成: the heading at the caret and what is under it, a subdocument.
    pub(crate) fn create_subdocument(&mut self, cx: &mut Context<Self>) {
        let marked = match self.marked_master((self.insert_at.0, 0)) {
            Ok(docx) => docx,
            Err(error) => return self.show_message(t("master.title"), &error, cx),
        };
        let directory = self.master_directory();
        self.change_master(cx, move |_, _| {
            let exists = |name: &str| directory.join(name).exists();
            master::create(&marked, &exists).map(|(docx, _)| docx)
        });
    }

    /// 挿入: a file, chosen, as an expanded subdocument after the caret's
    /// paragraph.
    pub(crate) fn insert_subdocument(&mut self, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(SharedString::from(t("master.insert_prompt"))),
        });
        cx.spawn(async move |view, cx| {
            let Ok(Ok(Some(paths))) = prompt.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            view.update(cx, |view, cx| view.insert_subdocument_file(&path, cx))
                .ok();
        })
        .detach();
    }

    pub(crate) fn insert_subdocument_file(&mut self, path: &Path, cx: &mut Context<Self>) {
        let directory = self.master_directory();
        // Beside the master, by its name; elsewhere, by its full path.
        let target = match path.parent() {
            Some(parent) if parent == directory => crate::shell::file_name(path),
            _ => path.to_string_lossy().into_owned(),
        };
        let end = self
            .document
            .paragraph_text(self.insert_at.0)
            .map_or(0, |text| text.len());
        let marked = match self.marked_master((self.insert_at.0, end)) {
            Ok(docx) => docx,
            Err(error) => return self.show_message(t("master.title"), &error, cx),
        };
        self.change_master(cx, move |_, files| master::insert(&marked, &target, files));
    }

    /// リンク解除: the subdocument at the caret, the master's own text.
    pub(crate) fn unlink_subdocument(&mut self, cx: &mut Context<Self>) {
        let Some(tag) = self.subdocument_at_caret() else {
            return self.show_message(t("master.title"), t("master.not_in_subdocument"), cx);
        };
        self.change_master(cx, move |docx, files| {
            master::unlink(docx, &tag.rel_id, files)
        });
    }

    /// 文書のロック: the subdocument at the caret, locked or unlocked.
    pub(crate) fn toggle_subdocument_lock(&mut self, cx: &mut Context<Self>) {
        let Some(tag) = self.subdocument_at_caret() else {
            return self.show_message(t("master.title"), t("master.not_in_subdocument"), cx);
        };
        self.change_master(cx, move |docx, _| {
            master::set_locked(docx, &tag.rel_id, !tag.locked)
        });
    }

    /// サブ文書を開く: the subdocument at the caret, in a tab of its own.
    pub(crate) fn open_subdocument(&mut self, cx: &mut Context<Self>) {
        let Some(tag) = self.subdocument_at_caret() else {
            return self.show_message(t("master.title"), t("master.not_in_subdocument"), cx);
        };
        let Ok(docx) = self.document.export_docx() else {
            return;
        };
        let Some(subdocument) = master::subdocuments(&docx)
            .into_iter()
            .find(|subdocument| subdocument.rel_id == tag.rel_id)
        else {
            return;
        };
        let path = resolve(&self.master_directory(), &subdocument.target);
        if !path.exists() {
            return self.show_message(
                t("master.title"),
                &i18n::format("master.unreachable", &[("names", &subdocument.target)]),
                cx,
            );
        }
        crate::shell::open_paths(&[path], cx);
    }

    /// Works out the subdocuments' boxes again, when they are shown.
    pub(crate) fn refresh_master_boxes(&mut self) {
        let mut boxes = Vec::new();
        if self.master.show && !self.editing_story() {
            // Runs of paragraphs in the same subdocument.
            let count = self.document.paragraph_count();
            let mut runs: Vec<(String, bool, usize, usize)> = Vec::new();
            for paragraph in 0..count {
                let tag = self
                    .document
                    .word_document()
                    .block_content_control_tags(paragraph)
                    .iter()
                    .find_map(|tag| Tag::parse(tag));
                let Some(tag) = tag else {
                    continue;
                };
                match runs.last_mut() {
                    Some(run) if run.0 == tag.rel_id && run.3 + 1 == paragraph => run.3 = paragraph,
                    _ => runs.push((tag.rel_id, tag.locked, paragraph, paragraph)),
                }
            }
            let (page_width, _) = self.document.layout_page_size();
            for (_, locked, first, last) in runs {
                let layout = {
                    let document = self.document.word_document();
                    document.section_layout(document.section_index_for_paragraph(first))
                };
                let scale = if layout.width > 0.0 {
                    page_width / layout.width as f64
                } else {
                    1.0
                };
                let left = layout.margin_left as f64 * scale - 14.0;
                let right = page_width - layout.margin_right as f64 * scale + 6.0;
                let end = self
                    .document
                    .paragraph_text(last)
                    .map_or(0, |text| text.len());
                let (Some(top), Some(bottom)) = (
                    self.document.position_rect(first, 0),
                    self.document.position_rect(last, end),
                ) else {
                    continue;
                };
                let (first_page, top_y) = self.document.page_and_offset(top.document_y);
                let (last_page, bottom_y) = self
                    .document
                    .page_and_offset(bottom.document_y + bottom.height);
                let (_, page_height) = self.document.layout_page_size();
                for page in first_page..=last_page {
                    boxes.push(SubdocumentBox {
                        page,
                        left,
                        right,
                        top: if page == first_page {
                            top_y - 4.0
                        } else {
                            layout.margin_top as f64 * scale
                        },
                        bottom: if page == last_page {
                            bottom_y + 4.0
                        } else {
                            page_height - layout.margin_bottom as f64 * scale
                        },
                        locked,
                        first: page == first_page,
                    });
                }
            }
        }
        self.master.boxes = boxes;
    }

    /// The subdocuments' boxes on page `page_index`.
    pub(crate) fn master_boxes(&self, page_index: usize) -> Vec<AnyElement> {
        let zoom = self.zoom;
        self.master
            .boxes
            .iter()
            .filter(|subdocument| subdocument.page == page_index)
            .map(|subdocument| {
                let mark = |path: &'static str| {
                    svg()
                        .path(path)
                        .size(px(11.0))
                        .text_color(theme::text_dim())
                };
                div()
                    .absolute()
                    .left(px(subdocument.left as f32 * zoom))
                    .top(px(subdocument.top as f32 * zoom))
                    .w(px((subdocument.right - subdocument.left) as f32 * zoom))
                    .h(px(
                        (subdocument.bottom - subdocument.top).max(1.0) as f32 * zoom
                    ))
                    .border_1()
                    .border_color(theme::text_faint())
                    .when(subdocument.first, |frame| {
                        frame.child(
                            div()
                                .absolute()
                                .left(px(-16.0))
                                .top(px(0.0))
                                .flex()
                                .flex_col()
                                .gap_0p5()
                                .child(mark("icons/file-text.svg"))
                                .when(subdocument.locked, |icons| {
                                    icons.child(mark("icons/lock.svg"))
                                }),
                        )
                    })
                    .into_any_element()
            })
            .collect()
    }

    /// 表示's マスター文書 group.
    pub(crate) fn master_group(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let in_subdocument = self.subdocument_at_caret();
        let expanded = in_subdocument.as_ref().is_some_and(|tag| tag.expanded);
        let locked = in_subdocument.as_ref().is_some_and(|tag| tag.locked);
        Self::ribbon_group(
            t("master.group"),
            vec![
                Self::ribbon_row(vec![
                    self.ribbon_button(
                        "master-show",
                        t("master.show"),
                        t("master.show_title"),
                        self.master.show,
                        cx,
                        |view, cx| view.toggle_master_view(cx),
                    )
                    .into_any_element(),
                    self.ribbon_button(
                        "master-expand",
                        t("master.expand_collapse"),
                        t("master.expand_collapse_title"),
                        expanded,
                        cx,
                        |view, cx| view.expand_or_collapse_subdocuments(cx),
                    )
                    .into_any_element(),
                    self.ribbon_button(
                        "master-create",
                        t("master.create"),
                        t("master.create_title"),
                        false,
                        cx,
                        |view, cx| view.create_subdocument(cx),
                    )
                    .into_any_element(),
                    self.ribbon_button(
                        "master-insert",
                        t("master.insert"),
                        t("master.insert_title"),
                        false,
                        cx,
                        |view, cx| view.insert_subdocument(cx),
                    )
                    .into_any_element(),
                ]),
                Self::ribbon_row(vec![
                    self.ribbon_button(
                        "master-unlink",
                        t("master.unlink"),
                        t("master.unlink_title"),
                        false,
                        cx,
                        |view, cx| view.unlink_subdocument(cx),
                    )
                    .into_any_element(),
                    self.ribbon_button(
                        "master-lock",
                        t("master.lock"),
                        t("master.lock_title"),
                        locked,
                        cx,
                        |view, cx| view.toggle_subdocument_lock(cx),
                    )
                    .into_any_element(),
                    self.ribbon_button(
                        "master-open",
                        t("master.open"),
                        t("master.open_title"),
                        false,
                        cx,
                        |view, cx| view.open_subdocument(cx),
                    )
                    .into_any_element(),
                ]),
            ],
        )
        .into_any_element()
    }
}

/// Writes subdocument files beside the master in `directory`. A locked one
/// that cannot be written is left as it is: its lock was why.
fn write_subdocuments(directory: &Path, saved: Vec<master::Saved>) -> Result<(), String> {
    for file in saved {
        let path = resolve(directory, &file.target);
        match std::fs::write(&path, &file.bytes) {
            Ok(()) => {}
            Err(_) if file.locked => {}
            Err(error) => return Err(format!("{}: {error}", path.display())),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::Focusable;
    use jade_opc::OpcPackage;

    fn document_with(body: &str) -> Vec<u8> {
        let mut package = OpcPackage::open(&jade_office_model::word::new::blank_document(
            jade_office_model::word::new::Blank::English,
        ))
        .unwrap();
        let xml = package.part_str("word/document.xml").unwrap().unwrap();
        package.replace(
            "word/document.xml",
            xml.replace("<w:body><w:p/>", &format!("<w:body>{body}"))
                .into_bytes(),
        );
        package.write().unwrap()
    }

    fn paragraph(text: &str) -> String {
        format!("<w:p><w:r><w:t>{text}</w:t></w:r></w:p>")
    }

    fn heading(text: &str) -> String {
        format!(
            r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>{text}</w:t></w:r></w:p>"#
        )
    }

    /// A folder with a master and its one subdocument, as Word saves them.
    fn master_folder(name: &str) -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("jade-master-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(
            folder.join("chapter.docx"),
            document_with(&format!(
                "{}{}",
                heading("Chapter"),
                paragraph("Chapter text")
            )),
        )
        .unwrap();
        let mut package = OpcPackage::open(&document_with(&format!(
            r#"{}<w:subDoc r:id="rId90"/>{}"#,
            paragraph("Master start"),
            paragraph("Master end")
        )))
        .unwrap();
        let rels = package
            .part_str("word/_rels/document.xml.rels")
            .unwrap()
            .unwrap();
        package.replace(
            "word/_rels/document.xml.rels",
            rels.replace(
                "</Relationships>",
                &format!(r#"<Relationship Id="rId90" Type="{}" Target="chapter.docx" TargetMode="External"/></Relationships>"#, master::SUBDOCUMENT),
            )
            .into_bytes(),
        );
        std::fs::write(folder.join("master.docx"), package.write().unwrap()).unwrap();
        folder
    }

    fn open<'a>(
        path: &Path,
        cx: &'a mut gpui::TestAppContext,
    ) -> (gpui::Entity<DocumentView>, &'a mut gpui::VisualTestContext) {
        let path = path.to_path_buf();
        let (view, cx) =
            cx.add_window_view(move |_window, cx| crate::open_document_view(&path, cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        (view, cx)
    }

    fn texts(view: &DocumentView) -> Vec<String> {
        (0..view.document.paragraph_count())
            .filter_map(|paragraph| view.document.paragraph_text(paragraph))
            .collect()
    }

    fn put_caret(view: &mut DocumentView, paragraph: usize) {
        view.insert_at = (paragraph, 0);
        view.caret = view.document.place_caret(paragraph, 0);
    }

    /// A master opens with its subdocument a link in a box; expanded, its
    /// text is in the master, edited there and saved back to its own file
    /// while the master keeps only the link.
    #[gpui::test]
    fn a_master_opens_collapsed_expands_and_saves_its_subdocument(cx: &mut gpui::TestAppContext) {
        let folder = master_folder("expand");
        let (view, cx) = open(&folder.join("master.docx"), cx);
        view.update(cx, |view, cx| {
            let link = folder.join("chapter.docx").to_string_lossy().into_owned();
            assert_eq!(texts(view)[1], link);
            view.toggle_master_view(cx);
            let frame = view
                .master
                .boxes
                .first()
                .expect("the subdocument has its box")
                .clone();
            assert!(frame.page == 0 && frame.left < frame.right && frame.top < frame.bottom);
            // Its link is not text to edit.
            put_caret(view, 1);
            assert!(view.edit_is_locked());
            view.expand_or_collapse_subdocuments(cx);
            assert!(texts(view).contains(&"Chapter text".to_string()));
            assert!(view.master.boxes.len() == 1);
        });
        let chapter = view.read_with(cx, |view, _| {
            texts(view)
                .iter()
                .position(|text| text == "Chapter text")
                .unwrap()
        });
        view.update(cx, |view, _| put_caret(view, chapter));
        cx.simulate_input("Revised ");
        cx.dispatch_action(crate::Save);
        cx.run_until_parked();
        let saved = std::fs::read(folder.join("chapter.docx")).unwrap();
        let text = jade_docx_convert::read_docx(&saved).unwrap().plain_text();
        assert!(text.contains("Revised Chapter text"), "{text}");
        let stored = std::fs::read(folder.join("master.docx")).unwrap();
        let xml = OpcPackage::open(&stored)
            .unwrap()
            .part_str("word/document.xml")
            .unwrap()
            .unwrap();
        assert!(xml.contains(r#"<w:subDoc r:id="rId90"/>"#));
        assert!(!xml.contains("Chapter text"));
        std::fs::remove_dir_all(folder).ok();
    }

    /// 作成 makes the heading at the caret a subdocument, written on save;
    /// 文書のロック stops typing in it; リンク解除 makes it the master's own.
    #[gpui::test]
    fn create_lock_and_unlink(cx: &mut gpui::TestAppContext) {
        let folder =
            std::env::temp_dir().join(format!("jade-master-create-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("book.docx");
        std::fs::write(
            &path,
            document_with(&format!(
                "{}{}{}",
                paragraph("Foreword"),
                heading("Part one"),
                paragraph("Part one text")
            )),
        )
        .unwrap();
        let (view, cx) = open(&path, cx);
        view.update(cx, |view, cx| {
            put_caret(view, 1);
            view.create_subdocument(cx);
            assert!(view.files.dialog.is_none());
            put_caret(view, 2);
            assert_eq!(
                view.subdocument_at_caret().map(|tag| tag.expanded),
                Some(true)
            );
            view.toggle_subdocument_lock(cx);
            put_caret(view, 2);
            assert!(view.edit_is_locked());
        });
        let before = view.read_with(cx, |view, _| texts(view));
        cx.simulate_input("typed");
        view.read_with(cx, |view, _| assert_eq!(texts(view), before, "locked"));
        view.update(cx, |view, cx| {
            put_caret(view, 2);
            view.toggle_subdocument_lock(cx);
            view.files.checked = true;
        });
        cx.dispatch_action(crate::Save);
        cx.run_until_parked();
        let created = folder.join("Part one.docx");
        let text = jade_docx_convert::read_docx(&std::fs::read(&created).unwrap())
            .unwrap()
            .plain_text();
        assert!(text.contains("Part one text"), "{text}");
        view.update(cx, |view, cx| {
            put_caret(view, 2);
            view.unlink_subdocument(cx);
            put_caret(view, 2);
            assert!(view.subdocument_at_caret().is_none());
            assert!(texts(view).contains(&"Part one text".to_string()));
        });
        std::fs::remove_dir_all(folder).ok();
    }

    /// 挿入 puts a file in after the caret's paragraph, expanded.
    #[gpui::test]
    fn a_file_is_inserted_as_a_subdocument(cx: &mut gpui::TestAppContext) {
        let folder = master_folder("insert");
        let path = folder.join("plain.docx");
        std::fs::write(&path, document_with(&paragraph("Plain"))).unwrap();
        let (view, cx) = open(&path, cx);
        view.update(cx, |view, cx| {
            put_caret(view, 0);
            view.insert_subdocument_file(&folder.join("chapter.docx"), cx);
            assert_eq!(texts(view)[..3], ["Plain", "Chapter", "Chapter text"]);
            put_caret(view, 1);
            assert!(view.subdocument_at_caret().is_some());
        });
        std::fs::remove_dir_all(folder).ok();
    }
}

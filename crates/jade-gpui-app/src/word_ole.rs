//! 挿入 › オブジェクト… › ファイルから: a file embedded in the document as
//! Word embeds one — an OLE package shown as the file's icon and name
//! (アイコンで表示), or a Word document's first page — and a double-click
//! on it opens the embedded file with the app the system opens it with.
//! What is embedded and how is `jade-office-model`'s `word::ole_objects`.

use std::path::{Path, PathBuf};

use gpui::{
    actions, div, prelude::*, px, AnyElement, ClickEvent, Context, PathPromptOptions, SharedString,
};
use jade_docx_native_render::ObjectHit;
#[cfg(test)]
use jade_office_model::word::ole_objects::OBJECT_ICON_SIZE;
use jade_office_model::word::ole_objects::{
    object_picture, ObjectDialog, ObjectPicture, WordOleObjects,
};
use jade_office_model::word::signatures::rgba_png;

use crate::word_characters::{button, check};
use crate::word_files::FileDialog;
use crate::{i18n, DocumentView};

actions!(
    jade,
    [
        /// 挿入 › オブジェクト…: the dialog a file is embedded from.
        InsertObject,
    ]
);

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

/// オブジェクト…: the dialog.
pub fn open_dialog(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    view.files.dialog = Some(FileDialog::Object(ObjectDialog::default()));
    cx.notify();
}

/// Page 0 of `docx`, as a PNG, and its size in pixels.
fn first_page(docx: &[u8]) -> Result<(Vec<u8>, f64, f64), String> {
    let document = crate::open_document_bytes(docx)?;
    let (bgra, width, height) = document
        .page_pixels(0)
        .ok_or("the object could not be drawn")?;
    let mut rgba = bgra;
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    Ok((
        rgba_png(width as u32, height as u32, &rgba)?,
        width as f64,
        height as f64,
    ))
}

/// The picture the object is shown as, and its size: a Word document's
/// first page when not shown as an icon, the icon otherwise.
pub fn picture(name: &str, bytes: &[u8], icon: bool) -> Result<ObjectPicture, String> {
    object_picture(name, bytes, icon, first_page)
}

/// `name`'s `bytes` embedded at the caret.
pub fn insert_file(
    view: &mut DocumentView,
    name: &str,
    bytes: Vec<u8>,
    icon: bool,
    cx: &mut Context<DocumentView>,
) {
    if view.document.selection().is_none() {
        view.caret = view
            .document
            .place_caret(view.insert_at.0, view.insert_at.1);
    }
    let name = name.to_string();
    view.object_edit(cx, move |view| {
        let picture = picture(&name, &bytes, icon)?;
        view.document
            .insert_ole_object(&name, bytes, picture.png, picture.size, picture.icon)
    });
}

/// ファイルから…: asks for the file, then embeds it.
fn choose_file(dialog: ObjectDialog, cx: &mut Context<DocumentView>) {
    let paths = cx.prompt_for_paths(PathPromptOptions {
        files: true,
        directories: false,
        multiple: false,
        prompt: Some(SharedString::from(t("object.from_file"))),
    });
    cx.spawn(async move |view, cx| {
        let Ok(Ok(Some(paths))) = paths.await else {
            return;
        };
        let Some(path) = paths.into_iter().next() else {
            return;
        };
        let Ok(bytes) = std::fs::read(&path) else {
            return;
        };
        let name = crate::shell::file_name(&path);
        view.update(cx, |view, cx| {
            insert_file(view, &name, bytes, dialog.icon, cx)
        })
        .ok();
    })
    .detach();
}

/// The dialog's OK (ファイルから…).
pub fn confirm(_view: &mut DocumentView, dialog: ObjectDialog, cx: &mut Context<DocumentView>) {
    choose_file(dialog, cx);
}

/// The dialog: what it does, アイコンで表示, and ファイルから….
pub fn dialog(
    dialog: ObjectDialog,
    cx: &mut Context<DocumentView>,
) -> (SharedString, AnyElement, AnyElement) {
    let body = div()
        .flex()
        .flex_col()
        .gap_3()
        .w(px(380.0))
        .child(
            div()
                .text_color(crate::theme::text_dim())
                .child(t("object.detail")),
        )
        .child(
            check("object-icon", t("object.icon"), dialog.icon).on_click(cx.listener(
                |view, _: &ClickEvent, _, cx| {
                    if let Some(FileDialog::Object(dialog)) = view.files.dialog.as_mut() {
                        dialog.icon = !dialog.icon;
                    }
                    cx.notify();
                },
            )),
        )
        .into_any_element();
    let buttons = div()
        .flex()
        .justify_end()
        .gap_2()
        .child(
            button("object-cancel", t("format.cancel"), false).on_click(cx.listener(
                |view, _: &ClickEvent, _, cx| {
                    view.files.dialog = None;
                    cx.notify();
                },
            )),
        )
        .child(
            button("object-file", t("object.from_file"), true).on_click(cx.listener(
                |view, _: &ClickEvent, _, cx| {
                    if let Some(FileDialog::Object(dialog)) = view.files.dialog.take() {
                        confirm(view, dialog, cx);
                    }
                    cx.notify();
                },
            )),
        )
        .into_any_element();
    (t("object.title").into(), body, buttons)
}

/// Where an embedded file is put to be opened: a folder of the app's own
/// in the temporary folder, one per object so names never collide.
fn opened_path(paragraph: usize, run: usize, name: &str) -> PathBuf {
    let safe: String = name
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':') {
                '_'
            } else {
                c
            }
        })
        .collect();
    std::env::temp_dir()
        .join(format!("jade-objects-{}", std::process::id()))
        .join(format!("{paragraph}-{run}"))
        .join(if safe.is_empty() {
            "object".into()
        } else {
            safe
        })
}

/// The embedded file of the object `object`, written out to be opened.
pub fn extract(view: &DocumentView, object: &ObjectHit) -> Option<PathBuf> {
    let file = view
        .document
        .embedded_file_at(object.paragraph, object.run)?;
    let path = opened_path(object.paragraph, object.run, &file.name);
    std::fs::create_dir_all(path.parent()?).ok()?;
    std::fs::write(&path, &file.bytes).ok()?;
    Some(path)
}

/// A double-click on `object`: when it embeds a file, the file opens in
/// the app the system opens it with. Whether it did.
pub fn open_object(view: &mut DocumentView, object: &ObjectHit) -> bool {
    let Some(path) = extract(view, object) else {
        return false;
    };
    open_with_system(&path);
    true
}

#[cfg(not(test))]
fn open_with_system(path: &Path) {
    std::process::Command::new("open").arg(path).spawn().ok();
}

#[cfg(test)]
fn open_with_system(path: &Path) {
    OPENED.with(|opened| opened.borrow_mut().push(path.to_path_buf()));
}

#[cfg(test)]
thread_local! {
    static OPENED: std::cell::RefCell<Vec<PathBuf>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The 挿入 tab's オブジェクト button.
pub fn insert_group(view: &mut DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    DocumentView::ribbon_group(
        t("object.group"),
        vec![DocumentView::ribbon_row(vec![view
            .ribbon_button(
                "insert-object",
                t("object.button"),
                t("object.tooltip"),
                false,
                cx,
                open_dialog,
            )
            .into_any_element()])],
    )
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// オブジェクト embeds a file as an icon; a double-click opens the file
    /// it holds; saved, the package is in the file and reads back the same.
    #[gpui::test]
    fn a_file_is_embedded_shown_and_opened(cx: &mut gpui::TestAppContext) {
        use gpui::Focusable;
        let bytes = jade_office_model::word::new::blank_document(
            jade_office_model::word::new::Blank::English,
        );
        let (view, cx) = cx.add_window_view(|_window, cx| {
            crate::untitled_view(&bytes, "objects.docx", cx).unwrap()
        });
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        cx.dispatch_action(InsertObject);
        view.update(cx, |view, cx| {
            assert!(matches!(
                view.files.dialog,
                Some(FileDialog::Object(ObjectDialog { icon: true }))
            ));
            view.files.dialog = None;
            view.insert_at = (0, 0);
            view.caret = view.document.place_caret(0, 0);
            insert_file(view, "見積書.csv", b"a,b\n1,2\n".to_vec(), true, cx);
            assert!(view.objects.status.is_none(), "{:?}", view.objects.status);
        });
        let (paragraph, run) = view.read_with(cx, |view, _| {
            let document = view.document.word_document();
            let paragraph = document.paragraph(0).unwrap();
            let run = paragraph
                .runs
                .iter()
                .position(|run| {
                    run.image
                        .as_ref()
                        .is_some_and(|image| image.ole_program_id.is_some())
                })
                .unwrap();
            let image = paragraph.runs[run].image.as_ref().unwrap();
            assert_eq!(image.width_px, Some(OBJECT_ICON_SIZE.0));
            (0, run)
        });
        // The icon is drawn: the page is no longer blank where it is.
        view.read_with(cx, |view, _| {
            let (bgra, _, _) = view.document.page_pixels(0).unwrap();
            assert!(bgra.chunks_exact(4).any(|pixel| pixel[0] < 200));
        });
        view.update(cx, |view, _| {
            let hit = ObjectHit {
                kind: jade_docx_editor::word_document::FloatingObjectKind::Image,
                paragraph,
                run,
                page: 1,
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            };
            assert!(open_object(view, &hit));
            let opened = OPENED
                .with(|opened| opened.borrow().last().cloned())
                .unwrap();
            assert!(opened.ends_with("見積書.csv"));
            assert_eq!(std::fs::read(&opened).unwrap(), b"a,b\n1,2\n");
            let saved = view.document.export_docx().unwrap();
            let reopened = crate::open_document_bytes(&saved).unwrap();
            let file = reopened.embedded_file_at(paragraph, run).unwrap();
            assert_eq!(file.name, "見積書.csv");
        });
    }

    /// A Word document shown as its content is drawn as its first page.
    #[gpui::test]
    fn a_word_document_can_show_its_first_page(_cx: &mut gpui::TestAppContext) {
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../jade-core/contract.docx"
        ))
        .unwrap();
        let ObjectPicture { size, icon, .. } = picture("contract.docx", &bytes, false).unwrap();
        assert!(!icon);
        assert!(size.1 > size.0, "a portrait page: {size:?}");
        let ObjectPicture { size, icon, .. } = picture("contract.pdf", &bytes, false).unwrap();
        assert!(icon);
        assert_eq!(size, OBJECT_ICON_SIZE);
    }
}

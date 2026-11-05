//! 自動保存 and ファイル › バージョン履歴 for a Word document, over
//! `jade-office-model`'s `versions`:
//!
//! - the title bar's 自動保存 switch saves the document to its own file a
//!   moment after the edits stop, never while a dialog is open over it;
//!   a new document asks where to save first, and a format other than
//!   Word's own is refused, as Word refuses it;
//! - every save leaves a version in the app's own folder (Application
//!   Support/jade-office/Versions), the file as it was before the first
//!   save kept too; バージョン履歴 lists them beside the pages, 開く opens
//!   one as a document of its own, 復元 puts one back and saves it.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use gpui::{actions, div, prelude::*, px, AnyElement, ClickEvent, Context, SharedString};
use jade_office_model::versions::{
    opened_name, saved_label, AutoSave, Version, VersionKind, VersionStore,
};
use jade_office_model::word::formats::{open_as_docx, OpenOptions};

use crate::word_characters::button;
use crate::{i18n, theme, DocumentView};

actions!(
    jade,
    [
        /// ファイル › バージョン履歴: the pane listing the saved versions.
        VersionHistory,
    ]
);

/// A document's 自動保存 switch, its versions pane and what the next save
/// records itself as.
pub struct Versions {
    pub auto: AutoSave,
    /// The pane, while it is open: the versions newest first.
    pub pane: Option<Vec<Version>>,
    /// What the save under way is, when not a plain 保存.
    next_kind: Option<VersionKind>,
    /// Whether the file as it was opened has been kept.
    original_kept: bool,
    clock: Instant,
}

impl Default for Versions {
    /// The switch as 環境設定 › 保存 sets it for a document just opened.
    fn default() -> Self {
        Self {
            auto: AutoSave::new(crate::autorecover::settings().autosave),
            pane: None,
            next_kind: None,
            original_kept: false,
            clock: Instant::now(),
        }
    }
}

thread_local! {
    /// Where the versions go instead of Application Support, for tests.
    static ROOT: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub fn set_root_for_tests(root: Option<PathBuf>) {
    ROOT.with(|cell| *cell.borrow_mut() = root);
}

/// The store, in the app's own data folder, never beside the user's file.
pub fn store() -> Option<VersionStore> {
    ROOT.with(|cell| cell.borrow().clone())
        .or_else(|| crate::autorecover::support().map(|support| support.join("Versions")))
        .map(VersionStore::new)
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

/// Starts `view`'s clock: twice a second it checks whether 自動保存 is due.
pub fn start(cx: &mut Context<DocumentView>) {
    cx.spawn(async move |view, cx| loop {
        cx.background_executor()
            .timer(Duration::from_millis(500))
            .await;
        let alive = view.update(cx, tick).is_ok();
        if !alive {
            break;
        }
    })
    .detach();
}

/// Whether something is being filled in over the document — a dialog, a
/// bar, a composition, a drag — which an autosave waits for.
pub fn busy(view: &DocumentView) -> bool {
    view.dialog.is_some()
        || view.page_dialog.is_some()
        || view.tools.dialog.is_some()
        || view.tables.dialog.is_some()
        || view.fields.dialog.is_some()
        || view.files.dialog.is_some()
        || view.printing.is_some()
        || view.characters.is_open()
        || view.review.is_open()
        || view.views.is_open()
        || view.references.is_open()
        || view.controls.is_open()
        || view.mailings.is_open()
        || view.comment_bar.is_some()
        || view.story_bar.is_some()
        || view.marked.is_some()
        || view.drag_anchor.is_some()
}

/// Whether 自動保存 can save this document: one with a file of its own, in
/// one of Word's own formats.
fn can_autosave(view: &DocumentView) -> bool {
    !view.untitled && view.files.format.is_package()
}

/// Saves the document when 自動保存 says it is due.
pub fn tick(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let now = view.versions.clock.elapsed().as_secs_f64() * 1000.0;
    let busy = busy(view);
    if view.versions.auto.poll(view.edits, view.dirty, busy, now) && can_autosave(view) {
        autosave(view, cx);
    }
}

/// Saves over the document's file as 自動保存 does.
pub fn autosave(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    view.versions.next_kind = Some(VersionKind::AutoSaved);
    // The compatibility check asks only on a save someone made.
    view.files.checked = true;
    let target = PathBuf::from(view.path.to_string());
    view.write_file(crate::word_files::Write::Save(target), None, cx);
    view.versions.next_kind = None;
    view.files.checked = false;
}

/// The title bar's switch, clicked.
pub fn toggle(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    if view.versions.auto.on {
        view.versions.auto.toggle();
        return cx.notify();
    }
    if !view.untitled && !view.files.format.is_package() {
        view.show_message(
            i18n::text("versions.autosave"),
            i18n::text("versions.unsupported"),
            cx,
        );
        return;
    }
    view.versions.auto.toggle();
    // A new document has nowhere to save to yet: asked for first, as
    // Word asks where to keep it.
    if view.untitled {
        view.save_file_as(cx);
    }
    cx.notify();
}

/// Before a save replaces `target`: what is there now is kept as the
/// version the history starts from.
pub fn before_writing(view: &mut DocumentView, target: &Path) {
    if view.versions.original_kept {
        return;
    }
    view.versions.original_kept = true;
    if let Some(store) = store() {
        store.keep_original(target, now_seconds()).ok();
    }
}

/// After a save to `path` wrote `bytes`: a version of them.
pub fn saved(view: &mut DocumentView, path: &Path, bytes: &[u8]) {
    let kind = view.versions.next_kind.unwrap_or(VersionKind::Saved);
    view.versions.auto.saved();
    if let Some(store) = store() {
        store.record(path, bytes, kind, now_seconds()).ok();
        if view.versions.pane.is_some() {
            view.versions.pane = Some(store.list(path));
        }
    }
}

/// ファイル › バージョン履歴: the pane, or shut.
pub fn toggle_pane(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    view.versions.pane = match view.versions.pane.take() {
        Some(_) => None,
        None => Some(listed(view)),
    };
    cx.notify();
}

fn listed(view: &DocumentView) -> Vec<Version> {
    if view.untitled {
        return Vec::new();
    }
    store()
        .map(|store| store.list(Path::new(view.path.as_ref())))
        .unwrap_or_default()
}

fn label(version: &Version) -> String {
    let offset = crate::word_fields::local_now().1;
    saved_label(
        version.saved_at,
        offset,
        i18n::language() == i18n::Language::Japanese,
    )
}

/// The document a version opens as: its name and its bytes.
pub fn opened(view: &DocumentView, index: usize) -> Option<(String, Vec<u8>)> {
    let version = view.versions.pane.as_ref()?.get(index)?;
    let bytes = std::fs::read(&version.file).ok()?;
    Some((
        opened_name(Path::new(view.path.as_ref()), &label(version)),
        bytes,
    ))
}

/// 開く: the version as a document of its own, beside this one.
pub fn open(view: &mut DocumentView, index: usize, cx: &mut Context<DocumentView>) {
    if let Some((name, bytes)) = opened(view, index) {
        crate::shell::open_untitled(name, bytes, cx);
    }
}

/// 復元: the version in place of the document, saved as its newest.
pub fn restore(view: &mut DocumentView, index: usize, cx: &mut Context<DocumentView>) {
    let Some(version) = view
        .versions
        .pane
        .as_ref()
        .and_then(|pane| pane.get(index))
        .cloned()
    else {
        return;
    };
    let name = crate::shell::file_name(Path::new(view.path.as_ref()));
    let document = std::fs::read(&version.file)
        .map_err(|error| error.to_string())
        .and_then(|bytes| {
            open_as_docx(&name, &bytes, OpenOptions::default(), &|_| None)
                .map_err(|error| format!("{error:?}"))
        })
        .and_then(|opened| crate::open_document_bytes(&opened.docx));
    match document {
        Ok(document) => {
            view.replace_document(document);
            view.dirty = true;
            if !view.untitled {
                view.versions.next_kind = Some(VersionKind::Restored);
                view.save_file(cx);
                view.versions.next_kind = None;
            }
            view.versions.pane = Some(listed(view));
            cx.notify();
        }
        Err(error) => view.show_message(i18n::text("versions.title"), &error, cx),
    }
}

/// The 自動保存 switch for the title bar: its label and whether it is on.
pub fn switch_label(on: bool) -> SharedString {
    SharedString::from(i18n::text(if on { "versions.on" } else { "versions.off" }))
}

/// The pane beside the pages, while it is open.
pub fn pane(view: &mut DocumentView, cx: &mut Context<DocumentView>) -> Option<AnyElement> {
    let versions = view.versions.pane.as_ref()?;
    let mut rows: Vec<AnyElement> = Vec::new();
    if versions.is_empty() {
        rows.push(
            div()
                .text_color(theme::text_dim())
                .child(i18n::text(if view.untitled {
                    "versions.unsaved"
                } else {
                    "versions.none"
                }))
                .into_any_element(),
        );
    }
    for (index, version) in versions.iter().enumerate() {
        let note = if index == 0 {
            i18n::text("versions.current")
        } else {
            i18n::text(version.kind.label_key())
        };
        rows.push(
            div()
                .id(("version-row", index))
                .flex()
                .flex_col()
                .gap_1()
                .p_2()
                .rounded_md()
                .border_1()
                .border_color(theme::border())
                .child(div().text_sm().child(label(version)))
                .child(
                    div()
                        .text_color(theme::text_dim())
                        .child(format!("{note} · {} KB", version.size.div_ceil(1024))),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            button(("version-open", index), i18n::text("versions.open"), false)
                                .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                                    open(view, index, cx)
                                })),
                        )
                        .when(index > 0, |buttons| {
                            buttons.child(
                                button(
                                    ("version-restore", index),
                                    i18n::text("versions.restore"),
                                    false,
                                )
                                .on_click(cx.listener(
                                    move |view, _: &ClickEvent, _, cx| restore(view, index, cx),
                                )),
                            )
                        }),
                )
                .into_any_element(),
        );
    }
    Some(
        div()
            .id("versions-pane")
            .flex()
            .flex_col()
            .flex_none()
            .w(px(260.0))
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
                    .child(div().text_sm().child(i18n::text("versions.title")))
                    .child(
                        div()
                            .id("versions-close")
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

#[cfg(test)]
mod tests {
    use super::*;

    fn open_view<'a>(
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

    /// 自動保存 on: an edit is saved to the file once the typing stops, not
    /// while a dialog is open; each save is a version, the file as opened
    /// kept first; 復元 puts an earlier one back and saves it, 開く gives
    /// a version as a document of its own.
    #[gpui::test]
    fn autosave_saves_after_edits_and_versions_restore(cx: &mut gpui::TestAppContext) {
        let fixture = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../jade-core/contract.docx"
        ));
        let path = crate::shell::scratch_copy(&fixture, "autosave.docx");
        let root = path.with_file_name("Versions");
        std::fs::remove_dir_all(&root).ok();
        set_root_for_tests(Some(root.clone()));
        let original = std::fs::read(&path).unwrap();
        let (view, cx) = open_view(&path, cx);
        view.update(cx, |view, cx| {
            assert!(!view.versions.auto.on, "off unless the preference says");
            toggle(view, cx);
            assert!(view.versions.auto.on);
            view.insert_at = (0, 0);
            view.caret = view.document.place_caret(0, 0);
        });
        cx.simulate_input("自動保存");
        view.update(cx, |view, cx| {
            assert!(view.dirty);
            // Straight after the edit: not yet.
            tick(view, cx);
            assert!(view.dirty);
            // A dialog open holds it however long.
            view.show_message("t", "b", cx);
            view.versions.clock -= Duration::from_secs(5);
            tick(view, cx);
            assert!(view.dirty, "no save while a dialog is open");
            view.files.dialog = None;
            tick(view, cx);
            assert!(!view.dirty, "saved once the dialog closed");
        });
        let saved = std::fs::read(&path).unwrap();
        assert_ne!(saved, original);
        let store = store().unwrap();
        let versions = store.list(&path);
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].kind, VersionKind::AutoSaved);
        assert_eq!(versions[1].kind, VersionKind::Original);
        assert_eq!(store.read(&versions[1]).unwrap(), original);
        assert!(
            versions[0].file.starts_with(&root),
            "kept in the app's folder"
        );

        // The history: 開く gives the version as a document of its own,
        // 復元 puts the original back and saves it.
        cx.dispatch_action(VersionHistory);
        view.update(cx, |view, cx| {
            assert_eq!(view.versions.pane.as_ref().unwrap().len(), 2);
            let (name, bytes) = opened(view, 1).unwrap();
            assert!(
                name.starts_with("autosave (") && name.ends_with(").docx"),
                "{name}"
            );
            assert_eq!(bytes, original);
            assert!(text_of(view).contains("自動保存"));
            restore(view, 1, cx);
            assert!(!text_of(view).contains("自動保存"));
            assert!(!view.dirty);
            let pane = view.versions.pane.as_ref().unwrap();
            assert_eq!(pane.len(), 3);
            assert_eq!(pane[0].kind, VersionKind::Restored);
            toggle_pane(view, cx);
            assert!(view.versions.pane.is_none());
        });
        set_root_for_tests(None);
        std::fs::remove_dir_all(root).ok();
    }

    /// The switch on a new document asks where to save it first; on a
    /// format other than Word's own it stays off and says why.
    #[gpui::test]
    fn autosave_needs_a_word_file(cx: &mut gpui::TestAppContext) {
        let fixture = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../jade-core/contract.docx"
        ));
        let path = crate::shell::scratch_copy(&fixture, "autosave-format.docx");
        let (view, cx) = open_view(&path, cx);
        view.update(cx, |view, cx| {
            view.files.format = jade_office_model::word::formats::WordFormat::Rtf;
            toggle(view, cx);
            assert!(!view.versions.auto.on);
            assert!(view.files.dialog.is_some(), "says why");
        });
    }
}

//! The browser shell's copy of the native app's `word_references`: Word's
//! 参考資料 tab beyond inserting a note and a table of contents, and
//! the ways around a document, over the shared commands in
//! `jade-office-model` (`word::references`):
//!
//! - 脚注と文末脚注 (place, number format, start, restart), 変換, 次の脚注
//!   and 注の表示 (`notes`),
//! - the 目次 and 図表目次 dialogs, 手動作成目次 and 目次の更新 (`toc`),
//! - ブックマーク, 相互参照 and 図表番号の挿入 (`marks`),
//! - 資料文献の管理, 引用文献の挿入, スタイル and 文献目録 (`citations`),
//! - 索引登録, 索引, 引用文の登録 and 引用文献一覧 (`index`),
//! - the ナビゲーション pane, 見出しの折りたたみ, ジャンプ and 次/前のコメ
//!   ント (`navigation`).
//!
//! One dialog is open at a time, over the window; while it is, it takes the
//! keyboard — typing goes into its focused field, not the document.

mod citations;
mod index;
mod marks;
mod navigation;
mod notes;
mod toc;

use std::collections::BTreeSet;

use gpui::{
    anchored, deferred, div, point, prelude::*, px, AnyElement, ClickEvent, Context, ElementId,
    MouseButton, SharedString, Window,
};
use jade_web::text::TextEdit;
use jade_web::word::references::bookmarks::BookmarkDialog;
use jade_web::word::references::captions::CaptionDialog;
use jade_web::word::references::citations::{BibliographyStyle, Source};
use jade_web::word::references::crossref::CrossReferenceDialog;
use jade_web::word::references::index::{
    AuthoritiesDialog, IndexDialog, MarkCitationDialog, MarkEntryDialog,
};
use jade_web::word::references::navigation::GoToTarget;
use jade_web::word::references::notes::{NoteConversion, NoteDialog};
use jade_web::word::references::toc::TocDialog;
use jade_web::word::NoteKind;

use crate::ribbon::{dropdown, group, listener, menu_panel, menu_row, row, Action, Popup};
use crate::tooltip::Tooltip;
use crate::view::word_characters::{button, check};
use crate::view::DocView as DocumentView;
use crate::{i18n, palette as theme};

pub use navigation::{bookmark_marks, navigation_pane, page_marks, refresh_collapsed};

/// The lists the 参考資料 tab drops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefPopup {
    /// 目次.
    Contents,
    /// 次の脚注.
    NextNote,
    /// 引用文献の挿入.
    Citation,
    /// スタイル.
    Style,
    /// 文献目録.
    Bibliography,
}

/// The 目次 gallery's entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TocPreset {
    /// 自動作成の目次 1: levels 1–3 with a dot leader.
    Automatic1,
    /// 自動作成の目次 2: the same, under 目次.
    Automatic2,
    /// 手動作成目次.
    Manual,
}

/// What the tab's controls do.
#[derive(Clone, Debug, PartialEq)]
pub enum RefAction {
    Toc(TocPreset),
    /// ユーザー設定の目次….
    OpenToc,
    /// 目次の更新: the dialog asking what to update.
    OpenUpdateToc,
    /// 脚注と文末脚注….
    OpenNotes,
    /// 次の脚注 / 前の脚注 / 次の文末脚注 / 前の文末脚注.
    StepNote(NoteKind, bool),
    /// 注の表示.
    ShowNotes,
    /// 引用文献の挿入's sources.
    InsertCitation(String),
    /// 新しい資料文献の追加….
    NewSource,
    /// 資料文献の管理….
    OpenSources,
    SetStyle(BibliographyStyle),
    /// 文献目録's 参考文献, 引用文献 (under a heading) or 文献目録.
    InsertBibliography(Option<&'static str>),
    /// 図表番号の挿入….
    OpenCaption,
    /// 図表目次の挿入….
    OpenFigures,
    /// 図表目次の更新.
    UpdateFigures,
    /// 相互参照….
    OpenCrossReference,
    /// ブックマーク….
    OpenBookmark,
    /// 索引登録….
    OpenMarkEntry,
    /// 索引の挿入….
    OpenIndex,
    /// 索引の更新.
    UpdateIndex,
    /// 引用文の登録….
    OpenMarkCitation,
    /// 引用文献一覧の挿入….
    OpenAuthorities,
    /// 引用文献一覧の更新.
    UpdateAuthorities,
    /// ナビゲーション ウィンドウ.
    ToggleNavigation,
    /// ブックマーク表示.
    ToggleShowBookmarks,
    /// ジャンプ….
    OpenGoTo,
    /// 次のコメント / 前のコメント.
    StepComment(bool),
}

/// The open dialog.
pub enum Dialog {
    Notes(NoteDialog),
    /// 変換…, from 脚注と文末脚注.
    Convert(NoteConversion, NoteDialog),
    /// 目次の更新: page numbers only, or the whole table.
    UpdateToc(bool),
    /// 目次 or 図表目次; with its オプション open.
    Toc(Box<TocDialog>, bool),
    Bookmark(BookmarkDialog),
    CrossReference(CrossReferenceDialog),
    /// 図表番号; with ラベル名 being typed for a new label.
    Caption(CaptionDialog, bool),
    /// 資料文献の管理: the picked source in the master list and the
    /// current one.
    Sources(Option<usize>, Option<usize>),
    /// 資料文献の作成 / 編集: the source edited (by tag) and its type, and
    /// whether to go back to 資料文献の管理 and into the master list.
    SourceEdit {
        editing: Option<String>,
        source_type: usize,
        back_to_sources: bool,
        cite: bool,
    },
    MarkEntry(MarkEntryDialog),
    Index(IndexDialog),
    MarkCitation(MarkCitationDialog),
    Authorities(AuthoritiesDialog),
    GoTo(GoToTarget),
}

/// What the dialogs' lists show while one is dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefList {
    NoteFormat,
    Place,
    Restart,
    Leader,
    Levels,
    CaptionLabel,
    CaptionFormat,
    ChapterLevel,
    Separator,
    ReferenceType,
    Category,
    Bookmark,
    SourceType,
}

/// The pane's tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavTab {
    Headings,
    Pages,
    Results,
}

/// The view's share of this.
pub struct References {
    pub dialog: Option<Dialog>,
    /// The dialog's text fields, and which has the keyboard.
    pub fields: Vec<TextEdit>,
    pub focus: usize,
    pub list: Option<RefList>,
    /// Why OK did nothing.
    pub problem: Option<String>,
    /// 図表番号's labels the user added (ラベル名).
    pub custom_labels: Vec<String>,
    /// 資料文献の管理's マスター リスト, read from beside the app once.
    pub master: Option<Vec<Source>>,
    /// The navigation pane.
    pub navigation: bool,
    pub nav_tab: NavTab,
    pub query: TextEdit,
    /// Whether typing goes into the pane's search box.
    pub searching: bool,
    /// The headings collapsed in the body, by paragraph.
    pub collapsed: BTreeSet<usize>,
    /// The headings folded in the pane's outline, by paragraph.
    pub pane_folded: BTreeSet<usize>,
    /// ブックマーク表示.
    pub show_bookmarks: bool,
    /// Where typing goes while a dialog without fields is open: nowhere.
    sink: TextEdit,
}

impl Default for References {
    fn default() -> Self {
        Self {
            dialog: None,
            fields: Vec::new(),
            focus: 0,
            list: None,
            problem: None,
            custom_labels: Vec::new(),
            master: None,
            navigation: false,
            nav_tab: NavTab::Headings,
            query: TextEdit::new(String::new()),
            searching: false,
            collapsed: BTreeSet::new(),
            pane_folded: BTreeSet::new(),
            show_bookmarks: false,
            sink: TextEdit::new(String::new()),
        }
    }
}

impl References {
    pub fn is_open(&self) -> bool {
        self.dialog.is_some()
    }

    /// The field typing goes into: the dialog's focused one, or the pane's
    /// search box.
    pub fn text_field(&self) -> Option<&TextEdit> {
        if self.dialog.is_some() {
            return self.fields.get(self.focus);
        }
        self.searching.then_some(&self.query)
    }

    pub fn text_field_mut(&mut self) -> Option<&mut TextEdit> {
        if self.dialog.is_some() {
            if self.focus >= self.fields.len() {
                self.sink = TextEdit::new(String::new());
                return Some(&mut self.sink);
            }
            return self.fields.get_mut(self.focus);
        }
        if self.searching {
            return Some(&mut self.query);
        }
        None
    }

    /// Opens a dialog with its fields' first text.
    fn open(&mut self, dialog: Dialog, fields: Vec<String>) {
        self.dialog = Some(dialog);
        self.fields = fields.into_iter().map(TextEdit::new).collect();
        for field in &mut self.fields {
            field.whole = !field.text.is_empty();
        }
        self.focus = 0;
        self.list = None;
        self.problem = None;
        self.searching = false;
    }

    pub fn close(&mut self) {
        self.dialog = None;
        self.fields.clear();
        self.focus = 0;
        self.list = None;
        self.problem = None;
    }

    /// A field's text.
    pub fn field(&self, index: usize) -> &str {
        self.fields
            .get(index)
            .map_or("", |field| field.text.as_str())
    }

    /// The master list, read from beside the app the first time.
    pub fn master(&mut self) -> &mut Vec<Source> {
        self.master.get_or_insert_with(|| {
            std::fs::read_to_string(master_list_path())
                .map(|xml| jade_web::word::references::citations::parse_master_list(&xml))
                .unwrap_or_default()
        })
    }

    /// Writes the master list back.
    pub fn save_master(&self) {
        if let Some(sources) = &self.master {
            let path = master_list_path();
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(
                path,
                jade_web::word::references::citations::master_list_xml(sources),
            );
        }
    }
}

/// Where the master list is kept: beside the app's other settings, as Word
/// keeps `Sources.xml`.
fn master_list_path() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("JADE_SOURCES_XML") {
        return std::path::PathBuf::from(path);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home)
        .join("Library/Application Support/jade-office")
        .join("Sources.xml")
}

pub(crate) fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

/// Runs a 参考資料 action.
pub fn run(view: &mut DocumentView, action: RefAction, cx: &mut Context<DocumentView>) {
    if view.word_ref().is_none() {
        return;
    }
    match action {
        RefAction::Toc(preset) => toc::insert_preset(view, preset, cx),
        RefAction::OpenToc => toc::open(view, None, cx),
        RefAction::OpenUpdateToc => toc::open_update(view, cx),
        RefAction::OpenFigures => {
            let label = view
                .references
                .custom_labels
                .first()
                .cloned()
                .unwrap_or_else(|| "図".to_string());
            toc::open(view, Some(label), cx)
        }
        RefAction::UpdateFigures => toc::update_figures(view, cx),
        RefAction::OpenNotes => notes::open(view, NoteKind::Footnote, cx),
        RefAction::StepNote(kind, forwards) => notes::step(view, kind, forwards, cx),
        RefAction::ShowNotes => notes::show(view, cx),
        RefAction::InsertCitation(tag) => citations::insert_citation(view, &tag, cx),
        RefAction::NewSource => citations::open_new_source(view, false, true, cx),
        RefAction::OpenSources => citations::open_sources(view, cx),
        RefAction::SetStyle(style) => citations::set_style(view, style, cx),
        RefAction::InsertBibliography(heading) => citations::insert_bibliography(view, heading, cx),
        RefAction::OpenCaption => marks::open_caption(view, cx),
        RefAction::OpenCrossReference => marks::open_cross_reference(view, cx),
        RefAction::OpenBookmark => marks::open_bookmark(view, cx),
        RefAction::OpenMarkEntry => index::open_mark_entry(view, cx),
        RefAction::OpenIndex => index::open_index(view, cx),
        RefAction::UpdateIndex => index::update_index(view, cx),
        RefAction::OpenMarkCitation => index::open_mark_citation(view, cx),
        RefAction::OpenAuthorities => index::open_authorities(view, cx),
        RefAction::UpdateAuthorities => index::update_authorities(view, cx),
        RefAction::ToggleNavigation => navigation::toggle(view, cx),
        RefAction::ToggleShowBookmarks => {
            view.references.show_bookmarks = !view.references.show_bookmarks;
            cx.notify();
        }
        RefAction::OpenGoTo => navigation::open_go_to(view, cx),
        RefAction::StepComment(forwards) => navigation::step_comment(view, forwards, cx),
    }
}

/// Nothing to hand over: the browser's document holds its own selection.
pub(crate) fn sync_selection(_view: &mut DocumentView) {}

impl DocumentView {
    /// The Word document. The 参考資料 tab and its dialogs exist only over
    /// one, so there always is one here.
    pub(crate) fn doc(&self) -> &jade_web::editing::WordDoc {
        self.word_ref().expect("a Word document")
    }

    pub(crate) fn doc_mut(&mut self) -> &mut jade_web::editing::WordDoc {
        self.word_mut().expect("a Word document")
    }

    /// Where the caret is.
    pub(crate) fn caret_at(&self) -> (usize, usize) {
        self.word_ref()
            .and_then(|document| document.caret())
            .unwrap_or((0, 0))
    }

    pub(crate) fn set_status(&mut self, status: SharedString) {
        self.status = status;
    }

    /// The paragraph of the picture or shape held, when one is.
    pub(crate) fn held_object_paragraph(&self) -> Option<usize> {
        match &self.document {
            super::Document::Pages { object, .. } => {
                object.map(|grab| grab.object.target.paragraph_index)
            }
            _ => None,
        }
    }

    /// Lets go of the picture or shape held.
    pub(crate) fn drop_held_object(&mut self) {
        if let super::Document::Pages { object, .. } = &mut self.document {
            *object = None;
        }
    }

    /// Opens the comments pane.
    pub(crate) fn show_comments_pane(&mut self) {
        if let super::Document::Pages { show_comments, .. } = &mut self.document {
            *show_comments = true;
        }
    }

    /// Scrolls to a page (0-based).
    pub(crate) fn show_page(&mut self, index: usize) {
        if let super::Document::Pages {
            scroll, current, ..
        } = &mut self.document
        {
            *current = index;
            scroll.scroll_to_item(index);
        }
    }

    /// The page on show (0-based).
    pub(crate) fn page_on_show(&self) -> usize {
        match &self.document {
            super::Document::Pages { current, .. } => *current,
            _ => 0,
        }
    }

    /// After a reference command: the pages repainted and the caret where
    /// the command left it, or why nothing changed.
    pub(crate) fn after_reference_edit(
        &mut self,
        done: Result<Option<(usize, usize)>, String>,
        cx: &mut Context<DocumentView>,
    ) {
        if let Ok(Some((paragraph, offset))) = done {
            self.doc_mut().place_caret(paragraph, offset);
        }
        navigation::refresh_collapsed(self);
        self.refresh_repainted();
        self.reveal_caret();
        self.word_result(done.map(|_| ()), cx);
    }

    /// The caret to a place, or a selection over a range, scrolled into
    /// view.
    pub(crate) fn reveal(
        &mut self,
        start: (usize, usize),
        end: (usize, usize),
        cx: &mut Context<DocumentView>,
    ) {
        navigation::expand_to(self, start.0);
        let moved = if start == end {
            self.doc_mut().place_caret(end.0, end.1);
            Ok(())
        } else {
            self.doc_mut().select(start, end)
        };
        if let Err(error) = moved {
            self.status = error.into();
        }
        self.refresh_repainted();
        self.reveal_caret();
        cx.notify();
    }
}

/// A key while a dialog is open; `true` when it was taken.
pub fn handle_key(
    view: &mut DocumentView,
    key: &str,
    shift: bool,
    cx: &mut Context<DocumentView>,
) -> bool {
    let references = &mut view.references;
    if references.dialog.is_none() {
        if references.searching {
            return navigation::search_key(view, key, cx);
        }
        return false;
    }
    if references
        .fields
        .get(references.focus)
        .is_some_and(|field| field.marked.is_some())
    {
        return false;
    }
    match key {
        "escape" => {
            if references.list.take().is_none() {
                cancel(view);
            }
        }
        "enter" | "return" => {
            references.list = None;
            confirm(view, cx);
            return true;
        }
        "tab" => {
            if !references.fields.is_empty() {
                let count = references.fields.len();
                references.focus = if shift {
                    (references.focus + count - 1) % count
                } else {
                    (references.focus + 1) % count
                };
                if let Some(field) = references.fields.get_mut(references.focus) {
                    field.whole = !field.text.is_empty();
                }
            }
        }
        _ => {
            let edited = references
                .fields
                .get_mut(references.focus)
                .is_some_and(|field| field.edit_key(key));
            if !edited {
                // The dialog is modal: the document gets nothing.
                return !matches!(key, "space") && key.chars().count() > 1;
            }
        }
    }
    cx.notify();
    true
}

/// Escape or キャンセル: nothing changes; a dialog opened from another goes
/// back to it.
pub fn cancel(view: &mut DocumentView) {
    match view.references.dialog.take() {
        Some(Dialog::Convert(_, notes_dialog)) => {
            view.references
                .open(Dialog::Notes(notes_dialog), Vec::new());
        }
        Some(Dialog::SourceEdit {
            back_to_sources: true,
            ..
        }) => {
            view.references
                .open(Dialog::Sources(None, None), Vec::new());
        }
        Some(Dialog::Toc(dialog, true)) => {
            view.references.open(Dialog::Toc(dialog, false), Vec::new());
        }
        Some(Dialog::Caption(dialog, true)) => {
            let text = view.references.field(0).to_string();
            view.references
                .open(Dialog::Caption(dialog, false), vec![text]);
        }
        _ => view.references.close(),
    }
}

/// OK: what the dialog says lands on the document.
pub fn confirm(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let Some(dialog) = view.references.dialog.take() else {
        return;
    };
    sync_selection(view);
    match dialog {
        Dialog::Notes(dialog) => notes::confirm(view, dialog, false, cx),
        Dialog::Convert(conversion, back) => notes::confirm_convert(view, conversion, back, cx),
        Dialog::UpdateToc(page_numbers_only) => toc::confirm_update(view, page_numbers_only, cx),
        Dialog::Toc(dialog, options) => toc::confirm(view, *dialog, options, cx),
        Dialog::Bookmark(dialog) => marks::confirm_bookmark(view, dialog, cx),
        Dialog::CrossReference(dialog) => marks::confirm_cross_reference(view, dialog, cx),
        Dialog::Caption(dialog, new_label) => marks::confirm_caption(view, dialog, new_label, cx),
        Dialog::Sources(..) => {
            view.references.save_master();
            view.references.close();
            cx.notify();
        }
        Dialog::SourceEdit {
            editing,
            source_type,
            back_to_sources,
            cite,
        } => citations::confirm_source(view, editing, source_type, back_to_sources, cite, cx),
        Dialog::MarkEntry(dialog) => index::confirm_mark_entry(view, dialog, false, cx),
        Dialog::Index(dialog) => index::confirm_index(view, dialog, cx),
        Dialog::MarkCitation(dialog) => index::confirm_mark_citation(view, dialog, cx),
        Dialog::Authorities(dialog) => index::confirm_authorities(view, dialog, cx),
        Dialog::GoTo(target) => navigation::confirm_go_to(view, target, cx),
    }
}

/// Edits the open dialog from a click on one of its controls.
pub(crate) fn edit(
    view: &mut DocumentView,
    cx: &mut Context<DocumentView>,
    change: impl FnOnce(&mut References),
) {
    change(&mut view.references);
    view.references.problem = None;
    cx.notify();
}

/// The open dialog, over the window.
pub fn overlay(
    view: &mut DocumentView,
    window: &mut Window,
    cx: &mut Context<DocumentView>,
) -> Option<AnyElement> {
    let (title, body, ok): (&'static str, AnyElement, &'static str) =
        match view.references.dialog.as_ref()? {
            Dialog::Notes(_) => ("notes.title", notes::body(view, cx), "notes.apply"),
            Dialog::Convert(..) => (
                "notes.convert.title",
                notes::convert_body(view, cx),
                "format.ok",
            ),
            Dialog::UpdateToc(_) => ("toc.update.title", toc::update_body(view, cx), "format.ok"),
            Dialog::Toc(dialog, options) => (
                if *options {
                    "toc.options.title"
                } else if matches!(
                    dialog.kind,
                    jade_web::word::references::toc::TocKind::Figures { .. }
                ) {
                    "toc.figures.title"
                } else {
                    "toc.title"
                },
                toc::body(view, cx),
                "format.ok",
            ),
            Dialog::Bookmark(_) => (
                "bookmark.title",
                marks::bookmark_body(view, cx),
                "bookmark.add",
            ),
            Dialog::CrossReference(_) => (
                "xref.title",
                marks::cross_reference_body(view, cx),
                "xref.insert",
            ),
            Dialog::Caption(_, new_label) => (
                if *new_label {
                    "caption.new_label.title"
                } else {
                    "caption.title"
                },
                marks::caption_body(view, cx),
                "format.ok",
            ),
            Dialog::Sources(..) => (
                "sources.title",
                citations::sources_body(view, cx),
                "sources.close",
            ),
            Dialog::SourceEdit { editing, .. } => (
                if editing.is_some() {
                    "sources.edit.title"
                } else {
                    "sources.new.title"
                },
                citations::source_edit_body(view, cx),
                "format.ok",
            ),
            Dialog::MarkEntry(_) => (
                "index.mark.title",
                index::mark_entry_body(view, cx),
                "index.mark",
            ),
            Dialog::Index(_) => ("index.title", index::index_body(view, cx), "format.ok"),
            Dialog::MarkCitation(_) => (
                "toa.mark.title",
                index::mark_citation_body(view, cx),
                "toa.mark",
            ),
            Dialog::Authorities(_) => ("toa.title", index::authorities_body(view, cx), "format.ok"),
            Dialog::GoTo(_) => ("goto.title", navigation::go_to_body(view, cx), "goto.go"),
        };
    let problem = view.references.problem.clone();
    let viewport = window.viewport_size();
    Some(
        deferred(
            anchored().position(point(px(0.0), px(0.0))).child(
                div()
                    .id("ref-dialog-backdrop")
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
                            .id("ref-dialog")
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
                            .font_family(crate::UI_FONT)
                            .text_xs()
                            .text_color(theme::text())
                            .child(div().text_sm().child(t(title)))
                            .child(body)
                            .children(problem.map(|problem| {
                                div().text_color(gpui::rgb(0xD13438)).child(problem)
                            }))
                            .child(
                                div()
                                    .flex()
                                    .justify_end()
                                    .gap_2()
                                    .child(
                                        button("ref-cancel", t("format.cancel"), false).on_click(
                                            cx.listener(|view, _: &ClickEvent, _, cx| {
                                                cancel(view);
                                                cx.notify();
                                            }),
                                        ),
                                    )
                                    .child(button("ref-ok", t(ok), true).on_click(cx.listener(
                                        |view, _: &ClickEvent, _, cx| confirm(view, cx),
                                    ))),
                            ),
                    ),
            ),
        )
        .with_priority(3)
        .into_any_element(),
    )
}

// ---- the dialogs' controls ----------------------------------------------

/// A labelled one-line field of the open dialog; a click gives it the
/// keyboard.
pub(crate) fn text_field(
    view: &DocumentView,
    index: usize,
    label: &'static str,
    width: f32,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    let focused = view.references.focus == index && view.references.dialog.is_some();
    let empty = TextEdit::new(String::new());
    let field = view.references.fields.get(index).unwrap_or(&empty);
    let (before, after) = field.split();
    div()
        .flex()
        .items_center()
        .gap_2()
        .when(!label.is_empty(), |line| {
            line.child(
                div()
                    .w(px(120.0))
                    .flex_none()
                    .text_color(theme::dim())
                    .child(t(label)),
            )
        })
        .child(
            div()
                .id(("ref-field", index))
                .flex()
                .items_center()
                .w(px(width))
                .h(px(22.0))
                .px_2()
                .rounded_sm()
                .bg(theme::canvas())
                .border_1()
                .border_color(if focused {
                    theme::accent()
                } else {
                    theme::border()
                })
                .overflow_hidden()
                .whitespace_nowrap()
                .cursor_text()
                .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                    view.references.focus = index;
                    view.references.list = None;
                    cx.notify();
                }))
                .when(focused && field.whole, |row| {
                    row.child(div().bg(theme::selected()).child(field.text.clone()))
                        .child(div().w(px(1.0)).h(px(14.0)).bg(theme::accent()))
                })
                .when(!(focused && field.whole), |row| {
                    row.child(before.to_string())
                        .when(focused, |row| {
                            row.child(div().w(px(1.0)).h(px(14.0)).bg(theme::accent()))
                        })
                        .child(after.to_string())
                }),
        )
        .into_any_element()
}

/// A choice in a row of choices.
pub(crate) fn choice(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    on: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_1()
        .cursor_pointer()
        .child(
            div()
                .size(px(12.0))
                .rounded_full()
                .border_1()
                .border_color(if on { theme::accent() } else { theme::border() })
                .when(on, |dot| dot.bg(theme::accent())),
        )
        .child(label.into())
}

/// A labelled line of controls.
pub(crate) fn line(label: &'static str) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap_3()
        .when(!label.is_empty(), |line| {
            line.child(
                div()
                    .w(px(120.0))
                    .flex_none()
                    .text_color(theme::dim())
                    .child(t(label)),
            )
        })
}

/// A section heading inside a dialog.
pub(crate) fn heading(label: &'static str) -> gpui::Div {
    div()
        .pt_1()
        .text_color(theme::faint())
        .border_b_1()
        .border_color(theme::border())
        .child(t(label))
}

/// A drop-down select of the open dialog: its value, and the list under it
/// while open.
pub(crate) fn select(
    view: &DocumentView,
    list: RefList,
    value: impl Into<SharedString>,
    width: f32,
    entries: Vec<(SharedString, bool)>,
    cx: &mut Context<DocumentView>,
    pick: impl Fn(&mut References, usize) + Clone + 'static,
) -> AnyElement {
    let open = view.references.list == Some(list);
    let trigger = div()
        .id(SharedString::from(format!("ref-select-{list:?}")))
        .flex()
        .items_center()
        .justify_between()
        .w(px(width))
        .h(px(22.0))
        .px_2()
        .rounded_sm()
        .border_1()
        .border_color(if open {
            theme::accent()
        } else {
            theme::border()
        })
        .bg(theme::canvas())
        .cursor_pointer()
        .overflow_hidden()
        .whitespace_nowrap()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .child(value.into()),
        )
        .child(
            gpui::svg()
                .path("icons/chevron-down.svg")
                .size_3()
                .flex_none()
                .text_color(theme::dim()),
        )
        .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
            let references = &mut view.references;
            references.list = (references.list != Some(list)).then_some(list);
            cx.notify();
        }));
    let menu = open.then(|| {
        crate::view::word_characters::list_panel("ref-select-list", width.max(160.0))
            .children(entries.into_iter().enumerate().map(|(index, (label, on))| {
                let pick = pick.clone();
                crate::view::word_characters::list_row(("ref-select-row", index), label, on)
                    .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                        pick(&mut view.references, index);
                        view.references.list = None;
                        view.references.problem = None;
                        cx.notify();
                    }))
                    .into_any_element()
            }))
            .into_any_element()
    });
    crate::view::word_characters::with_list(trigger, menu)
}

/// A list box of the open dialog: rows to pick one of.
pub(crate) fn list_box(
    id: &'static str,
    width: f32,
    height: f32,
    rows: Vec<(SharedString, bool)>,
    cx: &mut Context<DocumentView>,
    pick: impl Fn(&mut DocumentView, usize, &mut Context<DocumentView>) + Clone + 'static,
) -> AnyElement {
    div()
        .id(id)
        .w(px(width))
        .h(px(height))
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .p_0p5()
        .rounded_sm()
        .border_1()
        .border_color(theme::border())
        .bg(theme::canvas())
        .children(rows.into_iter().enumerate().map(|(index, (label, on))| {
            let pick = pick.clone();
            div()
                .id((id, index))
                .px_1()
                .h(px(20.0))
                .flex()
                .items_center()
                .flex_none()
                .whitespace_nowrap()
                .overflow_hidden()
                .rounded_sm()
                .cursor_pointer()
                .when(on, |row| row.bg(theme::selected()))
                .hover(|row| row.bg(theme::hovered()))
                .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                    pick(view, index, cx);
                    cx.notify();
                }))
                .child(label)
                .into_any_element()
        }))
        .into_any_element()
}

/// A push button inside a dialog that runs `act`.
pub(crate) fn action_button(
    id: &'static str,
    label: &'static str,
    cx: &mut Context<DocumentView>,
    act: impl Fn(&mut DocumentView, &mut Context<DocumentView>) + 'static,
) -> AnyElement {
    button(id, t(label), false)
        .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| act(view, cx)))
        .into_any_element()
}

/// A check box inside a dialog.
pub(crate) fn check_box(
    id: &'static str,
    label: &'static str,
    on: bool,
    cx: &mut Context<DocumentView>,
    toggle: impl Fn(&mut References) + 'static,
) -> AnyElement {
    check(id, t(label), on)
        .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
            edit(view, cx, |references| toggle(references))
        }))
        .into_any_element()
}

// ---- the ribbon ---------------------------------------------------------

fn text_button(
    id: &'static str,
    label: &'static str,
    tooltip: &'static str,
    active: bool,
    action: RefAction,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .h(px(26.0))
        .px_2()
        .rounded_md()
        .text_xs()
        .text_color(theme::text())
        .whitespace_nowrap()
        .cursor_pointer()
        .when(active, |button| button.bg(theme::selected()))
        .hover(|button| button.bg(theme::hovered()))
        .tooltip(Tooltip::text(tooltip))
        .on_click(listener(Action::References(action), cx))
        .child(label)
        .into_any_element()
}

fn list_button(
    id: &'static str,
    label: impl Into<SharedString>,
    popup: RefPopup,
    open: bool,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap_1()
        .h(px(26.0))
        .px_2()
        .rounded_md()
        .text_xs()
        .text_color(theme::text())
        .whitespace_nowrap()
        .cursor_pointer()
        .when(open, |button| button.bg(theme::selected()))
        .hover(|button| button.bg(theme::hovered()))
        .on_click(listener(Action::Popup(Popup::References(popup)), cx))
        .child(label.into())
        .child(
            gpui::svg()
                .path("icons/chevron-down.svg")
                .size_3()
                .text_color(theme::dim()),
        )
        .into_any_element()
}

fn item(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    checked: bool,
    action: RefAction,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    menu_row(id, label, checked, Action::References(action), cx)
}

/// What the 参考資料 tab shows, read while it is up.
#[derive(Clone, Debug, Default)]
pub struct RibbonState {
    pub sources: Vec<Source>,
    pub style: Option<BibliographyStyle>,
    pub navigation: bool,
    pub show_bookmarks: bool,
}

/// Reads what the tab shows.
pub fn ribbon_state(
    document: &jade_web::editing::EditableDocument,
    references: &References,
) -> RibbonState {
    use jade_web::word::references::citations::WordCitations;
    RibbonState {
        sources: document.sources(),
        style: Some(document.bibliography_style()),
        navigation: references.navigation,
        show_bookmarks: references.show_bookmarks,
    }
}

/// The 参考資料 tab's groups.
pub fn ribbon_groups(
    popup: Option<Popup>,
    shown: Option<&RibbonState>,
    cx: &mut Context<DocumentView>,
) -> Vec<AnyElement> {
    let shown = shown.cloned().unwrap_or_default();
    let open = match popup {
        Some(Popup::References(popup)) => Some(popup),
        _ => None,
    };
    let contents_menu = (open == Some(RefPopup::Contents)).then(|| {
        menu_panel("ref-contents-menu")
            .w(px(220.0))
            .child(item(
                "toc-automatic-1",
                t("toc.automatic1"),
                false,
                RefAction::Toc(TocPreset::Automatic1),
                cx,
            ))
            .child(item(
                "toc-automatic-2",
                t("toc.automatic2"),
                false,
                RefAction::Toc(TocPreset::Automatic2),
                cx,
            ))
            .child(item(
                "toc-manual",
                t("toc.manual"),
                false,
                RefAction::Toc(TocPreset::Manual),
                cx,
            ))
            .child(item(
                "toc-custom",
                t("toc.custom"),
                false,
                RefAction::OpenToc,
                cx,
            ))
            .into_any_element()
    });
    let note_menu = (open == Some(RefPopup::NextNote)).then(|| {
        menu_panel("ref-next-note-menu")
            .w(px(180.0))
            .child(item(
                "next-footnote",
                t("notes.next_footnote"),
                false,
                RefAction::StepNote(NoteKind::Footnote, true),
                cx,
            ))
            .child(item(
                "previous-footnote",
                t("notes.previous_footnote"),
                false,
                RefAction::StepNote(NoteKind::Footnote, false),
                cx,
            ))
            .child(item(
                "next-endnote",
                t("notes.next_endnote"),
                false,
                RefAction::StepNote(NoteKind::Endnote, true),
                cx,
            ))
            .child(item(
                "previous-endnote",
                t("notes.previous_endnote"),
                false,
                RefAction::StepNote(NoteKind::Endnote, false),
                cx,
            ))
            .into_any_element()
    });
    let style = shown.style.unwrap_or(BibliographyStyle::Apa);
    let citation_menu = (open == Some(RefPopup::Citation)).then(|| {
        let sources = shown.sources.clone();
        menu_panel("ref-citation-menu")
            .w(px(280.0))
            .children(sources.into_iter().enumerate().map(|(index, source)| {
                let label = format!("{} — {}", style.citation(&source, index + 1), source.title);
                item(
                    ("cite-source", index),
                    label,
                    false,
                    RefAction::InsertCitation(source.tag),
                    cx,
                )
            }))
            .child(item(
                "cite-new",
                t("sources.new"),
                false,
                RefAction::NewSource,
                cx,
            ))
            .into_any_element()
    });
    let style_menu = (open == Some(RefPopup::Style)).then(|| {
        menu_panel("ref-style-menu")
            .w(px(260.0))
            .children(
                BibliographyStyle::ALL
                    .into_iter()
                    .enumerate()
                    .map(|(index, entry)| {
                        item(
                            ("bib-style", index),
                            entry.name(),
                            entry == style,
                            RefAction::SetStyle(entry),
                            cx,
                        )
                    }),
            )
            .into_any_element()
    });
    let bibliography_menu = (open == Some(RefPopup::Bibliography)).then(|| {
        menu_panel("ref-bibliography-menu")
            .w(px(200.0))
            .child(item(
                "bib-references",
                t("bibliography.references"),
                false,
                RefAction::InsertBibliography(Some("参考文献")),
                cx,
            ))
            .child(item(
                "bib-works-cited",
                t("bibliography.works_cited"),
                false,
                RefAction::InsertBibliography(Some("引用文献")),
                cx,
            ))
            .child(item(
                "bib-plain",
                t("bibliography.insert"),
                false,
                RefAction::InsertBibliography(None),
                cx,
            ))
            .into_any_element()
    });
    vec![
        group(
            t("doc.toc"),
            vec![
                row(vec![dropdown(
                    list_button(
                        "ref-contents",
                        t("doc.toc"),
                        RefPopup::Contents,
                        open == Some(RefPopup::Contents),
                        cx,
                    ),
                    contents_menu,
                )]),
                row(vec![text_button(
                    "ref-toc-update",
                    t("doc.toc_update"),
                    t("doc.toc_update_title"),
                    false,
                    RefAction::OpenUpdateToc,
                    cx,
                )]),
            ],
        ),
        group(
            t("note.group"),
            vec![
                row(vec![
                    crate::word_page::text_button(
                        "ref-footnote",
                        t("note.footnote"),
                        t("note.footnote_title"),
                        false,
                        Action::InsertNote(NoteKind::Footnote),
                        cx,
                    ),
                    crate::word_page::text_button(
                        "ref-endnote",
                        t("note.endnote"),
                        t("note.endnote_title"),
                        false,
                        Action::InsertNote(NoteKind::Endnote),
                        cx,
                    ),
                ]),
                row(vec![
                    dropdown(
                        list_button(
                            "ref-next-note",
                            t("notes.next_footnote"),
                            RefPopup::NextNote,
                            open == Some(RefPopup::NextNote),
                            cx,
                        ),
                        note_menu,
                    ),
                    text_button(
                        "ref-show-notes",
                        t("notes.show"),
                        t("notes.show_title"),
                        false,
                        RefAction::ShowNotes,
                        cx,
                    ),
                    text_button(
                        "ref-note-options",
                        "…",
                        t("notes.title"),
                        false,
                        RefAction::OpenNotes,
                        cx,
                    ),
                ]),
            ],
        ),
        group(
            t("citations.group"),
            vec![
                row(vec![
                    dropdown(
                        list_button(
                            "ref-cite",
                            t("citations.insert"),
                            RefPopup::Citation,
                            open == Some(RefPopup::Citation),
                            cx,
                        ),
                        citation_menu,
                    ),
                    text_button(
                        "ref-sources",
                        t("sources.manage"),
                        t("sources.title"),
                        false,
                        RefAction::OpenSources,
                        cx,
                    ),
                ]),
                row(vec![
                    dropdown(
                        list_button(
                            "ref-style",
                            format!("{}: {}", t("citations.style"), style.name()),
                            RefPopup::Style,
                            open == Some(RefPopup::Style),
                            cx,
                        ),
                        style_menu,
                    ),
                    dropdown(
                        list_button(
                            "ref-bibliography",
                            t("bibliography.title"),
                            RefPopup::Bibliography,
                            open == Some(RefPopup::Bibliography),
                            cx,
                        ),
                        bibliography_menu,
                    ),
                ]),
            ],
        ),
        group(
            t("captions.group"),
            vec![
                row(vec![
                    text_button(
                        "ref-caption",
                        t("caption.insert"),
                        t("caption.title"),
                        false,
                        RefAction::OpenCaption,
                        cx,
                    ),
                    text_button(
                        "ref-figures",
                        t("toc.figures.insert"),
                        t("toc.figures.title"),
                        false,
                        RefAction::OpenFigures,
                        cx,
                    ),
                ]),
                row(vec![
                    text_button(
                        "ref-figures-update",
                        t("toc.figures.update"),
                        t("toc.figures.update"),
                        false,
                        RefAction::UpdateFigures,
                        cx,
                    ),
                    text_button(
                        "ref-xref",
                        t("xref.title"),
                        t("xref.title"),
                        false,
                        RefAction::OpenCrossReference,
                        cx,
                    ),
                ]),
            ],
        ),
        group(
            t("index.group"),
            vec![
                row(vec![text_button(
                    "ref-mark-entry",
                    t("index.mark.title"),
                    t("index.mark.title"),
                    false,
                    RefAction::OpenMarkEntry,
                    cx,
                )]),
                row(vec![
                    text_button(
                        "ref-index",
                        t("index.insert"),
                        t("index.title"),
                        false,
                        RefAction::OpenIndex,
                        cx,
                    ),
                    text_button(
                        "ref-index-update",
                        t("index.update"),
                        t("index.update"),
                        false,
                        RefAction::UpdateIndex,
                        cx,
                    ),
                ]),
            ],
        ),
        group(
            t("toa.group"),
            vec![
                row(vec![text_button(
                    "ref-mark-citation",
                    t("toa.mark.title"),
                    t("toa.mark.title"),
                    false,
                    RefAction::OpenMarkCitation,
                    cx,
                )]),
                row(vec![
                    text_button(
                        "ref-toa",
                        t("toa.insert"),
                        t("toa.title"),
                        false,
                        RefAction::OpenAuthorities,
                        cx,
                    ),
                    text_button(
                        "ref-toa-update",
                        t("toa.update"),
                        t("toa.update"),
                        false,
                        RefAction::UpdateAuthorities,
                        cx,
                    ),
                ]),
            ],
        ),
        // The browser shell has no 表示 tab: the pane, the brackets and
        // ジャンプ go here, with ブックマーク and 相互参照.
        group(
            t("nav.title"),
            vec![
                row(view_buttons(&shown, cx)),
                row(vec![text_button(
                    "insert-bookmark",
                    t("bookmark.title"),
                    t("bookmark.title"),
                    false,
                    RefAction::OpenBookmark,
                    cx,
                )]),
            ],
        ),
    ]
}

/// 挿入's リンク group: ブックマーク and 相互参照, beside ⌘K's link.
pub fn links_group(cx: &mut Context<DocumentView>) -> AnyElement {
    group(
        t("links.group"),
        vec![row(vec![
            text_button(
                "insert-bookmark",
                t("bookmark.title"),
                t("bookmark.title"),
                false,
                RefAction::OpenBookmark,
                cx,
            ),
            text_button(
                "insert-xref",
                t("xref.title"),
                t("xref.title"),
                false,
                RefAction::OpenCrossReference,
                cx,
            ),
        ])],
    )
}

/// ナビゲーション ウィンドウ, ブックマーク表示 and ジャンプ.
pub fn view_buttons(shown: &RibbonState, cx: &mut Context<DocumentView>) -> Vec<AnyElement> {
    vec![
        text_button(
            "view-navigation",
            t("nav.title"),
            t("nav.title"),
            shown.navigation,
            RefAction::ToggleNavigation,
            cx,
        ),
        text_button(
            "view-bookmarks",
            t("bookmark.show"),
            t("bookmark.show"),
            shown.show_bookmarks,
            RefAction::ToggleShowBookmarks,
            cx,
        ),
        text_button(
            "view-goto",
            t("goto.title"),
            t("goto.title"),
            false,
            RefAction::OpenGoTo,
            cx,
        ),
    ]
}

/// 校閲's 前のコメント / 次のコメント.
pub fn comment_steps(cx: &mut Context<DocumentView>) -> Vec<AnyElement> {
    vec![
        text_button(
            "comment-previous",
            t("comments.previous"),
            t("comments.previous"),
            false,
            RefAction::StepComment(false),
            cx,
        ),
        text_button(
            "comment-next",
            t("comments.next"),
            t("comments.next"),
            false,
            RefAction::StepComment(true),
            cx,
        ),
    ]
}

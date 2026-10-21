//! 引用文献と文献目録: 資料文献の管理 (the master list beside the app and
//! the document's current list, sources copied, created, edited and
//! deleted), 引用文献の挿入, スタイル and 文献目録.

use gpui::{div, prelude::*, px, AnyElement, Context, SharedString};
use jade_web::word::references::citations::{
    BibliographyStyle, Source, WordCitations, SOURCE_TYPES,
};

use super::{action_button, heading, line, list_box, select, t, text_field, Dialog, RefList};
use crate::palette as theme;
use crate::view::DocView as DocumentView;
#[allow(unused_imports)]
use jade_web::editing::WordSession;

/// The source's fields, in the order the dialog has them.
const FIELDS: [&str; 6] = [
    "sources.author",
    "sources.title_field",
    "sources.year",
    "sources.publisher",
    "sources.url",
    "sources.tag",
];

pub fn insert_citation(view: &mut DocumentView, tag: &str, cx: &mut Context<DocumentView>) {
    let done = view.doc_mut().insert_citation(tag).map(Some);
    view.after_reference_edit(done, cx);
}

pub fn set_style(
    view: &mut DocumentView,
    style: BibliographyStyle,
    cx: &mut Context<DocumentView>,
) {
    let done = view.doc_mut().set_bibliography_style(style);
    view.after_reference_edit(done.map(|()| None), cx);
}

pub fn insert_bibliography(
    view: &mut DocumentView,
    heading: Option<&'static str>,
    cx: &mut Context<DocumentView>,
) {
    let done = view.doc_mut().insert_bibliography(heading);
    view.after_reference_edit(done.map(|()| None), cx);
}

pub fn open_sources(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    view.references.master();
    view.references
        .open(Dialog::Sources(None, None), Vec::new());
    cx.notify();
}

/// 資料文献の作成: from 資料文献の管理 (`back_to_sources`) or from 引用
/// 文献の挿入's 新しい資料文献の追加 (`cite`: cited at the caret after).
pub fn open_new_source(
    view: &mut DocumentView,
    back_to_sources: bool,
    cite: bool,
    cx: &mut Context<DocumentView>,
) {
    view.references.open(
        Dialog::SourceEdit {
            editing: None,
            source_type: 0,
            back_to_sources,
            cite,
        },
        vec![String::new(); FIELDS.len()],
    );
    cx.notify();
}

fn edit_source(view: &mut DocumentView, source: Source, cx: &mut Context<DocumentView>) {
    let source_type = SOURCE_TYPES
        .iter()
        .position(|kind| *kind == source.source_type)
        .unwrap_or(0);
    view.references.open(
        Dialog::SourceEdit {
            editing: Some(source.tag.clone()),
            source_type,
            back_to_sources: true,
            cite: false,
        },
        vec![
            source.author,
            source.title,
            source.year,
            source.publisher.unwrap_or_default(),
            source.url.unwrap_or_default(),
            source.tag,
        ],
    );
    cx.notify();
}

/// The source the edit dialog's fields describe.
fn typed_source(view: &DocumentView, source_type: usize) -> Source {
    let field = |index: usize| view.references.field(index).trim().to_string();
    let optional = |index: usize| Some(field(index)).filter(|value| !value.is_empty());
    Source {
        tag: field(5),
        source_type: SOURCE_TYPES[source_type.min(SOURCE_TYPES.len() - 1)].to_string(),
        author: field(0),
        title: field(1),
        year: field(2),
        publisher: optional(3),
        url: optional(4),
    }
}

pub fn confirm_source(
    view: &mut DocumentView,
    editing: Option<String>,
    source_type: usize,
    back_to_sources: bool,
    cite: bool,
    cx: &mut Context<DocumentView>,
) {
    let source = typed_source(view, source_type);
    let reopen = |view: &mut DocumentView, problem: String| {
        view.references.dialog = Some(Dialog::SourceEdit {
            editing: editing.clone(),
            source_type,
            back_to_sources,
            cite,
        });
        view.references.problem = Some(problem);
    };
    if source.author.is_empty() || source.title.is_empty() {
        reopen(view, t("sources.need_author_title").to_string());
        return cx.notify();
    }
    let done = match &editing {
        Some(tag) => {
            // Edited where it is: in the document, the master list, or both.
            let in_document = view.doc().sources().iter().any(|source| &source.tag == tag);
            let edited = Source {
                tag: tag.clone(),
                ..source.clone()
            };
            let master = view.references.master();
            if let Some(entry) = master.iter_mut().find(|entry| &entry.tag == tag) {
                *entry = edited.clone();
            }
            if in_document {
                view.doc_mut().edit_source(tag, edited).map(|()| None)
            } else {
                Ok(None)
            }
        }
        None => match view.doc_mut().add_source(source.clone()) {
            Ok(tag) => {
                // A new source goes into the master list too, as in Word.
                let master = view.references.master();
                if !master.iter().any(|entry| entry.tag == tag) {
                    master.push(Source {
                        tag: tag.clone(),
                        ..source.clone()
                    });
                }
                if cite {
                    view.doc_mut().insert_citation(&tag).map(Some)
                } else {
                    Ok(None)
                }
            }
            Err(problem) => Err(problem),
        },
    };
    match done {
        Ok(caret) => {
            view.references.save_master();
            if back_to_sources {
                view.references
                    .open(Dialog::Sources(None, None), Vec::new());
            } else {
                view.references.close();
            }
            view.after_reference_edit(Ok(caret), cx);
        }
        Err(problem) => {
            reopen(view, problem);
            cx.notify();
        }
    }
}

pub fn source_edit_body(view: &DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    let Some(Dialog::SourceEdit { source_type, .. }) = view.references.dialog.as_ref() else {
        return div().into_any_element();
    };
    let entries: Vec<(SharedString, bool)> = SOURCE_TYPES
        .iter()
        .enumerate()
        .map(|(index, kind)| (t(source_type_key(kind)).into(), index == *source_type))
        .collect();
    div()
        .w(px(420.0))
        .flex()
        .flex_col()
        .gap_2()
        .child(line("sources.type").child(select(
            view,
            RefList::SourceType,
            t(source_type_key(SOURCE_TYPES[*source_type])),
            200.0,
            entries,
            cx,
            |references, index| {
                if let Some(Dialog::SourceEdit { source_type, .. }) = references.dialog.as_mut() {
                    *source_type = index;
                }
            },
        )))
        .children(
            FIELDS
                .iter()
                .enumerate()
                .map(|(index, label)| text_field(view, index, label, 280.0, cx)),
        )
        .into_any_element()
}

fn source_type_key(kind: &str) -> &'static str {
    match kind {
        "Book" => "sources.type.book",
        "JournalArticle" => "sources.type.journal",
        "ArticleInAPeriodical" => "sources.type.periodical",
        "ConferenceProceedings" => "sources.type.conference",
        "Report" => "sources.type.report",
        _ => "sources.type.web",
    }
}

/// 資料文献の管理: the master list on the left, the document's on the
/// right, and the buttons between them.
pub fn sources_body(view: &DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    let Some(Dialog::Sources(master_pick, current_pick)) = view.references.dialog.as_ref() else {
        return div().into_any_element();
    };
    let (master_pick, current_pick) = (*master_pick, *current_pick);
    let master = view.references.master.clone().unwrap_or_default();
    let current = view.doc().sources();
    let cited = view.doc().cited_tags();
    let style = view.doc().bibliography_style();
    let row = |source: &Source, mark: bool| -> SharedString {
        format!(
            "{}{}; {} ({})",
            if mark { "✓ " } else { "" },
            source.author,
            source.title,
            source.year
        )
        .into()
    };
    let master_rows: Vec<(SharedString, bool)> = master
        .iter()
        .enumerate()
        .map(|(index, source)| (row(source, false), master_pick == Some(index)))
        .collect();
    let current_rows: Vec<(SharedString, bool)> = current
        .iter()
        .enumerate()
        .map(|(index, source)| {
            (
                row(source, cited.contains(&source.tag)),
                current_pick == Some(index),
            )
        })
        .collect();
    let preview = current_pick
        .and_then(|index| current.get(index))
        .or_else(|| master_pick.and_then(|index| master.get(index)))
        .map(|source| format!("{}\n{}", style.citation(source, 1), style.entry(source, 1)))
        .unwrap_or_default();
    div()
        .w(px(620.0))
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(heading("sources.master"))
                        .child(list_box(
                            "sources-master",
                            260.0,
                            220.0,
                            master_rows,
                            cx,
                            |view, index, _| {
                                if let Some(Dialog::Sources(master, current)) =
                                    view.references.dialog.as_mut()
                                {
                                    *master = Some(index);
                                    *current = None;
                                }
                            },
                        )),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .justify_center()
                        .gap_2()
                        .child(action_button(
                            "sources-copy",
                            "sources.copy",
                            cx,
                            copy_to_document,
                        ))
                        .child(action_button(
                            "sources-copy-back",
                            "sources.copy_back",
                            cx,
                            copy_to_master,
                        )),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(heading("sources.current"))
                        .child(list_box(
                            "sources-current",
                            260.0,
                            220.0,
                            current_rows,
                            cx,
                            |view, index, _| {
                                if let Some(Dialog::Sources(master, current)) =
                                    view.references.dialog.as_mut()
                                {
                                    *current = Some(index);
                                    *master = None;
                                }
                            },
                        )),
                ),
        )
        .child(
            div()
                .text_color(theme::dim())
                .child(t("sources.cited_note")),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .child(action_button(
                    "sources-new",
                    "sources.new_button",
                    cx,
                    |view, cx| open_new_source(view, true, false, cx),
                ))
                .child(action_button(
                    "sources-edit",
                    "sources.edit",
                    cx,
                    |view, cx| {
                        let Some(Dialog::Sources(master, current)) =
                            view.references.dialog.as_ref()
                        else {
                            return;
                        };
                        let source = match (current, master) {
                            (Some(index), _) => view.doc().sources().get(*index).cloned(),
                            (None, Some(index)) => view
                                .references
                                .master
                                .as_ref()
                                .and_then(|master| master.get(*index).cloned()),
                            _ => None,
                        };
                        if let Some(source) = source {
                            edit_source(view, source, cx);
                        }
                    },
                ))
                .child(action_button(
                    "sources-delete",
                    "sources.delete",
                    cx,
                    delete_picked,
                )),
        )
        .child(heading("sources.preview"))
        .child(div().whitespace_normal().child(preview))
        .into_any_element()
}

/// コピー →: the master list's source into the document.
fn copy_to_document(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let Some(Dialog::Sources(Some(index), _)) = view.references.dialog.as_ref() else {
        return;
    };
    let Some(source) = view
        .references
        .master
        .as_ref()
        .and_then(|master| master.get(*index).cloned())
    else {
        return;
    };
    let done = view.doc_mut().add_source(source).map(|_| None);
    view.after_reference_edit(done, cx);
}

/// ← コピー: the document's source into the master list.
fn copy_to_master(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let Some(Dialog::Sources(_, Some(index))) = view.references.dialog.as_ref() else {
        return;
    };
    let Some(source) = view.doc().sources().get(*index).cloned() else {
        return;
    };
    let master = view.references.master();
    if !master.iter().any(|entry| entry.tag == source.tag) {
        master.push(source);
    }
    view.references.save_master();
    cx.notify();
}

/// 削除: from whichever list it was picked in; a cited source stays.
fn delete_picked(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    match view.references.dialog.as_ref() {
        Some(Dialog::Sources(_, Some(index))) => {
            let Some(tag) = view
                .doc()
                .sources()
                .get(*index)
                .map(|source| source.tag.clone())
            else {
                return;
            };
            let done = view.doc_mut().delete_source(&tag);
            if let Err(problem) = &done {
                view.references.problem = Some(problem.clone());
                return cx.notify();
            }
            if let Some(Dialog::Sources(_, current)) = view.references.dialog.as_mut() {
                *current = None;
            }
            view.after_reference_edit(done.map(|()| None), cx);
        }
        Some(Dialog::Sources(Some(index), _)) => {
            let index = *index;
            let master = view.references.master();
            if index < master.len() {
                master.remove(index);
            }
            view.references.save_master();
            if let Some(Dialog::Sources(master, _)) = view.references.dialog.as_mut() {
                *master = None;
            }
            cx.notify();
        }
        _ => {}
    }
}

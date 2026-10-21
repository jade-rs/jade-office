//! 索引 and 引用文献一覧: 索引登録 (登録 / すべて登録), the 索引 dialog,
//! 索引の更新, 引用文の登録 and the 引用文献一覧 dialog.

use gpui::{div, prelude::*, px, AnyElement, ClickEvent, Context, SharedString};
use jade_web::word::references::bookmarks::WordBookmarks;
use jade_web::word::references::index::{
    AuthoritiesDialog, EntryReference, IndexDialog, IndexLanguage, IndexLayout,
    MarkCitationDialog, MarkEntryDialog, WordIndex, TOA_CATEGORIES,
};

use super::{
    action_button, check_box, choice, edit, heading, line, select, t, text_field, Dialog,
    RefList, References,
};
use crate::view::DocView as DocumentView;
#[allow(unused_imports)]
use jade_web::editing::WordSession;
use crate::palette as theme;

/// The fields of 索引登録.
const MAIN: usize = 0;
const MAIN_READING: usize = 1;
const SUB: usize = 2;
const SUB_READING: usize = 3;
const SEE: usize = 4;

pub fn open_mark_entry(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let text = view.doc().selected_text().unwrap_or_default();
    let dialog = MarkEntryDialog::for_text(&text);
    let main = dialog.main.clone();
    view.references.open(
        Dialog::MarkEntry(dialog),
        vec![main, String::new(), String::new(), String::new(), "→ ".to_string()],
    );
    cx.notify();
}

/// The dialog with what was typed into its fields.
fn typed_entry(view: &DocumentView, mut dialog: MarkEntryDialog) -> MarkEntryDialog {
    let field = |index| view.references.field(index).to_string();
    dialog.main = field(MAIN);
    dialog.main_reading = field(MAIN_READING);
    dialog.sub = field(SUB);
    dialog.sub_reading = field(SUB_READING);
    if let EntryReference::CrossReference(_) = dialog.reference {
        dialog.reference = EntryReference::CrossReference(field(SEE));
    }
    dialog
}

/// 登録 (or すべて登録). Word keeps the dialog open for more; so does this.
pub fn confirm_mark_entry(
    view: &mut DocumentView,
    dialog: MarkEntryDialog,
    all: bool,
    cx: &mut Context<DocumentView>,
) {
    let typed = typed_entry(view, dialog.clone());
    let done = if all {
        let marked = view.doc_mut().mark_all_index_entries(&typed);
        marked.map(|count| {
            view.set_status(SharedString::from(crate::i18n::format(
                "index.marked",
                &[("n", &count)],
            )));
        })
    } else {
        view.doc_mut().mark_index_entry(&typed)
    };
    view.references.dialog = Some(Dialog::MarkEntry(dialog));
    match done {
        Ok(()) => {
            let status = view.status.clone();
            view.after_reference_edit(Ok(None), cx);
            view.status = status;
        }
        Err(problem) => {
            view.references.problem = Some(problem);
            cx.notify();
        }
    }
}

fn entry_dialog(references: &mut References) -> Option<&mut MarkEntryDialog> {
    match references.dialog.as_mut() {
        Some(Dialog::MarkEntry(dialog)) => Some(dialog),
        _ => None,
    }
}

pub fn mark_entry_body(view: &DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    let Some(Dialog::MarkEntry(dialog)) = view.references.dialog.as_ref() else {
        return div().into_any_element();
    };
    let bookmarks: Vec<String> = view.doc()
        .bookmarks()
        .into_iter()
        .filter(|bookmark| !bookmark.hidden())
        .map(|bookmark| bookmark.name)
        .collect();
    let range = match &dialog.reference {
        EntryReference::PageRange(name) => name.clone(),
        _ => String::new(),
    };
    let entries: Vec<(SharedString, bool)> = bookmarks
        .iter()
        .map(|name| (name.clone().into(), *name == range))
        .collect();
    let reference = dialog.reference.clone();
    div()
        .w(px(440.0))
        .flex()
        .flex_col()
        .gap_2()
        .child(text_field(view, MAIN, "index.main", 220.0, cx))
        .child(text_field(view, MAIN_READING, "index.reading", 220.0, cx))
        .child(text_field(view, SUB, "index.sub", 220.0, cx))
        .child(text_field(view, SUB_READING, "index.reading", 220.0, cx))
        .child(heading("index.options"))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    choice("index-see", t("index.see"), matches!(reference, EntryReference::CrossReference(_)))
                        .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
                            edit(view, cx, |references| {
                                if let Some(dialog) = entry_dialog(references) {
                                    dialog.reference = EntryReference::CrossReference(String::new());
                                }
                                references.focus = SEE;
                            })
                        })),
                )
                .child(text_field(view, SEE, "", 200.0, cx)),
        )
        .child(
            choice("index-current", t("index.current_page"), reference == EntryReference::CurrentPage).on_click(
                cx.listener(|view, _: &ClickEvent, _, cx| {
                    edit(view, cx, |references| {
                        if let Some(dialog) = entry_dialog(references) {
                            dialog.reference = EntryReference::CurrentPage;
                        }
                    })
                }),
            ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    choice("index-range", t("index.page_range"), matches!(reference, EntryReference::PageRange(_)))
                        .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
                            edit(view, cx, |references| {
                                if let Some(dialog) = entry_dialog(references) {
                                    dialog.reference = EntryReference::PageRange(String::new());
                                }
                            })
                        })),
                )
                .child(select(
                    view,
                    RefList::Bookmark,
                    if range.is_empty() { t("index.bookmark") .to_string() } else { range.clone() },
                    160.0,
                    entries,
                    cx,
                    move |references, index| {
                        if let Some(dialog) = entry_dialog(references) {
                            dialog.reference = EntryReference::PageRange(bookmarks[index].clone());
                        }
                    },
                )),
        )
        .child(heading("index.page_format"))
        .child(
            div()
                .flex()
                .gap_3()
                .child(check_box("index-bold", "index.bold", dialog.bold, cx, |references| {
                    if let Some(dialog) = entry_dialog(references) {
                        dialog.bold = !dialog.bold;
                    }
                }))
                .child(check_box("index-italic", "index.italic", dialog.italic, cx, |references| {
                    if let Some(dialog) = entry_dialog(references) {
                        dialog.italic = !dialog.italic;
                    }
                })),
        )
        .child(action_button("index-mark-all", "index.mark_all", cx, |view, cx| {
            if let Some(Dialog::MarkEntry(dialog)) = view.references.dialog.take() {
                super::sync_selection(view);
                confirm_mark_entry(view, dialog, true, cx);
            }
        }))
        .into_any_element()
}

pub fn open_index(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    view.references
        .open(Dialog::Index(IndexDialog::default()), Vec::new());
    cx.notify();
}

pub fn confirm_index(view: &mut DocumentView, dialog: IndexDialog, cx: &mut Context<DocumentView>) {
    match view.doc_mut().insert_index(&dialog) {
        Ok(()) => {
            view.references.close();
            view.after_reference_edit(Ok(None), cx);
        }
        Err(problem) => {
            view.references.dialog = Some(Dialog::Index(dialog));
            view.references.problem = Some(problem);
            cx.notify();
        }
    }
}

pub fn update_index(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let done = view.doc_mut().update_indexes().map(|_| None);
    view.after_reference_edit(done, cx);
}

fn index_dialog(references: &mut References) -> Option<&mut IndexDialog> {
    match references.dialog.as_mut() {
        Some(Dialog::Index(dialog)) => Some(dialog),
        _ => None,
    }
}

pub fn index_body(view: &DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    let Some(Dialog::Index(dialog)) = view.references.dialog.as_ref() else {
        return div().into_any_element();
    };
    let preview = view.doc()
        .index_preview(dialog)
        .into_iter()
        .take(12)
        .map(|line| {
            div()
                .flex()
                .pl(px((line.indent - 10.5).max(0.0)))
                .when(line.bold, |row| row.font_weight(gpui::FontWeight::BOLD))
                .child(div().flex_none().child(line.text))
                .children(line.page.map(|page| {
                    div()
                        .pl_1()
                        .text_color(theme::dim())
                        .child(match line.separator {
                            Some(separator) => format!("{separator}{page}"),
                            None => format!(" … {page}"),
                        })
                }))
                .into_any_element()
        })
        .collect::<Vec<_>>();
    let columns: Vec<(SharedString, bool)> = (1..=4u8)
        .map(|count| (count.to_string().into(), count == dialog.columns))
        .collect();
    div()
        .w(px(440.0))
        .flex()
        .flex_col()
        .gap_2()
        .child(heading("toc.preview"))
        .child(
            div()
                .id("index-preview")
                .h(px(160.0))
                .overflow_y_scroll()
                .p_2()
                .flex()
                .flex_col()
                .rounded_sm()
                .border_1()
                .border_color(theme::border())
                .bg(theme::canvas())
                .children(preview),
        )
        .child(
            line("index.layout")
                .child(choice("index-indented", t("index.indented"), dialog.layout == IndexLayout::Indented).on_click(
                    cx.listener(|view, _: &ClickEvent, _, cx| {
                        edit(view, cx, |references| {
                            if let Some(dialog) = index_dialog(references) {
                                dialog.layout = IndexLayout::Indented;
                            }
                        })
                    }),
                ))
                .child(choice("index-run-in", t("index.run_in"), dialog.layout == IndexLayout::RunIn).on_click(
                    cx.listener(|view, _: &ClickEvent, _, cx| {
                        edit(view, cx, |references| {
                            if let Some(dialog) = index_dialog(references) {
                                dialog.layout = IndexLayout::RunIn;
                            }
                        })
                    }),
                )),
        )
        .child(line("index.columns").child(select(
            view,
            RefList::Levels,
            dialog.columns.to_string(),
            60.0,
            columns,
            cx,
            |references, index| {
                if let Some(dialog) = index_dialog(references) {
                    dialog.columns = index as u8 + 1;
                }
            },
        )))
        .child(
            line("index.language")
                .child(choice("index-ja", t("index.japanese"), dialog.language == IndexLanguage::Japanese).on_click(
                    cx.listener(|view, _: &ClickEvent, _, cx| {
                        edit(view, cx, |references| {
                            if let Some(dialog) = index_dialog(references) {
                                dialog.language = IndexLanguage::Japanese;
                            }
                        })
                    }),
                ))
                .child(choice("index-en", t("index.english"), dialog.language == IndexLanguage::English).on_click(
                    cx.listener(|view, _: &ClickEvent, _, cx| {
                        edit(view, cx, |references| {
                            if let Some(dialog) = index_dialog(references) {
                                dialog.language = IndexLanguage::English;
                            }
                        })
                    }),
                )),
        )
        .child(check_box("index-headings", "index.headings", dialog.headings, cx, |references| {
            if let Some(dialog) = index_dialog(references) {
                dialog.headings = !dialog.headings;
            }
        }))
        .child(check_box("index-right-align", "toc.right_align", dialog.right_align, cx, |references| {
            if let Some(dialog) = index_dialog(references) {
                dialog.right_align = !dialog.right_align;
            }
        }))
        .into_any_element()
}

// ---- 引用文献一覧 -----------------------------------------------------------

pub fn open_mark_citation(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let text = view.doc().selected_text().unwrap_or_default();
    let text = text.trim().to_string();
    view.references.open(
        Dialog::MarkCitation(MarkCitationDialog {
            long: text.clone(),
            short: text.clone(),
            category: 1,
        }),
        vec![text.clone(), text],
    );
    cx.notify();
}

pub fn confirm_mark_citation(
    view: &mut DocumentView,
    mut dialog: MarkCitationDialog,
    cx: &mut Context<DocumentView>,
) {
    dialog.long = view.references.field(0).to_string();
    dialog.short = view.references.field(1).to_string();
    match view.doc_mut().mark_citation(&dialog) {
        Ok(()) => {
            view.references.close();
            view.after_reference_edit(Ok(None), cx);
        }
        Err(problem) => {
            view.references.dialog = Some(Dialog::MarkCitation(dialog));
            view.references.problem = Some(problem);
            cx.notify();
        }
    }
}

pub fn mark_citation_body(view: &DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    let Some(Dialog::MarkCitation(dialog)) = view.references.dialog.as_ref() else {
        return div().into_any_element();
    };
    let entries: Vec<(SharedString, bool)> = TOA_CATEGORIES
        .iter()
        .enumerate()
        .map(|(index, name)| ((*name).into(), index + 1 == dialog.category as usize))
        .collect();
    div()
        .w(px(420.0))
        .flex()
        .flex_col()
        .gap_2()
        .child(text_field(view, 0, "toa.long", 280.0, cx))
        .child(line("toa.category").child(select(
            view,
            RefList::Category,
            TOA_CATEGORIES[(dialog.category as usize).clamp(1, TOA_CATEGORIES.len()) - 1],
            160.0,
            entries,
            cx,
            |references, index| {
                if let Some(Dialog::MarkCitation(dialog)) = references.dialog.as_mut() {
                    dialog.category = index as u8 + 1;
                }
            },
        )))
        .child(text_field(view, 1, "toa.short", 280.0, cx))
        .into_any_element()
}

pub fn open_authorities(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    view.references
        .open(Dialog::Authorities(AuthoritiesDialog::default()), Vec::new());
    cx.notify();
}

pub fn confirm_authorities(
    view: &mut DocumentView,
    dialog: AuthoritiesDialog,
    cx: &mut Context<DocumentView>,
) {
    match view.doc_mut().insert_table_of_authorities(&dialog) {
        Ok(()) => {
            view.references.close();
            view.after_reference_edit(Ok(None), cx);
        }
        Err(problem) => {
            view.references.dialog = Some(Dialog::Authorities(dialog));
            view.references.problem = Some(problem);
            cx.notify();
        }
    }
}

/// 引用文献一覧の更新: each table made again under its instruction.
pub fn update_authorities(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let blocks: Vec<_> = view.doc()
        .word_document()
        .field_blocks()
        .into_iter()
        .filter(|block| block.name() == "TOA")
        .collect();
    if blocks.is_empty() {
        view.set_status(SharedString::from(t("toa.none")));
        return cx.notify();
    }
    let done = view.doc_mut().update_tables_of_authorities().map(|_| None);
    view.after_reference_edit(done, cx);
}

pub fn authorities_body(view: &DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    let Some(Dialog::Authorities(dialog)) = view.references.dialog.as_ref() else {
        return div().into_any_element();
    };
    let mut entries: Vec<(SharedString, bool)> = vec![(t("toa.all").into(), dialog.category.is_none())];
    entries.extend(
        TOA_CATEGORIES
            .iter()
            .enumerate()
            .map(|(index, name)| ((*name).into(), dialog.category == Some(index as u8 + 1))),
    );
    div()
        .w(px(380.0))
        .flex()
        .flex_col()
        .gap_2()
        .child(line("toa.category").child(select(
            view,
            RefList::Category,
            dialog
                .category
                .map(|category| TOA_CATEGORIES[(category as usize).clamp(1, 7) - 1])
                .unwrap_or(t("toa.all")),
            160.0,
            entries,
            cx,
            |references, index| {
                if let Some(Dialog::Authorities(dialog)) = references.dialog.as_mut() {
                    dialog.category = (index > 0).then_some(index as u8);
                }
            },
        )))
        .child(check_box("toa-passim", "toa.passim", dialog.passim, cx, |references| {
            if let Some(Dialog::Authorities(dialog)) = references.dialog.as_mut() {
                dialog.passim = !dialog.passim;
            }
        }))
        .into_any_element()
}

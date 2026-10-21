//! ブックマーク, 相互参照 and 図表番号の挿入.

use gpui::{div, prelude::*, px, AnyElement, ClickEvent, Context, SharedString};
use jade_office_model::word::references::bookmarks::{
    BookmarkDialog, BookmarkSort, WordBookmarks,
};
use jade_office_model::word::references::captions::{
    format_sample, CaptionDialog, WordCaptions, CAPTION_FORMATS, CHAPTER_SEPARATORS,
};
use jade_office_model::word::references::crossref::{
    CrossReferenceDialog, ReferenceType, WordCrossReferences,
};

use super::{
    action_button, check_box, choice, edit, heading, line, list_box, select, t, text_field,
    Dialog, RefList, References,
};
use crate::DocumentView;

// ---- ブックマーク ---------------------------------------------------------

pub fn open_bookmark(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    // The selected text suggests a name, as Word does.
    let suggestion: String = view
        .document
        .selected_text()
        .unwrap_or_default()
        .chars()
        .filter(|ch| ch.is_alphanumeric() || *ch == '_')
        .take(40)
        .collect();
    let suggestion = if suggestion
        .chars()
        .next()
        .is_some_and(|first| first.is_alphabetic())
    {
        suggestion
    } else {
        String::new()
    };
    view.references
        .open(Dialog::Bookmark(BookmarkDialog::default()), vec![suggestion]);
    cx.notify();
}

/// 追加.
pub fn confirm_bookmark(view: &mut DocumentView, dialog: BookmarkDialog, cx: &mut Context<DocumentView>) {
    let name = view.references.field(0).to_string();
    match view.document.add_bookmark(&name) {
        Ok(()) => {
            view.references.close();
            view.after_reference_edit(Ok(None), cx);
        }
        Err(problem) => {
            view.references.dialog = Some(Dialog::Bookmark(dialog));
            view.references.problem = Some(problem);
            cx.notify();
        }
    }
}

fn bookmark_dialog(references: &mut References) -> Option<&mut BookmarkDialog> {
    match references.dialog.as_mut() {
        Some(Dialog::Bookmark(dialog)) => Some(dialog),
        _ => None,
    }
}

pub fn bookmark_body(view: &DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    let Some(Dialog::Bookmark(dialog)) = view.references.dialog.as_ref() else {
        return div().into_any_element();
    };
    let list = view.document.bookmark_list(dialog);
    let selected = dialog.selected.clone();
    let rows: Vec<(SharedString, bool)> = list
        .iter()
        .map(|bookmark| {
            (
                bookmark.name.clone().into(),
                selected.as_deref() == Some(bookmark.name.as_str()),
            )
        })
        .collect();
    let names: Vec<String> = list.iter().map(|bookmark| bookmark.name.clone()).collect();
    div()
        .w(px(360.0))
        .flex()
        .flex_col()
        .gap_2()
        .child(text_field(view, 0, "bookmark.name", 240.0, cx))
        .child(list_box("bookmark-list", 360.0, 160.0, rows, cx, move |view, index, _| {
            if let Some(name) = names.get(index) {
                if let Some(dialog) = bookmark_dialog(&mut view.references) {
                    dialog.selected = Some(name.clone());
                }
                view.references.fields[0] = jade_office_model::text::TextEdit::new(name.clone());
            }
        }))
        .child(
            line("bookmark.sort")
                .child(choice("bookmark-sort-name", t("bookmark.sort.name"), dialog.sort == BookmarkSort::Name).on_click(
                    cx.listener(|view, _: &ClickEvent, _, cx| {
                        edit(view, cx, |references| {
                            if let Some(dialog) = bookmark_dialog(references) {
                                dialog.sort = BookmarkSort::Name;
                            }
                        })
                    }),
                ))
                .child(
                    choice("bookmark-sort-location", t("bookmark.sort.location"), dialog.sort == BookmarkSort::Location)
                        .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
                            edit(view, cx, |references| {
                                if let Some(dialog) = bookmark_dialog(references) {
                                    dialog.sort = BookmarkSort::Location;
                                }
                            })
                        })),
                ),
        )
        .child(check_box("bookmark-hidden", "bookmark.hidden", dialog.show_hidden, cx, |references| {
            if let Some(dialog) = bookmark_dialog(references) {
                dialog.show_hidden = !dialog.show_hidden;
            }
        }))
        .child(
            div()
                .flex()
                .gap_2()
                .child(action_button("bookmark-delete", "bookmark.delete", cx, |view, cx| {
                    let Some(name) = view.references.dialog.as_ref().and_then(|dialog| match dialog {
                        Dialog::Bookmark(dialog) => dialog.selected.clone(),
                        _ => None,
                    }) else {
                        return;
                    };
                    let done = view.document.remove_bookmark(&name);
                    if let Some(dialog) = bookmark_dialog(&mut view.references) {
                        dialog.selected = None;
                    }
                    view.after_reference_edit(done.map(|()| None), cx);
                }))
                .child(action_button("bookmark-go", "bookmark.go", cx, |view, cx| {
                    let name = view
                        .references
                        .dialog
                        .as_ref()
                        .and_then(|dialog| match dialog {
                            Dialog::Bookmark(dialog) => dialog.selected.clone(),
                            _ => None,
                        })
                        .unwrap_or_else(|| view.references.field(0).to_string());
                    if let Some((start, end)) = view.document.bookmark_range(&name) {
                        view.reveal(start, end, cx);
                    }
                })),
        )
        .into_any_element()
}

// ---- 相互参照 ---------------------------------------------------------------

pub fn open_cross_reference(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let dialog = view.document.cross_reference_dialog(&view.references.custom_labels);
    view.references.open(Dialog::CrossReference(dialog), Vec::new());
    cx.notify();
}

fn xref_dialog(references: &mut References) -> Option<&mut CrossReferenceDialog> {
    match references.dialog.as_mut() {
        Some(Dialog::CrossReference(dialog)) => Some(dialog),
        _ => None,
    }
}

fn type_label(kind: &ReferenceType) -> String {
    match kind {
        ReferenceType::Heading => t("xref.type.heading").to_string(),
        ReferenceType::Bookmark => t("xref.type.bookmark").to_string(),
        ReferenceType::Footnote => t("xref.type.footnote").to_string(),
        ReferenceType::Endnote => t("xref.type.endnote").to_string(),
        ReferenceType::Caption(label) => label.clone(),
    }
}

/// 挿入: Word keeps the dialog open for the next one; here it closes.
pub fn confirm_cross_reference(
    view: &mut DocumentView,
    dialog: CrossReferenceDialog,
    cx: &mut Context<DocumentView>,
) {
    let targets = view.document.reference_targets(&dialog.kind);
    let Some(target) = dialog.selected.and_then(|index| targets.get(index)) else {
        view.references.dialog = Some(Dialog::CrossReference(dialog));
        view.references.problem = Some(t("xref.pick").to_string());
        return cx.notify();
    };
    match view
        .document
        .insert_cross_reference(target, dialog.text, dialog.hyperlink)
    {
        Ok(caret) => {
            view.references.close();
            view.after_reference_edit(Ok(Some(caret)), cx);
        }
        Err(problem) => {
            view.references.dialog = Some(Dialog::CrossReference(dialog));
            view.references.problem = Some(problem);
            cx.notify();
        }
    }
}

pub fn cross_reference_body(view: &DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    let Some(Dialog::CrossReference(dialog)) = view.references.dialog.as_ref() else {
        return div().into_any_element();
    };
    let kinds = dialog.kinds.clone();
    let kind_entries: Vec<(SharedString, bool)> = kinds
        .iter()
        .map(|kind| (type_label(kind).into(), *kind == dialog.kind))
        .collect();
    let texts = dialog.kind.texts();
    let kind = dialog.kind.clone();
    let text_entries: Vec<(SharedString, bool)> = texts
        .iter()
        .map(|text| (t(kind.text_key(*text)).into(), *text == dialog.text))
        .collect();
    let targets = view.document.reference_targets(&dialog.kind);
    let rows: Vec<(SharedString, bool)> = targets
        .iter()
        .enumerate()
        .map(|(index, target)| (target.label.clone().into(), dialog.selected == Some(index)))
        .collect();
    div()
        .w(px(440.0))
        .flex()
        .flex_col()
        .gap_2()
        .child(
            line("xref.type").child(select(
                view,
                RefList::ReferenceType,
                type_label(&dialog.kind),
                160.0,
                kind_entries,
                cx,
                move |references, index| {
                    if let Some(dialog) = xref_dialog(references) {
                        dialog.set_kind(kinds[index].clone());
                    }
                },
            )),
        )
        .child(
            line("xref.text").child(select(
                view,
                RefList::Category,
                t(dialog.kind.text_key(dialog.text)),
                200.0,
                text_entries,
                cx,
                move |references, index| {
                    if let Some(dialog) = xref_dialog(references) {
                        dialog.text = texts[index];
                    }
                },
            )),
        )
        .child(check_box("xref-hyperlink", "xref.hyperlink", dialog.hyperlink, cx, |references| {
            if let Some(dialog) = xref_dialog(references) {
                dialog.hyperlink = !dialog.hyperlink;
            }
        }))
        .child(heading("xref.targets"))
        .child(list_box("xref-targets", 440.0, 200.0, rows, cx, |view, index, _| {
            if let Some(dialog) = xref_dialog(&mut view.references) {
                dialog.selected = Some(index);
            }
            view.references.problem = None;
        }))
        .into_any_element()
}

// ---- 図表番号 ---------------------------------------------------------------

pub fn open_caption(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let dialog = view.document.caption_dialog(&view.references.custom_labels);
    view.references.open(Dialog::Caption(dialog, false), vec![String::new()]);
    cx.notify();
}

/// The paragraph a caption goes by: the clicked picture's, or the caret's.
fn caption_anchor(view: &DocumentView) -> usize {
    view.selected_object
        .as_ref()
        .map(|object| object.paragraph)
        .unwrap_or(view.insert_at.0)
}

fn caption_dialog(references: &mut References) -> Option<&mut CaptionDialog> {
    match references.dialog.as_mut() {
        Some(Dialog::Caption(dialog, _)) => Some(dialog),
        _ => None,
    }
}

pub fn confirm_caption(
    view: &mut DocumentView,
    mut dialog: CaptionDialog,
    new_label: bool,
    cx: &mut Context<DocumentView>,
) {
    if new_label {
        // ラベル名's OK: the label joins the list and is picked.
        let label = view.references.field(1).trim().to_string();
        let text = view.references.field(0).to_string();
        if !label.is_empty() {
            if !dialog.labels.contains(&label) {
                dialog.labels.push(label.clone());
            }
            if !view.references.custom_labels.contains(&label) {
                view.references.custom_labels.push(label.clone());
            }
            dialog.label = label;
        }
        view.references.open(Dialog::Caption(dialog, false), vec![text]);
        return cx.notify();
    }
    let anchor = caption_anchor(view);
    let text = view.references.field(0).to_string();
    match view.document.insert_caption(&dialog, anchor, &text) {
        Ok(caret) => {
            view.references.close();
            view.selected_object = None;
            view.after_reference_edit(Ok(Some(caret)), cx);
        }
        Err(problem) => {
            view.references.open(Dialog::Caption(dialog, false), vec![text]);
            view.references.problem = Some(problem);
            cx.notify();
        }
    }
}

pub fn caption_body(view: &DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    let Some(Dialog::Caption(dialog, new_label)) = view.references.dialog.as_ref() else {
        return div().into_any_element();
    };
    if *new_label {
        return div()
            .w(px(300.0))
            .child(text_field(view, 1, "caption.label_name", 180.0, cx))
            .into_any_element();
    }
    let preview = view.document.caption_preview(dialog, caption_anchor(view));
    let labels = dialog.labels.clone();
    let label_entries: Vec<(SharedString, bool)> = labels
        .iter()
        .map(|label| (label.clone().into(), *label == dialog.label))
        .collect();
    let format_entries: Vec<(SharedString, bool)> = CAPTION_FORMATS
        .iter()
        .map(|format| (format_sample(*format).into(), *format == dialog.format))
        .collect();
    let level_entries: Vec<(SharedString, bool)> = (1..=9u8)
        .map(|level| (format!("{} {level}", t("caption.heading")).into(), level == dialog.chapter_level))
        .collect();
    let separator_entries: Vec<(SharedString, bool)> = CHAPTER_SEPARATORS
        .iter()
        .map(|separator| {
            (
                format!("{separator} ({})", t(separator_name(separator))).into(),
                *separator == dialog.separator,
            )
        })
        .collect();
    div()
        .w(px(420.0))
        .flex()
        .flex_col()
        .gap_2()
        .child(line("caption.caption").child(div().flex().items_center().gap_1().child(preview).child(
            text_field(view, 0, "", 220.0, cx),
        )))
        .child(line("caption.label").child(select(
            view,
            RefList::CaptionLabel,
            dialog.label.clone(),
            120.0,
            label_entries,
            cx,
            move |references, index| {
                if let Some(dialog) = caption_dialog(references) {
                    dialog.label = labels[index].clone();
                }
            },
        )))
        .child(
            line("caption.position")
                .child(choice("caption-below", t("caption.below"), !dialog.above).on_click(cx.listener(
                    |view, _: &ClickEvent, _, cx| {
                        edit(view, cx, |references| {
                            if let Some(dialog) = caption_dialog(references) {
                                dialog.above = false;
                            }
                        })
                    },
                )))
                .child(choice("caption-above", t("caption.above"), dialog.above).on_click(cx.listener(
                    |view, _: &ClickEvent, _, cx| {
                        edit(view, cx, |references| {
                            if let Some(dialog) = caption_dialog(references) {
                                dialog.above = true;
                            }
                        })
                    },
                ))),
        )
        .child(check_box("caption-exclude", "caption.exclude_label", dialog.exclude_label, cx, |references| {
            if let Some(dialog) = caption_dialog(references) {
                dialog.exclude_label = !dialog.exclude_label;
            }
        }))
        .child(action_button("caption-new-label", "caption.new_label", cx, |view, cx| {
            if let Some(Dialog::Caption(dialog, _)) = view.references.dialog.take() {
                let text = view.references.field(0).to_string();
                view.references.open(Dialog::Caption(dialog, true), vec![text, String::new()]);
                view.references.focus = 1;
            }
            cx.notify();
        }))
        .child(heading("caption.numbering"))
        .child(line("caption.format").child(select(
            view,
            RefList::CaptionFormat,
            format_sample(dialog.format),
            140.0,
            format_entries,
            cx,
            |references, index| {
                if let Some(dialog) = caption_dialog(references) {
                    dialog.format = CAPTION_FORMATS[index];
                }
            },
        )))
        .child(check_box("caption-chapter", "caption.chapter", dialog.chapter, cx, |references| {
            if let Some(dialog) = caption_dialog(references) {
                dialog.chapter = !dialog.chapter;
            }
        }))
        .when(dialog.chapter, |body| {
            body.child(line("caption.chapter_style").child(select(
                view,
                RefList::ChapterLevel,
                format!("{} {}", t("caption.heading"), dialog.chapter_level),
                120.0,
                level_entries,
                cx,
                |references, index| {
                    if let Some(dialog) = caption_dialog(references) {
                        dialog.chapter_level = index as u8 + 1;
                    }
                },
            )))
            .child(line("caption.separator").child(select(
                view,
                RefList::Separator,
                format!("{} ({})", dialog.separator, t(separator_name(dialog.separator))),
                160.0,
                separator_entries,
                cx,
                |references, index| {
                    if let Some(dialog) = caption_dialog(references) {
                        dialog.separator = CHAPTER_SEPARATORS[index];
                    }
                },
            )))
        })
        .into_any_element()
}

fn separator_name(separator: &str) -> &'static str {
    match separator {
        "-" => "caption.sep.hyphen",
        "." => "caption.sep.period",
        ":" => "caption.sep.colon",
        "—" => "caption.sep.em_dash",
        _ => "caption.sep.en_dash",
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{fixture, open_contract, open_path, type_at_start};
    use super::super::{Dialog, RefAction};
    use jade_office_model::word::references::bookmarks::WordBookmarks;
    use jade_office_model::word::references::crossref::{ReferenceText, ReferenceType};

    /// ブックマーク: a name typed over the selection is added; ジャンプ
    /// selects it again; 削除 takes it away.
    #[gpui::test]
    fn the_bookmark_dialog_adds_goes_to_and_deletes(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_contract(cx);
        let paragraph = type_at_start(&view, cx, "anchor text ");
        view.update(cx, |view, cx| {
            view.document.select((paragraph, 0), (paragraph, 6), 1).unwrap();
            view.insert_at = (paragraph, 6);
            super::super::run(view, RefAction::OpenBookmark, cx);
            assert_eq!(view.references.field(0), "anchor");
        });
        cx.simulate_keystrokes("backspace");
        cx.simulate_input("Clause_1");
        cx.simulate_keystrokes("enter");
        view.update(cx, |view, cx| {
            assert!(view.references.dialog.is_none(), "{:?}", view.references.problem);
            let range = view.document.bookmark_range("Clause_1").expect("the bookmark");
            assert_eq!(range, ((paragraph, 0), (paragraph, 6)));
            view.insert_at = (0, 0);
            view.caret = view.document.place_caret(0, 0);
            super::super::run(view, RefAction::OpenBookmark, cx);
            if let Some(Dialog::Bookmark(dialog)) = view.references.dialog.as_mut() {
                dialog.selected = Some("Clause_1".to_string());
            }
            view.references.problem = None;
        });
        view.update(cx, |view, cx| {
            // ジャンプ, as its button does.
            let (start, end) = view.document.bookmark_range("Clause_1").unwrap();
            view.reveal(start, end, cx);
            assert_eq!(view.document.selection(), Some(((paragraph, 0), (paragraph, 6))));
            assert!(view.document.remove_bookmark("Clause_1").is_ok());
            assert!(view.document.bookmark_range("Clause_1").is_none());
        });
    }

    /// 相互参照 to a heading's text and its page, and to a footnote's number.
    #[gpui::test]
    fn cross_references_go_in_from_the_dialog(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_path(cx, &fixture("toc.docx"));
        view.update(cx, |view, cx| {
            let last = view.document.paragraph_count() - 1;
            let end = view.document.paragraph_text(last).unwrap().len();
            view.insert_at = (last, end);
            view.caret = view.document.place_caret(last, end);
            super::super::run(view, RefAction::OpenCrossReference, cx);
            let Some(Dialog::CrossReference(dialog)) = view.references.dialog.as_mut() else {
                panic!("the cross-reference dialog");
            };
            assert_eq!(dialog.kind, ReferenceType::Heading);
            dialog.selected = Some(0);
            super::super::confirm(view, cx);
            assert!(view.references.dialog.is_none(), "{:?}", view.references.problem);
            let xml = view.document.export_docx().unwrap();
            let package = jade_opc::OpcPackage::open(&xml).unwrap();
            let body = package.part_str("word/document.xml").unwrap().unwrap();
            assert!(body.contains("REF _Ref"), "a REF field");

            super::super::run(view, RefAction::OpenCrossReference, cx);
            let Some(Dialog::CrossReference(dialog)) = view.references.dialog.as_mut() else {
                panic!("the cross-reference dialog");
            };
            dialog.text = ReferenceText::Page;
            dialog.hyperlink = false;
            dialog.selected = Some(0);
            super::super::confirm(view, cx);
            let xml = view.document.export_docx().unwrap();
            let package = jade_opc::OpcPackage::open(&xml).unwrap();
            let body = package.part_str("word/document.xml").unwrap().unwrap();
            assert!(body.contains("PAGEREF _Ref"), "a PAGEREF field");
            assert!(body.contains("PAGEREF _Ref") && body.matches(" \\h ").count() >= 1);
        });
    }

    /// 図表番号の挿入: 図 with the chapter's number, above the item; a new
    /// label joins the list.
    #[gpui::test]
    fn captions_go_in_with_chapter_numbers_and_new_labels(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_path(cx, &fixture("toc.docx"));
        view.update(cx, |view, cx| {
            let heading = jade_office_model::word::references::navigation::WordNavigation::outline(
                &view.document,
            )[0]
            .paragraph;
            view.insert_at = (heading + 1, 0);
            view.caret = view.document.place_caret(heading + 1, 0);
            super::super::run(view, RefAction::OpenCaption, cx);
            let Some(Dialog::Caption(dialog, false)) = view.references.dialog.as_mut() else {
                panic!("the caption dialog");
            };
            assert_eq!(dialog.labels[..3], ["図", "表", "数式"]);
            dialog.chapter = true;
            dialog.above = true;
        });
        cx.simulate_input("構成");
        cx.simulate_keystrokes("enter");
        view.update(cx, |view, cx| {
            assert!(view.references.dialog.is_none(), "{:?}", view.references.problem);
            // What the page shows: the fields' results with the text.
            let caption: String = view
                .document
                .word_document()
                .paragraph(view.insert_at.0)
                .unwrap()
                .runs
                .iter()
                .map(|run| match &run.field {
                    Some(field) if run.text.is_empty() => field
                        .resolved_value
                        .clone()
                        .or_else(|| field.cached_text.clone())
                        .unwrap_or_default(),
                    _ => run.text.clone(),
                })
                .collect();
            assert!(caption.starts_with("図 1-1"), "{caption}");
            assert!(caption.ends_with("構成"), "{caption}");

            super::super::run(view, RefAction::OpenCaption, cx);
            if let Some(Dialog::Caption(dialog, _)) = view.references.dialog.take() {
                view.references
                    .open(Dialog::Caption(dialog, true), vec![String::new(), "写真".to_string()]);
            }
            super::super::confirm(view, cx);
            let Some(Dialog::Caption(dialog, false)) = view.references.dialog.as_ref() else {
                panic!("back in the caption dialog");
            };
            assert_eq!(dialog.label, "写真");
            assert!(view.references.custom_labels.contains(&"写真".to_string()));
            super::super::cancel(view);
        });
    }
}

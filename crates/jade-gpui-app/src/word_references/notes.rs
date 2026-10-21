//! 脚注と文末脚注: the dialog — 場所, 番号書式, 開始番号, 番号の付け方,
//! 変更の対象, 変換… — and the tab's 次の脚注 and 注の表示.

use gpui::{div, prelude::*, AnyElement, ClickEvent, Context, SharedString};
use jade_office_model::word::references::notes::{
    format_sample, EndnotePlace, FootnotePlace, NoteConversion, NoteDialog, NoteScope,
    WordNotes, NOTE_FORMATS,
};
use jade_office_model::word::NoteKind;

use super::{
    action_button, check_box, choice, edit, line, select, t, text_field, Dialog, RefList,
    References,
};
use crate::DocumentView;

/// Opens 脚注と文末脚注 on a kind.
pub fn open(view: &mut DocumentView, kind: NoteKind, cx: &mut Context<DocumentView>) {
    let dialog = view.document.note_dialog(kind);
    let start = dialog.start.to_string();
    view.references.open(Dialog::Notes(dialog), vec![start]);
    cx.notify();
}

/// 次の脚注 / 前の脚注 and the endnote twins: the mark selected, as Word
/// moves to it.
pub fn step(view: &mut DocumentView, kind: NoteKind, forwards: bool, cx: &mut Context<DocumentView>) {
    let from = match view.document.selection() {
        Some((anchor, focus)) if forwards => anchor.max(focus),
        Some((anchor, focus)) => anchor.min(focus),
        None => view.insert_at,
    };
    match view.document.step_note(kind, from, forwards) {
        Some(mark) => view.reveal(mark.at, mark.after(), cx),
        None => {
            view.table_status = Some(SharedString::from(t(match kind {
                NoteKind::Footnote => "notes.no_more_footnotes",
                NoteKind::Endnote => "notes.no_more_endnotes",
            })));
            cx.notify();
        }
    }
}

/// 注の表示: between a reference and its note.
pub fn show(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    match view.document.show_note(view.insert_at) {
        Some(at) => view.reveal(at, at, cx),
        None => {
            view.table_status = Some(SharedString::from(t("notes.none")));
            cx.notify();
        }
    }
}

fn note_form(references: &mut References) -> Option<&mut NoteDialog> {
    match references.dialog.as_mut() {
        Some(Dialog::Notes(dialog)) => Some(dialog),
        _ => None,
    }
}

/// The dialog with the start typed into it.
fn with_start(view: &DocumentView, mut dialog: NoteDialog) -> Result<NoteDialog, String> {
    let typed = view.references.field(0).trim();
    if !typed.is_empty() {
        dialog.start = typed
            .chars()
            .map(|ch| match ch {
                '０'..='９' => char::from(b'0' + (ch as u32 - '０' as u32) as u8),
                other => other,
            })
            .collect::<String>()
            .parse()
            .map_err(|_| t("notes.bad_start").to_string())?;
    }
    Ok(dialog)
}

/// 適用 (or 挿入: the settings, then the note's bar at the caret).
pub fn confirm(
    view: &mut DocumentView,
    dialog: NoteDialog,
    insert: bool,
    cx: &mut Context<DocumentView>,
) {
    let dialog = match with_start(view, dialog.clone()) {
        Ok(dialog) => dialog,
        Err(problem) => {
            view.references.dialog = Some(Dialog::Notes(dialog));
            view.references.problem = Some(problem);
            return cx.notify();
        }
    };
    let done = view.document.apply_note_dialog(&dialog);
    match done {
        Ok(()) => {
            view.references.close();
            view.after_reference_edit(Ok(None), cx);
            if insert {
                view.open_note_dialog(dialog.kind, cx);
            }
        }
        Err(problem) => {
            view.references.dialog = Some(Dialog::Notes(dialog));
            view.references.problem = Some(problem);
            cx.notify();
        }
    }
}

/// 変換's OK.
pub fn confirm_convert(
    view: &mut DocumentView,
    conversion: NoteConversion,
    back: NoteDialog,
    cx: &mut Context<DocumentView>,
) {
    match view.document.convert_all_notes(conversion) {
        Ok(()) => {
            let start = back.start.to_string();
            view.references.open(Dialog::Notes(back), vec![start]);
            view.after_reference_edit(Ok(None), cx);
        }
        Err(problem) => {
            view.references.dialog = Some(Dialog::Convert(conversion, back));
            view.references.problem = Some(problem);
            cx.notify();
        }
    }
}

/// The 脚注と文末脚注 dialog's body.
pub fn body(view: &DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    let Some(Dialog::Notes(dialog)) = view.references.dialog.as_ref() else {
        return div().into_any_element();
    };
    let kind = dialog.kind;
    let formats: Vec<(SharedString, bool)> = NOTE_FORMATS
        .iter()
        .map(|format| (format_sample(format).into(), *format == dialog.format))
        .collect();
    let restarts = dialog.restarts();
    let restart_entries: Vec<(SharedString, bool)> = restarts
        .iter()
        .map(|restart| (t(restart.label_key()).into(), *restart == dialog.restart))
        .collect();
    let place = match kind {
        NoteKind::Footnote => line("notes.place")
            .child(select(
                view,
                RefList::Place,
                t(match dialog.footnote_place {
                    FootnotePlace::PageBottom => "notes.page_bottom",
                    FootnotePlace::BeneathText => "notes.beneath_text",
                }),
                150.0,
                vec![
                    (t("notes.page_bottom").into(), dialog.footnote_place == FootnotePlace::PageBottom),
                    (t("notes.beneath_text").into(), dialog.footnote_place == FootnotePlace::BeneathText),
                ],
                cx,
                |references, index| {
                    if let Some(dialog) = note_form(references) {
                        dialog.footnote_place = if index == 0 {
                            FootnotePlace::PageBottom
                        } else {
                            FootnotePlace::BeneathText
                        };
                    }
                },
            )),
        NoteKind::Endnote => line("notes.place")
            .child(select(
                view,
                RefList::Place,
                t(match dialog.endnote_place {
                    EndnotePlace::SectionEnd => "notes.section_end",
                    EndnotePlace::DocumentEnd => "notes.document_end",
                }),
                150.0,
                vec![
                    (t("notes.section_end").into(), dialog.endnote_place == EndnotePlace::SectionEnd),
                    (t("notes.document_end").into(), dialog.endnote_place == EndnotePlace::DocumentEnd),
                ],
                cx,
                |references, index| {
                    if let Some(dialog) = note_form(references) {
                        dialog.endnote_place = if index == 0 {
                            EndnotePlace::SectionEnd
                        } else {
                            EndnotePlace::DocumentEnd
                        };
                    }
                },
            )),
    };
    let sample = format_sample(&dialog.format);
    let scope_choices = div()
        .flex()
        .gap_3()
        .child(
            choice("scope-whole", t("notes.scope.whole"), dialog.scope == NoteScope::WholeDocument)
                .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
                    edit(view, cx, |references| {
                        if let Some(dialog) = note_form(references) {
                            dialog.scope = NoteScope::WholeDocument;
                        }
                    })
                })),
        )
        .when(dialog.sections > 1, |line| {
            line.child(
                choice("scope-section", t("notes.scope.section"), dialog.scope == NoteScope::ThisSection)
                    .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
                        edit(view, cx, |references| {
                            if let Some(dialog) = note_form(references) {
                                dialog.scope = NoteScope::ThisSection;
                            }
                        })
                    })),
            )
        });
    div()
        .w(gpui::px(420.0))
        .flex()
        .flex_col()
        .gap_2()
        .child(
            line("notes.kind")
                .child(choice("kind-footnote", t("notes.footnotes"), kind == NoteKind::Footnote).on_click(
                    cx.listener(|view, _: &ClickEvent, _, cx| switch(view, NoteKind::Footnote, cx)),
                ))
                .child(choice("kind-endnote", t("notes.endnotes"), kind == NoteKind::Endnote).on_click(
                    cx.listener(|view, _: &ClickEvent, _, cx| switch(view, NoteKind::Endnote, cx)),
                )),
        )
        .child(place)
        .child(line("notes.format").child(select(
            view,
            RefList::NoteFormat,
            sample,
            150.0,
            formats,
            cx,
            |references, index| {
                if let Some(dialog) = note_form(references) {
                    dialog.format = NOTE_FORMATS[index].to_string();
                }
            },
        )))
        .child(text_field(view, 0, "notes.start", 60.0, cx))
        .child(line("notes.restart").child(select(
            view,
            RefList::Restart,
            t(dialog.restart.label_key()),
            180.0,
            restart_entries,
            cx,
            move |references, index| {
                if let Some(dialog) = note_form(references) {
                    dialog.restart = dialog.restarts()[index];
                }
            },
        )))
        .child(line("notes.scope").child(scope_choices))
        .child(
            line("notes.separators")
                .child(check_box("notes-separator", "notes.separator", dialog.separator, cx, |references| {
                    if let Some(dialog) = note_form(references) {
                        dialog.separator = !dialog.separator;
                    }
                }))
                .child(check_box(
                    "notes-continuation-separator",
                    "notes.continuation_separator",
                    dialog.continuation_separator,
                    cx,
                    |references| {
                        if let Some(dialog) = note_form(references) {
                            dialog.continuation_separator = !dialog.continuation_separator;
                        }
                    },
                )),
        )
        .child(
            div()
                .flex()
                .justify_between()
                .child(action_button("notes-convert", "notes.convert", cx, |view, cx| {
                    if let Some(Dialog::Notes(dialog)) = view.references.dialog.take() {
                        view.references
                            .open(Dialog::Convert(NoteConversion::FootnotesToEndnotes, dialog), Vec::new());
                    }
                    cx.notify();
                }))
                .child(action_button("notes-insert", "notes.insert", cx, |view, cx| {
                    if let Some(Dialog::Notes(dialog)) = view.references.dialog.take() {
                        super::sync_selection(view);
                        confirm(view, dialog, true, cx);
                    }
                })),
        )
        .into_any_element()
}

/// Switches the dialog between 脚注 and 文末脚注, its settings read again.
fn switch(view: &mut DocumentView, kind: NoteKind, cx: &mut Context<DocumentView>) {
    if let Some(Dialog::Notes(mut dialog)) = view.references.dialog.take() {
        view.document.switch_note_dialog(&mut dialog, kind);
        let start = dialog.start.to_string();
        view.references.open(Dialog::Notes(dialog), vec![start]);
    }
    cx.notify();
}

/// 変換's body: the three conversions.
pub fn convert_body(view: &DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    let Some(Dialog::Convert(current, _)) = view.references.dialog.as_ref() else {
        return div().into_any_element();
    };
    let current = *current;
    div()
        .flex()
        .flex_col()
        .gap_2()
        .children(
            [
                (NoteConversion::FootnotesToEndnotes, "notes.convert.to_endnotes"),
                (NoteConversion::EndnotesToFootnotes, "notes.convert.to_footnotes"),
                (NoteConversion::Swap, "notes.convert.swap"),
            ]
            .into_iter()
            .enumerate()
            .map(|(index, (conversion, label))| {
                choice(("convert", index), t(label), conversion == current)
                    .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                        edit(view, cx, |references| {
                            if let Some(Dialog::Convert(chosen, _)) = references.dialog.as_mut() {
                                *chosen = conversion;
                            }
                        })
                    }))
                    .into_any_element()
            }),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::super::tests::{fixture, open_path, type_at_start, open_contract};
    use super::super::{Dialog, RefAction};
    use jade_office_model::word::references::notes::{
        FootnotePlace, NoteConversion, NoteRestart, WordNotes,
    };
    use jade_office_model::word::{NoteKind, WordReferences};

    /// ⌥⌘F puts a footnote in; Backspace just after its mark takes the
    /// note away, and the notes after it are numbered again.
    #[gpui::test]
    fn backspace_after_a_mark_deletes_its_note(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_contract(cx);
        let paragraph = type_at_start(&view, cx, "claim ");
        let before = view.read_with(cx, |view, _| view.document.note_texts(NoteKind::Footnote).len());
        cx.simulate_keystrokes("cmd-alt-f");
        cx.simulate_input("first");
        cx.simulate_keystrokes("enter");
        let caret = view.read_with(cx, |view, _| {
            assert_eq!(view.document.note_texts(NoteKind::Footnote).len(), before + 1);
            let mark = view
                .document
                .note_marks()
                .into_iter()
                .find(|mark| mark.at.0 == paragraph)
                .expect("the mark");
            assert_eq!(view.insert_at, mark.after(), "the caret follows the mark");
            view.insert_at
        });
        assert_eq!(caret.0, paragraph);
        cx.simulate_keystrokes("backspace");
        view.read_with(cx, |view, _| {
            assert_eq!(view.document.note_texts(NoteKind::Footnote).len(), before);
            assert!(view
                .document
                .note_marks()
                .iter()
                .all(|mark| mark.at.0 != paragraph));
        });
    }

    /// 脚注と文末脚注: footnotes numbered i, ii, iii from 3, restarting each
    /// page, under the text; then every footnote made an endnote.
    #[gpui::test]
    fn the_notes_dialog_sets_numbering_and_converts(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_path(cx, &fixture("footnote.docx"));
        let footnotes = view.read_with(cx, |view, _| view.document.note_texts(NoteKind::Footnote).len());
        assert!(footnotes > 0);
        view.update(cx, |view, cx| {
            super::super::run(view, RefAction::OpenNotes, cx);
            let Some(Dialog::Notes(dialog)) = view.references.dialog.as_mut() else {
                panic!("the notes dialog");
            };
            dialog.format = "lowerRoman".to_string();
            dialog.restart = NoteRestart::EachPage;
            dialog.footnote_place = FootnotePlace::BeneathText;
            dialog.separator = false;
            view.references.focus = 0;
        });
        cx.simulate_keystrokes("backspace");
        cx.simulate_input("3");
        cx.simulate_keystrokes("enter");
        view.read_with(cx, |view, _| {
            assert!(view.references.dialog.is_none(), "{:?}", view.references.problem);
            let dialog = view.document.note_dialog(NoteKind::Footnote);
            assert_eq!(dialog.format, "lowerRoman");
            assert_eq!(dialog.start, 3);
            assert_eq!(dialog.restart, NoteRestart::EachPage);
            assert_eq!(dialog.footnote_place, FootnotePlace::BeneathText);
            let xml = view.document.export_docx().unwrap();
            let package = jade_opc::OpcPackage::open(&xml).unwrap();
            let body = package.part_str("word/document.xml").unwrap().unwrap();
            assert!(body.contains(r#"<w:numFmt w:val="lowerRoman"/>"#), "{body}");
            assert!(body.contains(r#"<w:numStart w:val="3"/>"#));
            assert!(body.contains(r#"<w:pos w:val="beneathText"/>"#));
            // 境界線 off: the separator note is emptied.
            assert!(!dialog.separator);
            assert!(dialog.continuation_separator);
            let notes = package.part_str("word/footnotes.xml").unwrap().unwrap();
            assert!(!notes.contains("<w:separator/>"), "{notes}");
        });
        view.update(cx, |view, cx| {
            super::super::run(view, RefAction::OpenNotes, cx);
            if let Some(Dialog::Notes(dialog)) = view.references.dialog.take() {
                view.references
                    .open(Dialog::Convert(NoteConversion::FootnotesToEndnotes, dialog), Vec::new());
            }
            super::super::confirm(view, cx);
            super::super::cancel(view);
        });
        view.read_with(cx, |view, _| {
            assert_eq!(view.document.note_texts(NoteKind::Footnote).len(), 0);
            assert!(view.document.note_texts(NoteKind::Endnote).len() >= footnotes);
        });
    }

    /// 次の脚注 selects the next mark; 注の表示 goes to its note's text and
    /// back.
    #[gpui::test]
    fn next_footnote_and_show_notes_move_between_marks_and_notes(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_path(cx, &fixture("footnote.docx"));
        view.update(cx, |view, cx| {
            view.insert_at = (0, 0);
            view.caret = view.document.place_caret(0, 0);
            super::super::run(view, RefAction::StepNote(NoteKind::Footnote, true), cx);
            let mark = view.document.note_marks()[0].clone();
            assert_eq!(view.insert_at, mark.after());
            super::super::run(view, RefAction::ShowNotes, cx);
            let text = view
                .document
                .note_text_position(NoteKind::Footnote, &mark.id)
                .unwrap();
            assert_eq!(view.insert_at, text);
            super::super::run(view, RefAction::ShowNotes, cx);
            assert_eq!(view.insert_at, mark.after());
        });
    }
}

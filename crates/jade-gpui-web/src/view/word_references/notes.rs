//! 脚注と文末脚注: the dialog — 場所, 番号書式, 開始番号, 番号の付け方,
//! 変更の対象, 変換… — and the tab's 次の脚注 and 注の表示.

use gpui::{div, prelude::*, AnyElement, ClickEvent, Context, SharedString};
use jade_web::word::references::notes::{
    format_sample, EndnotePlace, FootnotePlace, NoteConversion, NoteDialog, NoteScope,
    WordNotes, NOTE_FORMATS,
};
use jade_web::word::NoteKind;

use super::{
    action_button, check_box, choice, edit, line, select, t, text_field, Dialog, RefList,
    References,
};
use crate::view::DocView as DocumentView;
#[allow(unused_imports)]
use jade_web::editing::WordSession;

/// Opens 脚注と文末脚注 on a kind.
pub fn open(view: &mut DocumentView, kind: NoteKind, cx: &mut Context<DocumentView>) {
    let dialog = view.doc().note_dialog(kind);
    let start = dialog.start.to_string();
    view.references.open(Dialog::Notes(dialog), vec![start]);
    cx.notify();
}

/// 次の脚注 / 前の脚注 and the endnote twins: the mark selected, as Word
/// moves to it.
pub fn step(view: &mut DocumentView, kind: NoteKind, forwards: bool, cx: &mut Context<DocumentView>) {
    let from = match view.doc().selection() {
        Some((anchor, focus)) if forwards => anchor.max(focus),
        Some((anchor, focus)) => anchor.min(focus),
        None => view.caret_at(),
    };
    match view.doc().step_note(kind, from, forwards) {
        Some(mark) => view.reveal(mark.at, mark.after(), cx),
        None => {
            view.set_status(SharedString::from(t(match kind {
                NoteKind::Footnote => "notes.no_more_footnotes",
                NoteKind::Endnote => "notes.no_more_endnotes",
            })));
            cx.notify();
        }
    }
}

/// 注の表示: between a reference and its note.
pub fn show(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    match view.doc().show_note(view.caret_at()) {
        Some(at) => view.reveal(at, at, cx),
        None => {
            view.set_status(SharedString::from(t("notes.none")));
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
    let done = view.doc_mut().apply_note_dialog(&dialog);
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
    match view.doc_mut().convert_all_notes(conversion) {
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
        view.doc().switch_note_dialog(&mut dialog, kind);
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

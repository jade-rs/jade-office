//! Footnotes and endnotes as Word manages them: deleting a note's
//! reference mark — with Backspace, or inside a selection — deletes the
//! note, and the notes after it are numbered again.

mod common;

use jade_web::editing::{EditableDocument, WordSession};
use jade_web::word::{NoteForm, NoteKind, WordReferences};

fn note(document: &mut EditableDocument, kind: NoteKind, at: (usize, usize), text: &str) {
    document.place_caret(at.0, at.1);
    let mut form = NoteForm::new(document, kind);
    form.text.insert(text);
    form.submit(document).unwrap();
}

/// Each reference mark in the paragraph: its note, and where it is drawn.
fn marks(document: &EditableDocument, paragraph: usize) -> Vec<(String, usize)> {
    let mut rendered = 0;
    document
        .word_document()
        .reference_marker_spans(paragraph)
        .into_iter()
        .map(|span| {
            let at = span.model_offset + rendered;
            rendered += span.rendered_len;
            (span.note_id, at)
        })
        .collect()
}

#[test]
fn deleting_a_reference_mark_deletes_its_note() {
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "one two three ");
    let before = document.note_texts(NoteKind::Footnote);
    note(&mut document, NoteKind::Footnote, (paragraph, 3), "first");
    note(&mut document, NoteKind::Footnote, (paragraph, 8), "second");
    note(&mut document, NoteKind::Footnote, (paragraph, 14), "third");
    let added = |document: &EditableDocument| {
        document
            .note_texts(NoteKind::Footnote)
            .into_iter()
            .filter(|text| !before.contains(text))
            .collect::<Vec<_>>()
    };
    assert_eq!(added(&document), ["first", "second", "third"]);
    let mark = marks(&document, paragraph)[0].1;

    // Backspace just after the first mark takes the mark and its note.
    document.place_caret(paragraph, mark + 1);
    document.delete(true).unwrap();
    assert_eq!(added(&document), ["second", "third"]);
    assert_eq!(marks(&document, paragraph).len(), 2);

    // A selection over the next mark takes that note too.
    let (_, second) = marks(&document, paragraph)[0].clone();
    document
        .select((paragraph, second - 2), (paragraph, second + 2))
        .unwrap();
    document.delete(true).unwrap();
    assert_eq!(added(&document), ["third"]);
    assert_eq!(marks(&document, paragraph).len(), 1);

    // The note left is numbered first now: the footnotes' part has it alone.
    let saved = document.export_docx().unwrap();
    let package = jade_opc::OpcPackage::open(&saved).unwrap();
    let footnotes = package.part_str("word/footnotes.xml").unwrap().unwrap();
    assert!(footnotes.contains("third"));
    assert!(!footnotes.contains("second"));

    // Each deletion is one undo step.
    document.undo().unwrap();
    assert_eq!(added(&document), ["second", "third"]);
    document.undo().unwrap();
    assert_eq!(added(&document), ["first", "second", "third"]);
}

#[test]
fn deleting_an_endnote_mark_deletes_the_endnote() {
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "end notes ");
    let before = document.note_texts(NoteKind::Endnote).len();
    note(&mut document, NoteKind::Endnote, (paragraph, 3), "巻末");
    assert_eq!(document.note_texts(NoteKind::Endnote).len(), before + 1);
    let (_, at) = marks(&document, paragraph)[0].clone();
    document.place_caret(paragraph, at);
    document.delete(false).unwrap();
    assert_eq!(document.note_texts(NoteKind::Endnote).len(), before);
}

#[test]
fn the_notes_dialog_numbers_and_converts_notes() {
    use jade_web::word::references::notes::{NoteConversion, NoteRestart, WordNotes};
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "a b c ");
    note(&mut document, NoteKind::Footnote, (paragraph, 1), "one");
    note(&mut document, NoteKind::Footnote, (paragraph, 4), "two");
    let mut dialog = document.note_dialog(NoteKind::Footnote);
    dialog.format = "upperRoman".to_string();
    dialog.start = 4;
    dialog.restart = NoteRestart::EachSection;
    document.apply_note_dialog(&dialog).unwrap();
    let read = document.note_dialog(NoteKind::Footnote);
    assert_eq!((read.format.as_str(), read.start, read.restart), ("upperRoman", 4, NoteRestart::EachSection));
    // The marks are drawn in the new format: IV and V.
    let text_marks: Vec<usize> = document.note_marks().iter().map(|mark| mark.len).collect();
    assert!(text_marks.contains(&2), "{text_marks:?}");

    // Next and previous footnote, and 注の表示.
    let first = document.step_note(NoteKind::Footnote, (paragraph, 0), true).unwrap();
    let second = document.step_note(NoteKind::Footnote, first.after(), true).unwrap();
    assert!(second.at > first.at);
    assert_eq!(document.step_note(NoteKind::Footnote, second.at, false), Some(first.clone()));
    let text = document.show_note(first.after()).unwrap();
    assert_eq!(document.note_at(text).map(|(_, id)| id), Some(first.id.clone()));
    assert_eq!(document.show_note(text), Some(first.after()));

    let footnotes = document.note_texts(NoteKind::Footnote).len();
    let endnotes = document.note_texts(NoteKind::Endnote).len();
    document.convert_all_notes(NoteConversion::FootnotesToEndnotes).unwrap();
    assert_eq!(document.note_texts(NoteKind::Footnote).len(), 0);
    assert_eq!(document.note_texts(NoteKind::Endnote).len(), endnotes + footnotes);
    document.undo().unwrap();
    assert_eq!(document.note_texts(NoteKind::Footnote).len(), footnotes);
}

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
    assert_eq!(
        (read.format.as_str(), read.start, read.restart),
        ("upperRoman", 4, NoteRestart::EachSection)
    );
    // The marks are drawn in the new format: IV and V.
    let text_marks: Vec<usize> = document.note_marks().iter().map(|mark| mark.len).collect();
    assert!(text_marks.contains(&2), "{text_marks:?}");

    // Next and previous footnote, and 注の表示.
    let first = document
        .step_note(NoteKind::Footnote, (paragraph, 0), true)
        .unwrap();
    let second = document
        .step_note(NoteKind::Footnote, first.after(), true)
        .unwrap();
    assert!(second.at > first.at);
    assert_eq!(
        document.step_note(NoteKind::Footnote, second.at, false),
        Some(first.clone())
    );
    let text = document.show_note(first.after()).unwrap();
    assert_eq!(
        document.note_at(text).map(|(_, id)| id),
        Some(first.id.clone())
    );
    assert_eq!(document.show_note(text), Some(first.after()));

    let footnotes = document.note_texts(NoteKind::Footnote).len();
    let endnotes = document.note_texts(NoteKind::Endnote).len();
    document
        .convert_all_notes(NoteConversion::FootnotesToEndnotes)
        .unwrap();
    assert_eq!(document.note_texts(NoteKind::Footnote).len(), 0);
    assert_eq!(
        document.note_texts(NoteKind::Endnote).len(),
        endnotes + footnotes
    );
    document.undo().unwrap();
    assert_eq!(document.note_texts(NoteKind::Footnote).len(), footnotes);
}

/// Word types behind a reference mark when the caret is after it: at the
/// end of a paragraph ("Hello¹|"), where the caret used to be refused as
/// past the paragraph's end, and inside one ("abc¹|def"), where the text
/// went in front of the mark.
#[test]
fn text_typed_after_a_reference_mark_goes_behind_it() {
    let bytes = std::fs::read(common::core().join("public/fixtures/footnote.docx")).unwrap();
    let mut document = common::open_bytes(&bytes);
    assert_eq!(document.paragraph_text(0).as_deref(), Some("Hello"));
    document.place_caret(0, 0);
    document
        .move_caret(jade_web::editing::Motion::LineEnd, false)
        .unwrap();
    // "Hello" and the mark.
    assert_eq!(document.caret(), Some((0, "Hello".len() + 1)));
    document.insert_text(" world").unwrap();
    assert_eq!(document.paragraph_text(0).as_deref(), Some("Hello world"));
    assert_eq!(
        marks(&document, 0)
            .iter()
            .map(|(_, at)| *at)
            .collect::<Vec<_>>(),
        vec![5]
    );
    assert_eq!(
        document.caret(),
        Some((0, "Hello¹ world".len() - "¹".len() + 1))
    );

    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "abcdef ");
    note(&mut document, NoteKind::Footnote, (paragraph, 3), "note");
    // Just behind the mark: "abc" and its one-character number.
    document.place_caret(paragraph, 4);
    document.insert_text("X").unwrap();
    let text = document.paragraph_text(paragraph).unwrap();
    assert!(text.starts_with("abcXdef"), "{text}");
    // The mark stays after "abc", in front of what was typed.
    assert_eq!(marks(&document, paragraph)[0].1, 3);
    let saved = document.export_docx().unwrap();
    let package = jade_opc::OpcPackage::open(&saved).unwrap();
    let xml = package.part_str("word/document.xml").unwrap().unwrap();
    let reference = xml.find("<w:footnoteReference").unwrap();
    let typed = xml.find(">X</w:t>").unwrap();
    assert!(reference < typed, "the mark comes first");
}

/// Enter after a reference mark at the end of a paragraph starts the next
/// paragraph without the mark, as in Word: the mark stays with its text.
#[test]
fn enter_after_a_reference_mark_leaves_the_mark_behind() {
    let bytes = std::fs::read(common::core().join("public/fixtures/footnote.docx")).unwrap();
    let mut document = common::open_bytes(&bytes);
    document.place_caret(0, "Hello".len() + 1);
    document.insert_paragraph_break().unwrap();
    // The caret is in the new body paragraph, past the note's own.
    let (next, offset) = document.caret().unwrap();
    assert_eq!(offset, 0);
    assert!(next > 1 && document.is_body_paragraph(next), "{next}");
    assert_eq!(marks(&document, 0).len(), 1, "the mark stays on 'Hello'");
    assert!(marks(&document, next).is_empty());
    document.insert_text("next").unwrap();
    assert_eq!(document.paragraph_text(next).as_deref(), Some("next"));
    assert_eq!(document.paragraph_text(0).as_deref(), Some("Hello"));
    let saved = document.export_docx().unwrap();
    let package = jade_opc::OpcPackage::open(&saved).unwrap();
    let xml = package.part_str("word/document.xml").unwrap().unwrap();
    let reference = xml.find("<w:footnoteReference").unwrap();
    let hello = xml.find(">Hello<").unwrap();
    let next = xml.find(">next<").unwrap();
    assert!(hello < reference && reference < next, "{xml}");
    // Undone, the split goes and the mark is still on "Hello".
    document.undo().unwrap();
    document.undo().unwrap();
    assert_eq!(document.paragraph_text(0).as_deref(), Some("Hello"));
    assert_eq!(marks(&document, 0).len(), 1);
    assert!(!document.is_body_paragraph(2));
    // In front of the mark, Enter takes it to the next paragraph.
    let mut document = common::open_bytes(&bytes);
    document.place_caret(0, "Hello".len());
    document.insert_paragraph_break().unwrap();
    assert_eq!(document.caret(), Some((1, 0)));
    assert!(marks(&document, 0).is_empty());
    assert_eq!(marks(&document, 1).len(), 1);
    // Undone, the paragraph is whole again with its mark.
    document.undo().unwrap();
    assert_eq!(document.paragraph_text(0).as_deref(), Some("Hello"));
    assert_eq!(marks(&document, 0).len(), 1);
}

/// A note inserted here starts as Word starts one: its number, then a
/// space in a run of its own and the text, not a tab.
#[test]
fn a_new_note_starts_with_a_space_after_its_number() {
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "abc ");
    note(&mut document, NoteKind::Footnote, (paragraph, 3), "text");
    let saved = document.export_docx().unwrap();
    let package = jade_opc::OpcPackage::open(&saved).unwrap();
    let notes = package.part_str("word/footnotes.xml").unwrap().unwrap();
    let marker = notes.rfind("<w:footnoteRef/>").unwrap();
    let rest = &notes[marker..];
    let text = rest
        .find(r#"<w:t xml:space="preserve"> </w:t></w:r><w:r><w:t>text</w:t>"#)
        .expect("a space before the text");
    assert!(!rest[..text].contains("<w:tab/>"), "{rest}");
    assert!(document
        .note_texts(NoteKind::Footnote)
        .iter()
        .any(|text| text == "text"));
}

/// The arrows step character by character through a note's text and the
/// body text after a note mark, past the note number and the mark as Word
/// does: never into the middle of a character, and on to the note's end.
#[test]
fn arrows_step_whole_characters_past_note_numbers_and_marks() {
    use jade_web::editing::Motion;
    use jade_web::word::headless::HeadlessCommand;
    let mut document = common::contract();
    document
        .apply_commands(
            vec![HeadlessCommand::InsertFootnote {
                paragraph: 3,
                offset: 0,
                text: "日本語".into(),
            }],
            false,
        )
        .unwrap();
    let note = (0..document.word_document().paragraph_count())
        .find(|&index| {
            document
                .word_document()
                .rendered_paragraph_text(index)
                .ends_with(" 日本語")
        })
        .unwrap();
    let text = document.caret_text(note).unwrap();
    assert_eq!(
        text.len(),
        document.word_document().rendered_paragraph_len(note)
    );
    let mut stops = Vec::new();
    document.place_caret(note, 0);
    for _ in 0..5 {
        document.move_caret(Motion::Right, false).unwrap();
        stops.push(document.caret().unwrap());
    }
    let expected: Vec<_> = text
        .char_indices()
        .skip(1)
        .map(|(offset, _)| (note, offset))
        .chain([(note, text.len())])
        .collect();
    assert_eq!(stops, expected);

    // In the body, after the mark: whole characters too.
    let body = document.caret_text(3).unwrap();
    assert!(body.is_char_boundary(1));
    document.place_caret(3, 0);
    let mut stops = Vec::new();
    for _ in 0..3 {
        document.move_caret(Motion::Right, false).unwrap();
        stops.push(document.caret().unwrap().1);
    }
    let expected: Vec<_> = body
        .char_indices()
        .skip(1)
        .take(3)
        .map(|(o, _)| o)
        .collect();
    assert_eq!(stops, expected);
}

/// The text of each footnote in the saved document, in its part's order.
fn saved_footnotes(document: &mut EditableDocument) -> Vec<String> {
    document
        .word_document()
        .footnote_infos()
        .into_iter()
        .map(|note| note.text)
        .collect()
}

#[test]
fn a_copy_after_a_note_takes_the_text_it_shows() {
    use jade_web::word::clipboard::WordClipboard;
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "one two ");
    note(
        &mut document,
        NoteKind::Footnote,
        (paragraph, 3),
        "first note",
    );
    // "one¹ two": the selection counts the number the page draws.
    document.select((paragraph, 5), (paragraph, 8)).unwrap();
    assert_eq!(document.selected_text().as_deref(), Some("two"));
    let clip = document.copy_rich().unwrap();
    assert_eq!(clip.plain_text(), "two");
    assert!(clip.notes.is_empty());
}

#[test]
fn a_pasted_reference_mark_brings_a_note_of_its_own() {
    use jade_web::word::clipboard::WordClipboard;
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "one two ");
    note(
        &mut document,
        NoteKind::Footnote,
        (paragraph, 3),
        "first note",
    );
    let before = saved_footnotes(&mut document).len();

    // Copied with its mark, "one¹" pastes as "one²" with a copy of the note.
    document.select((paragraph, 0), (paragraph, 4)).unwrap();
    let clip = document.copy_rich().unwrap();
    assert_eq!(clip.plain_text(), "one");
    assert_eq!(clip.notes.len(), 1);
    let end = document.word_document().rendered_paragraph_len(paragraph);
    document.place_caret(paragraph, end);
    let after = document.paste_rich(&clip).unwrap();
    let notes = saved_footnotes(&mut document);
    assert_eq!(notes.len(), before + 1, "{notes:?}");
    assert_eq!(
        notes
            .iter()
            .filter(|text| text.contains("first note"))
            .count(),
        2
    );
    let pasted = marks(&document, paragraph);
    assert_eq!(pasted.len(), 2, "{pasted:?}");
    // The caret goes past the pasted number, at the end of the paragraph.
    assert_eq!(
        after,
        (
            paragraph,
            document.word_document().rendered_paragraph_len(paragraph)
        )
    );
    // One undo takes the text and the note back.
    document.undo().unwrap();
    assert_eq!(saved_footnotes(&mut document).len(), before);
    assert_eq!(
        document.word_document().rendered_paragraph_len(paragraph),
        end
    );

    // Cut and pasted, the note moves with its mark.
    document.select((paragraph, 0), (paragraph, 4)).unwrap();
    let clip = document.copy_rich().unwrap();
    document.delete(true).unwrap();
    assert_eq!(saved_footnotes(&mut document).len(), before - 1);
    let end = document.word_document().rendered_paragraph_len(paragraph);
    document.place_caret(paragraph, end);
    document.paste_rich(&clip).unwrap();
    let notes = saved_footnotes(&mut document);
    assert_eq!(notes.len(), before, "{notes:?}");
    assert!(notes.iter().any(|text| text.contains("first note")));
    let text = document.paragraph_text(paragraph).unwrap();
    assert!(text.ends_with("one"), "{text}");
    assert_eq!(marks(&document, paragraph).len(), 1);
}

#[test]
fn a_pasted_note_keeps_its_formatting() {
    use jade_docx_editor::word_document::ParagraphStory;
    use jade_web::editing::{CharacterFormat, WordFormatting};
    use jade_web::word::clipboard::WordClipboard;
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "one two ");
    note(
        &mut document,
        NoteKind::Footnote,
        (paragraph, 3),
        "bold plain",
    );
    let note_paragraph = |document: &EditableDocument, id: &str| {
        document
            .word_document()
            .paragraph_story_surface(&ParagraphStory::Footnote {
                note_id: id.to_string(),
                paragraph: 0,
            })
            .unwrap()
    };
    let first = marks(&document, paragraph)[0].0.clone();
    let surface = note_paragraph(&document, &first);
    let text = document.caret_text(surface).unwrap();
    let bold = text.find("bold").unwrap();
    document
        .select((surface, bold), (surface, bold + 4))
        .unwrap();
    document.toggle_format(CharacterFormat::Bold).unwrap();

    document.select((paragraph, 0), (paragraph, 4)).unwrap();
    let clip = document.copy_rich().unwrap();
    assert!(!clip.notes[0].runs.is_empty());
    let end = document.word_document().rendered_paragraph_len(paragraph);
    document.place_caret(paragraph, end);
    document.paste_rich(&clip).unwrap();

    let pasted = marks(&document, paragraph)[1].0.clone();
    assert_ne!(pasted, first);
    let surface = note_paragraph(&document, &pasted);
    let text = document.caret_text(surface).unwrap();
    let bold = text.find("bold").unwrap();
    let plain = text.find("plain").unwrap();
    assert_eq!(
        document.run_style_at((surface, bold)).unwrap().bold,
        Some(true)
    );
    assert_ne!(
        document.run_style_at((surface, plain)).unwrap().bold,
        Some(true)
    );
    // One undo takes the paste back, its note's formatting with it.
    document.undo().unwrap();
    assert_eq!(marks(&document, paragraph).len(), 1);
}

#[test]
fn a_note_takes_a_pasted_notes_text_without_its_mark() {
    use jade_web::word::clipboard::WordClipboard;
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "noted text ");
    note(
        &mut document,
        NoteKind::Footnote,
        (paragraph, 5),
        "the note",
    );
    document.select((paragraph, 0), (paragraph, 11)).unwrap();
    let clip = document.copy_rich().unwrap();
    assert_eq!(clip.notes.len(), 1);
    assert!(document.can_insert_note());
    let in_note = (0..document.paragraph_count())
        .find(|&index| document.word_document().is_note_paragraph_surface(index))
        .unwrap();

    // Word greys out 脚注の挿入 in a note, and pastes the text alone.
    let end = document.word_document().rendered_paragraph_len(in_note);
    document.place_caret(in_note, end);
    assert!(!document.can_insert_note());
    document.paste_rich(&clip).unwrap();
    assert_eq!(
        document.note_texts(NoteKind::Footnote),
        ["the notenoted text"]
    );
    assert!(document
        .insert_note(NoteKind::Footnote, (in_note, 1), "nested")
        .is_err());
    // One undo takes the paste back.
    document.undo().unwrap();
    assert_eq!(document.note_texts(NoteKind::Footnote), ["the note"]);
}

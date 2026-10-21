//! 参考資料 in the browser build's Word document, as both shells have it: a
//! footnote and an endnote put in at the caret with their text, each one
//! undo step and written into the .docx; and a table of contents inserted
//! from the headings, then updated after a heading is added.

mod common;

use jade_web::editing::WordFormatting;
use jade_web::word::{NoteForm, NoteKind, WordReferences};

#[test]
fn notes_go_in_at_the_caret_with_their_text() {
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "claim ");
    let footnotes = document.note_texts(NoteKind::Footnote).len();
    let endnotes = document.note_texts(NoteKind::Endnote).len();

    document.place_caret(paragraph, 5);
    let mut form = NoteForm::new(&document, NoteKind::Footnote);
    assert_eq!(form.at, (paragraph, 5));
    form.text.insert("See the annex.");
    form.submit(&mut document).unwrap();
    let notes = document.note_texts(NoteKind::Footnote);
    assert_eq!(notes.len(), footnotes + 1);
    assert!(notes.iter().any(|text| text.contains("See the annex.")));

    // After a selection, the reference goes at its end.
    document.select((paragraph, 0), (paragraph, 5)).unwrap();
    let mut form = NoteForm::new(&document, NoteKind::Endnote);
    assert_eq!(form.at, (paragraph, 5));
    form.text.insert("巻末の注");
    form.submit(&mut document).unwrap();
    assert_eq!(document.note_texts(NoteKind::Endnote).len(), endnotes + 1);

    let saved = document.export_docx().unwrap();
    let package = jade_opc::OpcPackage::open(&saved).unwrap();
    let body = package.part_str("word/document.xml").unwrap().unwrap();
    assert!(body.contains("<w:footnoteReference"));
    assert!(body.contains("<w:endnoteReference"));
    let footnotes_xml = package.part_str("word/footnotes.xml").unwrap().unwrap();
    assert!(footnotes_xml.contains("See the annex."));
    let endnotes_xml = package.part_str("word/endnotes.xml").unwrap().unwrap();
    assert!(endnotes_xml.contains("巻末の注"));

    // One undo each.
    document.undo().unwrap();
    assert_eq!(document.note_texts(NoteKind::Endnote).len(), endnotes);
    document.undo().unwrap();
    assert_eq!(document.note_texts(NoteKind::Footnote).len(), footnotes);
}

#[test]
fn the_table_of_contents_is_updated_from_the_headings() {
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "目次の見出し");
    document.place_caret(paragraph, 0);
    document.set_paragraph_style(Some("Heading1")).unwrap();
    let start = document.paragraph_count();
    document.place_caret(0, 0);
    document.insert_table_of_contents().unwrap();
    assert!(document.paragraph_count() > start, "the contents went in");
    let contents = |document: &jade_web::editing::EditableDocument| {
        (0..document.paragraph_count())
            .filter_map(|index| document.paragraph_text(index))
            .take(40)
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert!(contents(&document).contains("目次の見出し"));
    assert!(!contents(&document).contains("新しい見出し"));

    // A heading added later is in the contents once they are updated.
    let last = document.paragraph_count() - 1;
    let end = document.paragraph_text(last).unwrap().len();
    document.place_caret(last, end);
    document.insert_paragraph_break().unwrap();
    document.insert_text("新しい見出し").unwrap();
    document.set_paragraph_style(Some("Heading1")).unwrap();
    document.update_table_of_contents().unwrap();
    assert!(contents(&document).contains("新しい見出し"));
    document.undo().unwrap();
    assert!(!contents(&document).contains("新しい見出し"));
}

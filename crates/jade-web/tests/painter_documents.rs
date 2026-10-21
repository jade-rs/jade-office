//! 書式のコピー/貼り付け over the browser build's Word document, as both
//! shells' paintbrush runs it: the format at a bare caret — its characters
//! and its paragraph — goes onto the text selected next, in one undo step;
//! from a selection inside a paragraph only the characters' format goes.

mod common;

use jade_web::editing::{CharacterFormat, WordFormatting};
use jade_web::word::WordFormatPainter;

/// Another body paragraph with text, after `after`.
fn other_paragraph(document: &jade_web::editing::EditableDocument, after: usize) -> usize {
    use jade_web::editing::WordSession;
    (after + 2..document.paragraph_count())
        .find(|&index| {
            document.cell_id_for_paragraph(index).is_none()
                && document
                    .paragraph_text(index)
                    .is_some_and(|text| text.chars().count() > 6)
        })
        .unwrap()
}

#[test]
fn the_format_at_a_caret_paints_characters_and_paragraph() {
    let mut document = common::contract();
    let source = common::typed(&mut document, "SRC ");
    document.select((source, 0), (source, 3)).unwrap();
    document.toggle_format(CharacterFormat::Bold).unwrap();
    document.toggle_format(CharacterFormat::Italic).unwrap();
    document.set_font_size(20.0).unwrap();
    document.set_font_color("C00000").unwrap();
    document.set_alignment("center").unwrap();

    document.place_caret(source, 1);
    let copied = document.copy_format();
    assert!(
        copied.paragraph.is_some(),
        "a bare caret takes the paragraph too"
    );
    assert_eq!(copied.run.bold, Some(true));

    let target = other_paragraph(&document, source);
    document.place_caret(target, 0);
    let before = document.format_state();
    assert!(!before.bold && before.alignment.as_deref() != Some("center"));
    document.select((target, 0), (target, 6)).unwrap();
    document.paste_format(&copied).unwrap();
    let after = document.format_state();
    assert!(after.bold && after.italic, "{after:?}");
    assert_eq!(after.font_size, Some(20.0));
    assert_eq!(after.alignment.as_deref(), Some("center"));
    assert!(document.selection().is_some(), "the selection stays");

    // One undo takes the whole painting back.
    document.undo().unwrap();
    document.place_caret(target, 0);
    let undone = document.format_state();
    assert_eq!(undone.bold, before.bold);
    assert_eq!(undone.alignment, before.alignment);
}

#[test]
fn a_selection_inside_a_paragraph_paints_only_characters() {
    let mut document = common::contract();
    let source = common::typed(&mut document, "SRC ");
    document.select((source, 0), (source, 3)).unwrap();
    document.toggle_format(CharacterFormat::Underline).unwrap();
    let copied = document.copy_format();
    assert!(copied.paragraph.is_none());
    assert_eq!(copied.run.underline, Some(true));

    // A bare caret in a word paints the word, as a click does in Word.
    let target = other_paragraph(&document, source);
    let alignment = {
        document.place_caret(target, 0);
        document.format_state().alignment
    };
    let text = document.paragraph_text(target).unwrap();
    let inside = text.char_indices().nth(1).unwrap().0;
    document.place_caret(target, inside);
    document.paste_format(&copied).unwrap();
    document.place_caret(target, inside);
    let painted = document.format_state();
    assert!(painted.underline);
    assert_eq!(painted.alignment, alignment);
}

//! What a screen reader is given of the browser build's Word document —
//! what both shells hand the system: the text a paragraph a line, the caret
//! and the selection in it in UTF-16 units, and the caret's paragraph.

mod common;
use common::{contract, typed};
use jade_web::word::screen_reader::screen_reader_text;

#[test]
fn the_text_and_the_caret_are_handed_over_in_utf16() {
    let mut document = contract();
    let paragraph = typed(&mut document, "😀 契約 ");
    let text = document.paragraph_text(paragraph).unwrap();
    // After the emoji (two UTF-16 units, four bytes) and the space.
    document.place_caret(paragraph, "😀 ".len());
    let read = screen_reader_text(&document);
    assert_eq!(read.paragraph, paragraph);
    assert_eq!(read.paragraph_text, text);
    let lines: Vec<&str> = read.text.split('\n').collect();
    assert_eq!(lines[paragraph], text);
    let before: usize = lines[..paragraph]
        .iter()
        .map(|line| line.encode_utf16().count() + 1)
        .sum();
    assert_eq!(read.selection, (before + 3, before + 3));

    document
        .select((paragraph, "😀 ".len()), (paragraph, "😀 契約".len()))
        .unwrap();
    let read = screen_reader_text(&document);
    assert_eq!(read.selection, (before + 3, before + 5));
    let units: Vec<u16> = read.text.encode_utf16().collect();
    assert_eq!(
        String::from_utf16(&units[read.selection.0..read.selection.1]).unwrap(),
        "契約"
    );
}

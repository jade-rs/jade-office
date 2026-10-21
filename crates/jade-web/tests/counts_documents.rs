//! 文字カウント over the browser build's Word document, as both shells'
//! status bars show it: typing changes the document's count as Word would
//! count what was typed — each Japanese character a word — and a selection
//! is counted on its own.

mod common;

use jade_web::word::count::count_text;
use jade_web::word::WordCounting;

#[test]
fn typing_and_selecting_change_the_count_as_word_counts() {
    let mut document = common::contract();
    let before = document.document_count();
    assert!(before.words > 100 && before.characters > before.words / 2);
    assert!(document.selection_count().is_none());

    let paragraph = common::typed(&mut document, "jade 文書 ");
    let after = document.document_count();
    assert_eq!(after.words, before.words + 3);
    assert_eq!(after.characters, before.characters + 6);
    assert_eq!(
        after.characters_with_spaces,
        before.characters_with_spaces + 8
    );

    document
        .select((paragraph, 0), (paragraph, "jade 文書".len()))
        .unwrap();
    let selected = document.selection_count().unwrap();
    assert_eq!((selected.words, selected.characters), (3, 6));

    // Across paragraphs, the whole of each between.
    let next = document.paragraph_text(paragraph + 1).unwrap_or_default();
    let end = next.len();
    document
        .select((paragraph, 0), (paragraph + 1, end))
        .unwrap();
    let across = document.selection_count().unwrap();
    let mut expected = count_text(&document.paragraph_text(paragraph).unwrap());
    expected += count_text(&next);
    assert_eq!(across, expected);
}

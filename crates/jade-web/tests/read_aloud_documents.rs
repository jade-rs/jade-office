//! 校閲 › 音声読み上げ › 読み上げ on the browser build's Word document — what
//! both shells run: the reading starts at the word the caret is in, goes
//! paragraph by paragraph past empty ones, and the voice's UTF-16 word
//! ranges come back as the document's positions to select.

mod common;
use common::{contract, typed};
use jade_web::word::read_aloud::{utterances, ReadAloud};

#[test]
fn reading_starts_at_the_caret_word_and_selects_what_is_said() {
    let mut document = contract();
    let paragraph = typed(&mut document, "日本語 and 😀 words ");
    let text = document.paragraph_text(paragraph).unwrap();
    // From inside "and": the reading starts at the word's start.
    let inside = text.find("and").unwrap() + 1;
    let mut reading = ReadAloud::start(&document, (paragraph, inside), 1.0).unwrap();
    let first = reading.utterance().unwrap().clone();
    assert_eq!(first.paragraph, paragraph);
    assert!(first.text.starts_with("and 😀 words"));
    // The voice says "words": after "and " (4) and the emoji (2) and a space.
    let (start, end) = reading.word(7, 5).unwrap();
    assert_eq!(start, (paragraph, text.find("words").unwrap()));
    assert_eq!(end, (paragraph, text.find("words").unwrap() + 5));

    // Every later paragraph with text is read, in order, none empty.
    let all = utterances(&document, (paragraph, 0));
    assert!(all.len() > 1);
    assert!(all
        .windows(2)
        .all(|pair| pair[0].paragraph < pair[1].paragraph));
    assert!(all
        .iter()
        .all(|utterance| !utterance.text.trim().is_empty()));
    assert!(reading.forward());
    assert_eq!(reading.utterance().unwrap().paragraph, all[1].paragraph);
    reading.back();
    assert_eq!(reading.utterance().unwrap().paragraph, paragraph);
}

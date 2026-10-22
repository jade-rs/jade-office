//! 表示 › イマーシブ リーダー on the browser build's Word document — what
//! both shells show: every paragraph with text, with 音節 and テキストの間隔,
//! and a reading's word found on the line that shows it.

mod common;
use common::{contract, typed};
use jade_web::word::immersive::{
    lines_of, shown_paragraphs, shown_range, ImmersiveSettings, SYLLABLE_MARK,
};

#[test]
fn the_reader_shows_each_paragraph_and_finds_a_word_on_its_line() {
    let mut document = contract();
    let paragraph = typed(&mut document, "Understanding documents ");
    let settings = ImmersiveSettings {
        syllables: true,
        text_spacing: true,
        ..ImmersiveSettings::default()
    };
    let shown = shown_paragraphs(&document, &settings);
    assert!(shown.iter().all(|shown| !shown.text.trim().is_empty()));
    let ours = shown
        .iter()
        .find(|shown| shown.paragraph == paragraph)
        .unwrap();
    assert!(ours.text.starts_with(&format!("Un{SYLLABLE_MARK}der")));
    // Broken into two lines after the first word, "documents" is on the
    // second, whole.
    let first_break = ours.text.find("doc").unwrap();
    let lines = lines_of(ours, &[first_break]);
    let text = document.paragraph_text(paragraph).unwrap();
    let start = text.find("documents").unwrap();
    assert!(shown_range(ours, &lines[0], (start, start + 9)).is_none());
    let (from, to) = shown_range(ours, &lines[1], (start, start + 9)).unwrap();
    assert_eq!(ours.text[from..to].replace(SYLLABLE_MARK, ""), "documents");
}

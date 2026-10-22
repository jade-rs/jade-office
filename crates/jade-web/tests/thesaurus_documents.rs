//! 類義語辞典 on the browser build's Word document — what both shells run:
//! the word at the caret (or the one selected) is what is looked up, and
//! 挿入 puts a synonym in its place with its capitals, as one undo step.

mod common;
use common::{contract, typed};
use jade_web::word::thesaurus::{BuiltinThesaurus, Thesaurus, WordThesaurus};

#[test]
fn the_word_at_the_caret_is_replaced_by_a_synonym() {
    let mut document = contract();
    let paragraph = typed(&mut document, "Begin the review. ");
    document.place_caret(paragraph, 3);
    let target = document.word_for_thesaurus().unwrap();
    assert_eq!(target.2, "Begin");
    assert_eq!((target.0, target.1), ((paragraph, 0), (paragraph, 5)));
    let senses = BuiltinThesaurus.lookup(&target.2);
    assert!(senses[0].synonyms.iter().any(|word| word == "start"));
    document.replace_with_synonym(&target, "start").unwrap();
    assert!(document
        .paragraph_text(paragraph)
        .unwrap()
        .starts_with("Start the review."));
    document.undo().unwrap();
    assert!(document
        .paragraph_text(paragraph)
        .unwrap()
        .starts_with("Begin the review."));

    // A selection is looked up as it is.
    document.select((paragraph, 10), (paragraph, 16)).unwrap();
    assert_eq!(document.word_for_thesaurus().unwrap().2, "review");
}

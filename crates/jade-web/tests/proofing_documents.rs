//! スペル チェックと文章校正 on the browser build's Word document — the
//! commands the native app runs too, with a word-list checker standing in
//! for the platform's: misspellings drawn with a red wave and grammar with
//! a blue double line, and the 文章校正 pane's 変更, すべて変更 and 無視.

mod common;
use common::{contract, typed};
use jade_docx_editor::plugins::spelling::{GRAMMAR_COLOR, SPELLING_COLOR};
use jade_web::editing::EditableDocument;
use jade_web::word::proofing::{
    ProofingKind, ProofingPane, WordListChecker, WordProofing, WordProofingDisplay,
};

fn rgb(hex: &str) -> [u8; 3] {
    let value = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap();
    [(value >> 16) as u8, (value >> 8) as u8, value as u8]
}

fn painted_in(document: &EditableDocument, color: [u8; 3]) -> usize {
    let mut count = 0;
    for index in 0..document.page_count() {
        let page = document.page(index).unwrap();
        for pixel in page.rgba_premultiplied().chunks(4) {
            if pixel[..3]
                .iter()
                .zip(color)
                .all(|(&have, want)| have.abs_diff(want) <= 40)
            {
                count += 1;
            }
        }
    }
    count
}

fn checker() -> WordListChecker {
    WordListChecker::new(["the", "cat", "sat", "on", "mat", "a"])
}

#[test]
fn misspellings_and_grammar_are_marked_as_the_pages_are_drawn() {
    let mut document = contract();
    typed(&mut document, "Teh cat sat on the the mat. ");
    assert_eq!(painted_in(&document, rgb(SPELLING_COLOR)), 0);
    let mut checker = checker();
    document.set_proofing(true).unwrap();
    assert!(document.proof_with(&mut checker).unwrap());
    assert!(
        painted_in(&document, rgb(SPELLING_COLOR)) > 10,
        "a red wave under Teh"
    );
    assert!(
        painted_in(&document, rgb(GRAMMAR_COLOR)) > 10,
        "a blue double line under the the"
    );
    // Asked again with nothing new drawn: nothing to do.
    assert!(!document.proof_with(&mut checker).unwrap());
    document.set_proofing(false).unwrap();
    assert_eq!(painted_in(&document, rgb(SPELLING_COLOR)), 0);
}

#[test]
fn the_pane_walks_the_issues_and_changes_them() {
    let mut document = contract();
    let paragraph = typed(&mut document, "Teh cat sat on the the mat, teh end. ");
    let mut checker = checker();
    let mut pane = ProofingPane::default();
    pane.advance(&document, (paragraph, 0), &mut checker);
    let issue = pane.current.clone().expect("an issue");
    assert_eq!(
        (issue.paragraph, issue.start, issue.text.as_str()),
        (paragraph, 0, "Teh")
    );
    assert_eq!(pane.replacement(), Some("The"));

    // 変更.
    let next = document.change_issue(&issue, "The").unwrap();
    pane.advance(&document, next, &mut checker);
    let issue = pane.current.clone().unwrap();
    assert_eq!(issue.kind, ProofingKind::Grammar);
    assert_eq!(issue.text, "the the");
    assert_eq!(pane.replacement(), Some("the"));
    let next = document.change_issue(&issue, "the").unwrap();
    pane.advance(&document, next, &mut checker);
    let issue = pane.current.clone().unwrap();
    assert_eq!(issue.text, "teh");

    // 無視 passes this one over; すべて変更 then reaches it anyway.
    pane.ignore_once(&document, &mut checker);
    assert_ne!(
        pane.current.as_ref().map(|issue| issue.start()),
        Some(issue.start())
    );
    assert_eq!(document.change_every("teh", "the").unwrap(), 1);
    let text = document.paragraph_text(paragraph).unwrap();
    assert!(
        text.starts_with("The cat sat on the mat, the end. "),
        "{text}"
    );
}

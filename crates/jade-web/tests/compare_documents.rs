//! 校閲 › 比較 on Word documents — what both shells run: two versions made
//! into a third with every difference tracked under the name given, with
//! the formatting and comment settings; 組み込み puts a reviewer's copy's
//! changes down to that reviewer.

mod common;
use common::{contract, open_bytes, typed};
use jade_web::word::compare::{combine_documents, compare_documents, CompareOptions};
use jade_web::word::review::WordReview;
use jade_web::word::reviewer::Reviewer;
use jade_web::word::WordFormatting;

const DATE: &str = "2026-10-22T09:00:00Z";

#[test]
fn comparing_two_versions_tracks_their_differences() {
    let mut original = contract();
    let paragraph = typed(&mut original, "Plain words stay here. ");
    let original_bytes = original.export_docx().unwrap();

    // The revised copy: a sentence added, a word made bold.
    let mut revised = open_bytes(&original_bytes);
    revised.place_caret(paragraph, 0);
    revised.insert_text("Added sentence. ").unwrap();
    let words = "Added sentence. Plain".len();
    revised
        .select((paragraph, "Added sentence. ".len()), (paragraph, words))
        .unwrap();
    revised
        .toggle_format(jade_web::editing::CharacterFormat::Bold)
        .unwrap();
    let revised_bytes = revised.export_docx().unwrap();

    let options = CompareOptions::new("Ann");
    let (result, summary) =
        compare_documents(&original_bytes, &revised_bytes, &options, DATE).unwrap();
    assert!(summary.insertions >= 1, "{summary:?}");
    assert_eq!(
        summary.formatting, 0,
        "the paragraph's text changed: {summary:?}"
    );
    let compared = open_bytes(&result);
    let revisions = compared.revisions();
    assert!(
        revisions.iter().any(|revision| revision.is_insertion()
            && revision.text.contains("Added")
            && revision.author.as_deref() == Some("Ann")),
        "{revisions:?}"
    );

    // Only formatting changed: a formatting change, under Ann's name.
    let mut restyled = open_bytes(&original_bytes);
    restyled
        .select((paragraph, 0), (paragraph, "Plain".len()))
        .unwrap();
    restyled
        .toggle_format(jade_web::editing::CharacterFormat::Italic)
        .unwrap();
    let (result, summary) = compare_documents(
        &original_bytes,
        &restyled.export_docx().unwrap(),
        &options,
        DATE,
    )
    .unwrap();
    assert!(summary.formatting >= 1, "{summary:?}");
    let compared = open_bytes(&result);
    assert!(compared
        .revisions()
        .iter()
        .any(|revision| revision.is_formatting() && revision.author.as_deref() == Some("Ann")));
    // With 書式 off, nothing.
    let quiet = CompareOptions {
        formatting: false,
        ..options.clone()
    };
    let (result, summary) = compare_documents(
        &original_bytes,
        &restyled.export_docx().unwrap(),
        &quiet,
        DATE,
    )
    .unwrap();
    assert_eq!(summary.formatting, 0);
    assert!(open_bytes(&result).revisions().is_empty());
}

#[test]
fn combining_puts_the_reviewers_changes_down_to_them() {
    let mut original = contract();
    let original_bytes = original.export_docx().unwrap();
    let mut copy = open_bytes(&original_bytes);
    let paragraph = typed(&mut copy, "");
    copy.set_tracking_changes(true, &Reviewer::named("Bob"))
        .unwrap();
    copy.place_caret(paragraph, 0);
    copy.insert_text("Bob's words. ").unwrap();
    let (result, _) = combine_documents(
        &original_bytes,
        &copy.export_docx().unwrap(),
        &CompareOptions::new("Ann"),
        DATE,
    )
    .unwrap();
    let combined = open_bytes(&result);
    let revisions = combined.revisions();
    assert!(
        revisions
            .iter()
            .any(|revision| revision.text.contains("Bob's")
                && revision.author.as_deref() == Some("Bob")),
        "{revisions:?}"
    );
}

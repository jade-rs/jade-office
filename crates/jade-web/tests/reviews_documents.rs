//! The 校閲 tab on the browser build's Word document — the commands the
//! native app runs too: a comment written on the selection, answered,
//! resolved, edited and deleted; changes tracked as they are typed, walked
//! one by one, accepted or rejected one at a time or all at once.

mod common;
use common::{contract, typed};
use jade_web::word::review::WordReview;

#[test]
fn a_comment_is_written_answered_resolved_and_deleted() {
    let mut document = contract();
    let paragraph = typed(&mut document, "Clause one. ");
    let count = document.comments().len();
    document
        .select((paragraph, 0), (paragraph, "Clause".len()))
        .unwrap();
    let id = document
        .add_comment("Check this.", "jade")
        .unwrap()
        .expect("a new comment");
    let comment = document
        .comments()
        .into_iter()
        .find(|comment| comment.id == id)
        .unwrap();
    assert_eq!(comment.text, "Check this.");
    assert_eq!(comment.author.as_deref(), Some("jade"));
    assert_eq!(comment.anchor.map(|(start, _)| start), Some((paragraph, 0)));

    document.reply_to_comment(&id, "Done.", "jade").unwrap();
    document.edit_comment(&id, "Check this, please.").unwrap();
    document.resolve_comment(&id, true).unwrap();
    let comment = document
        .comments()
        .into_iter()
        .find(|comment| comment.id == id)
        .unwrap();
    assert_eq!(comment.text, "Check this, please.");
    assert!(comment.resolved);
    assert_eq!(comment.replies.len(), 1);
    assert_eq!(comment.replies[0].text, "Done.");

    document.delete_comment(&id).unwrap();
    assert_eq!(document.comments().len(), count);
    document.undo().unwrap();
    assert_eq!(document.comments().len(), count + 1);
}

#[test]
fn tracked_changes_are_walked_and_decided_one_at_a_time() {
    let mut document = contract();
    let paragraph = typed(&mut document, "");
    assert!(!document.is_tracking_changes());
    document.set_tracking_changes(true, "jade").unwrap();
    assert!(document.is_tracking_changes());

    document.place_caret(paragraph, 0);
    document.insert_text("kept ").unwrap();
    // Apart from the first, at the paragraph's end: two changes.
    let end = document.paragraph_text(paragraph).unwrap().len();
    document.place_caret(paragraph, end);
    document.insert_text(" dropped").unwrap();
    let revisions = document.revisions();
    let ours: Vec<_> = revisions
        .iter()
        .filter(|revision| revision.paragraph == paragraph && revision.is_insertion())
        .collect();
    assert_eq!(ours.len(), 2, "{revisions:?}");

    // 次へ from the paragraph's start lands on the first change in it.
    let first = document.step_revision((paragraph, 0), true).unwrap();
    assert!(first.start() >= (paragraph, 0));
    let previous = document.step_revision(first.start(), false);
    assert!(previous.is_some());

    // The caret in "kept": accepted, it stays as plain text.
    document.place_caret(paragraph, 1);
    document.decide_and_step(true).unwrap();
    assert!(document
        .paragraph_text(paragraph)
        .unwrap()
        .starts_with("kept "));
    // The caret in "dropped": rejected, it goes.
    document.place_caret(paragraph, end + 2);
    document.decide_and_step(false).unwrap();
    let text = document.paragraph_text(paragraph).unwrap();
    assert!(
        text.starts_with("kept ") && !text.contains("dropped"),
        "{text}"
    );
    assert!(document
        .revisions()
        .iter()
        .all(|revision| revision.paragraph != paragraph || !revision.is_insertion()));

    document.set_tracking_changes(false, "jade").unwrap();
    assert!(!document.is_tracking_changes());
    document.decide_all_revisions(true).unwrap();
    assert!(document.revisions().is_empty());
}

/// Typed a keystroke at a time, a tracked word is one change that 元に戻す
/// takes out whole.
#[test]
fn a_word_typed_a_key_at_a_time_is_rejected_whole() {
    let mut document = contract();
    let paragraph = typed(&mut document, "");
    let before = document.paragraph_text(paragraph).unwrap();
    document.set_tracking_changes(true, "jade").unwrap();
    document.place_caret(paragraph, 0);
    for key in ["n", "e", "w", " "] {
        document.insert_text(key).unwrap();
    }
    let ours: Vec<_> = document
        .revisions()
        .into_iter()
        .filter(|revision| revision.paragraph == paragraph)
        .collect();
    assert_eq!(ours.len(), 1, "one change: {ours:?}");
    assert_eq!(ours[0].text, "new ");
    document.place_caret(paragraph, 1);
    document.decide_and_step(false).unwrap();
    assert_eq!(document.paragraph_text(paragraph).unwrap(), before);
}

/// The engine's markup colours: an insertion's, a deletion's.
const INSERTED: [u8; 3] = [0x1b, 0x5e, 0x20];
const DELETED: [u8; 3] = [0xb7, 0x1c, 0x1c];

/// How many pixels of the document's pages are drawn in `color`, and the
/// longest horizontal line of them — an underline or a strikethrough is far
/// longer than any stroke of a glyph.
fn painted_in(document: &jade_web::editing::EditableDocument, color: [u8; 3]) -> (usize, usize) {
    let (mut count, mut longest) = (0, 0);
    for index in 0..document.page_count() {
        let page = document.page(index).unwrap();
        let width = page.width() as usize;
        for row in page.rgba_premultiplied().chunks(width * 4) {
            let mut run = 0;
            for pixel in row.chunks(4) {
                let close = pixel[..3]
                    .iter()
                    .zip(color)
                    .all(|(&have, want)| have.abs_diff(want) <= 24);
                if close {
                    count += 1;
                    run += 1;
                    longest = longest.max(run);
                } else {
                    run = 0;
                }
            }
        }
    }
    (count, longest)
}

/// As Word's 変更内容の表示 shows them: typed with changes tracked, text is
/// drawn in the insertion colour and underlined; deleted, it stays, struck
/// through in the deletion colour. Accepted or rejected, the markup goes.
#[test]
fn tracked_changes_are_drawn_as_markup() {
    let mut document = contract();
    let paragraph = typed(&mut document, "");
    assert_eq!(painted_in(&document, INSERTED).0, 0);
    assert_eq!(painted_in(&document, DELETED).0, 0);
    let before = document.paragraph_text(paragraph).unwrap();
    document.set_tracking_changes(true, "jade").unwrap();

    document.place_caret(paragraph, 0);
    document.insert_text("Inserted words ").unwrap();
    let (count, longest) = painted_in(&document, INSERTED);
    assert!(count > 50, "the insertion is coloured: {count} pixels");
    assert!(longest > 40, "and underlined: its longest line {longest}px");

    // The paragraph's first three characters, deleted: still there, struck
    // through.
    let start = "Inserted words ".len();
    let word = before.char_indices().nth(3).unwrap().0;
    document
        .select((paragraph, start), (paragraph, start + word))
        .unwrap();
    document.delete(true).unwrap();
    let (count, longest) = painted_in(&document, DELETED);
    assert!(count > 20, "the deletion is coloured: {count} pixels");
    assert!(
        longest > 15,
        "and struck through: its longest line {longest}px"
    );

    document.decide_all_revisions(false).unwrap();
    assert_eq!(document.paragraph_text(paragraph).unwrap(), before);
    assert_eq!(painted_in(&document, INSERTED).0, 0, "rejected, no markup");
    assert_eq!(painted_in(&document, DELETED).0, 0, "rejected, no markup");
}

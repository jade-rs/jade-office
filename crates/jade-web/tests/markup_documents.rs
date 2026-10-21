//! 校閲 › 変更履歴 on the browser build's Word document — the commands the
//! native app runs too: who the review work is recorded under, the
//! 変更内容の表示 views, reviewers shown one by one in colours of their
//! own, deciding only the changes on show, and walking the comments.

mod common;
use common::{contract, typed};
use jade_docx_editor::revision_view::AUTHOR_COLORS;
use jade_web::editing::EditableDocument;
use jade_web::word::markup::{
    MarkupMode, MarkupOptions, WordMarkup, ALL_CHANGE_BAR, SIMPLE_CHANGE_BAR,
};
use jade_web::word::review::WordReview;
use jade_web::word::reviewer::Reviewer;

fn rgb(hex: &str) -> [u8; 3] {
    let value = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap();
    [(value >> 16) as u8, (value >> 8) as u8, value as u8]
}

/// Pixels of the pages drawn in `color`, and the longest vertical line of
/// them: a change bar is a line down the margin, which no glyph is.
fn painted_in(document: &EditableDocument, color: [u8; 3]) -> (usize, usize) {
    let (mut count, mut tallest) = (0, 0);
    for index in 0..document.page_count() {
        let page = document.page(index).unwrap();
        let width = page.width() as usize;
        let pixels = page.rgba_premultiplied();
        let height = pixels.len() / (width * 4);
        let close = |x: usize, y: usize| {
            let at = (y * width + x) * 4;
            pixels[at..at + 3]
                .iter()
                .zip(color)
                .all(|(&have, want)| have.abs_diff(want) <= 24)
        };
        for x in 0..width {
            let mut run = 0;
            for y in 0..height {
                if close(x, y) {
                    count += 1;
                    run += 1;
                    tallest = tallest.max(run);
                } else {
                    run = 0;
                }
            }
        }
    }
    (count, tallest)
}

fn author_color(document: &EditableDocument, name: &str) -> [u8; 3] {
    let index = document
        .reviewers()
        .iter()
        .position(|reviewer| reviewer == name)
        .expect("a reviewer");
    rgb(AUTHOR_COLORS[index % AUTHOR_COLORS.len()])
}

/// Comments and changes are put down to the reviewer's name and initials.
#[test]
fn review_work_is_recorded_under_the_reviewers_name_and_initials() {
    let mut document = contract();
    let paragraph = typed(&mut document, "Clause one. ");
    document
        .select((paragraph, 0), (paragraph, "Clause".len()))
        .unwrap();
    let ann = Reviewer::new("Ann Lee", "AJL");
    let id = document.add_comment("Check.", &ann).unwrap().unwrap();
    let comment = document
        .comments()
        .into_iter()
        .find(|comment| comment.id == id)
        .unwrap();
    assert_eq!(comment.author.as_deref(), Some("Ann Lee"));
    document.set_tracking_changes(true, &ann).unwrap();
    document.place_caret(paragraph, 0);
    document.insert_text("x").unwrap();
    assert!(document
        .revisions()
        .iter()
        .any(|revision| revision.author.as_deref() == Some("Ann Lee")));
    let saved = document.export_docx().unwrap();
    let package = jade_opc::OpcPackage::open(&saved).unwrap();
    let comments = package.part_str("word/comments.xml").unwrap().unwrap();
    assert!(comments.contains(r#"w:initials="AJL""#), "{comments}");
}

/// すべての変更履歴 draws a reviewer's changes in their colour, a bar beside
/// the line; シンプルな変更履歴 shows the text as it will read with a red
/// bar; 変更履歴なし no bar; a reviewer unticked reads as accepted.
#[test]
fn markup_views_show_the_changes_as_word_does() {
    let mut document = contract();
    let paragraph = typed(&mut document, "");
    document
        .set_tracking_changes(true, &Reviewer::named("Ann Lee"))
        .unwrap();
    document.place_caret(paragraph, 0);
    document.insert_text("Inserted words ").unwrap();
    let ann = author_color(&document, "Ann Lee");

    let mut options = MarkupOptions::default();
    document.show_markup(&options).unwrap();
    let (count, _) = painted_in(&document, ann);
    assert!(count > 50, "Ann's insertion in her colour: {count}");
    let (_, bar) = painted_in(&document, rgb(ALL_CHANGE_BAR));
    assert!(bar >= 20, "a bar beside the changed line: {bar}px");

    options.mode = MarkupMode::Simple;
    assert!(document.show_markup(&options).unwrap());
    assert_eq!(painted_in(&document, ann).0, 0, "no markup in place");
    let (_, bar) = painted_in(&document, rgb(SIMPLE_CHANGE_BAR));
    assert!(bar >= 20, "a red bar beside the line: {bar}px");

    options.mode = MarkupMode::NoMarkup;
    document.show_markup(&options).unwrap();
    assert!(painted_in(&document, rgb(SIMPLE_CHANGE_BAR)).1 < 20);
    assert!(painted_in(&document, rgb(ALL_CHANGE_BAR)).1 < 20);

    options.mode = MarkupMode::All;
    options.set_reviewer_shown("Ann Lee", false);
    document.show_markup(&options).unwrap();
    assert_eq!(painted_in(&document, ann).0, 0, "Ann unticked");
    options.set_reviewer_shown("Ann Lee", true);
    document.show_markup(&options).unwrap();
    assert!(painted_in(&document, ann).0 > 50);
    // The views change the pages only.
    assert!(document
        .paragraph_text(paragraph)
        .unwrap()
        .starts_with("Inserted words "));
}

/// 表示されたすべての変更を反映 with one reviewer unticked decides the
/// other's changes and leaves theirs.
#[test]
fn only_the_changes_on_show_are_decided() {
    let mut document = contract();
    let paragraph = typed(&mut document, "");
    document
        .set_tracking_changes(true, &Reviewer::named("Ann"))
        .unwrap();
    document.place_caret(paragraph, 0);
    document.insert_text("first ").unwrap();
    document
        .set_tracking_changes(true, &Reviewer::named("Bob"))
        .unwrap();
    let end = document.paragraph_text(paragraph).unwrap().len();
    document.place_caret(paragraph, end);
    document.insert_text(" second").unwrap();
    let mut options = MarkupOptions::default();
    options.set_reviewer_shown("Bob", false);
    document.decide_visible_revisions(&options, true).unwrap();
    let left: Vec<_> = document
        .revisions()
        .into_iter()
        .filter(|revision| revision.paragraph == paragraph)
        .collect();
    assert!(
        left.iter()
            .all(|revision| revision.author.as_deref() == Some("Bob")),
        "{left:?}"
    );
    assert!(!left.is_empty());
    let text = document.paragraph_text(paragraph).unwrap();
    assert!(text.starts_with("first ") && text.ends_with(" second"));
}

/// コメント › 次へ and 前へ go from comment to comment in document order,
/// round from the last to the first.
#[test]
fn next_and_previous_comment_walk_the_comments() {
    let mut document = contract();
    let paragraph = typed(&mut document, "One two three. ");
    let reviewer = Reviewer::named("jade");
    document.select((paragraph, 0), (paragraph, 3)).unwrap();
    let first = document.add_comment("first", &reviewer).unwrap().unwrap();
    document.select((paragraph, 4), (paragraph, 7)).unwrap();
    let second = document.add_comment("second", &reviewer).unwrap().unwrap();
    let from_first = document.step_comment((paragraph, 0), true).unwrap();
    assert_eq!(from_first.id, second);
    let back = document.step_comment((paragraph, 4), false).unwrap();
    assert_eq!(back.id, first);
    let last = document
        .comments()
        .into_iter()
        .filter_map(|comment| comment.anchor.map(|(start, _)| (start, comment.id)))
        .max()
        .unwrap();
    let round = document.step_comment(last.0, true).unwrap();
    let first_of_all = document
        .comments()
        .into_iter()
        .filter_map(|comment| comment.anchor.map(|(start, _)| (start, comment.id)))
        .min()
        .unwrap();
    assert_eq!(round.id, first_of_all.1);
}

/// With changes tracked, bold and a paragraph's alignment are formatting
/// changes: listed as Word lists them, named as its balloons name them,
/// taken back by undo, and rejected back to what they were.
#[test]
fn formatting_changes_are_tracked_described_and_decided() {
    use jade_web::word::markup::describe_formatting;
    use jade_web::word::WordFormatting;
    let mut document = contract();
    let paragraph = typed(&mut document, "Bold words ");
    let before_alignment = document.format_state().alignment;
    document
        .set_tracking_changes(true, &Reviewer::named("Ann"))
        .unwrap();
    document.select((paragraph, 0), (paragraph, 4)).unwrap();
    document
        .toggle_format(jade_web::editing::CharacterFormat::Bold)
        .unwrap();
    let formatted: Vec<_> = document
        .revisions()
        .into_iter()
        .filter(|revision| revision.is_formatting())
        .collect();
    assert_eq!(formatted.len(), 1, "{formatted:?}");
    assert_eq!(formatted[0].author.as_deref(), Some("Ann"));
    assert_eq!(
        describe_formatting(&document, &formatted[0]),
        ["review.format.bold"]
    );
    // Undone, the change goes with the bold.
    document.undo().unwrap();
    assert!(document
        .revisions()
        .iter()
        .all(|revision| !revision.is_formatting()));
    document.redo().unwrap();
    assert_eq!(
        document
            .revisions()
            .iter()
            .filter(|revision| revision.is_formatting())
            .count(),
        1
    );

    // The paragraph centred: its own change.
    document.place_caret(paragraph, 0);
    WordFormatting::set_alignment(&mut document, "center").unwrap();
    let changes: Vec<_> = document
        .revisions()
        .into_iter()
        .filter(|revision| revision.is_formatting())
        .collect();
    assert_eq!(changes.len(), 2, "{changes:?}");
    // Rejected, both go back.
    document.decide_revisions(&changes, false).unwrap();
    assert!(document
        .revisions()
        .iter()
        .all(|revision| !revision.is_formatting()));
    document.place_caret(paragraph, 1);
    let state = document.format_state();
    assert!(!state.bold);
    assert_eq!(state.alignment, before_alignment);
    let saved = document.export_docx().unwrap();
    let xml = jade_opc::OpcPackage::open(&saved)
        .unwrap()
        .part_str("word/document.xml")
        .unwrap()
        .unwrap();
    assert!(!xml.contains("PrChange"));
}

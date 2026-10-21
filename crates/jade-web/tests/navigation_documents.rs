//! Finding the way around a document in the browser build, through the
//! shared model: the navigation pane's headings with their pages, a
//! heading's section collapsed and moved, ジャンプ, and bookmarks.

mod common;

use jade_web::editing::{EditableDocument, WordSession};
use jade_web::word::references::bookmarks::WordBookmarks;
use jade_web::word::references::navigation::{GoTo, GoToTarget, WordNavigation};

fn toc() -> EditableDocument {
    common::open_bytes(&std::fs::read(common::core().join("public/fixtures/toc.docx")).unwrap())
}

#[test]
fn the_outline_lists_headings_with_their_pages() {
    let document = toc();
    let outline = document.outline();
    assert!(outline.len() >= 2, "{outline:?}");
    assert!(outline.iter().all(|heading| heading.level == 1));
    assert_eq!(outline[0].page, Some(1));
    assert_eq!(document.current_heading(outline[1].paragraph + 1), Some(1));
}

#[test]
fn a_collapsed_heading_hides_its_section() {
    let mut document = toc();
    let outline = document.outline();
    let first = outline[0].paragraph;
    let (start, end) = document.heading_section(first).unwrap();
    assert!(end > start);
    let hidden = document.hidden_by_collapse(&[first].into_iter().collect());
    assert_eq!(hidden, ((start + 1)..=end).collect::<Vec<_>>());
    let next = outline[1].paragraph;
    let top = |document: &EditableDocument, paragraph: usize| {
        document.position_metrics((paragraph, 0)).map(|(_, top, _)| top)
    };
    let before = top(&document, next).unwrap();
    assert!(top(&document, first + 1).is_some());
    document.set_collapsed_blocks(hidden.clone()).unwrap();
    assert_eq!(document.collapsed_blocks(), hidden.as_slice());
    assert!(document.is_collapsed_away(first + 1, &hidden));
    // The section takes no room: the next heading comes up under this one.
    assert_eq!(top(&document, first + 1), None);
    assert!(top(&document, next).unwrap() < before);
    document.set_collapsed_blocks(Vec::new()).unwrap();
    assert_eq!(top(&document, next), Some(before));
}

#[test]
fn a_section_moves_down_and_back_up_in_one_undo_each() {
    let mut document = toc();
    let before: Vec<String> = document.outline().into_iter().map(|h| h.text).collect();
    let first = document.outline()[0].paragraph;
    document.move_heading_section(first, false).unwrap();
    let after: Vec<String> = document.outline().into_iter().map(|h| h.text).collect();
    assert_eq!(after[0], before[1]);
    document.undo().unwrap();
    let undone: Vec<String> = document.outline().into_iter().map(|h| h.text).collect();
    assert_eq!(undone, before);
}

#[test]
fn go_to_finds_headings_pages_and_bookmarks() {
    let mut document = toc();
    let outline = document.outline();
    assert_eq!(
        document.go_to(GoToTarget::Heading, "2", (0, 0)).unwrap(),
        GoTo::Position((outline[1].paragraph, 0))
    );
    assert_eq!(
        document.go_to(GoToTarget::Heading, "", (outline[0].paragraph, 0)).unwrap(),
        GoTo::Position((outline[1].paragraph, 0))
    );
    assert_eq!(document.go_to(GoToTarget::Page, "+1", (0, 0)).unwrap(), GoTo::Page(2));
    assert!(document.go_to(GoToTarget::Comment, "", (0, 0)).is_err());

    let paragraph = outline[1].paragraph;
    document.select((paragraph, 0), (paragraph, 2)).unwrap();
    document.add_bookmark("Second").unwrap();
    assert_eq!(
        document.go_to(GoToTarget::Bookmark, "Second", (0, 0)).unwrap(),
        GoTo::Position((paragraph, 0))
    );
    assert_eq!(document.bookmark_range("Second"), Some(((paragraph, 0), (paragraph, 2))));
    document.remove_bookmark("Second").unwrap();
    assert!(document.bookmarks().iter().all(|bookmark| bookmark.name != "Second"));
}

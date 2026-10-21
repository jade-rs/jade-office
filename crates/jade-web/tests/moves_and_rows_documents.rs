//! 校閲 › 変更履歴 for moved text and table rows on the browser build's
//! Word document — what both shells run: a move is listed as its two
//! halves and decided whole; a row inserted or deleted with changes
//! tracked is listed as 行の挿入 / 行の削除, and 承諾 / 元に戻す keeps or
//! takes out the row.

mod common;
use common::{contract, core, open_bytes, typed};
use jade_web::editing::{WordFormatting, WordSession};
use jade_web::word::markup::kind_key;
use jade_web::word::review::WordReview;
use jade_web::word::reviewer::Reviewer;
use jade_web::word::TableCommand;

#[test]
fn a_move_is_decided_with_its_other_half() {
    let bytes = std::fs::read(core().join("public/fixtures/review-kinds.docx")).unwrap();
    let mut document = open_bytes(&bytes);
    let revisions = document.revisions();
    let to = revisions
        .iter()
        .find(|revision| revision.kind == "movedTo")
        .expect("the fixture moves text")
        .clone();
    assert!(revisions
        .iter()
        .any(|revision| revision.kind == "movedFrom"));
    assert_eq!(kind_key(&to), "review.kind.moved_to");
    document.decide_revisions(&[to], true).unwrap();
    let left = document.revisions();
    assert!(
        !left
            .iter()
            .any(|revision| revision.kind.starts_with("moved")),
        "{left:?}"
    );
}

#[test]
fn rows_inserted_and_deleted_with_changes_tracked_are_decided_as_rows() {
    let mut document = contract();
    let paragraph = typed(&mut document, "");
    document.place_caret(paragraph, 0);
    let caret = WordFormatting::insert_table(&mut document, 2, 1).unwrap();
    document.place_caret(caret.0, caret.1);
    document.insert_text("first").unwrap();
    let table = document.caret_table_cell().unwrap().table_order_index;
    let ann = Reviewer::named("Ann");
    document.set_tracking_changes(true, &ann).unwrap();
    let caret = document
        .table_command(TableCommand::InsertRowBelow)
        .unwrap();
    document.place_caret(caret.0, caret.1);
    let rows = |document: &jade_web::editing::EditableDocument| {
        document.word_document().table_dimensions(table).unwrap().0
    };
    assert_eq!(rows(&document), 3);
    let inserted: Vec<_> = document
        .revisions()
        .into_iter()
        .filter(|revision| revision.kind == "insertedRow")
        .collect();
    assert_eq!(inserted.len(), 1);
    assert_eq!(inserted[0].author.as_deref(), Some("Ann"));
    assert_eq!(kind_key(&inserted[0]), "review.kind.inserted_row");

    // The first row deleted: it stays, listed as 行の削除.
    let first = (0..document.paragraph_count())
        .find(|&index| document.paragraph_text(index).as_deref() == Some("first"))
        .unwrap();
    document.place_caret(first, 0);
    document.table_command(TableCommand::DeleteRow).unwrap();
    assert_eq!(rows(&document), 3);
    let deleted: Vec<_> = document
        .revisions()
        .into_iter()
        .filter(|revision| revision.kind == "deletedRow")
        .collect();
    assert_eq!(deleted.len(), 1, "{:?}", document.revisions());

    // 元に戻す for the insertion takes the new row out; 承諾 for the
    // deletion takes the deleted one.
    document.decide_revisions(&inserted, false).unwrap();
    assert_eq!(rows(&document), 2);
    let deleted: Vec<_> = document
        .revisions()
        .into_iter()
        .filter(|revision| revision.kind == "deletedRow")
        .collect();
    document.decide_revisions(&deleted, true).unwrap();
    assert_eq!(rows(&document), 1);
    // Undo brings the row back, marked.
    document.undo().unwrap();
    assert_eq!(rows(&document), 2);
    let saved = document.export_docx().unwrap();
    let reopened = open_bytes(&saved);
    assert!(reopened
        .revisions()
        .iter()
        .any(|revision| revision.kind == "deletedRow"));
}

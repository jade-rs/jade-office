//! Opening an AutoRecover snapshot must require saving before any new edits.
mod common;

#[test]
fn recovered_snapshot_remains_unsaved_until_exported() {
    let mut original = common::contract();
    let paragraph = common::typed(&mut original, "回復された編集 ");
    let expected = original.paragraph_text(paragraph).unwrap();
    let snapshot = original.word_docx().unwrap();
    let mut recovered = common::open_bytes(&snapshot);
    assert!(!recovered.is_dirty());

    assert!(!recovered.can_undo());
    recovered.mark_recovered();
    assert!(!recovered.can_undo());
    assert!(recovered.is_dirty());
    assert_eq!(recovered.paragraph_text(paragraph).unwrap(), expected);
    // Snapshot creation must not count as saving the recovered document.
    let snapshot = recovered.word_docx().unwrap();
    assert!(recovered.is_dirty());
    assert_eq!(
        common::open_bytes(&snapshot)
            .paragraph_text(paragraph)
            .unwrap(),
        expected
    );

    let saved = recovered.export_docx().unwrap();
    assert!(!recovered.is_dirty());
    assert_eq!(
        common::open_bytes(&saved)
            .paragraph_text(paragraph)
            .unwrap(),
        expected
    );
}

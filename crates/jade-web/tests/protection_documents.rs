//! 校閲 › 保護 on the browser build's Word document — what both shells run:
//! a read-only restriction with a password and an exception everyone may
//! edit, 変更履歴のロック, 読み取り専用を推奨 and 最終版, and a password to
//! open the file.

mod common;
use common::{contract, open_bytes, typed};
use jade_web::word::protection::{
    decrypt_package, encrypt_package, is_encrypted_package, is_marked_final, mark_final,
    DecryptError, EditingKind, WordProtection, WordReadOnly,
};
use jade_web::word::review::WordReview;
use jade_web::word::reviewer::Reviewer;

#[test]
fn a_read_only_restriction_leaves_only_its_exceptions_editable() {
    let mut document = contract();
    let open = typed(&mut document, "Anyone may edit this. ");
    let locked = (open + 1..document.paragraph_count())
        .find(|&index| {
            document
                .paragraph_text(index)
                .is_some_and(|text| text.chars().count() > 3)
        })
        .unwrap();
    document.place_caret(open, 0);
    document.allow_everyone_here().unwrap();
    document
        .start_restriction(Some(EditingKind::ReadOnly), false, Some("pw"), [1; 16])
        .unwrap();
    assert!(document.is_restricted());
    document.place_caret(locked, 0);
    assert!(
        document.insert_text("x").is_err(),
        "locked text refuses typing"
    );
    document.place_caret(open, 3);
    document.insert_text("!").unwrap();
    assert!(document
        .paragraph_text(open)
        .unwrap()
        .starts_with("Any!one"));

    assert!(document.stop_restriction("wrong").is_err());
    document.stop_restriction("pw").unwrap();
    assert!(!document.is_restricted());
    document.place_caret(locked, 0);
    document.insert_text("x").unwrap();

    // Saved and opened again, the exception is still there.
    let reopened = open_bytes(&document.export_docx().unwrap());
    assert_eq!(reopened.editable_ranges().len(), 1);
}

#[test]
fn locked_tracking_cannot_be_turned_off() {
    let mut document = contract();
    let paragraph = typed(&mut document, "");
    let ann = Reviewer::named("Ann");
    document.set_tracking_changes(true, &ann).unwrap();
    document
        .start_restriction(Some(EditingKind::TrackedChanges), false, None, [2; 16])
        .unwrap();
    assert!(document.tracking_locked());
    assert!(document.set_tracking_changes(false, &ann).is_err());
    document.place_caret(paragraph, 0);
    document.insert_text("tracked ").unwrap();
    assert!(document
        .revisions()
        .iter()
        .any(|revision| revision.text.contains("tracked")));
    document.stop_restriction("").unwrap();
    document.set_tracking_changes(false, &ann).unwrap();
}

#[test]
fn read_only_recommended_final_and_a_password_to_open() {
    let mut document = contract();
    document.set_write_protection(true, None, [3; 16]).unwrap();
    let bytes = document.export_docx().unwrap();
    let mut reopened = open_bytes(&bytes);
    assert!(reopened.write_protection().unwrap().recommended);
    // Opened read-only as recommended: nothing can be typed.
    reopened.set_opened_read_only(true);
    let paragraph = typed_or_none(&mut reopened);
    assert!(paragraph.is_err());
    reopened.set_opened_read_only(false);

    let final_bytes = mark_final(&bytes, true).unwrap();
    assert!(is_marked_final(&final_bytes));
    open_bytes(&final_bytes);

    let mut counter = 0u8;
    let encrypted = encrypt_package(&bytes, "開けゴマ", &mut |buffer| {
        for byte in buffer {
            counter = counter.wrapping_add(37);
            *byte = counter;
        }
    })
    .unwrap();
    assert!(is_encrypted_package(&encrypted));
    assert_eq!(
        decrypt_package(&encrypted, "nope"),
        Err(DecryptError::WrongPassword)
    );
    let opened = open_bytes(&decrypt_package(&encrypted, "開けゴマ").unwrap());
    assert_eq!(opened.paragraph_count(), document.paragraph_count());
}

fn typed_or_none(document: &mut jade_web::editing::EditableDocument) -> Result<(), String> {
    document.place_caret(0, 0);
    document.insert_text("x")
}

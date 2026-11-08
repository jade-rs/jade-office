//! A failed format/protection step must not acknowledge an unsaved document.
mod common;

#[test]
fn failed_save_retains_edits_dirty_state_and_undo() {
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "保存を確認 ");
    let expected = document.paragraph_text(paragraph).unwrap();
    assert!(document.is_dirty());
    assert!(document.can_undo());

    let error = document.export_docx_with(|bytes| {
        // The package was successfully produced; the later step failed.
        assert_eq!(
            common::open_bytes(&bytes)
                .paragraph_text(paragraph)
                .unwrap(),
            expected
        );
        Err("cannot encrypt the output".to_string())
    });
    assert_eq!(error.unwrap_err(), "cannot encrypt the output");
    assert!(document.is_dirty());
    assert!(document.can_undo());
    assert_eq!(document.paragraph_text(paragraph).unwrap(), expected);

    document.undo().unwrap();
    assert_ne!(document.paragraph_text(paragraph).unwrap(), expected);
    document.redo().unwrap();
    assert_eq!(document.paragraph_text(paragraph).unwrap(), expected);

    let saved = document.export_docx_with(Ok).unwrap();
    assert!(!document.is_dirty());
    assert!(document.can_undo());
    assert_eq!(
        common::open_bytes(&saved)
            .paragraph_text(paragraph)
            .unwrap(),
        expected
    );
}

#[test]
fn failed_save_of_a_clean_document_does_not_create_changes() {
    let mut document = common::contract();
    assert!(!document.is_dirty());
    assert!(!document.can_undo());
    assert!(document
        .export_docx_with(|_| Err("conversion failed".into()))
        .is_err());
    assert!(!document.is_dirty());
    assert!(!document.can_undo());
}

#[test]
fn successful_format_conversion_returns_the_converted_edits() {
    use jade_web::word::formats::{
        open_as_docx, save_from_docx, OpenOptions, SaveOptions, WordFormat,
    };
    let mut document = common::contract();
    common::typed(&mut document, "ConvertedSaveMarker ");
    let saved = document
        .export_docx_with(|docx| {
            save_from_docx(WordFormat::Rtf, &docx, SaveOptions::default())
                .map_err(|error| error.to_string())
        })
        .unwrap();
    assert_eq!(WordFormat::sniff(&saved), Some(WordFormat::Rtf));
    assert!(!document.is_dirty());
    let reopened = open_as_docx("saved.rtf", &saved, OpenOptions::default(), &|_| None).unwrap();
    let reopened = common::open_bytes(&reopened.docx);
    assert!((0..reopened.paragraph_count()).any(|index| {
        reopened
            .paragraph_text(index)
            .is_some_and(|text| text.contains("ConvertedSaveMarker"))
    }));
}

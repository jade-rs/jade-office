//! A Word document laid out again from a change to its package, as the
//! browser's file tools make one: 互換モードの変換, a template attached and
//! detached — the view's page box, the marks and the caret kept, the
//! document unsaved.

mod common;
use common::{core, open_bytes};
use jade_web::word::compat;
use jade_web::word::templates;
use jade_web::word::view::{view_geometry, ViewKind};

#[test]
fn converting_lays_the_document_out_again_as_current() {
    let bytes =
        std::fs::read(core().join("public/fixtures/word-compat-comprehensive.docx")).unwrap();
    let mut document = open_bytes(&bytes);
    assert!(!document.is_dirty());
    let web = view_geometry(ViewKind::Web, (900.0, 700.0), 1.0);
    document.set_view_geometry(web).unwrap();
    document.set_formatting_marks(false).unwrap();
    document.place_caret(0, 0);
    let docx = document.word_docx().unwrap();
    assert_eq!(compat::compatibility_mode(&docx), 14);
    let upgraded = compat::upgrade(&docx).unwrap();
    document.reload(&upgraded).unwrap();
    assert!(document.is_dirty(), "a change to be saved");
    assert_eq!(document.view_geometry(), web, "the view stays");
    assert!(!document.formatting_marks(), "the marks as they were");
    assert_eq!(document.caret(), Some((0, 0)));
    let saved = document.word_docx().unwrap();
    assert_eq!(compat::compatibility_mode(&saved), compat::CURRENT_MODE);
}

#[test]
fn a_template_attached_and_detached_goes_through_a_reload() {
    let mut document = common::contract();
    let docx = document.word_docx().unwrap();
    let template = jade_web::word::new::blank_document(jade_web::word::new::Blank::Japanese);
    let attached = templates::attach_template(&docx, "Letter.dotx", Some(&template), true).unwrap();
    document.reload(&attached).unwrap();
    let saved = document.word_docx().unwrap();
    assert_eq!(
        templates::attached_template(&saved).as_deref(),
        Some("Letter.dotx")
    );
    assert!(templates::links_styles(&saved));
    let detached = templates::detach_template(&saved).unwrap();
    document.reload(&detached).unwrap();
    assert_eq!(
        templates::attached_template(&document.word_docx().unwrap()),
        None
    );
}

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

/// 挿入 › ファイルからテキスト: a .docx's body at the caret — its first
/// paragraph joining the text before the caret, its last the text after —
/// its heading style kept, as one undo step; a .txt a paragraph a line.
#[test]
fn text_from_a_file_comes_in_at_the_caret() {
    use jade_web::editing::WordSession;
    use jade_web::word::new::{blank_document, Blank};
    use jade_web::word::styles::WordStyles;
    use jade_web::word::text_from_file::insert_text_from_file;

    let mut source = open_bytes(&blank_document(Blank::Japanese));
    source.place_caret(0, 0);
    source.insert_text("見出し").unwrap();
    WordStyles::apply_style(&mut source, "Heading1").unwrap();
    source.insert_paragraph_break().unwrap();
    WordStyles::apply_style(&mut source, "Normal").unwrap();
    source.insert_text("本文").unwrap();
    let source = source.export_docx().unwrap();

    let mut document = open_bytes(&blank_document(Blank::Japanese));
    document.place_caret(0, 0);
    document.insert_text("前後").unwrap();
    document.place_caret(0, "前".len());
    let caret = insert_text_from_file(&mut document, &source).unwrap();
    let texts: Vec<String> = (0..document.paragraph_count())
        .filter_map(|index| document.paragraph_text(index))
        .collect();
    assert_eq!(texts[..2], ["前見出し".to_string(), "本文後".to_string()]);
    assert_eq!(caret, (1, "本文".len()));
    let style = |document: &jade_web::editing::EditableDocument, index: usize| {
        document
            .word_document()
            .paragraph(index)
            .and_then(|paragraph| paragraph.properties.as_ref())
            .and_then(|props| props.style.as_deref().map(str::to_string))
    };
    assert_eq!(style(&document, 0).as_deref(), Some("Heading1"));
    let reopened = open_bytes(&document.export_docx().unwrap());
    assert_eq!(reopened.paragraph_text(0).as_deref(), Some("前見出し"));
    document.undo().unwrap();
    assert_eq!(document.paragraph_text(0).as_deref(), Some("前後"));

    // A text file: a paragraph a line, in place of the selection.
    document.select((0, 0), (0, "前".len())).unwrap();
    insert_text_from_file(&mut document, "一行目\r\n二行目\r\n".as_bytes()).unwrap();
    assert_eq!(document.paragraph_text(0).as_deref(), Some("一行目"));
    assert_eq!(document.paragraph_text(1).as_deref(), Some("二行目後"));
}

//! A master document in the browser: the page cannot reach the files its
//! subdocuments are in, so each shows as its link, which a save keeps a
//! link to the file.

use std::path::PathBuf;

use jade_opc::OpcPackage;
use jade_web::editing::EditableDocument;
use jade_web::word::master::{self, Files};
use jade_web::word::WordSession;

fn master_docx() -> Vec<u8> {
    let mut package = OpcPackage::open(&jade_web::word::new::blank_document(
        jade_web::word::new::Blank::English,
    ))
    .unwrap();
    let xml = package.part_str("word/document.xml").unwrap().unwrap();
    package.replace(
        "word/document.xml",
        xml.replace(
            "<w:body><w:p/>",
            r#"<w:body><w:p><w:r><w:t>Master</w:t></w:r></w:p><w:subDoc r:id="rId90"/>"#,
        )
        .into_bytes(),
    );
    let rels = package
        .part_str("word/_rels/document.xml.rels")
        .unwrap()
        .unwrap();
    package.replace(
        "word/_rels/document.xml.rels",
        rels.replace(
            "</Relationships>",
            &format!(r#"<Relationship Id="rId90" Type="{}" Target="chapters/one.docx" TargetMode="External"/></Relationships>"#, master::SUBDOCUMENT),
        )
        .into_bytes(),
    );
    package.write().unwrap()
}

fn open(bytes: &[u8]) -> EditableDocument {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let fonts_dir = core.join("fonts/Noto_Sans_JP/static");
    let regular = std::fs::read(fonts_dir.join("NotoSansJP-Regular.ttf")).unwrap();
    let bold = std::fs::read(fonts_dir.join("NotoSansJP-Bold.ttf")).unwrap();
    let fonts = jade_web::backend(regular.clone(), Some(bold.clone()))
        .unwrap()
        .fonts();
    let measurer = jade_web::document::measurer(&regular, Some(&bold)).unwrap();
    EditableDocument::open(bytes, 1.0, fonts, measurer).unwrap()
}

#[test]
fn a_master_shows_its_subdocuments_as_links_and_keeps_them() {
    let unreachable = |_: &str| None;
    let as_named = |target: &str| target.to_string();
    let files = Files {
        load: &unreachable,
        display: &as_named,
    };
    let editing = master::for_editing(&master_docx(), &files).unwrap();
    let mut document = open(&editing);
    assert_eq!(
        document.paragraph_text(1).as_deref(),
        Some("chapters/one.docx")
    );
    // The link is a locked subdocument's: not text to edit.
    let tags = document.word_document().block_content_control_tags(1);
    let tag = master::Tag::parse(&tags[0]).unwrap();
    assert!(!tag.expanded);
    let exported = document.export_docx().unwrap();
    let (stored, saved) = master::for_saving(&exported, &files).unwrap();
    assert!(saved.is_empty());
    let xml = OpcPackage::open(&stored)
        .unwrap()
        .part_str("word/document.xml")
        .unwrap()
        .unwrap();
    assert!(xml.contains(r#"<w:subDoc r:id="rId90"/>"#));
}

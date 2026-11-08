//! Opening DATE/TIME fields uses the host clock without editing saved results.
mod common;

use jade_web::word::fields::FieldDateTime;
use jade_web::word::new::{blank_document, Blank};

const DATE_FIELD: &str = r#"<w:p><w:fldSimple w:instr='DATE \@ "yyyy-MM-dd"'><w:r><w:t>2000-01-01</w:t></w:r></w:fldSimple></w:p>"#;
const TIME_FIELD: &str = r#"<w:p><w:fldSimple w:instr='TIME \@ "HH:mm:ss"'><w:r><w:t>00:00:00</w:t></w:r></w:fldSimple></w:p>"#;

fn clock_document(story: &str, fields: &str, blank: Blank) -> Vec<u8> {
    let mut package = jade_opc::OpcPackage::open(&blank_document(blank)).unwrap();
    let xml = package.part_str("word/document.xml").unwrap().unwrap();
    if story == "body" {
        assert!(xml.contains("<w:body><w:p/>"));
        package.replace(
            "word/document.xml",
            xml.replace("<w:body><w:p/>", &format!("<w:body>{fields}"))
                .into_bytes(),
        );
    } else {
        let tag = if story == "header" { "hdr" } else { "ftr" };
        package.replace(
            &format!("word/{story}1.xml"),
            format!(r#"<w:{tag} xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">{fields}</w:{tag}>"#).into_bytes(),
        );
        assert!(xml.contains("<w:sectPr>"));
        package.replace(
            "word/document.xml",
            xml.replace(
                "<w:sectPr>",
                &format!(r#"<w:sectPr><w:{story}Reference w:type="default" r:id="clockStory"/>"#),
            )
            .into_bytes(),
        );
        let rels = package
            .part_str("word/_rels/document.xml.rels")
            .unwrap()
            .unwrap();
        package.replace("word/_rels/document.xml.rels", rels.replace("</Relationships>", &format!(r#"<Relationship Id="clockStory" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/{story}" Target="{story}1.xml"/></Relationships>"#)).into_bytes());
        let types = package.part_str("[Content_Types].xml").unwrap().unwrap();
        package.replace("[Content_Types].xml", types.replace("</Types>", &format!(r#"<Override PartName="/word/{story}1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.{story}+xml"/></Types>"#)).into_bytes());
    }
    package.write().unwrap()
}

fn assert_opening_clock(story: &str, fields: &str, blank: Blank) {
    let bytes = clock_document(story, fields, blank);
    let saved_clock = common::open_bytes(&bytes);
    let saved_before = saved_clock.word_docx().unwrap();
    let before = saved_clock.page(0).unwrap().rgba_premultiplied();
    let document = common::open_bytes_with_datetime(
        &bytes,
        Some(FieldDateTime {
            year: 2026,
            month: 11,
            day: 7,
            hour: 13,
            minute: 24,
            second: 35,
            weekday: 6,
        }),
    );
    assert_ne!(
        document.page(0).unwrap().rgba_premultiplied(),
        before,
        "{story} clock must affect the initial paint"
    );
    assert!(!document.is_dirty());
    assert!(!document.can_undo());
    let saved_after = document.word_docx().unwrap();
    let before = jade_opc::OpcPackage::open(&saved_before).unwrap();
    let after = jade_opc::OpcPackage::open(&saved_after).unwrap();
    for part in ["word/document.xml".to_owned(), format!("word/{story}1.xml")] {
        assert_eq!(
            before.part_str(&part).unwrap(),
            after.part_str(&part).unwrap()
        );
    }
}

#[test]
fn opening_body_clock_preserves_saved_results() {
    for blank in [Blank::English, Blank::Japanese] {
        for fields in [DATE_FIELD, TIME_FIELD] {
            assert_opening_clock("body", fields, blank);
        }
    }
}

#[test]
fn opening_header_only_clock_preserves_saved_results() {
    for fields in [DATE_FIELD, TIME_FIELD] {
        assert_opening_clock("header", fields, Blank::English);
    }
}

#[test]
fn opening_footer_only_clock_preserves_saved_results() {
    for fields in [DATE_FIELD, TIME_FIELD] {
        assert_opening_clock("footer", fields, Blank::English);
    }
}

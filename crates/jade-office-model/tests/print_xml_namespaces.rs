use jade_office_model::word::printing::{
    has_background, has_hidden_text, print_docx, PrintSettings,
};
use jade_opc::OpcPackage;
use std::io::{Cursor, Write};

fn package(document: &str, styles: &str) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, xml) in [("word/document.xml", document), ("word/styles.xml", styles)] {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(xml.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

#[test]
fn print_options_follow_word_namespaces_and_preserve_other_xml_bytes() {
    for ns in [
        "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
        "http://purl.oclc.org/ooxml/wordprocessingml/main",
    ] {
        for prefix in ["m:", ""] {
            let declaration = if prefix.is_empty() {
                format!("xmlns=\"{ns}\"")
            } else {
                format!("xmlns:m=\"{ns}\"")
            };
            let background =
                format!("<{prefix}background note=\"a > b\"><x:background/></{prefix}background>");
            let hidden = format!("<{prefix}vanish note=\"a > b\"></{prefix}vanish>");
            let document = format!("<{prefix}document {declaration} xmlns:x=\"urn:other\">{background}<{prefix}body><!-- <{prefix}background/> --><x:background/><{prefix}p><{prefix}r><{prefix}rPr>{hidden}<x:vanish/></{prefix}rPr><{prefix}t><![CDATA[<{prefix}vanish/>]]></{prefix}t></{prefix}r></{prefix}p></{prefix}body></{prefix}document>");
            let styles = format!("<{prefix}styles {declaration}><{prefix}style><{prefix}rPr>{hidden}</{prefix}rPr></{prefix}style></{prefix}styles>");
            let source = package(&document, &styles);
            assert!(has_background(&source), "{prefix:?}: {ns}");
            assert!(has_hidden_text(&source), "{prefix:?}: {ns}");
            let printed = print_docx(&source, &PrintSettings::default()).unwrap();
            let printed = OpcPackage::open(&printed).unwrap();
            assert_eq!(
                printed.part_str("word/document.xml").unwrap().unwrap(),
                document.replacen(&background, "", 1)
            );
            assert_eq!(
                printed.part_str("word/styles.xml").unwrap().unwrap(),
                styles
            );
            let printed = print_docx(
                &source,
                &PrintSettings {
                    background: true,
                    hidden_text: true,
                    ..PrintSettings::default()
                },
            )
            .unwrap();
            let printed = OpcPackage::open(&printed).unwrap();
            assert_eq!(
                printed.part_str("word/document.xml").unwrap().unwrap(),
                document.replacen(&hidden, "", 1)
            );
            assert_eq!(
                printed.part_str("word/styles.xml").unwrap().unwrap(),
                styles.replacen(&hidden, "", 1)
            );
        }
    }
}

#[test]
fn comments_cdata_and_foreign_elements_are_not_print_properties() {
    let document = r#"<w:document xmlns:w="urn:other"><!-- <w:background/> --><w:background/><w:body><![CDATA[<w:vanish/>]]><w:vanish/></w:body></w:document>"#;
    let source = package(document, "<styles/>");
    assert!(!has_background(&source));
    assert!(!has_hidden_text(&source));
    assert_eq!(
        print_docx(
            &source,
            &PrintSettings {
                hidden_text: true,
                ..PrintSettings::default()
            }
        )
        .unwrap(),
        source
    );
}

#[test]
fn malformed_print_xml_returns_an_error_instead_of_truncating_content() {
    let document = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:background><w:body/></w:document>"#;
    let source = package(document, "<styles/>");
    assert!(print_docx(&source, &PrintSettings::default()).is_err());
    assert_eq!(
        OpcPackage::open(&source)
            .unwrap()
            .part_str("word/document.xml")
            .unwrap()
            .unwrap(),
        document
    );
}

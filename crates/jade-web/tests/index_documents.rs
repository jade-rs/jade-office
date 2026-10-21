//! 索引 and 引用文献一覧 in the browser build, through the shared model:
//! entries marked with XE, an index built from them in columns, citations
//! marked with TA and their table.

mod common;

use jade_web::editing::WordSession;
use jade_web::word::references::index::{
    AuthoritiesDialog, IndexDialog, MarkCitationDialog, MarkEntryDialog, WordIndex,
};

#[test]
fn an_index_is_built_in_two_columns_and_updated() {
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "契約 ");
    document
        .select((paragraph, 0), (paragraph, "契約".len()))
        .unwrap();
    let mut entry = MarkEntryDialog::for_text("契約");
    entry.main_reading = "けいやく".to_string();
    document.mark_index_entry(&entry).unwrap();
    assert_eq!(document.index_entries().len(), 1);
    document.place_caret(paragraph, 0);
    document.insert_index(&IndexDialog::default()).unwrap();
    let blocks = document.word_document().field_blocks();
    let index = blocks
        .iter()
        .find(|block| block.name() == "INDEX")
        .expect("an index");
    let texts: Vec<String> = (index.start..=index.end)
        .filter_map(|p| document.paragraph_text(p))
        .collect();
    assert!(
        texts.iter().any(|text| text.starts_with("契約, ")),
        "{texts:?}"
    );
    let word = document.word_document();
    let section = word.section_index_for_paragraph(index.start).unwrap();
    let columns = word.document.as_ref().unwrap().sections[section]
        .properties
        .as_ref()
        .and_then(|properties| properties.columns.as_ref())
        .and_then(|columns| columns.count);
    assert_eq!(columns, Some(2));
    assert_eq!(document.update_indexes().unwrap(), 1);
    document.undo().unwrap();
    assert_eq!(
        document
            .word_document()
            .field_blocks()
            .iter()
            .filter(|b| b.name() == "INDEX")
            .count(),
        1
    );
}

#[test]
fn a_table_of_authorities_lists_marked_citations() {
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "最判 ");
    document
        .select((paragraph, 0), (paragraph, "最判".len()))
        .unwrap();
    document
        .mark_citation(&MarkCitationDialog {
            long: "最判平成元年1月1日".to_string(),
            short: "平成元年判決".to_string(),
            category: 1,
        })
        .unwrap();
    document.place_caret(paragraph, 0);
    document
        .insert_table_of_authorities(&AuthoritiesDialog::default())
        .unwrap();
    let blocks = document.word_document().field_blocks();
    let table = blocks
        .iter()
        .find(|block| block.name() == "TOA")
        .expect("a table");
    assert_eq!(document.paragraph_text(table.start).unwrap(), "判例");
    let entry = document.paragraph_text(table.start + 1).unwrap();
    assert!(entry.starts_with("最判平成元年1月1日\t"), "{entry}");
    assert_eq!(document.update_tables_of_authorities().unwrap(), 1);
}

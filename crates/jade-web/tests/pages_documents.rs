//! ページ設定, ヘッダー, フッター and ページ番号 on the browser build's Word
//! document — the commands the native app runs too: margins from the gallery,
//! the page turned, another paper, columns, a section break, the header's
//! and footer's text and a page number in them, each one undo step.

mod common;
use common::{contract, typed};
use jade_web::word::page::{MarginPreset, NumberAlignment, Paper, SectionBreak, Story, WordPage};

#[test]
fn the_page_takes_margins_orientation_paper_and_columns() {
    let mut document = contract();
    let before = document.page_setup();

    let narrow = MarginPreset::Narrow.margins(true);
    document.set_margins(narrow).unwrap();
    assert_eq!(document.page_setup().margins, narrow);

    document.set_landscape(!before.landscape).unwrap();
    let turned = document.page_setup();
    assert_eq!(turned.landscape, !before.landscape);
    let (width, height) = document.page_size();
    assert_eq!(
        width > height,
        turned.landscape,
        "the pages are laid out turned"
    );

    document.set_paper(Paper::A5).unwrap();
    let setup = document.page_setup();
    assert_eq!(Paper::of(setup.width, setup.height), Some(Paper::A5));
    assert_eq!(
        setup.landscape, turned.landscape,
        "the paper keeps its way up"
    );

    document.set_columns(2).unwrap();
    assert_eq!(document.page_setup().columns, 2);

    for _ in 0..4 {
        document.undo().unwrap();
    }
    assert_eq!(document.page_setup(), before);
}

#[test]
fn a_section_break_goes_in_at_the_caret() {
    let mut document = contract();
    let paragraph = typed(&mut document, "before|after ");
    let paragraphs = document.paragraph_count();
    let pages = document.page_count();
    document.place_caret(paragraph, "before|".len());
    document
        .insert_section_break(SectionBreak::NextPage)
        .unwrap();
    assert_eq!(document.paragraph_count(), paragraphs + 1);
    assert!(
        document.page_count() > pages,
        "the next section starts a page"
    );
    document.undo().unwrap();
    assert_eq!(document.paragraph_count(), paragraphs);
}

/// PAGE fields across the footers the saved document carries.
fn footer_page_fields(document: &mut jade_web::editing::EditableDocument) -> usize {
    let bytes = document.export_docx().unwrap();
    let package = jade_opc::OpcPackage::open(&bytes).unwrap();
    package
        .part_names()
        .iter()
        .filter(|name| name.starts_with("word/footer"))
        .filter_map(|name| package.part_str(name).ok().flatten())
        .map(|xml| xml.matches("PAGE").count())
        .sum()
}

#[test]
fn headers_and_footers_take_text_and_a_page_number() {
    let mut document = contract();
    let header = document.story_text(Story::Header);
    document
        .set_story_text(Story::Header, "jade\tcontract\t2026")
        .unwrap();
    assert_eq!(document.story_text(Story::Header), "jade\tcontract\t2026");
    document.undo().unwrap();
    assert_eq!(document.story_text(Story::Header), header);

    let fields = footer_page_fields(&mut document);
    document
        .insert_page_number(Story::Footer, NumberAlignment::Center)
        .unwrap();
    assert!(footer_page_fields(&mut document) > 0);
    document.remove_page_numbers().unwrap();
    assert_eq!(footer_page_fields(&mut document), 0);
    document.undo().unwrap();
    document.undo().unwrap();
    assert_eq!(footer_page_fields(&mut document), fields);
}

//! 表示's views on the browser build's Word document: Web レイアウト and
//! 閲覧モード lay it out in a page box of their own and give the sections'
//! pages back after, アウトライン sets paragraphs in by level, and Word の
//! オプション's marks by kind reach the pages.

mod common;
use common::contract;
use jade_web::editing::WordSession;
use jade_web::word::view::{view_geometry, ViewKind, WEB_STRIP_HEIGHT};

#[test]
fn web_layout_wraps_to_the_window_and_gives_the_pages_back() {
    let mut document = contract();
    let page = document.page_size();
    let pages = document.page_count();
    let web = view_geometry(ViewKind::Web, (1000.0, 800.0), 1.0);
    assert!(document.set_view_geometry(web).unwrap());
    assert!(!document.set_view_geometry(web).unwrap(), "nothing to do");
    let (width, height) = document.page_size();
    assert_eq!(height, WEB_STRIP_HEIGHT);
    assert_eq!(width, web.unwrap().page_width);
    assert!(document.page(0).is_some(), "the strip is painted");
    // 閲覧モード: screen-sized columns, more of them than pages.
    let read = view_geometry(ViewKind::Read, (900.0, 700.0), 1.0);
    document.set_view_geometry(read).unwrap();
    assert!(document.page_count() > pages);
    assert!(document.page_size().1 < 700.0);
    document.set_view_geometry(None).unwrap();
    assert_eq!(document.page_size(), page);
    assert_eq!(document.page_count(), pages);
}

#[test]
fn the_outline_sets_paragraphs_in_and_takes_them_back() {
    let mut document = contract();
    let paragraph = (0..document.paragraph_count())
        .find(|&index| {
            document.cell_id_for_paragraph(index).is_none()
                && document
                    .paragraph_text(index)
                    .is_some_and(|text| text.chars().count() > 3)
        })
        .unwrap();
    let x_with = |document: &mut jade_web::editing::EditableDocument, indent: f64| {
        let indents = [(paragraph, indent)].into_iter().collect();
        document.set_outline_indents(Some(indents)).unwrap();
        document.place_caret(paragraph, 0);
        document.caret_rect().unwrap().x
    };
    let near = x_with(&mut document, 40.0);
    let far = x_with(&mut document, 240.0);
    assert!(far > near + 50.0, "{far} > {near}");
    assert!(document.set_outline_indents(None).unwrap());
    assert!(!document.set_outline_indents(None).unwrap());
}

#[test]
fn marks_by_kind_show_whatever_the_toggle_says() {
    let mut document = contract();
    let mut kinds = document.formatting_mark_kinds();
    assert!(!kinds.spaces);
    kinds.spaces = true;
    assert!(document.set_formatting_mark_kinds(kinds).unwrap());
    assert!(document.formatting_mark_kinds().spaces);
    assert!(!document.set_formatting_mark_kinds(kinds).unwrap());
}

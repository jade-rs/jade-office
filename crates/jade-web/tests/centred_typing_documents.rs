//! Typing into a centred line repaints that line's band, not the document.
//!
//! A centred or right-aligned line starts further left with every character
//! typed into it. The edit used to count that as its paragraph having moved,
//! and repainted every page after it on each keystroke: 33 ms a key in a
//! 37-page contract, 155 ms in a 151-page one, where a left-aligned line took
//! a fraction of a millisecond.

mod common;

use common::open_bytes;

#[test]
fn typing_into_a_centred_title_repaints_only_its_band() {
    let bytes =
        std::fs::read(common::core().join("public/fixtures/trust-beneficiary-sale-keep-next.docx"))
            .unwrap();
    let mut document = open_bytes(&bytes);
    assert!(document.page_count() > 10, "a long document");
    // The first paragraph is centred, and empty: every character typed into
    // it moves the line's left edge.
    let take = |document: &jade_web::editing::EditableDocument| {
        (0..document.page_count())
            .filter_map(|index| {
                let page = document.page(index)?;
                page.take_damaged_rows()
                    .map(|rows| (index, rows, page.height()))
            })
            .collect::<Vec<_>>()
    };
    document.place_caret(0, 0);
    take(&document);
    for character in ["a", "b", "c"] {
        document.insert_text(character).unwrap();
        let damage = take(&document);
        assert_eq!(damage.len(), 1, "one page repainted: {damage:?}");
        let (page, rows, height) = &damage[0];
        assert_eq!(*page, 0);
        assert!(
            rows.len() < (*height as usize) / 4,
            "only the title's band: {rows:?} of {height}"
        );
    }
    assert_eq!(document.paragraph_text(0).unwrap(), "abc");
}

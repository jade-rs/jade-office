//! Formulas ▸ Name Manager as Excel's: the names the user made, by name,
//! with what they refer to; the selected one's reference changed or the
//! name deleted, each one undo step.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::name_manager::{NameManager, NameManagerKey};
use jade_office_model::sheet::{span, CellRef, Workbook};
use jade_office_model::tiny::TinySkiaBackend;

fn book() -> Workbook<TinySkiaBackend> {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let directory = core.join("fonts/Noto_Sans_JP/static");
    let backend = TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        None,
    ))
    .unwrap();
    let bytes = std::fs::read(core.join("public/fixtures/grid-operations-workbook.xlsx")).unwrap();
    Workbook::open(&bytes, "UTC", backend).unwrap()
}

#[test]
fn the_names_are_listed_edited_and_deleted() {
    let mut book = book();
    let at = |column, row| CellRef::new(column, row);
    book.define_range_name("Sales", span(at(1, 1), at(1, 3)))
        .unwrap();
    book.define_range_name("costs", span(at(2, 1), at(2, 3)))
        .unwrap();
    let rows = NameManager::rows(&book);
    let names: Vec<_> = rows.iter().map(|row| row.name.as_str()).collect();
    assert_eq!(names, ["costs", "Sales"]);
    assert!(rows[1].refers_to.starts_with('=') && rows[1].refers_to.ends_with("$B$2:$B$4"));

    let mut dialog = NameManager::open(&book);
    assert_eq!(dialog.selected, Some(0));
    assert_eq!(dialog.key(&book, "down", false), NameManagerKey::Handled);
    assert_eq!(dialog.selected, Some(1));
    let sheet = rows[1].refers_to.split('!').next().unwrap().to_string();
    dialog.refers_to = jade_office_model::text::TextEdit::new(format!("{sheet}!$B$2:$B$9"));
    assert_eq!(dialog.key(&book, "enter", false), NameManagerKey::Take);
    assert!(dialog.apply(&mut book).unwrap());
    assert!(NameManager::rows(&book)[1].refers_to.ends_with("$B$2:$B$9"));
    // Nothing changed: nothing to do.
    assert!(!dialog.apply(&mut book).unwrap());

    assert!(dialog.delete(&mut book).unwrap());
    let names: Vec<_> = NameManager::rows(&book)
        .into_iter()
        .map(|row| row.name)
        .collect();
    assert_eq!(names, ["costs"]);
    assert_eq!(dialog.selected, Some(0));
    assert!(book.undo().unwrap());
    assert_eq!(NameManager::rows(&book).len(), 2);
}

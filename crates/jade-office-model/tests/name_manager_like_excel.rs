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

/// New…: a name typed for one sheet, referring to the selection; one by
/// that name in the same scope refused; Filter narrows the list.
#[test]
fn new_makes_a_name_for_a_scope_and_filter_narrows_the_list() {
    use jade_office_model::name_manager::NameFilter;
    let mut book = book();
    let at = |column, row| CellRef::new(column, row);
    book.define_range_name("Sales", span(at(1, 1), at(1, 3)))
        .unwrap();
    let mut dialog = NameManager::open(&book);
    dialog.start_new(&book, span(at(0, 0), at(2, 4)));
    assert!(
        dialog.refers_to.text.ends_with("!$A$1:$C$5"),
        "{}",
        dialog.refers_to.text
    );
    // Typing goes to the name first.
    dialog.field_mut().insert("Sales");
    assert_eq!(dialog.new.as_ref().unwrap().name.text, "Sales");
    assert_eq!(dialog.key(&book, "enter", false), NameManagerKey::Take);
    // The workbook has a Sales already.
    assert!(dialog.apply(&mut book).is_err());
    // For the first sheet it is a name of its own.
    dialog.set_scope(Some(0));
    assert!(dialog.apply(&mut book).unwrap());
    assert!(dialog.new.is_none());
    let rows = dialog.listed(&book);
    let made = &rows[dialog.selected.unwrap()];
    assert_eq!((made.name.as_str(), made.scope), ("Sales", Some(0)));
    assert!(made.refers_to.ends_with("!$A$1:$C$5"));

    // Tab moves to the reference; Escape gives up.
    dialog.start_new(&book, span(at(0, 0), at(0, 0)));
    assert!(dialog.refers_to.text.ends_with("!$A$1"));
    dialog.key(&book, "tab", false);
    assert!(!dialog.new.as_ref().unwrap().on_name);
    dialog.key(&book, "escape", false);
    assert!(dialog.new.is_none());

    book.set_defined_name(&jade_xlsx_reader::DefinedName {
        name: "Gone".into(),
        formula: "#REF!$A$1".into(),
        local_sheet_index: None,
        hidden: false,
    })
    .unwrap();
    let shown = |dialog: &NameManager, book: &Workbook<TinySkiaBackend>| {
        dialog
            .listed(book)
            .into_iter()
            .map(|row| (row.name, row.scope))
            .collect::<Vec<_>>()
    };
    dialog.set_filter(&book, NameFilter::Worksheet);
    assert_eq!(shown(&dialog, &book), [("Sales".to_string(), Some(0))]);
    dialog.set_filter(&book, NameFilter::Workbook);
    assert_eq!(
        shown(&dialog, &book),
        [("Gone".to_string(), None), ("Sales".to_string(), None)]
    );
    dialog.set_filter(&book, NameFilter::Errors);
    assert_eq!(shown(&dialog, &book), [("Gone".to_string(), None)]);
    dialog.set_filter(&book, NameFilter::WithoutErrors);
    assert_eq!(shown(&dialog, &book).len(), 2);
    // The selection counts the rows shown: Delete takes the one selected.
    dialog.set_filter(&book, NameFilter::Errors);
    assert!(dialog.delete(&mut book).unwrap());
    assert!(NameManager::rows(&book)
        .iter()
        .all(|row| row.name != "Gone"));
}

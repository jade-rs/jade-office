//! View ▸ Freeze Panes as Excel's menu does it: the rows above and the
//! columns left of the active cell, the top row, the first column, and
//! Unfreeze Panes — each one undo step, and saved so that Excel opens the
//! sheet frozen.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::sheet::{CellRef, Freeze, Workbook};
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
fn panes_freeze_at_a_cell_the_top_row_and_the_first_column() {
    let mut book = book();
    // The fixture keeps its header row and first column in place.
    assert_eq!(book.frozen_panes(), Some((1, 1)));
    assert!(book.freeze_panes(Freeze::Unfreeze).unwrap());
    assert_eq!(book.frozen_panes(), None);
    // A1 has nothing above or left of it; nothing is left to unfreeze.
    assert!(!book
        .freeze_panes(Freeze::Panes(CellRef::new(0, 0)))
        .unwrap());
    assert!(!book.freeze_panes(Freeze::Unfreeze).unwrap());

    assert!(book
        .freeze_panes(Freeze::Panes(CellRef::new(2, 3)))
        .unwrap());
    assert_eq!(book.frozen_panes(), Some((2, 3)));
    assert!(book.is_dirty());
    // The same again changes nothing.
    assert!(!book
        .freeze_panes(Freeze::Panes(CellRef::new(2, 3)))
        .unwrap());

    assert!(book.freeze_panes(Freeze::TopRow).unwrap());
    assert_eq!(book.frozen_panes(), Some((0, 1)));
    assert!(book.freeze_panes(Freeze::FirstColumn).unwrap());
    assert_eq!(book.frozen_panes(), Some((1, 0)));

    // Saved and opened again, the sheet is frozen.
    let saved = book.save_bytes().unwrap();
    let reread = jade_xlsx_reader::read_xlsx(&saved).unwrap();
    assert_eq!(
        reread.sheets[book.sheet_index()]
            .freeze_pane
            .as_ref()
            .map(|pane| (pane.frozen_columns, pane.frozen_rows)),
        Some((1, 0))
    );

    assert!(book.freeze_panes(Freeze::Unfreeze).unwrap());
    assert_eq!(book.frozen_panes(), None);
    // Each was one step.
    assert!(book.undo().unwrap());
    assert_eq!(book.frozen_panes(), Some((1, 0)));
    assert!(book.undo().unwrap());
    assert_eq!(book.frozen_panes(), Some((0, 1)));
    assert!(book.undo().unwrap());
    assert_eq!(book.frozen_panes(), Some((2, 3)));
    assert!(book.undo().unwrap());
    assert_eq!(book.frozen_panes(), None);
    assert!(book.undo().unwrap());
    assert_eq!(book.frozen_panes(), Some((1, 1)));
}

//! Tables, charts and SmartArt in decks made in PowerPoint (Apache POI's test
//! data) edit in the app as in PowerPoint: a double-click in a table types
//! into the cell under it, a merged cell is typed into where it starts, a
//! table or chart resizes with its frame, and a SmartArt diagram moves but
//! keeps its size.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::deck::{Deck, ShapeRect, TextTarget};
use jade_office_model::tiny::TinySkiaBackend;

fn core() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"))
}

fn open(name: &str) -> Deck<TinySkiaBackend> {
    let directory = core().join("fonts/Noto_Sans_JP/static");
    let backend = TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        None,
    ))
    .unwrap();
    let bytes = std::fs::read(core().join("public/fixtures/poi").join(name)).unwrap();
    Deck::open(&bytes, backend).unwrap()
}

/// The middle of the cell at `row`, `column` of the table, in slide px.
fn middle_of(deck: &mut Deck<TinySkiaBackend>, id: u32, row: usize, column: usize) -> (f64, f64) {
    let target = TextTarget::Cell { id, row, column };
    let rect = deck.text_rect(0, target).expect("the cell is placed");
    (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0)
}

#[test]
fn a_double_click_in_a_table_types_into_the_cell_under_it() {
    let mut deck = open("table-with-different-font-colors.pptx");
    let (x, y) = middle_of(&mut deck, 4, 2, 1);
    // The click selects the table, as PowerPoint does.
    assert_eq!(deck.shape_at(0, x, y), Some(4));
    let target = deck.text_target(0, 4, Some((x, y)));
    assert_eq!(
        target,
        TextTarget::Cell {
            id: 4,
            row: 2,
            column: 1
        }
    );
    assert_eq!(deck.text(0, target).as_deref(), Some("Text 10"));

    assert!(deck.set_text(0, target, "売上\n前年比").unwrap());
    assert_eq!(deck.text(0, target).as_deref(), Some("売上\n前年比"));
    // The neighbours keep theirs.
    let left = TextTarget::Cell {
        id: 4,
        row: 2,
        column: 0,
    };
    assert_eq!(deck.text(0, left).as_deref(), Some("Text 9"));
    assert!(deck.is_dirty());
    // The saved deck reopens with the text in the cell.
    let saved = deck.save_bytes();
    let backend = TinySkiaBackend::new(&aliased_faces(
        Arc::new(
            std::fs::read(core().join("fonts/Noto_Sans_JP/static/NotoSansJP-Regular.ttf")).unwrap(),
        ),
        None,
    ))
    .unwrap();
    let mut reopened = Deck::open(&saved, backend).unwrap();
    assert_eq!(reopened.text(0, target).as_deref(), Some("売上\n前年比"));
    assert!(reopened.render_slide(0, 480).is_some());

    assert!(deck.undo().unwrap());
    assert_eq!(deck.text(0, target).as_deref(), Some("Text 10"));
}

#[test]
fn enter_on_a_selected_table_types_into_its_first_cell() {
    let mut deck = open("table-with-different-font-colors.pptx");
    let target = deck.text_target(0, 4, None);
    assert_eq!(
        target,
        TextTarget::Cell {
            id: 4,
            row: 0,
            column: 0
        }
    );
    assert_eq!(deck.text(0, target).as_deref(), Some("Text 1"));
}

#[test]
fn a_merged_cell_is_typed_into_where_it_starts() {
    let mut deck = open("table_test2.pptx");
    // Header2 spans the second and third columns: a click over the third
    // lands in the second, where its text is.
    let header = TextTarget::Cell {
        id: 4,
        row: 0,
        column: 1,
    };
    let rect = deck.text_rect(0, header).unwrap();
    let target = deck.text_target(0, 4, Some((rect.x + rect.width * 0.8, rect.y + 2.0)));
    assert_eq!(target, header);
    assert!(deck.set_text(0, target, "見出し").unwrap());
    assert_eq!(deck.text(0, header).as_deref(), Some("見出し"));
    // The cell it covers takes no text of its own.
    let covered = TextTarget::Cell {
        id: 4,
        row: 0,
        column: 2,
    };
    assert_eq!(deck.text(0, covered), None);
    assert!(deck.set_text(0, covered, "x").is_err());
}

#[test]
fn a_table_and_a_chart_resize_with_their_frames() {
    for (name, id) in [("table_test.pptx", 4), ("chart-slide-bg.pptx", 0)] {
        let mut deck = open(name);
        let id = if id == 0 {
            deck.shapes(0)
                .iter()
                .find(|shape| shape.kind == "chart")
                .expect("the slide has a chart")
                .id
        } else {
            id
        };
        let shape = deck.shape(0, id).unwrap();
        assert!(shape.can_resize, "{name}");
        let before = deck.shape_rect(0, id).unwrap();
        let wider = ShapeRect {
            width: before.width + 120.0,
            height: before.height - 20.0,
            ..before
        };
        assert!(deck.set_shape_rect(0, id, wider).unwrap(), "{name}");
        let after = deck.shape_rect(0, id).unwrap();
        assert!((after.width - wider.width).abs() < 0.01, "{name}");
        assert!((after.height - wider.height).abs() < 0.01, "{name}");
        assert!(deck.render_slide(0, 480).is_some(), "{name}");
        assert!(deck.undo().unwrap());
        assert_eq!(deck.shape_rect(0, id), Some(before), "{name}");
    }
}

#[test]
fn a_table_drawn_wider_has_its_cells_wider() {
    let mut deck = open("table_test.pptx");
    let first = TextTarget::Cell {
        id: 4,
        row: 0,
        column: 0,
    };
    let cell = deck.text_rect(0, first).unwrap();
    let frame = deck.shape_rect(0, 4).unwrap();
    assert!(deck
        .set_shape_rect(
            0,
            4,
            ShapeRect {
                width: frame.width * 1.5,
                ..frame
            }
        )
        .unwrap());
    let widened = deck.text_rect(0, first).unwrap();
    assert!(
        (widened.width - cell.width * 1.5).abs() < 1.0,
        "{cell:?} → {widened:?}"
    );
}

#[test]
fn a_smartart_diagram_moves_but_keeps_its_size() {
    let mut deck = open("SmartArt.pptx");
    let diagram = deck
        .shapes(0)
        .iter()
        .find(|shape| shape.kind == "diagram")
        .cloned()
        .expect("the slide has a diagram");
    assert!(diagram.can_move);
    assert!(!diagram.can_resize);
    let before = deck.shape_rect(0, diagram.id).unwrap();
    assert!(deck.move_shape(0, diagram.id, 24.0, 0.0).unwrap());
    let after = deck.shape_rect(0, diagram.id).unwrap();
    assert!((after.x - before.x - 24.0).abs() < 0.01);
    assert_eq!((after.width, after.height), (before.width, before.height));
    // Not a table: typed into as itself, as any other frame.
    assert_eq!(
        deck.text_target(0, diagram.id, None),
        TextTarget::Shape(diagram.id)
    );
}

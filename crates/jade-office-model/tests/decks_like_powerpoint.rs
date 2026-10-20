//! Tables, charts and SmartArt in decks made in PowerPoint (Apache POI's test
//! data) edit in the app as in PowerPoint: a double-click in a table types
//! into the cell under it, a merged cell is typed into where it starts, a
//! table or chart resizes with its frame, and a SmartArt diagram resizes with
//! its drawing scaled to the frame. A corner drag resizes from the opposite corner, and a body
//! drag moves the shape, each as one undoable step.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::deck::{
    step_size, Deck, DragMode, Insert, Motion, ParagraphAlignment, ParagraphSpacingState,
    ShapeDrag, ShapeRect, ShapeText, TableChange, TextCommand, TextTarget, MIN_SHAPE_PX,
    SHAPE_PRESETS,
};
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
fn a_smartart_diagram_moves_and_resizes_with_its_drawing() {
    let mut deck = open("SmartArt.pptx");
    let diagram = deck
        .shapes(0)
        .iter()
        .find(|shape| shape.kind == "diagram")
        .cloned()
        .expect("the slide has a diagram");
    assert!(diagram.can_move && diagram.can_resize);
    let before = deck.shape_rect(0, diagram.id).unwrap();
    assert!(deck.move_shape(0, diagram.id, 24.0, 0.0).unwrap());
    let moved = deck.shape_rect(0, diagram.id).unwrap();
    assert!((moved.x - before.x - 24.0).abs() < 0.01);
    // Resized narrower and shorter: its shapes are drawn inside the new box.
    let smaller = ShapeRect {
        width: moved.width * 0.6,
        height: moved.height * 0.5,
        ..moved
    };
    assert!(deck.set_shape_rect(0, diagram.id, smaller).unwrap());
    let layout = deck.layout(0).unwrap();
    let placed: Vec<_> = layout
        .shapes
        .iter()
        .filter(|shape| shape.group_ids.contains(&diagram.id))
        .collect();
    assert!(!placed.is_empty());
    for shape in placed {
        let rect = shape.rect;
        assert!(rect.x >= smaller.x - 1.0, "{rect:?} in {smaller:?}");
        assert!(
            rect.x + rect.width <= smaller.x + smaller.width + 1.0,
            "{rect:?}"
        );
        assert!(
            rect.y + rect.height <= smaller.y + smaller.height + 1.0,
            "{rect:?}"
        );
    }
    assert!(deck.render_slide(0, 480).is_some());
    assert!(deck.undo().unwrap());
    assert_eq!(deck.shape_rect(0, diagram.id), Some(moved));
    // Not a table: typed into as itself, as any other frame.
    assert_eq!(
        deck.text_target(0, diagram.id, None),
        TextTarget::Shape(diagram.id)
    );
}

/// A table's corner dragged as in PowerPoint: the opposite corner stays put,
/// the drag is one undoable step, and the preview the shells draw while the
/// mouse is down is the box the drop writes.
#[test]
fn a_corner_drag_resizes_from_the_opposite_corner() {
    let mut deck = open("table_test.pptx");
    let before = deck.shape_rect(0, 4).unwrap();
    let corner = (before.x, before.y);
    let mut drag = ShapeDrag::new(
        4,
        DragMode::Resize {
            left: true,
            top: true,
        },
        corner,
    );
    drag.drag_to(corner.0 - 30.0, corner.1 - 10.0);
    let preview = drag.rect(before);
    assert!(deck.drag_shape(0, &drag).unwrap());
    let after = deck.shape_rect(0, 4).unwrap();
    assert!(
        (after.x - preview.x).abs() < 0.01,
        "{preview:?} → {after:?}"
    );
    assert!((after.width - (before.width + 30.0)).abs() < 0.01);
    assert!((after.height - (before.height + 10.0)).abs() < 0.01);
    assert!((after.x + after.width - (before.x + before.width)).abs() < 0.01);
    assert!((after.y + after.height - (before.y + before.height)).abs() < 0.01);

    assert!(deck.undo().unwrap());
    assert_eq!(deck.shape_rect(0, 4), Some(before));
}

/// Dragged past the opposite edge, a shape stops at the smallest size.
#[test]
fn a_corner_dragged_past_the_far_edge_stops_at_the_smallest_size() {
    let rect = ShapeRect {
        x: 10.0,
        y: 20.0,
        width: 100.0,
        height: 50.0,
    };
    let mut drag = ShapeDrag::new(
        1,
        DragMode::Resize {
            left: false,
            top: false,
        },
        (110.0, 70.0),
    );
    drag.drag_to(-400.0, 70.0);
    let squashed = drag.rect(rect);
    assert_eq!((squashed.x, squashed.width), (10.0, MIN_SHAPE_PX));
    assert_eq!(squashed.height, 50.0);
    // From the left, the right edge stays where it was.
    let mut drag = ShapeDrag::new(
        1,
        DragMode::Resize {
            left: true,
            top: false,
        },
        (10.0, 70.0),
    );
    drag.drag_to(500.0, 70.0);
    let squashed = drag.rect(rect);
    assert_eq!(squashed.width, MIN_SHAPE_PX);
    assert_eq!(squashed.x + squashed.width, 110.0);
}

/// Dragging the body moves the shape, size kept, as one undoable step; a
/// press without a drag changes nothing.
#[test]
fn dragging_a_shape_moves_it() {
    let mut deck = open("table_test.pptx");
    let before = deck.shape_rect(0, 4).unwrap();
    let mut drag = ShapeDrag::new(4, DragMode::Move, (before.x + 5.0, before.y + 5.0));
    drag.drag_to(before.x + 5.2, before.y + 4.8);
    assert!(!drag.moved());
    assert!(!deck.drag_shape(0, &drag).unwrap());
    assert!(!deck.is_dirty());

    drag.drag_to(before.x + 45.0, before.y + 25.0);
    assert!(drag.moved());
    assert!(deck.drag_shape(0, &drag).unwrap());
    let after = deck.shape_rect(0, 4).unwrap();
    assert!((after.x - (before.x + 40.0)).abs() < 0.01);
    assert!((after.y - (before.y + 20.0)).abs() < 0.01);
    assert_eq!((after.width, after.height), (before.width, before.height));
    assert!(deck.undo().unwrap());
    assert_eq!(deck.shape_rect(0, 4), Some(before));
}

/// A SmartArt diagram's corner drags as any frame's does, from the opposite
/// corner, and dragging its body moves it.
#[test]
fn a_smartart_diagram_corner_and_body_drag() {
    let mut deck = open("SmartArt.pptx");
    let id = deck
        .shapes(0)
        .iter()
        .find(|shape| shape.kind == "diagram")
        .expect("the slide has a diagram")
        .id;
    let before = deck.shape_rect(0, id).unwrap();
    let mut resize = ShapeDrag::new(
        id,
        DragMode::Resize {
            left: false,
            top: false,
        },
        (before.x + before.width, before.y + before.height),
    );
    resize.drag_to(before.x + before.width + 50.0, before.y + before.height);
    assert!(deck.drag_shape(0, &resize).unwrap());
    let resized = deck.shape_rect(0, id).unwrap();
    assert_eq!((resized.x, resized.y), (before.x, before.y));
    assert!((resized.width - before.width - 50.0).abs() < 0.01);
    assert!(deck.undo().unwrap());
    assert_eq!(deck.shape_rect(0, id), Some(before));

    let mut moving = ShapeDrag::new(id, DragMode::Move, (before.x, before.y));
    moving.drag_to(before.x + 12.0, before.y);
    assert!(deck.drag_shape(0, &moving).unwrap());
    let after = deck.shape_rect(0, id).unwrap();
    assert!((after.x - before.x - 12.0).abs() < 0.01);
    assert_eq!((after.width, after.height), (before.width, before.height));
}

fn cell(id: u32, row: usize, column: usize) -> TextTarget {
    TextTarget::Cell { id, row, column }
}

/// Tab steps through a table's cells as in PowerPoint: along the row, then
/// to the next row's first; ⇧Tab back. A merged cell is one stop. Past the
/// last cell there is none — the shell adds a row there.
#[test]
fn tab_steps_through_the_cells_a_merge_being_one() {
    let deck = open("table_test2.pptx");
    assert_eq!(deck.next_cell(0, cell(4, 0, 0), false), Some(cell(4, 0, 1)));
    // Header2 covers columns 1 and 2.
    assert_eq!(deck.next_cell(0, cell(4, 0, 1), false), Some(cell(4, 0, 3)));
    assert_eq!(deck.next_cell(0, cell(4, 0, 5), false), Some(cell(4, 1, 0)));
    assert_eq!(deck.next_cell(0, cell(4, 1, 0), true), Some(cell(4, 0, 5)));
    assert_eq!(deck.next_cell(0, cell(4, 0, 0), true), None);
    let deck = open("table-with-different-font-colors.pptx");
    assert_eq!(deck.next_cell(0, cell(4, 3, 3), false), None);
}

/// A row added below the cell being typed into, and a column to its right,
/// are empty; removing them puts the table back. Each is one undoable step.
#[test]
fn rows_and_columns_are_added_and_removed_beside_a_cell() {
    let mut deck = open("table-with-different-font-colors.pptx");
    let height = deck.shape_rect(0, 4).unwrap().height;
    assert!(deck.change_table(0, 4, TableChange::InsertRow(2)).unwrap());
    assert_eq!(deck.text(0, cell(4, 2, 0)).as_deref(), Some(""));
    assert_eq!(deck.text(0, cell(4, 3, 0)).as_deref(), Some("Text 9"));
    assert!(deck.shape_rect(0, 4).unwrap().height > height);
    assert!(deck
        .change_table(0, 4, TableChange::InsertColumn(1))
        .unwrap());
    assert_eq!(deck.text(0, cell(4, 0, 1)).as_deref(), Some(""));
    assert_eq!(deck.text(0, cell(4, 0, 2)).as_deref(), Some("Text 2"));
    // The new cell takes text, and the slide draws.
    assert!(deck.set_text(0, cell(4, 2, 1), "新").unwrap());
    assert!(deck.render_slide(0, 480).is_some());
    assert!(deck
        .change_table(0, 4, TableChange::DeleteColumn(3))
        .unwrap());
    assert_eq!(deck.text(0, cell(4, 0, 3)).as_deref(), Some("Text 4"));
    for _ in 0..4 {
        assert!(deck.undo().unwrap());
    }
    assert_eq!(deck.text(0, cell(4, 2, 0)).as_deref(), Some("Text 9"));
    assert!(!deck.is_dirty());
    // Cutting a merge is refused and changes nothing.
    let mut deck = open("table_test2.pptx");
    assert!(deck
        .change_table(0, 4, TableChange::DeleteColumn(2))
        .is_err());
    assert!(!deck.is_dirty());
}

/// Cells selected across a table merge into one, as PowerPoint's Merge
/// Cells does: the selection's box is the merged cell's, its text the
/// cells' paragraphs; the merged cell splits back into its cells.
#[test]
fn a_selected_range_merges_into_one_cell_and_splits_back() {
    let mut deck = open("table-with-different-font-colors.pptx");
    // The selection, dragged from (2, 2) back to (1, 1).
    let range = deck.cell_range_rect(0, 4, (2, 2), (1, 1)).unwrap();
    let first = deck.text_rect(0, cell(4, 1, 1)).unwrap();
    let last = deck.text_rect(0, cell(4, 2, 2)).unwrap();
    assert_eq!((range.x, range.y), (first.x, first.y));
    assert!((range.x + range.width - (last.x + last.width)).abs() < 0.01);
    assert!((range.y + range.height - (last.y + last.height)).abs() < 0.01);
    assert_eq!(deck.cell_span(0, cell(4, 1, 1)), Some((1, 1)));

    let merge = jade_office_model::deck::merge_cells((2, 2), (1, 1));
    assert!(deck.change_table(0, 4, merge).unwrap());
    assert_eq!(
        deck.text(0, cell(4, 1, 1)).as_deref(),
        Some("Text 6\nText 7\nText 10\nText 11")
    );
    assert_eq!(deck.text(0, cell(4, 2, 2)), None);
    assert_eq!(deck.cell_span(0, cell(4, 1, 1)), Some((2, 2)));
    let merged = deck.text_rect(0, cell(4, 1, 1)).unwrap();
    assert!((merged.width - range.width).abs() < 0.01);
    assert!(merged.height >= range.height - 0.01);
    // A double-click anywhere over it types into it.
    let (x, y) = (last.x + last.width / 2.0, last.y + last.height / 2.0);
    assert_eq!(deck.text_target(0, 4, Some((x, y))), cell(4, 1, 1));
    assert!(deck.render_slide(0, 480).is_some());

    assert!(deck
        .change_table(0, 4, TableChange::SplitCell { row: 1, column: 1 })
        .unwrap());
    assert_eq!(deck.cell_span(0, cell(4, 1, 1)), Some((1, 1)));
    assert_eq!(deck.text(0, cell(4, 2, 2)).as_deref(), Some(""));
}

fn backend() -> TinySkiaBackend {
    let directory = core().join("fonts/Noto_Sans_JP/static");
    TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        None,
    ))
    .unwrap()
}

/// One of the decks in jade-core's own fixtures, not POI's.
fn open_own(name: &str) -> Deck<TinySkiaBackend> {
    let bytes = std::fs::read(core().join("public/fixtures").join(name)).unwrap();
    Deck::open(&bytes, backend()).unwrap()
}

/// The slides' first lines of text, in order: which slide is where.
fn titles(deck: &mut Deck<TinySkiaBackend>) -> Vec<String> {
    (0..deck.slide_count())
        .map(|slide| {
            let ids: Vec<u32> = deck.shapes(slide).iter().map(|shape| shape.id).collect();
            ids.into_iter()
                .filter_map(|id| deck.shape_text(slide, id))
                .find(|text| !text.is_empty())
                .unwrap_or_default()
        })
        .collect()
}

#[test]
fn slides_are_added_duplicated_moved_and_deleted_each_as_one_undoable_step() {
    let mut deck = open_own("sample-deck.pptx");
    let before = titles(&mut deck);
    let count = before.len();
    assert!(count >= 2);

    // New Slide goes after the current one, on its layout, with nothing typed.
    assert!(deck.insert_slide(1).unwrap());
    assert_eq!(deck.slide_count(), count + 1);
    assert_eq!(titles(&mut deck)[1], "");
    assert!(deck.render_slide(1, 320).is_some());

    // Duplicate puts the copy straight after its slide.
    assert!(deck.duplicate_slide(0).unwrap());
    let after = titles(&mut deck);
    assert_eq!(after[0], after[1]);
    assert_eq!(after[0], before[0]);

    // A slide dragged to the end of the list goes last.
    assert!(deck.move_slide(0, deck.slide_count() - 1).unwrap());
    assert_eq!(titles(&mut deck).last(), Some(&before[0]));

    assert!(deck.delete_slide(0).unwrap());
    assert_eq!(deck.slide_count(), count + 1);

    for _ in 0..4 {
        assert!(deck.undo().unwrap());
    }
    assert_eq!(titles(&mut deck), before);
    assert!(!deck.is_dirty());
}

#[test]
fn the_last_slide_is_not_deleted() {
    let mut deck = open_own("sample-deck.pptx");
    while deck.slide_count() > 1 {
        assert!(deck.delete_slide(0).unwrap());
    }
    // PowerPoint leaves an empty deck; the views need a slide to show.
    assert!(!deck.delete_slide(0).unwrap());
    assert_eq!(deck.slide_count(), 1);
}

#[test]
fn a_hidden_slide_is_skipped_by_the_slide_show() {
    let mut deck = open_own("sample-deck.pptx");
    let count = deck.slide_count();
    assert!(count >= 3);
    assert!(!deck.is_hidden(1));
    assert!(deck.set_slide_hidden(1, true).unwrap());
    assert!(deck.is_hidden(1));
    assert_eq!(deck.next_shown(0, true), Some(2));
    assert_eq!(deck.next_shown(2, false), Some(0));
    assert_eq!(deck.next_shown(count - 1, true), None);
    // The flag is written to the file, and read back as PowerPoint reads it.
    let saved = deck.save_bytes();
    let reopened = Deck::open(&saved, backend()).unwrap();
    assert!(reopened.is_hidden(1));
    // A show from the beginning starts on the first slide shown.
    assert!(deck.set_slide_hidden(0, true).unwrap());
    assert_eq!(deck.first_shown(), 2);
    assert!(deck.set_slide_hidden(1, false).unwrap());
    assert_eq!(deck.first_shown(), 1);
}

#[test]
fn a_slide_takes_another_layout() {
    let mut deck = open_own("sample-deck.pptx");
    let layouts = deck.slide_layouts(1);
    assert!(layouts.len() > 1, "{layouts:?}");
    let current = layouts
        .iter()
        .find(|layout| layout.current)
        .unwrap()
        .clone();
    let other = layouts
        .iter()
        .find(|layout| !layout.current)
        .unwrap()
        .clone();
    assert!(deck.set_slide_layout(1, &other.part_name).unwrap());
    let now = deck.slide_layouts(1);
    assert_eq!(
        now.iter()
            .find(|layout| layout.current)
            .map(|l| &l.part_name),
        Some(&other.part_name)
    );
    assert!(deck.render_slide(1, 320).is_some());
    assert!(deck.undo().unwrap());
    assert_eq!(
        deck.slide_layouts(1)
            .iter()
            .find(|layout| layout.current)
            .map(|l| l.part_name.clone()),
        Some(current.part_name)
    );
}

/// The first shape on `slide` with text that may be edited, and its text.
fn text_shape(deck: &mut Deck<TinySkiaBackend>, slide: usize) -> (u32, String) {
    let ids: Vec<u32> = deck.shapes(slide).iter().map(|shape| shape.id).collect();
    ids.into_iter()
        .find_map(|id| {
            deck.shape_text(slide, id)
                .filter(|text| text.chars().count() >= 4)
                .map(|text| (id, text))
        })
        .expect("the slide has a text shape")
}

#[test]
fn typing_in_place_goes_in_at_the_caret_and_keeps_the_runs_formatting() {
    let mut deck = open_own("sample-deck.pptx");
    let (id, before) = text_shape(&mut deck, 0);
    let first = before.chars().next().unwrap().to_string();
    let format_before = deck.text_format(0, id, 0..1);

    // A click at the text's start puts the caret there.
    let mut text = ShapeText::begin(&mut deck, 0, id, None).unwrap();
    assert_eq!(text.focus, before.encode_utf16().count());
    text.move_caret(&mut deck, Motion::Start, false);
    assert_eq!(text.focus, 0);
    text.move_caret(&mut deck, Motion::Right, false);
    for ch in ["新", "し", "い"] {
        assert!(text.type_text(&mut deck, ch).unwrap());
    }
    let after = deck.shape_text(0, id).unwrap();
    assert_eq!(after, format!("{first}新しい{}", &before[first.len()..]));
    assert_eq!(text.focus, first.encode_utf16().count() + 3);
    // What was typed took the formatting of the run it went into.
    assert_eq!(deck.text_format(0, id, 1..4), format_before);

    // Backspace and Delete take one character each side.
    assert!(text.backspace(&mut deck).unwrap());
    assert!(text.delete(&mut deck).unwrap());
    let shorter = deck.shape_text(0, id).unwrap();
    assert_eq!(shorter.chars().count(), after.chars().count() - 2);

    // The typing was one step of Undo, each deletion one more.
    assert!(deck.undo().unwrap());
    assert!(deck.undo().unwrap());
    assert_eq!(deck.shape_text(0, id).unwrap(), after);
    assert!(deck.undo().unwrap());
    assert_eq!(deck.shape_text(0, id).unwrap(), before);
    assert!(!deck.is_dirty());
}

#[test]
fn a_selection_is_made_with_shift_and_replaced_by_what_is_typed() {
    let mut deck = open_own("sample-deck.pptx");
    let (id, before) = text_shape(&mut deck, 0);
    let mut text = ShapeText::begin(&mut deck, 0, id, None).unwrap();
    text.move_caret(&mut deck, Motion::Start, false);
    text.move_caret(&mut deck, Motion::Right, true);
    text.move_caret(&mut deck, Motion::Right, true);
    assert_eq!(text.selection(), 0..2);
    // The selection is drawn over the first two characters, the caret after
    // them.
    let geometry = text.geometry(&mut deck).unwrap();
    assert_eq!(geometry.highlights.len(), 1);
    assert!(geometry.caret[0].x > geometry.highlights[0][0].x);
    assert!(text.type_text(&mut deck, "X").unwrap());
    let skip: String = before.chars().skip(2).collect();
    assert_eq!(deck.shape_text(0, id).unwrap(), format!("X{skip}"));
    // ⌘A, then Left: the caret at the start.
    text.select_all(&deck);
    assert_eq!(text.selection().start, 0);
    text.move_caret(&mut deck, Motion::Left, false);
    assert_eq!(text.focus, 0);
}

#[test]
fn a_click_puts_the_caret_where_it_lands_and_arrows_go_down_a_line() {
    let mut deck = open_own("sample-deck.pptx");
    let (id, before) = text_shape(&mut deck, 0);
    let mut text = ShapeText::begin(&mut deck, 0, id, None).unwrap();
    text.move_caret(&mut deck, Motion::Start, false);
    let start = text.geometry(&mut deck).unwrap().caret;
    // A click a little right of the start of the text: a caret a character
    // or so in.
    let (x, y) = (start[0].x + 1.0, (start[0].y + start[1].y) / 2.0);
    let mut clicked = ShapeText::begin(&mut deck, 0, id, Some((x, y))).unwrap();
    assert!(clicked.focus <= 1, "{}", clicked.focus);
    // End goes to the line's end; Start and End to the text's.
    clicked.move_caret(&mut deck, Motion::LineEnd, false);
    assert!(clicked.focus > 0);
    clicked.move_caret(&mut deck, Motion::End, false);
    assert_eq!(clicked.focus, before.encode_utf16().count());
    clicked.move_caret(&mut deck, Motion::WordLeft, false);
    assert!(clicked.focus < before.encode_utf16().count());
}

#[test]
fn enter_starts_a_paragraph_and_a_new_slide_placeholder_takes_text() {
    let mut deck = open_own("sample-deck.pptx");
    assert!(deck.insert_slide(1).unwrap());
    let ids: Vec<u32> = deck.shapes(1).iter().map(|shape| shape.id).collect();
    let id = ids
        .into_iter()
        .find(|id| deck.shape_text(1, *id).is_some())
        .expect("the new slide has a placeholder");
    let mut text = ShapeText::begin(&mut deck, 1, id, None).unwrap();
    assert!(text.type_text(&mut deck, "一行目").unwrap());
    assert!(text.type_text(&mut deck, "\n").unwrap());
    assert!(text.type_text(&mut deck, "二行目").unwrap());
    assert_eq!(deck.shape_text(1, id).unwrap(), "一行目\n二行目");
    let geometry = text.geometry(&mut deck).unwrap();
    text.move_caret(&mut deck, Motion::Up, false);
    let up = text.geometry(&mut deck).unwrap();
    assert!(up.caret[0].y < geometry.caret[0].y);
    assert!(text.focus <= 3);
}

#[test]
fn an_input_methods_composition_shows_at_the_caret_and_goes_in_when_committed() {
    let mut deck = open_own("sample-deck.pptx");
    let (id, before) = text_shape(&mut deck, 0);
    let mut text = ShapeText::begin(&mut deck, 0, id, None).unwrap();
    text.compose(None, "にほ");
    let (seen, selection, marked) = text.input_text(&deck);
    assert_eq!(seen, format!("{before}にほ"));
    let end = before.encode_utf16().count();
    assert_eq!(marked, Some(end..end + 2));
    assert_eq!(selection, end + 2..end + 2);
    // Nothing is in the slide until the composition is committed.
    assert_eq!(deck.shape_text(0, id).unwrap(), before);
    assert!(text.commit(&mut deck, None, "日本").unwrap());
    assert_eq!(deck.shape_text(0, id).unwrap(), format!("{before}日本"));
    assert_eq!(text.composition, None);
    assert_eq!(text.focus, end + 2);
}

/// `#C00000` and `c00000` alike.
fn same_colour(a: Option<&str>, b: &str) -> bool {
    a.is_some_and(|a| {
        a.trim_start_matches('#')
            .eq_ignore_ascii_case(b.trim_start_matches('#'))
    })
}

#[test]
fn a_selection_takes_character_formatting_and_its_paragraph_paragraph_formatting() {
    let mut deck = open_own("sample-deck.pptx");
    let (id, _) = text_shape(&mut deck, 0);
    let untouched = deck.text_format(0, id, 3..4);
    let mut text = ShapeText::begin(&mut deck, 0, id, None).unwrap();
    text.move_caret(&mut deck, Motion::Start, false);
    text.move_caret(&mut deck, Motion::Right, true);
    text.move_caret(&mut deck, Motion::Right, true);
    let before = text.format_strip(&deck);
    let size = before.size.expect("one size");

    assert!(text.format(&mut deck, &TextCommand::Bold).unwrap());
    assert_eq!(text.format_strip(&deck).bold, !before.bold);
    assert!(text.format(&mut deck, &TextCommand::Italic).unwrap());
    assert!(text.format(&mut deck, &TextCommand::Grow).unwrap());
    assert_eq!(text.format_strip(&deck).size, Some(step_size(size, true)));
    assert!(text
        .format(&mut deck, &TextCommand::Font("Meiryo".into()))
        .unwrap());
    assert_eq!(text.format_strip(&deck).font.as_deref(), Some("Meiryo"));
    assert!(text
        .format(&mut deck, &TextCommand::Color("#C00000".into()))
        .unwrap());
    assert!(same_colour(
        text.format_strip(&deck).color.as_deref(),
        "C00000"
    ));
    // Characters outside the selection keep theirs.
    assert_eq!(deck.text_format(0, id, 3..4), untouched);

    assert!(text
        .format(&mut deck, &TextCommand::Align(ParagraphAlignment::Center))
        .unwrap());
    assert_eq!(
        text.format_strip(&deck).alignment,
        Some(ParagraphAlignment::Center)
    );
    assert!(text.format(&mut deck, &TextCommand::Bullets).unwrap());
    assert!(text.format_strip(&deck).bullets);
    assert!(text.format(&mut deck, &TextCommand::Numbering).unwrap());
    let strip = text.format_strip(&deck);
    assert!(strip.numbering && !strip.bullets);
    assert!(text.format(&mut deck, &TextCommand::Numbering).unwrap());
    assert!(!text.format_strip(&deck).numbering);
    assert!(text
        .format(&mut deck, &TextCommand::LineSpacing(1.5))
        .unwrap());
    assert!(text
        .format(&mut deck, &TextCommand::SpaceAfter(12.0))
        .unwrap());
    let paragraph = deck.paragraph_format(0, id, text.selection()).unwrap();
    assert_eq!(
        paragraph.line_spacing,
        Some(ParagraphSpacingState::Percent(150.0))
    );
    assert_eq!(
        paragraph.space_after,
        Some(ParagraphSpacingState::Points(12.0))
    );
    // Saved and reopened, the formatting is there.
    let saved = deck.save_bytes();
    let reopened = Deck::open(&saved, backend()).unwrap();
    let strip = reopened.text_format(0, id, 0..2).unwrap();
    assert_eq!(strip.font.as_deref(), Some("Meiryo"));
}

#[test]
fn with_nothing_selected_formatting_goes_to_what_is_typed_next() {
    let mut deck = open_own("sample-deck.pptx");
    let (id, before) = text_shape(&mut deck, 0);
    let end = before.encode_utf16().count();
    let mut text = ShapeText::begin(&mut deck, 0, id, None).unwrap();
    let bold = text.format_strip(&deck).bold;
    // Nothing changes yet; the strip shows what typing will take.
    assert!(!text.format(&mut deck, &TextCommand::Bold).unwrap());
    assert_eq!(text.format_strip(&deck).bold, !bold);
    assert!(!text.format(&mut deck, &TextCommand::Size(40.0)).unwrap());
    assert!(text.type_text(&mut deck, "AB").unwrap());
    let typed = deck.text_format(0, id, end..end + 2).unwrap();
    assert_eq!(typed.bold, Some(!bold));
    assert_eq!(typed.size_pt, Some(40.0));
    // The text before keeps its own.
    assert_eq!(deck.text_format(0, id, 0..1).unwrap().bold, Some(bold));
}

#[test]
fn a_selected_shape_is_formatted_as_a_whole() {
    let mut deck = open_own("sample-deck.pptx");
    let (id, before) = text_shape(&mut deck, 0);
    let bold = deck.shape_format(0, id).unwrap().bold;
    assert!(deck.format_shape(0, id, &TextCommand::Bold).unwrap());
    let end = before.encode_utf16().count();
    assert_eq!(deck.text_format(0, id, 0..end).unwrap().bold, Some(!bold));
    assert_eq!(deck.shape_format(0, id).unwrap().bold, !bold);
    assert!(deck.undo().unwrap());
    assert!(!deck.is_dirty());
}

#[test]
fn font_sizes_step_through_powerpoints_list() {
    assert_eq!(step_size(18.0, true), 20.0);
    assert_eq!(step_size(18.0, false), 16.0);
    assert_eq!(step_size(17.0, true), 18.0);
    assert_eq!(step_size(72.0, true), 80.0);
    assert_eq!(step_size(8.0, false), 8.0);
}

fn close(a: ShapeRect, b: ShapeRect) -> bool {
    (a.x - b.x).abs() < 0.01
        && (a.y - b.y).abs() < 0.01
        && (a.width - b.width).abs() < 0.01
        && (a.height - b.height).abs() < 0.01
}

#[test]
fn a_text_box_drawn_on_a_slide_is_typed_into() {
    let mut deck = open_own("sample-deck.pptx");
    let before = deck.shapes(1).len();
    let rect = Insert::TextBox.rect((400.0, 180.0), (100.0, 120.0));
    assert!(close(
        rect,
        ShapeRect {
            x: 100.0,
            y: 120.0,
            width: 300.0,
            height: 60.0
        }
    ));
    let id = deck.insert(1, Insert::TextBox, rect).unwrap();
    assert_eq!(deck.shapes(1).len(), before + 1);
    assert!(close(deck.shape_rect(1, id).unwrap(), rect));
    // In front of everything: a click on it selects it.
    assert_eq!(deck.shape_at(1, 250.0, 150.0), Some(id));
    let mut text = ShapeText::begin(&mut deck, 1, id, None).unwrap();
    assert!(text.type_text(&mut deck, "新しいテキスト").unwrap());
    assert_eq!(deck.shape_text(1, id).as_deref(), Some("新しいテキスト"));
    assert!(deck.render_slide(1, 320).is_some());
    assert!(deck.undo().unwrap());
    assert!(deck.undo().unwrap());
    assert_eq!(deck.shapes(1).len(), before);
    assert!(!deck.is_dirty());
}

#[test]
fn a_click_drops_a_shape_at_its_usual_size() {
    let mut deck = open_own("sample-deck.pptx");
    for preset in SHAPE_PRESETS {
        let rect = Insert::Shape(preset).rect((50.0, 60.0), (51.0, 60.0));
        assert!(close(
            rect,
            ShapeRect {
                x: 50.0,
                y: 60.0,
                width: 96.0,
                height: 96.0
            }
        ));
        let id = deck.insert(0, Insert::Shape(preset), rect).unwrap();
        assert!(close(deck.shape_rect(0, id).unwrap(), rect));
        // A shape takes text, as PowerPoint's do.
        assert_eq!(deck.shape_text(0, id).as_deref(), Some(""));
    }
    assert!(deck.render_slide(0, 320).is_some());
}

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_image_data(&vec![180u8; (width * height * 3) as usize])
            .unwrap();
    }
    bytes
}

#[test]
fn a_picture_goes_in_the_middle_of_the_slide_shrunk_to_fit() {
    let mut deck = open_own("sample-deck.pptx");
    let (slide_width, slide_height) = deck.slide_size();
    // Small enough: its own size, in the middle.
    let id = deck.insert_picture(0, &png(120, 60)).unwrap();
    let rect = deck.shape_rect(0, id).unwrap();
    assert!(close(
        rect,
        ShapeRect {
            x: (slide_width - 120.0) / 2.0,
            y: (slide_height - 60.0) / 2.0,
            width: 120.0,
            height: 60.0
        }
    ));
    // Too big: shrunk to fit, its shape kept.
    let id = deck.insert_picture(0, &png(4000, 1000)).unwrap();
    let rect = deck.shape_rect(0, id).unwrap();
    assert!(rect.width <= slide_width * 0.9 + 0.01);
    assert!((rect.width / rect.height - 4.0).abs() < 0.01);
    assert!(deck.render_slide(0, 320).is_some());
    // Saved and reopened, the picture is there.
    let saved = deck.save_bytes();
    let mut reopened = Deck::open(&saved, backend()).unwrap();
    assert!(reopened.shape_rect(0, id).is_some());
    // Anything but PNG or JPEG is refused, and nothing changes.
    assert!(deck.insert_picture(0, b"GIF89a").is_err());
}

#[test]
fn speaker_notes_are_typed_and_written_as_one_step() {
    let mut deck = open_own("sample-deck.pptx");
    let count = deck.slide_count();
    let with_notes = (0..count).find(|&slide| deck.notes(slide).is_some());
    // A slide without notes gets them.
    let bare = (0..count)
        .find(|&slide| deck.notes_text(slide).as_deref() == Some(""))
        .expect("a slide without notes");
    assert!(deck.set_notes(bare, "話すこと\n次の話").unwrap());
    assert_eq!(deck.notes_text(bare).as_deref(), Some("話すこと\n次の話"));
    assert_eq!(deck.notes(bare), Some("話すこと\n次の話"));
    assert!(!deck.set_notes(bare, "話すこと\n次の話").unwrap());
    // Notes that were there keep what is not changed.
    if let Some(slide) = with_notes {
        let before = deck.notes_text(slide).unwrap();
        assert!(deck.set_notes(slide, &format!("{before} 追記")).unwrap());
        assert_eq!(deck.notes_text(slide).unwrap(), format!("{before} 追記"));
        assert!(deck.undo().unwrap());
        assert_eq!(deck.notes_text(slide).unwrap(), before);
    }
    // Saved and reopened, the notes are there.
    let saved = deck.save_bytes();
    let reopened = Deck::open(&saved, backend()).unwrap();
    assert_eq!(reopened.notes(bare), Some("話すこと\n次の話"));
    assert!(deck.undo().unwrap());
    assert_eq!(deck.notes_text(bare).as_deref(), Some(""));
}

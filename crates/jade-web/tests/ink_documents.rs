//! The 描画 tab's model over the browser build's Word document: strokes go
//! in as Word ink and are drawn, erased, moved, restyled and turned into
//! shapes, and survive a save.

mod common;

use jade_office_model::word::ink_tools::{default_pens, WordInkTools};
use jade_web::word::new::{blank_document, Blank};

fn pixels(document: &jade_web::editing::EditableDocument) -> Vec<u8> {
    document.page(0).expect("first page").rgba_premultiplied()
}

fn line(from: (f64, f64), to: (f64, f64), steps: usize) -> Vec<(f64, f64)> {
    (0..=steps)
        .map(|step| {
            let t = step as f64 / steps as f64;
            (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t)
        })
        .collect()
}

#[test]
fn ink_is_drawn_erased_and_saved() {
    let mut document = common::open_bytes(&blank_document(Blank::English));
    document.place_caret(0, 0);
    let blank = pixels(&document);
    let pens = default_pens();

    document
        .draw_stroke(0, line((200.0, 300.0), (400.0, 360.0), 20), &pens[1], false)
        .unwrap();
    let inked = pixels(&document);
    assert!(blank != inked, "the stroke is drawn");
    // Red where the stroke passes.
    let width = document.page(0).unwrap().width() as usize;
    let scale = width as f64 / document.page_size().0;
    let at = |x: f64, y: f64| {
        let index = ((y * scale) as usize * width + (x * scale) as usize) * 4;
        [inked[index], inked[index + 1], inked[index + 2]]
    };
    let [r, g, b] = at(300.0, 330.0);
    assert!(r > 180 && g < 90 && b < 90, "{r} {g} {b}");

    // Saved and opened again, it is still ink.
    let reopened = common::open_bytes(&document.export_docx().unwrap());
    let strokes = reopened.ink_strokes();
    assert_eq!(strokes.len(), 1);
    assert_eq!(strokes[0].trace.brush.color, "#E71224");

    // A highlighter stroke, moved and restyled; then everything erased.
    document
        .draw_stroke(0, line((200.0, 500.0), (400.0, 500.0), 10), &pens[3], false)
        .unwrap();
    assert_eq!(document.ink_strokes().len(), 2);
    let selected = document.lasso_strokes(
        0,
        &[
            (150.0, 450.0),
            (450.0, 450.0),
            (450.0, 550.0),
            (150.0, 550.0),
        ],
    );
    assert_eq!(selected.len(), 1);
    document
        .transform_strokes(&selected, (100.0, 600.0, 300.0, 640.0))
        .unwrap();
    assert!(document.ink_strokes().iter().any(|stroke| stroke
        .trace
        .points
        .iter()
        .all(|point| point.1 > 590.0)));
    assert!(document.erase_strokes_at(0, (300.0, 330.0), 6.0).unwrap());
    assert_eq!(document.ink_strokes().len(), 1);
    document.undo().unwrap();
    assert_eq!(document.ink_strokes().len(), 2);
}

#[test]
fn ink_to_shape_makes_a_shape() {
    let mut document = common::open_bytes(&blank_document(Blank::English));
    document.place_caret(0, 0);
    let pens = default_pens();
    let mut ellipse = Vec::new();
    for step in 0..=48 {
        let angle = step as f64 / 48.0 * std::f64::consts::TAU;
        ellipse.push((300.0 + 80.0 * angle.cos(), 400.0 + 40.0 * angle.sin()));
    }
    let shape = document.draw_stroke(0, ellipse, &pens[0], true).unwrap();
    assert!(shape.is_some());
    assert!(document.ink_strokes().is_empty());
    use jade_web::editing::WordSession;
    let shapes = document
        .word_document()
        .document
        .as_ref()
        .unwrap()
        .body
        .iter()
        .flat_map(|body| body.elements.iter())
        .filter_map(|element| element.as_paragraph())
        .flat_map(|paragraph| paragraph.runs.iter())
        .filter(|run| run.text_box.is_some())
        .count();
    assert_eq!(shapes, 1);
}

/// A perfectly straight vertical or horizontal stroke has no extent across
/// it: as ink it is drawn in a box as wide as the stroke, and with
/// インクを図形に変換 it becomes a line shape a pixel across, drawn level or
/// upright — not a slope across a box made a text line tall.
#[test]
fn straight_strokes_stay_lines_as_ink_and_as_shapes() {
    let pens = default_pens();
    for ink_to_shape in [false, true] {
        for (from, to) in [
            ((300.0, 200.0), (300.0, 400.0)),
            ((200.0, 300.0), (400.0, 300.0)),
        ] {
            let mut document = common::open_bytes(&blank_document(Blank::English));
            document.place_caret(0, 0);
            let blank = pixels(&document);
            let shape = document
                .draw_stroke(0, line(from, to, 20), &pens[1], ink_to_shape)
                .unwrap();
            assert_eq!(shape.is_some(), ink_to_shape, "{from:?} {ink_to_shape}");
            let drawn = pixels(&document);
            let width = document.page(0).unwrap().width() as usize;
            let scale = width as f64 / document.page_size().0;
            let changed: Vec<(f64, f64)> = blank
                .chunks(4)
                .zip(drawn.chunks(4))
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .map(|(index, _)| {
                    (
                        (index % width) as f64 / scale,
                        (index / width) as f64 / scale,
                    )
                })
                .collect();
            assert!(changed.len() > 100, "the line is drawn");
            // Nothing far from the line changes, but for the anchor mark at
            // the paragraph's start.
            let far = changed
                .iter()
                .filter(|&&(x, y)| {
                    let near = x >= from.0.min(to.0) - 8.0
                        && x <= from.0.max(to.0) + 8.0
                        && y >= from.1.min(to.1) - 8.0
                        && y <= from.1.max(to.1) + 8.0;
                    !near && y > 150.0
                })
                .count();
            assert_eq!(far, 0, "{from:?} {ink_to_shape}: {far} pixels off the line");
        }
    }
}

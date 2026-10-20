//! A sheet painted in pieces — tiles of cells fixed in sheet space, each
//! pane a window onto them, and the header bands over their edges — is the
//! sheet painted in one picture, scrolled anywhere, frozen panes included.
//! The browser shell keeps the pieces between frames so that scrolling and
//! moving the selection paint only what changed.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::grid::Selection;
use jade_office_model::sheet::{Axis, CellRef, Overlay, Workbook};
use jade_office_model::tiny::TinySkiaBackend;

/// Device px per sheet px: a Retina screen.
const SCALE: f64 = 2.0;
/// A tile's side in device px, as the shell cuts them.
const TILE: u32 = 512;
/// The viewport in device px.
const VIEWPORT: (u32, u32) = (1500, 1000);

fn open(fixture: &str) -> Workbook<TinySkiaBackend> {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let directory = core.join("fonts/Noto_Sans_JP/static");
    let backend = TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        None,
    ))
    .unwrap();
    let bytes = std::fs::read(core.join("public/fixtures").join(fixture)).unwrap();
    Workbook::open(&bytes, "UTC", backend).unwrap()
}

/// Pixel (x, y) of an RGBA surface `width` px wide.
fn at(pixels: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let start = ((y * width + x) * 4) as usize;
    pixels[start..start + 4].try_into().unwrap()
}

/// The viewport at `scroll` (sheet px) put together from the pieces, the way
/// the shell lays them out.
fn assembled(
    book: &Workbook<TinySkiaBackend>,
    scroll: (f64, f64),
    selection: Selection,
) -> Vec<u8> {
    let (width, height) = VIEWPORT;
    let size = (f64::from(width) / SCALE, f64::from(height) / SCALE);
    let span = f64::from(TILE) / SCALE;
    let mut out = vec![0u8; (width * height * 4) as usize];
    let mut put = |x: u32, y: u32, pixel: [u8; 4]| {
        if x < width && y < height {
            let start = ((y * width + x) * 4) as usize;
            out[start..start + 4].copy_from_slice(&pixel);
        }
    };
    let mut tiles = std::collections::HashMap::new();
    for ((left, top), (x, y, w, h)) in book.panes(scroll, size) {
        let origin = ((left * SCALE).round() as i64, (top * SCALE).round() as i64);
        let shown = ((x * SCALE).round() as i64, (y * SCALE).round() as i64);
        let extent = ((w * SCALE).round() as i64, (h * SCALE).round() as i64);
        for dy in 0..extent.1 {
            for dx in 0..extent.0 {
                // The pixel's place in sheet space, in device px, and the
                // tile holding it.
                let (sx, sy) = (shown.0 + dx, shown.1 + dy);
                let tile = (
                    sx.div_euclid(i64::from(TILE)),
                    sy.div_euclid(i64::from(TILE)),
                );
                let pixels = tiles.entry(tile).or_insert_with(|| {
                    book.render_cells(
                        (tile.0 as f64 * span, tile.1 as f64 * span, span, span),
                        TILE,
                        TILE,
                        SCALE,
                    )
                    .unwrap()
                    .rgba_premultiplied()
                });
                let pixel = at(
                    pixels,
                    TILE,
                    sx.rem_euclid(i64::from(TILE)) as u32,
                    sy.rem_euclid(i64::from(TILE)) as u32,
                );
                put((origin.0 + dx) as u32, (origin.1 + dy) as u32, pixel);
            }
        }
    }
    // The row band, then the column band over it, as the shell stacks them.
    let metrics = book.header_metrics();
    let below = (metrics.column_height * SCALE).round() as u32;
    let rows = book
        .render_row_headers(scroll, size, height - below, SCALE, Some(selection))
        .unwrap();
    let row_pixels = rows.rgba_premultiplied();
    for y in 0..rows.height() {
        for x in 0..rows.width() {
            put(x, below + y, at(&row_pixels, rows.width(), x, y));
        }
    }
    let columns = book
        .render_column_headers(scroll, size, width, SCALE, Some(selection))
        .unwrap();
    let column_pixels = columns.rgba_premultiplied();
    for y in 0..columns.height() {
        for x in 0..columns.width() {
            put(x, y, at(&column_pixels, columns.width(), x, y));
        }
    }
    out
}

/// The share of pixels more than a few levels apart.
fn differing(a: &[u8], b: &[u8]) -> f64 {
    let off = a
        .chunks_exact(4)
        .zip(b.chunks_exact(4))
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(a, b)| a.abs_diff(*b) > 24))
        .count();
    off as f64 / (a.len() / 4) as f64
}

fn check(fixture: &str, scrolls: &[(f64, f64)]) {
    let book = open(fixture);
    // A selection whose rows and columns the bands highlight; the selection
    // itself the shell draws over the tiles, so the picture is painted
    // without it.
    let selection = Selection {
        active: CellRef::new(2, 3),
        anchor: CellRef::new(2, 3),
        focus: CellRef::new(4, 6),
    };
    for &scroll in scrolls {
        let pieces = assembled(&book, scroll, selection);
        let whole = book
            .render_viewport(
                scroll,
                VIEWPORT.0,
                VIEWPORT.1,
                SCALE,
                &Overlay {
                    selection: None,
                    ..Overlay::default()
                },
            )
            .unwrap()
            .rgba_premultiplied();
        // The bands' highlight is the selection's; the picture has none, so
        // compare the bands with a picture painted with it, and the cells
        // with one painted without.
        let with_bands = book
            .render_viewport(
                scroll,
                VIEWPORT.0,
                VIEWPORT.1,
                SCALE,
                &Overlay {
                    selection: Some(selection),
                    ..Overlay::default()
                },
            )
            .unwrap()
            .rgba_premultiplied();
        let metrics = book.header_metrics();
        let (band_x, band_y) = (
            (metrics.row_width * SCALE).ceil() as u32 + 2,
            (metrics.column_height * SCALE).ceil() as u32 + 2,
        );
        let mut expected = whole.clone();
        for y in 0..VIEWPORT.1 {
            for x in 0..VIEWPORT.0 {
                if x < band_x || y < band_y {
                    let start = ((y * VIEWPORT.0 + x) * 4) as usize;
                    expected[start..start + 4].copy_from_slice(&with_bands[start..start + 4]);
                }
            }
        }
        let share = differing(&pieces, &expected);
        assert!(
            share < 0.002,
            "{fixture} at {scroll:?}: {:.3}% of pixels differ",
            share * 100.0
        );
    }
}

#[test]
fn a_sheet_in_pieces_is_the_sheet_in_one_picture() {
    check(
        "grid-operations-workbook.xlsx",
        &[(0.0, 0.0), (37.5, 120.0), (300.0, 900.0)],
    );
}

#[test]
fn charts_cut_across_tiles_come_out_whole() {
    check("charts-workbook.xlsx", &[(0.0, 0.0), (120.5, 640.0)]);
}

#[test]
fn frozen_panes_are_windows_onto_the_same_tiles() {
    check("clipboard-workbook.xlsx", &[(0.0, 0.0), (210.0, 333.5)]);
}

/// The selection the shell draws is where the picture's is: its range and
/// active cell, merges taken in.
#[test]
fn the_selection_shape_is_the_painted_selection() {
    let book = open("grid-operations-workbook.xlsx");
    let selection = Selection {
        active: CellRef::new(1, 1),
        anchor: CellRef::new(1, 1),
        focus: CellRef::new(3, 4),
    };
    let ((x, y, width, height), (ax, ay, aw, ah)) = book.selection_shape(selection).unwrap();
    let first = book.cell_rect(CellRef::new(1, 1), (0.0, 0.0)).unwrap();
    let last = book.cell_rect(CellRef::new(3, 4), (0.0, 0.0)).unwrap();
    let metrics = book.header_metrics();
    assert!((x + metrics.row_width - first.0).abs() < 0.01);
    assert!((y + metrics.column_height - first.1).abs() < 0.01);
    assert!((x + width + metrics.row_width - (last.0 + last.2)).abs() < 0.01);
    assert!((y + height + metrics.column_height - (last.1 + last.3)).abs() < 0.01);
    assert_eq!((ax, ay, aw, ah), (x, y, first.2, first.3));
}

/// The tiles one edit leaves alone are the tiles a repaint would paint: what
/// `take_changes` reports covers every pixel the edit changed — the cell, a
/// formula reading it, text spilling over a neighbour — and resizing a
/// column is reported as anywhere.
#[test]
fn an_edit_reports_every_tile_it_changes() {
    let mut book = open("grid-operations-workbook.xlsx");
    let span = f64::from(TILE) / SCALE;
    let tiles: Vec<(i64, i64)> = (0..4)
        .flat_map(|row| (0..4).map(move |column| (column, row)))
        .collect();
    let paint = |book: &Workbook<TinySkiaBackend>, (column, row): (i64, i64)| {
        book.render_cells(
            (column as f64 * span, row as f64 * span, span, span),
            TILE,
            TILE,
            SCALE,
        )
        .unwrap()
        .rgba_premultiplied()
    };
    book.take_changes();
    for (cell, text) in [
        (CellRef::new(1, 1), "42"),
        (
            CellRef::new(0, 6),
            "a long line of text that runs over its neighbours",
        ),
        (CellRef::new(5, 30), "=B2*2"),
        (CellRef::new(0, 6), ""),
    ] {
        let before: Vec<_> = tiles.iter().map(|&tile| paint(&book, tile)).collect();
        book.set_cell(cell, text).unwrap();
        let changes = book.take_changes().expect("a typed cell is somewhere");
        assert!(!changes.is_empty(), "{text:?} changed nothing");
        for (&(column, row), before) in tiles.iter().zip(&before) {
            let (x, y) = (column as f64 * span, row as f64 * span);
            let touched = changes.iter().any(|&(cx, cy, cw, ch)| {
                cx < x + span && x < cx + cw && cy < y + span && y < cy + ch
            });
            if !touched {
                assert!(
                    &paint(&book, (column, row)) == before,
                    "typing {text:?} into {cell:?} changed tile {column},{row} unreported"
                );
            }
        }
    }
    book.resize(Axis::Columns, 1, 1, 160.0).unwrap();
    assert_eq!(
        book.take_changes(),
        None,
        "a wider column moves every cell right of it"
    );
}

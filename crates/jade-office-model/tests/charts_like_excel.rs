//! Charts as Excel works them: Insert ▸ Chart over the selection; a click
//! selects one, its border drags it and its handles resize it; Delete
//! removes it; Chart Design changes its type, title, legend and data. Each
//! is one undo step.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::charts::{
    dragged, handle_at, ChartChange, ChartKind, Handle, LegendPosition, CHART_KINDS,
};
use jade_office_model::sheet::{CellRange, CellRef, Workbook};
use jade_office_model::tiny::TinySkiaBackend;

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

/// Operations!A1:C4: Name, Amount, Double over three rows.
fn book() -> Workbook<TinySkiaBackend> {
    open("grid-operations-workbook.xlsx")
}

fn data() -> CellRange {
    CellRange {
        start: CellRef::new(0, 0),
        end: CellRef::new(2, 3),
    }
}

fn close(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1.0
        && (a.1 - b.1).abs() < 1.0
        && (a.2 - b.2).abs() < 1.0
        && (a.3 - b.3).abs() < 1.0
}

#[test]
fn insert_chart_puts_a_chart_of_the_selection_beside_it() {
    let mut book = book();
    let id = book.insert_chart(data(), ChartKind::Column).unwrap();
    let info = book.chart(id).expect("a chart");
    assert_eq!(info.kind, Some(ChartKind::Column));
    assert_eq!(info.title, "Chart Title");
    assert_eq!(info.legend, Some(LegendPosition::Bottom));
    assert_eq!(info.data.as_deref(), Some("A1:C4"));
    // Its top-left at E2.
    let bounds = book.object_bounds(id).unwrap();
    let (x, y, _, _) = book.cell_rect(CellRef::new(4, 1), (0.0, 0.0)).unwrap();
    let headers = book.header_metrics();
    assert!(
        (bounds.0 + headers.row_width - x).abs() < 1.0,
        "{bounds:?} {x}"
    );
    assert!(
        (bounds.1 + headers.column_height - y).abs() < 1.0,
        "{bounds:?} {y}"
    );
    assert!(book.is_dirty());
    assert!(book.undo().unwrap());
    assert!(book.chart(id).is_none());
    assert!(book.redo().unwrap());
    assert!(book.chart(id).is_some());
}

#[test]
fn a_click_on_a_chart_finds_it_and_its_border_and_handles_move_and_resize_it() {
    let mut book = book();
    let id = book.insert_chart(data(), ChartKind::Line).unwrap();
    let bounds = book.object_bounds(id).unwrap();
    let headers = book.header_metrics();
    // In the viewport, headers included.
    let rect = book.object_rect(id, (0.0, 0.0)).unwrap();
    assert!(close(
        rect,
        (
            bounds.0 + headers.row_width,
            bounds.1 + headers.column_height,
            bounds.2,
            bounds.3
        )
    ));
    let middle = (rect.0 + rect.2 / 2.0, rect.1 + rect.3 / 2.0);
    assert_eq!(book.object_at(middle.0, middle.1, (0.0, 0.0)), Some(id));
    assert_eq!(
        book.object_at(
            headers.row_width + 5.0,
            headers.column_height + 5.0,
            (0.0, 0.0)
        ),
        None
    );
    // The handles sit on its corners and edges.
    assert_eq!(
        handle_at(rect, rect.0 + rect.2, rect.1 + rect.3),
        Some(Handle::BottomRight)
    );
    assert_eq!(
        handle_at(rect, rect.0, rect.1 + rect.3 / 2.0),
        Some(Handle::Left)
    );
    assert_eq!(handle_at(rect, middle.0, middle.1), None);
    // Dragged by its border: moved, the same size.
    let moved = dragged(bounds, None, (64.0, 40.0));
    assert!(book.place_object(id, moved).unwrap());
    assert!(close(book.object_bounds(id).unwrap(), moved));
    // By its bottom-right handle: resized from the top-left.
    let resized = dragged(moved, Some(Handle::BottomRight), (100.0, 50.0));
    assert_eq!((resized.0, resized.1), (moved.0, moved.1));
    assert!(book.place_object(id, resized).unwrap());
    assert!(close(book.object_bounds(id).unwrap(), resized));
    // A handle dragged past the opposite edge leaves a small chart, not a
    // negative one.
    let tiny = dragged(resized, Some(Handle::Right), (-10_000.0, 0.0));
    assert!(tiny.2 >= 8.0);
    assert!(book.undo().unwrap());
    assert!(close(book.object_bounds(id).unwrap(), moved));
}

#[test]
fn delete_removes_a_chart_and_undo_brings_it_back() {
    let mut book = book();
    let id = book.insert_chart(data(), ChartKind::Pie).unwrap();
    assert!(book.delete_object(id).unwrap());
    assert!(book.chart(id).is_none());
    assert!(book.object_bounds(id).is_none());
    assert!(book.undo().unwrap());
    assert!(book.chart(id).is_some());
}

#[test]
fn chart_design_changes_type_title_legend_and_data() {
    let mut book = book();
    let id = book.insert_chart(data(), ChartKind::Column).unwrap();
    assert!(book
        .set_chart(
            id,
            &ChartChange {
                kind: Some(ChartKind::Line),
                title: Some(Some("Sales".into())),
                legend: Some(Some(LegendPosition::Right)),
                ..ChartChange::default()
            },
        )
        .unwrap());
    let info = book.chart(id).unwrap();
    assert_eq!(info.kind, Some(ChartKind::Line));
    assert_eq!(info.title, "Sales");
    assert_eq!(info.legend, Some(LegendPosition::Right));
    assert!(book
        .set_chart(
            id,
            &ChartChange {
                data: Some("A1:B4".into()),
                legend: Some(None),
                ..ChartChange::default()
            },
        )
        .unwrap());
    let info = book.chart(id).unwrap();
    assert_eq!(info.data.as_deref(), Some("A1:B4"));
    assert_eq!(info.legend, None);
    assert_eq!(info.title, "Sales");
    // A range that is no chart's says so.
    assert!(book
        .set_chart(
            id,
            &ChartChange {
                data: Some("nowhere".into()),
                ..ChartChange::default()
            },
        )
        .is_err());
    // Every kind the menu offers has words.
    assert_eq!(CHART_KINDS.len(), 10);
    assert!(CHART_KINDS
        .iter()
        .all(|(_, key)| key.starts_with("chart.kind.")));
}

#[test]
fn an_excel_chart_is_found_and_changed_too() {
    let mut book = open("charts-workbook.xlsx");
    let info = book.chart(1).expect("Columns");
    assert_eq!(info.kind, Some(ChartKind::Column));
    assert_eq!(info.title, "Quarter & Plan");
    assert_eq!(info.data.as_deref(), Some("A2:C5"));
    assert!(book
        .set_chart(
            1,
            &ChartChange {
                kind: Some(ChartKind::StackedColumn),
                ..ChartChange::default()
            }
        )
        .unwrap());
    assert_eq!(book.chart(1).unwrap().kind, Some(ChartKind::StackedColumn));
}

/// Chart Title… and Select Data… ask in a dialog filled in with what the
/// chart has; an emptied title is no title.
#[test]
fn the_text_dialog_asks_for_a_title_or_a_range() {
    use jade_office_model::charts::{ChartAsk, ChartField, ChartKey};
    let mut book = book();
    let id = book.insert_chart(data(), ChartKind::Column).unwrap();
    let info = book.chart(id).unwrap();
    let mut ask = ChartAsk::new(id, ChartField::Data, &info);
    assert_eq!(ask.text.text, "A1:C4");
    ask.text.replace(None, "A1:B4", false);
    assert_eq!(ask.key("x", false), ChartKey::Unhandled);
    assert_eq!(ask.key("enter", false), ChartKey::Take);
    assert!(book.set_chart(id, &ask.change()).unwrap());
    assert_eq!(book.chart(id).unwrap().data.as_deref(), Some("A1:B4"));
    let mut ask = ChartAsk::new(id, ChartField::Title, &book.chart(id).unwrap());
    assert_eq!(ask.text.text, "Chart Title");
    ask.text.replace(None, "", false);
    assert!(book.set_chart(id, &ask.change()).unwrap());
    assert_eq!(book.chart(id).unwrap().title, "");
    assert_eq!(ask.key("escape", false), ChartKey::Close);
}

#[test]
fn pivot_refresh_repaints_added_bars_and_restores_them_through_save_and_history() {
    use jade_office_model::{sheet::Overlay, Backend};
    use jade_xlsx_layout::{
        chart::{layout_chart, ChartOp},
        LayoutOptions, Rect,
    };
    use std::rc::Rc;
    fn painted(book: &mut Workbook<TinySkiaBackend>, backend: &TinySkiaBackend) -> usize {
        let bytes = book.save_bytes().unwrap();
        let parsed = jade_xlsx_reader::read_xlsx(&bytes).unwrap();
        let chart = parsed.sheets[1]
            .shapes
            .iter()
            .find_map(|s| s.chart.as_ref())
            .unwrap();
        let (x, y, width, height) = book.object_bounds(2).unwrap();
        let options = LayoutOptions {
            measure: Rc::new(jade_xlsx_render::PainterMeasure::new(
                backend.surface(1, 1).unwrap(),
            )),
            ..Default::default()
        }
        .with_normal_font_metrics(&parsed.styles);
        let placed = layout_chart(
            chart,
            Rect {
                x,
                y,
                width,
                height,
            },
            &parsed.sheets[1],
            Some(&parsed),
            &options,
            &parsed.styles,
        );
        let bars = placed
            .ops
            .iter()
            .filter_map(|op| match op {
                ChartOp::Rectangle {
                    rect,
                    fill: Some(fill),
                    ..
                } if ["#336699", "#ff9900"]
                    .iter()
                    .any(|c| fill.eq_ignore_ascii_case(c))
                    && rect.x >= placed.plot.x
                    && rect.right() <= placed.plot.right() + 0.01
                    && rect.y >= placed.plot.y
                    && rect.bottom() <= placed.plot.bottom() + 0.01 =>
                {
                    Some((*rect, fill.clone()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let headers = book.header_metrics();
        let painter = book
            .render_viewport((0., 0.), 1400, 700, 1., &Overlay::default())
            .unwrap();
        let (pixels, width, _) = TinySkiaBackend::bgra(&painter).unwrap();
        for (rect, fill) in &bars {
            let x = (headers.row_width + rect.x + rect.width / 2.).floor() as usize;
            let y = (headers.column_height + rect.y + rect.height / 2.).floor() as usize;
            let i = (y * width as usize + x) * 4;
            let rgb = [
                u8::from_str_radix(&fill[1..3], 16).unwrap(),
                u8::from_str_radix(&fill[3..5], 16).unwrap(),
                u8::from_str_radix(&fill[5..7], 16).unwrap(),
            ];
            assert_eq!(
                &pixels[i..i + 4],
                &[rgb[2], rgb[1], rgb[0], 255],
                "bar at {x},{y}"
            );
        }
        bars.len()
    }
    let path = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let backend = TinySkiaBackend::new(&aliased_faces(
        Arc::new(
            std::fs::read(path.join("fonts/Noto_Sans_JP/static/NotoSansJP-Regular.ttf")).unwrap(),
        ),
        None,
    ))
    .unwrap();
    let mut book = Workbook::open(
        include_bytes!("../../../jade-core/public/fixtures/pivot-chart-compat.xlsx"),
        "UTC",
        backend.clone(),
    )
    .unwrap();
    book.select_sheet(1).unwrap();
    assert_eq!(painted(&mut book, &backend), 2);
    book.select_sheet(0).unwrap();
    book.set_cell(CellRef::parse("A3").unwrap(), "North")
        .unwrap();
    book.select_sheet(1).unwrap();
    let before = book.save_bytes().unwrap();
    assert!(book
        .refresh_pivot_at(CellRef::parse("F4").unwrap())
        .unwrap());
    let saved = book.save_bytes().unwrap();
    assert_eq!(painted(&mut book, &backend), 3);
    let mut reopened = Workbook::open(&saved, "UTC", backend.clone()).unwrap();
    reopened.select_sheet(1).unwrap();
    assert_eq!(painted(&mut reopened, &backend), 3);
    assert!(book.undo().unwrap());
    assert_eq!(book.save_bytes().unwrap(), before);
    assert_eq!(painted(&mut book, &backend), 2);
    assert!(book.redo().unwrap());
    assert_eq!(book.save_bytes().unwrap(), saved);
    assert_eq!(painted(&mut book, &backend), 3);
}

#[test]
fn deleting_a_pivot_keeps_its_actual_chart_pixels_and_restores_binding_with_undo() {
    use jade_office_model::{sheet::Overlay, Backend};
    fn colors(b: &Workbook<TinySkiaBackend>) -> (usize, usize) {
        let p = b
            .render_viewport((0., 0.), 1400, 700, 1., &Overlay::default())
            .unwrap();
        let (pixels, _, _) = TinySkiaBackend::bgra(&p).unwrap();
        (
            pixels
                .chunks_exact(4)
                .filter(|p| *p == [153, 102, 51, 255])
                .count(),
            pixels
                .chunks_exact(4)
                .filter(|p| *p == [0, 153, 255, 255])
                .count(),
        )
    }
    let path = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let backend = TinySkiaBackend::new(&aliased_faces(
        Arc::new(
            std::fs::read(path.join("fonts/Noto_Sans_JP/static/NotoSansJP-Regular.ttf")).unwrap(),
        ),
        None,
    ))
    .unwrap();
    let mut package = jade_xlsx_reader::OpcPackage::open(include_bytes!(
        "../../../jade-core/public/fixtures/pivot-chart-compat.xlsx"
    ))
    .unwrap();
    let chart = package
        .required_part_str("xl/charts/pivot.xml")
        .unwrap()
        .replace("<c:v>15</c:v>", "<c:v>1500</c:v>");
    package.replace("xl/charts/pivot.xml", chart.into_bytes());
    let bytes = package.write().unwrap();
    let imported = jade_xlsx_reader::read_xlsx(&bytes).unwrap();
    assert_eq!(
        imported.sheets[1].shapes[0].chart.as_ref().unwrap().series[0]
            .values
            .points[0]
            .as_deref(),
        Some("1500")
    );
    let mut b = Workbook::open(&bytes, "UTC", backend).unwrap();
    b.select_sheet(1).unwrap();
    let before = b.save_bytes().unwrap();
    let expected = colors(&b);
    assert!(expected.0 > 100 && expected.1 > 100);
    assert!(b.delete_pivot_at(CellRef::parse("E3").unwrap()).unwrap());
    assert_eq!(colors(&b), expected);
    let saved = b.save_bytes().unwrap();
    let parsed = jade_xlsx_reader::read_xlsx(&saved).unwrap();
    assert!(parsed.pivots.tables.is_empty());
    assert!(parsed.sheets[1].shapes[0]
        .chart
        .as_ref()
        .unwrap()
        .pivot_source
        .is_none());
    assert!(b.undo().unwrap());
    assert_eq!(b.save_bytes().unwrap(), before);
    assert_eq!(colors(&b), expected);
    assert!(b.redo().unwrap());
    assert_eq!(b.save_bytes().unwrap(), saved);
    assert_eq!(colors(&b), expected);
}

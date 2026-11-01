// Shared native/browser: Trace Precedents' arrows over the grid.

/// One arrow, in px from the grid's top-left: from a precedent to the cell
/// that reads it, with the precedent's box when it is a range.
#[derive(Clone, Copy)]
struct TraceArrow {
    from: gpui::Point<gpui::Pixels>,
    to: gpui::Point<gpui::Pixels>,
    range: Option<gpui::Bounds<gpui::Pixels>>,
}

/// Excel's trace blue.
const TRACE_COLOR: u32 = 0x0000ff;

/// Paints `arrows` over the grid whose top-left is `origin`: a line with a
/// dot at the precedent and a head at the cell, and a box round a range.
fn paint_trace_arrows(
    window: &mut gpui::Window,
    origin: gpui::Point<gpui::Pixels>,
    arrows: &[TraceArrow],
) {
    use gpui::{point, px, PathBuilder};
    let color = gpui::rgb(TRACE_COLOR);
    for arrow in arrows {
        let from = origin + arrow.from;
        let to = origin + arrow.to;
        if let Some(range) = arrow.range {
            let range = gpui::Bounds {
                origin: origin + range.origin,
                size: range.size,
            };
            let mut outline = PathBuilder::stroke(px(1.5));
            outline.move_to(range.origin);
            outline.line_to(range.top_right());
            outline.line_to(range.bottom_right());
            outline.line_to(range.bottom_left());
            outline.close();
            if let Ok(path) = outline.build() {
                window.paint_path(path, color);
            }
        }
        let mut line = PathBuilder::stroke(px(1.5));
        line.move_to(from);
        line.line_to(to);
        if let Ok(path) = line.build() {
            window.paint_path(path, color);
        }
        // The dot at the precedent.
        let mut dot = PathBuilder::fill();
        dot.move_to(point(from.x - px(2.5), from.y - px(2.5)));
        dot.line_to(point(from.x + px(2.5), from.y - px(2.5)));
        dot.line_to(point(from.x + px(2.5), from.y + px(2.5)));
        dot.line_to(point(from.x - px(2.5), from.y + px(2.5)));
        dot.close();
        if let Ok(path) = dot.build() {
            window.paint_path(path, color);
        }
        // The head at the cell, pointing along the line.
        let (dx, dy) = (f32::from(to.x - from.x), f32::from(to.y - from.y));
        let length = (dx * dx + dy * dy).sqrt();
        if length > 1.0 {
            let (ux, uy) = (dx / length, dy / length);
            let back = point(to.x - px(ux * 8.0), to.y - px(uy * 8.0));
            let mut head = PathBuilder::fill();
            head.move_to(to);
            head.line_to(point(back.x - px(uy * 4.0), back.y + px(ux * 4.0)));
            head.line_to(point(back.x + px(uy * 4.0), back.y - px(ux * 4.0)));
            head.close();
            if let Ok(path) = head.build() {
                window.paint_path(path, color);
            }
        }
    }
}

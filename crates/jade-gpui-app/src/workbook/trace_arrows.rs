// Shared native/browser: Trace Precedents' arrows over the grid.

/// One arrow, in px from the grid's top-left: from a precedent to the cell
/// that reads it, with the precedent's box when it is a range — or, for
/// what it reads on other sheets, Excel's dashed black arrow from a
/// worksheet icon.
#[derive(Clone, Copy)]
struct TraceArrow {
    from: gpui::Point<gpui::Pixels>,
    to: gpui::Point<gpui::Pixels>,
    range: Option<gpui::Bounds<gpui::Pixels>>,
    other_sheet: bool,
    /// The worksheet icon at the head rather than the tail: a cell read by
    /// other sheets' formulas, as Trace Dependents shows it.
    icon_at_head: bool,
}

impl TraceArrow {
    /// The arrow into `to` from the worksheet icon up and to its left.
    fn from_other_sheet(to: gpui::Point<gpui::Pixels>) -> Self {
        use gpui::px;
        Self {
            from: gpui::point((to.x - px(48.)).max(px(10.)), (to.y - px(28.)).max(px(8.))),
            to,
            range: None,
            other_sheet: true,
            icon_at_head: false,
        }
    }

    /// The arrow from `from` to the worksheet icon down and to its right.
    fn to_other_sheet(from: gpui::Point<gpui::Pixels>) -> Self {
        use gpui::px;
        Self {
            from,
            to: gpui::point(from.x + px(48.), from.y + px(28.)),
            range: None,
            other_sheet: true,
            icon_at_head: true,
        }
    }
}

/// Excel's trace blue.
const TRACE_COLOR: u32 = 0x0000ff;
/// The black of an arrow from another sheet.
const OTHER_SHEET_COLOR: u32 = 0x000000;

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
        let color = if arrow.other_sheet {
            gpui::rgb(OTHER_SHEET_COLOR)
        } else {
            color
        };
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
        if arrow.other_sheet {
            line = line.dash_array(&[px(4.), px(3.)]);
        }
        line.move_to(from);
        line.line_to(to);
        if let Ok(path) = line.build() {
            window.paint_path(path, color);
        }
        if arrow.other_sheet {
            paint_sheet_icon(window, if arrow.icon_at_head { to } else { from }, color);
        } else {
            paint_dot(window, from, color);
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

/// The dot at a precedent on this sheet.
fn paint_dot(window: &mut gpui::Window, at: gpui::Point<gpui::Pixels>, color: gpui::Rgba) {
    use gpui::{point, px, PathBuilder};
    let mut dot = PathBuilder::fill();
    dot.move_to(point(at.x - px(2.5), at.y - px(2.5)));
    dot.line_to(point(at.x + px(2.5), at.y - px(2.5)));
    dot.line_to(point(at.x + px(2.5), at.y + px(2.5)));
    dot.line_to(point(at.x - px(2.5), at.y + px(2.5)));
    dot.close();
    if let Ok(path) = dot.build() {
        window.paint_path(path, color);
    }
}

/// Excel's worksheet icon where an arrow from another sheet starts: a
/// small white sheet with a grid, centred on `at`.
fn paint_sheet_icon(window: &mut gpui::Window, at: gpui::Point<gpui::Pixels>, color: gpui::Rgba) {
    use gpui::{point, px, size, Bounds, PathBuilder};
    let icon = Bounds::new(point(at.x - px(8.), at.y - px(6.)), size(px(16.), px(12.)));
    window.paint_quad(gpui::fill(icon, gpui::rgb(0xffffff)));
    let mut lines = PathBuilder::stroke(px(1.));
    lines.move_to(icon.origin);
    lines.line_to(icon.top_right());
    lines.line_to(icon.bottom_right());
    lines.line_to(icon.bottom_left());
    lines.close();
    for row in 1..3 {
        let y = icon.origin.y + px(4. * row as f32);
        lines.move_to(point(icon.origin.x, y));
        lines.line_to(point(icon.origin.x + icon.size.width, y));
    }
    let x = icon.origin.x + px(6.);
    lines.move_to(point(x, icon.origin.y));
    lines.line_to(point(x, icon.origin.y + icon.size.height));
    if let Ok(path) = lines.build() {
        window.paint_path(path, color);
    }
}

/// Data ▸ Circle Invalid Data: a red oval round each of `cells`, boxes in px
/// from the grid's top-left whose top-left is `origin`.
fn paint_invalid_circles(
    window: &mut gpui::Window,
    origin: gpui::Point<gpui::Pixels>,
    cells: &[gpui::Bounds<gpui::Pixels>],
) {
    use gpui::{point, px, PathBuilder};
    // A circle's quarter as a cubic: the control points this far along.
    const KAPPA: f32 = 0.552_284_8;
    for cell in cells {
        let (cx, cy) = (
            f32::from(origin.x + cell.center().x),
            f32::from(origin.y + cell.center().y),
        );
        let (rx, ry) = (
            f32::from(cell.size.width) / 2.0 + 4.0,
            f32::from(cell.size.height) / 2.0 + 3.0,
        );
        let (kx, ky) = (rx * KAPPA, ry * KAPPA);
        let at = |x: f32, y: f32| point(px(x), px(y));
        let mut oval = PathBuilder::stroke(px(1.5));
        oval.move_to(at(cx + rx, cy));
        oval.cubic_bezier_to(at(cx, cy + ry), at(cx + rx, cy + ky), at(cx + kx, cy + ry));
        oval.cubic_bezier_to(at(cx - rx, cy), at(cx - kx, cy + ry), at(cx - rx, cy + ky));
        oval.cubic_bezier_to(at(cx, cy - ry), at(cx - rx, cy - ky), at(cx - kx, cy - ry));
        oval.cubic_bezier_to(at(cx + rx, cy), at(cx + kx, cy - ry), at(cx + rx, cy - ky));
        if let Ok(path) = oval.build() {
            window.paint_path(path, gpui::rgb(0xff0000));
        }
    }
}

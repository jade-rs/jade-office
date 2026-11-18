//! Ribbons and toolbars that fit their window, as jade-elsa's responsive
//! ribbon does (`ui/src/responsive-ribbon.tsx` there):
//!
//! - every group is measured as it is drawn; when the row is wider than its
//!   room, groups fold, the rightmost first, into a button — the group's
//!   icon and name over a ▼ — that opens the group's controls under it;
//! - when even folded they do not fit, the row scrolls, with ‹ and › at its
//!   ends;
//! - a strip with no groups (the tab strip) only scrolls.
//!
//! Nothing is ever cut off: a control is in the row, in a folded group's
//! popup, or a scroll away.
//!
//! A strip is built with [`strip`]; a group inside it calls [`piece`], which
//! draws the group in full, or folded, as the last frame's measurements
//! decided. Groups built outside a strip are drawn in full.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use gpui::{
    anchored, canvas, div, point, prelude::*, px, svg, AnyElement, Div, ElementId, MouseButton,
    Pixels, ScrollHandle, SharedString, Size, Window,
};

use crate::layer::deferred;
use crate::tooltip::Tooltip;
use crate::{i18n, theme};

/// How a folded group is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compact {
    /// The icon over the name over a ▼, as tall as the ribbon: Word's.
    Stacked,
    /// The icon, the name and a ▼ on one line: a toolbar's.
    Inline,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Edges {
    overflow: bool,
    left: bool,
    right: bool,
}

/// What a strip remembers from frame to frame.
#[derive(Default)]
struct Strip {
    /// The groups in this frame's order, left to right.
    order: Vec<SharedString>,
    /// This frame's widths, as drawn.
    width: HashMap<SharedString, Pixels>,
    /// Each group's width in full and folded, the last time it was drawn so.
    full: HashMap<SharedString, Pixels>,
    folded: HashMap<SharedString, Pixels>,
    /// The tallest group in full, for a folded one to match.
    tallest: Pixels,
    row: Pixels,
    row_height: Pixels,
    /// How far this frame's spacers stretched: room, not content.
    spacers: Pixels,
    collapsed: HashSet<SharedString>,
    open: Option<SharedString>,
    scroll: ScrollHandle,
    edges: Edges,
    window: Size<Pixels>,
}

struct Building {
    key: SharedString,
    compact: Compact,
    seen: HashMap<SharedString, usize>,
    spacer: bool,
}

thread_local! {
    /// The window the last strip was drawn in, for lists to keep inside.
    static WINDOW: std::cell::Cell<Option<Size<Pixels>>> = const { std::cell::Cell::new(None) };
    static STRIPS: RefCell<HashMap<SharedString, Strip>> = RefCell::new(HashMap::new());
    static BUILDING: RefCell<Option<Building>> = const { RefCell::new(None) };
}

fn with_strip<R>(key: &SharedString, f: impl FnOnce(&mut Strip) -> R) -> R {
    STRIPS.with(|strips| f(strips.borrow_mut().entry(key.clone()).or_default()))
}

/// The space a popup may take: the window less a margin all round.
fn room(window: Size<Pixels>) -> Size<Pixels> {
    Size {
        width: (window.width - px(16.0)).max(px(120.0)),
        height: (window.height - px(16.0)).max(px(80.0)),
    }
}

/// A row of `build`'s groups (and anything else it returns, which never
/// folds) that fits its width. `row` is the row's own style — its gaps and
/// padding; `id` names the scrolling element, `key` the strip's memory, which
/// is kept per window: a ribbon tab passes its own, so each tab's groups
/// are measured apart.
pub fn strip(
    id: impl Into<ElementId>,
    key: impl Into<SharedString>,
    window: &Window,
    compact: Compact,
    row: Div,
    build: impl FnOnce() -> Vec<AnyElement>,
) -> AnyElement {
    let key: SharedString = format!(
        "{}/{}",
        window.window_handle().window_id().as_u64(),
        key.into()
    )
    .into();
    let viewport = window.viewport_size();
    WINDOW.with(|cell| cell.set(Some(viewport)));
    let (scroll, edges, row_height) = with_strip(&key, |strip| {
        strip.order.clear();
        strip.window = viewport;
        (strip.scroll.clone(), strip.edges, strip.row_height)
    });
    let outer = BUILDING.with(|building| {
        building.borrow_mut().replace(Building {
            key: key.clone(),
            compact,
            seen: HashMap::new(),
            spacer: false,
        })
    });
    let children = build();
    let spacer = BUILDING.with(|building| {
        std::mem::replace(&mut *building.borrow_mut(), outer).is_some_and(|b| b.spacer)
    });

    let measure_row = {
        let key = key.clone();
        canvas(
            move |bounds, _window, _cx| {
                with_strip(&key, |strip| {
                    strip.row = bounds.size.width;
                    strip.row_height = bounds.size.height;
                })
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    };
    // Drawn after the row, so every group has been measured by then.
    let measure_view = {
        let key = key.clone();
        canvas(
            move |bounds, window, _cx| fit(&key, bounds.size.width, window),
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    };
    let chevron_height = if row_height > px(0.0) {
        row_height
    } else {
        px(24.0)
    };
    div()
        .flex()
        .items_center()
        .flex_1()
        .min_w_0()
        .when(edges.overflow, |strip| {
            strip.child(chevron(&key, -1.0, edges.left, chevron_height))
        })
        .child(
            div()
                .id(id)
                .relative()
                .flex()
                .flex_1()
                .min_w_0()
                .overflow_x_scroll()
                .track_scroll(&scroll)
                .child(
                    row.relative()
                        .flex_none()
                        // At least as wide as the strip, for a `spacer` to
                        // push what follows it to the right (it takes all
                        // the slack, which is not counted as content).
                        .when(spacer, |row| row.min_w(gpui::relative(1.0)))
                        .children(children)
                        .child(measure_row),
                )
                .child(measure_view),
        )
        .when(edges.overflow, |strip| {
            strip.child(chevron(&key, 1.0, edges.right, chevron_height))
        })
        .into_any_element()
}

/// Decides, once the strip is drawn, which groups fold and whether it
/// scrolls; draws again when that changed. Elsa's `ResponsiveRibbonPanel`.
fn fit(key: &SharedString, viewport: Pixels, window: &mut Window) {
    let changed = with_strip(key, |strip| {
        for name in &strip.order {
            if let Some(&width) = strip.width.get(name) {
                if strip.collapsed.contains(name) {
                    strip.folded.insert(name.clone(), width);
                } else {
                    strip.full.insert(name.clone(), width);
                }
            }
        }
        let full = |strip: &Strip, name: &SharedString| {
            strip
                .full
                .get(name)
                .or_else(|| strip.width.get(name))
                .copied()
                .unwrap_or_default()
        };
        // The row as it would be with every group in full: what is not a
        // group (separators, gaps, padding) stays as it is.
        let mut required = strip.row - strip.spacers;
        strip.spacers = px(0.0);
        for name in &strip.order {
            required -= strip.width.get(name).copied().unwrap_or_default();
            required += full(strip, name);
        }
        let mut collapsed = HashSet::new();
        for name in strip.order.iter().rev() {
            if required <= viewport + px(0.5) {
                break;
            }
            let whole = full(strip, name);
            let folded = strip.folded.get(name).copied().unwrap_or(px(56.0));
            if whole <= folded {
                continue;
            }
            collapsed.insert(name.clone());
            required -= whole - folded;
        }
        let max = (strip.row - viewport).max(px(0.0));
        let mut offset = strip.scroll.offset();
        if -offset.x > max {
            offset.x = -max;
            strip.scroll.set_offset(offset);
        }
        let edges = Edges {
            overflow: max > px(1.0),
            left: -offset.x > px(1.0),
            right: -offset.x < max - px(1.0),
        };
        let changed = collapsed != strip.collapsed || edges != strip.edges;
        if strip
            .open
            .as_ref()
            .is_some_and(|open| !collapsed.contains(open))
        {
            strip.open = None;
        }
        strip.collapsed = collapsed;
        strip.edges = edges;
        strip.width.clear();
        changed
    });
    if changed {
        window.on_next_frame(|window, _cx| window.refresh());
    }
}

/// ‹ or ›: scrolls the strip most of its width.
fn chevron(key: &SharedString, direction: f32, enabled: bool, height: Pixels) -> AnyElement {
    let left = direction < 0.0;
    let key = key.clone();
    div()
        .id(if left {
            "fit-scroll-left"
        } else {
            "fit-scroll-right"
        })
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .w(px(22.0))
        .h(height)
        .rounded_sm()
        .border_1()
        .border_color(theme::border())
        .bg(theme::panel())
        .when(!enabled, |button| button.opacity(0.3))
        .when(enabled, |button| {
            button
                .cursor_pointer()
                .hover(|button| button.bg(theme::hovered()))
        })
        .tooltip(Tooltip::text(i18n::text(if left {
            "fit.scroll_left"
        } else {
            "fit.scroll_right"
        })))
        .child(
            svg()
                .path(if left {
                    "icons/chevron-left.svg"
                } else {
                    "icons/chevron-right.svg"
                })
                .size_4()
                .text_color(theme::text()),
        )
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |_, window, _cx| {
            with_strip(&key, |strip| {
                let viewport = strip.scroll.bounds().size.width;
                let max = (strip.row - viewport).max(px(0.0));
                let step = (viewport * 0.7).max(px(120.0));
                let mut offset = strip.scroll.offset();
                let scrolled = (-offset.x + step * direction).clamp(px(0.0), max);
                offset.x = -scrolled;
                strip.scroll.set_offset(offset);
            });
            window.refresh();
        })
        .into_any_element()
}

/// Room between the groups before it and those after it, which it pushes
/// to the strip's right end. Its stretch is not counted as content.
pub fn spacer() -> AnyElement {
    let key = BUILDING.with(|building| {
        building.borrow_mut().as_mut().map(|building| {
            building.spacer = true;
            building.key.clone()
        })
    });
    div()
        .flex_1()
        .relative()
        .when_some(key, |spacer, key| {
            spacer.child(
                canvas(
                    move |bounds, _window, _cx| {
                        with_strip(&key, |strip| strip.spacers += bounds.size.width)
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
        })
        .into_any_element()
}

/// One group of a strip, `content` being its controls. `full` draws it
/// whole: given the controls and an element that measures the group, which
/// it puts inside the group (it is absolutely placed, over the whole group,
/// so the group must be `relative`). Folded, the group is a button with
/// `icon` and `caption` that opens the controls in a popup.
pub fn piece(
    caption: impl Into<SharedString>,
    icon: &'static str,
    content: AnyElement,
    full: impl FnOnce(AnyElement, AnyElement) -> AnyElement,
) -> AnyElement {
    let caption: SharedString = caption.into();
    let building = BUILDING.with(|building| {
        building.borrow_mut().as_mut().map(|building| {
            // The same name twice in a strip is two groups.
            let seen = building.seen.entry(caption.clone()).or_default();
            let name: SharedString = if *seen == 0 {
                caption.clone()
            } else {
                format!("{caption}#{seen}").into()
            };
            *seen += 1;
            (building.key.clone(), building.compact, name)
        })
    });
    let Some((key, compact, name)) = building else {
        return full(content, div().absolute().into_any_element());
    };
    let (collapsed, open, tallest, window) = with_strip(&key, |strip| {
        strip.order.push(name.clone());
        (
            strip.collapsed.contains(&name),
            strip.open.as_ref() == Some(&name),
            strip.tallest,
            strip.window,
        )
    });
    let measure = {
        let key = key.clone();
        let name = name.clone();
        canvas(
            move |bounds, _window, _cx| {
                with_strip(&key, |strip| {
                    strip.width.insert(name.clone(), bounds.size.width);
                    if !collapsed {
                        strip.tallest = strip.tallest.max(bounds.size.height);
                    }
                })
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .into_any_element()
    };
    if !collapsed {
        return full(content, measure);
    }

    let toggle = {
        let key = key.clone();
        let name = name.clone();
        move |window: &mut Window| {
            with_strip(&key, |strip| {
                strip.open = if strip.open.as_ref() == Some(&name) {
                    None
                } else {
                    Some(name.clone())
                };
            });
            window.refresh();
        }
    };
    let button = div()
        .id(ElementId::Name(format!("fit-group-{name}").into()))
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .cursor_pointer()
        .whitespace_nowrap()
        .text_xs()
        .text_color(if open {
            theme::text()
        } else {
            theme::text_dim()
        })
        .when(open, |button| button.bg(theme::selected()))
        .hover(|button| button.bg(theme::hovered()).text_color(theme::text()))
        .tooltip(Tooltip::text(caption.to_string()))
        .tab_index(0)
        .focus(crate::ribbon::focus_ring)
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click({
            let toggle = toggle.clone();
            move |_, window, _cx| toggle(window)
        })
        .on_key_down({
            let toggle = toggle.clone();
            move |event, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space" | "down") {
                    cx.stop_propagation();
                    toggle(window);
                }
            }
        });
    let chevron = svg()
        .path("icons/chevron-down.svg")
        .size(px(10.0))
        .flex_none()
        .text_color(theme::text_dim());
    let button = match compact {
        Compact::Stacked => button
            .flex_col()
            .gap(px(3.0))
            .px(px(10.0))
            .py(px(6.0))
            .min_h(if tallest > px(0.0) { tallest } else { px(64.0) })
            .child(svg().path(icon).size(px(20.0)).text_color(theme::text()))
            .child(caption.clone())
            .child(chevron),
        Compact::Inline => button
            .gap_1()
            .h(px(24.0))
            .px_1p5()
            .child(svg().path(icon).size_3p5().text_color(theme::text()))
            .child(caption.clone())
            .child(chevron),
    };

    let popup = open.then(|| {
        let room = room(window);
        let panel = div()
            .id(ElementId::Name(format!("fit-popup-{name}").into()))
            .occlude()
            .mt_1()
            .p_3()
            .flex()
            .flex_col()
            .max_w(room.width)
            .max_h(room.height)
            .overflow_y_scroll()
            .rounded_lg()
            .bg(theme::panel())
            .border_1()
            .border_color(theme::border())
            .shadow_lg()
            .text_color(theme::text())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_key_down({
                let toggle = toggle.clone();
                move |event, window, cx| {
                    if event.keystroke.key == "escape" {
                        cx.stop_propagation();
                        toggle(window);
                    }
                }
            })
            .child(content);
        // A click anywhere else closes it, and goes no further.
        let backdrop = deferred(
            anchored().position(point(px(0.0), px(0.0))).child(
                div()
                    .id("fit-backdrop")
                    .w(window.width)
                    .h(window.height)
                    .occlude()
                    .on_mouse_down(MouseButton::Left, {
                        let toggle = toggle.clone();
                        move |_, window, cx| {
                            cx.stop_propagation();
                            toggle(window);
                        }
                    }),
            ),
        )
        .with_priority(1);
        div().absolute().top_full().left_0().child(backdrop).child(
            deferred(anchored().snap_to_window_with_margin(px(8.0)).child(panel)).with_priority(2),
        )
    });
    div()
        .relative()
        .flex_none()
        .child(button)
        .children(popup)
        .child(measure)
        .into_any_element()
}

/// Where the open list's button was and how tall the list is, as last
/// drawn: only one list is open at a time.
#[derive(Clone, Copy, Default)]
struct Drop {
    button: Option<gpui::Bounds<Pixels>>,
    natural: Pixels,
}

thread_local! {
    static DROP: std::cell::Cell<Drop> = const {
        std::cell::Cell::new(Drop { button: None, natural: Pixels::ZERO })
    };
}

/// A button with its list under it while open, as jade-elsa's ribbon drops
/// them (`ResponsiveRibbonDropdown`): the list opens below the button, or
/// above it when it does not fit below and there is more room above, and is
/// never taller than that room — it scrolls instead. Over everything else,
/// and inside a folded group's popup too.
pub fn drop_down(trigger: AnyElement, menu: Option<AnyElement>) -> AnyElement {
    let Some(menu) = menu else {
        return div()
            .relative()
            .flex_none()
            .child(trigger)
            .into_any_element();
    };
    let last = DROP.with(|drop| drop.get());
    let window = WINDOW.with(|cell| cell.get());
    let margin = px(10.0);
    // Below unless it does not fit there and there is more room above.
    let (down, space) = match (last.button, window) {
        (Some(button), Some(window)) => {
            let below = (window.height - button.bottom() - px(12.0)).max(px(0.0));
            let above = (button.top() - px(12.0)).max(px(0.0));
            let down = last.natural <= below || below >= above;
            (down, if down { below } else { above })
        }
        (None, Some(window)) => (true, room(window).height),
        _ => (true, px(10_000.0)),
    };
    let width = window.map_or(px(10_000.0), |window| room(window).width);
    // The list's own height, before it is cut to the room.
    let natural = canvas(
        |bounds, window, _cx| {
            let height = bounds.size.height;
            let changed = DROP.with(|drop| {
                let mut value = drop.get();
                let changed = (value.natural - height).abs() > px(1.0);
                value.natural = height;
                drop.set(value);
                changed
            });
            if changed {
                window.on_next_frame(|window, _cx| window.refresh());
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full();
    let list = div()
        .id("fit-list")
        .max_h(space + margin * 2.0)
        .max_w(width + margin * 2.0)
        .overflow_y_scroll()
        .p(margin)
        .ml(-margin)
        .when(down, |list| list.mt(-margin))
        .when(!down, |list| list.mb(-margin - px(8.0)))
        .child(div().relative().child(menu).child(natural));
    let button = canvas(
        |bounds, window, _cx| {
            let changed = DROP.with(|drop| {
                let mut value = drop.get();
                let changed = value.button != Some(bounds);
                value.button = Some(bounds);
                drop.set(value);
                changed
            });
            if changed {
                window.on_next_frame(|window, _cx| window.refresh());
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full();
    let anchor = if down {
        div().absolute().top_full().left_0()
    } else {
        div().absolute().top_0().left_0()
    };
    div()
        .relative()
        .flex_none()
        .child(trigger)
        .child(button)
        .child(
            anchor.child(
                deferred(
                    anchored()
                        .anchor(if down {
                            gpui::Corner::TopLeft
                        } else {
                            gpui::Corner::BottomLeft
                        })
                        .snap_to_window_with_margin(px(8.0))
                        .child(list),
                )
                .with_priority(2),
            ),
        )
        .into_any_element()
}

/// The icon a folded group shows: one for each of Word's groups that has a
/// fitting one, a generic one for the rest.
pub fn icon_for(caption: &str) -> &'static str {
    const ICONS: &[(&str, &str)] = &[
        ("doc.group.clipboard", "icons/files.svg"),
        ("doc.group.font", "icons/type.svg"),
        ("doc.group.paragraph", "icons/align-left.svg"),
        ("word.group.lists", "icons/list.svg"),
        ("doc.group.styles", "icons/paintbrush.svg"),
        ("word.group.style_tools", "icons/palette.svg"),
        ("doc.group.editing", "icons/settings-2.svg"),
        ("doc.group.table", "icons/table.svg"),
        ("table.group.table", "icons/table.svg"),
        ("table.group.styles", "icons/table.svg"),
        ("table.group.style_options", "icons/table.svg"),
        ("table.group.borders", "icons/borders.svg"),
        ("table.group.draw", "icons/borders.svg"),
        ("table.group.cells", "icons/merge.svg"),
        ("ribbon.group.rows", "icons/arrow-down-to-line.svg"),
        ("ribbon.group.columns", "icons/arrow-right-to-line.svg"),
        ("ribbon.group.merge", "icons/merge.svg"),
        ("ribbon.group.table_format", "icons/table.svg"),
        ("doc.group.pages", "icons/file-plus.svg"),
        ("page.group.setup", "icons/file.svg"),
        ("page.group.header_footer", "icons/panels-top-left.svg"),
        ("setup.group.background", "icons/paint-bucket.svg"),
        ("doc.group.tracking", "icons/file-text.svg"),
        ("review.group.changes", "icons/check.svg"),
        ("doc.group.zoom", "icons/chevrons-up-down.svg"),
        ("doc.group.show", "icons/ruler.svg"),
        ("views.group.views", "icons/panels-top-left.svg"),
        ("views.group.window", "icons/panel-right.svg"),
        ("objects.group.insert_shapes", "icons/shapes.svg"),
        ("objects.group.shape_styles", "icons/shapes.svg"),
        ("objects.group.picture_styles", "icons/image.svg"),
        ("objects.group.adjust", "icons/image.svg"),
        ("objects.group.chart", "icons/chart-column.svg"),
        ("draw.group.tools", "icons/highlighter.svg"),
        ("equation.group.symbols", "icons/sparkles.svg"),
        ("equation.group.structures", "icons/sparkles.svg"),
    ];
    ICONS
        .iter()
        .find(|(key, _)| i18n::text(key) == caption)
        .map_or("icons/blocks.svg", |(_, icon)| icon)
}

#[cfg(test)]
mod tests {
    use super::{Strip, STRIPS};
    use gpui::{px, size, SharedString, VisualTestContext};
    use jade_office_model::word::new::{blank_document, Blank};

    /// The strip whose memory is kept under a key ending `suffix`.
    fn strip<R>(suffix: &str, f: impl FnOnce(&mut Strip) -> R) -> R {
        STRIPS.with(|strips| {
            let mut strips = strips.borrow_mut();
            let (_, strip) = strips
                .iter_mut()
                .find(|(key, _)| key.ends_with(suffix))
                .expect("the strip was drawn");
            f(strip)
        })
    }

    /// Draws until the strips have measured and refitted.
    fn settle(cx: &mut VisualTestContext) {
        for _ in 0..4 {
            cx.update(|window, _| window.refresh());
            cx.run_until_parked();
        }
    }

    /// As in jade-elsa: a narrow window folds the ribbon's groups, the
    /// rightmost first, and widening it unfolds them; a folded group opens
    /// in a popup, and a list inside it (the font family's) drops from it —
    /// a layer inside a layer, which gpui's own `deferred` cannot draw.
    #[gpui::test]
    fn a_narrow_ribbon_folds_its_groups_and_opens_them(cx: &mut gpui::TestAppContext) {
        let bytes = blank_document(Blank::Japanese);
        let (view, cx) = cx
            .add_window_view(|_window, cx| crate::untitled_view(&bytes, "test.docx", cx).unwrap());
        const HOME: &str = "word-groups/Home";
        cx.simulate_resize(size(px(2400.0), px(900.0)));
        settle(cx);
        assert!(strip(HOME, |strip| strip.collapsed.is_empty()));

        cx.simulate_resize(size(px(640.0), px(700.0)));
        settle(cx);
        let font = SharedString::from(crate::i18n::text("doc.group.font"));
        let (order, collapsed) =
            strip(HOME, |strip| (strip.order.clone(), strip.collapsed.clone()));
        assert!(collapsed.contains(&font), "{collapsed:?}");
        assert!(collapsed.contains(order.last().unwrap()));
        // Folded from the right: no group in full right of a folded one,
        // but for one no wider in full than folded.
        let first = order
            .iter()
            .position(|name| collapsed.contains(name))
            .unwrap();
        assert!(
            order[first..]
                .iter()
                .filter(|name| !collapsed.contains(*name))
                .count()
                <= 1
        );

        strip(HOME, |strip| strip.open = Some(font.clone()));
        view.update(cx, |view, cx| {
            view.ribbon.toggle(crate::ribbon::Popup::FontFamily);
            cx.notify();
        });
        settle(cx);
        assert_eq!(strip(HOME, |strip| strip.open.clone()), Some(font.clone()));

        cx.simulate_resize(size(px(2400.0), px(900.0)));
        settle(cx);
        assert!(strip(HOME, |strip| strip.collapsed.is_empty()
            && strip.open.is_none()));
    }
}

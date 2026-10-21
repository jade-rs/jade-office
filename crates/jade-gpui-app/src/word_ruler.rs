//! The rulers (ルーラー), as Word draws them over and beside the page: the
//! margins in grey, ticks and numbers in the chosen unit (mm, cm, inches,
//! points or 字), the caret paragraph's indent markers — 1 行目のインデント
//! on top, ぶら下げインデント and 左インデント below, 右インデント at the
//! right — and its tab stops.
//!
//! On the horizontal ruler a click between the margins sets a tab stop of
//! the kind the selector at its left shows (click the selector for the
//! next kind); a tab stop dragged along moves, dragged off the ruler goes;
//! an indent marker dragged sets that indent. The vertical ruler beside the
//! caret's page shows its top and bottom margins.

use gpui::{
    canvas, div, prelude::*, px, rgb, AnyElement, Context, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent,
};
use jade_office_model::word::page::WordPage;
use jade_office_model::word::paragraph_options::{
    RulerIndents, Tab, TabAlignment, TabLeader, WordParagraphOptions,
};
use jade_office_model::word::ruler::{snap, ticks, RulerUnit};

use crate::ribbon::{dropdown, menu_panel, menu_row, Action, Popup};
use crate::word_dialog::{on, on_event};
use crate::word_tools::{ToolCommand, ToolMenu};
use crate::{i18n, theme, DocumentView};

/// The ruler's height on screen.
const RULER_HEIGHT: f32 = 20.0;
/// How near (points) a click must be to grab a marker.
const GRAB: f32 = 4.0;

/// What is being dragged along the ruler, and where it is now (points
/// from the page's left edge).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RulerDrag {
    pub what: Dragged,
    pub at: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dragged {
    /// A tab stop, by its position from the margin.
    Tab(Tab),
    FirstLine,
    Hanging,
    Left,
    Right,
}

fn store() -> Option<std::path::PathBuf> {
    if cfg!(test) {
        return None;
    }
    let home = std::env::var_os("HOME")?;
    Some(
        std::path::PathBuf::from(home)
            .join("Library/Application Support/jade-office")
            .join("ruler-unit.txt"),
    )
}

/// The unit chosen last time; 字, as Japanese Word starts with, else.
pub fn load_unit() -> RulerUnit {
    store()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| RulerUnit::from_key(&text))
        .unwrap_or(match i18n::language() {
            i18n::Language::Japanese => RulerUnit::Characters,
            i18n::Language::English => RulerUnit::Inches,
        })
}

pub fn save_unit(unit: RulerUnit) {
    let Some(path) = store() else { return };
    if let Some(directory) = path.parent() {
        std::fs::create_dir_all(directory).ok();
    }
    std::fs::write(path, unit.key()).ok();
}

/// The glyph a tab stop of `alignment` is drawn with, as Word's.
pub fn tab_glyph(alignment: TabAlignment) -> &'static str {
    match alignment {
        TabAlignment::Left => "└",
        TabAlignment::Center => "┴",
        TabAlignment::Right => "┘",
        TabAlignment::Decimal => "┴.",
        TabAlignment::Bar => "│",
    }
}

pub fn unit_label(unit: RulerUnit) -> &'static str {
    i18n::text(match unit {
        RulerUnit::Millimeters => "word.unit.mm",
        RulerUnit::Centimeters => "word.unit.cm",
        RulerUnit::Inches => "word.unit.inch",
        RulerUnit::Points => "word.unit.pt",
        RulerUnit::Characters => "word.unit.char",
    })
}

/// The page box the rulers measure: its size and margins, in points.
struct Measure {
    width: f32,
    height: f32,
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
}

impl DocumentView {
    fn measure(&self) -> Measure {
        let setup = self.document.page_setup();
        let (width, height) = self.document.layout_page_size();
        Measure {
            width: width as f32,
            height: height as f32,
            left: setup.margins.left,
            right: setup.margins.right,
            top: setup.margins.top,
            bottom: setup.margins.bottom,
        }
    }

    /// Points from the page's left edge under a window x.
    fn ruler_point(&self, x: gpui::Pixels) -> Option<f32> {
        let bounds = self.tools.ruler_bounds.get()?;
        Some(f32::from(x - bounds.origin.x) / self.zoom)
    }

    /// What a press on the ruler at `at` (points from the page edge) grabs,
    /// `down` how far down the ruler (0 top, 1 bottom): the top marker is
    /// the first line's, the triangle below the hanging indent's and the
    /// square under it the left indent's, which moves both.
    fn ruler_grab(&mut self, at: f32, down: f32) -> Option<Dragged> {
        let upper = down < 0.5;
        let measure = self.measure();
        let char_width = self.char_width();
        let indents = self.document.ruler_indents(char_width);
        let left_edge = measure.left + indents.left;
        let first = left_edge + indents.first_line;
        let right_edge = measure.width - measure.right - indents.right;
        if upper && (at - first).abs() <= GRAB {
            return Some(Dragged::FirstLine);
        }
        if !upper && (at - left_edge).abs() <= GRAB {
            return Some(if down > 0.8 {
                Dragged::Left
            } else {
                Dragged::Hanging
            });
        }
        if (at - right_edge).abs() <= GRAB {
            return Some(Dragged::Right);
        }
        self.document
            .paragraph_options()
            .tabs
            .into_iter()
            .find(|tab| (measure.left + tab.position - at).abs() <= GRAB)
            .map(Dragged::Tab)
    }

    fn ruler_down(&mut self, event: &MouseDownEvent, cx: &mut Context<Self>) {
        let Some(at) = self.ruler_point(event.position.x) else {
            return;
        };
        self.tools_caret();
        let down = self.tools.ruler_bounds.get().map_or(0.0, |bounds| {
            f32::from(event.position.y - bounds.origin.y) / f32::from(bounds.size.height).max(1.0)
        });
        if let Some(what) = self.ruler_grab(at, down) {
            self.tools.ruler_drag = Some(RulerDrag { what, at });
            return cx.notify();
        }
        // A click between the margins sets a tab stop there.
        let measure = self.measure();
        if at <= measure.left || at >= measure.width - measure.right {
            return;
        }
        let char_width = self.char_width();
        let position = snap(self.tools.unit, char_width, at - measure.left).max(0.0);
        let tab = Tab::new(position, self.tools.tab_kind, TabLeader::None);
        let result = self.document.set_tab(tab);
        self.tool_done(result, cx);
    }

    fn ruler_move(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(drag) = self.tools.ruler_drag else {
            return;
        };
        if !event.dragging() {
            return;
        }
        if let Some(at) = self.ruler_point(event.position.x) {
            self.tools.ruler_drag = Some(RulerDrag { at, ..drag });
            cx.notify();
        }
    }

    /// The drag ends: on the ruler it sets what was dragged; off it, a tab
    /// stop is cleared.
    fn ruler_up(&mut self, inside: bool, cx: &mut Context<Self>) {
        let Some(drag) = self.tools.ruler_drag.take() else {
            return;
        };
        let measure = self.measure();
        let char_width = self.char_width();
        let unit = self.tools.unit;
        let from_margin = snap(unit, char_width, drag.at - measure.left);
        let indents = self.document.ruler_indents(char_width);
        let result = match drag.what {
            Dragged::Tab(tab) if !inside => self.document.move_tab(tab.position, None),
            Dragged::Tab(tab) => {
                if (from_margin - tab.position).abs() < 0.5 {
                    return cx.notify();
                }
                self.document
                    .move_tab(tab.position, Some(from_margin.max(0.0)))
            }
            Dragged::FirstLine => self.document.set_ruler_indents(RulerIndents {
                first_line: from_margin - indents.left,
                ..indents
            }),
            Dragged::Hanging => {
                // The first line stays where it is on the page.
                let first = indents.left + indents.first_line;
                self.document.set_ruler_indents(RulerIndents {
                    left: from_margin,
                    first_line: first - from_margin,
                    ..indents
                })
            }
            Dragged::Left => self.document.set_ruler_indents(RulerIndents {
                left: from_margin,
                ..indents
            }),
            Dragged::Right => {
                let right_edge = snap(unit, char_width, measure.width - measure.right - drag.at);
                self.document.set_ruler_indents(RulerIndents {
                    right: right_edge,
                    ..indents
                })
            }
        };
        self.tool_done(result, cx);
    }

    /// The horizontal ruler, over the pages.
    pub(crate) fn horizontal_ruler(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.tools.ruler {
            return None;
        }
        let measure = self.measure();
        let zoom = self.zoom;
        let unit = self.tools.unit;
        let char_width = self.char_width();
        let indents = self.document.ruler_indents(char_width);
        let tabs = self.document.paragraph_options().tabs;
        let drag = self.tools.ruler_drag;
        let x = |points: f32| px(points * zoom);
        let mut strip = div()
            .id("ruler-strip")
            .relative()
            .flex_none()
            .w(x(measure.width))
            .h(px(RULER_HEIGHT))
            .bg(theme::sidebar())
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                on_event(cx, |view, event: &MouseDownEvent, _, cx| {
                    view.ruler_down(event, cx)
                }),
            )
            .on_mouse_move(on_event(cx, |view, event: &MouseMoveEvent, _, cx| {
                view.ruler_move(event, cx)
            }))
            .on_mouse_up(
                MouseButton::Left,
                on_event(cx, |view, _: &MouseUpEvent, _, cx| view.ruler_up(true, cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                on_event(cx, |view, _: &MouseUpEvent, _, cx| view.ruler_up(false, cx)),
            )
            // The writing area in white, the margins left grey.
            .child(
                div()
                    .absolute()
                    .top(px(2.0))
                    .left(x(measure.left))
                    .w(x(measure.width - measure.left - measure.right))
                    .h(px(RULER_HEIGHT - 4.0))
                    .bg(theme::paper()),
            );
        for tick in ticks(
            unit,
            char_width,
            -measure.left,
            measure.width - measure.left,
        ) {
            let at = x(measure.left + tick.at);
            strip = match tick.label {
                Some(label) => strip.child(
                    div()
                        .absolute()
                        .top(px(3.0))
                        .left(at - px(10.0))
                        .w(px(20.0))
                        .flex()
                        .justify_center()
                        .text_size(px(9.0))
                        .text_color(rgb(0x606060))
                        .child(label),
                ),
                None => {
                    let length = if tick.half { 6.0 } else { 3.0 };
                    strip.child(
                        div()
                            .absolute()
                            .top(px((RULER_HEIGHT - length) / 2.0))
                            .left(at)
                            .w(px(1.0))
                            .h(px(length))
                            .bg(rgb(0x808080)),
                    )
                }
            };
        }
        let marker = |at: f32, top: bool, glyph: &'static str| {
            div()
                .absolute()
                .when(top, |marker| marker.top(px(-3.0)))
                .when(!top, |marker| marker.top(px(RULER_HEIGHT - 11.0)))
                .left(x(at) - px(4.0))
                .w(px(9.0))
                .flex()
                .justify_center()
                .text_size(px(10.0))
                .text_color(theme::accent())
                .child(glyph)
        };
        let dragged = |what: Dragged| drag.filter(|drag| drag.what == what).map(|drag| drag.at);
        let left_edge = measure.left + indents.left;
        let first = dragged(Dragged::FirstLine).unwrap_or(left_edge + indents.first_line);
        let hanging = dragged(Dragged::Hanging).unwrap_or(left_edge);
        let right =
            dragged(Dragged::Right).unwrap_or(measure.width - measure.right - indents.right);
        strip = strip
            .child(marker(first, true, "▼"))
            .child(marker(hanging, false, "▲"))
            .child(marker(right, false, "▲"));
        for tab in tabs {
            let at = dragged(Dragged::Tab(tab)).unwrap_or(measure.left + tab.position);
            strip = strip.child(
                div()
                    .absolute()
                    .top(px(RULER_HEIGHT - 12.0))
                    .left(x(at) - px(1.0))
                    .text_size(px(11.0))
                    .text_color(rgb(0x202020))
                    .child(tab_glyph(tab.alignment)),
            );
        }
        let bounds = std::rc::Rc::clone(&self.tools.ruler_bounds);
        strip = strip.child(
            canvas(
                move |area, _window, _cx| {
                    bounds.set(Some(area));
                    area
                },
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        );
        let selector = div()
            .id("ruler-tab-kind")
            .absolute()
            .left(px(8.0))
            .top(px(2.0))
            .size(px(18.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_sm()
            .border_1()
            .border_color(theme::border())
            .bg(theme::panel())
            .cursor_pointer()
            .text_size(px(11.0))
            .tooltip(crate::tooltip::Tooltip::text(i18n::text(
                "word.ruler.tab_kind",
            )))
            .on_click(on(cx, |view, cx| {
                view.run_tool(ToolCommand::NextTabKind, cx)
            }))
            .child(tab_glyph(self.tools.tab_kind));
        Some(
            div()
                .id("ruler")
                .relative()
                .flex()
                .flex_none()
                .justify_center()
                .w_full()
                .py(px(2.0))
                .bg(theme::panel())
                .border_b_1()
                .border_color(theme::border())
                .child(selector)
                .child(strip)
                .into_any_element(),
        )
    }

    /// The vertical ruler, beside page `index` when the caret is on it.
    pub(crate) fn vertical_ruler(&self, index: usize) -> Option<AnyElement> {
        if !self.tools.ruler || index + 1 != self.caret_page_1idx() {
            return None;
        }
        let measure = self.measure();
        let zoom = self.zoom;
        let unit = self.tools.unit;
        let char_width = self
            .format
            .as_ref()
            .and_then(|(_, state)| state.font_size)
            .unwrap_or(10.5);
        // 字 down a page is a line: Word measures the vertical ruler in lines
        // there; a line is the character's height and a half.
        let (unit, char_width) = match unit {
            RulerUnit::Characters => (RulerUnit::Characters, char_width * 1.5),
            unit => (unit, char_width),
        };
        let y = |points: f32| px(points * zoom);
        let mut ruler = div()
            .absolute()
            .top_0()
            .left(px(-24.0))
            .w(px(18.0))
            .h(y(measure.height))
            .bg(theme::sidebar())
            .child(
                div()
                    .absolute()
                    .left(px(2.0))
                    .w(px(14.0))
                    .top(y(measure.top))
                    .h(y(measure.height - measure.top - measure.bottom))
                    .bg(theme::paper()),
            );
        for tick in ticks(unit, char_width, -measure.top, measure.height - measure.top) {
            let at = y(measure.top + tick.at);
            ruler = match tick.label {
                Some(label) => ruler.child(
                    div()
                        .absolute()
                        .left_0()
                        .w(px(18.0))
                        .top(at - px(6.0))
                        .flex()
                        .justify_center()
                        .text_size(px(8.0))
                        .text_color(rgb(0x606060))
                        .child(label),
                ),
                None => {
                    let length = if tick.half { 6.0 } else { 3.0 };
                    ruler.child(
                        div()
                            .absolute()
                            .left(px((18.0 - length) / 2.0))
                            .top(at)
                            .h(px(1.0))
                            .w(px(length))
                            .bg(rgb(0x808080)),
                    )
                }
            };
        }
        Some(ruler.into_any_element())
    }

    /// 表示 → ルーラー: on and off, and its unit.
    pub(crate) fn ruler_group(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let open = self.ribbon.popup == Some(Popup::Word(ToolMenu::RulerUnits));
        let unit = self.tools.unit;
        let menu = open.then(|| {
            menu_panel("ruler-units-menu")
                .w(px(160.0))
                .children(
                    RulerUnit::ALL
                        .into_iter()
                        .enumerate()
                        .map(|(index, entry)| {
                            menu_row(
                                ("ruler-unit", index),
                                unit_label(entry),
                                entry == unit,
                                Action::Word(ToolCommand::RulerUnit(entry)),
                                cx,
                            )
                        }),
                )
                .into_any_element()
        });
        let ruler = self.tools.ruler;
        crate::ribbon::group(
            i18n::text("word.ruler.group"),
            vec![crate::ribbon::row(vec![
                div()
                    .id("toggle-ruler")
                    .flex()
                    .items_center()
                    .h(px(26.0))
                    .px_2()
                    .rounded_md()
                    .text_xs()
                    .cursor_pointer()
                    .when(ruler, |button| button.bg(theme::selected()))
                    .hover(|button| button.bg(theme::hovered()))
                    .on_click(on(cx, |view, cx| {
                        view.run_tool(ToolCommand::ToggleRuler, cx)
                    }))
                    .child(i18n::text("word.ruler.show"))
                    .into_any_element(),
                dropdown(
                    crate::word_page::list_button(
                        "ruler-units",
                        unit_label(unit),
                        open,
                        Popup::Word(ToolMenu::RulerUnits),
                        cx,
                    ),
                    menu,
                ),
            ])],
        )
    }
}
